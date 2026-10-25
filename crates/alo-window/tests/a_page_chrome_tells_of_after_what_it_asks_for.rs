/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A request a new document makes is that document's, even when Chrome
//! pauses it before this browser has read that the document arrived: queue
//! items 647 and 656, ADR 0060's amendment of 25 October, ADR 0012. And a
//! request the old document makes as it is left is the old document's, even
//! when Chrome pauses it after: queue item 657, ADR 0060's second amendment.
//!
//! Chrome pauses a request in its browser process and tells of a commit from
//! the page's renderer, and under load the first can reach this browser
//! before the second — measured against Chrome 155, ten times in a hundred
//! navigations under 96 busy processes, and the cause of the gate's
//! failures in `every_request_of_a_page_in_chrome_recorded.rs`. Waiting for a
//! busy machine to do it again is a test that fails one run in forty.
//!
//! So the order is made, not waited for. The Chrome this test starts is the
//! machine's own behind a relay: this test's own program, run again as the
//! far side of the pipe, which hands every message on as Chrome sent it but
//! one — the commit of the document named, and everything after it through
//! the same session, is held back until Chrome has paused the request
//! named, which goes ahead of it. That is the order Chrome was measured to
//! send under load, with nothing altered but when this browser reads it.
//! The other way round, a paused request named is held back, alone, until
//! the commit named has gone ahead of it.
//!
//! - The tab's own frame: an agent follows a link to a page with a style
//!   sheet, whose commit is held until the sheet is paused. The sheet is the
//!   new page's, which the agent's action brought — not the agent's action
//!   in the page it left (item 647).
//! - A frame from another site goes to a second page of its own, which
//!   fetches, and its commit is held until the fetch is paused. The fetch is
//!   the frame's second document's, not its first's (item 656).
//! - A page sends itself to another, and sends a beacon as it is hidden,
//!   held back until the new page's commit is read. The beacon is the page
//!   left's, not the page arrived's (item 657): Chrome names no request id
//!   for it, as for every request of a page being left, measured against
//!   Chrome 155 — and after the commit, every time, when the page arrived
//!   is from another site.

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_net::activity::Entry;
use alo_net::request::Purpose;
use alo_net::{Cause, Place};
use alo_window::beside::{chrome_in, chrome_places};
use alo_window::chrome::Chrome;
use alo_window::chrome_protocol::{self, Message};
use alo_window::chrome_tab::Tab;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size every page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what the page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// Set, in the program run again as the far side: the machine's Chrome.
const FAR_SIDE_OF: &str = "ALO_FAR_SIDE_OF";

/// Set beside it: the directory the far side was made in, which holds its
/// arguments and where it marks that it held a commit back.
const FAR_SIDE_IN: &str = "ALO_FAR_SIDE_IN";

/// Set beside it: what is held — a `commit` or a paused `request` — and
/// what it is held behind — the other — each as the end of an address, as
/// `commit /next.html behind /next.css` or `request /beacon behind
/// /left-for.html`.
const FAR_SIDE_HOLDS: &str = "ALO_FAR_SIDE_HOLDS";

/// What the server received: each request's path.
type Log = Arc<Mutex<Vec<String>>>;

/// The server's answer to `path`, on port `port`.
fn answer(path: &str, port: u16) -> (&'static str, String) {
    let html = "text/html; charset=utf-8";
    match path {
        "/page.html" => (
            html,
            "<!doctype html><link rel=stylesheet href=\"/first.css\">\
             <a href=\"/next.html\">Next</a>"
                .to_owned(),
        ),
        "/next.html" => (
            html,
            "<!doctype html><link rel=stylesheet href=\"/next.css\"><h1>Second</h1>".to_owned(),
        ),
        "/framed.html" => (
            html,
            format!("<!doctype html><iframe src=\"http://localhost:{port}/other.html\"></iframe>"),
        ),
        "/other.html" => (
            html,
            "<!doctype html><script>location = '/other2.html'</script>".to_owned(),
        ),
        "/other2.html" => (
            html,
            "<!doctype html><script>fetch('/from-other2')</script>".to_owned(),
        ),
        "/leaving.html" => (
            html,
            "<!doctype html><script>addEventListener('pagehide', () => {\
             navigator.sendBeacon('/beacon', 'x'); fetch('/kept', {keepalive: true}) });\
             setTimeout(() => location = '/left-for.html', 300)</script>"
                .to_owned(),
        ),
        "/left-for.html" => (
            html,
            "<!doctype html><link rel=stylesheet href=\"/left-for.css\"><h1>Arrived</h1>"
                .to_owned(),
        ),
        _ => ("text/css", String::new()),
    }
}

/// A server on this machine: its port, and what it received.
fn serve() -> Option<(u16, Log)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let log: Log = Arc::default();
    let logging = Arc::clone(&log);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let logging = Arc::clone(&logging);
            std::thread::spawn(move || {
                if socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .is_err()
                {
                    return;
                }
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let path = asked.split(' ').nth(1).unwrap_or_default().to_owned();
                // A connection Chrome opened ahead of need and closed unused
                // asked for nothing.
                if path.is_empty() {
                    return;
                }
                if let Ok(mut log) = logging.lock() {
                    log.push(path.clone());
                }
                let (kind, body) = answer(&path, port);
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Some((port, log))
}

/// The tab read again and again — each read answering every request paused
/// meanwhile — until the server has received `awaited`.
fn read_until(tab: &mut Tab<'_>, log: &Log, awaited: &str) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    loop {
        tab.read().map_err(|why| why.to_string())?;
        if log
            .lock()
            .is_ok_and(|log| log.iter().any(|path| path == awaited))
        {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!("never received {awaited}: {log:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The one line for `url`, or what the lines were.
fn line<'r>(lines: &[&'r Entry], url: &str) -> Result<&'r Entry, String> {
    match lines
        .iter()
        .filter(|entry| entry.url().to_string() == url)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [only] => Ok(only),
        found => Err(format!("{} lines for {url}: {lines:#?}", found.len())),
    }
}

/// A far side made for the test `test`, in a directory of its own, that
/// holds back the commit `holds` names: the program to start as Chrome.
fn far_side(test: &str, holds: &str) -> Result<FarSide, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let chrome = chrome_in(&chrome_places(home.as_deref())).map_err(|why| why.to_string())?;
    let me = std::env::current_exe().map_err(|why| why.to_string())?;
    let directory =
        std::env::temp_dir().join(format!("alo-far-side-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).map_err(|why| why.to_string())?;
    let quoted = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', r"'\''"));
    let program = directory.join("chrome");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > {args}\n\
             {FAR_SIDE_OF}={chrome} {FAR_SIDE_IN}={directory} {FAR_SIDE_HOLDS}='{holds}' \
             exec {me} {test} --exact --test-threads=1\n",
            args = quoted(&directory.join("args")),
            chrome = quoted(&chrome),
            directory = quoted(&directory),
            me = quoted(&me),
        ),
    )
    .map_err(|why| why.to_string())?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .map_err(|why| why.to_string())?;
    Ok(FarSide { directory })
}

/// A far side's directory, removed with it.
struct FarSide {
    directory: PathBuf,
}

impl FarSide {
    /// The program to start as Chrome.
    fn program(&self) -> PathBuf {
        self.directory.join("chrome")
    }

    /// Whether it held a commit back behind the request named.
    fn held(&self) -> bool {
        self.directory.join("held").exists()
    }
}

impl Drop for FarSide {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// When this program was run again as a far side, be it — relay the
/// machine's Chrome, holding back the commit named — and end; otherwise do
/// nothing.
fn be_the_far_side_if_asked() {
    let (Some(chrome), Some(directory), Some(holds)) = (
        std::env::var_os(FAR_SIDE_OF),
        std::env::var_os(FAR_SIDE_IN),
        std::env::var(FAR_SIDE_HOLDS).ok(),
    ) else {
        return;
    };
    let ended = relay(Path::new(&chrome), Path::new(&directory), &holds);
    std::process::exit(i32::from(ended.is_err()));
}

/// What a far side holds back.
enum Held {
    /// Nothing yet.
    Nothing,
    /// A commit, and every later message through its session but a paused
    /// request.
    Commit(Option<String>, Vec<Vec<u8>>),
    /// One paused request.
    Request(Vec<u8>),
    /// What was held has gone on, behind what it was held behind.
    Done,
}

impl Held {
    /// What is held, in the order it came.
    fn kept(self) -> Vec<Vec<u8>> {
        match self {
            Held::Commit(_, kept) => kept,
            Held::Request(paused) => vec![paused],
            Held::Nothing | Held::Done => Vec::new(),
        }
    }
}

/// What a far side holds back, and whether what it is held behind has gone.
struct Hold {
    held: Held,
    behind_gone: bool,
    /// Whether a commit is held, rather than a paused request.
    commit_first: bool,
}

impl Hold {
    /// `message`, through `session`, read: what to hand on now, and whether
    /// the order was made. `is_held` says it is what is held, `is_behind`
    /// what that is held behind, and `paused` that it is a paused request.
    fn pass(
        &mut self,
        message: &[u8],
        session: Option<String>,
        is_held: bool,
        is_behind: bool,
        paused: bool,
    ) -> (Vec<Vec<u8>>, bool) {
        match &mut self.held {
            Held::Nothing if is_held && self.commit_first => {
                self.held = Held::Commit(session, vec![message.to_vec()]);
                (Vec::new(), false)
            }
            // What it is held behind has gone already: the order is
            // Chrome's own.
            Held::Nothing if is_held && self.behind_gone => {
                self.held = Held::Done;
                (vec![message.to_vec()], true)
            }
            Held::Nothing if is_held => {
                self.held = Held::Request(message.to_vec());
                (Vec::new(), false)
            }
            Held::Commit(from, kept) if *from == session && !paused => {
                kept.push(message.to_vec());
                (Vec::new(), false)
            }
            _ if is_behind => {
                self.behind_gone = true;
                let kept = match std::mem::replace(&mut self.held, Held::Done) {
                    Held::Nothing => {
                        self.held = Held::Nothing;
                        Vec::new()
                    }
                    other => other.kept(),
                };
                let ordered = !kept.is_empty();
                let mut sent = vec![message.to_vec()];
                sent.extend(kept);
                (sent, ordered)
            }
            _ => (vec![message.to_vec()], false),
        }
    }
}

/// Start the machine's Chrome with the arguments this far side was given,
/// and hand each message on between it and the browser on this side's
/// descriptors 3 and 4, holding back the commit or the paused request
/// `holds` names until the paused request or the commit it names has gone
/// ahead.
fn relay(chrome: &Path, directory: &Path, holds: &str) -> Result<(), String> {
    let (commit_first, document, behind) = match holds.split(' ').collect::<Vec<_>>()[..] {
        ["commit", document, "behind", behind] => (true, document, behind),
        ["request", document, "behind", behind] => (false, document, behind),
        _ => return Err(format!("no hold in {holds}")),
    };
    let (held_as, behind_as) = if commit_first {
        ("Page.frameNavigated", "Fetch.requestPaused")
    } else {
        ("Fetch.requestPaused", "Page.frameNavigated")
    };
    let args = std::fs::read_to_string(directory.join("args")).map_err(|why| why.to_string())?;
    let mut started = Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"exec "$0" "$@" 3<&0 4>&1 0</dev/null 1>/dev/null"#)
        .arg(chrome)
        .args(args.lines())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|why| why.to_string())?;
    let mut to_chrome = started.stdin.take().ok_or("no stdin")?;
    let from_chrome = started.stdout.take().ok_or("no stdout")?;
    // What the browser sends goes to Chrome as it is; when the browser
    // closes its end, Chrome's is closed, and Chrome ends.
    let from_browser = File::open("/dev/fd/3").map_err(|why| why.to_string())?;
    std::thread::spawn(move || {
        let mut from_browser = BufReader::new(from_browser);
        let mut message = Vec::new();
        while from_browser
            .read_until(0, &mut message)
            .is_ok_and(|read| read > 0)
        {
            if to_chrome.write_all(&message).is_err() {
                return;
            }
            message.clear();
        }
    });
    let mut to_browser = OpenOptions::new()
        .write(true)
        .open("/dev/fd/4")
        .map_err(|why| why.to_string())?;
    let mut from_chrome = BufReader::new(from_chrome);
    let mut hold = Hold {
        held: Held::Nothing,
        behind_gone: false,
        commit_first,
    };
    let mut message = Vec::new();
    while from_chrome
        .read_until(0, &mut message)
        .is_ok_and(|read| read > 0)
    {
        // Read as this browser reads it: a message's method and session,
        // and the address a commit or a paused request names.
        let (method, session, params) =
            match chrome_protocol::read(message.strip_suffix(&[0]).unwrap_or(&message)) {
                Ok(Message::Event {
                    method,
                    session,
                    params,
                }) => (method, session, params),
                Ok(Message::Reply { session, .. }) => (String::new(), session, Vec::new()),
                _ => (String::new(), None, Vec::new()),
            };
        let address = |method_named: &str| match method_named {
            "Page.frameNavigated" => chrome_protocol::committed(&params)
                .map(|committed| committed.url)
                .unwrap_or_default(),
            "Fetch.requestPaused" => chrome_protocol::paused(&params)
                .map(|paused| paused.url)
                .unwrap_or_default(),
            _ => String::new(),
        };
        let is_held = method == held_as && address(&method).ends_with(document);
        let is_behind = method == behind_as && address(&method).ends_with(behind);
        let paused = method == "Fetch.requestPaused";
        let (sent, ordered) = hold.pass(&message, session, is_held, is_behind, paused);
        for later in sent {
            to_browser
                .write_all(&later)
                .map_err(|why| why.to_string())?;
        }
        if ordered {
            File::create(directory.join("held")).map_err(|why| why.to_string())?;
        }
        message.clear();
    }
    for later in hold.held.kept() {
        to_browser
            .write_all(&later)
            .map_err(|why| why.to_string())?;
    }
    let _ = started.wait();
    Ok(())
}

#[test]
fn a_new_pages_sheet_chrome_pauses_before_the_page_is_its_own() {
    be_the_far_side_if_asked();
    let far = far_side(
        "a_new_pages_sheet_chrome_pauses_before_the_page_is_its_own",
        "commit /next.html behind /next.css",
    )
    .unwrap();
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start_at(&far.program()).unwrap();
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/page.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(&mut tab, &log, "/first.css").unwrap();
    let outcome = tab
        .act(&Target::Named("Next".to_owned()), &Verb::Activate)
        .unwrap();
    assert!(matches!(outcome, Outcome::Followed { .. }), "{outcome:?}");
    let action = tab.acted().unwrap();
    read_until(&mut tab, &log, "/next.css").unwrap();
    drop(tab);
    assert!(
        far.held(),
        "the new page's commit was not held back behind its sheet"
    );

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    let next = line(&lines, &format!("http://127.0.0.1:{port}/next.html")).unwrap();
    let Cause::Agent {
        action: named,
        place: Place::Document(left),
    } = next.cause()
    else {
        panic!("the navigation is not the agent's: {next}");
    };
    assert_eq!(*named, action);
    // Paused before the page it is for was read as arrived, and still that
    // page's — which the agent's action brought — and not the action's in
    // the page it left.
    let sheet = line(&lines, &format!("http://127.0.0.1:{port}/next.css")).unwrap();
    assert_eq!(sheet.purpose(), &Purpose::Style);
    let Cause::Document { document } = sheet.cause() else {
        panic!("the new page's sheet is not its page's: {sheet}");
    };
    assert_ne!(document, left, "{sheet}");
    assert_eq!(record.documents().cause_of(*document), Some(next.cause()));
    let chain = sheet.chain(record.documents());
    assert!(chain.followed_from(action) && chain.is_whole(), "{chain:?}");
}

#[test]
fn a_frames_new_documents_fetch_chrome_pauses_before_the_document_is_its_own() {
    be_the_far_side_if_asked();
    let far = far_side(
        "a_frames_new_documents_fetch_chrome_pauses_before_the_document_is_its_own",
        "commit /other2.html behind /from-other2",
    )
    .unwrap();
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start_at(&far.program()).unwrap();
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/framed.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(&mut tab, &log, "/from-other2").unwrap();
    drop(tab);
    assert!(
        far.held(),
        "the frame's second document's commit was not held back behind its fetch"
    );

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    // The frame's second page is its first page's doing.
    let going = line(&lines, &format!("http://localhost:{port}/other2.html")).unwrap();
    let Cause::Document { document: first } = going.cause() else {
        panic!("the frame's navigation is not its document's: {going}");
    };
    let fetch = line(&lines, &format!("http://localhost:{port}/from-other2")).unwrap();
    let Cause::Document { document: second } = fetch.cause() else {
        panic!("the frame's fetch is not a document's: {fetch}");
    };
    assert_ne!(second, first, "pinned on the document before it: {fetch}");
    assert_eq!(record.documents().cause_of(*second), Some(going.cause()));
    assert!(fetch.chain(record.documents()).is_whole());
}

#[test]
fn an_old_pages_beacon_chrome_pauses_after_the_new_page_is_the_old_pages() {
    be_the_far_side_if_asked();
    let far = far_side(
        "an_old_pages_beacon_chrome_pauses_after_the_new_page_is_the_old_pages",
        "request /beacon behind /left-for.html",
    )
    .unwrap();
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start_at(&far.program()).unwrap();
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/leaving.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(&mut tab, &log, "/left-for.css").unwrap();
    read_until(&mut tab, &log, "/beacon").unwrap();
    read_until(&mut tab, &log, "/kept").unwrap();
    drop(tab);
    assert!(
        far.held(),
        "the old page's beacon was not held back behind the new page's commit"
    );

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    // The page sent itself on: the navigation is the page left's doing.
    let going = line(&lines, &format!("http://127.0.0.1:{port}/left-for.html")).unwrap();
    let Cause::Document { document: left } = going.cause() else {
        panic!("the page's navigation is not its own: {going}");
    };
    let sheet = line(&lines, &format!("http://127.0.0.1:{port}/left-for.css")).unwrap();
    let Cause::Document { document: arrived } = sheet.cause() else {
        panic!("the new page's sheet is not its page's: {sheet}");
    };
    assert_ne!(arrived, left, "{sheet}");
    assert_eq!(record.documents().cause_of(*arrived), Some(going.cause()));
    // Paused after the new page was read as arrived, and still the page
    // left's — as is its `keepalive` fetch, whichever side of the commit
    // Chrome paused it.
    for asked in ["/beacon", "/kept"] {
        let it = line(&lines, &format!("http://127.0.0.1:{port}{asked}")).unwrap();
        assert_eq!(
            it.cause(),
            &Cause::Document { document: *left },
            "pinned on the page arrived ({arrived}): {it}"
        );
        assert!(it.chain(record.documents()).is_whole(), "{it}");
    }
}

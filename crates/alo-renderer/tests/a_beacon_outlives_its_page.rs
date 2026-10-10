/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 369 (ADR 0040 §§ 2–4): a beacon outlives its page, and is held
//! to every rule a fetch is.
//!
//! The page is `alo-sites-cta`'s, the markup frozen in the corpus, served
//! here from a local server so that what it sends can be heard: its own
//! analytics script, with **no beacon lent** — `navigator.sendBeacon` is the
//! engine's. It is loaded into real [`Tabs`] over the confined `alo-render`
//! binary, and every request is made through a real [`Pool`] by
//! [`Answering`], as the window's conductor makes them. Three things are read
//! back: what the server heard, what the session's record says, and what
//! each process counts in flight.
//!
//! The frozen page links `/assets/site.css`, which this server does not
//! have, so it is drawn unstyled. That changes nothing it sends: it is still
//! shorter than its 800 × 600 window, so it is read to the bottom.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alo_layout::Size;
use alo_net::activity::{Entry, Happened};
use alo_net::cause::{Cause, DocumentId, TabId};
use alo_net::{Pool, Purpose, Trust};
use alo_renderer::fetch_answering::{Answered, Answering};
use alo_renderer::fetch_decide::Rule;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page, Visibility};

/// The page alo Sites published, as the corpus froze it.
const FROZEN: &str = include_str!("../../alo-corpus/cases/alo-sites-cta/page.html");

/// What the page reports as it goes: read to the bottom of a window 800
/// wide at its root path, then no whole second spent.
const REPORTS: [&str; 2] = ["d=1000&p=%2F&w=800", "t=0"];

/// One request a server was sent, whole: its head and its body.
type Heard = Arc<Mutex<Vec<String>>>;

/// A server on a port of this machine, answering a beacon `204` with no
/// body as alo Sites' own does, and anything else `404`; its origin and what
/// it heard.
fn serve() -> Option<(String, Heard)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let heard: Heard = Arc::new(Mutex::new(Vec::new()));
    let keeping = Arc::clone(&heard);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else {
                return;
            };
            answer_one(socket, &keeping);
        }
    });
    Some((format!("http://127.0.0.1:{port}"), heard))
}

/// Read one request from `socket`, keep it, and answer it.
fn answer_one(mut socket: TcpStream, heard: &Heard) {
    if socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .is_err()
    {
        return;
    }
    let mut asked = Vec::new();
    let mut block = [0u8; 4096];
    while let Ok(got) = socket.read(&mut block) {
        if got == 0 {
            break;
        }
        asked.extend_from_slice(block.get(..got).unwrap_or_default());
        if whole_request(&asked) {
            break;
        }
    }
    let text = String::from_utf8_lossy(&asked).into_owned();
    let beacon = text.starts_with("POST /_alo/collect ");
    if let Ok(mut heard) = heard.lock() {
        heard.push(text);
    }
    let reply = if beacon {
        "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n"
    } else {
        "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    };
    let _ = socket.write_all(reply.as_bytes());
    let _ = socket.flush();
}

/// Whether these bytes are a head and as much body as it promised.
fn whole_request(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let declared = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    body.len() >= declared
}

/// The beacons a server heard, each as its body, after checking each was
/// sent as a beacon is: a `POST` of `text/plain;charset=UTF-8` to the
/// collector. [`None`] for one that was not.
fn beacons(server: &Heard) -> Option<Vec<String>> {
    let requests = server.lock().ok()?.clone();
    requests
        .iter()
        .filter(|request| request.starts_with("POST "))
        .map(|request| {
            let (head, sent) = request.split_once("\r\n\r\n")?;
            let carries = |name: &str, value: &str| {
                head.lines().skip(1).any(|line| {
                    line.split_once(':').is_some_and(|(held, said)| {
                        held.trim().eq_ignore_ascii_case(name) && said.trim() == value
                    })
                })
            };
            let right = head.starts_with("POST /_alo/collect HTTP/1.1")
                && carries("Content-Type", "text/plain;charset=UTF-8");
            right.then(|| sent.to_owned())
        })
        .collect()
}

/// The record's lines for beacons, as `method url status`, each checked to
/// be a beacon caused by `document`. [`None`] for one that was not.
fn recorded(network: &Network, document: DocumentId) -> Option<Vec<String>> {
    network
        .pool
        .activity()
        .entries()
        .filter(|entry| entry.purpose() == &Purpose::Beacon)
        .map(|entry: &Entry| {
            (entry.cause() == &Cause::Document { document }).then(|| {
                format!(
                    "{} {} {}",
                    entry.method(),
                    entry.url().path,
                    match entry.happened() {
                        Happened::Answered { status, .. } => status.to_string(),
                        Happened::Served { status } => format!("served {status}"),
                        Happened::Refused { rule } => format!("refused: {rule}"),
                        Happened::Failed { why } => format!("failed: {why}"),
                    }
                )
            })
        })
        .collect()
}

/// A server, a tab over the real renderer binary showing `markup` from it at
/// 800 × 600, a session's network and the queue the conductor keeps: what
/// every test here starts from. [`None`] when a port, a URL or the load
/// could not be had.
struct Set {
    heard: Heard,
    tabs: Tabs,
    tab: TabId,
    document: DocumentId,
    network: Network,
    answering: Answering,
}

impl Set {
    fn up(markup: &str) -> Option<Self> {
        let (origin, heard) = serve()?;
        let url = alo_url::parse(&format!("{origin}/")).ok()?;
        let mut tabs = Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]));
        let tab = tabs.open(url.clone());
        let page = Page::new(markup, Size::new(800.0, 600.0)).at(url);
        let Ok(FromRenderer::Loaded { .. }) = tabs.load(tab, page, Cause::Person { tab }) else {
            return None;
        };
        let document = tabs.tab(tab).and_then(Tab::document)?;
        let pool = Pool::with_trust(Trust::of(&[]).ok()?).patient_for(Duration::from_secs(5));
        let mut set = Self {
            heard,
            tabs,
            tab,
            document,
            network: Network::over(pool),
            answering: Answering::new(),
        };
        // Its linked sheet, which this server answers `404`.
        set.answering.take_from(&mut set.tabs, tab);
        set.answer_all();
        Some(set)
    }

    /// Make everything waiting, and what its answers ask for in turn: each
    /// answer, in order.
    fn answer_all(&mut self) -> Vec<Answered> {
        let mut answered = Vec::new();
        while let Some(one) = self
            .answering
            .answer_next(&mut self.tabs, &mut self.network)
        {
            answered.push(one);
        }
        answered
    }

    /// Tell the page whether it can be seen, and queue what it asked for.
    fn shown(&mut self, to: Visibility) -> bool {
        let told = self.tabs.visibility(self.tab, to);
        self.answering.take_from(&mut self.tabs, self.tab);
        matches!(told, Ok(FromRenderer::Delivered { .. }))
    }

    /// Queue what every page left since the last look asked to outlive it,
    /// as the conductor does, and answer what it refused.
    fn left(&mut self) -> Vec<alo_renderer::tab::Leaving> {
        let left = self.tabs.left();
        for leaving in &left {
            if let Some(document) = leaving.document {
                self.answering
                    .outlive(leaving.tab, document, leaving.outliving.clone());
            }
        }
        left
    }
}

/// ADR 0040 § 6's first closing condition: with no beacon lent, leaving
/// the page makes the browser process send its two reports, each a beacon
/// caused by the document and made after the page has gone.
#[test]
fn a_closed_tabs_page_sends_its_reports_after_it_has_gone() {
    let mut set = Set::up(FROZEN).expect("the page loads from its server");
    assert_eq!(beacons(&set.heard), Some(Vec::new()), "nothing sent yet");

    assert!(set.tabs.close(set.tab));
    let left = set.left();
    let [leaving] = left.as_slice() else {
        panic!("one page left: {left:?}");
    };
    assert!(leaving.refused.is_empty(), "{:?}", leaving.refused);
    let decided: Vec<(String, Purpose, Vec<u8>)> = leaving
        .outliving
        .iter()
        .map(|fetch| {
            assert_eq!(
                fetch.request.cause,
                Cause::Document {
                    document: set.document
                }
            );
            (
                fetch.request.method.clone(),
                fetch.request.purpose.clone(),
                fetch.request.body.clone(),
            )
        })
        .collect();
    assert_eq!(
        decided,
        REPORTS.map(|body| ("POST".to_owned(), Purpose::Beacon, body.as_bytes().to_vec()))
    );
    // The page has gone, with its process: nothing is holding it.
    assert!(set.tabs.renderers().is_empty());
    assert!(set.tabs.tab(set.tab).is_none());
    assert_eq!(
        set.tabs.kept_alive(set.document),
        REPORTS.iter().map(|body| body.len()).sum::<usize>(),
        "counted until each is made"
    );

    // Made in their turn, answered by nobody.
    let answered = set.answer_all();
    assert_eq!(answered.len(), 2);
    assert!(
        answered.iter().all(|one| one.delivered == Ok(None)),
        "{answered:?}"
    );
    assert_eq!(
        beacons(&set.heard),
        Some(REPORTS.map(str::to_owned).to_vec())
    );
    assert_eq!(
        recorded(&set.network, set.document),
        Some(vec![
            "POST /_alo/collect 204".to_owned(),
            "POST /_alo/collect 204".to_owned(),
        ])
    );
    assert_eq!(set.tabs.kept_alive(set.document), 0);
}

/// The frozen page with one more listener: shown again, it sends 64 KiB, the
/// whole of what may be in flight — which its renderer refuses while its two
/// reports are unanswered, and sends once their answers have freed it.
fn with_a_full_beacon() -> String {
    FROZEN.replacen(
        "</body>",
        "<script>document.addEventListener('visibilitychange', function () { \
           if (document.visibilityState !== 'visible') { return; } \
           var big = 'x'; for (var i = 0; i < 16; i++) { big = big + big; } \
           navigator.sendBeacon('/_alo/collect', big); });</script></body>",
        1,
    )
}

/// ADR 0040 § 6's second: hiding the page sends the same two while it is
/// held, their answers reach the renderer, and both processes' counts are
/// freed by them.
#[test]
fn hidden_it_sends_them_while_held_and_their_answers_free_both_counts() {
    let mut set = Set::up(&with_a_full_beacon()).expect("the page loads from its server");
    assert!(set.shown(Visibility::Hidden));
    assert_eq!(set.answering.len(), 2, "its two reports, queued");
    assert_eq!(
        set.tabs.kept_alive(set.document),
        REPORTS.iter().map(|body| body.len()).sum::<usize>()
    );

    // Shown before they are answered: 64 KiB more would pass the bound, so
    // its renderer answers `false` and asks nothing.
    assert!(set.shown(Visibility::Visible));
    assert_eq!(set.answering.len(), 2, "nothing more asked");

    let answered = set.answer_all();
    assert_eq!(answered.len(), 2);
    assert!(
        answered.iter().all(|one| matches!(
            &one.delivered,
            Ok(Some(FromRenderer::Delivered { issues, .. })) if issues.is_empty()
        )),
        "each answer reaches the page, which expected it: {answered:?}"
    );
    assert_eq!(
        beacons(&set.heard),
        Some(REPORTS.map(str::to_owned).to_vec())
    );
    assert_eq!(set.tabs.kept_alive(set.document), 0);

    // Hidden again it has nothing more to report; shown again, its whole
    // 64 KiB now fits in its renderer's count and in this process's.
    assert!(set.shown(Visibility::Hidden));
    assert!(set.shown(Visibility::Visible));
    assert_eq!(set.answer_all().len(), 1);
    let bodies = beacons(&set.heard).unwrap_or_default();
    assert_eq!(
        bodies.len(),
        3,
        "{:?}",
        bodies.iter().map(String::len).collect::<Vec<_>>()
    );
    assert_eq!(bodies.last().map(String::len), Some(65_536));
    assert_eq!(set.tabs.kept_alive(set.document), 0);
}

/// ADR 0040 § 6's third: a page's `fetch` asked as it is left is still
/// refused by `Rule::Leaving`, beside the beacon that is made.
#[test]
fn a_fetch_asked_as_the_page_is_left_is_refused_beside_the_beacon_made() {
    let mut set = Set::up(
        "<!DOCTYPE html><p>x</p><script>window.addEventListener('pagehide', function () { \
           fetch('/_alo/fetched'); navigator.sendBeacon('/_alo/collect', 'last'); });</script>",
    )
    .expect("the page loads from its server");
    assert!(set.tabs.close(set.tab));
    let left = set.left();
    let [leaving] = left.as_slice() else {
        panic!("one page left: {left:?}");
    };
    let refused: Vec<(&str, &Rule, &Purpose)> = leaving
        .refused
        .iter()
        .map(|refusal| (refusal.asked.as_str(), &refusal.rule, &refusal.purpose))
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert_eq!(refused[0].1, &Rule::Leaving);
    assert_eq!(refused[0].2, &Purpose::Fetch);
    assert!(refused[0].0.ends_with("/_alo/fetched"), "{refused:?}");
    assert!(
        leaving
            .said
            .iter()
            .any(|line| line.contains("did not ask to outlive it")),
        "{:?}",
        leaving.said
    );
    assert_eq!(leaving.outliving.len(), 1);
    set.answer_all();
    assert_eq!(beacons(&set.heard), Some(vec!["last".to_owned()]));
}

/// A page replaced by a load sends its beacon too, made after the next page
/// is showing and answered by nobody: the tab's new document is not the one
/// that asked.
#[test]
fn a_page_replaced_by_a_load_sends_its_beacon_to_nobody() {
    let mut set = Set::up(
        "<!DOCTYPE html><p>x</p><script>window.addEventListener('pagehide', function () { \
           navigator.sendBeacon('/_alo/collect', 'replaced'); });</script>",
    )
    .expect("the page loads from its server");
    let url = set.tabs.tab(set.tab).map(|tab| tab.url().clone());
    let next = Page::new("<!DOCTYPE html><p>next</p>", Size::new(800.0, 600.0))
        .at(url.expect("the tab is open"));
    let loaded = set.tabs.load(set.tab, next, Cause::Person { tab: set.tab });
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert_ne!(
        set.tabs.tab(set.tab).and_then(Tab::document),
        Some(set.document)
    );
    let left = set.left();
    assert_eq!(left.len(), 1, "{left:?}");
    let answered = set.answer_all();
    assert_eq!(
        answered
            .iter()
            .filter(|one| one.delivered == Ok(None))
            .count(),
        answered.len(),
        "{answered:?}"
    );
    assert_eq!(beacons(&set.heard), Some(vec!["replaced".to_owned()]));
    assert_eq!(
        recorded(&set.network, set.document),
        Some(vec!["POST /_alo/collect 204".to_owned()])
    );
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 338: a decided fetch is made (ADR 0032 § 3).
//!
//! A page served from one local server fetches from it and from a second,
//! which is another origin because it is another port. The page is loaded
//! into real [`Tabs`] over the confined `alo-render` binary, and every fetch
//! it asks for is made through a real [`Pool`] by
//! [`alo_renderer::fetch_answering`], one at a time, each answer delivered
//! as a task of its own. Three things are then read back: what the page
//! heard, what each server was sent, and the session's record.
//!
//! Queue item 340 is here too: a redirect the page's fetch follows is judged
//! by the page's `connect-src`, with the paths it names ignored.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alo_layout::Size;
use alo_net::activity::{Entry, Happened};
use alo_net::cause::Cause;
use alo_net::{Pool, Purpose, Trust};
use alo_renderer::fetch_answering::Answering;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page, ToRenderer};

/// One request a server was sent: its request line and its headers.
type Heard = Arc<Mutex<Vec<String>>>;

/// Which of the two servers is answering.
#[derive(Clone, Copy)]
enum Server {
    /// The page's own origin.
    Home,
    /// Another origin, which agrees to be read by the page for some paths.
    Other,
}

/// Where the two servers are: each origin, serialised.
#[derive(Clone)]
struct Origins {
    home: String,
    other: String,
}

/// A port on this machine to listen on, and its origin.
fn listen() -> Option<(TcpListener, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    Some((listener, format!("http://127.0.0.1:{port}")))
}

/// Answer on `listener` as `server`, knowing both origins: what it heard.
fn serve(listener: TcpListener, server: Server, origins: Origins) -> Heard {
    let heard: Heard = Arc::new(Mutex::new(Vec::new()));
    let keeping = Arc::clone(&heard);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else {
                return;
            };
            answer_one(socket, server, &origins, &keeping);
        }
    });
    heard
}

/// Read one request from `socket`, note it, and answer it.
fn answer_one(mut socket: TcpStream, server: Server, origins: &Origins, heard: &Heard) {
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
    let request_head = text.split("\r\n\r\n").next().unwrap_or_default().to_owned();
    let line = request_head.lines().next().unwrap_or_default().to_owned();
    if let Ok(mut heard) = heard.lock() {
        heard.push(request_head);
    }
    let reply = route(server, origins, &line);
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

/// A whole response: `status` and its reason, `headers`, and `body`.
fn reply(status: &str, headers: &[String], body: &str) -> String {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    for header in headers {
        out.push_str(header);
        out.push_str("\r\n");
    }
    let _ = write!(
        out,
        "Cache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    out
}

/// What each server says to each request line.
fn route(server: Server, origins: &Origins, line: &str) -> String {
    let agreed = format!("Access-Control-Allow-Origin: {}", origins.home);
    match (server, line) {
        (Server::Home, "GET /text HTTP/1.1") => reply(
            "200 Fine",
            &[
                "Content-Type: text/plain".to_owned(),
                "Set-Cookie: seen=1".to_owned(),
            ],
            "hello",
        ),
        (Server::Home, "GET /hop HTTP/1.1") => {
            reply("302 Found", &["Location: /text".to_owned()], "")
        }
        (Server::Home, "GET /leave HTTP/1.1") => reply(
            "302 Found",
            &[format!("Location: {}/open", origins.other)],
            "",
        ),
        (Server::Home, "GET /loop HTTP/1.1") => {
            reply("302 Found", &["Location: /loop".to_owned()], "")
        }
        (Server::Home, "GET /back HTTP/1.1") => reply(
            "200 OK",
            &["Access-Control-Allow-Origin: null".to_owned()],
            "back",
        ),
        // Back to the page's origin: as one hop of a chain that bounces, and
        // as a hop out of `/api/` that a policy naming only `/api/` refuses.
        (Server::Other, "GET /bounce HTTP/1.1" | "GET /api/away HTTP/1.1") => reply(
            "302 Found",
            &[agreed, format!("Location: {}/back", origins.home)],
            "",
        ),
        // A redirect that does not agree to be read, to an answer that would.
        (Server::Other, "GET /astray HTTP/1.1") => reply(
            "302 Found",
            &[format!("Location: {}/back", origins.home)],
            "",
        ),
        (Server::Other, "GET /open HTTP/1.1") => reply("200 OK", &[agreed], "open"),
        (Server::Other, "GET /closed HTTP/1.1") => reply("200 OK", &[], "closed"),
        (Server::Other, "OPTIONS /put HTTP/1.1") => reply(
            "204 No Content",
            &[
                agreed,
                "Access-Control-Allow-Methods: PUT".to_owned(),
                "Access-Control-Allow-Headers: x-thing".to_owned(),
            ],
            "",
        ),
        (Server::Other, "PUT /put HTTP/1.1") => reply("200 OK", &[agreed], "put"),
        // Out of `/api/` on the same server, which a policy naming only `/api/`
        // follows.
        (Server::Other, "GET /api/stay HTTP/1.1") => {
            reply("302 Found", &[agreed, "Location: /other".to_owned()], "")
        }
        (Server::Other, "GET /other HTTP/1.1") => reply("200 OK", &[agreed], "other"),
        _ => reply("404 Not Found", &[], "no"),
    }
}

/// The closing condition's page: a same-origin text, another origin's
/// answer with and without `Access-Control-Allow-Origin`, a request that is
/// asked about first, a redirect under each mode, a fetch refused before it
/// is sent, and a redirect refused before it is followed.
const EVERY_KIND: &str = "fetch('/text').then(heard('same'), failed('same'));
     fetch(OTHER + '/open').then(heard('open'), failed('open'));
     fetch(OTHER + '/closed').then(heard('closed'), failed('closed'));
     fetch(OTHER + '/put', { method: 'PUT', headers: { 'X-Thing': '1' }, body: 'x' })
       .then(heard('put'), failed('put'));
     fetch('/hop').then(heard('follow'), failed('follow'));
     fetch('/hop', { redirect: 'manual' }).then(heard('manual'), failed('manual'));
     fetch('/hop', { redirect: 'error' }).then(heard('error'), failed('error'));
     fetch(OTHER + '/x', { mode: 'same-origin' }).then(heard('refused'), failed('refused'));
     fetch('/leave', { mode: 'same-origin' }).then(heard('left'), failed('left'));";

/// A page at `home` running `fetches`, with `OTHER`, `say`, `heard(label)`
/// and `failed(label)` to write what each answer was into `<p id=out>`.
fn page_fetching(home: &str, other: &str, fetches: &str) -> Option<Page> {
    let url = alo_url::parse(&format!("{home}/page")).ok()?;
    let script = format!(
        "var OTHER = '{other}';
         var out = document.getElementById('out');
         function say(what) {{ out.textContent = out.textContent + ' [' + what + ']'; }}
         function heard(label) {{
           return function (r) {{
             return r.text().then(function (t) {{
               say(label + ' ' + r.type + ' ' + r.status + ' ' + r.statusText + ' ' +
                   r.redirected + ' ' + t);
             }});
           }};
         }}
         function failed(label) {{ return function (e) {{ say(label + ' ' + e.name); }}; }}
         {fetches}"
    );
    Some(
        Page::new(
            format!("<!doctype html><body><p id=out>-</p><script>{script}</script></body>"),
            Size::new(800.0, 400.0),
        )
        .at(url),
    )
}

/// What one server heard, each request's head.
fn heads(heard: &Heard) -> Vec<String> {
    heard.lock().map(|heard| heard.clone()).unwrap_or_default()
}

/// The request lines among `heads`, in order.
fn lines(heads: &[String]) -> Vec<String> {
    heads
        .iter()
        .map(|head| head.lines().next().unwrap_or_default().to_owned())
        .collect()
}

/// Whether the head of a request has a header, named in any case, with
/// this value.
fn carries(head: &str, name: &str, value: &str) -> bool {
    head.lines().skip(1).any(|line| {
        line.split_once(':').is_some_and(|(held, said)| {
            held.trim().eq_ignore_ascii_case(name) && said.trim() == value
        })
    })
}

/// Whether the head of a request has a header of this name at all.
fn names(head: &str, name: &str) -> bool {
    head.lines().skip(1).any(|line| {
        line.split_once(':')
            .is_some_and(|(held, _)| held.trim().eq_ignore_ascii_case(name))
    })
}

/// What a line in the record says, in a form an assertion can read.
fn said(entry: &Entry) -> String {
    format!(
        "{} {} {}",
        entry.method(),
        entry.url().serialised,
        match entry.happened() {
            Happened::Answered { status, .. } => status.to_string(),
            Happened::Served { status } => format!("served {status}"),
            Happened::Refused { rule } => format!("refused: {rule}"),
            Happened::Failed { why } => format!("failed: {why}"),
        }
    )
}

/// Two servers answering, a tab over the real renderer binary showing a page
/// at the first that runs `fetches`, and a session's network: everything a
/// test here needs before it answers anything. [`None`] when a port, a URL or
/// the load could not be had.
struct Set {
    origins: Origins,
    home_heard: Heard,
    other_heard: Heard,
    tabs: Tabs,
    tab: alo_net::cause::TabId,
    document: alo_net::cause::DocumentId,
    network: Network,
}

fn set_up(fetches: &str) -> Option<Set> {
    set_up_under(fetches, |_| None)
}

/// [`set_up`], with the page under the `Content-Security-Policy` header
/// `policy` writes from the two origins, when it writes one.
fn set_up_under(fetches: &str, policy: impl Fn(&Origins) -> Option<String>) -> Option<Set> {
    let (home_listener, home) = listen()?;
    let (other_listener, other) = listen()?;
    let origins = Origins { home, other };
    let home_heard = serve(home_listener, Server::Home, origins.clone());
    let other_heard = serve(other_listener, Server::Other, origins.clone());
    let mut tabs = Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]));
    let tab = tabs.open(alo_url::parse(&format!("{}/", origins.home)).ok()?);
    let mut page = page_fetching(&origins.home, &origins.other, fetches)?;
    if let Some(policy) = policy(&origins) {
        page = page.with_policy(policy);
    }
    let Ok(FromRenderer::Loaded { .. }) = tabs.load(tab, page, Cause::Person { tab }) else {
        return None;
    };
    let document = tabs.tab(tab).and_then(Tab::document)?;
    let pool = Pool::with_trust(Trust::of(&[]).ok()?).patient_for(Duration::from_secs(5));
    Some(Set {
        origins,
        home_heard,
        other_heard,
        tabs,
        tab,
        document,
        network: Network::over(pool),
    })
}

/// What answering a page's every fetch came to.
struct Answers {
    /// The page's `<p id=out>` afterwards, as the agent tree reads it.
    page: String,
    /// What the person was told, each line.
    told: Vec<String>,
    /// How many answers were delivered as tasks.
    delivered: usize,
}

/// Answer everything `set`'s page asks, one at a time, and in turn what its
/// answers' reactions ask.
fn answer_all(set: &mut Set) -> Option<Answers> {
    let mut answering = Answering::new();
    answering.take_from(&mut set.tabs, set.tab);
    let mut told = Vec::new();
    let mut delivered = 0;
    while let Some(answered) = answering.answer_next(&mut set.tabs, &mut set.network) {
        if answered.tab != set.tab {
            return None;
        }
        if let Ok(Some(FromRenderer::Delivered { .. })) = answered.delivered {
            delivered += 1;
        }
        told.extend(answered.said);
    }
    let Ok(FromRenderer::Tree(snapshot)) = set.tabs.ask(set.tab, &ToRenderer::ReadTree) else {
        return None;
    };
    Some(Answers {
        page: format!("{snapshot:?}"),
        told,
        delivered,
    })
}

#[test]
fn the_page_hears_each_answer_and_the_person_why_one_failed() {
    let mut set = set_up(EVERY_KIND).expect("set up");
    let answers = answer_all(&mut set).expect("answered");
    assert_eq!(answers.delivered, 9, "every ask answered as a task");
    for heard in [
        "[same basic 200 Fine false hello]",
        "[open cors 200 OK false open]",
        "[closed TypeError]",
        "[put cors 200 OK false put]",
        "[follow basic 200 Fine true hello]",
        // Its status text is empty, and the text's white space collapses.
        "[manual opaqueredirect 0 false ]",
        "[error TypeError]",
        "[refused TypeError]",
        "[left TypeError]",
    ] {
        assert!(
            answers.page.contains(heard),
            "{heard} not in {}",
            answers.page
        );
    }
    // What the person is told, and the page is not.
    let told = answers.told.join("\n");
    for reason in [
        "Access-Control-Allow-Origin",
        "it asked for the same origin only, and http://127.0.0.1",
        "it asked for the same origin only, and was redirected to",
        "asked not to be redirected",
    ] {
        assert!(told.contains(reason), "{reason} not in {told}");
    }
}

#[test]
fn each_server_is_sent_what_the_rules_say_and_nothing_refused() {
    let mut set = set_up(EVERY_KIND).expect("set up");
    answer_all(&mut set).expect("answered");
    let home = &set.origins.home;
    let home_heads = heads(&set.home_heard);
    assert_eq!(
        lines(&home_heads),
        [
            "GET /text HTTP/1.1",
            "GET /hop HTTP/1.1",
            "GET /text HTTP/1.1",
            "GET /hop HTTP/1.1",
            "GET /hop HTTP/1.1",
            "GET /leave HTTP/1.1",
        ]
    );
    assert!(!names(&home_heads[0], "Cookie"), "nothing set yet");
    for later in &home_heads[1..] {
        assert!(
            carries(later, "Cookie", "seen=1"),
            "the page's own Set-Cookie went back with its own origin's requests: {later}"
        );
        assert!(!names(later, "Origin"), "a same-origin GET says no Origin");
        assert!(carries(later, "Referer", &format!("{home}/page")));
    }
    let other_heads = heads(&set.other_heard);
    assert_eq!(
        lines(&other_heads),
        [
            "GET /open HTTP/1.1",
            "GET /closed HTTP/1.1",
            "OPTIONS /put HTTP/1.1",
            "PUT /put HTTP/1.1",
        ],
        "and the refused fetch and the refused redirect to it were never sent"
    );
    for head in &other_heads {
        assert!(carries(head, "Origin", home), "{head}");
        assert!(
            !names(head, "Cookie"),
            "same-origin credentials stay home: {head}"
        );
        assert!(
            carries(head, "Referer", &format!("{home}/")),
            "strict-origin-when-cross-origin: {head}"
        );
    }
    assert!(carries(
        &other_heads[2],
        "Access-Control-Request-Method",
        "PUT"
    ));
    assert!(carries(
        &other_heads[2],
        "Access-Control-Request-Headers",
        "x-thing"
    ));
    assert_eq!(set.network.preflights.counts(), (1, 0), "asked once");
    assert_eq!(
        set.network.jar.len(),
        1,
        "seen=1, kept under the page's site"
    );
}

/// The closing condition's record: every request with the document's cause,
/// the preflight before the request it asked about, and each refusal by its
/// rule.
#[test]
fn the_record_says_each_request_with_its_cause_and_each_refusal_by_its_rule() {
    let mut set = set_up(EVERY_KIND).expect("set up");
    answer_all(&mut set).expect("answered");
    let record: Vec<&Entry> = set.network.pool.activity().entries().collect();
    for entry in &record {
        assert_eq!(
            entry.cause(),
            &Cause::Document {
                document: set.document
            },
            "{}",
            said(entry)
        );
        assert_eq!(entry.purpose(), &Purpose::Fetch, "{}", said(entry));
    }
    let record: Vec<String> = record.into_iter().map(said).collect();
    let (home, other) = (&set.origins.home, &set.origins.other);
    assert_eq!(
        record,
        [
            format!("GET {home}/text 200"),
            format!("GET {other}/open 200"),
            format!("GET {other}/closed 200"),
            format!("OPTIONS {other}/put 204"),
            format!("PUT {other}/put 200"),
            format!("GET {home}/hop 302"),
            format!("GET {home}/text 200"),
            format!("GET {home}/hop 302"),
            format!("GET {home}/hop 302"),
            format!(
                "GET {other}/x refused: it asked for the same origin only, and {other} is not \
                 {home}"
            ),
            format!("GET {home}/leave 302"),
            format!(
                "GET {other}/open refused: it asked for the same origin only, and was redirected \
                 to {other}"
            ),
        ]
    );
}

#[test]
fn a_chain_through_another_origin_says_null_and_carries_no_cookies_home() {
    let mut set = set_up(
        "fetch('/text').then(heard('same'), failed('same'));
         fetch(OTHER + '/bounce').then(heard('bounce'), failed('bounce'));
         fetch('/loop').then(heard('loop'), failed('loop'));
         fetch('/text', { credentials: 'omit' }).then(heard('omit'), failed('omit'));
         fetch(OTHER + '/open', { mode: 'no-cors' }).then(heard('opaque'), failed('opaque'));
         fetch(OTHER + '/astray').then(heard('astray'), failed('astray'));",
    )
    .expect("set up");
    let page_said = answer_all(&mut set).expect("answered").page;
    for heard in [
        "[same basic 200 Fine false hello]",
        // Back at the page's own origin, read only because it agreed to
        // `null`, and only what CORS lets through.
        "[bounce cors 200 OK true back]",
        "[loop TypeError]",
        "[omit basic 200 Fine false hello]",
        "[opaque opaque 0 false ]",
        // Every answer on a CORS chain must agree, the redirect included, so
        // the hop it pointed at is never sent.
        "[astray TypeError]",
    ] {
        assert!(page_said.contains(heard), "{heard} not in {page_said}");
    }

    let home_heads = heads(&set.home_heard);
    assert_eq!(
        lines(&home_heads),
        [
            "GET /text HTTP/1.1",
            "GET /back HTTP/1.1",
            "GET /loop HTTP/1.1",
            "GET /text HTTP/1.1",
        ]
    );
    let back = &home_heads[1];
    assert!(
        carries(back, "Origin", "null"),
        "a chain from the page to another origin and on to a third says null: {back}"
    );
    assert!(
        !names(back, "Cookie"),
        "a chain that left the page's origin carries no same-origin credentials home: {back}"
    );
    assert!(
        carries(&home_heads[2], "Cookie", "seen=1"),
        "{}",
        home_heads[2]
    );
    assert!(
        !names(&home_heads[3], "Cookie"),
        "omit sends nothing: {}",
        home_heads[3]
    );

    let other_heads = heads(&set.other_heard);
    assert_eq!(
        lines(&other_heads),
        [
            "GET /bounce HTTP/1.1",
            "GET /open HTTP/1.1",
            "GET /astray HTTP/1.1"
        ]
    );
    assert!(carries(&other_heads[0], "Origin", &set.origins.home));
    assert!(
        !names(&other_heads[1], "Origin"),
        "a no-cors GET says no Origin: {}",
        other_heads[1]
    );

    let record: Vec<String> = set.network.pool.activity().entries().map(said).collect();
    let (home, other) = (&set.origins.home, &set.origins.other);
    assert_eq!(
        record,
        [
            format!("GET {home}/text 200"),
            format!("GET {other}/bounce 302"),
            format!("GET {home}/back 200"),
            format!("GET {home}/loop 302"),
            format!("GET {home}/loop refused: redirected in a circle, back to {home}/loop"),
            format!("GET {home}/text 200"),
            format!("GET {other}/open 200"),
            format!("GET {other}/astray 302"),
        ]
    );
    assert!(
        set.network
            .pool
            .activity()
            .entries()
            .all(|entry| entry.cause()
                == &Cause::Document {
                    document: set.document
                })
    );
}

#[test]
fn an_answer_for_a_page_that_has_gone_is_made_for_nobody() {
    let mut set = set_up("fetch('/text').then(heard('same'), failed('same'));").expect("set up");
    let mut answering = Answering::new();
    answering.take_from(&mut set.tabs, set.tab);
    assert_eq!(answering.len(), 1);
    let next = page_fetching(&set.origins.home, &set.origins.other, "say('second');")
        .expect("a second page");
    let loaded = set.tabs.load(set.tab, next, Cause::Person { tab: set.tab });
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert_eq!(
        answering.answer_next(&mut set.tabs, &mut set.network),
        None,
        "the first page's fetch is answered by nobody"
    );
    assert!(heads(&set.home_heard).is_empty(), "and nothing was sent");
    assert!(set.network.pool.activity().is_empty());
}

/// Queue item 340's closing condition. Under `connect-src {other}/api/`, a
/// fetch the other origin redirects out of `/api/` on its own server is
/// followed, because CSP ignores a source's path once a request has been
/// redirected; one it redirects to the page's own origin, which the policy
/// does not name, is refused before it is sent and recorded by its rule; and
/// a first request outside `/api/` is still judged by the path.
#[test]
fn a_redirect_is_judged_by_connect_src_with_its_paths_ignored() {
    let mut set = set_up_under(
        "fetch(OTHER + '/api/stay').then(heard('stay'), failed('stay'));
         fetch(OTHER + '/api/away').then(heard('away'), failed('away'));
         fetch(OTHER + '/other').then(heard('first'), failed('first'));",
        |origins| Some(format!("connect-src {}/api/", origins.other)),
    )
    .expect("set up");
    let answers = answer_all(&mut set).expect("answered");
    for heard in [
        "[stay cors 200 OK true other]",
        "[away TypeError]",
        "[first TypeError]",
    ] {
        assert!(
            answers.page.contains(heard),
            "{heard} not in {}",
            answers.page
        );
    }
    let (home, other) = (&set.origins.home, &set.origins.other);
    let told = answers.told.join("\n");
    assert!(
        told.contains(&format!(
            "the page's fetch of {other}/api/away failed: it was redirected, and this page's \
             content security policy does not allow a fetch from {home}/back"
        )),
        "{told}"
    );

    assert!(
        heads(&set.home_heard).is_empty(),
        "the redirect the policy refused was sent"
    );
    assert_eq!(
        lines(&heads(&set.other_heard)),
        [
            "GET /api/stay HTTP/1.1",
            "GET /other HTTP/1.1",
            "GET /api/away HTTP/1.1",
        ],
        "and the first request outside /api/ was never sent"
    );

    let record: Vec<String> = set.network.pool.activity().entries().map(said).collect();
    let refused = |url: String, redirected: bool| {
        format!(
            "GET {url} refused: {}this page's content security policy does not allow a fetch \
             from {url}: connect-src allows {other}/api/",
            if redirected {
                "it was redirected, and "
            } else {
                ""
            }
        )
    };
    assert_eq!(
        record,
        [
            format!("GET {other}/api/stay 302"),
            format!("GET {other}/other 200"),
            format!("GET {other}/api/away 302"),
            refused(format!("{home}/back"), true),
            refused(format!("{other}/other"), false),
        ]
    );
}

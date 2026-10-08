/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 237, cut from 236: a page's author is told about inline script
//! their own policy refused — or, under a report-only policy, would have.
//!
//! *A page loaded under a report-only `script-src 'none'` runs its inline
//! script and the browser process posts one report naming `script-src` and
//! `inline`, and an enforced refusal is reported the same way.*
//!
//! # Why the closing tests drive a real renderer and a real socket
//!
//! The renderer is the only process that sees the script and the browser
//! process the only one that may post, so what has to be shown is the claim
//! crossing between them and becoming a post — not a report written by hand
//! in one process. So the two closing tests spawn the confined `alo-render`
//! binary through [`Tabs`], take what it answered, and post with
//! [`alo_net::Pool::report`] to a collector on `127.0.0.1`.
//!
//! The collector is a socket in this file rather than a dependency, for the
//! reason `alo-net`'s `a_violation_a_page_reports.rs` gives: nothing here
//! reaches the network.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use alo_js::interpret::Trouble;
use alo_js::{Value, script};
use alo_layout::Size;
use alo_net::cause::{Cause, Identities};
use alo_net::csp::{Inline, Placement};
use alo_net::csp_report::{self, Endpoints};
use alo_net::{Response, Trust};
use alo_renderer::host::Renderers;
use alo_renderer::tab::{TabId, Tabs};
use alo_renderer::violations::{self, MOST_OBJECTIONS, Objection};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// One inline script, which says it ran.
const A_SCRIPT: &str = "<p>hello</p><script>out = 'ran'</script>";

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

/// What a load answered, taken apart.
struct Answer {
    issues: Vec<String>,
    objections: Vec<Objection>,
}

fn answered(answer: FromRenderer) -> Answer {
    match answer {
        FromRenderer::Loaded {
            issues, objections, ..
        } => Answer { issues, objections },
        other => Answer {
            issues: vec![format!("not loaded: {other:?}")],
            objections: Vec::new(),
        },
    }
}

/// Load a page into a renderer in this process.
fn load(page: Page) -> (Renderer, Answer) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = answered(renderer.handle(ToRenderer::Load(Box::new(page))));
    (renderer, answer)
}

/// What the global `out` holds, or why not — "no script ran" when the page's
/// loop was never made.
fn out(renderer: &mut Renderer) -> String {
    let Some(looping) = renderer.event_loop() else {
        return "no script ran".to_owned();
    };
    let program = match script("this.out") {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match looping.engine().evaluate(&program) {
        Ok(Value::Text(held)) => match looping.engine().objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "a string that has gone".to_owned(),
        },
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

fn script_objection(policy: usize) -> Objection {
    Objection {
        policy,
        kind: Inline::Script,
        placement: Placement::Element,
    }
}

// --- Over the boundary, and over a socket ------------------------------------

/// Take one request, hand back its bytes, and answer `204`.
fn collector() -> (u16, mpsc::Receiver<Vec<u8>>) {
    let (say, heard) = mpsc::channel();
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return (0, heard);
    };
    let port = listener
        .local_addr()
        .map(|at| at.port())
        .unwrap_or_default();
    std::thread::spawn(move || {
        let Ok((mut socket, _)) = listener.accept() else {
            return;
        };
        if socket
            .set_read_timeout(Some(Duration::from_millis(400)))
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
        let _ = say.send(asked);
        let _ = socket.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
        let _ = socket.flush();
    });
    (port, heard)
}

/// Whether these bytes are a head and as much body as it promised.
fn whole_request(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let declared = head
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .and_then(|length| length.trim().parse::<usize>().ok())
        .unwrap_or(0);
    body.len() >= declared
}

/// A response from this machine carrying one policy header, so that a report
/// to this machine is one a local page made (a public page may not post to
/// it: item 188's rebinding test).
fn a_response(port: u16, header: &str, policy: &str) -> Response {
    let mut response = Response::ok(
        url(&format!("http://127.0.0.1:{port}/checkout")),
        A_SCRIPT.as_bytes().to_vec(),
    );
    response.headers.add(header, policy);
    response
}

/// What the browser process did with one load: the renderer's answer, the
/// reports it posted, and everything it said instead of posting.
struct Reported {
    answer: Answer,
    posted: usize,
    said: Vec<String>,
}

/// The browser process's half, end to end: load the response's page into a
/// tab through the real renderer binary, make the reports from what came
/// back, and post them.
fn loaded_and_reported(response: &Response) -> Reported {
    let mut tabs = Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]));
    let tab: TabId = tabs.open(response.url.clone());
    let page = Page::from_response(response, WINDOW);
    let answer = match tabs.load(tab, page.clone(), Cause::Person { tab }) {
        Ok(answer) => answered(answer),
        Err(lost) => Answer {
            issues: vec![format!("not loaded: {lost}")],
            objections: Vec::new(),
        },
    };
    let Some(cause) = tabs.a_page_fetching(tab) else {
        return Reported {
            answer,
            posted: 0,
            said: vec!["the tab shows no document".to_owned()],
        };
    };
    // Everything a report says about the document is what the browser process
    // knows — the URL it fetched, the status, the endpoints its headers
    // defined and the cause it recorded — and never anything the renderer
    // said.
    let about = csp_report::Page::at(response.url.clone(), cause)
        .answered(response.status.0)
        .reporting_to(Endpoints::stated_by(&response.headers));
    let reports = violations::reports(&page, &about, &answer.objections);
    let mut said = reports.disbelieved;
    said.extend(reports.unusable);
    let mut pool = alo_net::Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| no_trust()))
        .patient_for(Duration::from_millis(500));
    said.extend(pool.report(&reports.posts));
    Reported {
        answer,
        posted: reports.posts.len(),
        said,
    }
}

fn no_trust() -> Trust {
    Trust::of(&[]).unwrap_or_else(|_| no_trust())
}

/// What the collector heard, as text, within two seconds.
fn heard(from: &mpsc::Receiver<Vec<u8>>) -> String {
    String::from_utf8_lossy(
        &from
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_default(),
    )
    .into_owned()
}

// --- The closing condition ---------------------------------------------------

/// The item's first clause: a report-only `script-src 'none'` refuses nothing,
/// so the script runs, and its author is told it would not have.
#[test]
fn a_watched_policy_lets_the_script_run_and_its_author_is_told() {
    let (port, collected) = collector();
    assert!(port != 0, "loopback is unavailable");
    let response = a_response(
        port,
        "Content-Security-Policy-Report-Only",
        "script-src 'none'; report-uri /csp",
    );

    // It ran: asked of a renderer in this process, where the page's globals
    // can be read.
    let (mut renderer, here) = load(Page::from_response(&response, WINDOW));
    assert_eq!(out(&mut renderer), "ran", "{:?}", here.issues);
    assert_eq!(here.objections, vec![script_objection(0)]);

    // And through the boundary, one report, posted by the browser process.
    let reported = loaded_and_reported(&response);
    assert!(reported.said.is_empty(), "{:?}", reported.said);
    assert_eq!(reported.posted, 1, "{:?}", reported.answer.issues);
    assert!(
        reported
            .answer
            .issues
            .iter()
            .any(|issue| issue.starts_with("script 1: runs, but a policy being watched")),
        "{:?}",
        reported.answer.issues
    );
    let sent = heard(&collected);
    assert!(sent.starts_with("POST /csp HTTP/1.1\r\n"), "{sent}");
    assert!(
        sent.contains("Content-Type: application/csp-report"),
        "{sent}"
    );
    assert!(
        sent.contains("\"effective-directive\":\"script-src\""),
        "{sent}"
    );
    assert!(sent.contains("\"blocked-uri\":\"inline\""), "{sent}");
    assert!(sent.contains("\"disposition\":\"report\""), "{sent}");
    assert!(
        sent.contains(&format!(
            "\"document-uri\":\"http://127.0.0.1:{port}/checkout\""
        )),
        "{sent}"
    );
}

/// The item's second clause: an enforced refusal is reported the same way —
/// and the script does not run.
#[test]
fn an_enforced_refusal_is_reported_the_same_way() {
    let (port, collected) = collector();
    assert!(port != 0, "loopback is unavailable");
    let response = a_response(
        port,
        "Content-Security-Policy",
        "script-src 'none'; report-uri /csp",
    );

    let (mut renderer, here) = load(Page::from_response(&response, WINDOW));
    assert_eq!(out(&mut renderer), "no script ran");
    assert_eq!(here.objections, vec![script_objection(0)]);

    let reported = loaded_and_reported(&response);
    assert!(reported.said.is_empty(), "{:?}", reported.said);
    assert_eq!(reported.posted, 1);
    assert!(
        reported
            .answer
            .issues
            .iter()
            .any(|issue| issue.starts_with("script 1: refused:")),
        "{:?}",
        reported.answer.issues
    );
    let sent = heard(&collected);
    assert!(
        sent.contains("\"effective-directive\":\"script-src\""),
        "{sent}"
    );
    assert!(sent.contains("\"blocked-uri\":\"inline\""), "{sent}");
    assert!(sent.contains("\"disposition\":\"enforce\""), "{sent}");
}

/// `report-to` is resolved against the endpoints the **browser process** read
/// from the response, so the Reporting API's document goes where the page's
/// own headers said.
#[test]
fn a_report_to_group_is_posted_where_the_responses_own_headers_say() {
    let (port, collected) = collector();
    assert!(port != 0, "loopback is unavailable");
    let mut response = a_response(
        port,
        "Content-Security-Policy-Report-Only",
        "default-src 'self'; report-to csp",
    );
    response
        .headers
        .add("Reporting-Endpoints", "csp=\"/reports\"");
    let reported = loaded_and_reported(&response);
    assert!(reported.said.is_empty(), "{:?}", reported.said);
    assert_eq!(reported.posted, 1);
    let sent = heard(&collected);
    assert!(sent.starts_with("POST /reports HTTP/1.1\r\n"), "{sent}");
    assert!(sent.contains("application/reports+json"), "{sent}");
    assert!(
        sent.contains("\"effectiveDirective\":\"script-src\""),
        "{sent}"
    );
}

// --- Which policies object -----------------------------------------------------

/// An enforced policy that allows the script and a watched one that would not:
/// it runs, and only the watched one is reported — by its place, after the
/// enforced one.
#[test]
fn only_the_policy_that_objected_is_named() {
    let page = Page::new(A_SCRIPT, WINDOW)
        .with_policy("script-src 'unsafe-inline'")
        .watched_by("script-src 'none'");
    let (mut renderer, answer) = load(page);
    assert_eq!(out(&mut renderer), "ran");
    assert_eq!(answer.objections, vec![script_objection(1)]);
}

/// What a watched policy would have allowed — by nonce, by hash — is not an
/// objection, and nothing is reported.
#[test]
fn a_script_a_watched_policy_allows_is_not_reported() {
    let page = Page::new(
        "<script nonce=n>out = 'n'</script><script>out = 'h'</script>",
        WINDOW,
    )
    .watched_by("script-src 'nonce-n' 'sha256-h5TOCDfr+OZ4139xU5rsIfxy/kMia6G98aipaCdqbJI='");
    let (mut renderer, answer) = load(page);
    assert_eq!(out(&mut renderer), "h");
    assert_eq!(answer.objections, Vec::new(), "{:?}", answer.issues);
    assert!(
        !answer.issues.iter().any(|issue| issue.contains("watched")),
        "{:?}",
        answer.issues
    );
}

/// Each script is its own objection, in document order, and a script refused
/// by two policies is two.
#[test]
fn every_script_and_every_policy_is_its_own_objection() {
    let page = Page::new(
        "<script>out = 'a'</script><script nonce=k>out = 'b'</script>",
        WINDOW,
    )
    .with_policy("script-src 'nonce-k'")
    .watched_by("script-src 'none'");
    let (mut renderer, answer) = load(page);
    assert_eq!(out(&mut renderer), "b");
    assert_eq!(
        answer.objections,
        vec![
            script_objection(0),
            script_objection(1),
            script_objection(1)
        ],
    );
}

/// A `<meta>` policy is obeyed and its refusal said, but it is not in the list
/// the browser process holds, so no objection crosses for it (its `report-to`
/// is queue item 240).
#[test]
fn a_meta_policys_refusal_is_obeyed_and_not_passed_on() {
    let (mut renderer, answer) = load(Page::new(
        "<head><meta http-equiv=Content-Security-Policy content=\"script-src 'none'\">\
         <script>out = 'ran'</script></head>",
        WINDOW,
    ));
    assert_eq!(out(&mut renderer), "no script ran");
    assert_eq!(answer.objections, Vec::new());
    assert!(
        answer
            .issues
            .iter()
            .any(|issue| issue.starts_with("script 1: refused:")),
        "{:?}",
        answer.issues
    );
}

/// A script that is never prepared — every one after the page's script
/// stopped — is objected to by nobody.
#[test]
fn a_script_after_the_page_stopped_is_not_objected_to() {
    let page = Page::new(
        "<script>(function () {}).toString()</script><script>out = 'b'</script>",
        WINDOW,
    )
    .watched_by("script-src 'none'");
    let (_renderer, answer) = load(page);
    assert_eq!(
        answer.objections,
        vec![script_objection(0)],
        "{:?}",
        answer.issues
    );
}

/// A page with more inline scripts than one load may carry objections for:
/// the renderer sends the first [`MOST_OBJECTIONS`] and says how many it left
/// out, and every script still runs.
#[test]
fn a_page_of_many_scripts_sends_no_more_objections_than_a_load_may_carry() {
    let many = MOST_OBJECTIONS + 6;
    let html = "<script>out = (this.out || '') + 'x'</script>".repeat(many);
    let page = Page::new(html, WINDOW).watched_by("script-src 'none'");
    let (mut renderer, answer) = load(page);
    assert_eq!(out(&mut renderer).len(), many);
    assert_eq!(answer.objections.len(), MOST_OBJECTIONS);
    assert!(
        answer
            .issues
            .iter()
            .any(|issue| issue.starts_with("6 more policy objections")),
        "{:?}",
        answer.issues
    );
}

// --- Hostile pages -------------------------------------------------------------

/// Every prefix of a page with policies and scripts loads, and every objection
/// names a policy that exists and is believed by the browser process.
#[test]
fn every_prefix_of_a_watched_page_objects_only_to_what_is_there() {
    let whole = "<head><meta http-equiv=Content-Security-Policy content=\"script-src 'nonce-q'\">\
                 <script nonce=q>out = 'a'</script></head>\
                 <body><script>out += 'b'</script><script nonce=q>out += 'c'</script>\
                 <script src=x.js></script></body>";
    let mut cuts = 0_usize;
    for (at, _) in whole.char_indices() {
        let Some(prefix) = whole.get(..at) else {
            continue;
        };
        let page = Page::new(prefix, WINDOW)
            .with_policy("script-src 'nonce-q' 'unsafe-inline'")
            .watched_by("script-src 'none'; report-uri https://c.example/r")
            .watched_by("default-src 'self'");
        let (_renderer, answer) = load(page.clone());
        assert!(
            !answer
                .issues
                .iter()
                .any(|issue| issue.starts_with("not loaded")),
            "cut at {at}: {:?}",
            answer.issues
        );
        let about = csp_report::Page::at(
            url("https://shop.example/"),
            Cause::Document {
                document: Identities::default().a_document(),
            },
        );
        let reports = violations::reports(&page, &about, &answer.objections);
        assert!(
            reports.disbelieved.is_empty(),
            "cut at {at}: {:?}",
            reports.disbelieved
        );
        cuts += 1;
    }
    assert!(cuts > 200);
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 548: a navigation refused in the window is recorded (ADR 0012
//! § 5, ADR 0020).
//!
//! *The conductor records every refused navigation it takes, as it records
//! refused fetches, with a test reading the line back.*
//!
//! Each test runs the real conductor over the real confined renderers, makes
//! a page ask to go somewhere it may not — a person's click, a page's own
//! script as it loads, an agent's `activate` through the socket — closes the
//! window, and reads the session's record the conductor hands back: one line
//! per refusal, naming its rule, its method and its cause. A navigation that
//! was let go writes no line, because nothing was refused and nothing was
//! requested.

use alo_layout::Size;
use alo_net::activity::{Entry, Happened};
use alo_net::request::Purpose;
use alo_net::{Cause, Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::pointer::{Pointer, Went};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_window::agents::Listening;
use alo_window::aim::{Aimed, aim, on_page};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::inbox::Orders;
use alo_window::message::{News, Order};
use alo_window::place::{Contents, Placement, viewport};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// How long any answer the test expects may take before the test says so.
const AT_MOST: Duration = Duration::from_secs(60);

/// The window, in device pixels, at scale 2: an 800 × 600 page below the
/// strip.
const WINDOW: (u32, u32) = (1600, 1272);
const SCALE: u32 = 2;

/// Where every page here is.
const HERE: &str = "https://example.com/";

/// A link a page may not send its tab to, absolutely placed so a click at
/// (20, 160) on the page is on it.
const DATA_LINK: &str = "<body style=margin:0><a href=\"data:text/html,hi\" \
                         style=position:absolute;left:10px;top:150px;width:100px;height:30px;\
                         display:block>Away</a>";

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// A conductor over real renderers with the corpus's fonts, telling every
/// piece of news to the receiver.
fn conducting() -> Result<(Conductor, Receiver<News>), String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let network =
        Network::over(Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(5)));
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(renderers),
        Fonts::AsStarted,
        network,
        move |said| tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    Ok((conductor, news))
}

fn send(orders: &Orders, order: Order) -> Result<(), String> {
    orders
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())
}

/// What the conductor says next, or why it said nothing.
fn next(news: &Receiver<News>) -> Result<News, String> {
    news.recv_timeout(AT_MOST)
        .map_err(|why| format!("the conductor said nothing in {AT_MOST:?}: {why}"))
}

/// The next thing said to the person, passing over frames.
fn said(news: &Receiver<News>) -> Result<String, String> {
    loop {
        match next(news)? {
            News::Said(said) => return Ok(said),
            News::Painted(_) | News::Strip(_) | News::StripSaid(_) | News::Chosen(_) => {}
            News::Closed => return Err("the window closed".to_owned()),
        }
    }
}

/// Open `html` at [`HERE`] in a window of [`WINDOW`], and wait for it to be
/// painted: the frame.
fn showing(orders: &Orders, news: &Receiver<News>, html: &str) -> Result<Frame, String> {
    send(orders, Order::Resize(viewport(WINDOW, SCALE)))?;
    let url = alo_url::parse(HERE).map_err(|why| format!("not a URL: {why:?}"))?;
    send(
        orders,
        Order::Open {
            url: url.clone(),
            page: Some(Box::new(Page::new(html, Size::ZERO).at(url))),
        },
    )?;
    loop {
        if let News::Painted(frame) = next(news)? {
            return Ok(frame);
        }
    }
}

/// A press and release of the main button at `at`, in the window's device
/// pixels, aimed as the event loop aims what the window server says.
fn click(orders: &Orders, frame: &Frame, at: (f64, f64)) -> Result<(), String> {
    let placed = Placement::of(
        WINDOW,
        SCALE,
        Contents {
            frame: Some((frame.width, frame.height)),
            ..Contents::default()
        },
    );
    let Aimed::Page(down) = aim(WINDOW, &placed, SCALE, at) else {
        return Err(format!("{at:?} is not over the page"));
    };
    send(
        orders,
        Order::Pointer(Pointer {
            went: Went::Down,
            at: down,
        }),
    )?;
    send(
        orders,
        Order::Pointer(Pointer {
            went: Went::Up,
            at: on_page(&placed, SCALE, at),
        }),
    )
}

/// Close the window and take back the session's record: every line in it.
fn closed(conductor: Conductor, orders: &Orders) -> Result<Vec<Entry>, String> {
    send(orders, Order::CloseEverything)?;
    let network = conductor
        .hand_back()
        .ok_or("the conductor did not hand back its network")?;
    Ok(network.pool.activity().entries().cloned().collect())
}

/// The lines that are a refusal, with their rules.
fn refused(lines: &[Entry]) -> Vec<(&Entry, &str)> {
    lines
        .iter()
        .filter_map(|line| match line.happened() {
            Happened::Refused { rule } => Some((line, rule.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_persons_click_refused_is_said_and_recorded_as_the_persons() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(&orders, &news, DATA_LINK).unwrap_or_else(|why| panic!("{why}"));

    // (20, 160) on the page: at scale 2, 40 across and 320 down, below the
    // strip's 72.
    click(&orders, &frame, (40.0, 392.0)).unwrap_or_else(|why| panic!("{why}"));
    let said = said(&news).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        said.starts_with("the page asked, by ")
            && said.contains("to go to \"data:text/html,hi\", and was refused"),
        "{said}"
    );

    let lines = closed(conductor, &orders).unwrap_or_else(|why| panic!("{why}"));
    let refused = refused(&lines);
    let [(line, rule)] = refused.as_slice() else {
        panic!("not one refusal: {lines:#?}");
    };
    assert!(matches!(line.cause(), Cause::Person { .. }), "{line:?}");
    assert_eq!(line.method(), "GET");
    assert_eq!(line.purpose(), &Purpose::Document);
    assert_eq!(line.url().serialised, "data:text/html,hi");
    assert!(said.ends_with(rule), "said {said:?}, recorded {rule:?}");
}

#[test]
fn a_form_refused_is_recorded_under_its_method() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(
        &orders,
        &news,
        "<body style=margin:0><form method=post action=http://example.com/send>\
         <input type=hidden name=password value=hunter2>\
         <button style=position:absolute;left:10px;top:150px;width:100px;height:30px;margin:0>\
         Send</button></form>",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    click(&orders, &frame, (40.0, 392.0)).unwrap_or_else(|why| panic!("{why}"));
    let said = said(&news).unwrap_or_else(|why| panic!("{why}"));
    assert!(said.contains("may not send a form where anybody"), "{said}");

    let lines = closed(conductor, &orders).unwrap_or_else(|why| panic!("{why}"));
    let refused = refused(&lines);
    let [(line, _)] = refused.as_slice() else {
        panic!("not one refusal: {lines:#?}");
    };
    assert_eq!(line.method(), "POST");
    assert_eq!(line.url().serialised, "http://example.com/send");
    assert!(matches!(line.cause(), Cause::Person { .. }), "{line:?}");
    // The body is never recorded (ADR 0012 § 5), and nothing else here
    // carries it.
    assert!(!format!("{lines:?}").contains("hunter2"));
}

#[test]
fn a_pages_own_script_refused_as_it_loads_is_recorded_as_the_documents() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    showing(
        &orders,
        &news,
        "<body><a href=\"javascript:void 0\">Run</a>\
         <script>document.body.firstChild.click();</script>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    let lines = closed(conductor, &orders).unwrap_or_else(|why| panic!("{why}"));
    let refused = refused(&lines);
    let [(line, rule)] = refused.as_slice() else {
        panic!("not one refusal: {lines:#?}");
    };
    assert!(matches!(line.cause(), Cause::Document { .. }), "{line:?}");
    assert_eq!(line.url().serialised, "javascript:void 0");
    assert!(rule.contains("javascript:"), "{rule}");
}

#[test]
fn a_pages_script_refused_as_a_fetch_is_answered_is_recorded_after_the_fetch() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    // The fetch is refused by the browser process, which the page hears as a failure in a task of
    // its own: a delivery, and the link is followed from that.
    showing(
        &orders,
        &news,
        "<body><a href=\"javascript:void 0\">Run</a>\
         <script>fetch('ftp://example.com/x').catch(() => document.body.firstChild.click());\
         </script>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    // Closing waits for nothing the page asked, so the delivery is waited
    // for first: it is painted again after it.
    let deadline = std::time::Instant::now().checked_add(AT_MOST);
    while !matches!(next(&news), Ok(News::Painted(_))) {
        assert!(
            deadline.is_some_and(|deadline| std::time::Instant::now() < deadline),
            "the delivery was never painted"
        );
    }
    let lines = closed(conductor, &orders).unwrap_or_else(|why| panic!("{why}"));
    let refused = refused(&lines);
    let [(fetch, _), (navigation, rule)] = refused.as_slice() else {
        panic!("not two refusals: {lines:#?}");
    };
    assert_eq!(fetch.purpose(), &Purpose::Fetch);
    assert_eq!(navigation.purpose(), &Purpose::Document);
    assert!(
        matches!(navigation.cause(), Cause::Document { .. }),
        "{navigation:?}"
    );
    assert_eq!(navigation.url().serialised, "javascript:void 0");
    assert!(rule.contains("javascript:"), "{rule}");
}

#[test]
fn a_navigation_let_go_writes_no_line() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(
        &orders,
        &news,
        "<body style=margin:0><a href=/elsewhere \
         style=position:absolute;left:10px;top:150px;width:100px;height:30px;display:block>\
         Elsewhere</a>",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    click(&orders, &frame, (40.0, 392.0)).unwrap_or_else(|why| panic!("{why}"));
    let said = said(&news).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        said.starts_with("this link goes to https://example.com/elsewhere"),
        "{said}"
    );
    let lines = closed(conductor, &orders).unwrap_or_else(|why| panic!("{why}"));
    assert!(lines.is_empty(), "{lines:#?}");
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-refused-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line per answer.
fn ask(socket: &Path, lines: &[&str]) -> Result<Vec<String>, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("alo-ask did not start: {why}"))?;
    let mut stdin = asking.stdin.take().ok_or("alo-ask has no input")?;
    stdin
        .write_all(lines.join("\n").as_bytes())
        .map_err(|why| format!("the lines were not sent: {why}"))?;
    drop(stdin);
    let finished = asking
        .wait_with_output()
        .map_err(|why| format!("alo-ask did not finish: {why}"))?;
    if !finished.status.success() {
        return Err(format!(
            "alo-ask: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

#[test]
fn an_agents_activate_refused_is_recorded_as_its_action() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    showing(&orders, &news, DATA_LINK).unwrap_or_else(|why| panic!("{why}"));
    let path = socket("agent").unwrap_or_else(|why| panic!("{why}"));
    let listening =
        Listening::at(&path, orders.clone()).unwrap_or_else(|why| panic!("not listening: {why}"));
    let answers = ask(
        &path,
        &[&format!(
            r#"{{"v":1,"id":1,"ask":"act","tab":"{HERE}","target":{{"name":"Away"}},"verb":"activate"}}"#
        )],
    );
    listening.stop();
    let answers = answers.unwrap_or_else(|why| panic!("{why}"));
    assert!(
        answers
            .first()
            .is_some_and(|answer| answer.starts_with(r#"{"answer":"done","id":1,"#)),
        "{answers:#?}"
    );

    let lines = closed(conductor, &orders).unwrap_or_else(|why| panic!("{why}"));
    let refused = refused(&lines);
    let [(line, rule)] = refused.as_slice() else {
        panic!("not one refusal: {lines:#?}");
    };
    assert!(matches!(line.cause(), Cause::Agent { .. }), "{line:?}");
    assert_eq!(line.url().serialised, "data:text/html,hi");
    assert!(rule.contains("data:"), "{rule}");
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 539: a person's press on the tab strip chooses the tab it was
//! on, claimed by the strip's renderer and decided by the browser process
//! (ADR 0024 § 5).
//!
//! The press is made as the window's event loop makes one: the window's
//! placement says where the strip is, [`aim`] says the point is the
//! strip's and where, and [`Cursor::pressed`] says what is sent. The order
//! goes to the real conductor over the real confined renderers — one for
//! the strip at its own site, one for each page's. What the window is then
//! told is checked: the strip against the same strip drawn here by the
//! engine, so a frame that matches is a strip with exactly those tabs with
//! that one selected; the tab against the frame its page painted.
//!
//! Which page is told what, and in which order, is read from what the
//! pages' `visibilitychange` listeners fetch, from a server on this machine,
//! as `the_browsers_keys.rs` reads it for the keys.
//!
//! A claim the browser process refuses — a place the strip was not shown, a
//! tab closed since, a claim with no press behind it — is one a confined
//! renderer drawing an honest strip never makes, so it is not driven here:
//! `alo-renderer`'s `strip_decide` refuses each by name in its own tests,
//! and the conductor says what it refuses.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Screen;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{Strip, StripTab};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;
use alo_window::aim::{Aimed, aim};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::cursor::{Cursor, Pressed};
use alo_window::inbox::Orders;
use alo_window::message::{News, Order, TabOrder};
use alo_window::place::{Contents, Placement, STRIP_HEIGHT, strip_viewport};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long any answer the test expects may take before the test says so.
const AT_MOST: Duration = Duration::from_secs(60);

/// The size the page is laid out at, below the strip.
const PAGE: Size = Size {
    width: 480.0,
    height: 400.0,
};

/// The window: [`PAGE`] and the strip above it, at scale 2.
const WINDOW: (u32, u32) = (960, (400 + STRIP_HEIGHT) * 2);

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

/// A conductor over real renderers, telling every piece of news — the
/// strip's and the tab's — to the receiver.
fn conducting(renderers: Renderers) -> Result<(Conductor, Receiver<News>), String> {
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

/// The corpus's sans-serif, and what `system-ui` and `sans-serif` mean.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (weight, bytes) in [
        (Weight::NORMAL, dejavu::sans::regular()),
        (Weight::BOLD, dejavu::sans::bold()),
    ] {
        if let Some(font) = Font::load("DejaVu Sans", weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// `tabs` as the engine draws them, here, in a strip above [`PAGE`].
fn drawn(tabs: Vec<StripTab>) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    let strip = Strip {
        tabs,
        viewport: strip_viewport(PAGE),
        scale: 1.0,
    };
    match renderer.handle(ToRenderer::Strip(Box::new(strip))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the strip was not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the strip was not painted: {other:?}")),
    }
}

fn send(orders: &Orders, order: Order) -> Result<(), String> {
    orders
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())
}

/// Press the main button at `at`, in the window's device pixels, as the
/// window's event loop does: aimed from the window's placement, made a
/// press by a cursor, and sent as the order it is. The cursor is let go
/// after, as a person's button is.
fn press(orders: &Orders, at: (f64, f64)) -> Result<(), String> {
    let placed = Placement::of(
        WINDOW,
        2,
        Contents {
            strip: Some((480, STRIP_HEIGHT)),
            frame: Some((480, 400)),
            ..Contents::default()
        },
    );
    let aimed = aim(WINDOW, &placed, 2, at);
    let mut cursor = Cursor::default();
    let sent = cursor.pressed(aimed, true);
    let _ = cursor.released(alo_paint::Point::default());
    match sent {
        Some(Pressed::Strip(point)) => send(orders, Order::StripPressed(point)),
        other => Err(format!(
            "{at:?} is {aimed:?}, and pressing it sent {other:?}"
        )),
    }
}

/// What the conductor says next, or why it said nothing.
fn next(news: &Receiver<News>) -> Result<News, String> {
    news.recv_timeout(AT_MOST)
        .map_err(|why| format!("the conductor said nothing in {AT_MOST:?}: {why}"))
}

/// Everything the conductor says up to and including the next strip.
fn until_strip(news: &Receiver<News>) -> Result<(Vec<News>, Frame), String> {
    let mut before = Vec::new();
    loop {
        match next(news)? {
            News::Strip(frame) => return Ok((before, frame)),
            other => before.push(other),
        }
    }
}

/// A page of `html` at `address`, as an order to open it.
fn opening(address: &str, html: &str) -> Result<(Url, Order), String> {
    let url = alo_url::parse(address).map_err(|why| format!("not a URL: {why:?}"))?;
    let order = Order::Open {
        url: url.clone(),
        page: Some(Box::new(Page::new(html, Size::ZERO).at(url.clone()))),
    };
    Ok((url, order))
}

#[test]
fn the_window_sends_a_press_on_the_band_to_the_strip_in_its_css_pixels() {
    let placed = Placement::of(WINDOW, 2, Contents::default());
    assert_eq!(placed.band.height, 72);
    // Device pixel (200, 40) is the strip's CSS pixel (100, 20); the band's
    // last row is the strip's, and the row below it is the page's.
    for (at, strip) in [
        ((200.0, 40.0), (100.0, 20.0)),
        ((424.0, 71.0), (212.0, 35.5)),
    ] {
        let aimed = aim(WINDOW, &placed, 2, at);
        assert_eq!(
            aimed,
            Aimed::Strip(alo_paint::Point::new(strip.0, strip.1)),
            "{at:?}"
        );
        assert_eq!(
            Cursor::default().pressed(aimed, true),
            Some(Pressed::Strip(alo_paint::Point::new(strip.0, strip.1)))
        );
    }
    assert!(matches!(
        aim(WINDOW, &placed, 2, (424.0, 72.0)),
        Aimed::Page(_)
    ));
}

#[test]
fn a_press_on_a_tab_in_the_strip_chooses_it() {
    let renderers = Renderers::running(renderer().unwrap_or_else(|why| panic!("{why}")), &[])
        .with_machine(alo_window::fonts::frozen());
    let (conductor, news) = conducting(renderers).unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let strip = |tabs| drawn(tabs).unwrap_or_else(|why| panic!("{why}"));
    let blank = || StripTab::new("", Url::about_blank().to_string());
    let strip_after = |at: (f64, f64)| {
        press(&orders, at).unwrap_or_else(|why| panic!("{why}"));
        until_strip(&news).unwrap_or_else(|why| panic!("{why}"))
    };

    send(&orders, Order::Resize(Screen::new(PAGE))).unwrap_or_else(|why| panic!("{why}"));
    let (books, open) = opening(
        "https://books.example/",
        "<title>Invoices</title><p style='background:#2f6f4f;height:40px'>a page</p>",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    send(&orders, open).unwrap_or_else(|why| panic!("{why}"));
    let invoices = || StripTab::new("Invoices", books.to_string());
    // The loading strip, then the page and the strip it loaded into.
    let _ = until_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    let (said, shown) = until_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    let page = match said.as_slice() {
        [News::Painted(page)] => page.clone(),
        other => panic!("the page was not painted: {other:?}"),
    };
    assert_eq!(shown, strip(vec![invoices().selected()]));
    send(&orders, Order::Tabs(TabOrder::New)).unwrap_or_else(|why| panic!("{why}"));
    let (said, shown) = until_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(said, vec![News::Chosen(None)]);
    assert_eq!(shown, strip(vec![invoices(), blank().selected()]));

    // Two tabs of 200 CSS pixels from x 8, 4 apart, from y 6: the first
    // tab's middle is device pixel (216, 42), the second's (624, 42).
    let (said, shown) = strip_after((216.0, 42.0));
    assert_eq!(
        said,
        vec![
            News::Chosen(Some(page.clone())),
            News::Painted(page.clone())
        ]
    );
    assert_eq!(shown, strip(vec![invoices().selected(), blank()]));
    let (said, shown) = strip_after((624.0, 42.0));
    assert_eq!(said, vec![News::Chosen(None)]);
    assert_eq!(shown, strip(vec![invoices(), blank().selected()]));

    // On the gap between them, above them, past the last, and on the tab
    // already chosen: nothing, so the next thing said is the key's.
    for at in [(420.0, 42.0), (216.0, 6.0), (900.0, 42.0), (624.0, 42.0)] {
        press(&orders, at).unwrap_or_else(|why| panic!("{why}"));
    }
    send(&orders, Order::Tabs(TabOrder::Previous)).unwrap_or_else(|why| panic!("{why}"));
    let (said, shown) = until_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        said,
        vec![
            News::Chosen(Some(page.clone())),
            News::Painted(page.clone())
        ]
    );
    assert_eq!(shown, strip(vec![invoices().selected(), blank()]));

    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    loop {
        match next(&news) {
            Ok(News::Closed) => break,
            Ok(_) => {}
            Err(why) => panic!("{why}"),
        }
    }
    assert!(conductor.finish(), "the conductor did not finish");
}

#[test]
fn a_press_before_the_strip_has_painted_is_nobodys() {
    let renderers = Renderers::running(renderer().unwrap_or_else(|why| panic!("{why}")), &[])
        .with_machine(alo_window::fonts::frozen());
    let (conductor, news) = conducting(renderers).unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    // Before the window has said its size nothing is drawn, so a press on
    // the band presses no strip, and the next thing said is the second
    // tab's being chosen.
    send(&orders, Order::Tabs(TabOrder::New)).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(next(&news), Ok(News::Chosen(None)), "the first tab's");
    press(&orders, (216.0, 42.0)).unwrap_or_else(|why| panic!("{why}"));
    send(&orders, Order::Tabs(TabOrder::New)).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(next(&news), Ok(News::Chosen(None)), "the second tab's");
    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(next(&news), Ok(News::Closed), "and nothing else is said");
    assert!(conductor.finish(), "the conductor did not finish");
}

/// A server on this machine at `bind`: its origin, and every path asked of
/// it, written into `log` as it arrives. Each is answered `200` and empty.
fn serve(bind: &str, log: &Arc<Mutex<Vec<String>>>) -> Result<String, String> {
    let listener = TcpListener::bind(bind).map_err(|why| format!("no server at {bind}: {why}"))?;
    let at = listener
        .local_addr()
        .map_err(|why| format!("no address: {why}"))?;
    let log = Arc::clone(log);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            if socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .is_err()
            {
                continue;
            }
            let mut asked = Vec::new();
            let mut block = [0u8; 1024];
            while let Ok(got) = socket.read(&mut block) {
                asked.extend_from_slice(block.get(..got).unwrap_or_default());
                if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                    break;
                }
            }
            let line = String::from_utf8_lossy(&asked);
            let path = line.split(' ').nth(1).unwrap_or_default().to_owned();
            if let Ok(mut log) = log.lock() {
                log.push(path);
            }
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nCache-Control: no-store\r\n\
                  Connection: close\r\n\r\n",
            );
        }
    });
    Ok(format!("http://{at}"))
}

/// The log once it holds `count` paths, or what it held when the test gave up.
fn logged(log: &Arc<Mutex<Vec<String>>>, count: usize) -> Vec<String> {
    let deadline = Instant::now().checked_add(AT_MOST);
    loop {
        let now = log.lock().map(|log| log.clone()).unwrap_or_default();
        if now.len() >= count || deadline.is_none_or(|deadline| Instant::now() > deadline) {
            return now;
        }
        std::thread::yield_now();
    }
}

/// A page whose listener asks its own server for `/` and its visibility and
/// `name` each time it is told its visibility changed.
fn telling(origin: &str, name: &str) -> Result<(Url, Order), String> {
    opening(
        &format!("{origin}/{name}"),
        &format!(
            "<script>document.addEventListener('visibilitychange', function () {{ \
             fetch('/' + document.visibilityState + '-{name}'); }});</script>"
        ),
    )
}

/// Wait for the selected tab to be painted and the strip after it, passing
/// over everything else.
fn painted_and_strip(news: &Receiver<News>) -> Result<(), String> {
    loop {
        if let News::Painted(_) = next(news)? {
            break;
        }
    }
    until_strip(news).map(|_| ())
}

#[test]
fn choosing_by_a_press_tells_the_tab_left_before_the_tab_chosen() {
    let log = Arc::new(Mutex::new(Vec::new()));
    // Two sites, so two renderers: the server by its address, and by the
    // name this machine gives itself.
    let first = serve("127.0.0.1:0", &log).unwrap_or_else(|why| panic!("{why}"));
    let second = first.replace("127.0.0.1", "localhost");
    let renderers = Renderers::running(renderer().unwrap_or_else(|why| panic!("{why}")), &[])
        .with_machine(alo_window::fonts::frozen());
    let (conductor, news) = conducting(renderers).unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    send(&orders, Order::Resize(Screen::new(PAGE))).unwrap_or_else(|why| panic!("{why}"));

    let (_, a) = telling(&first, "a").unwrap_or_else(|why| panic!("{why}"));
    send(&orders, a).unwrap_or_else(|why| panic!("{why}"));
    painted_and_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    let (_, b) = telling(&second, "b").unwrap_or_else(|why| panic!("{why}"));
    send(&orders, b).unwrap_or_else(|why| panic!("{why}"));
    painted_and_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(logged(&log, 1), vec!["/hidden-a"]);

    // A press on the first tab: the second is told it is hidden, then the
    // first that it is seen — the order ⌃⇧Tab tells them in.
    press(&orders, (216.0, 42.0)).unwrap_or_else(|why| panic!("{why}"));
    painted_and_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        logged(&log, 3),
        vec!["/hidden-a", "/hidden-b", "/visible-a"]
    );
    // And on the second again.
    press(&orders, (624.0, 42.0)).unwrap_or_else(|why| panic!("{why}"));
    painted_and_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        logged(&log, 5),
        vec![
            "/hidden-a",
            "/hidden-b",
            "/visible-a",
            "/hidden-a",
            "/visible-b"
        ]
    );

    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    loop {
        match next(&news) {
            Ok(News::Closed) => break,
            Ok(_) => {}
            Err(why) => panic!("{why}"),
        }
    }
    assert!(conductor.finish(), "the conductor did not finish");
}

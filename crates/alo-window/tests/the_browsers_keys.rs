/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 529: the browser's own keys (ADR 0024 §§ 5, 6; ADR 0039 § 1).
//!
//! Each key is a [`Press`] — what the window's event loop makes of what the
//! window server says — given to [`keys::order`], the function the event loop
//! asks, and the order it answers is sent to the real conductor over the real
//! confined renderers. What the window is then told is checked: the strip
//! against the same strip drawn here by the engine, so a frame that matches
//! is a strip with exactly those tabs, in that order, with that one
//! selected; the tab against the frame its page painted.
//!
//! Which page is told what, and in which order, is read from what the pages'
//! `visibilitychange` listeners fetch, from a server on this machine: the
//! conductor makes one request at a time, in the order the pages asked.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{Strip, StripTab};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::inbox::Orders;
use alo_window::keys::{self, Key, Modifier, Platform, Press};
use alo_window::message::{News, Order};
use alo_window::place::strip_viewport;
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

/// The key that writes `letter`, with this platform's shortcut key held.
fn shortcut(letter: &str) -> Press {
    let held = match Platform::HERE {
        Platform::Mac => Modifier::Command,
        Platform::Elsewhere => Modifier::Control,
    };
    Press::character(letter).with(held)
}

/// ⌃Tab.
fn next_tab() -> Press {
    Press::of(Key::Tab).with(Modifier::Control)
}

/// ⌃⇧Tab.
fn previous_tab() -> Press {
    next_tab().with(Modifier::Shift)
}

/// Press `press`, as the window's event loop does: send the order it means.
fn press(orders: &Orders, press: &Press) -> Result<(), String> {
    let order = keys::order(press).ok_or_else(|| format!("{press:?} is not the browser's"))?;
    send(orders, order)
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
fn each_key_changes_the_strip_as_it_says() {
    let renderers = Renderers::running(renderer().unwrap_or_else(|why| panic!("{why}")), &[])
        .with_machine(alo_window::fonts::frozen());
    let (conductor, news) = conducting(renderers).unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let strip = |tabs| drawn(tabs).unwrap_or_else(|why| panic!("{why}"));
    let blank = || StripTab::new("", Url::about_blank().to_string());
    let strip_after = |key: &Press| {
        press(&orders, key).unwrap_or_else(|why| panic!("{why}"));
        until_strip(&news).unwrap_or_else(|why| panic!("{why}"))
    };

    send(&orders, Order::Resize(PAGE)).unwrap_or_else(|why| panic!("{why}"));
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

    // ⌘T: a new tab, empty and selected, and nothing of the page under it.
    let (said, shown) = strip_after(&shortcut("t"));
    assert_eq!(said, vec![News::Chosen(None)]);
    assert_eq!(shown, strip(vec![invoices(), blank().selected()]));

    // ⌃⇧Tab: the page again, its last frame at once and then painted anew.
    let (said, shown) = strip_after(&previous_tab());
    assert_eq!(
        said,
        vec![
            News::Chosen(Some(page.clone())),
            News::Painted(page.clone())
        ]
    );
    assert_eq!(shown, strip(vec![invoices().selected(), blank()]));

    // ⌃Tab: the empty tab; and ⌃Tab from the last tab is the first.
    let (said, shown) = strip_after(&next_tab());
    assert_eq!(said, vec![News::Chosen(None)]);
    assert_eq!(shown, strip(vec![invoices(), blank().selected()]));
    let (said, shown) = strip_after(&next_tab());
    assert_eq!(
        said,
        vec![
            News::Chosen(Some(page.clone())),
            News::Painted(page.clone())
        ]
    );
    assert_eq!(shown, strip(vec![invoices().selected(), blank()]));

    // ⌃⇧Tab from the first is the last.
    let (said, shown) = strip_after(&previous_tab());
    assert_eq!(said, vec![News::Chosen(None)]);
    assert_eq!(shown, strip(vec![invoices(), blank().selected()]));
    let _ = strip_after(&previous_tab());

    // ⌘W: the page's tab closes, and the one after it is selected.
    let (said, shown) = strip_after(&shortcut("w"));
    assert_eq!(said, vec![News::Chosen(None)]);
    assert_eq!(shown, strip(vec![blank().selected()]));

    // ⌘W on the last tab closes the window.
    press(&orders, &shortcut("w")).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(next(&news), Ok(News::Closed));
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

/// Wait for the selected tab to be painted, passing over everything else.
fn painted(news: &Receiver<News>) -> Result<(), String> {
    loop {
        if let News::Painted(_) = next(news)? {
            return Ok(());
        }
    }
}

#[test]
fn choosing_a_tab_tells_the_tab_left_before_the_tab_chosen() {
    let log = Arc::new(Mutex::new(Vec::new()));
    // Two sites, so two renderers: the server by its address, and by the
    // name this machine gives itself.
    let first = serve("127.0.0.1:0", &log).unwrap_or_else(|why| panic!("{why}"));
    let second = first.replace("127.0.0.1", "localhost");
    let renderers = Renderers::running(renderer().unwrap_or_else(|why| panic!("{why}")), &[]);
    let (conductor, news) = conducting(renderers).unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    send(&orders, Order::Resize(PAGE)).unwrap_or_else(|why| panic!("{why}"));

    let (_, a) = telling(&first, "a").unwrap_or_else(|why| panic!("{why}"));
    send(&orders, a).unwrap_or_else(|why| panic!("{why}"));
    painted(&news).unwrap_or_else(|why| panic!("{why}"));
    // Opening a second tab chooses it: the first is told it is hidden, and
    // the second loads visible, which is no change and fires nothing.
    let (_, b) = telling(&second, "b").unwrap_or_else(|why| panic!("{why}"));
    send(&orders, b).unwrap_or_else(|why| panic!("{why}"));
    painted(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(logged(&log, 1), vec!["/hidden-a"]);

    // ⌃⇧Tab: the second is told it is hidden, then the first that it is seen.
    press(&orders, &previous_tab()).unwrap_or_else(|why| panic!("{why}"));
    painted(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        logged(&log, 3),
        vec!["/hidden-a", "/hidden-b", "/visible-a"]
    );

    // ⌘T, then ⌘W on the empty tab: the first is left for it, and nothing
    // is told about a tab with no page. The empty tab was last, so closing it
    // chooses the one before it — the second — which is told it is seen.
    press(&orders, &shortcut("t")).unwrap_or_else(|why| panic!("{why}"));
    press(&orders, &shortcut("w")).unwrap_or_else(|why| panic!("{why}"));
    painted(&news).unwrap_or_else(|why| panic!("{why}"));
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

    // A key that is not the browser's is not sent anywhere.
    for other in [
        Press::character("t"),
        Press::character("a").with(Modifier::Command),
        shortcut("t").with(Modifier::Shift),
        shortcut("w").repeating(),
    ] {
        assert!(keys::order(&other).is_none(), "{other:?}");
    }

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

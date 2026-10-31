/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 717, ADR 0076 B: a page the conversation opens is a card,
//! drawn from the PNG frames Chrome streams of it — through the machine's
//! Chrome and through the Chromium alo carries (alo's CEF host, ADR 0079),
//! each the same test.
//!
//! A shown Chrome holds a page of this test's own server, animating, as the
//! conversation holds a card's (`ChromeServing::start_as_a_card`): its
//! window parked off every display (ADR 0076 B4), its page laid out at the
//! card's page box, 878 × 520. Then, each asserted:
//!
//! - **The window is parked**: by Chromium's word, left of every display,
//!   and the window server lists no window of it on the screen there.
//! - **Frames stream at the card's size times the scale**: the conversation
//!   asks for 1756 × 1040 (the card at two), and every frame told is a PNG
//!   of the page box at the scale of the display the parked window is by —
//!   one, on this Mac with two displays at one beside its own (queue item
//!   738) — which this engine's picture reader reads, at about sixty a second while
//!   the page moves. What arrived is said on the error output: frames a
//!   second, bytes a second.
//! - **The agent's pipe does not queue behind the frames past ADR 0076
//!   B2's numbers.** An agent's reads of the page's tree through the socket
//!   (`alo-ask`'s protocol, ADR 0055), timed with no stream and then while
//!   the page streams: the median a read waits more while streaming is held
//!   to [`MORE_AT_THE_MEDIAN`], the ninetieth centile to [`MORE_AT_P90`] —
//!   B2's own growth at the card's size, measured with jpeg frames on a
//!   bare Node client, with slack for this test's debug build and a gate
//!   running beside it (see each). Measured numbers are said.
//! - **The stream stops when the card is closed**: closing the page
//!   (`Order::CloseEverything`, what the card's *Close* and the window's
//!   closing send) stops the stream first, Chrome answers
//!   `Page.stopScreencast`, and **no frame is told after**; the thread ends.
//!   *Unstream* (the card off the screen) does the same and keeps the page.
//! - **Expand** shows the same tab in Chromium's own window, the stream
//!   stopped, at the place and size asked, by Chromium's word and the window
//!   server's.
//!
//! Never the real network: the page is `http://127.0.0.1:PORT/card.html`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::Chromium;
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_screencast::png_size;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::chrome_window::PARKED;
use alo_window::frame_told::{ToFrame, Uncast};
use alo_window::message::{CardOrder, Order};
use alo_window::window_server;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// The card's page box, as `alo-renderer` lays it out.
const PAGE: (u32, u32) = alo_renderer::card::PAGE_SIZE;

/// What the conversation asks for: the page box at two, ADR 0076 B's size.
const ASKED: (u32, u32) = (1756, 1040);

/// How long a stream is watched to count its frames.
const WATCHED: Duration = Duration::from_secs(3);

/// How many reads an agent makes, with the stream and without.
const READS: usize = 40;

/// ADR 0076 B2: `Runtime.evaluate` on the pipe the frames came down, sixty
/// times, the page animating — medians of 0.75–1.39 ms with no stream and
/// 1.76–7.09 ms with one, so a stream made a median command wait up to
/// about 6.3 ms more (7.09 − 0.75). Here a read of the tree is timed through
/// the socket and this browser's own reading of every frame (base64 and
/// JSON, in a debug build): twice B2's growth.
const MORE_AT_THE_MEDIAN: Duration = Duration::from_micros(12_700);

/// B2's ninetieth centile: 1.15–1.68 ms with no stream, 7.7–9.6 with one —
/// up to about 8.5 ms more (9.6 − 1.15). Twice it, as above.
const MORE_AT_P90: Duration = Duration::from_millis(17);

/// The page: a box turning forever and a field of noise moved every display
/// frame, so Chrome always has a frame to send and each is about as large as
/// ADR 0076 B's — a PNG cannot make noise smaller — and a heading an agent
/// reads. The noise is made once and only moved, so the page's own thread
/// does next to nothing each frame and what an agent's read waits for is the
/// pipe, not the page.
const CARD_PAGE: &str = "<!doctype html><title>A card</title>\
    <style>body{margin:0}h1{font:24px sans-serif;margin:16px}\
    #turning{width:120px;height:120px;margin:20px;background:#2a9d8f;\
      animation:turn 1s linear infinite}\
    @keyframes turn{to{transform:rotate(360deg)}}\
    canvas{position:absolute;left:300px;top:80px}</style>\
    <h1>A page in a card</h1><div id=turning></div>\
    <canvas id=noise width=360 height=300></canvas>\
    <script>const made = document.createElement('canvas');\
      made.width = 720; made.height = 600;\
      const making = made.getContext('2d');\
      const field = making.createImageData(720, 600);\
      for (let at = 0; at < field.data.length; at += 4) {\
        const seen = Math.random() * 4294967296;\
        field.data[at] = seen & 255; field.data[at + 1] = (seen >> 8) & 255;\
        field.data[at + 2] = (seen >> 16) & 255; field.data[at + 3] = 255; }\
      making.putImageData(field, 0, 0);\
      const noise = document.getElementById('noise').getContext('2d');\
      let step = 0;\
      function draw() { step = (step + 37) % 300;\
        noise.drawImage(made, -step, -step); requestAnimationFrame(draw); }\
      requestAnimationFrame(draw);</script>";

/// A server on this machine answering `/card.html` with [`CARD_PAGE`]: its
/// port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let Some(path) = asked.split(' ').nth(1) else {
                    return;
                };
                let (status, body) = if path == "/card.html" {
                    ("200 OK", CARD_PAGE)
                } else {
                    ("404 Not Found", "")
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

/// A page held as a card, what it tells, and its page's address.
struct Card {
    serving: ChromeServing,
    told: Receiver<ToFrame>,
    address: String,
}

/// `chromium` started holding the test's page as the conversation holds a
/// card's.
fn card(chromium: &Chromium, port: u16) -> Result<Card, String> {
    let address = format!("http://127.0.0.1:{port}/card.html");
    let url = alo_url::parse(&address).map_err(|why| format!("{why:?}"))?;
    let (tell, told) = channel();
    let serving = ChromeServing::start_as_a_card(
        chromium.clone(),
        Opening::Network(url),
        PAGE,
        AgentSites::Every,
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no card in Chromium: {why}"))?;
    Ok(Card {
        serving,
        told,
        address,
    })
}

/// Every frame told within `long`, and whatever else was told.
fn frames_within(
    told: &Receiver<ToFrame>,
    long: Duration,
) -> (Vec<(Instant, Vec<u8>)>, Vec<ToFrame>) {
    let until = Instant::now() + long;
    let mut frames = Vec::new();
    let mut other = Vec::new();
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::Cast(frame)) => {
                frames.push((frame.arrived, frame.png().unwrap_or_default()));
            }
            Ok(news) => other.push(news),
            Err(_) => break,
        }
    }
    (frames, other)
}

/// What a stream that stopped said of itself, waited for: the frames told
/// before it, and the stop.
fn stopped(told: &Receiver<ToFrame>) -> Result<(usize, Uncast), String> {
    let until = Instant::now() + Duration::from_secs(60);
    let mut before = 0;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::Cast(_)) => before += 1,
            Ok(ToFrame::Uncast(uncast)) => return Ok((before, uncast)),
            Ok(_) => {}
            Err(why) => return Err(format!("the stream never said it stopped: {why}")),
        }
    }
    Err("the stream never said it stopped".to_owned())
}

/// The agent's socket, listening for `orders`, at a place of the test's
/// own named `name`.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-card-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// How long each of [`READS`] reads of `address`'s tree takes through the
/// socket at `path`, each sent once the last is answered, in order.
fn reads(path: &PathBuf, address: &str, first: usize) -> Result<Vec<Duration>, String> {
    let stream = UnixStream::connect(path).map_err(|why| format!("not connected: {why}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|why| why.to_string())?;
    let mut writer = stream.try_clone().map_err(|why| why.to_string())?;
    let mut answers = BufReader::new(stream);
    let mut took = Vec::with_capacity(READS);
    for id in first..first + READS {
        let line = format!("{{\"v\":1,\"id\":{id},\"ask\":\"read\",\"tab\":\"{address}\"}}\n");
        let sent = Instant::now();
        writer
            .write_all(line.as_bytes())
            .map_err(|why| format!("not sent: {why}"))?;
        let mut answer = String::new();
        answers
            .read_line(&mut answer)
            .map_err(|why| format!("no answer: {why}"))?;
        took.push(sent.elapsed());
        if !answer.contains("A page in a card") {
            return Err(format!("read {id} was not the page: {answer}"));
        }
    }
    Ok(took)
}

/// The value `share` of the way up `times`, sorted.
fn centile(times: &[Duration], share: f64) -> Duration {
    let mut sorted = times.to_vec();
    sorted.sort();
    let last = sorted.len().saturating_sub(1);
    let at = (0..=last)
        .find(|&at| {
            f64::from(u32::try_from(at).unwrap_or(u32::MAX))
                >= share * f64::from(u32::try_from(last).unwrap_or(0))
        })
        .unwrap_or(last);
    sorted.get(at).copied().unwrap_or_default()
}

/// Whether the window server lists a window of `process` on the screen
/// whose bounds are `bounds`, waited for up to ten seconds.
fn listed_at(process: u32, bounds: Bounds) -> Result<bool, String> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let listed = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
        if listed
            .iter()
            .any(|window| window.layer == 0 && window.bounds == Some(bounds))
        {
            return Ok(true);
        }
        if Instant::now() > until {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The card's window parked: left of every display, by Chromium's word, and
/// not listed on the screen anywhere a person sees.
fn parked(serving: &ChromeServing, who: &str) -> Result<(), String> {
    let process = serving.process();
    let window = serving
        .placed()
        .map(|placed| placed.window.bounds)
        .ok_or("a shown Chromium's window was not read")?;
    assert!(
        window.left + window.width <= 0 && i64::from(PARKED.0) <= window.left,
        "{who}'s window was not parked: {window:?}"
    );
    let shown = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
    assert!(
        shown
            .iter()
            .filter(|listed| listed.layer == 0)
            .all(|listed| listed.bounds.is_none_or(|bounds| bounds.left < 0)),
        "{who}'s window is on a display: {shown:?}"
    );

    Ok(())
}

/// A number of frames or bytes, as a float, for what is said of them.
fn float(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}

/// The frames told within [`WATCHED`] of a stream asked for at [`ASKED`]:
/// each a PNG of the page box at the screen's scale, at least twenty a
/// second, and what they came to said.
fn streamed(told: &Receiver<ToFrame>, who: &str) {
    let (frames, other) = frames_within(told, WATCHED);
    // Where the page is, told as it is first streamed (queue item 742), and
    // whether it is loading (queue item 751), are the card's own business;
    // nothing else is told.
    assert!(
        other
            .iter()
            .all(|news| matches!(news, ToFrame::Moved(_) | ToFrame::Loading(_))),
        "told something else: {other:?}"
    );
    let sizes: Vec<Option<(u32, u32)>> = frames.iter().map(|(_, png)| png_size(png)).collect();
    assert!(
        sizes
            .iter()
            .all(|size| *size == Some(PAGE) || *size == Some(ASKED)),
        "{who}'s frames are not the page box at the screen's scale: {:?}",
        sizes.iter().take(3).collect::<Vec<_>>()
    );
    // Each is a picture this engine reads, as the card's renderer reads it,
    // at the size its header states.
    let read = frames.last().map(|(_, png)| {
        alo_paint::picture::read(png).map(|canvas| (canvas.width(), canvas.height()))
    });
    assert_eq!(
        read.map(Result::ok),
        Some(sizes.last().copied().flatten()),
        "{who}'s last frame is not a picture this engine reads"
    );
    let bytes: usize = frames.iter().map(|(_, png)| png.len()).sum();
    let span = match (frames.first(), frames.last()) {
        (Some((first, _)), Some((last, _))) => last.duration_since(*first),
        _ => Duration::ZERO,
    };
    let rate = float(frames.len().saturating_sub(1)) / span.as_secs_f64().max(0.001);
    eprintln!(
        "{who}: {} frames of {:?} in {:.2} s — {rate:.1} a second, {:.1} MiB a second, \
         {:.0} KiB a frame",
        frames.len(),
        sizes.first().copied().flatten(),
        span.as_secs_f64(),
        float(bytes) / span.as_secs_f64().max(0.001) / 1_048_576.0,
        float(bytes) / float(frames.len().max(1)) / 1024.0
    );
    assert!(
        rate >= 20.0,
        "{who} streamed only {rate:.1} frames a second of a page that moves"
    );
}

/// The agent's reads while streaming held to ADR 0076 B2's growth against
/// its reads with no stream, and both said.
fn held_to_b2(idle: &[Duration], streaming: &[Duration], who: &str) {
    let (idle_median, idle_p90) = (centile(idle, 0.5), centile(idle, 0.9));
    let (median, p90) = (centile(streaming, 0.5), centile(streaming, 0.9));
    eprintln!(
        "{who}: an agent's read, {READS} each — no stream: median {:.2} ms, p90 {:.2}, max {:.2}; \
         streaming: median {:.2} ms, p90 {:.2}, max {:.2}",
        idle_median.as_secs_f64() * 1000.0,
        idle_p90.as_secs_f64() * 1000.0,
        centile(idle, 1.0).as_secs_f64() * 1000.0,
        median.as_secs_f64() * 1000.0,
        p90.as_secs_f64() * 1000.0,
        centile(streaming, 1.0).as_secs_f64() * 1000.0,
    );
    assert!(
        median.saturating_sub(idle_median) <= MORE_AT_THE_MEDIAN,
        "{who}: a read waited {median:?} at the median while streaming, {idle_median:?} without"
    );
    assert!(
        p90.saturating_sub(idle_p90) <= MORE_AT_P90,
        "{who}: a read waited {p90:?} at p90 while streaming, {idle_p90:?} without"
    );
}

/// Everything this item asks of a card, through `chromium`, named `who` in
/// what is said.
fn a_card_through(chromium: &Chromium, who: &str) -> Result<(), String> {
    let port = serve()?;
    let Card {
        serving,
        told,
        address,
    } = card(chromium, port)?;
    parked(&serving, who)?;

    // The agent's reads with no stream.
    let path = socket(who)?;
    let listening =
        Listening::at(&path, serving.orders()).map_err(|why| format!("not listening: {why}"))?;
    let idle = reads(&path, &address, 1)?;

    // Streamed at the card's size at two.
    let orders = serving.orders();
    orders
        .send(Order::Card(CardOrder::Stream {
            width: ASKED.0,
            height: ASKED.1,
        }))
        .map_err(|_| "the card's thread has gone")?;
    streamed(&told, who);

    // The agent's reads while the page streams, the frames told meanwhile
    // counted until they are done.
    let reading = Arc::new(AtomicBool::new(true));
    let still = Arc::clone(&reading);
    let watching = std::thread::spawn(move || {
        let mut counted = 0usize;
        while still.load(Ordering::SeqCst) {
            if let Ok(ToFrame::Cast(_)) = told.recv_timeout(Duration::from_millis(20)) {
                counted += 1;
            }
        }
        (told, counted)
    });
    let streaming = reads(&path, &address, 1 + READS)?;
    held_to_b2(&idle, &streaming, who);
    listening.stop();
    reading.store(false, Ordering::SeqCst);
    let (told, meanwhile) = watching.join().map_err(|_| "the watcher panicked")?;
    eprintln!("{who}: {meanwhile} frames told while the agent read");

    // Off the screen: stopped, Chrome's word heard, nothing told after —
    // and the page kept.
    orders
        .send(Order::Card(CardOrder::Unstream))
        .map_err(|_| "the card's thread has gone")?;
    let (_, unstreamed) = stopped(&told)?;
    assert!(unstreamed.answered, "{who} did not answer the stop");
    let (after, _) = frames_within(&told, Duration::from_secs(1));
    assert!(
        after.is_empty(),
        "{who}: {} frames told after the stop",
        after.len()
    );

    // Back on the screen, then the card closed: the stream stops first,
    // Chrome answers, and nothing is told after; the thread ends.
    orders
        .send(Order::Card(CardOrder::Stream {
            width: ASKED.0,
            height: ASKED.1,
        }))
        .map_err(|_| "the card's thread has gone")?;
    let (again, _) = frames_within(&told, Duration::from_secs(2));
    assert!(!again.is_empty(), "{who} streamed nothing the second time");
    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    let (_, closed) = stopped(&told)?;
    assert!(
        closed.answered,
        "{who} did not answer the stop as the card closed"
    );
    eprintln!(
        "{who}: closed after {} frames, {} read after Chrome said it stopped",
        closed.frames, closed.after
    );
    serving
        .records()
        .ok_or_else(|| format!("{who}'s thread did not finish"))?;
    let after: Vec<ToFrame> = told.try_iter().collect();
    assert!(
        !after.iter().any(|news| matches!(news, ToFrame::Cast(_))),
        "{who}: a frame was told after the card closed"
    );
    Ok(())
}

/// The card expanded: the same tab in Chromium's own window, at the place
/// and size asked, the stream stopped.
fn expanded_through(chromium: &Chromium, who: &str) -> Result<(), String> {
    let port = serve()?;
    let Card { serving, told, .. } = card(chromium, port)?;
    let orders = serving.orders();
    orders
        .send(Order::Card(CardOrder::Stream {
            width: ASKED.0,
            height: ASKED.1,
        }))
        .map_err(|_| "the card's thread has gone")?;
    let (frames, _) = frames_within(&told, Duration::from_secs(1));
    assert!(!frames.is_empty(), "{who} streamed nothing");
    orders
        .send(Order::Card(CardOrder::Expand {
            at: (120, 80),
            size: (900, 600),
        }))
        .map_err(|_| "the card's thread has gone")?;
    let (_, unstreamed) = stopped(&told)?;
    assert!(unstreamed.answered);
    // An app window with no sides: its page's width is the window's.
    let process = serving.process();
    let until = Instant::now() + Duration::from_secs(20);
    let mut found = None;
    while found.is_none() && Instant::now() < until {
        let listed = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
        found = listed
            .into_iter()
            .filter(|window| window.layer == 0)
            .filter_map(|window| window.bounds)
            .find(|bounds| (bounds.left, bounds.top, bounds.width) == (120, 80, 900));
        std::thread::sleep(Duration::from_millis(100));
    }
    let bounds = found.ok_or_else(|| format!("{who}'s window is not over the conversation"))?;
    assert!(listed_at(process, bounds)?);
    let (after, _) = frames_within(&told, Duration::from_millis(500));
    assert!(after.is_empty(), "{who} streamed an expanded page");
    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    serving
        .records()
        .ok_or_else(|| format!("{who}'s thread did not finish"))?;
    Ok(())
}

#[test]
fn a_card_streamed_read_stopped_and_closed_through_chrome() -> Result<(), String> {
    a_card_through(&Chromium::Machines, "Chrome")
}

#[test]
fn a_card_streamed_read_stopped_and_closed_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("717")? else {
        return Ok(());
    };
    a_card_through(&host.chromium(), "alo's CEF host")
}

#[test]
fn a_card_expanded_through_chrome() -> Result<(), String> {
    expanded_through(&Chromium::Machines, "Chrome")
}

#[test]
fn a_card_expanded_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("717")? else {
        return Ok(());
    };
    expanded_through(&host.chromium(), "alo's CEF host")
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 952, ADR 0076's sixth amendment: a card's frames are the
//! page's own sRGB colours whichever display its window is parked by —
//! through the machine's Chrome and through the Chromium alo carries.
//!
//! Left to itself a Chromium draws, and streams, in the colour space of the
//! display its window is on: parked by a laptop's built-in panel, a wide
//! gamut one, a page's `#2a9d8f` arrived as (79, 155, 143), and alo's
//! window — whose pixels the window server matches to the display as sRGB
//! (ADR 0105) — matched it again. Every card's Chromium is now started
//! told sRGB (`chrome::SRGB`). Here, for each Chromium:
//!
//! - **It was started with the switch**: the process's own command line,
//!   as the system lists it, holds `--force-color-profile=srgb`.
//! - **Every frame is the page's colour exactly, on every display** the
//!   card's window can be parked by — asked just past one of its edges,
//!   where no display is, so that the window server keeps a strip of it on
//!   that display, as `chrome_park.rs` parks a card — read back, and the
//!   display holding the most of it the one asked. A frame streamed after
//!   each park is read at its middle and must be `#2a9d8f`, not a level off.
//! - **A wide-gamut display, where there is one, is among them**: which
//!   displays can show Display P3 is asked of `AppKit` through `osascript`
//!   (`NSScreen`'s `canRepresentDisplayGamut:`), and every such display
//!   must have been parked by. With none — a Mac with only sRGB displays —
//!   the exact colour on every display is still asserted.
//!
//! The page animates one corner pixel so that Chromium streams on after
//! each park. Never the real network: the page is `http://127.0.0.1:PORT/`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::chrome::{Chrome, SRGB};
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_tab::Tab;
use alo_window::chrome_window;
use alo_window::scale::Scale;
use alo_window::window_server::{self, Display};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};

/// The card's page box, as `alo-renderer` lays it out.
const PAGE: (u32, u32) = alo_renderer::card::PAGE_SIZE;

/// The page's own colour, `#2a9d8f`.
const TEAL: (u8, u8, u8) = (0x2a, 0x9d, 0x8f);

/// The page: the colour everywhere but a corner pixel that changes, so
/// that a frame is streamed after the window moves.
const PAGE_HTML: &str = "<!doctype html><title>Teal</title>\
    <style>html,body{margin:0;height:100%;background:#2a9d8f}\
    i{position:fixed;right:0;top:0;width:2px;height:2px;\
    animation:t .2s steps(2) infinite}\
    @keyframes t{from{background:#000}to{background:#fff}}</style><i></i>";

/// How long a frame streamed after a park is waited for.
const AT_MOST: Duration = Duration::from_secs(20);

/// How long after a park its frames are let settle before one is read:
/// Chromium draws again in the new display's space once it has moved.
const SETTLE: Duration = Duration::from_millis(1500);

/// A server on this machine answering every request with the page: its
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
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n\
                     {PAGE_HTML}",
                    PAGE_HTML.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

/// `AppKit`'s word on each display, in the window server's points (the
/// main display's top left at the origin): its bounds, and whether it can
/// show Display P3.
const GAMUTS: &str = r#"ObjC.import("AppKit");
var screens = $.NSScreen.screens, lines = [];
var high = screens.objectAtIndex(0).frame.size.height;
for (var i = 0; i < screens.count; i++) {
  var s = screens.objectAtIndex(i), f = s.frame;
  lines.push([f.origin.x, high - f.origin.y - f.size.height, f.size.width,
    f.size.height, s.canRepresentDisplayGamut($.NSDisplayGamutP3) ? 1 : 0].join(" "));
}
lines.join("\n")"#;

/// The bounds of every display that can show Display P3.
fn wide_gamut() -> Result<Vec<Bounds>, String> {
    let out = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", GAMUTS])
        .output()
        .map_err(|why| format!("osascript: {why}"))?;
    let said = String::from_utf8_lossy(&out.stdout);
    let mut wide = Vec::new();
    for line in said.lines().filter(|line| !line.trim().is_empty()) {
        let numbers: Vec<f64> = line
            .split(' ')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|why| format!("AppKit said {line:?}: {why}"))?;
        let [left, top, width, height, p3] = numbers.as_slice() else {
            return Err(format!("AppKit said {line:?}"));
        };
        if *p3 > 0.5 {
            // Points, whole on every display macOS lays out.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a display's bounds are whole points"
            )]
            wide.push(Bounds {
                left: left.round() as i64,
                top: top.round() as i64,
                width: width.round() as i64,
                height: height.round() as i64,
            });
        }
    }
    Ok(wide)
}

/// How much of `one` lies on `other`, in square points.
fn overlap(one: Bounds, other: Bounds) -> i64 {
    let width = (one.left + one.width).min(other.left + other.width) - one.left.max(other.left);
    let height = (one.top + one.height).min(other.top + other.height) - one.top.max(other.top);
    width.max(0) * height.max(0)
}

/// The display holding the most of `window`, by its place in `displays`.
fn holding(window: Bounds, displays: &[Display]) -> Option<usize> {
    displays
        .iter()
        .enumerate()
        .map(|(at, display)| (at, overlap(window, display.bounds)))
        .filter(|(_, area)| *area > 0)
        .max_by_key(|(_, area)| *area)
        .map(|(at, _)| at)
}

/// Where a window `size` large is asked to be parked by `display`: just
/// past its right, left and bottom edges, each only where it meets no
/// display at all.
fn places(display: Bounds, displays: &[Display], (width, height): (i64, i64)) -> Vec<(i32, i32)> {
    [
        (display.left + display.width, display.top),
        (display.left - width, display.top),
        (display.left, display.top + display.height),
    ]
    .into_iter()
    .filter(|&(left, top)| {
        let asked = Bounds {
            left,
            top,
            width,
            height,
        };
        displays
            .iter()
            .all(|other| overlap(asked, other.bounds) == 0)
    })
    .filter_map(|(left, top)| Some((i32::try_from(left).ok()?, i32::try_from(top).ok()?)))
    .collect()
}

/// The colour at the middle of `png`, a frame of the page.
fn middle(png: &[u8]) -> Result<(u8, u8, u8), String> {
    let canvas = alo_paint::picture::read(png).map_err(|why| format!("{why:?}"))?;
    let pixel = canvas
        .at(canvas.width() / 2, canvas.height() * 3 / 4)
        .ok_or("a frame with no middle")?;
    let (red, green, blue, _) = pixel.to_rgba8();
    Ok((red, green, blue))
}

/// The newest frame streamed after `since` and [`SETTLE`]: its colour.
fn colour_after(tab: &mut Tab<'_>, since: Instant) -> Result<(u8, u8, u8), String> {
    let from = since + SETTLE;
    let until = Instant::now() + AT_MOST;
    let mut newest = None;
    while Instant::now() < until {
        if let Some(frame) = tab.take_frame().map_err(|why| why.to_string())?
            && frame.arrived >= from
        {
            newest = Some(frame);
        }
        if newest.is_some() && Instant::now() >= from + Duration::from_millis(300) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let frame = newest.ok_or("no frame streamed after the park")?;
    middle(&frame.png().map_err(|why| format!("{why:?}"))?)
}

/// The command line of the process `process` as the system lists it, once
/// it is no longer the shell [`Chrome`] starts it through.
fn command_line(process: u32) -> Result<String, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let out = Command::new("/bin/ps")
            .args(["-o", "command=", "-p", &process.to_string()])
            .output()
            .map_err(|why| format!("ps: {why}"))?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !line.starts_with("/bin/sh") || Instant::now() >= until {
            return Ok(line);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn through(chromium: &Chromium, who: &str) -> Result<(), String> {
    let port = serve()?;
    let displays = window_server::displays().map_err(|why| format!("{why:?}"))?;
    let wide = wide_gamut()?;
    let mut chrome = Chrome::start_shown_at(chromium, Scale::ONE, None)
        .map_err(|why| format!("{who}: {why}"))?;

    // Started told sRGB, by the system's own word for its command line —
    // once the shell that starts it has become it.
    let line = command_line(chrome.process())?;
    assert!(
        line.split_whitespace().any(|switch| switch == SRGB),
        "{who}: started without {SRGB}: {line}"
    );

    let address =
        alo_url::parse(&format!("http://127.0.0.1:{port}/")).map_err(|why| format!("{why:?}"))?;
    let mut tab =
        Tab::open_as_a_card(&mut chrome, &address, PAGE).map_err(|why| format!("{who}: {why}"))?;
    tab.show_as_a_card();
    // Streamed once Chromium takes it: just committed, a page may not be
    // one it streams yet ("Not attached to an active page").
    let until = Instant::now() + AT_MOST;
    while let Err(why) = tab.cast(PAGE.0, PAGE.1) {
        if Instant::now() >= until {
            return Err(format!("{who}: never streamed: {why}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let size = chrome_window::placed(&mut tab)
        .map(|placed| (placed.window.bounds.width, placed.window.bounds.height))
        .map_err(|why| format!("{who}: {why:?}"))?;

    let mut parked_by = Vec::new();
    for (at, display) in displays.iter().enumerate() {
        for (left, top) in places(display.bounds, &displays, size) {
            let asked = Instant::now();
            let placed = chrome_window::put_at(&mut tab, left, top)
                .map_err(|why| format!("{who}: {why:?}"))?;
            if holding(placed.window.bounds, &displays) != Some(at) {
                continue;
            }
            let colour = colour_after(&mut tab, asked)?;
            eprintln!(
                "{who}: parked by {:?} at {:?}, the page's middle {colour:?}",
                display.bounds, placed.window.bounds
            );
            assert_eq!(
                colour,
                TEAL,
                "{who}: parked by the display at {:?} (wide gamut: {}), the frame is not \
                 the page's sRGB colour",
                display.bounds,
                wide.contains(&display.bounds)
            );
            parked_by.push(display.bounds);
            break;
        }
    }
    assert!(
        !parked_by.is_empty(),
        "{who}: parked by no display of {displays:?}"
    );
    for display in &wide {
        assert!(
            parked_by.contains(display),
            "{who}: the wide-gamut display at {display:?} was never parked by; parked by \
             {parked_by:?}"
        );
    }
    drop(tab);
    let _ = chrome.close();
    Ok(())
}

#[test]
fn a_cards_colours_in_srgb_on_every_display_through_chrome() -> Result<(), String> {
    through(&Chromium::Machines, "Chrome")
}

#[test]
fn a_cards_colours_in_srgb_on_every_display_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("952")? else {
        return Ok(());
    };
    through(&host.chromium(), "alo's CEF host")
}

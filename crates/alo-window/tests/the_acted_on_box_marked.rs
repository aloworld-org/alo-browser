/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The box an agent's verb acted on is ringed in the window: queue item 624.
//!
//! *The ring's box is asserted in numbers against the node's own `rect` for
//! a named target, a refused verb marks nothing, and a reference render
//! shows it.*
//!
//! An agent in a process of its own — `alo-ask` — reads a page through the
//! browser's socket and then puts a name into its field **by the field's
//! name**, never by its place, in a conductor over the real renderer
//! program. The rectangle the agent was told the field has is read out of
//! its own answer, and the ring the window is sent is checked against it:
//! the same edges, times the window's scale, at one and at two, placed over
//! the frame and drawn there with the box's inside left as the page painted
//! it. A verb the page refuses is sent no ring, and takes away the ring of
//! the verb before it with the frame it was drawn on. The reference renders
//! are `the_window_composed.rs`'s.

use alo_layout::{Rect, Size};
use alo_net::{Pool, Trust};
use alo_renderer::Page;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::agents::Listening;
use alo_window::beside::renderer_beside;
use alo_window::colours::mark;
use alo_window::compose::compose;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::mark::{Marked, marked, ring};
use alo_window::message::{News, Order};
use alo_window::notice::Lettering;
use alo_window::place::{PixelRect, Placement, STRIP_HEIGHT, screen};
use alo_window::scale::Scale;
use alo_window::showing::Showing;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// Where the page is.
const HERE: &str = "https://example.com/";

/// A field after the words that name it, so its left edge is wherever the
/// words end — not a whole pixel — and a second, placed exactly, that
/// nobody acts on.
const PAGE: &str = "<!doctype html><body style=margin:20px>\
     <label>Name <input></label>\
     <input aria-label=Other style=position:absolute;left:20px;top:200px;width:120px;height:24px>";

/// A conductor over the real renderer program with [`PAGE`] open in its one
/// tab in a window of `window` device pixels at `scale`, and what the window
/// had been sent once that tab was first painted.
fn browsing(
    window: (u32, u32),
    scale: Scale,
) -> Result<(Conductor, Receiver<News>, Showing), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let url = alo_url::parse(HERE).map_err(|why| format!("{why:?}"))?;
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        Network::over(Pool::with_trust(Trust::of(&[])?)),
        move |said| said.is_strip() || tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    for order in [
        Order::Resize(screen(window, scale)),
        Order::Open {
            url: url.clone(),
            page: Some(Box::new(Page::new(PAGE, Size::ZERO).at(url))),
        },
    ] {
        conductor
            .orders()
            .send(order)
            .map_err(|_| "the conductor is gone".to_owned())?;
    }
    let mut showing = Showing::default();
    loop {
        let heard = news
            .recv_timeout(AT_MOST)
            .map_err(|why| format!("the page was not painted: {why}"))?;
        let painted = matches!(heard, News::Painted(_));
        showing.hear(heard);
        if painted {
            return Ok((conductor, news, showing));
        }
    }
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-mark-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line per answer.
fn ask(socket: &Path, lines: &[String]) -> Result<Vec<String>, String> {
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

/// Reading the page.
fn read(id: u32) -> String {
    format!(r#"{{"v":1,"id":{id},"ask":"read","tab":"{HERE}"}}"#)
}

/// Putting `text` into the field named `name`.
fn put(id: u32, name: &str, text: &str) -> String {
    format!(
        r#"{{"v":1,"id":{id},"ask":"act","tab":"{HERE}","target":{{"name":"{name}"}},"verb":"put_text","text":"{text}"}}"#
    )
}

/// What the window was sent while an agent's session ran.
struct Session {
    /// What the agent was answered, a line per request.
    answers: Vec<String>,
    /// Every ring the window was sent, in order.
    rings: Vec<Vec<Marked>>,
    /// What the window was showing as it closed.
    showing: Showing,
}

/// One agent's session over a window of `window` at `scale`: `lines` asked
/// through the socket.
fn session(
    name: &str,
    window: (u32, u32),
    scale: Scale,
    lines: &[String],
) -> Result<Session, String> {
    let (conductor, news, mut showing) = browsing(window, scale)?;
    if !showing.marked().is_empty() {
        return Err("a ring before any agent acted".to_owned());
    }
    let path = socket(name)?;
    let listening =
        Listening::at(&path, conductor.orders()).map_err(|why| format!("not listening: {why}"))?;
    let answers = ask(&path, lines);
    listening.stop();
    conductor
        .orders()
        .send(Order::CloseEverything)
        .map_err(|_| "the conductor is gone".to_owned())?;
    let mut rings = Vec::new();
    loop {
        let heard = news
            .recv_timeout(AT_MOST)
            .map_err(|why| format!("the window was not closed: {why}"))?;
        if let News::Marked(ring) = &heard {
            rings.push(ring.clone());
        }
        if heard == News::Closed {
            break;
        }
        showing.hear(heard);
    }
    if !conductor.finish() {
        return Err("the conductor did not finish".to_owned());
    }
    Ok(Session {
        answers: answers?,
        rings,
        showing,
    })
}

/// The rectangle a tree answer gives the node named `name`, as the agent
/// was told it: `"name":…,"node":…,"rect":{"height":…,"width":…,"x":…,"y":…}`.
fn told(tree: &str, name: &str) -> Result<Rect, String> {
    let named = format!(r#""name":"{name}","node":"#);
    let from = tree
        .find(&named)
        .ok_or_else(|| format!("no node named {name} in {tree}"))?;
    let rest = tree.get(from..).unwrap_or_default();
    let open = rest.find(r#""rect":{"#).ok_or("no rect")? + r#""rect":{"#.len();
    let close = rest
        .get(open..)
        .and_then(|after| after.find('}'))
        .ok_or("no end")?
        + open;
    let mut lengths = [0.0f32; 4];
    for (field, slot) in rest
        .get(open..close)
        .unwrap_or_default()
        .split(',')
        .zip(lengths.iter_mut())
    {
        let (_, number) = field.split_once(':').ok_or("not a field")?;
        *slot = number.parse().map_err(|why| format!("{number}: {why}"))?;
    }
    let [height, width, x, y] = lengths;
    Ok(Rect::new(x, y, width, height))
}

/// The window at one: a page of 480 × 400 below the strip.
const AT_ONE: (u32, u32) = (480, 436);

/// The window at two, the same page in device pixels.
const AT_TWO: (u32, u32) = (960, 872);

#[test]
fn the_ring_is_the_named_boxs_own_rect_in_numbers_and_is_drawn_there() {
    let lines = [read(1), put(2, "Name", "Ada Lovelace")];
    let Session {
        answers,
        rings,
        showing,
    } = session("one", AT_ONE, Scale::ONE, &lines).unwrap_or_else(|why| panic!("{why}"));
    let tree = answers.first().map_or("", String::as_str);
    let rect = told(tree, "Name").unwrap_or_else(|why| panic!("{why}"));
    // Where the words end, not a whole pixel: what the ring rounds.
    assert!(rect.left().fract() > 0.0, "{rect:?}");
    assert!(
        answers
            .get(1)
            .is_some_and(|done| done.starts_with(r#"{"answer":"done","id":2,"#)),
        "{answers:#?}"
    );
    // One ring, of the field's own rectangle, each edge rounded to the
    // nearest device pixel.
    let expected = marked(&[rect], 1.0);
    assert_eq!(rings, vec![expected.clone()]);
    let [field] = expected.as_slice() else {
        panic!("not one rectangle: {expected:?}");
    };
    #[expect(clippy::cast_possible_truncation, reason = "a small rounded length")]
    let rounded = |length: f32| length.round() as i32;
    assert_eq!(
        (field.x, field.y),
        (rounded(rect.left()), rounded(rect.top()))
    );
    assert_eq!(
        (
            field.x + i32::try_from(field.width).unwrap_or(i32::MAX),
            field.y + i32::try_from(field.height).unwrap_or(i32::MAX)
        ),
        (rounded(rect.right()), rounded(rect.bottom()))
    );
    assert_eq!(showing.marked(), expected.as_slice());

    placed_and_drawn(&showing, *field).unwrap_or_else(|why| panic!("{why}"));
}

/// `field`, ringed in what `showing` composes at one: placed over the frame
/// in numbers, and drawn with the page's own pixels in the gap and on the
/// field.
fn placed_and_drawn(showing: &Showing, field: Marked) -> Result<(), String> {
    // Placed over the frame, below the band of 36: two device pixels
    // outside the field's edge, and two thick.
    let scene = showing.scene(AT_ONE, Scale::ONE);
    let placed = Placement::of(AT_ONE, Scale::ONE, scene.contents());
    assert_eq!(
        placed.frame,
        PixelRect {
            x: 0,
            y: STRIP_HEIGHT,
            width: 480,
            height: 400
        }
    );
    let pieces = ring(field, placed.frame, Scale::ONE);
    let (left, top) = (
        u32::try_from(field.x).unwrap_or(0),
        u32::try_from(field.y).unwrap_or(0) + STRIP_HEIGHT,
    );
    let (right, bottom) = (left + field.width, top + field.height);
    assert_eq!(
        pieces,
        vec![
            PixelRect {
                x: left - 4,
                y: top - 4,
                width: field.width + 8,
                height: 2
            },
            PixelRect {
                x: left - 4,
                y: bottom + 2,
                width: field.width + 8,
                height: 2
            },
            PixelRect {
                x: left - 4,
                y: top - 2,
                width: 2,
                height: field.height + 4
            },
            PixelRect {
                x: right + 2,
                y: top - 2,
                width: 2,
                height: field.height + 4
            },
        ]
    );

    let lettering = Lettering::compiled_in().ok_or("DejaVu Sans is not compiled in")?;
    let canvas = compose(&scene, &lettering);
    let frame = scene.frame.ok_or("no frame")?;
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    let page = |x, y| frame.at(x, y - STRIP_HEIGHT);
    let accent = Some(mark().to_rgba8());
    for (x, y) in [
        (left - 4, top - 4),
        (right + 3, top - 4),
        (left - 4, bottom + 3),
        (right + 3, bottom + 3),
        (left - 3, top - 3),
    ] {
        assert_eq!(at(x, y), accent, "the ring at {x},{y}");
    }
    // Past it, in the gap between it and the field, and on the field: the
    // page as it painted itself.
    for (x, y) in [
        (left - 5, top),
        (right + 4, bottom - 1),
        (left - 2, top - 2),
        (right + 1, bottom + 1),
        (left, top),
        (right - 1, bottom - 1),
        (left + 2, top + 2),
    ] {
        assert_eq!(at(x, y), page(x, y), "the page at {x},{y}");
    }
    Ok(())
}

#[test]
fn at_two_the_ring_is_the_same_rect_in_device_pixels() {
    let lines = [read(1), put(2, "Name", "Ada Lovelace")];
    let Session { answers, rings, .. } =
        session("two", AT_TWO, Scale::TWO, &lines).unwrap_or_else(|why| panic!("{why}"));
    let rect = told(answers.first().map_or("", String::as_str), "Name")
        .unwrap_or_else(|why| panic!("{why}"));
    let expected = marked(&[rect], 2.0);
    assert_eq!(rings, vec![expected.clone()]);
    let [field] = expected.as_slice() else {
        panic!("not one rectangle: {expected:?}");
    };
    // Twice the CSS height exactly: the field is laid out on whole pixels
    // down the page.
    assert!(
        (f64::from(rect.size.height) * 2.0 - f64::from(field.height)).abs() < 1e-6,
        "{rect:?} is not {field:?} at two"
    );
    let pieces = ring(
        *field,
        PixelRect {
            x: 0,
            y: 72,
            width: 960,
            height: 800,
        },
        Scale::TWO,
    );
    // Four device pixels out, and four thick.
    assert_eq!(
        pieces.first().map(|top| (top.y, top.height)),
        Some((72 + u32::try_from(field.y).unwrap_or(0) - 8, 4))
    );
}

#[test]
fn a_refused_verb_marks_nothing_and_takes_the_last_ring_away_with_its_frame() {
    let refused_alone = session(
        "refused",
        AT_ONE,
        Scale::ONE,
        &[put(1, "Nobody's name", "Grace")],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(
        refused_alone
            .answers
            .first()
            .is_some_and(|refused| refused.starts_with(r#"{"answer":"refused","id":1,"#)),
        "{:#?}",
        refused_alone.answers
    );
    assert_eq!(refused_alone.rings, Vec::<Vec<Marked>>::new());
    assert!(refused_alone.showing.marked().is_empty());

    let after = session(
        "after",
        AT_ONE,
        Scale::ONE,
        &[
            put(1, "Name", "Ada Lovelace"),
            put(2, "Nobody's name", "Grace"),
        ],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(after.answers.len(), 2, "{:#?}", after.answers);
    assert_eq!(after.rings.len(), 1, "{:#?}", after.rings);
    // The refused verb's tab was painted again, and no ring came with it.
    assert!(after.showing.marked().is_empty());
}

#[test]
fn only_the_box_named_is_ringed_never_its_neighbour() {
    let lines = [read(1), put(2, "Other", "Grace Hopper")];
    let Session { answers, rings, .. } =
        session("other", AT_ONE, Scale::ONE, &lines).unwrap_or_else(|why| panic!("{why}"));
    let tree = answers.first().map_or("", String::as_str);
    let other = told(tree, "Other").unwrap_or_else(|why| panic!("{why}"));
    let name = told(tree, "Name").unwrap_or_else(|why| panic!("{why}"));
    assert!(
        (other.left() - 20.0).abs() < 1e-6 && (other.top() - 200.0).abs() < 1e-6,
        "{other:?} is not where its style places it"
    );
    assert_eq!(rings, vec![marked(&[other], 1.0)]);
    assert_ne!(rings, vec![marked(&[name], 1.0)]);
}

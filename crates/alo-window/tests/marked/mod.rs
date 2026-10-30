/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The box an agent's verb acted on, marked over a page by Chromium's own
//! highlight and seen on the screen (queue item 680, ADR 0066): the check
//! itself, shared by the machine's Chrome
//! (`the_box_marked_through_chrome.rs`) and the Chromium alo carries
//! (`the_box_marked_through_alos_cef_host_as_chrome.rs`, queue item 726),
//! each alone in its file. The first file says what is seen and why.

use alo_paint::Canvas;
use alo_paint::encode::picture_from_png;
use alo_value::Rgba;
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::Chromium;
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::opening::CHROME_AT;
use alo_window::window_server;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Where the page says it was served from. Frozen: nothing reaches it.
const ADDRESS: &str = "https://example.com/marked";

/// The page, 600 by 640 CSS pixels: a swatch of alo's accent at its top
/// left, as wide as 40 and 20 more for each time the page has loaded
/// before; and four boxes, each 120 by 40 with a grey border six thick,
/// far enough apart that Chrome's label beside one — about 190 by 120,
/// above the box or below it, as Chrome chooses — reaches no other's.
/// "Again" goes to another document of its own eight seconds after it is
/// pressed; "Onward" is a link to the page's own address.
const PAGE: &str = "<!doctype html><title>Marked</title><style>\
    body { margin: 0; background: #fff; font: 16px sans-serif }\
    #swatch { position: absolute; left: 0; top: 0; width: 40px; height: 40px;\
      background: #e76f51 }\
    .box { position: absolute; box-sizing: border-box; width: 120px; height: 40px;\
      border: 6px solid #888; background: #fff; color: #fff; padding: 0;\
      display: block; font: 16px sans-serif }\
    #alpha { left: 80px; top: 80px } #beta { left: 360px; top: 80px }\
    #again { left: 80px; top: 420px } #onward { left: 360px; top: 420px }\
    </style><div id=swatch></div>\
    <button class=box id=alpha>Alpha</button>\
    <button class=box id=beta>Beta</button>\
    <button class=box id=again \
      onclick=\"setTimeout(() => location.reload(), 8000)\">Again</button>\
    <a class=box id=onward href=\"https://example.com/marked\">Onward</a>\
    <script>\
      const loads = Number(sessionStorage.getItem('loads') || 0);\
      sessionStorage.setItem('loads', String(loads + 1));\
      document.getElementById('swatch').style.width = (40 + 20 * loads) + 'px';\
    </script>";

/// The size the page is laid out at, in CSS pixels.
const SIZE: (u32, u32) = (600, 640);

/// How long anything expected is waited for before the test says so.
const AT_MOST: Duration = Duration::from_secs(20);

/// How long a mark is watched for after the page has gone to another
/// document — ten of Chrome's frames many times over.
const WATCHED: Duration = Duration::from_secs(2);

/// How far a captured pixel may be from another and still be the same
/// colour: the screen's own rounding, not another colour.
const SAME: u8 = 6;

/// A box of the page, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BoxAt {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

/// The boxes, where the page puts them.
const ALPHA: BoxAt = at(80.0, 80.0);
const BETA: BoxAt = at(360.0, 80.0);
const AGAIN: BoxAt = at(80.0, 420.0);
const ONWARD: BoxAt = at(360.0, 420.0);

const fn at(left: f64, top: f64) -> BoxAt {
    BoxAt {
        left,
        top,
        width: 120.0,
        height: 40.0,
    }
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket() -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-marked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `line` through `alo-ask`, a process of its own, and its answer.
fn ask(socket: &Path, line: &str) -> Result<String, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("alo-ask did not start: {why}"))?;
    let mut stdin = asking.stdin.take().ok_or("alo-ask has no input")?;
    stdin
        .write_all(line.as_bytes())
        .map_err(|why| format!("the line was not sent: {why}"))?;
    drop(stdin);
    let finished = asking
        .wait_with_output()
        .map_err(|why| format!("alo-ask did not finish: {why}"))?;
    if !finished.status.success() {
        return Err(format!(
            "alo-ask failed: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout).into_owned())
}

/// Activate the box named `name`, and the kind of answer it was: the
/// answer's `answer`, as JSON writes it.
fn activate(socket: &Path, id: u32, name: &str) -> Result<String, String> {
    let answer = ask(
        socket,
        &format!(
            r#"{{"v":1,"id":{id},"ask":"act","tab":"{ADDRESS}","target":{{"name":"{name}"}},"verb":"activate"}}"#
        ),
    )?;
    answer
        .split_once(r#""answer":""#)
        .and_then(|(_, after)| after.split_once('"'))
        .map(|(kind, _)| kind.to_owned())
        .ok_or_else(|| format!("no kind of answer: {answer}"))
}

/// Where the tree an agent reads, as JSON writes it, says the button or
/// link named `name` is: its `rect`, which JSON writes after its name and
/// id, its four lengths in the order of their names, and then its role.
fn read_box(tree: &str, name: &str) -> Option<BoxAt> {
    let named = format!(r#""name":"{name}","node":"#);
    tree.match_indices(&named).find_map(|(at, _)| {
        let (_, rect) = tree.get(at..)?.split_once(r#""rect":{"#)?;
        let (lengths, after) = rect.split_once('}')?;
        if !(after.starts_with(r#","role":"button""#) || after.starts_with(r#","role":"link""#)) {
            return None;
        }
        let mut read = lengths.split(',').map(|field| {
            let (_, number) = field.split_once(':')?;
            number.parse::<f64>().ok()
        });
        let (height, width, left, top) =
            (read.next()??, read.next()??, read.next()??, read.next()??);
        Some(BoxAt {
            left,
            top,
            width,
            height,
        })
    })
}

/// Chrome's window, as the window server numbers it, once it is on the
/// screen at the bounds Chrome gave it.
fn window_of(serving: &ChromeServing, bounds: Bounds) -> Result<u64, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let listed = window_server::on_the_screen(serving.process())
            .map_err(|why| format!("the window server was not read: {why}"))?;
        if let Some(window) = listed
            .iter()
            .find(|window| window.layer == 0 && window.bounds == Some(bounds))
        {
            return Ok(window.number);
        }
        if Instant::now() > until {
            return Err(format!(
                "no window of Chrome's on the screen at {bounds:?}: {listed:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// What the window server shows of Chrome's window and the page in it.
struct Screen {
    /// The window, by the window server's number.
    window: u64,
    /// Where the page's top left corner is in a capture, in its pixels.
    page_left: f64,
    page_top: f64,
    /// Device pixels to a CSS pixel.
    scale: f64,
    /// The page's size, in CSS pixels.
    page: (f64, f64),
    /// Where captures are written.
    file: PathBuf,
}

impl Screen {
    /// The window as it is now, in its own device pixels.
    fn capture(&self) -> Result<Canvas, String> {
        let window = self.window.to_string();
        let file = self.file.to_string_lossy().into_owned();
        let done = Command::new("screencapture")
            .args(["-x", "-o", "-l", &window, &file])
            .status()
            .map_err(|why| format!("screencapture did not start: {why}"))?;
        if !done.success() {
            return Err(format!("screencapture failed: {done}"));
        }
        let bytes = std::fs::read(&self.file).map_err(|why| why.to_string())?;
        picture_from_png(&bytes).map_err(|why| format!("an unreadable capture: {why:?}"))
    }

    /// The pixel at the page's CSS point `x`, `y`, in `canvas` — none
    /// outside the page, where the title bar is.
    fn at(&self, canvas: &Canvas, x: f64, y: f64) -> Option<Rgba> {
        if x < 0.0 || y < 0.0 || x >= self.page.0 || y >= self.page.1 {
            return None;
        }
        let device = |page: f64, css: f64| {
            let point = (page + css * self.scale).floor();
            (point >= 0.0 && point < f64::from(u32::MAX)).then_some(point)
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "floored and checked to be within a u32"
        )]
        let (x, y) = (
            device(self.page_left, x)? as u32,
            device(self.page_top, y)? as u32,
        );
        canvas.at(x, y)
    }
}

fn same(one: Rgba, other: Rgba) -> bool {
    let (a, b) = (one.to_rgba8(), other.to_rgba8());
    a.0.abs_diff(b.0) <= SAME && a.1.abs_diff(b.1) <= SAME && a.2.abs_diff(b.2) <= SAME
}

/// The middle of each side of a box's border, six CSS pixels thick: the
/// top, the bottom, the left and the right.
fn border_of(at: BoxAt) -> [(f64, f64); 4] {
    let (middle_x, middle_y) = (at.left + at.width / 2.0, at.top + at.height / 2.0);
    [
        (middle_x, at.top + 3.0),
        (middle_x, at.top + at.height - 3.0),
        (at.left + 3.0, middle_y),
        (at.left + at.width - 3.0, middle_y),
    ]
}

/// What a capture shows of the page.
struct Seen {
    /// How wide the swatch is, in CSS pixels: which load of the page.
    swatch: u32,
    /// The colour of each side of each box's border, in the order the boxes
    /// are given.
    borders: Vec<[Option<Rgba>; 4]>,
    /// How many dark pixels there are where Chrome puts a box's label —
    /// the 130 CSS pixels above it and below it, 200 wide from its left —
    /// where the page draws nothing dark.
    labels: Vec<usize>,
}

/// The fewest dark pixels, sampled a CSS pixel apart, that are a label's
/// letters: a label sampled so has several hundred, and the page has none.
const LETTERS: usize = 50;

/// How far above and below a box its label is looked for, in CSS pixels.
const LABELLED: f64 = 130.0;

/// The boxes looked at, in this order.
const BOXES: [BoxAt; 4] = [ALPHA, BETA, AGAIN, ONWARD];

impl Screen {
    fn see(&self, canvas: &Canvas, accent: Rgba) -> Seen {
        let mut swatch = 0;
        while self
            .at(canvas, f64::from(swatch) + 0.5, 20.0)
            .is_some_and(|pixel| same(pixel, accent))
        {
            swatch += 1;
        }
        let borders = BOXES
            .iter()
            .map(|at| border_of(*at).map(|(x, y)| self.at(canvas, x, y)))
            .collect();
        let labels = BOXES
            .iter()
            .map(|at| {
                let mut dark = 0;
                let mut y = at.top - LABELLED;
                while y < at.top + at.height + LABELLED {
                    if y > at.top - 2.0 && y < at.top + at.height + 2.0 {
                        y += 1.0;
                        continue;
                    }
                    let mut x = at.left;
                    while x < at.left + 200.0 {
                        if self.at(canvas, x, y).is_some_and(|pixel| {
                            let (red, green, blue, _) = pixel.to_rgba8();
                            u32::from(red) + u32::from(green) + u32::from(blue) < 300
                        }) {
                            dark += 1;
                        }
                        x += 1.0;
                    }
                    y += 1.0;
                }
                dark
            })
            .collect();
        Seen {
            swatch,
            borders,
            labels,
        }
    }
}

/// Whether box `index` is marked in `seen`: every side of its border the
/// accent, and its label beside it.
fn marked(seen: &Seen, index: usize, accent: Rgba) -> bool {
    seen.borders.get(index).is_some_and(|sides| {
        sides
            .iter()
            .all(|side| side.is_some_and(|pixel| same(pixel, accent)))
    }) && seen.labels.get(index).is_some_and(|dark| *dark > LETTERS)
}

/// Whether box `index` is unmarked in `seen`: every side of its border the
/// page's own grey, and no label beside it.
fn unmarked(seen: &Seen, index: usize, grey: Rgba) -> bool {
    seen.borders.get(index).is_some_and(|sides| {
        sides
            .iter()
            .all(|side| side.is_some_and(|pixel| same(pixel, grey)))
    }) && seen.labels.get(index) == Some(&0)
}

/// Capture until `wanted` holds of what is seen, and say what was seen last
/// if it never does.
fn until(
    screen: &Screen,
    accent: Rgba,
    what: &str,
    wanted: impl Fn(&Seen) -> bool,
) -> Result<Seen, String> {
    let began = Instant::now();
    let until = began + AT_MOST;
    // Each look, briefly: when, the swatch, which boxes' borders were the
    // accent, and each box's label.
    let mut looks = Vec::new();
    loop {
        let seen = screen.see(&screen.capture()?, accent);
        if wanted(&seen) {
            return Ok(seen);
        }
        let accented: Vec<bool> = seen
            .borders
            .iter()
            .map(|sides| {
                sides
                    .iter()
                    .all(|side| side.is_some_and(|pixel| same(pixel, accent)))
            })
            .collect();
        looks.push(format!(
            "{} ms: swatch {}, accent {accented:?}, labels {:?}",
            began.elapsed().as_millis(),
            seen.swatch,
            seen.labels
        ));
        if Instant::now() > until {
            return Err(format!(
                "{what}: never seen; last, borders {:?}; each look: {looks:#?}",
                seen.borders
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A shown `chromium`, placed as `alo --chrome` places it, holding the
/// frozen page, and each verb of an agent in a process of its own seen to
/// mark — or not mark — its box on the screen, by Chromium's own highlight.
///
/// # Errors
///
/// What was not seen, or could not be looked at.
pub fn on_the_screen(chromium: Chromium) -> Result<(), String> {
    let serving = ChromeServing::start_shown(
        chromium,
        Opening::Frozen {
            url: ADDRESS.to_owned(),
            page: PAGE.as_bytes().to_vec(),
        },
        SIZE,
        CHROME_AT,
        AgentSites::Every,
        Box::new(|_| true),
    )
    .map_err(|why| format!("no shown Chrome: {why}"))?;
    let placed = *serving.placed().ok_or("a shown Chrome is placed")?;
    let bounds = placed.window.bounds;
    let path = socket()?;
    let listening =
        Listening::at(&path, serving.orders()).map_err(|why| format!("not listening: {why}"))?;
    let checked = check(&serving, bounds, placed.page, &path);
    listening.stop();
    serving
        .records()
        .ok_or("the tab in Chrome did not finish")?;
    checked
}

fn check(
    serving: &ChromeServing,
    bounds: Bounds,
    page: (f64, f64),
    socket: &Path,
) -> Result<(), String> {
    let window = window_of(serving, bounds)?;
    let file = std::env::temp_dir().join(format!("alo-marked-{}.png", std::process::id()));
    let mut screen = Screen {
        window,
        page_left: 0.0,
        page_top: 0.0,
        scale: 1.0,
        page,
        file,
    };
    let first = screen.capture()?;
    // The capture is the window in its device pixels; the page fills it
    // below the title bar.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a window's width in points is far below 2^52"
    )]
    let scale = f64::from(first.width()) / bounds.width as f64;
    screen.scale = scale;
    screen.page_top = f64::from(first.height()) - page.1 * scale;
    let accent = screen.at(&first, 20.0, 20.0).ok_or("no swatch")?;
    let grey = screen
        .at(&first, ALPHA.left + 3.0, ALPHA.top + 20.0)
        .ok_or("no box")?;
    // The page is where it was taken to be: the swatch's corners are the
    // accent, and white is beside it.
    for (x, y) in [(0.5, 0.5), (39.5, 39.5), (0.5, 39.5)] {
        let pixel = screen.at(&first, x, y).ok_or("no swatch")?;
        assert!(same(pixel, accent), "the swatch at {x},{y}: {pixel:?}");
    }
    let white = screen.at(&first, 41.0, 20.0).ok_or("no page")?;
    assert!(
        same(white, Rgba::from_rgba8(255, 255, 255, 255)),
        "{white:?}"
    );
    let (red, green, blue, _) = accent.to_rgba8();
    assert!(
        red > green && green > blue,
        "an accent, terracotta: {accent:?}"
    );

    // What the agent reads is where the page put each box: Chrome's word
    // for the node's box, the one it marks.
    let answer = ask(
        socket,
        &format!(r#"{{"v":1,"id":1,"ask":"read","tab":"{ADDRESS}"}}"#),
    )?;
    let tree = answer.as_str();
    for (name, at) in [
        ("Alpha", ALPHA),
        ("Beta", BETA),
        ("Again", AGAIN),
        ("Onward", ONWARD),
    ] {
        assert_eq!(read_box(tree, name), Some(at), "{name}");
    }

    let seen = until(&screen, accent, "nothing marked at first", |seen| {
        seen.swatch == 40 && (0..4).all(|index| unmarked(seen, index, grey))
    })?;
    assert_eq!(seen.swatch, 40);

    // A verb that ran marks its box.
    assert_eq!(activate(socket, 2, "Alpha")?, "done");
    until(&screen, accent, "Alpha marked and nothing else", |seen| {
        marked(seen, 0, accent) && (1..4).all(|index| unmarked(seen, index, grey))
    })?;

    // The next verb's mark replaces it.
    assert_eq!(activate(socket, 3, "Beta")?, "done");
    until(&screen, accent, "Beta marked, Alpha no longer", |seen| {
        marked(seen, 1, accent) && [0, 2, 3].iter().all(|index| unmarked(seen, *index, grey))
    })?;

    // A refused verb leaves no mark.
    assert_eq!(activate(socket, 4, "Nothing here")?, "refused");
    until(&screen, accent, "no mark after a refusal", |seen| {
        (0..4).all(|index| unmarked(seen, index, grey))
    })?;

    // A page that goes to another document on its own, after a verb marked
    // a box in it, is left with no mark.
    assert_eq!(activate(socket, 5, "Again")?, "done");
    until(&screen, accent, "Again marked", |seen| {
        seen.swatch == 40 && marked(seen, 2, accent)
    })?;
    until(&screen, accent, "the page loaded again", |seen| {
        seen.swatch == 60
    })?;
    let watched = Instant::now() + WATCHED;
    while Instant::now() < watched {
        let seen = screen.see(&screen.capture()?, accent);
        assert!(
            (0..4).all(|index| unmarked(&seen, index, grey)),
            "a mark left on the page loaded again: borders {:?}, labels {:?}",
            seen.borders,
            seen.labels
        );
    }

    // A link followed in place leaves no mark on the page it went to.
    assert_eq!(activate(socket, 6, "Onward")?, "done");
    until(&screen, accent, "the page the link went to", |seen| {
        seen.swatch == 80
    })?;
    let watched = Instant::now() + WATCHED;
    while Instant::now() < watched {
        let seen = screen.see(&screen.capture()?, accent);
        assert!(
            (0..4).all(|index| unmarked(&seen, index, grey)),
            "a mark on the page a link went to: borders {:?}, labels {:?}",
            seen.borders,
            seen.labels
        );
    }
    let _ = std::fs::remove_file(&screen.file);
    Ok(())
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 873 (ADR 0105): a window put on screen from pixels kept on
//! the GPU, against the same window copied whole by `softbuffer`, read back
//! from the screen in the same run.
//!
//! `alo-present` shows a frame of known colours — sixteen flat stripes over
//! the top half, among them alo's `#e62828`, greys and primaries, and four
//! thousand cells of other colours below (`present_shown::pattern`) — once
//! each way, in a window of its own at one place, and the window server's
//! picture of each is captured by its number with `screencapture -l`, in
//! device pixels.
//!
//! - **The copied way presents** — the way a machine with no GPU for the
//!   window falls back to: each stripe reads as its colour does on this
//!   screen, red as red, white as white, black as black.
//! - **The kept way presents the same frame**: every flat stripe pixel for
//!   pixel as the copied one, and every pixel of the cells within one level
//!   a channel — the window server matching the Metal layer's colours to
//!   the display where Core Animation matched `softbuffer`'s image, which
//!   rounds one colour in seven a level the other way (ADR 0105, measured
//!   nearer Chrome's).

use alo_paint::Canvas;
use alo_paint::encode::picture_from_png;
use alo_window::present_shown::STRIPES;
use alo_window::window_server;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

/// How long a window has to say it has shown its frame.
const AT_MOST: Duration = Duration::from_secs(30);

/// What one way showed: the line it said, and the window's content as the
/// screen showed it.
struct Seen {
    said: String,
    content: Content,
}

/// A window's content, read off the screen: eight-bit pixels, row by row.
#[derive(PartialEq)]
struct Content {
    width: u32,
    height: u32,
    pixels: Vec<(u8, u8, u8)>,
    /// Whether each pixel is the window's own, not its corner's.
    opaque: Vec<bool>,
}

impl Content {
    fn opaque(&self, x: u32, y: u32) -> bool {
        self.opaque
            .get((y * self.width + x) as usize)
            .copied()
            .unwrap_or(false)
    }
}

/// Show the pattern `how`, capture it, and close the window.
fn seen(how: &str) -> Result<Seen, String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_alo-present"))
        .args([how, "400", "300", "160", "160"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|why| format!("alo-present did not start: {why}"))?;
    let seen = capture(&mut child);
    drop(child.stdin.take());
    let ended = child.wait().map_err(|why| why.to_string())?;
    let seen = seen?;
    if !ended.success() {
        return Err(format!("alo-present {how} ended {ended}"));
    }
    Ok(seen)
}

/// The line `child` says once shown, and its window's content once two
/// captures in a row agree.
fn capture(child: &mut Child) -> Result<Seen, String> {
    let out = child.stdout.take().ok_or("no standard output")?;
    let (tell, heard) = channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(out).read_line(&mut line);
        let _ = tell.send(line);
    });
    let said = heard
        .recv_timeout(AT_MOST)
        .map_err(|_| "alo-present said nothing".to_owned())?;
    let size = said
        .split_whitespace()
        .nth(3)
        .and_then(|size| size.split_once('x'))
        .and_then(|(width, height)| Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?)))
        .ok_or_else(|| format!("not a size: {said:?}"))?;
    let until = Instant::now() + AT_MOST;
    let mut last: Option<Content> = None;
    loop {
        std::thread::sleep(Duration::from_millis(400));
        let number = window_server::on_the_screen(child.id())
            .map_err(|why| format!("the window server's list: {why:?}"))?
            .into_iter()
            .find(|listed| listed.layer == 0 && listed.alpha > 0)
            .map(|listed| listed.number);
        if let Some(number) = number {
            let content = content_of(&picture_of(number)?, size)?;
            if last.as_ref() == Some(&content) {
                return Ok(Seen {
                    said: said.trim().to_owned(),
                    content,
                });
            }
            last = Some(content);
        }
        if Instant::now() > until {
            return Err("the window's picture never held still".to_owned());
        }
    }
}

/// A picture of the window numbered `window`, title bar and all.
fn picture_of(window: u64) -> Result<Canvas, String> {
    let taken = std::env::temp_dir().join(format!("alo-873-present-{}.png", std::process::id()));
    let done = Command::new("screencapture")
        .args(["-x", "-o", "-l", &window.to_string()])
        .arg(&taken)
        .status()
        .map_err(|why| format!("screencapture did not start: {why}"))?;
    if !done.success() {
        return Err(format!("screencapture failed: {done}"));
    }
    let bytes = std::fs::read(&taken).map_err(|why| why.to_string())?;
    let _ = std::fs::remove_file(&taken);
    picture_from_png(&bytes).map_err(|why| format!("an unreadable capture: {why:?}"))
}

/// The bottom `size` of `picture`: the window's content, below its title
/// bar.
fn content_of(picture: &Canvas, (width, height): (u32, u32)) -> Result<Content, String> {
    if picture.width() != width || picture.height() < height {
        return Err(format!(
            "a capture of {} x {} for content of {width} x {height}",
            picture.width(),
            picture.height()
        ));
    }
    let top = picture.height() - height;
    let at = usize::try_from(top * width).map_err(|why| why.to_string())?;
    let shown = picture.pixels().get(at..).ok_or("no content")?;
    let opaque = shown
        .iter()
        .map(|each| each.to_rgba8().3 == u8::MAX)
        .collect();
    let pixels = shown
        .iter()
        .map(|each| {
            let (red, green, blue, _) = each.to_rgba8();
            (red, green, blue)
        })
        .collect();
    Ok(Content {
        width,
        height,
        pixels,
        opaque,
    })
}

/// The pixel at `x`, `y`.
fn pixel(content: &Content, x: u32, y: u32) -> (u8, u8, u8) {
    content
        .pixels
        .get((y * content.width + x) as usize)
        .copied()
        .unwrap_or_default()
}

#[test]
fn the_window_kept_on_the_gpu_shows_what_softbuffer_shows() {
    let copied = seen("copied").unwrap_or_else(|why| panic!("{why}"));
    let kept = seen("kept").unwrap_or_else(|why| panic!("{why}"));
    let (width, height) = (copied.content.width, copied.content.height);
    assert!(
        copied.said.starts_with("shown copied "),
        "{:?}",
        copied.said
    );
    assert_eq!(
        kept.said,
        format!(
            "shown kept {} {width}x{height}",
            u64::from(width) * u64::from(height) * 4
        ),
        "kept on this Mac's GPU, the first frame written whole"
    );
    assert_eq!((kept.content.width, kept.content.height), (width, height));

    // The copied way presents: each stripe as its colour, at its middle.
    let middle = |stripe: u32| (stripe * width / 16 + width / 32, height / 4);
    let reads = |stripe: u32| {
        let (x, y) = middle(stripe);
        pixel(&copied.content, x, y)
    };
    let (red, green, blue) = reads(1);
    assert!(red > 200 && green < 90 && blue < 90, "red: {:?}", reads(1));
    let (red, green, blue) = reads(2);
    assert!(
        red < 160 && green > 200 && blue < 120,
        "green: {:?}",
        reads(2)
    );
    assert_eq!(reads(7), (255, 255, 255), "white");
    assert_eq!(reads(8), (0, 0, 0), "black");
    assert_eq!(STRIPES.len(), 16);

    // The kept way: the stripes pixel for pixel, the cells within a level.
    let (mut stripes_differing, mut cells_differing, mut furthest) = (0, 0, 0);
    for y in 0..height {
        for x in 0..width {
            let (one, other) = (pixel(&copied.content, x, y), pixel(&kept.content, x, y));
            if !kept.content.opaque(x, y) || !copied.content.opaque(x, y) {
                // The window's rounded corners, where the screen behind
                // shows through.
                continue;
            }
            let apart = one
                .0
                .abs_diff(other.0)
                .max(one.1.abs_diff(other.1))
                .max(one.2.abs_diff(other.2));
            if apart > 0 && y < height / 2 {
                stripes_differing += 1;
            } else if apart > 0 {
                cells_differing += 1;
            }
            furthest = furthest.max(apart);
        }
    }
    assert_eq!(stripes_differing, 0, "every flat colour as softbuffer's");
    assert!(furthest <= 1, "a level at most, apart by {furthest}");
    let cells = u64::from(width) * u64::from(height - height / 2);
    assert!(
        cells_differing * 4 < cells,
        "{cells_differing} of {cells} cell pixels a level apart"
    );
}

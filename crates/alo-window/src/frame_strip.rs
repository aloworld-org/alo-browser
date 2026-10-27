/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The page's tab in the frame around Chrome's window, drawn by a tab
//! strip's renderer of its own (queue item 681, ADR 0024 § 4).
//!
//! This engine's window shows its tabs in a strip drawn by a sandboxed
//! renderer, because a title is a stranger's string and the browser process
//! never shapes one. The frame around Chrome's window shows its one page's
//! tab the same way: from what Chrome calls the page
//! ([`crate::chrome_named`]), the frame makes a [`Strip`] of one selected
//! tab ([`strip_of`]) and hands it to a thread of its own here, which asks
//! the strip's renderer to draw and paint it — `alo-renderer`'s own
//! template and stylesheet, the title a text node — and tells the frame the
//! frame it painted, or why it painted none.
//!
//! # Never waited on
//!
//! ADR 0024 § 2: the event loop never calls a renderer. So the frame only
//! sends a strip here, and hears the painting later as a [`ToFrame`]; a
//! renderer that stops answering leaves the frame its last tab and, at
//! `LONGEST_SILENCE`, its sentence. Strips sent faster than they are
//! painted are not queued: the thread paints the newest it was sent.
//!
//! # As wide as one tab
//!
//! The frame's band holds the tab, which renderer draws the page and what
//! agents did, side by side ([`crate::frame_place`]). So the strip is laid
//! out [`FRAME_STRIP_WIDTH`] wide — exactly one tab of the stylesheet's —
//! and [`STRIP_HEIGHT`] tall, at the frame's scale, as the engine's strip
//! is laid out at its window's.

use crate::chrome_protocol::Named;
use crate::chrome_serving::TellFrame;
use crate::conductor::strip_unexpected;
use crate::frame_told::ToFrame;
use crate::place::STRIP_HEIGHT;
use alo_layout::Size;
use alo_renderer::FromRenderer;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{Strip, StripTab};
use alo_renderer::tab::Tabs;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// How wide the frame's strip is laid out, in CSS pixels: one tab of
/// `alo-renderer`'s `interface/strip.css` — its 200 pixels of basis — and
/// the strip's eight pixels of padding either side of it.
pub const FRAME_STRIP_WIDTH: u32 = 216;

/// The strip the frame shows for a page Chrome calls `named`, on a screen
/// of `scale`: one tab, selected, showing its page — titled as Chrome
/// titles it, or by its address when its title is nothing.
pub fn strip_of(named: &Named, scale: f32) -> Strip {
    #[expect(clippy::cast_precision_loss, reason = "216 and 36 are exact in an f32")]
    let viewport = Size::new(FRAME_STRIP_WIDTH as f32, STRIP_HEIGHT as f32);
    Strip {
        tabs: vec![StripTab::new(named.title.as_str(), named.address.as_str()).selected()],
        viewport,
        scale: 1.0,
    }
    .at_scale(scale)
}

/// A thread holding the frame's strip's renderer.
#[derive(Debug)]
pub struct FrameStrip {
    /// Where strips are sent to be drawn; [`None`] once finished.
    strips: Option<Sender<Strip>>,
    /// The strip last sent, so the same one is not drawn twice.
    sent: Option<Strip>,
    thread: Option<JoinHandle<()>>,
}

impl FrameStrip {
    /// Start a thread that draws each strip it is sent in a renderer of
    /// `renderers`' at the strip's own site, and tells `frame` what came of
    /// it.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(renderers: Renderers, frame: TellFrame) -> std::io::Result<Self> {
        let (strips, asked) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-frame-strip".to_owned())
            .spawn(move || hold(Tabs::over(renderers), &asked, &frame))?;
        Ok(Self {
            strips: Some(strips),
            sent: None,
            thread: Some(thread),
        })
    }

    /// Have `strip` drawn, unless it is the strip last sent. Returns at
    /// once: what was painted is told to the frame.
    pub fn draw(&mut self, strip: Strip) {
        if self.sent.as_ref() == Some(&strip) {
            return;
        }
        if let Some(strips) = &self.strips {
            // A thread that has gone is a frame that is closing, which
            // nothing here waits for.
            let _ = strips.send(strip.clone());
        }
        self.sent = Some(strip);
    }

    /// Stop the thread and wait for it, its renderer stopped with it:
    /// whether it ended rather than panicked, which the lints make
    /// unreachable and is said rather than assumed.
    ///
    /// Called once the frame's event loop has ended, never in it — a
    /// renderer in the middle of painting is waited for.
    pub fn finish(mut self) -> bool {
        self.strips = None;
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

/// Draw each strip `asked` sends, the newest of those waiting, telling
/// `frame` what came of each, until nobody sends any more or the frame has
/// gone. Its renderer is stopped as `tabs` goes.
fn hold(mut tabs: Tabs, asked: &Receiver<Strip>, frame: &TellFrame) {
    while let Ok(first) = asked.recv() {
        let newest = asked.try_iter().last().unwrap_or(first);
        if !frame(drawn(&mut tabs, &newest)) {
            return;
        }
    }
}

/// What came of drawing `strip` in the strip's renderer of `tabs`, as the
/// frame is told it — in the words this engine's window says of its own
/// strip.
fn drawn(tabs: &mut Tabs, strip: &Strip) -> ToFrame {
    match tabs.draw_strip(strip) {
        Ok(FromRenderer::Painted(frame)) => ToFrame::Strip(frame),
        Ok(FromRenderer::Failed(failure)) => {
            ToFrame::StripSaid(format!("the tab strip could not be drawn: {failure}"))
        }
        Ok(other) => ToFrame::StripSaid(strip_unexpected(&other)),
        Err(lost) => ToFrame::StripSaid(
            tabs.strip_renderer()
                .what_happened()
                .unwrap_or_else(|| lost.to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::strip::Condition;

    fn named(title: &str) -> Named {
        Named {
            title: title.to_owned(),
            address: "https://example.com/a".to_owned(),
        }
    }

    #[test]
    fn the_frames_strip_is_one_selected_tab_as_wide_as_one_tab() {
        let strip = strip_of(&named("Invoices <b>"), 2.0);
        assert_eq!(strip.viewport, Size::new(216.0, 36.0));
        assert!((strip.scale - 2.0).abs() < f32::EPSILON, "{}", strip.scale);
        let [tab] = strip.tabs.as_slice() else {
            panic!("one tab: {strip:?}");
        };
        assert!(tab.selected);
        assert_eq!(tab.condition, Condition::Shown);
        assert_eq!(tab.label(), "Invoices <b>", "kept as text");
        assert_eq!(tab.address, "https://example.com/a");
    }

    #[test]
    fn a_page_with_no_title_is_shown_by_its_address() {
        let strip = strip_of(&named(" "), 1.0);
        assert_eq!(
            strip.tabs.first().map(StripTab::label),
            Some("https://example.com/a")
        );
    }

    #[test]
    fn a_scale_that_is_no_scale_is_one() {
        let strip = strip_of(&named("Invoices"), f32::NAN);
        assert!((strip.scale - 1.0).abs() < f32::EPSILON, "{}", strip.scale);
    }
}

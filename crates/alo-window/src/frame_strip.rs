/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The page's title and the application's name in the frame around
//! Chromium's window, drawn by a tab strip's renderer of its own (queue
//! items 681 and 991, ADR 0024 § 4).
//!
//! This engine's window shows its tabs in a strip drawn by a sandboxed
//! renderer, because a title is a stranger's string and the browser process
//! never shapes one. The frame around Chromium's window shows its one
//! page's title the same way: from what Chromium calls the page
//! ([`crate::chrome_named`]), the frame makes a [`Band`] ([`band_of`]) —
//! the title and where the page is, and how wide the band is laid out —
//! and hands it to a thread of its own here, which asks the strip's
//! renderer to draw and paint it as alo OS's window band — `alo-renderer`'s
//! own template and stylesheet, the title a text node, the application's
//! name after it — and tells the frame the frame it painted, or why it
//! painted none.
//!
//! # In the interface's faces
//!
//! The band is set in Manrope, the face the owner chose for alo's
//! interface, so the strip's renderer here is handed the interface's
//! typefaces beside whatever fonts it was given, as the conversation's
//! renderers are (ADR 0084 § 3 and its addendum of 14 November 2026). It
//! draws nothing but the band: no page is drawn in them.
//!
//! # Never waited on
//!
//! ADR 0024 § 2: the event loop never calls a renderer. So the frame only
//! sends a band here, and hears the painting later as a [`ToFrame`]; a
//! renderer that stops answering leaves the frame its last band and, at
//! `LONGEST_SILENCE`, its sentence. Bands sent faster than they are painted
//! are not queued: the thread paints the newest it was sent.
//!
//! # As wide as the frame gives it
//!
//! The frame's band holds the title, what agents did and which renderer
//! draws the page, side by side ([`crate::frame_place`]). So the band is
//! laid out as many CSS pixels wide as the frame leaves it
//! ([`crate::frame_place::FramePlacement::band_width`]) and
//! [`BAND_HEIGHT`] tall, at the frame's scale — and drawn again whenever
//! that changes, so the title is cut where the frame's room ends.

use crate::chrome_protocol::Named;
use crate::chrome_serving::TellFrame;
use crate::conductor::strip_unexpected;
use crate::frame_place::BAND_HEIGHT;
use crate::frame_told::ToFrame;
use alo_layout::Size;
use alo_renderer::FromRenderer;
use alo_renderer::band::Band;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// The band the frame shows for a page Chromium calls `named`, `width` CSS
/// pixels wide, on a screen of `scale`: titled as Chromium titles it, or by
/// its address when its title is nothing.
pub fn band_of(named: &Named, width: u32, scale: f32) -> Band {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a frame's width in CSS pixels, and 54, are exact in an f32"
    )]
    let viewport = Size::new(width as f32, BAND_HEIGHT as f32);
    Band::new(named.title.as_str(), named.address.as_str(), viewport).at_scale(scale)
}

/// A thread holding the frame's strip's renderer.
#[derive(Debug)]
pub struct FrameStrip {
    /// Where bands are sent to be drawn; [`None`] once finished.
    bands: Option<Sender<Band>>,
    /// The band last sent, so the same one is not drawn twice.
    sent: Option<Band>,
    thread: Option<JoinHandle<()>>,
}

impl FrameStrip {
    /// Start a thread that draws each band it is sent in a renderer of
    /// `renderers`' at the strip's own site — handed the interface's
    /// typefaces beside their fonts — and tells `frame` what came of it.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(renderers: Renderers, frame: TellFrame) -> std::io::Result<Self> {
        let faces = crate::fonts::for_the_interface(renderers.fonts());
        let renderers = renderers.with_fonts(faces);
        let (bands, asked) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-frame-strip".to_owned())
            .spawn(move || hold(Tabs::over(renderers), &asked, &frame))?;
        Ok(Self {
            bands: Some(bands),
            sent: None,
            thread: Some(thread),
        })
    }

    /// Have `band` drawn, unless it is the band last sent. Returns at
    /// once: what was painted is told to the frame.
    pub fn draw(&mut self, band: Band) {
        if self.sent.as_ref() == Some(&band) {
            return;
        }
        if let Some(bands) = &self.bands {
            // A thread that has gone is a frame that is closing, which
            // nothing here waits for.
            let _ = bands.send(band.clone());
        }
        self.sent = Some(band);
    }

    /// Stop the thread and wait for it, its renderer stopped with it:
    /// whether it ended rather than panicked, which the lints make
    /// unreachable and is said rather than assumed.
    ///
    /// Called once the frame's event loop has ended, never in it — a
    /// renderer in the middle of painting is waited for.
    pub fn finish(mut self) -> bool {
        self.bands = None;
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

/// Draw each band `asked` sends, the newest of those waiting, telling
/// `frame` what came of each, until nobody sends any more or the frame has
/// gone. Its renderer is stopped as `tabs` goes.
fn hold(mut tabs: Tabs, asked: &Receiver<Band>, frame: &TellFrame) {
    while let Ok(first) = asked.recv() {
        let newest = asked.try_iter().last().unwrap_or(first);
        if !frame(drawn(&mut tabs, &newest)) {
            return;
        }
    }
}

/// What came of drawing `band` in the strip's renderer of `tabs`, as the
/// frame is told it — in the words this engine's window says of its own
/// strip.
fn drawn(tabs: &mut Tabs, band: &Band) -> ToFrame {
    match tabs.draw_band(band) {
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

    fn named(title: &str) -> Named {
        Named {
            title: title.to_owned(),
            address: "https://example.com/a".to_owned(),
        }
    }

    #[test]
    fn the_frames_band_is_as_wide_as_it_is_given_and_54_tall() {
        let band = band_of(&named("Invoices <b>"), 920, 2.0);
        assert_eq!(band.viewport, Size::new(920.0, 54.0));
        assert!((band.scale - 2.0).abs() < f32::EPSILON, "{}", band.scale);
        assert_eq!(band.label(), "Invoices <b>", "kept as text");
        assert_eq!(band.address, "https://example.com/a");
    }

    #[test]
    fn a_page_with_no_title_is_shown_by_its_address() {
        assert_eq!(
            band_of(&named(" "), 600, 1.0).label(),
            "https://example.com/a"
        );
    }

    #[test]
    fn a_scale_that_is_no_scale_is_one() {
        let band = band_of(&named("Invoices"), 600, f32::NAN);
        assert!((band.scale - 1.0).abs() < f32::EPSILON, "{}", band.scale);
    }
}

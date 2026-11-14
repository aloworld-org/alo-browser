/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card's page drawn with no window at all — CEF's windowless
//! (off-screen) rendering — **a spike behind a switch** (queue item 959,
//! ADR 0113, proposed): nothing alo starts the host with turns it on, so
//! nothing changes for a person.
//!
//! Today a card's page is a shown window of Chromium's, parked off the
//! screen and streamed (ADR 0076 B4 as amended); Wayland lets no client
//! park or place a window (ADR 0112), so a card there must be drawn with
//! none. Told [`SWITCH`]`=WIDTHxHEIGHT@SCALE` beside `--app=`, the host
//! opens the page as a windowless browser instead (`window.rs`): CEF asks
//! the host the view's size ([`Viewport`], in CSS pixels) and the screen's
//! scale, and hands it each frame Chromium paints (`OnPaint`). Nothing in
//! the host is ever placed on a display.
//!
//! # What the spike hands alo, and what it does not
//!
//! The pixels `OnPaint` gives are behind a raw pointer, and reading one is
//! `unsafe` in Rust — a new boundary, which ADR 0080 does not allow and
//! only the owner can (ADR 0113 asks). So the spike reads **no pixel**:
//! each paint is said on the host's error output, one line ([`painted`]),
//! so its cadence and size can be measured, and alo has the frames as it
//! has them today — `Page.startScreencast` over the DevTools pipe, which
//! works on a windowless browser too (measured, ADR 0113).
//!
//! # Alloy style, always
//!
//! CEF draws a windowless browser in **Alloy style** only
//! (`include/internal/cef_types_runtime.h`: "Windowless rendering will
//! always use Alloy style"), where every other page of the host is Chrome
//! style for ADR 0060's held requests (CEF issue #4308, ADR 0079 § 3.2).
//! ADR 0113 says what was measured of that.
//!
//! # A popup is never made, but told (queue item 978)
//!
//! Measured through this host in 959 and again in 978: in Alloy style,
//! with Chromium's own session held as ADR 0060 holds every page
//! (`chrome_tab.rs`), a windowless page's `window.open` never returned (the
//! press said nothing for thirty seconds) and the popup was never asked
//! for; with the page's session alone, the popup was made and its requests
//! left unheld; and a host with a popup did not end on `Browser.close`. So
//! a windowless page's popup is **cancelled before Chromium makes it**
//! (`OnBeforePopup`, `window.rs`), and one a person's gesture opened is
//! told to alo as a link pressed with ⌘ is ([`popup`],
//! [`crate::told::Told::Open`]): alo opens it as the conversation's next
//! turn. One with no gesture — which Chromium's own blocker stops in a
//! window — is cancelled and told to nobody. Cancelled, `window.open`
//! returns at once and the page's opener is `null`, as a blocked popup's
//! is.
//!
//! # Its view told by alo (queue item 979)
//!
//! The view's size and scale are the switch's until alo tells others, with
//! whether the page is hidden and has the focus, on descriptor 5 the other
//! way (`heard.rs`): what a window would have told Chromium. *Expand* is
//! then alo's own — the view told the window's size — and no window is
//! ever placed.

use crate::told::Told;
use std::time::{SystemTime, UNIX_EPOCH};

/// The switch that opens the host's page windowless, with its size and
/// scale after `=`: the host's own, which Chromium passes over.
pub const SWITCH: &str = "--alo-windowless=";

/// How many frames a second CEF paints a windowless page at, at most: the
/// display's sixty. CEF's own default is thirty (`windowless_frame_rate`,
/// `include/internal/cef_types.h`), half what a parked window streams at
/// (ADR 0076 B4).
pub const FRAME_RATE: i32 = 60;

/// The colour a windowless page is painted on before it paints its own:
/// opaque white, as a window's page is. CEF paints a windowless page
/// transparent unless its background is opaque (`cef_types_mac.h`), and a
/// card shows no desktop behind it. `0xAARRGGBB`.
pub const GROUND: u32 = 0xFFFF_FFFF;

/// The most CSS pixels a viewport is taken at on a side: more than any
/// display, and small enough that its device pixels at [`MOST_SCALE`] fit
/// CEF's `int`s many times over.
pub const MOST_SIDE: i32 = 16_384;

/// The largest scale a viewport is taken at.
pub const MOST_SCALE: f32 = 4.0;

/// The page's view: its size in CSS pixels, and how many device pixels a
/// CSS pixel is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Its width, in CSS pixels.
    pub width: i32,
    /// Its height, in CSS pixels.
    pub height: i32,
    /// Device pixels to a CSS pixel.
    pub scale: f32,
}

impl Viewport {
    /// The viewport `value` says — `880x600@2` — or [`None`] when it is not
    /// written so, or a side is not between 1 and [`MOST_SIDE`], or the
    /// scale not above 0 and at most [`MOST_SCALE`].
    pub fn parse(value: &str) -> Option<Self> {
        let (sides, scale) = value.split_once('@')?;
        let (width, height) = sides.split_once('x')?;
        let one_side = |text: &str| {
            text.parse::<i32>()
                .ok()
                .filter(|side| (1..=MOST_SIDE).contains(side))
        };
        let scale = scale
            .parse::<f32>()
            .ok()
            .filter(|scale| scale.is_finite() && *scale > 0.0 && *scale <= MOST_SCALE)?;
        Some(Self {
            width: one_side(width)?,
            height: one_side(height)?,
            scale,
        })
    }
}

/// What alo is told of a popup a windowless page asked for at `address`,
/// which the host never makes: the address to open as the conversation's
/// next turn, when a person's gesture opened it — and nothing for one no
/// person asked for, or with no address.
pub fn popup(address: Option<String>, gesture: bool) -> Option<Told> {
    address
        .filter(|address| gesture && !address.is_empty())
        .map(Told::Open)
}

/// What a paint was of, as CEF names its two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Painted {
    /// The page's own view.
    View,
    /// A popup widget over it: a `<select>`'s list, say.
    Popup,
}

/// The line the spike says on the host's error output for one paint, at
/// `at` milliseconds since the Unix epoch: what, its size in device pixels
/// and how many rectangles of it changed.
pub fn painted(at: u128, what: Painted, width: i32, height: i32, dirty: usize) -> String {
    let what = match what {
        Painted::View => "view",
        Painted::Popup => "popup",
    };
    format!("alo's CEF host: windowless: painted\t{at}\t{what}\t{width}x{height}\t{dirty}")
}

/// The line the spike says for anything else CEF tells a windowless page's
/// host of — a popup shown, hidden or sized, a drag begun — at `at`.
pub fn said(at: u128, what: &str) -> String {
    format!("alo's CEF host: windowless: {what}\t{at}")
}

/// Now, in milliseconds since the Unix epoch, as the lines say it: a
/// probe in another process compares it with its own clock; 0 if the
/// clock is before the epoch.
pub fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewport_is_its_size_in_css_pixels_and_its_scale() {
        assert_eq!(
            Viewport::parse("880x600@2"),
            Some(Viewport {
                width: 880,
                height: 600,
                scale: 2.0
            })
        );
        let half = Viewport::parse("878x520@1.5").map(|viewport| viewport.scale);
        assert!(half.is_some_and(|scale| (scale - 1.5).abs() < f32::EPSILON));
    }

    #[test]
    fn a_viewport_not_written_so_or_out_of_bounds_is_refused() {
        for refused in [
            "",
            "880x600",
            "880@2",
            "x600@2",
            "880x@2",
            "0x600@2",
            "880x-1@2",
            "16385x600@2",
            "880x600@0",
            "880x600@-1",
            "880x600@4.5",
            "880x600@NaN",
            "880x600@inf",
            "880.5x600@2",
            "880x600@2@2",
        ] {
            assert_eq!(Viewport::parse(refused), None, "{refused:?}");
        }
        assert!(Viewport::parse("16384x1@4").is_some(), "the bounds are in");
    }

    #[test]
    fn a_paint_is_one_line_of_tab_separated_fields() {
        assert_eq!(
            painted(1_700, Painted::View, 1760, 1200, 1),
            "alo's CEF host: windowless: painted\t1700\tview\t1760x1200\t1"
        );
        assert_eq!(
            painted(2, Painted::Popup, 300, 90, 0),
            "alo's CEF host: windowless: painted\t2\tpopup\t300x90\t0"
        );
        assert_eq!(
            said(5, "popup shown"),
            "alo's CEF host: windowless: popup shown\t5"
        );
    }

    #[test]
    fn a_popup_a_person_opened_is_told_as_a_link_to_open_and_no_other() {
        assert_eq!(
            popup(Some("https://example.com/p".to_owned()), true),
            Some(Told::Open("https://example.com/p".to_owned()))
        );
        assert_eq!(
            popup(Some("https://example.com/p".to_owned()), false),
            None,
            "no person's gesture"
        );
        assert_eq!(popup(Some(String::new()), true), None);
        assert_eq!(popup(None, true), None);
    }

    #[test]
    fn sixty_frames_a_second_and_an_opaque_ground() {
        assert_eq!(FRAME_RATE, 60);
        assert_eq!(GROUND >> 24, 0xFF, "opaque");
    }
}

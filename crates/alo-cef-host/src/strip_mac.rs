/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The strip an expanded card shows a link's address in (queue item 811),
//! drawn by `AppKit` in the card's own window: one reason to change — how
//! the strip `strip.rs` places is put on the screen.
//!
//! Why `AppKit`, and not a view of CEF's laid over the page: measured with
//! CEF 156.0.14, an overlay docked at a window's bottom left stood a title
//! bar's height (32 points) above the window's foot, and one asked lower
//! was not drawn at all — CEF's views place an overlay from the window's
//! top, title bar and all, and keep it inside a height a title bar short.
//! An `NSBox` of the paper colour holding a label, added over the page in
//! the window's content view — which `AppKit` lays out from the content's
//! bottom left, under the title bar — stands exactly at the page's bottom
//! left corner.
//!
//! The window is found among the application's by its content view, which
//! is what CEF gives as a views window's handle (as `unseen_mac.rs` finds
//! it). Each call is a safe one of `objc2-app-kit`'s; a pointer is compared
//! by its address and never followed. No `unsafe` (ADR 0080's boundary is
//! `application_mac.rs` alone).

use crate::strip::{self, Placed};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApp, NSBox, NSBoxType, NSColor, NSFont, NSLineBreakMode, NSTextAlignment, NSTextField,
    NSTitlePosition, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;
use std::ffi::c_void;

/// A window's strip: its content view's address, the plate, its label.
type Made = (usize, Retained<NSBox>, Retained<NSTextField>);

thread_local! {
    /// Each window's strip once made, on the main thread, where `AppKit`
    /// keeps its views.
    static MADE: RefCell<Vec<Made>> = const { RefCell::new(Vec::new()) };
}

/// One of the interface's colours, `0xRRGGBB` in sRGB, as `AppKit`'s.
fn colour(rgb: u32) -> Retained<NSColor> {
    let channel = |shift: u32| f64::from((rgb >> shift) & 0xFF) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(channel(16), channel(8), channel(0), 1.0)
}

/// Show `says` in the strip of the window whose content view is `handle` —
/// made the first time — or, with nothing to say, hide it. Nothing when
/// not on the main thread, or when no window of the application has that
/// view.
pub fn show(handle: *mut c_void, says: Option<&str>) {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let named = handle.addr();
    let Some(content) = NSApp(main).windows().iter().find_map(|window| {
        window
            .contentView()
            .filter(|view| Retained::as_ptr(view).addr() == named)
    }) else {
        return;
    };
    let made = MADE.with_borrow(|made| {
        made.iter()
            .find(|(each, _, _)| *each == named)
            .map(|(_, plate, label)| (plate.clone(), label.clone()))
    });
    let Some(words) = says else {
        if let Some((plate, _)) = made {
            plate.setHidden(true);
        }
        return;
    };
    let (plate, label) = made.unwrap_or_else(|| {
        let made = make(main, &content);
        MADE.with_borrow_mut(|all| all.push((named, made.0.clone(), made.1.clone())));
        made
    });
    label.setStringValue(&NSString::from_str(words));
    label.sizeToFit();
    let room = content.frame().size;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a window's and a label's size in points, far inside an i32"
    )]
    let at = strip::placed(
        (room.width.round() as i32, room.height.round() as i32),
        label.frame().size.width.ceil() as i32,
    );
    let (plate_frame, label_frame) = frames(at, room.height, label.frame().size.height);
    plate.setFrame(plate_frame);
    label.setFrame(label_frame);
    plate.setHidden(false);
}

/// The plate's frame and its label's for the strip placed `at`, in a
/// content view `tall` points tall laid out from its bottom left, the
/// label's words `words` points tall: the label inset by the padding
/// either side and centred down the plate.
fn frames(at: Placed, tall: f64, words: f64) -> (NSRect, NSRect) {
    let height = f64::from(at.height);
    let plate = NSRect::new(
        NSPoint::new(f64::from(at.x), tall - f64::from(at.y) - height),
        NSSize::new(f64::from(at.width), height),
    );
    let padding = f64::from(strip::PADDING);
    let label = NSRect::new(
        NSPoint::new(padding, ((height - words) / 2.0).max(0.0)),
        NSSize::new(
            (f64::from(at.width) - 2.0 * padding).max(0.0),
            words.min(height),
        ),
    );
    (plate, label)
}

/// A strip in `content`, hidden: a plate of the paper colour, borderless,
/// holding a label in the quiet ink, 12 points, cut at its end.
fn make(main: MainThreadMarker, content: &NSView) -> (Retained<NSBox>, Retained<NSTextField>) {
    let plate = NSBox::initWithFrame(main.alloc::<NSBox>(), NSRect::ZERO);
    plate.setBoxType(NSBoxType::Custom);
    plate.setTitlePosition(NSTitlePosition::NoTitle);
    plate.setBorderWidth(0.0);
    plate.setContentViewMargins(NSSize::new(0.0, 0.0));
    plate.setFillColor(&colour(strip::PAPER));
    plate.setHidden(true);
    let label = NSTextField::labelWithString(&NSString::from_str(""), main);
    label.setTextColor(Some(&colour(strip::QUIET)));
    let face = NSFont::fontWithName_size(&NSString::from_str(strip::FACE_NAME), strip::SIZE)
        .unwrap_or_else(|| NSFont::systemFontOfSize(strip::SIZE));
    label.setFont(Some(&face));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setAlignment(NSTextAlignment::Left);
    plate.addSubview(&label);
    content.addSubview(&plate);
    (plate, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plate_stands_at_the_contents_bottom_left_and_its_words_inside_it() {
        // A 900 × 600 page, its strip 196 wide: at the foot, from the left.
        let at = strip::placed((900, 600), 180);
        let (plate, label) = frames(at, 600.0, 15.0);
        assert_eq!(
            (
                plate.origin.x,
                plate.origin.y,
                plate.size.width,
                plate.size.height
            ),
            (0.0, 0.0, 196.0, 24.0)
        );
        assert_eq!(
            (
                label.origin.x,
                label.origin.y,
                label.size.width,
                label.size.height
            ),
            (8.0, 4.5, 180.0, 15.0)
        );
    }
}

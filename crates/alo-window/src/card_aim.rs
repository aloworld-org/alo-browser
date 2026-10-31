/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a point of the conversation's window is over the card's page,
//! and where in the page it is (ADR 0076 B, ADR 0024 § 5, queue item 718).
//!
//! The browser process decides it from its own composition, as
//! [`crate::conversation_aim`] decides which part a point is over: the
//! point is in the first screen's frame, and in the card's page box there —
//! which the browser process computes from the first screen's viewport
//! ([`alo_renderer::card_place::page_box`]), never from anything a renderer
//! said. A point there is the page's, in the page's CSS pixels from its
//! top-left corner — Chrome lays the page out at the page box's size, so
//! one CSS pixel of the first screen is one of the page's at any scale —
//! and is sent to the page as a person's ([`crate::card_input`]). Anywhere
//! else — the card's bar, what the person said, the box, the panel — is
//! the part's, aimed as before.
//!
//! The card's page is pressed only while the first screen shows its
//! picture: before a frame has arrived the page box is the card's white,
//! and there is nothing a person can see to press.

use crate::conversation_aim;
use crate::conversation_drawing::Part;
use crate::conversation_place::ConversationPlacement;
use crate::scale::Scale;
use alo_layout::Size;
use alo_paint::Point;
use alo_renderer::card_place::page_box;

/// What a point of the conversation's window is over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aimed {
    /// The card's page, at this point of it in its CSS pixels.
    Page {
        /// From the page's left edge.
        x: f64,
        /// From the page's top edge.
        y: f64,
    },
    /// A part, at this point of it in its CSS pixels, as
    /// [`conversation_aim::aim`] says.
    Part(Part, Point),
}

/// How far a wheel or a trackpad turned, as the window server says it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Turned {
    /// By lines, a wheel's notches: positive as the content moves down and
    /// right, as the window server counts them.
    Lines {
        /// Across.
        across: f32,
        /// Down the window.
        down: f32,
    },
    /// By device pixels, a trackpad's: positive as the content moves down
    /// and right.
    Pixels {
        /// Across.
        across: f64,
        /// Down the window.
        down: f64,
    },
}

/// How many CSS pixels a wheel's line scrolls: Chrome's own on every
/// platform, `kPixelsPerLineStep`, 40.
pub const LINE: f64 = 40.0;

/// What the point `at`, in device pixels, is over in a window placed as
/// `placed` at `scale`, when the first screen — laid out at
/// `card_on_screen` while it shows a card's picture, [`None`] otherwise —
/// holds a card's page: the page, or a part, or [`None`] when it is over
/// nothing a renderer drew.
pub fn aim(
    placed: &ConversationPlacement,
    scale: Scale,
    card_on_screen: Option<Size>,
    at: (f64, f64),
) -> Option<Aimed> {
    let (part, on) = conversation_aim::aim(placed, scale, at)?;
    if part == Part::Start
        && let Some(viewport) = card_on_screen
        && page_box(viewport).contains(alo_layout::Point::new(on.x, on.y))
    {
        let (x, y) = on_the_page(placed, scale, viewport, at);
        return Some(Aimed::Page { x, y });
    }
    Some(Aimed::Part(part, on))
}

/// Where the point `at`, in device pixels, is in the card's page's CSS
/// pixels, wherever it is — outside the page as well, where a press that
/// began on the page is dragged and let go, as a page in Chrome's own
/// window hears it.
pub fn on_the_page(
    placed: &ConversationPlacement,
    scale: Scale,
    viewport: Size,
    at: (f64, f64),
) -> (f64, f64) {
    let page = page_box(viewport);
    (
        scale.css(at.0 - f64::from(placed.start.x)) - f64::from(page.left()),
        scale.css(at.1 - f64::from(placed.start.y)) - f64::from(page.top()),
    )
}

/// How far the page is asked to scroll, right and down, in its CSS pixels,
/// by a turn of `turned` on a screen of `scale`: a line is [`LINE`], and a
/// device pixel `1 / scale` of a CSS one; the window server's directions
/// are the content's, the page's the reverse.
pub fn scrolled(turned: Turned, scale: Scale) -> (f64, f64) {
    match turned {
        Turned::Lines { across, down } => (-f64::from(across) * LINE, -f64::from(down) * LINE),
        Turned::Pixels { across, down } => (-scale.css(across), -scale.css(down)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_place::{Parts, start_viewport};

    /// The design's window at `scale`, both parts painted to fill their
    /// rooms.
    fn the_window(scale: Scale) -> (ConversationPlacement, Size) {
        let window = (scale.device(1280), scale.device(820));
        let placed = ConversationPlacement::of(
            window,
            scale,
            Parts {
                panel: Some((scale.device(240), scale.device(820))),
                start: Some((scale.device(1040), scale.device(820))),
                ..Parts::default()
            },
        );
        (placed, start_viewport(window, scale))
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "compared with what `aim` answers, which is an `Option`"
    )]
    fn page(x: f64, y: f64) -> Option<Aimed> {
        Some(Aimed::Page { x, y })
    }

    #[test]
    fn at_one_the_page_box_is_at_321_145_of_the_window() {
        let (placed, viewport) = the_window(Scale::ONE);
        let card = Some(viewport);
        // The panel's 240, then the page box at 81, 145 of the first screen.
        assert_eq!(
            aim(&placed, Scale::ONE, card, (321.0, 145.0)),
            page(0.0, 0.0)
        );
        assert_eq!(
            aim(&placed, Scale::ONE, card, (571.5, 245.0)),
            page(250.5, 100.0)
        );
        assert_eq!(
            aim(&placed, Scale::ONE, card, (1198.75, 664.5)),
            page(877.75, 519.5)
        );
    }

    #[test]
    fn at_two_a_device_pixel_is_half_a_css_pixel_of_the_page() {
        let (placed, viewport) = the_window(Scale::TWO);
        let card = Some(viewport);
        assert_eq!(
            aim(&placed, Scale::TWO, card, (642.0, 290.0)),
            page(0.0, 0.0)
        );
        assert_eq!(
            aim(&placed, Scale::TWO, card, (1143.0, 490.0)),
            page(250.5, 100.0)
        );
        assert_eq!(
            aim(&placed, Scale::TWO, card, (2397.0, 1329.0)),
            page(877.5, 519.5)
        );
    }

    #[test]
    fn outside_the_page_box_is_the_parts_and_not_forwarded() {
        for scale in [Scale::ONE, Scale::TWO] {
            let (placed, viewport) = the_window(scale);
            let card = Some(viewport);
            let device =
                |x: f64, y: f64| (x * f64::from(scale.factor()), y * f64::from(scale.factor()));
            // Just above the page box: the card's bar, the first screen's.
            assert_eq!(
                aim(&placed, scale, card, device(500.0, 144.5)),
                Some(Aimed::Part(Part::Start, Point::new(260.0, 144.5)))
            );
            // Its right and bottom edges are not in it.
            assert!(matches!(
                aim(&placed, scale, card, device(1199.0, 300.0)),
                Some(Aimed::Part(Part::Start, _))
            ));
            assert!(matches!(
                aim(&placed, scale, card, device(500.0, 665.0)),
                Some(Aimed::Part(Part::Start, _))
            ));
            // Left of it, and the panel.
            assert!(matches!(
                aim(&placed, scale, card, device(320.5, 300.0)),
                Some(Aimed::Part(Part::Start, _))
            ));
            assert!(matches!(
                aim(&placed, scale, card, device(100.0, 300.0)),
                Some(Aimed::Part(Part::Panel, _))
            ));
            // Outside the window: nothing.
            assert_eq!(aim(&placed, scale, card, device(-1.0, 300.0)), None);
        }
    }

    #[test]
    fn with_no_card_on_the_screen_the_page_box_is_the_first_screens() {
        let (placed, _) = the_window(Scale::ONE);
        assert_eq!(
            aim(&placed, Scale::ONE, None, (571.5, 245.0)),
            Some(Aimed::Part(Part::Start, Point::new(331.5, 245.0)))
        );
    }

    #[test]
    fn a_drag_is_placed_on_the_page_beyond_its_box_too() {
        let (placed, viewport) = the_window(Scale::TWO);
        assert_eq!(
            on_the_page(&placed, Scale::TWO, viewport, (0.0, 0.0)),
            (-321.0, -145.0)
        );
        assert_eq!(
            on_the_page(&placed, Scale::TWO, viewport, (2560.0, 1640.0)),
            (959.0, 675.0)
        );
    }

    #[test]
    fn a_wheel_scrolls_by_forty_a_line_and_a_trackpad_by_css_pixels() {
        assert_eq!(
            scrolled(
                Turned::Lines {
                    across: 0.0,
                    down: -3.0
                },
                Scale::TWO
            ),
            (0.0, 120.0)
        );
        assert_eq!(
            scrolled(
                Turned::Lines {
                    across: 1.0,
                    down: 0.5
                },
                Scale::ONE
            ),
            (-40.0, -20.0)
        );
        assert_eq!(
            scrolled(
                Turned::Pixels {
                    across: 10.0,
                    down: -300.0
                },
                Scale::TWO
            ),
            (-5.0, 150.0)
        );
        assert_eq!(
            scrolled(
                Turned::Pixels {
                    across: 0.0,
                    down: -300.0
                },
                Scale::ONE
            ),
            (0.0, 300.0)
        );
    }
}

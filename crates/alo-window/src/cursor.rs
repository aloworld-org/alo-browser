/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's cursor sends the selected tab's page (ADR 0024 § 5,
//! queue items 535 and 542).
//!
//! The event loop says what the window server said — the cursor moved, left
//! the window, the main button went down or up — together with what the
//! point is over, decided from the window's own composition
//! ([`crate::aim`]), and where it is in the page's CSS pixels wherever it
//! is ([`crate::aim::on_page`]). This decides which of those the page is
//! sent, as a [`Pointer`]; it keeps no window and calls nothing.
//!
//! # A move
//!
//! Over the page, a move is sent. Off it — onto the tab strip, over a line
//! the window is saying something in, out of the window — the page is told
//! once that the pointer **left**, and then nothing until it is back.
//!
//! # A press, and the moves until its release
//!
//! A press of the main button over the page with no modifier held is sent,
//! and from then until its release **every move is sent**, wherever the
//! cursor goes, and the release too: the page pressed hears the drag, as
//! Chrome's does. A press that was not sent — over the strip, over a
//! notice, with a modifier held (⌘ over a link opens a tab in Chrome, which
//! no tab here does, queue item 545) — sends nothing until its release,
//! moves included: the page was not pressed, and a move reporting a button
//! the page never heard go down would be a lie about it.

use crate::aim::Aimed;
use alo_paint::Point;
use alo_renderer::pointer::Pointer;

/// What the window remembers of the cursor, for the page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Whether a press of the main button was sent to the page, so its
    /// moves and its release are owed.
    pressing: bool,
    /// Whether the main button is down after a press that was not sent.
    elsewhere: bool,
    /// Whether the page was last told the pointer is over it.
    over: bool,
}

impl Cursor {
    /// The cursor moved over `aimed`, at `on_page` in the page's CSS pixels:
    /// what the page is sent, if anything.
    pub fn moved(&mut self, aimed: Aimed, on_page: Point) -> Option<Pointer> {
        if self.pressing {
            return Some(Pointer::moved(on_page.x, on_page.y));
        }
        if self.elsewhere {
            return None;
        }
        match aimed {
            Aimed::Page(at) => {
                self.over = true;
                Some(Pointer::moved(at.x, at.y))
            }
            Aimed::Strip(_) | Aimed::Nothing => self.off(on_page),
        }
    }

    /// The cursor left the window, last at `on_page`: what the page is
    /// sent, if anything — nothing while a press it heard is held, whose
    /// release still reaches it.
    pub fn left(&mut self, on_page: Point) -> Option<Pointer> {
        if self.pressing {
            return None;
        }
        self.off(on_page)
    }

    /// The main button went down over `aimed`, with `bare` saying no
    /// modifier is held: the press the page is sent, if it is the page's.
    pub fn pressed(&mut self, aimed: Aimed, bare: bool) -> Option<Pointer> {
        match aimed {
            Aimed::Page(at) if bare && !self.pressing => {
                self.pressing = true;
                self.over = true;
                Some(Pointer::down(at.x, at.y))
            }
            _ => {
                self.elsewhere = !self.pressing;
                None
            }
        }
    }

    /// The main button came up at `on_page`: the release the page is sent,
    /// if it heard the press.
    pub fn released(&mut self, on_page: Point) -> Option<Pointer> {
        self.elsewhere = false;
        if !self.pressing {
            return None;
        }
        self.pressing = false;
        Some(Pointer::up(on_page.x, on_page.y))
    }

    /// Whether a press the page heard is held: its release is owed, from
    /// wherever the cursor was last seen.
    pub const fn is_pressing(&self) -> bool {
        self.pressing
    }

    /// The pointer is off the page: say so once.
    fn off(&mut self, on_page: Point) -> Option<Pointer> {
        if !self.over {
            return None;
        }
        self.over = false;
        Some(Pointer::left(on_page.x, on_page.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::pointer::Went;

    fn page(x: f32, y: f32) -> Aimed {
        Aimed::Page(Point::new(x, y))
    }

    fn went(sent: Option<Pointer>) -> Option<(Went, f32, f32)> {
        sent.map(|pointer| (pointer.went, pointer.at.x, pointer.at.y))
    }

    #[test]
    fn a_move_over_the_page_is_sent_and_leaving_it_is_said_once() {
        let mut cursor = Cursor::default();
        let off = Point::new(5.0, -10.0);
        assert_eq!(
            went(cursor.moved(Aimed::Strip(Point::new(5.0, 26.0)), off)),
            None,
            "nothing was over the page to leave"
        );
        assert_eq!(
            went(cursor.moved(page(5.0, 1.0), Point::new(5.0, 1.0))),
            Some((Went::Moved, 5.0, 1.0))
        );
        assert_eq!(
            went(cursor.moved(Aimed::Strip(Point::new(5.0, 26.0)), off)),
            Some((Went::Left, 5.0, -10.0))
        );
        assert_eq!(went(cursor.moved(Aimed::Nothing, off)), None, "only once");
        assert_eq!(
            went(cursor.moved(page(6.0, 2.0), Point::new(6.0, 2.0))),
            Some((Went::Moved, 6.0, 2.0))
        );
        assert_eq!(
            went(cursor.left(Point::new(-1.0, 2.0))),
            Some((Went::Left, -1.0, 2.0)),
            "off the window"
        );
        assert_eq!(went(cursor.left(Point::new(-1.0, 2.0))), None);
    }

    #[test]
    fn a_press_on_the_page_is_followed_by_every_move_and_its_release() {
        let mut cursor = Cursor::default();
        assert_eq!(
            went(cursor.pressed(page(10.0, 10.0), true)),
            Some((Went::Down, 10.0, 10.0))
        );
        assert!(cursor.is_pressing());
        let outside = Point::new(-30.0, 500.0);
        assert_eq!(
            went(cursor.moved(Aimed::Nothing, outside)),
            Some((Went::Moved, -30.0, 500.0)),
            "a drag off the page is still the page's"
        );
        assert_eq!(went(cursor.left(outside)), None, "its release is owed");
        assert_eq!(
            went(cursor.released(outside)),
            Some((Went::Up, -30.0, 500.0))
        );
        assert!(!cursor.is_pressing());
        assert_eq!(
            went(cursor.moved(Aimed::Nothing, outside)),
            Some((Went::Left, -30.0, 500.0)),
            "the page was over its root at the release, and is told it left"
        );
    }

    #[test]
    fn a_press_that_was_not_the_pages_sends_nothing_until_it_is_let_go() {
        let mut cursor = Cursor::default();
        assert_eq!(
            went(cursor.moved(page(1.0, 1.0), Point::new(1.0, 1.0))),
            Some((Went::Moved, 1.0, 1.0))
        );
        assert_eq!(
            went(cursor.pressed(page(1.0, 1.0), false)),
            None,
            "a modifier was held"
        );
        assert_eq!(
            went(cursor.moved(page(2.0, 1.0), Point::new(2.0, 1.0))),
            None
        );
        assert_eq!(went(cursor.released(Point::new(2.0, 1.0))), None);
        assert_eq!(
            went(cursor.moved(page(3.0, 1.0), Point::new(3.0, 1.0))),
            Some((Went::Moved, 3.0, 1.0))
        );
        assert_eq!(
            went(cursor.pressed(Aimed::Strip(Point::new(3.0, 3.0)), true)),
            None,
            "the strip's"
        );
        assert_eq!(went(cursor.moved(Aimed::Nothing, Point::default())), None);
        assert_eq!(went(cursor.released(Point::default())), None);
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome's own window on the screen, which is what a person looks at on the
//! product path (ADR 0061, queue item 627).
//!
//! A shown Chrome ([`Chrome::start_shown`]) draws its one tab's page in a
//! window of its own, in `--app` mode. This process sizes that window so the
//! page is laid out at the size asked ([`fit`]) — the same size a headless
//! Chrome is told to lay it out at, so a page reads the same either way —
//! and puts it where it is asked to be on the screen ([`place`]). Both go
//! over the pipe, by the window's id, never by anything a page could reach.
//!
//! # Where it went, not where it was sent
//!
//! The system may overrule a window's place: measured against Chrome 155 on
//! macOS, a window asked for at the top of the screen is put under the menu
//! bar, one asked for far off the screen is pulled back onto it, and one
//! asked for smaller than Chrome allows is made larger and then moved. So
//! every answer here is [`Placed`] — where the window is read to be once it
//! has stopped moving, and the size of the page inside it — and never the
//! bounds that were asked for. A frame that follows Chrome's window follows
//! that.
//!
//! # Less room than was asked
//!
//! A window manager is allowed to give a window less than it is asked for,
//! and does: on 2026-10-26, with the session locked, Chrome's window held
//! the page at 1200×571 where 1200×800 was asked, and the same window on
//! the same checkout held 1200×800 minutes later with the screen shown. A
//! laptop's screen, a dock or Stage Manager does the same. So [`fit`]
//! answers the size the page was given whenever it is a size at all, and
//! what differs from what was asked is said by [`Placed::unlike`] rather
//! than refused (queue item 671): the page is laid out at what the screen
//! has room for, and Chrome reads it and lays it out at that, as it would
//! for a window a person made smaller.
//!
//! # In front, not in a strip
//!
//! A window put on the screen is not yet a window a person sees. Measured
//! against Chrome 155 on macOS with Stage Manager on: of two shown Chromes,
//! the window of the one started first is shrunk into Stage Manager's strip
//! — the window server lists it at 16,774 and 93×114 — while Chrome still
//! gives the bounds it was put at. So [`place`] brings the window in front
//! as well ([`bring_to_front`]), and what it answers is Chrome's bounds for
//! a window that is in front; whether it stays there is the person's, and
//! another program's, to decide.
//!
//! macOS honours that activation. Measured on 2026-10-27 against Chrome 156
//! on macOS 26.5 (item 643): the window came out of the strip after Finder
//! or Terminal was activated by a script, after a click on Terminal, while
//! keys were pressed there, and after another shown Chrome was brought in
//! front. The window found in the strip is always the one another window
//! was brought in front of afterwards — so two shown Chromes must not take
//! turns at the front if both are to be seen, and the tests show one at a
//! time.
//!
//! # What it is not yet
//!
//! A frame of ours around it. ADR 0061 accepts two windows, one of ours
//! tracking Chrome's, as the riskiest thing on the product path; this places
//! Chrome's and says where it is, [`crate::chrome_frame`] says where a frame
//! of ours stands around it (item 672), and the window of ours standing
//! there is item 642.

use crate::chrome::{Chrome, Failed};
use crate::chrome_protocol::{self, Bounds, ChromeWindow, Command};
use crate::chrome_tab::Tab;
use core::fmt;
use std::time::{Duration, Instant};

/// The widest and tallest a page in a shown Chrome is laid out at, in CSS
/// pixels: four times the widest screen this machine has, and past any
/// window a person looks at. Anything larger is refused before Chrome is
/// asked.
pub const LARGEST: u32 = 16_384;

/// How long apart two readings of a window must agree to be taken as where
/// it has stopped. Measured against Chrome 155: a window put where the
/// system allows is where it was asked to be at the first reading; one the
/// system moves had moved within 122 ms.
const STILL_FOR: Duration = Duration::from_millis(150);

/// Where Chrome's window is, read, and the page inside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// The window, as Chrome reads it: its id, its bounds on the screen,
    /// title bar included, and whether it is shown at them.
    pub window: ChromeWindow,
    /// The size the page inside it is laid out at, in CSS pixels: its
    /// window without the title bar.
    pub page: (f64, f64),
}

impl Placed {
    /// How the page inside the window differs from the `asked` size, or
    /// [`None`] when it is that size exactly.
    pub fn unlike(&self, asked: (u32, u32)) -> Option<Unlike> {
        // In whole pixels, each side exactly: a page is laid out at the size
        // asked or it is not, and a fraction of one is not it.
        let same = page_whole(self.page) == Some((i64::from(asked.0), i64::from(asked.1)));
        (!same).then_some(Unlike {
            asked,
            page: self.page,
        })
    }
}

/// A page laid out at a size other than the one asked, because its window
/// was given more or less room than it asked for: what is said, plainly,
/// rather than refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unlike {
    /// What was asked, in CSS pixels.
    pub asked: (u32, u32),
    /// What the page is laid out at, in CSS pixels.
    pub page: (f64, f64),
}

impl Unlike {
    /// Whether the page was given less room than asked, either way.
    pub fn is_less(&self) -> bool {
        self.page.0 < f64::from(self.asked.0) || self.page.1 < f64::from(self.asked.1)
    }
}

impl fmt::Display for Unlike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let given = if self.is_less() { "less" } else { "more" };
        write!(
            f,
            "Chrome's window was given {given} room than the {}×{} CSS pixels asked, and the \
             page is laid out at the {}×{} it holds",
            self.asked.0, self.asked.1, self.page.0, self.page.1
        )
    }
}

/// Why Chrome's window was not put where it was asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Unplaced {
    /// The Chrome is headless, and has no window to place. Nothing was
    /// written to it.
    Headless,
    /// The page was asked for at a size of nothing, or past [`LARGEST`].
    /// Nothing was written to Chrome.
    Unsized {
        /// What was asked, in CSS pixels.
        width: u32,
        /// What was asked, in CSS pixels.
        height: u32,
    },
    /// The window was sized for the page, and holds a page of no size: one
    /// side nothing, or not a whole number of CSS pixels up to [`LARGEST`].
    /// A page of a size other than the one asked is not this, but
    /// [`Placed::unlike`].
    NotFitted {
        /// What was asked, in CSS pixels.
        asked: (u32, u32),
        /// Where the window is, and the page it holds.
        placed: Placed,
    },
    /// The window was still moving when Chrome had been waited for as long
    /// as it is.
    Unsettled {
        /// Where it was last read to be.
        placed: Placed,
    },
    /// Asking Chrome failed.
    Failed(Failed),
}

impl fmt::Display for Unplaced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Headless => f.write_str("Chrome is headless, and has no window to place"),
            Self::Unsized { width, height } => write!(
                f,
                "a page in Chrome's window is laid out at between 1 and {LARGEST} CSS pixels \
                 each way, not {width}×{height}"
            ),
            Self::NotFitted { asked, placed } => write!(
                f,
                "Chrome's window holds the page at {}×{} CSS pixels, which is no size a page is \
                 laid out at, where {}×{} was asked",
                placed.page.0, placed.page.1, asked.0, asked.1
            ),
            Self::Unsettled { placed } => {
                let at = placed.window.bounds;
                write!(
                    f,
                    "Chrome's window was still moving, last at {},{} and {}×{}",
                    at.left, at.top, at.width, at.height
                )
            }
            Self::Failed(why) => why.fmt(f),
        }
    }
}

impl From<Failed> for Unplaced {
    fn from(why: Failed) -> Self {
        Self::Failed(why)
    }
}

/// The page of a shown Chrome's own window, by its target id: the page
/// nothing is attached to yet. Waited for, within Chrome's patience, since
/// Chrome may answer its pipe before its window is made.
///
/// # Errors
///
/// [`Failed::Silent`] when Chrome lists no such page within its patience,
/// or another [`Failed`].
pub(crate) fn page_of_the_window(chrome: &mut Chrome) -> Result<String, Failed> {
    let until = Instant::now() + chrome.patience();
    loop {
        let listed = chrome.ask(None, &Command::Targets)?;
        if let Some(page) = chrome_protocol::pages(&listed)
            .map_err(Failed::Malformed)?
            .into_iter()
            .next()
        {
            return Ok(page);
        }
        if Instant::now() >= until {
            return Err(Failed::Silent {
                waited: chrome.patience(),
            });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Size the window of `tab` so the page inside it is laid out `width` by
/// `height` CSS pixels, where it already is on the screen — and where it
/// went, the page at the size the window was given.
///
/// The title bar is not assumed: the window is sized to the page, the
/// difference between the window and the page inside it read, and the
/// window sized again by that much. A screen that gives the window less
/// room than that, or a system that makes it larger, is answered as it
/// went, and [`Placed::unlike`] says how it differs.
///
/// # Errors
///
/// [`Unplaced::Headless`] or [`Unplaced::Unsized`] before anything is
/// written to Chrome; [`Unplaced::NotFitted`] when the window then holds a
/// page of no size; or [`Unplaced::Unsettled`] or [`Unplaced::Failed`].
pub fn fit(tab: &mut Tab<'_>, width: u32, height: u32) -> Result<Placed, Unplaced> {
    if !tab.is_shown() {
        return Err(Unplaced::Headless);
    }
    if !(1..=LARGEST).contains(&width) || !(1..=LARGEST).contains(&height) {
        return Err(Unplaced::Unsized { width, height });
    }
    let (wide, tall) = (i64::from(width), i64::from(height));
    let first = read(tab)?;
    let at = |width, height| Bounds {
        left: first.window.bounds.left,
        top: first.window.bounds.top,
        width,
        height,
    };
    let sized = put(tab, first.window.id, at(wide, tall))?;
    let page = page_whole(sized.page);
    let fitted = match page {
        Some((page_wide, page_tall)) if (page_wide, page_tall) != (wide, tall) => {
            let around = |window: i64, page: i64, asked: i64| {
                window.saturating_sub(page).saturating_add(asked)
            };
            put(
                tab,
                first.window.id,
                at(
                    around(sized.window.bounds.width, page_wide, wide),
                    around(sized.window.bounds.height, page_tall, tall),
                ),
            )?
        }
        _ => sized,
    };
    if page_whole(fitted.page).is_some_and(|(wide, tall)| wide > 0 && tall > 0) {
        Ok(fitted)
    } else {
        Err(Unplaced::NotFitted {
            asked: (width, height),
            placed: fitted,
        })
    }
}

/// Put the window of `tab` with its top left corner at `left`, `top` on the
/// screen, in the screen's points, at the size it already is, and bring it
/// in front of every other window — and where it went.
///
/// # Errors
///
/// [`Unplaced::Headless`] before anything is written to Chrome, or
/// [`Unplaced::Unsettled`] or [`Unplaced::Failed`].
pub fn place(tab: &mut Tab<'_>, left: i32, top: i32) -> Result<Placed, Unplaced> {
    if !tab.is_shown() {
        return Err(Unplaced::Headless);
    }
    let now = read(tab)?;
    let placed = put(
        tab,
        now.window.id,
        Bounds {
            left: i64::from(left),
            top: i64::from(top),
            ..now.window.bounds
        },
    )?;
    bring_to_front(tab)?;
    Ok(placed)
}

/// Bring the window of `tab` in front of every other window, out of Stage
/// Manager's strip if it was put there.
///
/// # Errors
///
/// [`Unplaced::Headless`] before anything is written to Chrome, or
/// [`Unplaced::Failed`].
pub fn bring_to_front(tab: &mut Tab<'_>) -> Result<(), Unplaced> {
    if !tab.is_shown() {
        return Err(Unplaced::Headless);
    }
    let target = tab.target().to_owned();
    tab.ask_chrome(&Command::Activate { target })?;
    Ok(())
}

/// Close the window of `tab`: its one page, as a person closing it does
/// (item 672). What the tab is asked after this is declined, and
/// [`crate::chrome_frame::read`] says the window is closed.
///
/// # Errors
///
/// [`Unplaced::Headless`] before anything is written to Chrome, or
/// [`Unplaced::Failed`].
pub fn close(tab: &mut Tab<'_>) -> Result<(), Unplaced> {
    if !tab.is_shown() {
        return Err(Unplaced::Headless);
    }
    let target = tab.target().to_owned();
    tab.ask_chrome(&Command::CloseTarget { target })?;
    Ok(())
}

/// Where the window of `tab` is now, and the page inside it, read once.
///
/// # Errors
///
/// [`Unplaced::Headless`] before anything is written to Chrome, or
/// [`Unplaced::Failed`].
pub fn placed(tab: &mut Tab<'_>) -> Result<Placed, Unplaced> {
    if !tab.is_shown() {
        return Err(Unplaced::Headless);
    }
    Ok(read(tab)?)
}

/// Ask for the window `window` at `bounds`, and where it is once it has
/// stopped moving and the page inside it has followed its size.
fn put(tab: &mut Tab<'_>, window: i64, bounds: Bounds) -> Result<Placed, Unplaced> {
    let before = read(tab)?;
    tab.ask_chrome(&Command::PlaceWindow { window, bounds })?;
    let until = Instant::now() + tab.patience();
    let mut last = read(tab)?;
    loop {
        std::thread::sleep(STILL_FOR);
        let now = read(tab)?;
        if now == last && followed(&before, &now) {
            return Ok(now);
        }
        if Instant::now() >= until {
            return Err(Unplaced::Unsettled { placed: now });
        }
        last = now;
    }
}

/// Whether the page inside the window has been laid out again each way the
/// window was resized since `before`, as it is `now`.
///
/// A window's size and the size of the page inside it are read apart, and
/// the page's may lag. Measured on 2026-10-30 against alo's CEF host
/// (Chromium 156, queue item 727): in the first seconds after it starts, a
/// window resized from 900×600 to 900×632 was read at its new size while
/// the page inside it was still read at its old height for longer than
/// [`STILL_FOR`], so two readings agreed on a page the window no longer
/// held. A window resized one way holds a page of another size that way,
/// so until the page has changed where the window did, it is waited for.
fn followed(before: &Placed, now: &Placed) -> bool {
    let (was, is) = (before.window.bounds, now.window.bounds);
    // Exactly as read: a page laid out again is read as another number.
    let changed = |was: f64, is: f64| was.to_bits() != is.to_bits();
    (is.width == was.width || changed(before.page.0, now.page.0))
        && (is.height == was.height || changed(before.page.1, now.page.1))
}

/// The window of `tab`, and the page inside it, read once.
fn read(tab: &mut Tab<'_>) -> Result<Placed, Failed> {
    let target = tab.target().to_owned();
    let window = chrome_protocol::window_of(&tab.ask_chrome(&Command::WindowOf { target })?)
        .map_err(Failed::Malformed)?;
    let page = chrome_protocol::window(&tab.ask(&Command::Window)?).map_err(Failed::Malformed)?;
    Ok(Placed { window, page })
}

/// A page's size in whole CSS pixels, or [`None`] when either side is not
/// a whole number up to [`LARGEST`] — which a window Chrome sized itself
/// may hold, and which is then not the size asked.
fn page_whole((width, height): (f64, f64)) -> Option<(i64, i64)> {
    Some((whole(width)?, whole(height)?))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "only a whole number from nought to `LARGEST` is cast, which an i64 holds exactly"
)]
fn whole(length: f64) -> Option<i64> {
    (length.fract() == 0.0 && (0.0..=f64::from(LARGEST)).contains(&length)).then_some(length as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_whole_page_up_to_the_largest_is_a_size() {
        assert_eq!(page_whole((1200.0, 800.0)), Some((1200, 800)));
        assert_eq!(page_whole((0.0, 16_384.0)), Some((0, 16_384)));
        assert_eq!(page_whole((1200.5, 800.0)), None);
        assert_eq!(page_whole((1200.0, 16_385.0)), None);
        assert_eq!(page_whole((-1.0, 800.0)), None);
        assert_eq!(page_whole((f64::NAN, 800.0)), None);
        assert_eq!(page_whole((f64::INFINITY, 800.0)), None);
    }

    fn placed(page: (f64, f64)) -> Placed {
        Placed {
            window: ChromeWindow {
                id: 1,
                bounds: Bounds {
                    left: 120,
                    top: 60,
                    width: 1200,
                    height: 603,
                },
                state: chrome_protocol::WindowState::Normal,
            },
            page,
        }
    }

    #[test]
    fn a_page_has_followed_its_window_once_it_changed_where_the_window_did() {
        let before = placed((900.0, 568.0));
        let mut taller = before;
        taller.window.bounds.height = 632;
        assert!(
            !followed(&before, &taller),
            "the window grew, the page not yet"
        );
        let mut laid_out = taller;
        laid_out.page = (900.0, 600.0);
        assert!(followed(&before, &laid_out));
        let mut moved = before;
        moved.window.bounds.left = 160;
        assert!(
            followed(&before, &moved),
            "moved, not resized: nothing to follow"
        );
        let mut wider = before;
        wider.window.bounds.width = 1000;
        assert!(!followed(&before, &wider));
        wider.page = (1000.0, 568.0);
        assert!(followed(&before, &wider));
    }

    #[test]
    fn a_page_of_the_size_asked_is_like_it() {
        assert_eq!(placed((1200.0, 800.0)).unlike((1200, 800)), None);
    }

    #[test]
    fn a_page_given_less_room_is_said_plainly() {
        let short = placed((1200.0, 571.0)).unlike((1200, 800));
        assert_eq!(
            short,
            Some(Unlike {
                asked: (1200, 800),
                page: (1200.0, 571.0)
            })
        );
        let short = short.expect("unlike");
        assert!(short.is_less());
        assert_eq!(
            short.to_string(),
            "Chrome's window was given less room than the 1200×800 CSS pixels asked, and the \
             page is laid out at the 1200×571 it holds"
        );
        let narrow = placed((900.0, 800.0)).unlike((1200, 800)).expect("unlike");
        assert!(narrow.is_less(), "narrower and as tall is less");
        let larger = placed((500.0, 300.0)).unlike((300, 200)).expect("unlike");
        assert!(!larger.is_less());
        assert_eq!(
            larger.to_string(),
            "Chrome's window was given more room than the 300×200 CSS pixels asked, and the \
             page is laid out at the 500×300 it holds"
        );
    }

    #[test]
    fn a_refusal_says_what_was_asked() {
        assert_eq!(
            Unplaced::Unsized {
                width: 0,
                height: 800
            }
            .to_string(),
            "a page in Chrome's window is laid out at between 1 and 16384 CSS pixels each way, \
             not 0×800"
        );
        assert_eq!(
            Unplaced::Headless.to_string(),
            "Chrome is headless, and has no window to place"
        );
    }
}

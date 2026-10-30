/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the frame of ours around Chrome's window stands (ADR 0062, queue
//! item 672, cut from 642).
//!
//! On the product path Chrome draws the page in a window of its own, and a
//! window of ours holds what goes around it (ADR 0061). ADR 0062 decides
//! where that window of ours is: a strip directly above Chrome's, put by
//! **Chrome's word** for where its window is, and **shown only while the
//! window server lists Chrome's window on the screen at those bounds**.
//! This is that rule, as a function — [`stand`] — and the one reading of
//! both witnesses it is decided from — [`read`].
//!
//! # Why Chrome's word follows a person
//!
//! Measured on 2026-10-26 against Chrome 155.0.8059.86 on macOS 26.5.2,
//! with the screen in use, Stage Manager on and a floating window of ours
//! above Chrome's: a person dragging Chrome's title bar — thirty steps of
//! five points, a tenth of a second apart — was followed by
//! `Browser.getWindowForTarget` at every reading, about a third of a second
//! apart, to the point, and the window server agreed. Dragging its corner
//! was followed the same way, and the page inside was laid out again at each
//! size: 940×552 down to 900×520 in ten steps, by Chrome's word and by the
//! page's own `innerWidth` and `innerHeight`. So Chrome's word is where the
//! frame goes, and it is read rather than assumed.
//!
//! # Why the window server decides whether it is shown
//!
//! With Stage Manager on, Chrome's window shrunk into the strip is still at
//! its bounds by Chrome's word, and its page still reads `visible` (ADR
//! 0062 § 2). A frame put by Chrome's word alone would float over somebody
//! else's work. So the frame stands beside only while the window server
//! lists a window of Chrome's process, an ordinary one, at exactly the
//! bounds Chrome gives — and otherwise stands aside.
//!
//! Measured the same day, the question ADR 0062 § 4 left open: **a person
//! clicking the floating frame does not send Chrome's window to the strip.**
//! In five runs of five, a click on ours made our process the frontmost
//! application and Chrome's window stayed on the screen at its bounds for
//! the three seconds it was watched, so the decision stands.
//!
//! # Closed
//!
//! A person closing Chrome's window closes its one page: measured, the next
//! reading over the tab's session is declined (`Session with given id not
//! found`) and the window server lists nothing — or, under load, is never
//! answered at all. Chrome says first that it has detached the tab's
//! session, and a tab that has heard so answers [`Failed::Detached`]
//! rather than waiting, which is [`Stance::Closed`]. A reading declined
//! otherwise is followed by asking Chrome which targets it holds, and a
//! page it no longer holds is closed too — decided from Chrome's own words
//! about its targets, never from the words of an error. A Chrome whose pipe
//! has closed is closed.
//!
//! # What this is not
//!
//! The window of ours that stands there. This says where it goes and when;
//! the floating window that follows it, and closing Chrome's when ours is
//! closed, are item 642's.

use crate::chrome::Failed;
use crate::chrome_protocol::{self, Bounds, Command, WindowState};
use crate::chrome_tab::Tab;
use crate::chrome_window::{self, Placed, Unplaced};
use crate::place::STRIP_HEIGHT;
use crate::window_server::{self, Listed, Unlisted};
use core::fmt;

/// How tall the frame is, in the screen's points: the tab strip's height,
/// so that a strip drawn in it is drawn as the engine's window draws its
/// own. The line where 623's sentence goes may make it taller.
pub const FRAME_HEIGHT: i64 = STRIP_HEIGHT as i64;

/// The layer of an ordinary window, as the window server numbers it.
const ORDINARY: i64 = 0;

/// Where the frame stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    /// Chrome's window is on the screen where Chrome says it is, and the
    /// frame stands at `frame`: directly above it, as wide as it is.
    Beside {
        /// Where the frame goes, in the screen's points.
        frame: Bounds,
    },
    /// Chrome's window is not where a person sees it, and the frame stands
    /// aside until it is.
    Aside(Aside),
    /// Chrome's window is gone, and so is the frame.
    Closed(Closed),
}

/// Why the frame stands aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aside {
    /// Chrome says its window is not shown as a window: minimised into the
    /// Dock, or filling a space of its own with no room above it.
    NotAWindow(WindowState),
    /// The window server shows no ordinary window of Chrome's at the
    /// bounds Chrome gives: shrunk into Stage Manager's strip, behind
    /// another application, on another space — or moving between the two
    /// readings, which the next one settles.
    NotOnTheScreen,
}

/// How Chrome's window went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// Its page was closed — by a person closing the window, or by this
    /// process ([`chrome_window::close`]) — and Chrome no longer holds it.
    Page,
    /// Chrome itself has gone: its pipe closed.
    Chrome,
}

/// What one reading saw, and where it says the frame stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// Where the frame stands.
    pub stance: Stance,
    /// Chrome's word for its window, unless it is closed.
    pub placed: Option<Placed>,
    /// What the window server showed of Chrome's process, frontmost first;
    /// empty when Chrome's window is closed, since nothing was asked.
    pub listed: Vec<Listed>,
}

/// Why where the frame stands was not read.
#[derive(Debug, Clone, PartialEq)]
pub enum Unread {
    /// The Chrome is headless, and has no window to frame. Nothing was
    /// written to it.
    Headless,
    /// Chrome's word for its window was not read, and Chrome still holds
    /// the page.
    Unplaced(Unplaced),
    /// The window server was not read.
    Unlisted(Unlisted),
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Headless => f.write_str("Chrome is headless, and has no window to frame"),
            Self::Unplaced(why) => write!(f, "where Chrome's window is was not read: {why}"),
            Self::Unlisted(why) => why.fmt(f),
        }
    }
}

impl std::error::Error for Unread {}

/// Where the frame stands around Chrome's window as Chrome reads it,
/// `placed`, given what the window server shows of the process `owner`.
pub fn stand(placed: &Placed, owner: u32, listed: &[Listed]) -> Stance {
    let window = placed.window;
    if matches!(
        window.state,
        WindowState::Minimized | WindowState::Fullscreen
    ) {
        return Stance::Aside(Aside::NotAWindow(window.state));
    }
    let shown = listed.iter().any(|seen| {
        seen.owner == owner && seen.layer == ORDINARY && seen.bounds == Some(window.bounds)
    });
    if !shown {
        return Stance::Aside(Aside::NotOnTheScreen);
    }
    let at = window.bounds;
    Stance::Beside {
        frame: Bounds {
            left: at.left,
            top: at.top.saturating_sub(FRAME_HEIGHT),
            width: at.width,
            height: FRAME_HEIGHT,
        },
    }
}

/// Read where Chrome's window of `tab` is, by Chrome's word and then the
/// window server's, and where the frame stands.
///
/// The two are read one after the other, so a window moving between them
/// reads as [`Aside::NotOnTheScreen`] once; the reading after settles it.
///
/// # Errors
///
/// [`Unread::Headless`] before anything is written to Chrome;
/// [`Unread::Unplaced`] when Chrome was not read and still holds the page;
/// [`Unread::Unlisted`] when the window server was not read.
pub fn read(tab: &mut Tab<'_>) -> Result<Reading, Unread> {
    let placed = match chromes_word(tab)? {
        Ok(placed) => placed,
        Err(how) => return Ok(closed(how)),
    };
    let owner = tab.chrome_process();
    let listed = window_server::on_the_screen(owner).map_err(Unread::Unlisted)?;
    Ok(Reading {
        stance: stand(&placed, owner, &listed),
        placed: Some(placed),
        listed,
    })
}

/// Whether Chrome's window of `tab` is gone, and how, by Chrome's word
/// alone — the half of [`read`] that asks nothing of the window server, for
/// a shown Chrome with no frame of ours to stand beside it (ADR 0075):
/// [`None`] while Chrome still has the window.
///
/// # Errors
///
/// As [`read`], less [`Unread::Unlisted`]: nothing is listed.
pub fn gone(tab: &mut Tab<'_>) -> Result<Option<Closed>, Unread> {
    Ok(chromes_word(tab)?.err())
}

/// Chrome's word for where its window of `tab` is, or how it is gone.
fn chromes_word(tab: &mut Tab<'_>) -> Result<Result<Placed, Closed>, Unread> {
    if tab.is_shown() && tab.is_detached() {
        return Ok(Err(Closed::Page));
    }
    match chrome_window::placed(tab) {
        Ok(placed) => Ok(Ok(placed)),
        Err(Unplaced::Headless) => Err(Unread::Headless),
        Err(Unplaced::Failed(Failed::Gone)) => Ok(Err(Closed::Chrome)),
        Err(Unplaced::Failed(Failed::Detached)) => Ok(Err(Closed::Page)),
        Err(Unplaced::Failed(Failed::Declined(declined))) => {
            if still_held(tab)? {
                Err(Unread::Unplaced(Unplaced::Failed(Failed::Declined(
                    declined,
                ))))
            } else {
                Ok(Err(Closed::Page))
            }
        }
        Err(other) => Err(Unread::Unplaced(other)),
    }
}

/// A reading of a window that is gone.
fn closed(how: Closed) -> Reading {
    Reading {
        stance: Stance::Closed(how),
        placed: None,
        listed: Vec::new(),
    }
}

/// Whether Chrome still holds the page of `tab`, by its own list.
fn still_held(tab: &mut Tab<'_>) -> Result<bool, Unread> {
    let target = tab.target().to_owned();
    match tab.ask_chrome(&Command::Targets) {
        Ok(listed) => chrome_protocol::holds(&listed, &target)
            .map_err(|why| Unread::Unplaced(Unplaced::Failed(Failed::Malformed(why)))),
        Err(Failed::Gone) => Ok(false),
        Err(why) => Err(Unread::Unplaced(Unplaced::Failed(why))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome_protocol::ChromeWindow;

    const CHROME: u32 = 64_062;

    fn at(left: i64, top: i64, width: i64, height: i64) -> Bounds {
        Bounds {
            left,
            top,
            width,
            height,
        }
    }

    fn placed(bounds: Bounds, state: WindowState) -> Placed {
        Placed {
            window: ChromeWindow {
                id: 1_852_001_420,
                bounds,
                state,
            },
            page: (1000.0, 600.0),
        }
    }

    fn listed(owner: u32, layer: i64, bounds: Option<Bounds>) -> Listed {
        Listed {
            number: 4211,
            owner,
            layer,
            bounds,
        }
    }

    #[test]
    fn the_frame_stands_directly_above_a_window_the_screen_shows() {
        let chrome = at(300, 240, 1000, 632);
        let ours = listed(CHROME + 1, 3, Some(at(300, 204, 1000, 36)));
        assert_eq!(
            stand(
                &placed(chrome, WindowState::Normal),
                CHROME,
                &[ours, listed(CHROME, ORDINARY, Some(chrome))]
            ),
            Stance::Beside {
                frame: at(300, 204, 1000, FRAME_HEIGHT)
            }
        );
        // Moved by a person, and read again: the frame goes where it went.
        let moved = at(350, 290, 900, 552);
        assert_eq!(
            stand(
                &placed(moved, WindowState::Normal),
                CHROME,
                &[listed(CHROME, ORDINARY, Some(moved))]
            ),
            Stance::Beside {
                frame: at(350, 254, 900, 36)
            }
        );
        assert_eq!(FRAME_HEIGHT, 36);
    }

    #[test]
    fn a_window_in_the_strip_is_not_one_the_screen_shows() {
        // As iteration 380 measured it: Chrome's word, and Stage Manager's
        // thumbnail.
        let chrome = at(160, 120, 900, 632);
        let thumbnail = listed(CHROME, ORDINARY, Some(at(16, 773, 97, 115)));
        assert_eq!(
            stand(&placed(chrome, WindowState::Normal), CHROME, &[thumbnail]),
            Stance::Aside(Aside::NotOnTheScreen)
        );
        assert_eq!(
            stand(&placed(chrome, WindowState::Normal), CHROME, &[]),
            Stance::Aside(Aside::NotOnTheScreen),
            "behind another application, or closed by the window server's word"
        );
    }

    #[test]
    fn only_an_ordinary_window_of_chromes_own_at_its_bounds_will_do() {
        let chrome = at(160, 120, 900, 632);
        let normal = placed(chrome, WindowState::Normal);
        for wrong in [
            listed(CHROME + 1, ORDINARY, Some(chrome)),
            listed(CHROME, 3, Some(chrome)),
            listed(CHROME, ORDINARY, None),
            listed(CHROME, ORDINARY, Some(at(161, 120, 900, 632))),
            listed(CHROME, ORDINARY, Some(at(160, 120, 900, 631))),
        ] {
            assert_eq!(
                stand(&normal, CHROME, &[wrong]),
                Stance::Aside(Aside::NotOnTheScreen),
                "{wrong:?}"
            );
        }
    }

    #[test]
    fn a_window_that_is_not_a_window_has_no_frame() {
        let chrome = at(0, 0, 1710, 1107);
        let shown = [listed(CHROME, ORDINARY, Some(chrome))];
        for state in [WindowState::Minimized, WindowState::Fullscreen] {
            assert_eq!(
                stand(&placed(chrome, state), CHROME, &shown),
                Stance::Aside(Aside::NotAWindow(state))
            );
        }
        assert_eq!(
            stand(&placed(chrome, WindowState::Maximized), CHROME, &shown),
            Stance::Beside {
                frame: at(0, -36, 1710, 36)
            },
            "a maximised window is a window; where the frame can go above it is the window's"
        );
    }
}

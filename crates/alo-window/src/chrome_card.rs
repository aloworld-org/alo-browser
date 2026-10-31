/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab in Chrome shown as a card in the conversation (ADR 0076 B, queue
//! item 717): what the thread holding it does with what the conversation
//! asks of the card ([`CardOrder`]).
//!
//! The card is drawn by this engine from the frames Chrome streams of the
//! page ([`crate::chrome_screencast`]), while Chrome's own window is parked
//! off every display ([`crate::chrome_window::PARKED`], ADR 0076 B4). So:
//!
//! - **Stream**: Chrome is asked for PNG frames at most the card's size
//!   times the window's scale; each is answered as it is read, and the
//!   newest told to the conversation ([`ToFrame::Cast`]) every
//!   [`CAST_EVERY`] at most.
//! - **Unstream**: Chrome is told to stop, and the conversation what the
//!   stream came to once Chrome has answered ([`ToFrame::Uncast`]).
//! - **Expand**: the stream stops — the card is covered — and Chrome's own
//!   window, the same tab with nothing reloaded, is given the size asked,
//!   put where it is asked and brought in front, as ADR 0075 placed it over
//!   the conversation.
//!
//! - **Where the page is**: told to the conversation whenever the page's
//!   own frame goes somewhere while it streams ([`tell_where`],
//!   [`ToFrame::Moved`], queue item 742), so the card's address follows it.
//! - **Input**: a person's press, drag, release, wheel or key over the card's
//!   page, sent to the page as a person's (queue item 718,
//!   [`crate::chrome_card_input`]).
//!
//! Closing the card is closing the page, as the window's closing is
//! ([`crate::message::Order::CloseEverything`]): the stream is stopped
//! first ([`stop`]), so Chrome answers `Page.stopScreencast` before its page
//! goes.
//!
//! Whatever Chrome declines here is the card's own business: a stream that
//! would not start leaves a card with no picture, and a window that would
//! not move is said by the next reading of where it is. Neither ends the
//! tab, which still answers its agents.

use crate::chrome_card_input;
use crate::chrome_tab::Tab;
use crate::chrome_window;
use crate::frame_told::{ToFrame, Uncast};
use crate::message::CardOrder;
use alo_url::Url;
use std::time::{Duration, Instant};

/// How often a streamed tab is read between orders, for frames to answer
/// and the newest to tell: a quarter of a 60 Hz display's frame, so a
/// frame waits for its answer no longer than Chrome takes to make the next.
pub const CAST_EVERY: Duration = Duration::from_millis(4);

/// Who is told what the card's tab streamed.
pub type TellCard<'t> = Option<&'t (dyn Fn(ToFrame) -> bool + Send)>;

/// Carry out `order` on `tab`, telling `tell`.
pub fn carry_out(tab: &mut Tab<'_>, order: CardOrder, tell: TellCard<'_>) {
    match order {
        CardOrder::Stream { width, height } => {
            if tab.casting().is_on() {
                // Asked again at another size: stopped and started, since
                // Chrome takes a stream's size only as it starts.
                stop(tab, tell);
            }
            // Declined, the card keeps no picture; see the module's words.
            let _ = tab.cast(width, height);
        }
        CardOrder::Unstream => stop(tab, tell),
        CardOrder::Expand { at, size } => {
            stop(tab, tell);
            // Said by the next reading of where the window is, if it fails.
            let _ = chrome_window::fit(tab, size.0, size.1)
                .and_then(|_| chrome_window::place(tab, at.0, at.1));
        }
        CardOrder::Input(input) => chrome_card_input::forward(tab, &input),
    }
}

/// Stop `tab`'s stream, if it streams, and tell `tell` what it came to once
/// Chrome has answered — or that it did not.
pub fn stop(tab: &mut Tab<'_>, tell: TellCard<'_>) {
    if !tab.casting().is_on() {
        return;
    }
    let answered = tab.uncast().is_ok();
    let (frames, bytes) = tab.casting().read_so_far();
    if let Some(tell) = tell {
        // Nobody to hear it is a conversation that has gone.
        let _ = tell(ToFrame::Uncast(Uncast {
            frames,
            bytes,
            after: tab.casting().after_stopping(),
            answered,
        }));
    }
}

/// When the thread holding `tab` next looks at Chrome if no order comes
/// first: by `until`, and while `tab` streams, within [`CAST_EVERY`] — so
/// every frame is answered as Chrome makes it.
pub fn until(tab: &Tab<'_>, until: Option<Instant>) -> Option<Instant> {
    if !tab.casting().is_on() {
        return until;
    }
    let soon = Instant::now() + CAST_EVERY;
    Some(until.map_or(soon, |until| until.min(soon)))
}

/// Tell `tell` where `tab`'s page is now, if it streams and that is not
/// `told`, the address it last told (queue item 742): the tab follows its
/// own frame's commits and moves within its document as it reads Chrome
/// ([`Tab::address`]), so this asks Chrome nothing. Only a streamed page
/// is a card's, and a card covered is told where its page went once it is
/// streamed again.
pub fn tell_where(tab: &Tab<'_>, told: &mut Option<Url>, tell: TellCard<'_>) {
    if !tab.casting().is_on() {
        return;
    }
    let (Some(now), Some(tell)) = (tab.address(), tell) else {
        return;
    };
    if told.as_ref() == Some(now) {
        return;
    }
    *told = Some(now.clone());
    // Nobody to hear it is a conversation that has gone.
    let _ = tell(ToFrame::Moved(now.clone()));
}

/// Tell `tell` the newest frame `tab` read since it was last asked, if it
/// streams and read one. Chrome gone, or unreadable, tells nothing: the
/// next ask of the tab says so.
pub fn tell_newest(tab: &mut Tab<'_>, tell: TellCard<'_>) {
    if !tab.casting().is_on() {
        return;
    }
    if let (Ok(Some(frame)), Some(tell)) = (tab.take_frame(), tell) {
        let _ = tell(ToFrame::Cast(frame));
    }
}

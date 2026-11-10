/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab in Chrome shown as a card in the conversation (ADR 0076 B, queue
//! item 717): what the thread holding it does with what the conversation
//! asks of the card ([`CardOrder`]).
//!
//! The card is drawn by this engine from the frames Chrome streams of the
//! page ([`crate::chrome_screencast`]), while Chrome's own window is parked
//! ([`crate::chrome_window::PARKED`], ADR 0076 B4) — drawn fully
//! transparent there by the Chromium alo carries, which the window server
//! lists on no display (queue item 771). So:
//!
//! - **Stream**: Chrome is asked for PNG frames at most the card's size
//!   times the window's scale; each is answered as it is read, and the
//!   newest told to the conversation ([`ToFrame::Cast`]) every
//!   [`CAST_EVERY`] at most. A card whose page box is another size than the
//!   page is laid out at — the window made narrower or wider, the panel
//!   hidden or shown (queue item 774) — has Chrome's window fitted to it
//!   first — where it was parked while it is parked, so it stays parked —
//!   so one CSS pixel of the page is still one of the card's. A stream
//!   asked at a scale the page is not drawn at — the conversation moved to
//!   a display of another — has the window of the Chromium alo carries
//!   parked again by a display at that scale ([`crate::chrome_park`],
//!   queue item 772), so its frames come at it; the machine's Chrome keeps
//!   the scale it was started at ([`Scaled::Told`]). Of the streams asked while the thread was busy, only the
//!   newest is carried out ([`only_the_newest_stream`]): a window dragged
//!   narrower asks one at every step.
//! - **Unstream**: Chrome is told to stop, and the conversation what the
//!   stream came to once Chrome has answered ([`ToFrame::Uncast`]).
//! - **Expand**: the stream stops — the card is covered — and Chrome's own
//!   window, the same tab with nothing reloaded, is given the size asked
//!   where it is asked, and brought in front, as ADR 0075 placed it over
//!   the conversation; the Chromium alo carries draws it opaque from then.
//!
//! - **Where the page is**: told to the conversation whenever the page's
//!   own frame goes somewhere while it streams ([`tell_where`],
//!   [`ToFrame::Moved`], queue item 742), so the card's address follows it.
//! - **Whether it is loading**: told to the conversation whenever that
//!   changes while it streams ([`tell_loading`], [`ToFrame::Loading`],
//!   queue item 751): the page is streamed from its navigation's commit,
//!   and the card says *loading* from each later navigation's start — a
//!   link pressed, *Back*, a script's `location` — until Chrome says it
//!   stopped, whether or not it committed (queue item 770).
//! - **Where it can go**: told to the conversation whenever that changes
//!   while it streams ([`tell_travel`], [`ToFrame::Travel`], queue item
//!   737), read from the tab's session history after each navigation
//!   ([`crate::chrome_history`]), so the card's *Back* and *Forward* are
//!   pressable only when there is somewhere to go.
//! - **Back and Forward**: a person's press on either, carried out as
//!   Chrome's own buttons are ([`crate::chrome_history::go`]).
//! - **Input**: a person's hover, press, drag, release, wheel or key over
//!   the card's page, sent to the page as a person's (queue items 718 and
//!   740, [`crate::chrome_card_input`]); of the pointer's moves waiting,
//!   only the newest of each run ([`only_the_newest_moves`]).
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

use crate::card_input::{CardInput, Went};
use crate::chrome_card_caret;
use crate::chrome_card_input;
use crate::chrome_card_list;
use crate::chrome_history;
use crate::chrome_park::{self, Parked};
use crate::chrome_tab::Tab;
use crate::chrome_window;
use crate::frame_told::{ToFrame, Uncast};
use crate::inbox::Arrival;
use crate::message::{CardOrder, Order};
use alo_url::Url;
use std::time::{Duration, Instant};

/// How often a streamed tab is read between orders, for frames to answer
/// and the newest to tell: a quarter of a 60 Hz display's frame, so a
/// frame waits for its answer no longer than Chrome takes to make the next.
pub const CAST_EVERY: Duration = Duration::from_millis(4);

/// Who is told what the card's tab streamed.
pub type TellCard<'t> = Option<&'t (dyn Fn(ToFrame) -> bool + Send)>;

/// What the thread holding a card's tab has made of its window.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    /// The size, in CSS pixels, the page was last laid out at as far as
    /// this thread has fitted it (queue item 774).
    pub page: (u32, u32),
    /// Whether the window is still parked, the card not yet expanded: a
    /// parked window is fitted where it was parked, so that it stays
    /// parked (queue item 771).
    pub parked: bool,
    /// How the scale its page is drawn at is had (queue item 772).
    pub scaled: Scaled,
    /// The closed list a person's press was held on, to be pressed again
    /// once the card is expanded (queue item 810, [`chrome_card_list`]).
    pub list: Option<u64>,
    /// Whether the moves and the release of a press held on a closed list
    /// are still to come, and are not sent.
    pub holding_a_press: bool,
    /// Whether an input method's composition is under way on the page
    /// (queue item 802): where the person is typing is read as one begins.
    pub composing: bool,
}

/// How the scale a card's page is drawn at is had (ADR 0090 as amended,
/// queue item 772).
#[derive(Debug, Clone, PartialEq)]
pub enum Scaled {
    /// Told its Chromium at start (`--force-device-scale-factor`), and kept
    /// for its life wherever its window is — the machine's Chrome, whose
    /// window is parked at [`chrome_window::PARKED`].
    Told,
    /// From the display its window is parked by — the Chromium alo
    /// carries, untold: parked again ([`chrome_park::park`]) whenever a
    /// stream is asked at a scale it is not drawn at, so a conversation
    /// moved to a display of another scale has its card's frames at it.
    ByItsDisplay(Parked),
}

impl Fitted {
    /// A card's window as the thread holding it starts: its page laid out
    /// at `page`, and parked, its scale had as `scaled` says.
    pub fn parked(page: (u32, u32), scaled: Scaled) -> Self {
        Self {
            page,
            parked: true,
            scaled,
            list: None,
            holding_a_press: false,
            composing: false,
        }
    }

    /// Where a parked window is fitted, so that it stays parked: where it
    /// was parked.
    fn parked_at(&self) -> (i32, i32) {
        match &self.scaled {
            Scaled::Told => chrome_window::PARKED,
            Scaled::ByItsDisplay(parked) => parked.at,
        }
    }
}

/// Carry out `order` on `tab`, whose window is as `fitted` says, telling
/// `tell`.
pub fn carry_out(tab: &mut Tab<'_>, order: CardOrder, tell: TellCard<'_>, fitted: &mut Fitted) {
    match order {
        CardOrder::Stream {
            width,
            height,
            page,
            scale,
        } => {
            if tab.casting().is_on() {
                // Asked again at another size: stopped and started, since
                // Chrome takes a stream's size only as it starts.
                stop(tab, tell);
            }
            // Whether fitting the page moved the window off the display it
            // was parked by: fitted wider, a window may be mostly on
            // another.
            let mut moved_off = false;
            if fitted.page != page {
                // Declined, the page keeps its size and its frames are
                // drawn in the card's box as they come; asked again with
                // the next stream.
                let fit = if fitted.parked {
                    chrome_window::fit_at(tab, page.0, page.1, fitted.parked_at())
                } else {
                    chrome_window::fit(tab, page.0, page.1)
                };
                if let Ok(placed) = fit {
                    fitted.page = page;
                    if let Scaled::ByItsDisplay(parked) = &mut fitted.scaled {
                        let before = parked.drawn;
                        parked.drawn =
                            chrome_park::drawn_at(placed.window.bounds, &parked.displays);
                        moved_off = before == Some(parked.scale) && parked.drawn != before;
                    }
                }
            }
            if fitted.parked
                && let Scaled::ByItsDisplay(parked) = &mut fitted.scaled
                && (parked.scale != scale || moved_off)
            {
                // Parked by a display at the scale asked, once per scale:
                // whatever it came to is what the frames are drawn at.
                *parked = chrome_park::park(tab, scale);
            }
            // Declined, the card keeps no picture; see the module's words.
            let _ = tab.cast(width, height);
        }
        CardOrder::Unstream => stop(tab, tell),
        CardOrder::Expand { at, size } => {
            stop(tab, tell);
            // Said by the next reading of where the window is, if it fails.
            // Fitted where it is going, never grown where it was parked
            // first, onto the display holding its strip.
            if chrome_window::fit_at(tab, size.0, size.1, at).is_ok() {
                fitted.page = size;
                let _ = chrome_window::place(tab, at.0, at.1);
                fitted.parked = false;
            }
            // A person's press held on a closed list (queue item 810): made
            // again, now its menu opens where a person sees it.
            if let Some(list) = fitted.list.take() {
                chrome_card_list::press(tab, list);
            }
        }
        CardOrder::Input(input) => person(tab, &input, tell, fitted),
        // Declined, the page stays where it is, and the card says so by
        // its address not moving.
        CardOrder::Travel(way) => {
            let _ = chrome_history::go(tab, way);
        }
        // The mark is the serving thread's, which carries this out before
        // any card order reaches here (`crate::chrome_mark`).
        CardOrder::Unmark => {}
    }
}

/// Send a person's `input` to `tab`'s page — but a first press on a closed
/// list of a parked card's page, which is held, the conversation told so
/// that it expands the card, and the press's moves and release with it
/// (queue item 810, [`chrome_card_list`]).
fn person(tab: &mut Tab<'_>, input: &CardInput, tell: TellCard<'_>, fitted: &mut Fitted) {
    if fitted.holding_a_press {
        match input {
            CardInput::Pointer {
                went: Went::Released { .. },
                ..
            } => {
                fitted.holding_a_press = false;
                return;
            }
            CardInput::Pointer {
                went: Went::Dragged,
                ..
            } => return,
            _ => fitted.holding_a_press = false,
        }
    }
    if let CardInput::Pointer {
        went: Went::Pressed { clicks: 1 },
        x,
        y,
    } = *input
        && fitted.parked
        && let Some(list) = chrome_card_list::closed_list_at(tab, x, y)
    {
        fitted.list = Some(list);
        fitted.holding_a_press = true;
        if let Some(tell) = tell {
            // Nobody to hear it is a conversation that has gone.
            let _ = tell(ToFrame::ListPressed);
        }
        return;
    }
    // An input method's composition beginning: where the person is
    // typing, for its candidates (queue item 802).
    match input {
        CardInput::Compose { text, .. } if !text.is_empty() && !fitted.composing => {
            fitted.composing = true;
            if let (Some(caret), Some(tell)) = (chrome_card_caret::caret(tab), tell) {
                let _ = tell(ToFrame::Caret(caret));
            }
        }
        CardInput::Compose { .. } | CardInput::Commit(_) => fitted.composing = false,
        _ => {}
    }
    chrome_card_input::forward(tab, input);
}

/// `arrived` without each stream a later one in it supersedes: the newest
/// says the whole of what the card is to be streamed at, so the earlier are
/// not carried out — a window dragged narrower asks for a stream, and a
/// page fitted to it, at every step (queue item 774). Everything else is
/// kept, in its order; a stream stopped after the newest is still stopped.
pub fn only_the_newest_stream(arrived: Vec<Arrival>) -> Vec<Arrival> {
    let streams = |arrival: &Arrival| {
        matches!(
            arrival,
            Arrival::Order(Order::Card(CardOrder::Stream { .. }))
        )
    };
    let Some(newest) = arrived.iter().rposition(streams) else {
        return arrived;
    };
    arrived
        .into_iter()
        .enumerate()
        .filter(|(at, arrival)| *at == newest || !streams(arrival))
        .map(|(_, arrival)| arrival)
        .collect()
}

/// Of `arrived`, what is to be carried out: only the newest stream
/// ([`only_the_newest_stream`]) and only the newest of each run of a
/// person's moves ([`only_the_newest_moves`]).
pub fn to_carry_out(arrived: Vec<Arrival>) -> Vec<Arrival> {
    only_the_newest_moves(only_the_newest_stream(arrived))
}

/// `arrived` without each of a person's pointer moves over the card's page
/// that a newer one of the same kind supersedes, before any other input of
/// theirs (queue item 740, [`crate::card_moves`]): moves never queue on
/// the pipe an agent's commands use. Everything else is kept, in its order.
pub fn only_the_newest_moves(arrived: Vec<Arrival>) -> Vec<Arrival> {
    crate::card_moves::newest(arrived, |arrival| match arrival {
        Arrival::Order(Order::Card(CardOrder::Input(input))) => Some(input),
        _ => None,
    })
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

/// Tell `tell` whether `tab`'s page is loading, if it streams and that is
/// not `told`, what it last told (queue items 751 and 770): read from what
/// Chrome last said of the tab's own frame ([`Tab::is_loading`]), so this asks
/// Chrome nothing. A card covered is told once it is streamed again.
pub fn tell_loading(tab: &Tab<'_>, told: &mut Option<bool>, tell: TellCard<'_>) {
    if !tab.casting().is_on() {
        return;
    }
    let Some(tell) = tell else {
        return;
    };
    let now = tab.is_loading();
    if *told == Some(now) {
        return;
    }
    *told = Some(now);
    // Nobody to hear it is a conversation that has gone.
    let _ = tell(ToFrame::Loading(now));
}

/// Tell `tell` where `tab`'s page can go back or forward to, if it streams
/// and that is not what `told` last told (queue item 737): the session
/// history is asked of Chrome only when the tab's own frame has navigated
/// since it was last read. Chrome declining, or gone, tells nothing: it is
/// asked again at the next pass, and the next ask of the tab says it gone.
pub fn tell_travel(tab: &mut Tab<'_>, told: &mut chrome_history::Told, tell: TellCard<'_>) {
    if !tab.casting().is_on() {
        return;
    }
    let Some(tell) = tell else {
        return;
    };
    if let Ok(Some(travel)) = told.changed(tab) {
        // Nobody to hear it is a conversation that has gone.
        let _ = tell(ToFrame::Travel(travel));
    }
}

/// What the card was last told of its page, so each pass of the thread
/// holding it tells only what changed.
#[derive(Debug, Default)]
pub struct Told {
    /// Where its page was last told to be (queue item 742).
    at: Option<Url>,
    /// Whether it was last told to be loading (queue item 751).
    loading: Option<bool>,
    /// Where it can go back or forward to (queue item 737).
    travel: chrome_history::Told,
    /// How many of its requests the lists refused have been told (queue
    /// item 736).
    blocked: usize,
    /// Where looking for its page's cookie banner stands (ADR 0108, queue
    /// item 920).
    answering: crate::consent_answer::Answering,
}

impl Told {
    /// Tell `tell` what changed of `tab`'s page since this was last told,
    /// if it streams: whether it is loading before the frame drawn of it,
    /// so the first frame is never shown without the card knowing; the
    /// newest frame; where the page is; where it can go back or forward
    /// to; and what the lists refused.
    pub fn tell(&mut self, tab: &mut Tab<'_>, tell: TellCard<'_>) {
        tell_loading(tab, &mut self.loading, tell);
        tell_newest(tab, tell);
        tell_where(tab, &mut self.at, tell);
        tell_travel(tab, &mut self.travel, tell);
        tell_blocked(tab, &mut self.blocked, tell);
        tell_told(tab, tell);
        tell_answered(tab, &mut self.answering, tell);
    }
}

/// Tell `tell` what `tab`'s Chromium told beside its pipe since this was
/// last asked ([`Tab::take_told`], queue items 799 and 798), in order: read
/// from what its own thread read, so this asks Chromium nothing. Told while
/// the card is not streamed, it is dropped: the pointer's shape and a press
/// are of a card on the screen.
pub fn tell_told(tab: &Tab<'_>, tell: TellCard<'_>) {
    let told = tab.take_told();
    let (true, Some(tell)) = (tab.casting().is_on(), tell) else {
        return;
    };
    for each in told {
        // Nobody to hear it is a conversation that has gone.
        let _ = tell(ToFrame::Told(each));
    }
}

/// Tell `tell` each request of `tab`'s page the lists alo carries refused
/// since the `told` first (ADR 0094, queue item 736), if it streams: read
/// from what the tab refused as it answered Chrome ([`Tab::blocked`]), so
/// this asks Chrome nothing. A card covered is told what was refused
/// meanwhile once it is streamed again.
pub fn tell_blocked(tab: &Tab<'_>, told: &mut usize, tell: TellCard<'_>) {
    if !tab.casting().is_on() {
        return;
    }
    let Some(tell) = tell else {
        return;
    };
    for (url, by) in tab.blocked().iter().skip(*told) {
        *told += 1;
        // Nobody to hear it is a conversation that has gone.
        let _ = tell(ToFrame::Blocked {
            url: url.clone(),
            by: by.clone(),
        });
    }
}

/// Answer `tab`'s page's cookie banner, if it is time to look and *Settings
/// › Privacy* asks it ([`crate::consent_answer`], ADR 0108), and tell
/// `tell` what was done: only for a card, which has someone to tell —
/// streamed or covered, since a banner on a covered card's page is still
/// its page's.
pub fn tell_answered(
    tab: &mut Tab<'_>,
    answering: &mut crate::consent_answer::Answering,
    tell: TellCard<'_>,
) {
    let Some(tell) = tell else {
        return;
    };
    let banners = crate::privacy_chosen::process().banners();
    if let Some(answered) = answering.pass(tab, banners) {
        // Nobody to hear it is a conversation that has gone.
        let _ = tell(ToFrame::Consent(answered));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::TabOrder;

    fn stream(width: u32) -> Arrival {
        Arrival::Order(Order::Card(CardOrder::Stream {
            width,
            height: 520,
            page: (width, 520),
            scale: crate::scale::Scale::ONE,
        }))
    }

    /// What each arrival is, in words, to compare.
    fn named(arrived: &[Arrival]) -> Vec<String> {
        arrived
            .iter()
            .map(|arrival| match arrival {
                Arrival::Order(Order::Card(CardOrder::Stream { width, .. })) => {
                    format!("stream {width}")
                }
                Arrival::Order(Order::Card(CardOrder::Unstream)) => "unstream".to_owned(),
                Arrival::Order(Order::Tabs(_)) => "tabs".to_owned(),
                Arrival::Abandoned => "abandoned".to_owned(),
                _ => "other".to_owned(),
            })
            .collect()
    }

    #[test]
    fn of_the_streams_asked_while_busy_only_the_newest_is_carried_out() {
        // A window dragged narrower: a stream at every step.
        let dragged = vec![
            stream(878),
            stream(800),
            Arrival::Order(Order::Tabs(TabOrder::CloseSelected)),
            stream(700),
            stream(661),
            Arrival::Abandoned,
        ];
        assert_eq!(
            named(&only_the_newest_stream(dragged)),
            ["tabs", "stream 661", "abandoned"]
        );
        // A stream stopped after the newest is still stopped, and one
        // started again after that is kept as well.
        let covered = vec![
            stream(878),
            Arrival::Order(Order::Card(CardOrder::Unstream)),
        ];
        assert_eq!(
            named(&only_the_newest_stream(covered)),
            ["stream 878", "unstream"]
        );
        let uncovered = vec![
            Arrival::Order(Order::Card(CardOrder::Unstream)),
            stream(661),
        ];
        assert_eq!(
            named(&only_the_newest_stream(uncovered)),
            ["unstream", "stream 661"]
        );
        // Nothing streamed, nothing dropped.
        assert_eq!(
            named(&only_the_newest_stream(Vec::new())),
            Vec::<String>::new()
        );
    }

    #[test]
    fn of_a_persons_moves_waiting_only_the_newest_of_each_run_is_sent() {
        use crate::card_input::{CardInput, Went};
        let input = |went, x| {
            Arrival::Order(Order::Card(CardOrder::Input(CardInput::Pointer {
                went,
                x,
                y: 0.0,
            })))
        };
        let said = |arrived: &[Arrival]| -> Vec<String> {
            arrived
                .iter()
                .map(|arrival| match arrival {
                    Arrival::Order(Order::Card(CardOrder::Input(CardInput::Pointer {
                        went,
                        x,
                        ..
                    }))) => format!("{went:?} {x}"),
                    other => named(core::slice::from_ref(other)).concat(),
                })
                .collect()
        };
        let arrived = vec![
            input(Went::Hovered, 1.0),
            stream(878),
            input(Went::Hovered, 2.0),
            input(Went::Hovered, 3.0),
            input(Went::Pressed { clicks: 1 }, 3.0),
            input(Went::Dragged, 4.0),
            input(Went::Dragged, 5.0),
            input(Went::Released { clicks: 1 }, 5.0),
        ];
        assert_eq!(
            said(&only_the_newest_moves(arrived)),
            [
                "stream 878",
                "Hovered 3",
                "Pressed { clicks: 1 } 3",
                "Dragged 5",
                "Released { clicks: 1 } 5"
            ]
        );
    }
}

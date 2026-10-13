/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's pointer, on the selected tab's page (ADR 0024 § 5, queue
//! items 535 and 542).
//!
//! The window has decided from its own composition that the point is over
//! the page and sent it in the page's CSS pixels ([`crate::aim`]). The
//! conductor sends it to the selected tab's renderer as a person's pointer —
//! the one message that carries a point, sent for a person and never for
//! an agent — whose hit test finds the element, and paints the tab again,
//! since what the page heard may have changed it.
//!
//! # A press, and its release
//!
//! A press goes to the selected tab, if it is showing a document and not
//! held for its style: a held tab shows the last frame of something else,
//! and a person pressing it is pressing that. Its release goes to **the tab
//! the press went to**, wherever the pointer has got to — and to no tab, if
//! that one is no longer selected, or there was no press.
//!
//! # Moving, and leaving
//!
//! A move goes where a release would while a press is held — the tab
//! pressed, if it is still selected — and otherwise to the selected tab, on
//! the terms a press does; so does the pointer leaving the page. The page is
//! painted again after each, since its listeners may have changed it.
//!
//! **Moves are coalesced**: of moves that arrived together, with nothing
//! between them, only the last is sent ([`latest_moves_only`]). The window
//! server reports a move for every few pixels, and a renderer that is
//! answering one is not ready for the next; Chrome, given five moves at
//! once over CDP, fired three. Nothing but a move is ever dropped, and
//! nothing is moved past anything else.
//!
//! # A link followed, or a form sent
//!
//! Decided as the answer passes, as the person's (`alo-renderer`'s
//! `Tabs::point`), and said to the person: going there is session history's
//! (item 85), and a link a person clicked that silently went nowhere would
//! be a page that looks broken. A form's submission is said as one, with
//! its method, since a form sent by `GET` is not a link to the person who
//! pressed its button (queue item 543). A refusal is written into the
//! session's record as well (queue item 548), as every refused navigation
//! the conductor takes is.

use super::{Conducting, said_of, unexpected};
use crate::inbox::Arrival;
use crate::message::{News, Order};
use alo_renderer::navigate::Decided;
use alo_renderer::pointer::{Pointer, Went};
use alo_renderer::{FromRenderer, Tab, TabId};

/// What is said after the browser process decided a person's click goes
/// somewhere it does not yet go.
pub const NOT_GONE: &str = "and going to another page is not built yet (queue item 85)";

impl Conducting {
    /// Send a person's pointer to the page it is aimed at, and paint the
    /// page again.
    pub(super) fn pointed(&mut self, pointer: Pointer) -> Vec<News> {
        let Some(id) = self.aimed(pointer.went) else {
            return Vec::new();
        };
        let answered = self.tabs.point(id, pointer);
        let decided = self.take_asks(id);
        let mut news = match answered {
            Ok(FromRenderer::Delivered { .. }) => self.paint(id),
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        };
        // After the paint, which clears what was said.
        if let Some(decided) = decided {
            news.push(News::Said(going(&decided)));
        }
        news
    }

    /// The tab a button that `went` goes to, if any.
    fn aimed(&mut self, went: Went) -> Option<TabId> {
        let shows = |conducting: &Self, id: TabId| {
            conducting.tabs.selected() == Some(id)
                && conducting.tabs.tab(id).and_then(Tab::document).is_some()
        };
        match went {
            Went::Down => {
                self.pressed = self
                    .tabs
                    .selected()
                    .filter(|id| shows(self, *id) && !self.is_held(*id));
                self.pressed
            }
            Went::Up => self.pressed.take().filter(|id| shows(self, *id)),
            Went::Moved | Went::Left => match self.pressed {
                Some(pressed) => Some(pressed).filter(|id| shows(self, *id)),
                None => self
                    .tabs
                    .selected()
                    .filter(|id| shows(self, *id) && !self.is_held(*id)),
            },
        }
    }
}

/// What arrived, with every move of a person's pointer that is followed
/// straight away by another dropped: nothing else is dropped, and nothing
/// changes place.
pub fn latest_moves_only(arrived: Vec<Arrival>) -> Vec<Arrival> {
    let is_move = |arrival: &Arrival| {
        matches!(
            arrival,
            Arrival::Order(Order::Pointer(Pointer {
                went: Went::Moved,
                ..
            }))
        )
    };
    let mut kept: Vec<Arrival> = Vec::with_capacity(arrived.len());
    for arrival in arrived {
        if is_move(&arrival) && kept.last().is_some_and(is_move) {
            kept.pop();
        }
        kept.push(arrival);
    }
    kept
}

/// What a person is told of where their click was decided to go.
fn going(decided: &Decided) -> String {
    match decided {
        Decided::Go(navigation) if navigation.form => format!(
            "this form is sent by {} to {}, {NOT_GONE}",
            navigation.method.name(),
            navigation.url
        ),
        Decided::Go(navigation) => format!("this link goes to {}, {NOT_GONE}", navigation.url),
        Decided::Refused(refusal) => refusal.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(pointer: Pointer) -> Arrival {
        Arrival::Order(Order::Pointer(pointer))
    }

    /// Each arrival, as a pointer's what and where, or `None`.
    fn seen(arrived: &[Arrival]) -> Vec<Option<(Went, f32)>> {
        arrived
            .iter()
            .map(|arrival| match arrival {
                Arrival::Order(Order::Pointer(pointer)) => Some((pointer.went, pointer.at.x)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn of_moves_together_only_the_last_is_kept() {
        let kept = latest_moves_only(vec![
            pointer(Pointer::moved(1.0, 0.0)),
            pointer(Pointer::moved(2.0, 0.0)),
            pointer(Pointer::moved(3.0, 0.0)),
        ]);
        assert_eq!(seen(&kept), [Some((Went::Moved, 3.0))]);
    }

    #[test]
    fn a_move_is_never_carried_past_anything_else() {
        let kept = latest_moves_only(vec![
            pointer(Pointer::moved(1.0, 0.0)),
            pointer(Pointer::moved(2.0, 0.0)),
            pointer(Pointer::down(2.0, 0.0)),
            pointer(Pointer::moved(3.0, 0.0)),
            Arrival::NetworkGone,
            pointer(Pointer::moved(4.0, 0.0)),
            pointer(Pointer::moved(5.0, 0.0)),
            pointer(Pointer::left(6.0, 0.0)),
            pointer(Pointer::up(6.0, 0.0)),
        ]);
        assert_eq!(
            seen(&kept),
            [
                Some((Went::Moved, 2.0)),
                Some((Went::Down, 2.0)),
                Some((Went::Moved, 3.0)),
                None,
                Some((Went::Moved, 5.0)),
                Some((Went::Left, 6.0)),
                Some((Went::Up, 6.0)),
            ]
        );
    }
}

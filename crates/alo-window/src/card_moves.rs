/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's pointer moves over a card's page, coalesced (queue item 740):
//! of the moves waiting to be sent, only the newest of each run.
//!
//! The window server says where the pointer is sixty times a second and
//! more, and the window hands every move over the card's page to the
//! thread holding its Chromium ([`crate::card_input::Went::Hovered`],
//! [`crate::card_input::Went::Dragged`]). That thread sends each to the
//! page and waits for Chrome's answer, on the pipe an agent's commands use
//! too — so moves arriving faster than Chrome answers would queue there,
//! and an agent's command would wait behind them. A move says the whole of
//! where the pointer is, so a newer one of the same kind supersedes it:
//! each time the thread takes what is waiting, a move is dropped when the
//! next of the person's inputs after it is a move of the same kind.
//! Everything else is kept, in its order: a press, a release, a key or a
//! wheel is never dropped, and no move is carried past one — a drag's
//! moves before its release are the release's, and the page hears the
//! newest of them before it.

use crate::card_input::CardInput;

/// `waiting`, without each move superseded by a newer one of the same kind
/// before any other of the person's inputs; `input` says which of
/// `waiting` is a person's input over the card, and which is not (never
/// dropped, and never between two moves).
pub fn newest<T>(waiting: Vec<T>, input: impl Fn(&T) -> Option<&CardInput>) -> Vec<T> {
    // Whether each is superseded, found from the newest back: a move is
    // superseded when the person's next input after it is a move alike.
    let mut superseded = vec![false; waiting.len()];
    let mut next = None;
    for (at, each) in waiting.iter().enumerate().rev() {
        let Some(card) = input(each) else {
            continue;
        };
        let moving = card.moving();
        if let (Some(this), Some(that)) = (moving, next.flatten())
            && this == that
            && let Some(dropped) = superseded.get_mut(at)
        {
            *dropped = true;
        }
        next = Some(moving);
    }
    waiting
        .into_iter()
        .zip(superseded)
        .filter(|(_, dropped)| !dropped)
        .map(|(each, _)| each)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_input::{Keyed, PersonsKey, Went};
    use crate::keys::Held;

    /// What waits: a person's input, or something else, by name.
    #[derive(Debug, Clone, PartialEq)]
    enum Waiting {
        Input(CardInput),
        Other(&'static str),
    }

    fn at(went: Went, x: f64) -> Waiting {
        Waiting::Input(CardInput::Pointer { went, x, y: 0.0 })
    }

    fn coalesced(waiting: &[Waiting]) -> Vec<Waiting> {
        newest(waiting.to_vec(), |each| match each {
            Waiting::Input(input) => Some(input),
            Waiting::Other(_) => None,
        })
    }

    #[test]
    fn of_a_run_of_hovers_only_the_newest_is_sent() {
        assert_eq!(
            coalesced(&[
                at(Went::Hovered, 1.0),
                at(Went::Hovered, 2.0),
                Waiting::Other("an agent's command"),
                at(Went::Hovered, 3.0),
            ]),
            vec![Waiting::Other("an agent's command"), at(Went::Hovered, 3.0)],
            "what is not a person's input is kept, and stands between nothing"
        );
        assert_eq!(
            coalesced(&[at(Went::Hovered, 1.0)]),
            vec![at(Went::Hovered, 1.0)]
        );
        assert_eq!(coalesced(&[]), Vec::new());
    }

    #[test]
    fn a_press_a_release_a_key_or_a_wheel_is_never_dropped_nor_passed() {
        let press = at(Went::Pressed { clicks: 1 }, 5.0);
        let release = at(Went::Released { clicks: 1 }, 9.0);
        assert_eq!(
            coalesced(&[
                at(Went::Hovered, 1.0),
                at(Went::Hovered, 4.0),
                press.clone(),
                at(Went::Dragged, 6.0),
                at(Went::Dragged, 8.0),
                release.clone(),
                at(Went::Hovered, 9.0),
            ]),
            vec![
                at(Went::Hovered, 4.0),
                press,
                at(Went::Dragged, 8.0),
                release,
                at(Went::Hovered, 9.0),
            ]
        );
        let key = Waiting::Input(CardInput::Key(PersonsKey {
            key: Keyed::Tab,
            held: Held::NOTHING,
        }));
        let wheel = Waiting::Input(CardInput::Wheel {
            x: 0.0,
            y: 0.0,
            right: 0.0,
            down: 40.0,
        });
        assert_eq!(
            coalesced(&[
                at(Went::Hovered, 1.0),
                key.clone(),
                at(Went::Hovered, 2.0),
                wheel.clone(),
                wheel.clone(),
                at(Went::Hovered, 3.0),
            ]),
            vec![
                at(Went::Hovered, 1.0),
                key,
                at(Went::Hovered, 2.0),
                wheel.clone(),
                wheel,
                at(Went::Hovered, 3.0),
            ],
            "two turns of a wheel are two scrolls, not one"
        );
    }

    #[test]
    fn a_hover_is_not_superseded_by_a_drag_nor_a_drag_by_a_hover() {
        assert_eq!(
            coalesced(&[at(Went::Dragged, 1.0), at(Went::Hovered, 2.0)]),
            vec![at(Went::Dragged, 1.0), at(Went::Hovered, 2.0)]
        );
    }
}

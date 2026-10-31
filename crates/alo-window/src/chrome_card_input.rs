/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's input over a card's page, sent to the page in Chrome (ADR
//! 0076 B, ADR 0024 § 5, queue item 718): what the thread holding the
//! card's tab asks Chrome for each [`CardInput`] the window sends.
//!
//! Each is Chrome's `Input` domain, as a person's input to Chrome's own
//! window reaches the page — never a verb of an agent's (ADR 0002), which
//! names a box and never a point:
//!
//! - **A press** is the pointer arriving over the point and the main button
//!   going down there; **a drag** the pointer moving with it held; **the
//!   release** the button coming up wherever the pointer then is. With the
//!   press and the release at one point, the page hears a click, and a link
//!   pressed is followed.
//! - **A wheel** is Chrome's `mouseWheel` at the point, the page scrolled
//!   by the CSS pixels the window measured.
//! - **A key** goes down and comes up ([`crate::chrome_protocol::Command::Typed`]).
//!
//! What Chrome declines is not said: a person's input that does nothing is
//! a press that did nothing, as on any page, and the next frame shows the
//! page as it is. A Chrome gone is said by the next ask of the tab.

use crate::card_input::{CardInput, Went};
use crate::chrome_protocol::{Command, Mouse};
use crate::chrome_tab::Tab;

/// What Chrome is asked, in order, for `input`.
pub fn commands(input: &CardInput) -> Vec<Command> {
    let mouse = |kind, x, y| Command::Mouse {
        kind,
        x,
        y,
        toggles: false,
    };
    match input {
        CardInput::Pointer { went, x, y } => match went {
            Went::Pressed => vec![mouse(Mouse::Moved, *x, *y), mouse(Mouse::Pressed, *x, *y)],
            Went::Dragged => vec![mouse(Mouse::Dragged, *x, *y)],
            Went::Released => vec![mouse(Mouse::Released, *x, *y)],
        },
        CardInput::Wheel { x, y, right, down } => vec![Command::Wheel {
            x: *x,
            y: *y,
            right: *right,
            down: *down,
        }],
        CardInput::Key(key) => [true, false]
            .map(|down| Command::Typed {
                key: key.clone(),
                down,
            })
            .into(),
    }
}

/// Send `input` to `tab`'s page, each command once Chrome has answered the
/// one before, so the page hears them in order. Chrome declining one, or
/// gone, sends no more of them; see the module's words.
pub fn forward(tab: &mut Tab<'_>, input: &CardInput) {
    for command in commands(input) {
        if tab.ask(&command).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_input::{Keyed, PersonsKey};

    #[test]
    fn a_press_arrives_then_goes_down_and_a_release_comes_up() {
        let mouse = |kind| Command::Mouse {
            kind,
            x: 250.5,
            y: 100.0,
            toggles: false,
        };
        let at = |went| CardInput::Pointer {
            went,
            x: 250.5,
            y: 100.0,
        };
        assert_eq!(
            commands(&at(Went::Pressed)),
            vec![mouse(Mouse::Moved), mouse(Mouse::Pressed)]
        );
        assert_eq!(commands(&at(Went::Dragged)), vec![mouse(Mouse::Dragged)]);
        assert_eq!(commands(&at(Went::Released)), vec![mouse(Mouse::Released)]);
    }

    #[test]
    fn a_wheel_is_one_turn_and_a_key_goes_down_and_up() {
        assert_eq!(
            commands(&CardInput::Wheel {
                x: 1.0,
                y: 2.0,
                right: 0.0,
                down: 300.0
            }),
            vec![Command::Wheel {
                x: 1.0,
                y: 2.0,
                right: 0.0,
                down: 300.0
            }]
        );
        let key = PersonsKey {
            key: Keyed::Text("a".to_owned()),
            shift: false,
            option: false,
        };
        assert_eq!(
            commands(&CardInput::Key(key.clone())),
            vec![
                Command::Typed {
                    key: key.clone(),
                    down: true
                },
                Command::Typed { key, down: false }
            ]
        );
    }
}

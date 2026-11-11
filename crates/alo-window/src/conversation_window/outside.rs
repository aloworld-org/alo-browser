/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A new tab asked for by an agent outside alo, over its socket (queue
//! item 956, ADR 0112 § 4), judged by the relay
//! ([`crate::conversation_agents`]) and handed here, since only the window
//! makes a chat.
//!
//! alo's new tab is a new chat. `new_tab` is a new chat with nothing in it,
//! made as ⌘T and *New chat* make one; `open` with no tab named is a new
//! chat whose first turn is the address, made as *Open in new chat* on a
//! link makes one (queue item 798) — the road a person takes, never
//! another, so the card it opens is held to every rule a person's is, and
//! the live card it replaces is closed as theirs is. The terminal says an
//! agent outside alo asked.
//!
//! The caller is answered once the card has opened — its navigation
//! committed — with the address it was opened at, or with why not: the
//! card failed, or another page was asked for before it opened.

use super::ConversationWindow;
use crate::asking::{Answer, Refused};
use crate::conversation::Pressed as Decided;
use crate::conversation_agents::NewTabAsked;
use crate::conversation_chrome::Went;
use crate::conversation_drawing::Part;
use alo_renderer::card_menu::MenuItem;
use alo_url::Url;
use std::sync::mpsc::Sender;

/// The page an agent outside asked to open in a new chat, until its card
/// has opened or not.
#[derive(Debug, Default)]
pub(super) struct Outside {
    asked: Option<(Url, String, Sender<Answer>)>,
}

impl ConversationWindow {
    /// An agent outside alo asked for a new tab: a new chat, empty or with
    /// a card at the address it named.
    pub(super) fn new_tab_asked(&mut self, asked: NewTabAsked) {
        // One asked for earlier and still opening is replaced, as a person's
        // second address replaces their first.
        self.unasked("another page was asked for before it opened");
        match asked.to {
            None => {
                self.said
                    .push("an agent outside alo opened a new chat".to_owned());
                self.act_on(Part::Panel, Decided::NewChat);
                let _ = asked.reply.send(Answer::NewTab);
            }
            Some((url, address)) => {
                if self.card_streamed().is_none() {
                    let refused = Refused::NotOpened {
                        address,
                        said: "the window is not on the screen yet".to_owned(),
                    };
                    let _ = asked.reply.send(Answer::Refused(refused));
                    return;
                }
                self.said
                    .push(format!("an agent outside alo opens {url} in a new chat"));
                self.outside.asked = Some((url.clone(), address, asked.reply));
                self.act_on(
                    Part::Panel,
                    Decided::MenuItem(MenuItem::OpenInNewChat(url.serialised)),
                );
            }
        }
    }

    /// What became of a card, for the agent outside waiting on it.
    pub(super) fn answer_outside(&mut self, went: &Went) {
        let waited = self.outside.asked.as_ref().map(|(url, ..)| url.clone());
        match went {
            Went::Opened { url, .. } if waited.as_ref() == Some(url) => {
                if let Some((url, _, reply)) = self.outside.asked.take() {
                    let _ = reply.send(Answer::Opened {
                        tab: url.to_string(),
                    });
                }
            }
            Went::Unopened { url, why } if waited.as_ref() == Some(url) => {
                self.unasked(why);
            }
            // Another page opening in its place before it opened.
            Went::Opening(url) if waited.as_ref().is_some_and(|waited| waited != url) => {
                self.unasked("another page was opened before it");
            }
            _ => {}
        }
    }

    /// The page asked for and still opening, if any, answered as not opened,
    /// `why`.
    fn unasked(&mut self, why: &str) {
        if let Some((_, address, reply)) = self.outside.asked.take() {
            let refused = Refused::NotOpened {
                address,
                said: why.to_owned(),
            };
            let _ = reply.send(Answer::Refused(refused));
        }
    }
}

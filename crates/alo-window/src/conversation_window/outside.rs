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
//!
//! # alo started again (queue item 976, ADR 0114)
//!
//! A second `alo --conversation ADDRESS` on this profile hands its address
//! over the socket as `launched`, and the window opens it by the same road
//! as `open` with no tab — and **comes forward**, as a person who started
//! alo again expects: the front is asked of the event loop at once, and
//! owed back from the card's Chromium as a person's own address owes it
//! (`Front`). The address an alo is started with when none is
//! running is opened the same way ([`ConversationWindow::with_launched`]).
//! A launch that arrives before the window is on the screen — a second
//! launch in the first one's first moments — waits for it, rather than be
//! refused as an agent's `open` is; a later launch replaces it.

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
    /// A launch handed over before the window was on the screen.
    waiting: Option<NewTabAsked>,
}

impl ConversationWindow {
    /// The window, opening `url` in a new chat and coming forward once it
    /// is on the screen: the address this alo was started with, if any
    /// (queue item 976). Nobody waits on the answer.
    #[must_use]
    pub fn with_launched(mut self, url: Option<Url>) -> Self {
        let Some(url) = url else {
            return self;
        };
        let (reply, _) = std::sync::mpsc::channel();
        let address = url.to_string();
        self.outside.waiting = Some(NewTabAsked {
            to: Some((url, address)),
            forward: true,
            reply,
        });
        self
    }

    /// A launch that waited for the window to be on the screen, opened now
    /// that it is.
    pub(super) fn launch_waiting(&mut self) {
        if self.card_streamed().is_some()
            && let Some(waiting) = self.outside.waiting.take()
        {
            self.new_tab_asked(waiting);
        }
    }

    /// An agent outside alo asked for a new tab: a new chat, empty or with
    /// a card at the address it named.
    pub(super) fn new_tab_asked(&mut self, asked: NewTabAsked) {
        // A launch waits for the window to be on the screen, the later
        // replacing the earlier.
        if asked.forward && asked.to.is_some() && self.card_streamed().is_none() {
            if let Some(replaced) = self.outside.waiting.replace(asked) {
                let refused = Refused::NotOpened {
                    address: replaced.to.map(|(_, address)| address).unwrap_or_default(),
                    said: "alo was started again with another page before it opened".to_owned(),
                };
                let _ = replaced.reply.send(Answer::Refused(refused));
            }
            return;
        }
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
                self.said.push(if asked.forward {
                    format!("alo was started again with {url}: it opens in a new chat, and alo comes forward")
                } else {
                    format!("an agent outside alo opens {url} in a new chat")
                });
                self.outside.asked = Some((url.clone(), address, asked.reply));
                self.act_on(
                    Part::Panel,
                    Decided::MenuItem(MenuItem::OpenInNewChat(url.serialised)),
                );
                if asked.forward {
                    // Forward now, and again once its Chromium, starting,
                    // has taken the front (`Front`).
                    self.front.asked = true;
                    self.front.owed = true;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beside::Chromium;
    use crate::conversation_chrome::ChromeOpener;
    use crate::conversation_drawing::ConversationDrawing;
    use crate::conversation_window::Measured;
    use crate::scale::Scale;
    use alo_renderer::host::Renderers;
    use std::sync::mpsc::channel;

    /// A window of no renderer and no Chromium — one at a place nothing
    /// is, so a card asked for starts none — not yet on the screen.
    fn unmeasured() -> ConversationWindow {
        let drawing = ConversationDrawing::start(
            Renderers::running("/nonexistent/alo-render", &[]),
            Box::new(|_| true),
        )
        .unwrap();
        let nowhere = Chromium::Carried("/nonexistent/alo Chromium".into());
        let chrome = ChromeOpener::start(nowhere, Box::new(|_| true)).unwrap();
        ConversationWindow::new(Some(Scale::ONE), drawing, chrome)
    }

    fn on_the_screen(window: &mut ConversationWindow) {
        window.measured_as(Measured {
            size: (1280, 820),
            scale_factor: 1.0,
            outer: Some((0, 0)),
        });
    }

    fn asked(address: &str, forward: bool) -> (NewTabAsked, std::sync::mpsc::Receiver<Answer>) {
        let (reply, answered) = channel();
        let url = alo_url::parse(address).unwrap();
        let asked = NewTabAsked {
            to: Some((url, address.to_owned())),
            forward,
            reply,
        };
        (asked, answered)
    }

    #[test]
    fn a_launch_opens_a_new_chat_and_comes_forward_and_an_agents_open_does_not() {
        let mut window = unmeasured();
        on_the_screen(&mut window);
        window.focused(false);
        let first = window.conversation().on_screen();
        let (open, _) = asked("http://127.0.0.1:9/agent", false);
        window.new_tab_asked(open);
        assert!(
            !window.take_front(),
            "an agent's open brings nothing forward"
        );
        let (launch, _) = asked("http://127.0.0.1:9/launched", true);
        window.new_tab_asked(launch);
        assert!(window.take_front(), "a launch comes forward");
        assert_ne!(window.conversation().on_screen(), first, "in a new chat");
        assert_eq!(
            window
                .conversation()
                .going()
                .map(ToString::to_string)
                .as_deref(),
            Some("http://127.0.0.1:9/launched")
        );
        assert!(
            window.take_said().contains(
                &"alo was started again with http://127.0.0.1:9/launched: it opens in a new \
              chat, and alo comes forward"
                    .to_owned()
            )
        );
        // Its Chromium, starting, takes the front: taken back.
        window.focused(true);
        window.focused(false);
        assert!(window.take_front());
        window.finish();
    }

    #[test]
    fn a_launch_before_the_window_is_on_the_screen_waits_for_it() {
        let mut window = unmeasured();
        let (agents, refused) = asked("http://127.0.0.1:9/agent", false);
        window.new_tab_asked(agents);
        assert!(matches!(
            refused.try_recv(),
            Ok(Answer::Refused(Refused::NotOpened { .. }))
        ));
        let (early, replaced) = asked("http://127.0.0.1:9/early", true);
        window.new_tab_asked(early);
        let (later, _) = asked("http://127.0.0.1:9/later", true);
        window.new_tab_asked(later);
        assert!(
            matches!(
                replaced.try_recv(),
                Ok(Answer::Refused(Refused::NotOpened { said, .. })) if said.contains("another page")
            ),
            "the later launch replaces the earlier"
        );
        assert!(window.conversation().going().is_none(), "nothing yet");
        on_the_screen(&mut window);
        assert_eq!(
            window
                .conversation()
                .going()
                .map(ToString::to_string)
                .as_deref(),
            Some("http://127.0.0.1:9/later")
        );
        assert!(window.take_front());
        window.finish();
    }

    #[test]
    fn the_address_alo_was_started_with_opens_once_the_window_is_on_the_screen() {
        let url = alo_url::parse("http://127.0.0.1:9/first").unwrap();
        let mut window = unmeasured().with_launched(Some(url));
        assert!(window.conversation().going().is_none());
        on_the_screen(&mut window);
        assert_eq!(
            window
                .conversation()
                .going()
                .map(ToString::to_string)
                .as_deref(),
            Some("http://127.0.0.1:9/first")
        );
        assert!(window.take_front());
        window.finish();
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the browser process decides of a claim on the chats panel or the
//! first screen (ADR 0074, ADR 0024 § 5, queue item 710).
//!
//! The renderer that drew the part answers a person's press with a claim
//! ([`crate::conversation_claim`]), and the claim is a stranger's word: the
//! panel shows every chat's name and the first screen every kept site's,
//! and a name that found a way into either renderer would be speaking here.
//! So the browser process acts **only on a claim about something the part
//! showed, which it still holds, answering a press it sent to that part**,
//! and refuses every other by name, as [`crate::strip_decide`] does a tab:
//!
//! - **A place the part was not shown.** A chat, a site or a waiting chat
//!   is claimed by its place, and the browser process kept which was at
//!   each place of the part it last saw painted ([`Shown`]). A place past
//!   them is refused.
//! - **Something gone since.** The chat at that place may have gone between
//!   the part being painted and the press being answered; it is refused,
//!   never moved along to whatever is in its place now, which is not what
//!   the person pressed.
//! - **Something the part does not draw.** The panel's renderer claiming
//!   the box, or the first screen's claiming *New chat*, is a renderer
//!   naming what the person could not have pressed there.
//! - **A claim with no press behind it.** A claim answers only
//!   [`crate::message::ToRenderer::ConversationPress`]; one in answer to
//!   anything else is refused ([`unasked`]), and nothing is done.
//!
//! What is decided is a thing the browser process names in its own terms —
//! its own identity for a chat, its own record of a site — never anything
//! the renderer said but a place or a kind.

use crate::conversation_claim::{Claim, Part};
use crate::message::{Failure, FromRenderer};
use crate::typed::Offer;
use alo_url::Url;
use core::fmt;

/// What a part showed when it was painted, in the order it drew each list,
/// as the browser process names each thing: `C` a chat, `S` a kept site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown<C, S> {
    /// The panel's chats, in the order it draws them
    /// ([`crate::panel::drawn_order`]).
    pub chats: Vec<C>,
    /// The first screen's kept sites, in order.
    pub sites: Vec<S>,
    /// The first screen's waiting chats, in order.
    pub waiting: Vec<C>,
    /// Whether the first screen shows a page as a card (queue item 717).
    pub card: bool,
    /// The addresses of the links the conversation draws, in order (queue
    /// items 708, 703): every search's sites on the All tab and every page
    /// shown earlier, oldest turn first.
    pub found: Vec<Url>,
    /// What the first screen offered for what was typed, in order, the
    /// first what Return chooses (queue item 714).
    pub offers: Vec<Offer>,
    /// Whether the panel was hidden, drawn as the rail (queue item 711):
    /// *Search chats*, the button that hides it and *Settings* are drawn
    /// only while it is not, and *Show the panel* only while it is.
    pub hidden: bool,
}

impl<C, S> Default for Shown<C, S> {
    /// Nothing shown.
    fn default() -> Self {
        Self {
            chats: Vec::new(),
            sites: Vec::new(),
            waiting: Vec::new(),
            card: false,
            found: Vec::new(),
            offers: Vec::new(),
            hidden: false,
        }
    }
}

/// What a press on the panel or the first screen came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided<C, S> {
    /// *New chat*.
    NewChat,
    /// This chat, from the panel.
    Chat(C),
    /// *Settings*.
    Settings,
    /// This kept site.
    Site(S),
    /// *Add a site*.
    AddASite,
    /// This chat, waiting on the person.
    Waiting(C),
    /// The one box.
    TheBox,
    /// The box's *Go*.
    Go,
    /// The card's *Back* (queue item 717).
    Back,
    /// The card's *Forward*.
    Forward,
    /// The card's *Expand to the whole window*.
    Expand,
    /// The card's *Close*.
    CloseTheCard,
    /// The title of a site a search found, or a page shown earlier, at this
    /// address (queue items 708, 703).
    Found(Url),
    /// This offer for what was typed, chosen as Return chooses the first
    /// (queue item 714).
    Offer(Offer),
    /// The panel's *Search chats* (queue item 711).
    SearchChats,
    /// The button that hides the panel.
    HideThePanel,
    /// The rail's button that shows the panel again.
    ShowThePanel,
    /// The press was on nothing, and nothing is done.
    Nothing,
    /// The claim was refused, and nothing is done.
    Refused(Refusal),
    /// The part's renderer could not answer the press.
    Failed(Failure),
}

/// Which of a part's lists a place was claimed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listed {
    /// The panel's chats.
    Chats,
    /// The first screen's kept sites.
    Sites,
    /// The first screen's waiting chats.
    Waiting,
    /// The sites a search found (queue item 708).
    Found,
    /// What was offered for what was typed (queue item 714).
    Offers,
}

impl fmt::Display for Listed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Listed::Chats => "chat",
            Listed::Sites => "site",
            Listed::Waiting => "waiting chat",
            Listed::Found => "site found",
            Listed::Offers => "offer",
        })
    }
}

/// Why a claim was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The claim named a place the part was not shown.
    NotShown {
        /// Which list.
        listed: Listed,
        /// The place claimed.
        at: usize,
        /// How many the part was shown.
        shown: usize,
    },
    /// What was at the place claimed has gone since the part was painted.
    Gone {
        /// Which list.
        listed: Listed,
        /// The place claimed.
        at: usize,
    },
    /// The part's renderer claimed something that part does not draw.
    NotDrawnThere {
        /// The part pressed.
        part: Part,
        /// What it claimed.
        claim: Claim,
    },
    /// The renderer claimed a button of a card the first screen did not
    /// show when it was painted, or no longer shows (queue item 717).
    NoCard {
        /// What it claimed.
        claim: Claim,
    },
    /// The renderer claimed a part of the panel it does not draw as it was
    /// painted, or as it is now: the rail's while it was not hidden, the
    /// whole panel's while it was (queue item 711).
    PanelNotSo {
        /// What it claimed.
        claim: Claim,
        /// Whether the panel was hidden, painted or now.
        hidden: bool,
    },
    /// A claim that answered no press.
    Unasked,
    /// The part's renderer answered a press with something that is not a
    /// claim.
    NotAClaim {
        /// The part pressed.
        part: Part,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let refused = "and nothing was done";
        match self {
            Refusal::NotShown { listed, at, shown } => write!(
                f,
                "a renderer claimed the {listed} at place {at}, and {shown} were shown, {refused}"
            ),
            Refusal::Gone { listed, at } => write!(
                f,
                "a renderer claimed the {listed} at place {at}, which has gone since it was \
                 drawn, {refused}"
            ),
            Refusal::NotDrawnThere { part, claim } => write!(
                f,
                "{part}'s renderer claimed {claim}, which {part} does not draw, {refused}"
            ),
            Refusal::NoCard { claim } => write!(
                f,
                "a renderer claimed {claim}, and the first screen shows no page as a card, \
                 {refused}"
            ),
            Refusal::PanelNotSo { claim, hidden } => write!(
                f,
                "a renderer claimed {claim}, and the chats panel is {}, {refused}",
                if *hidden {
                    "hidden to its rail"
                } else {
                    "shown whole"
                }
            ),
            Refusal::Unasked => write!(
                f,
                "a renderer claimed a part of the conversation with no press behind it, {refused}"
            ),
            Refusal::NotAClaim { part } => write!(
                f,
                "{part}'s renderer answered a press with something that is not a claim, {refused}"
            ),
        }
    }
}

/// What `part`'s renderer's `answer` to a press comes to, when that part,
/// as last painted, showed `painted`, and the browser process holds `now`
/// — what it would show if it were painted again.
pub fn decide<C, S>(
    answer: FromRenderer,
    part: Part,
    painted: &Shown<C, S>,
    now: &Shown<C, S>,
) -> Decided<C, S>
where
    C: Clone + PartialEq,
    S: Clone + PartialEq,
{
    let claim = match answer {
        FromRenderer::ConversationClaimed(claim) => claim,
        FromRenderer::Failed(failure) => return Decided::Failed(failure),
        _ => return Decided::Refused(Refusal::NotAClaim { part }),
    };
    let Some(claim) = claim else {
        return Decided::Nothing;
    };
    if claim.part() != part {
        return Decided::Refused(Refusal::NotDrawnThere { part, claim });
    }
    if let Some(refusal) = drawn_as_hidden(claim, painted.hidden, now.hidden) {
        return Decided::Refused(refusal);
    }
    match claim {
        Claim::NewChat => Decided::NewChat,
        Claim::Settings => Decided::Settings,
        Claim::SearchChats => Decided::SearchChats,
        Claim::HideThePanel => Decided::HideThePanel,
        Claim::ShowThePanel => Decided::ShowThePanel,
        Claim::AddASite => Decided::AddASite,
        Claim::TheBox => Decided::TheBox,
        Claim::Go => Decided::Go,
        Claim::Back | Claim::Forward | Claim::Expand | Claim::CloseTheCard
            if !(painted.card && now.card) =>
        {
            Decided::Refused(Refusal::NoCard { claim })
        }
        Claim::Back => Decided::Back,
        Claim::Forward => Decided::Forward,
        Claim::Expand => Decided::Expand,
        Claim::CloseTheCard => Decided::CloseTheCard,
        Claim::Chat { at } => held(Listed::Chats, at, &painted.chats, &now.chats)
            .map_or_else(Decided::Refused, Decided::Chat),
        Claim::Site { at } => held(Listed::Sites, at, &painted.sites, &now.sites)
            .map_or_else(Decided::Refused, Decided::Site),
        Claim::Waiting { at } => held(Listed::Waiting, at, &painted.waiting, &now.waiting)
            .map_or_else(Decided::Refused, Decided::Waiting),
        Claim::Found { at } => held(Listed::Found, at, &painted.found, &now.found)
            .map_or_else(Decided::Refused, Decided::Found),
        Claim::Offer { at } => held(Listed::Offers, at, &painted.offers, &now.offers)
            .map_or_else(Decided::Refused, Decided::Offer),
    }
}

/// Why `claim` is refused for the panel being hidden, or not, when it was
/// painted (`painted`) or now (`now`) — the rail draws *New chat* and *Show
/// the panel*, the whole panel everything else of it — or [`None`] when it
/// is drawn both ways or was drawn as it is and still is. A chat is refused
/// by its place instead, since the rail is shown none.
fn drawn_as_hidden(claim: Claim, painted: bool, now: bool) -> Option<Refusal> {
    let on_the_rail = match claim {
        Claim::ShowThePanel => true,
        Claim::SearchChats | Claim::HideThePanel | Claim::Settings => false,
        _ => return None,
    };
    [painted, now]
        .into_iter()
        .find(|&hidden| hidden != on_the_rail)
        .map(|hidden| Refusal::PanelNotSo { claim, hidden })
}

/// What was at place `at` of `painted`, if it is still in `now`.
fn held<T: Clone + PartialEq>(
    listed: Listed,
    at: usize,
    painted: &[T],
    now: &[T],
) -> Result<T, Refusal> {
    let shown = painted.get(at).ok_or(Refusal::NotShown {
        listed,
        at,
        shown: painted.len(),
    })?;
    if now.contains(shown) {
        Ok(shown.clone())
    } else {
        Err(Refusal::Gone { listed, at })
    }
}

/// The refusal for an answer that claims a part of the conversation when
/// no press was sent: any [`FromRenderer::ConversationClaimed`] that is not
/// the answer [`decide`] is given. [`None`] for every other answer.
pub fn unasked(answer: &FromRenderer) -> Option<Refusal> {
    matches!(answer, FromRenderer::ConversationClaimed(_)).then_some(Refusal::Unasked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three chats in the panel, two sites and one waiting chat on the
    /// first screen, as a browser process might name them.
    fn painted() -> Shown<u32, &'static str> {
        Shown {
            chats: vec![10, 11, 12],
            sites: vec!["mail", "bank"],
            waiting: vec![12],
            card: false,
            found: Vec::new(),
            offers: Vec::new(),
            hidden: false,
        }
    }

    #[test]
    fn an_offer_is_decided_by_its_place_among_those_painted_and_refused_past_them_or_gone() {
        let typed = crate::typed::offers("example.com");
        let with = Shown {
            offers: typed.clone(),
            ..painted()
        };
        for (at, offer) in typed.iter().enumerate() {
            assert_eq!(
                decide(claimed(Claim::Offer { at }), Part::Start, &with, &with),
                Decided::Offer(offer.clone()),
                "{at}"
            );
        }
        assert_eq!(
            decide(claimed(Claim::Offer { at: 3 }), Part::Start, &with, &with),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Offers,
                at: 3,
                shown: 3
            })
        );
        // Typed on since it was painted: what was offered there has gone,
        // and the offer now at its place is not what was pressed.
        let on = Shown {
            offers: crate::typed::offers("example.co"),
            ..painted()
        };
        assert_eq!(
            decide(claimed(Claim::Offer { at: 0 }), Part::Start, &with, &on),
            Decided::Refused(Refusal::Gone {
                listed: Listed::Offers,
                at: 0
            })
        );
        // Nothing offered when painted: no offer was shown to press.
        assert_eq!(
            decide(
                claimed(Claim::Offer { at: 0 }),
                Part::Start,
                &painted(),
                &with
            ),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Offers,
                at: 0,
                shown: 0
            })
        );
        assert_eq!(
            decide(claimed(Claim::Offer { at: 0 }), Part::Panel, &with, &with),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::Offer { at: 0 }
            })
        );
        assert_eq!(
            Refusal::NotShown {
                listed: Listed::Offers,
                at: 5,
                shown: 2
            }
            .to_string(),
            "a renderer claimed the offer at place 5, and 2 were shown, and nothing was done"
        );
    }

    #[test]
    fn the_panels_buttons_are_decided_only_as_it_was_painted_and_is_held() {
        let hidden = Shown {
            hidden: true,
            ..painted()
        };
        let shown = painted();
        for (claim, as_it) in [
            (Claim::SearchChats, Decided::SearchChats),
            (Claim::HideThePanel, Decided::HideThePanel),
            (Claim::Settings, Decided::Settings),
        ] {
            assert_eq!(decide(claimed(claim), Part::Panel, &shown, &shown), as_it);
            for (painted, now, which) in [(&hidden, &shown, true), (&shown, &hidden, true)] {
                assert_eq!(
                    decide(claimed(claim), Part::Panel, painted, now),
                    Decided::Refused(Refusal::PanelNotSo {
                        claim,
                        hidden: which
                    })
                );
            }
        }
        assert_eq!(
            decide(claimed(Claim::ShowThePanel), Part::Panel, &hidden, &hidden),
            Decided::ShowThePanel
        );
        for (painted, now) in [(&shown, &hidden), (&hidden, &shown)] {
            assert_eq!(
                decide(claimed(Claim::ShowThePanel), Part::Panel, painted, now),
                Decided::Refused(Refusal::PanelNotSo {
                    claim: Claim::ShowThePanel,
                    hidden: false
                })
            );
        }
        // New chat is drawn either way.
        for (painted, now) in [(&shown, &hidden), (&hidden, &hidden)] {
            assert_eq!(
                decide(claimed(Claim::NewChat), Part::Panel, painted, now),
                Decided::NewChat
            );
        }
        // And none of them from the first screen.
        assert_eq!(
            decide(claimed(Claim::HideThePanel), Part::Start, &shown, &shown),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Start,
                claim: Claim::HideThePanel
            })
        );
        assert_eq!(
            Refusal::PanelNotSo {
                claim: Claim::SearchChats,
                hidden: true
            }
            .to_string(),
            "a renderer claimed Search chats, and the chats panel is hidden to its rail, and \
             nothing was done"
        );
    }

    #[test]
    fn a_site_found_is_decided_by_its_place_and_refused_past_it_or_gone() {
        let at = |address: &str| alo_url::parse(address).unwrap();
        let with = Shown {
            found: vec![at("https://one.example/"), at("https://two.example/")],
            ..painted()
        };
        assert_eq!(
            decide(claimed(Claim::Found { at: 1 }), Part::Start, &with, &with),
            Decided::Found(at("https://two.example/"))
        );
        assert_eq!(
            decide(claimed(Claim::Found { at: 2 }), Part::Start, &with, &with),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Found,
                at: 2,
                shown: 2
            })
        );
        // Another search since: what was at the place has gone.
        assert_eq!(
            decide(
                claimed(Claim::Found { at: 0 }),
                Part::Start,
                &with,
                &painted()
            ),
            Decided::Refused(Refusal::Gone {
                listed: Listed::Found,
                at: 0
            })
        );
        assert_eq!(
            decide(claimed(Claim::Found { at: 0 }), Part::Panel, &with, &with),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::Found { at: 0 }
            })
        );
    }

    fn claimed(claim: Claim) -> FromRenderer {
        FromRenderer::ConversationClaimed(Some(claim))
    }

    fn decided(claim: Claim, part: Part) -> Decided<u32, &'static str> {
        decide(claimed(claim), part, &painted(), &painted())
    }

    #[test]
    fn a_cards_buttons_are_decided_only_while_a_card_was_painted_and_is_held() {
        let with = Shown {
            card: true,
            ..painted()
        };
        for (claim, as_it) in [
            (Claim::Back, Decided::Back),
            (Claim::Forward, Decided::Forward),
            (Claim::Expand, Decided::Expand),
            (Claim::CloseTheCard, Decided::CloseTheCard),
        ] {
            assert_eq!(decide(claimed(claim), Part::Start, &with, &with), as_it);
            let refused = Decided::Refused(Refusal::NoCard { claim });
            // Never painted, or closed since.
            assert_eq!(
                decide(claimed(claim), Part::Start, &painted(), &with),
                refused
            );
            assert_eq!(
                decide(claimed(claim), Part::Start, &with, &painted()),
                refused
            );
            // And never from the panel, which draws no card.
            assert_eq!(
                decide(claimed(claim), Part::Panel, &with, &with),
                Decided::Refused(Refusal::NotDrawnThere {
                    part: Part::Panel,
                    claim
                })
            );
        }
        assert_eq!(
            Refusal::NoCard {
                claim: Claim::CloseTheCard
            }
            .to_string(),
            "a renderer claimed the card's Close, and the first screen shows no page as a \
             card, and nothing was done"
        );
    }

    #[test]
    fn a_claim_on_what_was_shown_and_is_held_is_decided_as_it() {
        assert_eq!(decided(Claim::NewChat, Part::Panel), Decided::NewChat);
        assert_eq!(
            decided(Claim::Chat { at: 0 }, Part::Panel),
            Decided::Chat(10)
        );
        assert_eq!(
            decided(Claim::Chat { at: 2 }, Part::Panel),
            Decided::Chat(12)
        );
        assert_eq!(decided(Claim::Settings, Part::Panel), Decided::Settings);
        assert_eq!(
            decided(Claim::Site { at: 1 }, Part::Start),
            Decided::Site("bank")
        );
        assert_eq!(decided(Claim::AddASite, Part::Start), Decided::AddASite);
        assert_eq!(
            decided(Claim::Waiting { at: 0 }, Part::Start),
            Decided::Waiting(12)
        );
        assert_eq!(decided(Claim::TheBox, Part::Start), Decided::TheBox);
        assert_eq!(decided(Claim::Go, Part::Start), Decided::Go);
    }

    #[test]
    fn a_press_on_nothing_does_nothing() {
        for part in [Part::Panel, Part::Start] {
            assert_eq!(
                decide(
                    FromRenderer::ConversationClaimed(None),
                    part,
                    &painted(),
                    &painted()
                ),
                Decided::Nothing
            );
        }
    }

    #[test]
    fn a_place_past_what_was_shown_is_refused_by_name() {
        for at in [3, 4, usize::MAX] {
            assert_eq!(
                decided(Claim::Chat { at }, Part::Panel),
                Decided::Refused(Refusal::NotShown {
                    listed: Listed::Chats,
                    at,
                    shown: 3
                }),
                "{at}"
            );
        }
        assert_eq!(
            decided(Claim::Site { at: 2 }, Part::Start),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Sites,
                at: 2,
                shown: 2
            })
        );
        assert_eq!(
            decided(Claim::Waiting { at: 1 }, Part::Start),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Waiting,
                at: 1,
                shown: 1
            })
        );
        assert_eq!(
            Refusal::NotShown {
                listed: Listed::Chats,
                at: 7,
                shown: 3
            }
            .to_string(),
            "a renderer claimed the chat at place 7, and 3 were shown, and nothing was done"
        );
    }

    #[test]
    fn a_chat_gone_since_it_was_drawn_is_refused_and_not_moved_along() {
        // Chat 11 has gone; 12 moved up into its place.
        let now = Shown {
            chats: vec![10, 12],
            ..painted()
        };
        assert_eq!(
            decide(
                claimed(Claim::Chat { at: 1 }),
                Part::Panel,
                &painted(),
                &now
            ),
            Decided::Refused(Refusal::Gone {
                listed: Listed::Chats,
                at: 1
            })
        );
        assert_eq!(
            decide(
                claimed(Claim::Chat { at: 2 }),
                Part::Panel,
                &painted(),
                &now
            ),
            Decided::Chat(12)
        );
        assert!(
            Refusal::Gone {
                listed: Listed::Chats,
                at: 1
            }
            .to_string()
            .contains("has gone since it was drawn")
        );
    }

    #[test]
    fn a_claim_on_what_the_part_does_not_draw_is_refused_by_name() {
        assert_eq!(
            decided(Claim::TheBox, Part::Panel),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::TheBox
            })
        );
        assert_eq!(
            decided(Claim::Chat { at: 0 }, Part::Start),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Start,
                claim: Claim::Chat { at: 0 }
            })
        );
        assert_eq!(
            Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::Go
            }
            .to_string(),
            "the chats panel's renderer claimed Go, which the chats panel does not draw, and \
             nothing was done"
        );
    }

    #[test]
    fn a_press_answered_with_anything_else_is_not_a_claim() {
        assert_eq!(
            decide::<u32, &str>(
                FromRenderer::Failed(Failure::NotTheConversation),
                Part::Panel,
                &painted(),
                &painted()
            ),
            Decided::Failed(Failure::NotTheConversation)
        );
        assert_eq!(
            decide::<u32, &str>(
                FromRenderer::Claimed(None),
                Part::Start,
                &painted(),
                &painted()
            ),
            Decided::Refused(Refusal::NotAClaim { part: Part::Start })
        );
    }

    #[test]
    fn a_claim_answering_no_press_is_refused_by_name() {
        assert_eq!(unasked(&claimed(Claim::NewChat)), Some(Refusal::Unasked));
        assert_eq!(
            unasked(&FromRenderer::ConversationClaimed(None)),
            Some(Refusal::Unasked)
        );
        assert_eq!(unasked(&FromRenderer::Claimed(None)), None);
        assert_eq!(
            Refusal::Unasked.to_string(),
            "a renderer claimed a part of the conversation with no press behind it, and \
             nothing was done"
        );
    }
}

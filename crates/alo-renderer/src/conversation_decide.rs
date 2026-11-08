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

use crate::agent_limits::{Asks, Stepped};
use crate::card_menu::MenuItem;
use crate::conversation_claim::{Claim, Part};
use crate::interface_face::InterfaceFace;
use crate::message::{Failure, FromRenderer};
use crate::results::Tab;
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
    /// Every open tab of the search results drawn, in order (queue item
    /// 744): the chat, the turn's place in it, and the tab.
    pub tabs: Vec<(C, usize, Tab)>,
    /// Every *More sites ›* drawn, in order: the chat and the turn's place.
    pub more: Vec<(C, usize)>,
    /// Every *See all images* drawn, in order (queue item 743).
    pub see_all: Vec<(C, usize)>,
    /// Every *Show more sites* that can be pressed, in order (queue item
    /// 761): the chat and the turn's place.
    pub show_more: Vec<(C, usize)>,
    /// The pages every image drawn was found on, in order.
    pub pictured: Vec<Url>,
    /// Every *Full answer ›* drawn, in order (queue item 707): the chat and
    /// the turn's place.
    pub full_answer: Vec<(C, usize)>,
    /// The run of the agent at work on the card's page, while it works and
    /// *Stop* is drawn (queue item 704), by the browser process's number.
    pub working: Option<u64>,
    /// The run of the agent held with the page the person's, while *Let alo
    /// continue* is drawn (queue item 825), by the browser process's number.
    pub paused: Option<u64>,
    /// The question the agent is asking the person, by the browser
    /// process's number, while *Allow* and *Not now* are drawn under it.
    pub asking: Option<u64>,
    /// The choices a stuck agent offers, in order.
    pub choices: Vec<String>,
    /// The chat whose row offers *Delete*, the pointer over it (queue item
    /// 789): an earlier chat's.
    pub hovered: Option<C>,
    /// The chat whose row offers its ×, the pointer over it (queue item
    /// 791): an open chat's.
    pub closing: Option<C>,
    /// The chat just deleted, while the panel offers *Undo*.
    pub undo: Option<C>,
    /// The site of the live card's page while its shield is drawn (queue
    /// item 807).
    pub shield: Option<String>,
    /// The lines of alo's menu over the card's page, in order, while it is
    /// drawn (queue item 798).
    pub menu: Vec<MenuItem>,
    /// The faces *Settings* offers, in order, while it is drawn in place of
    /// the chat on screen (queue item 854); none while it is not.
    pub faces: Vec<InterfaceFace>,
    /// The stepper's buttons *alo at work* lets a person press, in order,
    /// while *Settings* is drawn (queue item 878).
    pub limits: Vec<Stepped>,
    /// The choices of asking it offers, in order, while it is drawn.
    pub asks: Vec<Asks>,
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
            tabs: Vec::new(),
            more: Vec::new(),
            see_all: Vec::new(),
            show_more: Vec::new(),
            pictured: Vec::new(),
            full_answer: Vec::new(),
            working: None,
            paused: None,
            asking: None,
            choices: Vec::new(),
            hovered: None,
            closing: None,
            undo: None,
            shield: None,
            menu: Vec::new(),
            faces: Vec::new(),
            limits: Vec::new(),
            asks: Vec::new(),
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
    /// This tab of the search results in this chat's turn at this place —
    /// pressed, or opened by *More sites ›* or *See all images* (queue
    /// items 743, 744).
    Tab {
        /// The chat.
        chat: C,
        /// The turn's place in it.
        turn: usize,
        /// The tab.
        tab: Tab,
    },
    /// An image a search found, on the page at this address (queue item
    /// 743): opened as a site's title is.
    Image(Url),
    /// *Show more sites* under the Sites tab of this chat's turn at this
    /// place (queue item 761): its next page asked.
    ShowMoreSites {
        /// The chat.
        chat: C,
        /// The turn's place in it.
        turn: usize,
    },
    /// The agent's *Stop*, for its run of this number (queue item 704).
    Stop(u64),
    /// The agent's *Let alo continue*, for its run of this number, held
    /// with the page the person's (queue item 825).
    Continue(u64),
    /// The agent's *Allow*, for its question of this number: the one the
    /// person was shown, and is still asked.
    Allow(u64),
    /// The agent's *Not now*, for its question of this number.
    NotNow(u64),
    /// The choice a stuck agent offered, in its words.
    Choice(String),
    /// *Delete* on this chat's row (queue item 789).
    DeleteChat(C),
    /// The × on this open chat's row, closing it to *Earlier* (queue item
    /// 791).
    CloseChat(C),
    /// *Undo*, bringing this chat back.
    Undo(C),
    /// The card's shield, for the page's site of this name (queue item
    /// 807).
    Shield(String),
    /// A line of alo's menu over the card's page (queue item 798).
    MenuItem(MenuItem),
    /// This face, chosen in *Settings* (queue item 854).
    Face(InterfaceFace),
    /// A stepper's button pressed in *alo at work* (queue item 878).
    Stepped(Stepped),
    /// This choice of asking, made in *alo at work*.
    Asks(Asks),
    /// *Settings*' *Back to the chat*.
    LeaveSettings,
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
    /// The open tabs of a search's results (queue item 744).
    Tabs,
    /// The *More sites ›* drawn.
    MoreSites,
    /// The *See all images* drawn (queue item 743).
    SeeAllImages,
    /// The *Show more sites* that can be pressed (queue item 761).
    ShowMoreSites,
    /// The images a search found.
    Images,
    /// The *Full answer ›* drawn (queue item 707).
    FullAnswer,
    /// The choices a stuck agent offered (queue item 704).
    Choices,
    /// The lines of alo's menu over the card's page (queue item 798).
    MenuItems,
    /// The faces *Settings* offers (queue item 854).
    Faces,
    /// The stepper's buttons *alo at work* offers (queue item 878).
    Limits,
    /// The choices of asking it offers.
    Asks,
}

impl fmt::Display for Listed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Listed::Chats => "chat",
            Listed::Sites => "site",
            Listed::Waiting => "waiting chat",
            Listed::Found => "site found",
            Listed::Offers => "offer",
            Listed::Tabs => "open tab",
            Listed::MoreSites => "More sites",
            Listed::SeeAllImages => "See all images",
            Listed::ShowMoreSites => "Show more sites",
            Listed::Images => "image found",
            Listed::FullAnswer => "Full answer",
            Listed::Choices => "agent's choice",
            Listed::MenuItems => "line of the page's menu",
            Listed::Faces => "face Settings offers",
            Listed::Limits => "stepper's button Settings offers",
            Listed::Asks => "choice of asking Settings offers",
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
    /// The renderer claimed the agent's *Stop*, *Allow* or *Not now* when
    /// the first screen did not draw it as painted, or would not now: no
    /// agent at work, no question asked, or another question than the one
    /// painted (queue item 704).
    NotAsked {
        /// What it claimed.
        claim: Claim,
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
            Refusal::NotAsked { claim } => write!(
                f,
                "a renderer claimed {claim}, which {} does not show as it was painted, or not \
                 still, {refused}",
                claim.part()
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
        Claim::Stop | Claim::Continue | Claim::Allow | Claim::NotNow | Claim::Choice { .. } => {
            agents(claim, painted, now)
        }
        Claim::MenuItem { at } => held(Listed::MenuItems, at, &painted.menu, &now.menu)
            .map_or_else(Decided::Refused, Decided::MenuItem),
        Claim::Face { .. } | Claim::LeaveSettings | Claim::Limit { .. } | Claim::Asks { .. } => {
            in_settings(claim, painted, now)
        }
        Claim::Expand => Decided::Expand,
        Claim::CloseTheCard => Decided::CloseTheCard,
        Claim::DeleteChat | Claim::CloseChat | Claim::Undo | Claim::Shield => {
            offered(claim, painted, now)
        }
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
        Claim::Tab { at } => held(Listed::Tabs, at, &painted.tabs, &now.tabs).map_or_else(
            Decided::Refused,
            |(chat, turn, tab)| Decided::Tab { chat, turn, tab },
        ),
        Claim::MoreSites { at } => held(Listed::MoreSites, at, &painted.more, &now.more)
            .map_or_else(Decided::Refused, |(chat, turn)| Decided::Tab {
                chat,
                turn,
                tab: Tab::Sites,
            }),
        Claim::SeeAllImages { at } => {
            held(Listed::SeeAllImages, at, &painted.see_all, &now.see_all).map_or_else(
                Decided::Refused,
                |(chat, turn)| Decided::Tab {
                    chat,
                    turn,
                    tab: Tab::Images,
                },
            )
        }
        Claim::ShowMoreSites { at } => held(
            Listed::ShowMoreSites,
            at,
            &painted.show_more,
            &now.show_more,
        )
        .map_or_else(Decided::Refused, |(chat, turn)| Decided::ShowMoreSites {
            chat,
            turn,
        }),
        Claim::Image { at } => held(Listed::Images, at, &painted.pictured, &now.pictured)
            .map_or_else(Decided::Refused, Decided::Image),
        Claim::FullAnswer { at } => held(
            Listed::FullAnswer,
            at,
            &painted.full_answer,
            &now.full_answer,
        )
        .map_or_else(Decided::Refused, |(chat, turn)| Decided::Tab {
            chat,
            turn,
            tab: Tab::Answer,
        }),
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

/// What a press on *Settings* comes to (queue item 854): a face it offered
/// as painted and still does; *Back to the chat* only while it was drawn and
/// still is.
fn in_settings<C, S>(claim: Claim, painted: &Shown<C, S>, now: &Shown<C, S>) -> Decided<C, S> {
    match claim {
        Claim::Face { at } => held(Listed::Faces, at, &painted.faces, &now.faces)
            .map_or_else(Decided::Refused, Decided::Face),
        // *alo at work* (queue item 878): the button painted at that place,
        // still offered — one at its number's end is not.
        Claim::Limit { at } => held(Listed::Limits, at, &painted.limits, &now.limits)
            .map_or_else(Decided::Refused, Decided::Stepped),
        Claim::Asks { at } => held(Listed::Asks, at, &painted.asks, &now.asks)
            .map_or_else(Decided::Refused, Decided::Asks),
        _ if painted.faces.is_empty() || now.faces.is_empty() => {
            Decided::Refused(Refusal::NotAsked { claim })
        }
        _ => Decided::LeaveSettings,
    }
}

/// What a thing offered for one chat or site comes to: the panel's *Delete*
/// deletes the one chat whose row was painted offering it and still does,
/// and its × closes the one open chat painted offering it (queue item 791),
/// and *Undo* brings back the one the panel said while it still says so
/// (queue item 789); the card's shield switches the one site it was painted
/// for, while the card still shows that site's page (queue item 807).
fn offered<C, S>(claim: Claim, painted: &Shown<C, S>, now: &Shown<C, S>) -> Decided<C, S>
where
    C: Clone + PartialEq,
{
    let refused = Decided::Refused(Refusal::NotAsked { claim });
    match claim {
        Claim::Undo => {
            same_one(painted.undo.as_ref(), now.undo.as_ref()).map_or(refused, Decided::Undo)
        }
        Claim::Shield => {
            same_one(painted.shield.as_ref(), now.shield.as_ref()).map_or(refused, Decided::Shield)
        }
        Claim::CloseChat => same_one(painted.closing.as_ref(), now.closing.as_ref())
            .map_or(refused, Decided::CloseChat),
        _ => same_one(painted.hovered.as_ref(), now.hovered.as_ref())
            .map_or(refused, Decided::DeleteChat),
    }
}

/// What was painted, if it is what is held now too.
fn same_one<T: Clone + PartialEq>(painted: Option<&T>, now: Option<&T>) -> Option<T> {
    painted.filter(|&shown| now == Some(shown)).cloned()
}

/// A press on one of the agent's buttons (queue items 704, 825): each only
/// for what was painted and is still so — *Allow* allows the one question
/// the person was shown, and none asked since; *Let alo continue* carries
/// on only the run the picture showed held.
fn agents<C, S>(claim: Claim, painted: &Shown<C, S>, now: &Shown<C, S>) -> Decided<C, S>
where
    C: Clone + PartialEq,
    S: Clone + PartialEq,
{
    let not_asked = || Decided::Refused(Refusal::NotAsked { claim });
    match claim {
        Claim::Stop => same(painted.working, now.working).map_or_else(not_asked, Decided::Stop),
        Claim::Continue => {
            same(painted.paused, now.paused).map_or_else(not_asked, Decided::Continue)
        }
        Claim::Allow => same(painted.asking, now.asking).map_or_else(not_asked, Decided::Allow),
        Claim::NotNow => same(painted.asking, now.asking).map_or_else(not_asked, Decided::NotNow),
        Claim::Choice { at } => held(Listed::Choices, at, &painted.choices, &now.choices)
            .map_or_else(Decided::Refused, Decided::Choice),
        other => Decided::Refused(Refusal::NotAsked { claim: other }),
    }
}

/// The number painted, if it is the one now too.
fn same(painted: Option<u64>, now: Option<u64>) -> Option<u64> {
    painted.filter(|&number| now == Some(number))
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
            ..Shown::default()
        }
    }

    #[test]
    fn show_more_sites_is_decided_by_its_place_and_refused_while_asked() {
        // *Show more sites* (761): by its place, and refused once its page
        // is being asked — no longer among those that can be pressed.
        let showing = Shown {
            show_more: vec![(10, 0), (11, 4)],
            ..painted()
        };
        assert_eq!(
            decide(
                claimed(Claim::ShowMoreSites { at: 1 }),
                Part::Start,
                &showing,
                &showing
            ),
            Decided::ShowMoreSites { chat: 11, turn: 4 }
        );
        let asking = Shown {
            show_more: vec![(11, 4)],
            ..painted()
        };
        assert_eq!(
            decide(
                claimed(Claim::ShowMoreSites { at: 0 }),
                Part::Start,
                &showing,
                &asking
            ),
            Decided::Refused(Refusal::Gone {
                listed: Listed::ShowMoreSites,
                at: 0
            })
        );
    }

    #[test]
    fn a_tab_more_sites_see_all_and_an_image_are_decided_by_their_place() {
        let image = alo_url::parse("https://pictures.example/a").unwrap();
        let with = Shown {
            tabs: vec![(10, 0, Tab::All), (10, 0, Tab::Sites), (10, 2, Tab::All)],
            more: vec![(10, 0)],
            see_all: vec![(10, 2)],
            pictured: vec![image.clone()],
            ..painted()
        };
        let decided = |claim| decide(claimed(claim), Part::Start, &with, &with);
        assert_eq!(
            decided(Claim::Tab { at: 1 }),
            Decided::Tab {
                chat: 10,
                turn: 0,
                tab: Tab::Sites
            }
        );
        assert_eq!(
            decided(Claim::MoreSites { at: 0 }),
            Decided::Tab {
                chat: 10,
                turn: 0,
                tab: Tab::Sites
            }
        );
        assert_eq!(
            decided(Claim::SeeAllImages { at: 0 }),
            Decided::Tab {
                chat: 10,
                turn: 2,
                tab: Tab::Images
            }
        );
        assert_eq!(decided(Claim::Image { at: 0 }), Decided::Image(image));
        assert_eq!(
            decided(Claim::Tab { at: 3 }),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Tabs,
                at: 3,
                shown: 3
            })
        );
        assert_eq!(
            decided(Claim::Image { at: 1 }),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Images,
                at: 1,
                shown: 1
            })
        );
        // Another chat on screen since: the tab painted is not there now.
        let now = Shown {
            tabs: vec![(11, 0, Tab::All)],
            ..painted()
        };
        assert_eq!(
            decide(claimed(Claim::Tab { at: 0 }), Part::Start, &with, &now),
            Decided::Refused(Refusal::Gone {
                listed: Listed::Tabs,
                at: 0
            })
        );
        assert_eq!(
            decide(
                claimed(Claim::MoreSites { at: 0 }),
                Part::Panel,
                &with,
                &with
            ),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::MoreSites { at: 0 }
            })
        );
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
    fn the_shield_switches_only_the_site_it_was_painted_for_while_it_shows_it() {
        let shown = |site: Option<&str>| Shown {
            shield: site.map(str::to_owned),
            ..painted()
        };
        let guardian = shown(Some("theguardian.com"));
        let shield = claimed(Claim::Shield);
        assert_eq!(
            decide(shield.clone(), Part::Start, &guardian, &guardian),
            Decided::Shield("theguardian.com".to_owned())
        );
        for (painted, now) in [
            (&guardian, &shown(Some("bbc.com"))),
            (&guardian, &shown(None)),
            (&shown(None), &guardian),
        ] {
            assert_eq!(
                decide(shield.clone(), Part::Start, painted, now),
                Decided::Refused(Refusal::NotAsked {
                    claim: Claim::Shield
                })
            );
        }
        assert_eq!(
            decide(shield, Part::Panel, &guardian, &guardian),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::Shield
            })
        );
    }

    #[test]
    fn a_close_is_decided_only_for_the_open_chat_painted_offering_it_and_still_so() {
        let offering = Shown {
            closing: Some(7),
            hovered: Some(9),
            ..painted()
        };
        let close = claimed(Claim::CloseChat);
        assert_eq!(
            decide(close.clone(), Part::Panel, &offering, &offering),
            Decided::CloseChat(7)
        );
        for (painted, now) in [
            (
                &offering,
                &Shown {
                    closing: Some(8),
                    ..painted()
                },
            ),
            (&offering, &painted()),
            (&painted(), &offering),
        ] {
            assert_eq!(
                decide(close.clone(), Part::Panel, painted, now),
                Decided::Refused(Refusal::NotAsked {
                    claim: Claim::CloseChat
                })
            );
        }
        assert_eq!(
            decide(close, Part::Start, &offering, &offering),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Start,
                claim: Claim::CloseChat
            })
        );
    }

    #[test]
    fn delete_and_undo_are_decided_only_for_the_chat_painted_and_still_so() {
        let offering = Shown {
            hovered: Some(7),
            undo: Some(4),
            ..painted()
        };
        let delete = claimed(Claim::DeleteChat);
        let undo = claimed(Claim::Undo);
        assert_eq!(
            decide(delete.clone(), Part::Panel, &offering, &offering),
            Decided::DeleteChat(7)
        );
        assert_eq!(
            decide(undo.clone(), Part::Panel, &offering, &offering),
            Decided::Undo(4)
        );
        // Another row hovered since, or none; the Undo passed since, or
        // never painted: refused, never moved along.
        for now in [
            Shown {
                hovered: Some(8),
                undo: Some(5),
                ..painted()
            },
            painted(),
        ] {
            for (claim, answer) in [(Claim::DeleteChat, &delete), (Claim::Undo, &undo)] {
                assert_eq!(
                    decide(answer.clone(), Part::Panel, &offering, &now),
                    Decided::Refused(Refusal::NotAsked { claim })
                );
                assert_eq!(
                    decide(answer.clone(), Part::Panel, &now, &offering),
                    Decided::Refused(Refusal::NotAsked { claim })
                );
            }
        }
        // Claimed by the first screen's renderer: it draws neither.
        assert_eq!(
            decide(delete, Part::Start, &offering, &offering),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Start,
                claim: Claim::DeleteChat
            })
        );
        assert_eq!(
            Refusal::NotAsked { claim: Claim::Undo }.to_string(),
            "a renderer claimed the panel's Undo, which the chats panel does not show as it \
             was painted, or not still, and nothing was done"
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

    /// Queue item 704: the agent's buttons are decided only for what was
    /// painted and still is — *Allow* for the question the person was
    /// shown, never one asked since, nor once it is answered.
    #[test]
    fn the_agents_buttons_are_decided_only_for_the_question_painted_and_still_asked() {
        let asking = |working, asking| Shown {
            working,
            asking,
            ..painted()
        };
        let shown = asking(Some(3), Some(7));
        for (claim, as_it) in [
            (Claim::Stop, Decided::Stop(3)),
            (Claim::Allow, Decided::Allow(7)),
            (Claim::NotNow, Decided::NotNow(7)),
        ] {
            assert_eq!(decide(claimed(claim), Part::Start, &shown, &shown), as_it);
            assert_eq!(
                decide(claimed(claim), Part::Panel, &shown, &shown),
                Decided::Refused(Refusal::NotDrawnThere {
                    part: Part::Panel,
                    claim
                })
            );
        }
        // Another question asked since it was painted, none asked any more,
        // or none painted: refused, and nothing allowed.
        for (painted_as, now) in [
            (asking(Some(3), Some(7)), asking(Some(3), Some(8))),
            (asking(Some(3), Some(7)), asking(Some(3), None)),
            (asking(Some(3), None), asking(Some(3), Some(7))),
        ] {
            for claim in [Claim::Allow, Claim::NotNow] {
                assert_eq!(
                    decide(claimed(claim), Part::Start, &painted_as, &now),
                    Decided::Refused(Refusal::NotAsked { claim })
                );
            }
        }
        // Stop, once the run painted has ended or another is at work.
        for now in [asking(None, None), asking(Some(4), None)] {
            assert_eq!(
                decide(claimed(Claim::Stop), Part::Start, &shown, &now),
                Decided::Refused(Refusal::NotAsked { claim: Claim::Stop })
            );
        }
        // A choice by its place among those painted, still offered.
        let stuck = Shown {
            choices: vec!["Try again".to_owned(), "Stop here".to_owned()],
            ..painted()
        };
        assert_eq!(
            decide(
                claimed(Claim::Choice { at: 1 }),
                Part::Start,
                &stuck,
                &stuck
            ),
            Decided::Choice("Stop here".to_owned())
        );
        assert_eq!(
            decide(
                claimed(Claim::Choice { at: 0 }),
                Part::Start,
                &stuck,
                &painted()
            ),
            Decided::Refused(Refusal::Gone {
                listed: Listed::Choices,
                at: 0
            })
        );
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

    /// Queue item 854: a face *Settings* offered as painted, and still
    /// offers, is decided as itself; one past them, or once *Settings* has
    /// closed, is refused; *Back to the chat* only while it was drawn and
    /// still is.
    #[test]
    fn a_face_settings_offered_is_decided_and_back_only_while_it_stands() {
        let open = Shown {
            faces: InterfaceFace::ALL.to_vec(),
            ..painted()
        };
        let closed = painted();
        let face = |at| claimed(Claim::Face { at });
        assert_eq!(
            decide(face(2), Part::Start, &open, &open),
            Decided::Face(InterfaceFace::DmSans)
        );
        assert_eq!(
            decide(face(5), Part::Start, &open, &open),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Faces,
                at: 5,
                shown: 5
            })
        );
        assert_eq!(
            decide(face(0), Part::Start, &closed, &open),
            Decided::Refused(Refusal::NotShown {
                listed: Listed::Faces,
                at: 0,
                shown: 0
            }),
            "not painted"
        );
        assert_eq!(
            decide(face(1), Part::Start, &open, &closed),
            Decided::Refused(Refusal::Gone {
                listed: Listed::Faces,
                at: 1
            }),
            "closed since"
        );
        let back = claimed(Claim::LeaveSettings);
        assert_eq!(
            decide(back.clone(), Part::Start, &open, &open),
            Decided::LeaveSettings
        );
        for (painted, now) in [(&closed, &open), (&open, &closed)] {
            assert_eq!(
                decide(back.clone(), Part::Start, painted, now),
                Decided::Refused(Refusal::NotAsked {
                    claim: Claim::LeaveSettings
                })
            );
        }
        // Neither is the panel's to claim.
        assert_eq!(
            decide(face(0), Part::Panel, &open, &open),
            Decided::Refused(Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::Face { at: 0 }
            })
        );
    }
}

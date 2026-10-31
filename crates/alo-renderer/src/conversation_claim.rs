/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's press on the chats panel or the first screen claims (ADR
//! 0074, ADR 0024 § 5, queue item 710).
//!
//! The two parts of the conversation are drawn as the tab strip is, and
//! pressed as it is: the browser process decides from its own composition
//! which part a point is over and sends it, in that part's CSS pixels, to
//! the renderer that drew the part; the renderer answers with **a claim** —
//! *the chat at place 2*, *New chat*, *the box* — found from the drawing the
//! person sees, as [`crate::strip_claim`] finds a tab. This is the claim,
//! and how it is found; [`crate::conversation_decide`] is the deciding.
//!
//! # Everything is named by its place or its kind
//!
//! A claim carries no string from the document. A chat is *the chat at this
//! place in the panel you sent me*, counting in the order the panel draws
//! them ([`crate::panel::drawn_order`]); a kept site is the tile at its place
//! among the sites, and a waiting chat its place among those waiting. The
//! rest — *New chat*, *Settings*, *Add a site*, the box, *Go* — are each one
//! of a closed list. Which chat or site a place was is the browser
//! process's to say, from what it sent, so a renderer that lies can name
//! only a place or a kind, and the worst it can do is choose among what the
//! person was shown, at the moment they pressed.
//!
//! # What a press is on
//!
//! The element under the point, found as a page's is ([`crate::hit`]), and
//! the nearest element from it up that the part's own template or its
//! document builder made a pressable thing of, known by its class — which
//! only those set, since a stranger's string enters either document as a
//! text node and nowhere else ([`crate::panel_document`],
//! [`crate::start_document`]). So a press on a chat's name, its site's mark
//! or the dot that says the agent is at work is on the chat; a press on the
//! field or the box around it is on the box, and on the arrow is on *Go*. A
//! press anywhere else — the mark, a group's heading, the gap between two
//! rows, the room below them — is on nothing, and claims nothing.
//!
//! A page shown as a card (queue item 717) is pressed the same way: its
//! *Back*, *Forward*, *Expand to the whole window* and *Close*, each one of
//! the closed list, and a greyed *Back* or *Forward* — nowhere to go — on
//! nothing. A press on the card's picture of the page claims nothing:
//! a person's pointer reaching the page is queue item 718's, and is never
//! a claim.
//!
//! A search's results (queue item 708) are pressed the same way: a site's
//! title is claimed by its place among the titles drawn, and nothing else
//! of them — its name, address, description, the tabs, *More sites ›* —
//! claims anything. A conversation of many turns (queue item 703) draws
//! every search's titles, and each page shown earlier as a link of the
//! same class, so each is claimed by its place among all of them, in the
//! order drawn, oldest turn first; what is scrolled out of the room is
//! clipped, and a press there finds nothing to claim.
//!
//! The offers for what was typed (queue items 701, 714), under the box or
//! above it in a conversation, are claimed by their place among the offers
//! drawn, whichever of them Return would choose and whether or not each is
//! set up: each is of a class `offer` with what marks it after, and is
//! counted among all of them. A box with the keyboard is drawn under a
//! `main` of class `start focused`; its own class stays `box`.
//!
//! The panel's *Search chats* — its field, its glass, the room around them
//! — and the button that hides the panel are each one of a kind (queue item
//! 711); so, once it is hidden, are the rail's *New chat* and *Show the
//! panel*. The rail draws no chat, and a press on its mark or between its
//! buttons is on nothing.

use crate::hit;
use crate::pipeline::Drawing;
use alo_dom::{Document, NodeId};
use alo_paint::Point;
use core::fmt;

/// Which part of the conversation a renderer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The chats panel, down the left.
    Panel,
    /// The first screen, beside it.
    Start,
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Part::Panel => "the chats panel",
            Part::Start => "the first screen",
        })
    }
}

/// What a press on the chats panel or the first screen means, as the
/// renderer that drew it claims it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// The panel's *New chat*.
    NewChat,
    /// The chat at this place in the panel, counting from zero in the order
    /// the panel draws them: the open ones, then the earlier ones.
    Chat {
        /// Its place.
        at: usize,
    },
    /// The panel's *Settings*.
    Settings,
    /// The kept site at this place among the first screen's sites, counting
    /// from zero; *Add a site* is not one of them.
    Site {
        /// Its place.
        at: usize,
    },
    /// The first screen's *Add a site*.
    AddASite,
    /// The chat waiting at this place on the first screen, counting from
    /// zero.
    Waiting {
        /// Its place.
        at: usize,
    },
    /// The first screen's one box: its field, or the box around it.
    TheBox,
    /// The box's *Go*.
    Go,
    /// The card's *Back*, when there is somewhere to go back to (queue item
    /// 717).
    Back,
    /// The card's *Forward*, when there is somewhere to go forward to.
    Forward,
    /// The card's *Expand to the whole window*.
    Expand,
    /// The card's *Close*.
    CloseTheCard,
    /// The title of the site at this place among a search's results, as
    /// the All tab draws them, counting from zero (queue item 708).
    Found {
        /// Its place.
        at: usize,
    },
    /// The offer at this place among those drawn for what was typed,
    /// counting from zero: the first is what Return chooses (queue item
    /// 714).
    Offer {
        /// Its place.
        at: usize,
    },
    /// The panel's *Search chats* (queue item 711).
    SearchChats,
    /// The panel's button that hides it, beside the mark.
    HideThePanel,
    /// The rail's button that shows the panel again.
    ShowThePanel,
}

impl Claim {
    /// The part that draws what this claims.
    pub fn part(self) -> Part {
        match self {
            Claim::NewChat
            | Claim::Chat { .. }
            | Claim::Settings
            | Claim::SearchChats
            | Claim::HideThePanel
            | Claim::ShowThePanel => Part::Panel,
            Claim::Site { .. }
            | Claim::AddASite
            | Claim::Waiting { .. }
            | Claim::TheBox
            | Claim::Go
            | Claim::Back
            | Claim::Forward
            | Claim::Expand
            | Claim::CloseTheCard
            | Claim::Found { .. }
            | Claim::Offer { .. } => Part::Start,
        }
    }
}

impl fmt::Display for Claim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Claim::NewChat => f.write_str("New chat"),
            Claim::Chat { at } => write!(f, "the chat at place {at}"),
            Claim::Settings => f.write_str("Settings"),
            Claim::Site { at } => write!(f, "the site at place {at}"),
            Claim::AddASite => f.write_str("Add a site"),
            Claim::Waiting { at } => write!(f, "the waiting chat at place {at}"),
            Claim::TheBox => f.write_str("the box"),
            Claim::Go => f.write_str("Go"),
            Claim::Back => f.write_str("the card's Back"),
            Claim::Forward => f.write_str("the card's Forward"),
            Claim::Expand => f.write_str("the card's Expand to the whole window"),
            Claim::CloseTheCard => f.write_str("the card's Close"),
            Claim::Found { at } => write!(f, "the title of the site found at place {at}"),
            Claim::Offer { at } => write!(f, "the offer at place {at}"),
            Claim::SearchChats => f.write_str("Search chats"),
            Claim::HideThePanel => f.write_str("the button that hides the panel"),
            Claim::ShowThePanel => f.write_str("the button that shows the panel"),
        }
    }
}

/// What a press is on, by the class its document gave it.
#[derive(Debug, Clone, Copy)]
enum Pressable {
    /// One of a kind: the claim is the same wherever it is.
    One(Claim),
    /// One of several of this class, claimed by its place among them.
    Placed(fn(usize) -> Claim),
}

/// What each part draws that a press can be on, by its class: each the
/// whole `class` attribute its template or document builder sets, and a
/// thing claimed by its place counted among every element of any of its
/// classes.
fn pressables(part: Part) -> &'static [(&'static [&'static str], Pressable)] {
    match part {
        Part::Panel => &[
            (&["new"], Pressable::One(Claim::NewChat)),
            (&["chat"], Pressable::Placed(|at| Claim::Chat { at })),
            (&["settings"], Pressable::One(Claim::Settings)),
            // Queue item 711: Search chats, the button beside the mark, and
            // the rail's two buttons once it is hidden.
            (&["find"], Pressable::One(Claim::SearchChats)),
            (&["hide"], Pressable::One(Claim::HideThePanel)),
            (&["rbtn new"], Pressable::One(Claim::NewChat)),
            (&["rbtn show"], Pressable::One(Claim::ShowThePanel)),
        ],
        Part::Start => &[
            (&["tile"], Pressable::Placed(|at| Claim::Site { at })),
            (&["tile add"], Pressable::One(Claim::AddASite)),
            (&["wait"], Pressable::Placed(|at| Claim::Waiting { at })),
            (&["go"], Pressable::One(Claim::Go)),
            (&["ask"], Pressable::One(Claim::TheBox)),
            (&["box"], Pressable::One(Claim::TheBox)),
            // A greyed Back or Forward is of the class `ibtn back off`, and
            // claims nothing (queue item 717).
            (&["ibtn back"], Pressable::One(Claim::Back)),
            (&["ibtn forward"], Pressable::One(Claim::Forward)),
            (&["ibtn expand"], Pressable::One(Claim::Expand)),
            (&["ibtn close"], Pressable::One(Claim::CloseTheCard)),
            // A site's title among a search's results (queue item 708).
            (&["title"], Pressable::Placed(|at| Claim::Found { at })),
            // An offer for what was typed, whichever is chosen and whether
            // it is set up (queue item 714).
            (
                &["offer", "offer chosen", "offer unset", "offer chosen unset"],
                Pressable::Placed(|at| Claim::Offer { at }),
            ),
        ],
    }
}

/// What a press at `at`, in the part's CSS pixels, claims in `drawing`, a
/// drawing of `document`, which is `part`'s document: what it is on, or
/// [`None`] when it is on nothing a person can press.
pub fn claimed(drawing: &Drawing, document: &Document, part: Part, at: Point) -> Option<Claim> {
    let found = hit::element_at(drawing, document, at)?;
    let kinds = pressables(part);
    let mut next = Some(found);
    while let Some(node) = next {
        let class = document.attribute_by_name(node, "class");
        if let Some((classes, pressable)) = kinds
            .iter()
            .find(|(classes, _)| class.is_some_and(|class| classes.contains(&class)))
        {
            return match pressable {
                Pressable::One(claim) => Some(*claim),
                Pressable::Placed(claim) => place_of(document, node, classes).map(claim),
            };
        }
        next = document.parent(node);
    }
    None
}

/// The place of `node` among the elements of `document` whose class is one
/// of `classes`, in document order.
fn place_of(document: &Document, node: NodeId, classes: &[&str]) -> Option<usize> {
    document
        .descendants(document.root())
        .filter(|&each| {
            document
                .attribute_by_name(each, "class")
                .is_some_and(|class| classes.contains(&class))
        })
        .position(|each| each == node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Failure, FromRenderer, ToRenderer};
    use crate::panel::{Chat, Doing, Panel};
    use crate::renderer::Renderer;
    use crate::start::{KeptSite, Start, Waiting};
    use alo_layout::Size;
    use alo_text::{Font, FontDatabase, Slant, Weight};

    /// The corpus's faces, and what the parts' stylesheets ask for.
    fn fonts() -> FontDatabase {
        let mut database = FontDatabase::new();
        for (family, weight, bytes) in [
            ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
            ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
            ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
        ] {
            if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
                database.add(font);
            }
        }
        database.map_generic("system-ui", "DejaVu Sans");
        database.map_generic("sans-serif", "DejaVu Sans");
        database.map_generic("serif", "DejaVu Serif");
        // The design's typefaces, as the browser process hands them to the
        // interface's renderers (ADR 0084).
        for font in crate::typefaces::fonts() {
            database.add(font);
        }
        database
    }

    /// `tests/the_chats_panel.rs`'s panel of five, 240 × 560, whose boxes
    /// that test asserts: three open — the one on screen, one the agent is
    /// at work in, one it finished — and two from before.
    fn five() -> Panel {
        Panel {
            chats: vec![
                Chat::new("Charging my EV", "evhomeguide.org").current(),
                Chat::new("Weekend in Lisbon", "lisbon.example").doing(Doing::Working),
                Chat::new("Invoice for March", "books.example").doing(Doing::Done),
                Chat::new("Compare phone plans", "").earlier(),
                Chat::new("Birthday present ideas", "").earlier(),
            ],
            viewport: Size::new(240.0, 560.0),
            scale: 1.0,
            ..Panel::default()
        }
    }

    /// `tests/the_first_screen.rs`'s screen 0a, 1040 × 820: four sites, two
    /// chats waiting.
    fn screen_0a() -> Start {
        Start {
            greeting: "Good morning.".to_owned(),
            sites: vec![
                KeptSite::new("Mail", "https://mail.example/"),
                KeptSite::new("Calendar", "https://calendar.example/"),
                KeptSite::new("Bank", "https://bank.example/"),
                KeptSite::new("", "evhomeguide.org"),
            ],
            waiting: vec![
                Waiting::new(
                    "Weekend in Lisbon",
                    "found 3 hotels · waiting for you to choose",
                ),
                Waiting::new("Invoice for March", "draft ready to check"),
            ],
            viewport: Size::new(1040.0, 820.0),
            ..Start::default()
        }
    }

    fn holding(work: ToRenderer) -> Renderer {
        let mut renderer = Renderer::new(fonts());
        let built = renderer.handle(work);
        assert!(matches!(built, FromRenderer::Loaded { .. }), "{built:?}");
        renderer
    }

    fn on_panel(x: f32, y: f32) -> FromRenderer {
        holding(ToRenderer::Panel(Box::new(five())))
            .handle(ToRenderer::ConversationPress(Point::new(x, y)))
    }

    fn on_start(x: f32, y: f32) -> FromRenderer {
        holding(ToRenderer::Start(Box::new(screen_0a())))
            .handle(ToRenderer::ConversationPress(Point::new(x, y)))
    }

    fn claim(claim: Claim) -> FromRenderer {
        FromRenderer::ConversationClaimed(Some(claim))
    }

    fn chat(at: usize) -> FromRenderer {
        claim(Claim::Chat { at })
    }

    #[test]
    fn a_press_on_a_chat_claims_its_place_in_the_order_drawn() {
        // Rows 220 × 38 from x 10, at y 178, 218 and 258 under *Open* and
        // 334 and 374 under *Earlier*, their corners rounded by 9.
        assert_eq!(on_panel(120.0, 194.0), chat(0));
        assert_eq!(on_panel(10.0, 197.0), chat(0));
        assert_eq!(on_panel(229.0, 197.0), chat(0));
        assert_eq!(on_panel(120.0, 178.0), chat(0));
        assert_eq!(on_panel(120.0, 215.0), chat(0));
        assert_eq!(on_panel(120.0, 234.0), chat(1));
        assert_eq!(on_panel(120.0, 274.0), chat(2));
        assert_eq!(on_panel(120.0, 344.0), chat(3));
        assert_eq!(on_panel(120.0, 411.0), chat(4));
    }

    #[test]
    fn a_press_on_what_a_row_holds_is_on_the_chat() {
        // The site's mark, 16 square from 20 in; the name after it; the
        // second's dot and the third's *Done* at their rows' ends.
        assert_eq!(on_panel(27.0, 196.0), chat(0));
        assert_eq!(on_panel(60.0, 196.0), chat(0));
        assert_eq!(on_panel(60.0, 236.0), chat(1));
        assert_eq!(on_panel(210.0, 276.0), chat(2));
    }

    #[test]
    fn new_chat_and_settings_are_claimed_as_themselves() {
        // New chat 220 × 40 at (10, 56), its ⌘T at its end; Settings 220 ×
        // 38 at (10, 510).
        assert_eq!(on_panel(60.0, 75.0), claim(Claim::NewChat));
        assert_eq!(on_panel(212.0, 75.0), claim(Claim::NewChat));
        assert_eq!(on_panel(10.0, 76.0), claim(Claim::NewChat));
        assert_eq!(on_panel(120.0, 529.0), claim(Claim::Settings));
        assert_eq!(on_panel(229.0, 520.0), claim(Claim::Settings));
    }

    #[test]
    fn a_press_on_nothing_in_the_panel_claims_nothing() {
        for (x, y) in [
            // The mark, the head's row, and just outside the button that
            // hides the panel, 36 square at (194, 11).
            (25.0, 28.0),
            (150.0, 28.0),
            (193.0, 28.0),
            (212.0, 48.0),
            // Either side of New chat, and the 4 under the head.
            (5.0, 75.0),
            (232.0, 75.0),
            (120.0, 53.0),
            // Under New chat and over Search chats, either side of Search
            // chats, a group's heading, the gap between two rows, outside
            // a row's rounded corner, and the room below the last row.
            (120.0, 99.0),
            (5.0, 121.0),
            (232.0, 121.0),
            (40.0, 165.0),
            (120.0, 217.0),
            (10.0, 178.0),
            (120.0, 450.0),
            // Under Settings, and outside the panel.
            (120.0, 552.0),
            (-1.0, 150.0),
            (240.0, 150.0),
            (120.0, 560.0),
        ] {
            assert_eq!(
                on_panel(x, y),
                FromRenderer::ConversationClaimed(None),
                "({x}, {y}) is on nothing"
            );
        }
    }

    #[test]
    fn a_press_on_a_site_or_add_a_site_claims_it() {
        // Tiles 110 × 84 from (160, 386), 122 apart: four sites, then Add a
        // site at 648.
        assert_eq!(on_start(215.0, 428.0), claim(Claim::Site { at: 0 }));
        assert_eq!(on_start(282.0, 428.0), claim(Claim::Site { at: 1 }));
        assert_eq!(on_start(459.0, 400.0), claim(Claim::Site { at: 2 }));
        assert_eq!(on_start(580.0, 469.0), claim(Claim::Site { at: 3 }));
        // The tile's letter and its name are the tile.
        assert_eq!(on_start(215.0, 405.0), claim(Claim::Site { at: 0 }));
        assert_eq!(on_start(215.0, 450.0), claim(Claim::Site { at: 0 }));
        assert_eq!(on_start(703.0, 428.0), claim(Claim::AddASite));
        assert_eq!(on_start(648.0, 428.0), claim(Claim::AddASite));
    }

    #[test]
    fn a_press_on_a_waiting_chat_claims_its_place() {
        // 354 × 68 at (160, 526) and (526, 526): its name, what it says,
        // and the padding around them.
        assert_eq!(on_start(250.0, 548.0), claim(Claim::Waiting { at: 0 }));
        assert_eq!(on_start(250.0, 568.0), claim(Claim::Waiting { at: 0 }));
        assert_eq!(on_start(165.0, 560.0), claim(Claim::Waiting { at: 0 }));
        assert_eq!(on_start(700.0, 560.0), claim(Claim::Waiting { at: 1 }));
        assert_eq!(on_start(879.0, 560.0), claim(Claim::Waiting { at: 1 }));
    }

    #[test]
    fn a_press_on_the_field_or_its_box_is_the_box_and_on_the_arrow_go() {
        // The box 720 × 64 at (160, 254); the field 632 × 40 at (183, 266);
        // Go 44 × 44 at (825, 264).
        assert_eq!(on_start(400.0, 285.0), claim(Claim::TheBox));
        assert_eq!(on_start(183.0, 266.0), claim(Claim::TheBox));
        assert_eq!(on_start(170.0, 285.0), claim(Claim::TheBox));
        assert_eq!(on_start(400.0, 256.0), claim(Claim::TheBox));
        assert_eq!(on_start(818.0, 285.0), claim(Claim::TheBox));
        assert_eq!(on_start(846.0, 285.0), claim(Claim::Go));
        assert_eq!(on_start(826.0, 285.0), claim(Claim::Go));
        assert_eq!(on_start(868.0, 285.0), claim(Claim::Go));
        assert_eq!(on_start(872.0, 285.0), claim(Claim::TheBox));
    }

    #[test]
    fn a_press_on_nothing_on_the_first_screen_claims_nothing() {
        for (x, y) in [
            // The greeting, the line under it, and either side of the box.
            (520.0, 170.0),
            (520.0, 214.0),
            (150.0, 285.0),
            (890.0, 285.0),
            // The sites' label, the gap between two tiles, the waiting
            // chats' label, the gap between them, and the room below.
            (170.0, 365.0),
            (275.0, 428.0),
            (170.0, 505.0),
            (519.0, 560.0),
            (520.0, 700.0),
            // Outside the screen.
            (1040.0, 300.0),
            (520.0, -1.0),
        ] {
            assert_eq!(
                on_start(x, y),
                FromRenderer::ConversationClaimed(None),
                "({x}, {y}) is on nothing"
            );
        }
    }

    #[test]
    fn a_chat_is_claimed_by_the_place_the_panel_draws_it_not_the_place_it_was_sent() {
        // Sent earlier, open, earlier, open: drawn open, open, earlier,
        // earlier, so the second row is the third chat sent.
        let panel = Panel {
            chats: vec![
                Chat::new("Old one", "").earlier(),
                Chat::new("First", "").current(),
                Chat::new("Old two", "").earlier(),
                Chat::new("Second", ""),
            ],
            viewport: Size::new(240.0, 560.0),
            scale: 1.0,
            ..Panel::default()
        };
        assert_eq!(crate::panel::drawn_order(&panel.chats), vec![1, 3, 0, 2]);
        let mut renderer = holding(ToRenderer::Panel(Box::new(panel)));
        let mut at = |y| renderer.handle(ToRenderer::ConversationPress(Point::new(120.0, y)));
        assert_eq!(at(194.0), chat(0));
        assert_eq!(at(234.0), chat(1));
        // The earlier group's heading is 18 below the last open row, so its
        // rows are at 294 and 334.
        assert_eq!(at(310.0), chat(2));
        assert_eq!(at(350.0), chat(3));
    }

    #[test]
    fn each_part_claims_only_what_it_draws_and_at_a_scale_in_css_pixels() {
        // The first screen draws no chat row: a press where the panel's
        // would be is on nothing there.
        assert_eq!(
            on_start(120.0, 150.0),
            FromRenderer::ConversationClaimed(None)
        );
        // At two, the same CSS pixels are claimed alike.
        let mut renderer = holding(ToRenderer::Panel(Box::new(five().at_scale(2.0))));
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(Point::new(120.0, 234.0))),
            chat(1)
        );
    }

    #[test]
    fn a_renderer_holding_neither_part_refuses_the_press() {
        let refused = FromRenderer::Failed(Failure::NotTheConversation);
        let press = ToRenderer::ConversationPress(Point::new(120.0, 194.0));
        let mut empty = Renderer::new(fonts());
        assert_eq!(empty.handle(press.clone()), refused);
        let page = crate::page::Page::new(
            "<a class=chat href=#chat style='display:block;height:400px'>a</a>",
            Size::new(240.0, 560.0),
        );
        let mut on_a_page = holding(ToRenderer::Load(Box::new(page.clone())));
        assert_eq!(on_a_page.handle(press.clone()), refused);
        let strip = crate::strip::Strip {
            tabs: vec![crate::strip::StripTab::new("a", "about:blank")],
            viewport: Size::new(240.0, 36.0),
            scale: 1.0,
        };
        let mut on_the_strip = holding(ToRenderer::Strip(Box::new(strip)));
        assert_eq!(on_the_strip.handle(press.clone()), refused);
        // Nor is the strip's press answered by a part.
        let mut panel = holding(ToRenderer::Panel(Box::new(five())));
        assert_eq!(
            panel.handle(ToRenderer::StripPress(Point::new(120.0, 150.0))),
            FromRenderer::Failed(Failure::NotTheStrip)
        );
        // A part let go of for a page is no longer held.
        assert_eq!(panel.handle(press.clone()), chat(0));
        let loaded = panel.handle(ToRenderer::Load(Box::new(page)));
        assert!(matches!(loaded, FromRenderer::Loaded { .. }), "{loaded:?}");
        assert_eq!(panel.handle(press), refused);
    }

    #[test]
    fn search_chats_and_the_button_that_hides_the_panel_are_claimed_as_themselves() {
        // Search chats 220 × 38 at (10, 102): its glass at 22, its field
        // from 46, and the room around them.
        for x in [12.0, 30.0, 100.0, 228.0] {
            assert_eq!(on_panel(x, 121.0), claim(Claim::SearchChats), "{x}");
        }
        assert_eq!(on_panel(120.0, 103.0), claim(Claim::SearchChats));
        // The button 36 square at (194, 11), and its icon in the middle.
        assert_eq!(on_panel(212.0, 29.0), claim(Claim::HideThePanel));
        assert_eq!(on_panel(194.0, 11.0), claim(Claim::HideThePanel));
        assert_eq!(on_panel(229.0, 46.0), claim(Claim::HideThePanel));
    }

    /// The panel hidden, as the rail 64 wide.
    fn on_the_rail(x: f32, y: f32) -> FromRenderer {
        let rail = Panel {
            hidden: true,
            viewport: Size::new(64.0, 560.0),
            ..five()
        };
        holding(ToRenderer::Panel(Box::new(rail)))
            .handle(ToRenderer::ConversationPress(Point::new(x, y)))
    }

    #[test]
    fn the_rails_new_chat_and_show_the_panel_are_claimed_and_nothing_else() {
        // New chat 44 square at (10, 56); Show the panel at (10, 504).
        assert_eq!(on_the_rail(32.0, 78.0), claim(Claim::NewChat));
        assert_eq!(on_the_rail(10.0, 56.0), claim(Claim::NewChat));
        assert_eq!(on_the_rail(32.0, 526.0), claim(Claim::ShowThePanel));
        assert_eq!(on_the_rail(53.0, 547.0), claim(Claim::ShowThePanel));
        for (x, y) in [
            // The mark, the line, the room, either side of a button, and
            // where the panel's chats and Search chats would be.
            (32.0, 30.0),
            (32.0, 110.0),
            (32.0, 300.0),
            (5.0, 78.0),
            (58.0, 526.0),
            (32.0, 194.0),
            (32.0, 121.0),
        ] {
            assert_eq!(
                on_the_rail(x, y),
                FromRenderer::ConversationClaimed(None),
                "({x}, {y}) is on nothing"
            );
        }
    }

    /// The first screen of 0a with `typed` typed into the box, which has the
    /// keyboard, and what is offered for it.
    fn typed(typed: &str) -> Start {
        Start {
            typed: typed.to_owned(),
            focused: true,
            offers: crate::typed::offers(typed),
            ..screen_0a()
        }
    }

    #[test]
    fn a_press_on_an_offer_claims_its_place_among_those_drawn() {
        // `tests/what_is_typed_on_the_first_screen.rs`: three offers 706 ×
        // 52 from (167, 333) for an address, the first chosen; two for
        // words; each claimed wherever in it, its words or what is under
        // them, chosen or not, set up or not.
        let mut address = holding(ToRenderer::Start(Box::new(typed("example.com"))));
        let mut at = |x, y| address.handle(ToRenderer::ConversationPress(Point::new(x, y)));
        assert_eq!(at(400.0, 359.0), claim(Claim::Offer { at: 0 }));
        assert_eq!(at(167.0, 359.0), claim(Claim::Offer { at: 0 }));
        assert_eq!(at(200.0, 345.0), claim(Claim::Offer { at: 0 }));
        assert_eq!(at(200.0, 372.0), claim(Claim::Offer { at: 0 }));
        assert_eq!(at(400.0, 411.0), claim(Claim::Offer { at: 1 }));
        assert_eq!(at(872.0, 463.0), claim(Claim::Offer { at: 2 }));
        // The card's padding and border around them, and past it.
        assert_eq!(at(163.0, 400.0), FromRenderer::ConversationClaimed(None));
        assert_eq!(at(400.0, 329.0), FromRenderer::ConversationClaimed(None));
        assert_eq!(at(400.0, 492.0), FromRenderer::ConversationClaimed(None));
        // The box is still the box.
        assert_eq!(at(400.0, 285.0), claim(Claim::TheBox));
        let mut words = holding(ToRenderer::Start(Box::new(typed("what is x"))));
        let mut at = |x, y| words.handle(ToRenderer::ConversationPress(Point::new(x, y)));
        assert_eq!(at(400.0, 359.0), claim(Claim::Offer { at: 0 }));
        assert_eq!(at(400.0, 411.0), claim(Claim::Offer { at: 1 }));
        assert_eq!(at(400.0, 463.0), FromRenderer::ConversationClaimed(None));
    }

    #[test]
    fn each_claim_and_part_is_said_in_words() {
        assert_eq!(Part::Panel.to_string(), "the chats panel");
        assert_eq!(Part::Start.to_string(), "the first screen");
        assert_eq!(Claim::Chat { at: 2 }.to_string(), "the chat at place 2");
        assert_eq!(Claim::TheBox.to_string(), "the box");
        assert_eq!(Claim::Go.part(), Part::Start);
        assert_eq!(Claim::Settings.part(), Part::Panel);
        assert_eq!(Claim::Offer { at: 1 }.to_string(), "the offer at place 1");
        assert_eq!(Claim::Offer { at: 1 }.part(), Part::Start);
        assert_eq!(Claim::SearchChats.to_string(), "Search chats");
        for claim in [Claim::SearchChats, Claim::HideThePanel, Claim::ShowThePanel] {
            assert_eq!(claim.part(), Part::Panel);
        }
    }
}

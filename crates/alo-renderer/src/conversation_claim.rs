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
//! Not drawn, so never claimed: *Search chats* and the button that hides
//! the panel (queue item 711). Drawn but not yet claimed: the offers under
//! the box for what was typed (queue item 701), a press on which claims
//! nothing until 714. A box with the keyboard is drawn under a `main` of
//! class `start focused`; its own class stays `box`.

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
}

impl Claim {
    /// The part that draws what this claims.
    pub fn part(self) -> Part {
        match self {
            Claim::NewChat | Claim::Chat { .. } | Claim::Settings => Part::Panel,
            Claim::Site { .. }
            | Claim::AddASite
            | Claim::Waiting { .. }
            | Claim::TheBox
            | Claim::Go => Part::Start,
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
/// whole `class` attribute its template or document builder sets.
fn pressables(part: Part) -> &'static [(&'static str, Pressable)] {
    match part {
        Part::Panel => &[
            ("new", Pressable::One(Claim::NewChat)),
            ("chat", Pressable::Placed(|at| Claim::Chat { at })),
            ("settings", Pressable::One(Claim::Settings)),
        ],
        Part::Start => &[
            ("tile", Pressable::Placed(|at| Claim::Site { at })),
            ("tile add", Pressable::One(Claim::AddASite)),
            ("wait", Pressable::Placed(|at| Claim::Waiting { at })),
            ("go", Pressable::One(Claim::Go)),
            ("ask", Pressable::One(Claim::TheBox)),
            ("box", Pressable::One(Claim::TheBox)),
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
        if let Some((class, pressable)) = kinds.iter().find(|(kind, _)| Some(*kind) == class) {
            return match pressable {
                Pressable::One(claim) => Some(*claim),
                Pressable::Placed(claim) => place_of(document, node, class).map(claim),
            };
        }
        next = document.parent(node);
    }
    None
}

/// The place of `node` among the elements of `document` whose class is
/// `class`, in document order.
fn place_of(document: &Document, node: NodeId, class: &str) -> Option<usize> {
    document
        .descendants(document.root())
        .filter(|&each| document.attribute_by_name(each, "class") == Some(class))
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
        // Rows 220 × 38 from x 10, at y 134, 174 and 214 under *Open* and
        // 290 and 330 under *Earlier*, their corners rounded by 9.
        assert_eq!(on_panel(120.0, 150.0), chat(0));
        assert_eq!(on_panel(10.0, 153.0), chat(0));
        assert_eq!(on_panel(229.0, 153.0), chat(0));
        assert_eq!(on_panel(120.0, 134.0), chat(0));
        assert_eq!(on_panel(120.0, 171.0), chat(0));
        assert_eq!(on_panel(120.0, 190.0), chat(1));
        assert_eq!(on_panel(120.0, 230.0), chat(2));
        assert_eq!(on_panel(120.0, 300.0), chat(3));
        assert_eq!(on_panel(120.0, 367.0), chat(4));
    }

    #[test]
    fn a_press_on_what_a_row_holds_is_on_the_chat() {
        // The site's mark, 16 square from 20 in; the name after it; the
        // second's dot and the third's *Done* at their rows' ends.
        assert_eq!(on_panel(27.0, 152.0), chat(0));
        assert_eq!(on_panel(60.0, 152.0), chat(0));
        assert_eq!(on_panel(60.0, 192.0), chat(1));
        assert_eq!(on_panel(210.0, 232.0), chat(2));
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
            // The mark, and the head's row.
            (25.0, 28.0),
            (200.0, 28.0),
            // Either side of New chat, and the 4 under the head.
            (5.0, 75.0),
            (232.0, 75.0),
            (120.0, 53.0),
            // A group's heading, the gap between two rows, outside a row's
            // rounded corner, and the room below the last row.
            (40.0, 120.0),
            (120.0, 172.0),
            (10.0, 134.0),
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
        };
        assert_eq!(crate::panel::drawn_order(&panel.chats), vec![1, 3, 0, 2]);
        let mut renderer = holding(ToRenderer::Panel(Box::new(panel)));
        let mut at = |y| renderer.handle(ToRenderer::ConversationPress(Point::new(120.0, y)));
        assert_eq!(at(150.0), chat(0));
        assert_eq!(at(190.0), chat(1));
        // The earlier group's heading is 18 below the last open row, so its
        // rows are at 250 and 290.
        assert_eq!(at(266.0), chat(2));
        assert_eq!(at(306.0), chat(3));
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
            renderer.handle(ToRenderer::ConversationPress(Point::new(120.0, 190.0))),
            chat(1)
        );
    }

    #[test]
    fn a_renderer_holding_neither_part_refuses_the_press() {
        let refused = FromRenderer::Failed(Failure::NotTheConversation);
        let press = ToRenderer::ConversationPress(Point::new(120.0, 150.0));
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
    fn each_claim_and_part_is_said_in_words() {
        assert_eq!(Part::Panel.to_string(), "the chats panel");
        assert_eq!(Part::Start.to_string(), "the first screen");
        assert_eq!(Claim::Chat { at: 2 }.to_string(), "the chat at place 2");
        assert_eq!(Claim::TheBox.to_string(), "the box");
        assert_eq!(Claim::Go.part(), Part::Start);
        assert_eq!(Claim::Settings.part(), Part::Panel);
    }
}

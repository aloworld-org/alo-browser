/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Every message the boundary carries, written down and read back — and what
//! happens when what comes back is a lie.
//!
//! ADR 0005: the browser process holds the network, the disk and the profile,
//! and a renderer is the process that parsed a hostile page. So the interesting
//! half of this file is not the round trips. It is that a message *from* a
//! renderer is treated as bytes a stranger chose, because if a page found a way
//! to steer that process, the stranger is the page.

use alo_agent::verb::{Invalid, Outcome, Refusal, ScrollBy, Target, Verb};
use alo_bindings::Visibility;
use alo_bindings::navigating::Submission;
use alo_box::invalid;
use alo_box::role::{KnownRole, Role};
use alo_box::state::{Checked, Current, States};
use alo_box::tree::BoxId;
use alo_css::media::ColorScheme;
use alo_dom::constraint::Constraint;
use alo_dom::submission::{Enctype, Entry, Method, Value};
use alo_layout::geometry::{Point, Rect, Size};
use alo_net::csp::{Inline, Placement};
use alo_net::referrer::Policy;
use alo_renderer::Screen;
use alo_renderer::ask::{Asked, By};
use alo_renderer::conversation_claim::Claim as ConversationClaim;
use alo_renderer::conversation_scroll::Bar;
use alo_renderer::frame::Frame;
use alo_renderer::generic::{Generics, MOST_PAIRS};
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::results::{
    Found, Images, LARGEST_THUMBNAIL, MOST_IMAGES, Pictured, Results, SetUp, Showing, Tab,
};
use alo_renderer::snapshot::{Snapshot, SnapshotNode};
use alo_renderer::start::{KeptSite, SettingsKeyed, Start, Waiting};
use alo_renderer::strip::LONGEST_TITLE;
use alo_renderer::turn::{MOST_TURNS, Turn};
use alo_renderer::typed::Offer;
use alo_renderer::violations::{MOST_OBJECTIONS, Objection};
use alo_renderer::wire::{
    DEEPEST_TREE, read_from_renderer, read_to_renderer, write_from_renderer, write_to_renderer,
};

fn id(n: usize) -> BoxId {
    BoxId::from_wire(n)
}

fn a_rect() -> Rect {
    Rect {
        origin: Point { x: 12.0, y: 34.5 },
        size: Size {
            width: 100.0,
            height: 20.25,
        },
    }
}

/// A field's caret, a selection and a hostile composition, scrolled in a
/// room (queue items 861, 848).
fn an_editing() -> alo_renderer::field::Editing {
    alo_renderer::field::Editing {
        composing: Some(alo_renderer::field::Composing {
            text: "きょう\0".to_owned(),
            caret: 2,
        }),
        scrolled: 12.5,
        room: Some(640.0),
        ..alo_renderer::field::Editing::caret_at(3).selecting_from(11)
    }
}

fn a_node(children: Vec<SnapshotNode>) -> SnapshotNode {
    SnapshotNode {
        id: id(7),
        role: Role::Known(KnownRole::Button),
        name: Some("Save".to_owned()),
        states: States {
            disabled: true,
            checked: Some(Checked::Mixed),
            selected: Some(false),
            expanded: None,
            pressed: Some(true),
            required: true,
            read_only: false,
            busy: true,
            // A value nobody here can judge crosses as such (ADR 0051 § 6).
            invalid: Some(invalid::Invalid::Unknown),
            hidden: false,
            level: Some(3),
            current: Some(Current::Page),
            takes_text: true,
            // A picture that failed crosses too (ADR 0046 § 5).
            broken: true,
            // And a field's selection, in characters (queue item 861).
            selection: Some((3, 7)),
        },
        // A field's hint crosses too, so the outline on the far side reads
        // as the tree did (ADR 0043 § 5).
        placeholder: Some("you@company.eu".to_owned()),
        // And a list item's marker (ADR 0044 § 5).
        marker: Some("3.".to_owned()),
        rect: a_rect(),
        // Two pieces, so the round trip carries a wrapped inline rather than
        // only the easy case of one rectangle.
        rects: vec![a_rect(), a_rect()],
        offscreen: true,
        scrolls: false,
        children,
    }
}

// --- Round trips -------------------------------------------------------------

#[test]
fn every_message_to_a_renderer_survives_the_crossing() {
    let messages = vec![
        ToRenderer::Load(Box::new(Page {
            url: alo_url::parse("https://example.com/a/b?c#d").unwrap(),
            html: "<p>hello</p>".to_owned(),
            sheets: vec!["p { color: red }".to_owned(), "p { margin: 0 }".to_owned()],
            viewport: Size {
                width: 900.0,
                height: 600.0,
            },
            // Not what `Page::new` gives, so a reader that assumed it would show.
            scale: 2.0,
            visibility: Visibility::Hidden,
            scheme: ColorScheme::Dark,
            policies: vec![
                "script-src 'self' 'nonce-abc'".to_owned(),
                "default-src 'none'".to_owned(),
            ],
            watching: vec!["script-src 'none'; report-uri /csp".to_owned()],
            // Not this build's, so a reader that made up its own would show.
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) alo/9.9".to_owned(),
            platform: "Linux x86_64".to_owned(),
        })),
        ToRenderer::Resize(Screen::new(Size::new(320.5, 480.0))),
        ToRenderer::Resize(Screen::new(Size::new(320.5, 480.0)).at_scale(1.5)),
        ToRenderer::Visibility(Visibility::Hidden),
        ToRenderer::Visibility(Visibility::Visible),
        ToRenderer::Leave,
        ToRenderer::Paint,
        ToRenderer::ReadTree,
        ToRenderer::Act {
            target: Target::NamedOfRole {
                role: Role::Known(KnownRole::Button),
                name: "Save".to_owned(),
            },
            verb: Verb::Activate,
        },
        ToRenderer::Act {
            target: Target::Named("First day away".to_owned()),
            verb: Verb::PutText("12 May".to_owned()),
        },
        ToRenderer::Act {
            target: Target::Node(id(42)),
            verb: Verb::Scroll(ScrollBy::Pixels(-120.0)),
        },
        ToRenderer::Act {
            target: Target::OfRole(Role::Declared("carousel".into())),
            verb: Verb::Scroll(ScrollBy::ToEnd),
        },
        // A generic meaning several families in preference order, which is what
        // a machine with more than one of them produces.
        ToRenderer::UseGenerics(Generics::stating(vec![
            ("sans-serif".to_owned(), "Helvetica Neue".to_owned()),
            ("sans-serif".to_owned(), "Geneva".to_owned()),
            ("monospace".to_owned(), "Menlo".to_owned()),
        ])),
        ToRenderer::UseGenerics(Generics::new()),
        // Every way a chat can be, and a name a page could have written.
        ToRenderer::Panel(Box::new(Panel {
            chats: vec![
                Chat::new("Charging my EV", "evhomeguide.org").current(),
                Chat::new("<b>a\0b</b>", "").doing(Doing::Working),
                Chat::new("Invoice", "books.example").doing(Doing::Done),
                Chat::new("Earlier", "").earlier(),
            ],
            viewport: Size::new(240.0, 560.5),
            scale: 2.0,
            ..Panel::default()
        })),
        ToRenderer::Panel(Box::default()),
        ToRenderer::Start(Box::new(Start {
            greeting: "Good morning.".to_owned(),
            sites: vec![
                KeptSite::new("Mail", "https://mail.example/"),
                KeptSite::new("", "a\0b"),
            ],
            waiting: vec![Waiting::new("Lisbon", "found 3 hotels")],
            viewport: Size::new(1040.0, 820.5),
            scale: 1.5,
            // What a person typed, hostile, and what was offered for it.
            typed: "example.com</script>\0<b>".to_owned(),
            focused: true,
            // Its caret, a selection and a composition (queue items 861,
            // 848).
            editing: an_editing(),
            offers: alo_renderer::typed::offers("example.com")
                .into_iter()
                .chain([Offer::Search("</script>\0<b>".to_owned())])
                .collect(),
            turns: Vec::new(),
            down: 0.0,
            bar: None,
            search: SetUp::Not("no key\0<b>".to_owned()),
            asking: SetUp::Not("no page\0<b>".to_owned()),
            face: InterfaceFace::PlusJakartaSans,
            ..Start::default()
        })),
        ToRenderer::Start(Box::default()),
        // A person's press on a part of the conversation, anywhere in it.
        ToRenderer::ConversationPress(alo_paint::Point::new(120.0, 153.5)),
        ToRenderer::ConversationPress(alo_paint::Point::new(-0.25, 1e9)),
    ];
    for original in messages {
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// Queue item 711: the panel searched for in words a page could have
/// written, with the keyboard, and hidden as the rail.
#[test]
fn the_panel_searched_and_hidden_survives_the_crossing() {
    let messages = vec![
        ToRenderer::Panel(Box::new(Panel {
            chats: vec![Chat::new("Invoice", "").current()],
            viewport: Size::new(240.0, 820.0),
            searched: "</nav><b>in\0</b>".to_owned(),
            searching: true,
            ..Panel::default()
        })),
        ToRenderer::Panel(Box::new(Panel {
            viewport: Size::new(64.0, 820.0),
            hidden: true,
            ..Panel::default()
        })),
        // Queue item 789: a row hovered offering Delete, and a chat just
        // deleted, named in words a page could have written; and the
        // pointer over the panel.
        ToRenderer::Panel(Box::new(Panel {
            chats: vec![
                Chat::new("Invoice", "").current(),
                Chat::new("Lisbon", "").hovered().earlier(),
            ],
            viewport: Size::new(240.0, 820.0),
            deleted: Some("</nav><b>gone\0</b>".to_owned()),
            ..Panel::default()
        })),
        ToRenderer::ConversationHover(alo_paint::Point::new(120.0, 273.5)),
        ToRenderer::ConversationHover(alo_paint::Point::new(-0.25, 1e9)),
        // Queue items 791 and 760: an open row hovered offering its ×, the
        // empty chat on screen offering none, and the rail's chats with one
        // hovered, its name beside it, laid out wider.
        ToRenderer::Panel(Box::new(Panel {
            chats: vec![
                Chat::new("", "").current().not_closable(),
                Chat::new("Lisbon", "").hovered(),
            ],
            viewport: Size::new(240.0, 820.0),
            ..Panel::default()
        })),
        ToRenderer::Panel(Box::new(Panel {
            chats: vec![
                Chat::new("Weekend in Lisbon", "").current(),
                Chat::new("</button>\u{202e}", "").hovered().earlier(),
            ],
            viewport: Size::new(320.0, 820.0),
            hidden: true,
            ..Panel::default()
        })),
    ];
    for original in messages {
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// Queue item 708: a search's results, each state a search can be in,
/// cross as they were sent.
#[test]
fn a_searchs_results_survive_the_crossing() {
    let messages = vec![
        // Strangers' strings in every field.
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(Results::of(
                "</p><script>".to_owned(),
                Showing::Found(vec![
                    Found::new(
                        "<b>Fish &amp; chips</b>\0",
                        alo_url::parse("https://fish.example/a?b#c")
                            .unwrap_or_else(|_| alo_url::Url::about_blank()),
                        "two <strong>lines</strong>",
                    )
                    .named("Fish\u{202e}"),
                ]),
            ))],
            search: SetUp::Ready,
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(Results::searching("x"))],
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(Results::of(
                "x".to_owned(),
                Showing::Unanswered("Brave Search could not answer just now (500).".to_owned()),
            ))],
            ..Start::default()
        })),
    ];
    for original in messages {
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// Queue item 707: a search's answer crosses as it was sent, in each state
/// and on each tab; an answer citing a site it was not given, an answer to
/// a search that found nothing, too many sentences, and a claim on *Full
/// answer ›* tagged as nobody knows, are refused.
#[test]
fn a_searchs_answer_crosses_and_one_citing_a_site_not_given_is_refused() {
    use alo_renderer::results_answer::{Answer, Cited, MOST_SENTENCES, Written};
    let sites = |count: usize| {
        Showing::Found(
            (0..count)
                .map(|n| {
                    Found::new(
                        "t",
                        alo_url::parse(&format!("https://s{n}.example/"))
                            .unwrap_or_else(|_| alo_url::Url::about_blank()),
                        "",
                    )
                })
                .collect(),
        )
    };
    let written = |cites: usize| {
        Answer::Written(Written {
            line: "</p><script>".to_owned(),
            sentences: vec![Cited::new("<b>one</b>\0", [1, cites])],
            points: vec![Cited::new("two", [cites])],
        })
    };
    let start = |showing: Showing, answer: Answer, tab: Tab| {
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(
                Results::of("ev", showing).with_answer(answer).on(tab),
            )],
            ..Start::default()
        }))
    };
    for original in [
        start(sites(3), Answer::Asking, Tab::All),
        start(sites(3), Answer::Asking, Tab::Answer),
        start(sites(3), written(3), Tab::Answer),
        start(sites(20), written(8), Tab::All),
        start(sites(1), Answer::NotFound, Tab::All),
        start(
            sites(1),
            Answer::Unanswered("<b>401</b>".to_owned()),
            Tab::Answer,
        ),
        start(sites(0), Answer::NotAsked, Tab::All),
    ] {
        let back = read_to_renderer(&write_to_renderer(&original))
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
    let mut many = Written {
        line: "x".to_owned(),
        sentences: vec![Cited::new("x", [1]); MOST_SENTENCES + 1],
        points: Vec::new(),
    };
    for (message, refusal) in [
        (
            start(sites(3), written(4), Tab::All),
            "cites source 4, and it was given 3",
        ),
        (
            start(sites(20), written(9), Tab::All),
            "cites source 9, and it was given 8",
        ),
        (
            start(sites(0), Answer::Asking, Tab::All),
            "an answer to a search that found no sites",
        ),
        (
            start(sites(3), Answer::NotAsked, Tab::Answer),
            "results shown on Answer, which they cannot be",
        ),
        (
            start(sites(3), Answer::Written(many.clone()), Tab::All),
            "an answer of 4 sentences, and at most 3 are drawn",
        ),
    ] {
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{refusal}: {why:?}");
    }
    many.sentences.clear();
    let why = read_to_renderer(&write_to_renderer(&start(
        sites(3),
        Answer::Written(many),
        Tab::All,
    )))
    .err()
    .map(|why| why.why)
    .unwrap_or_default();
    assert!(why.contains("the answer has no sentence"), "{why:?}");
    // A press on Full answer crosses back by its place.
    let claimed = FromRenderer::ConversationClaimed(Some(ConversationClaim::FullAnswer { at: 2 }));
    let back = read_from_renderer(&write_from_renderer(&claimed));
    assert_eq!(back.ok(), Some(claimed));
}

/// Queue item 761: a search's further pages of sites cross as they were
/// sent — how many were added and where the next is — and sites past what
/// those pages hold, or more pages than can be asked, are refused.
#[test]
fn a_searchs_further_pages_cross_and_too_many_are_refused() {
    use alo_renderer::results::{MOST_FOUND, MOST_PAGES, Next, Tab};
    let sites = |count: usize| {
        Showing::Found(
            (0..count)
                .map(|n| {
                    Found::new(
                        "t",
                        alo_url::parse(&format!("https://s{n}.example/"))
                            .unwrap_or_else(|_| alo_url::Url::about_blank()),
                        "",
                    )
                })
                .collect(),
        )
    };
    let paged = |count: usize, added: usize, next: Next| {
        let mut results = Results::of("ev", sites(count)).on(Tab::Sites);
        results.pages.added = added;
        results.pages.next = next;
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(results)],
            ..Start::default()
        }))
    };
    for original in [
        paged(45, 2, Next::Offered),
        paged(60, 2, Next::Asking),
        paged(21, 1, Next::Unanswered("<b>503</b>".to_owned())),
        paged(MOST_FOUND * (MOST_PAGES + 1), MOST_PAGES, Next::None),
    ] {
        let back = read_to_renderer(&write_to_renderer(&original))
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
    for (message, refusal) in [
        (
            paged(41, 1, Next::Offered),
            "41 sites found, and at most 40",
        ),
        (
            paged(20, MOST_PAGES + 1, Next::None),
            "10 further pages of sites, and at most 9",
        ),
        (paged(201, MOST_PAGES, Next::None), "at most 200"),
    ] {
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{refusal}: {why:?}");
    }
}

#[test]
fn every_message_from_a_renderer_survives_the_crossing() {
    // An act's objections cross in
    // `an_act_or_a_delivery_carries_objections_and_no_more_than_one_may`.
    let acted = |outcome, issues: Vec<String>| FromRenderer::Acted {
        outcome,
        issues,
        objections: Vec::new(),
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
    };
    let messages = vec![
        FromRenderer::Loaded {
            issues: vec!["refused `float: left`".to_owned()],
            wanted: vec!["Inter".to_owned()],
            objections: vec![
                Objection {
                    policy: 0,
                    kind: Inline::Script,
                    placement: Placement::Element,
                },
                Objection {
                    policy: 3,
                    kind: Inline::Style,
                    placement: Placement::Attribute,
                },
            ],
            navigation: None,
            fetches: Vec::new(),
            sheets: Vec::new(),
            left: Vec::new(),
            title: String::new(),
        },
        FromRenderer::Painted(Frame {
            width: 2,
            height: 1,
            pixels: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }),
        FromRenderer::Tree(Box::new(Snapshot { root: None })),
        FromRenderer::Tree(Box::new(Snapshot {
            root: Some(a_node(vec![a_node(vec![]), a_node(vec![a_node(vec![])])])),
        })),
        acted(
            Outcome::Activated {
                node: id(3),
                name: Some("Save".to_owned()),
            },
            Vec::new(),
        ),
        // A followed link's act is in
        // `where_a_page_asked_to_go_survives_the_crossing`, with its ask.
        acted(
            Outcome::TextPut {
                node: id(5),
                text: "12 May".to_owned(),
            },
            Vec::new(),
        ),
        acted(
            Outcome::Scrolled {
                node: id(6),
                by: ScrollBy::ToStart,
            },
            Vec::new(),
        ),
        acted(
            Outcome::TextCanceled {
                node: id(7),
                text: "refused".to_owned(),
            },
            vec!["the text: the page cancelled it".to_owned()],
        ),
        acted(
            Outcome::Chosen {
                node: id(43),
                list: id(29),
                chosen: false,
            },
            vec!["the choice: a listener put it back".to_owned()],
        ),
        FromRenderer::Failed(Failure::NothingLoaded),
        FromRenderer::Failed(Failure::Unpaintable {
            why: "a window of no size".to_owned(),
        }),
        FromRenderer::Failed(Failure::NoStrip {
            why: "the template has no tablist".to_owned(),
        }),
        FromRenderer::Failed(Failure::NoStart {
            why: "the template has no greeting".to_owned(),
        }),
        FromRenderer::Failed(Failure::NoPanel {
            why: "the template has no place for the chats".to_owned(),
        }),
        FromRenderer::UsingGenerics {
            answering: vec!["sans-serif".to_owned(), "monospace".to_owned()],
        },
        FromRenderer::UsingGenerics { answering: vec![] },
    ];
    for original in messages {
        let bytes = write_from_renderer(&original);
        let back = read_from_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// The first screen holding a page as a card (queue item 717): what the
/// person said, hostile, the address, somewhere to go back to, a frame and
/// its page loaded; and a card before its first frame, loading (queue item
/// 751).
#[test]
fn a_first_screen_holding_a_card_survives_the_crossing() {
    let messages = vec![
        // A page shown as a card, with somewhere to go back to and a frame
        // (queue item 717), and one before its first frame.
        ToRenderer::Start(Box::new(Start {
            turns: Some(
                alo_renderer::card::Card::new(
                    "example.com</p>\0",
                    alo_url::parse("https://example.com/a?b#c")
                        .unwrap_or_else(|_| panic!("parses")),
                )
                .holding(b"\x89PNG\r\n\x1a\nthe rest".to_vec())
                .loaded(),
            )
            .map(|mut card| {
                card.back = true;
                Turn::Page(card)
            })
            .into_iter()
            .collect(),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Page(alo_renderer::card::Card::new(
                "",
                alo_url::parse("http://127.0.0.1:8765/").unwrap_or_else(|_| panic!("parses")),
            ))],
            ..Start::default()
        })),
    ];
    for original in messages {
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// Queue item 704: the agent at work on a card's page crosses as it was
/// sent, in each thing it can be doing, its strangers' words intact; and
/// one holding more steps or choices than are drawn is refused.
#[test]
fn the_agent_at_work_crosses_and_too_many_steps_or_choices_are_refused() {
    use alo_renderer::agent_work::{AtWork, MOST_CHOICES, MOST_STEPS, Now, Step};
    let card = |work: AtWork| {
        let mut card = alo_renderer::card::Card::new(
            "a page",
            alo_url::parse("https://example.com/").unwrap_or_else(|_| panic!("parses")),
        );
        card.agent = Some(work);
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Page(card)],
            ..Start::default()
        }))
    };
    let step = Step {
        line: "Typing \u{201c}</li>\0\u{201d}".to_owned(),
        why: "<b>because</b>".to_owned(),
    };
    let mut nows = vec![
        Now::Acting,
        Now::Asking {
            what: "Press \u{201c}Send\u{201d}?\0".to_owned(),
            paying: None,
        },
        Now::Stuck {
            why: "no form".to_owned(),
            choices: vec!["a".to_owned(); MOST_CHOICES],
        },
        Now::Done {
            said: "Sent.".to_owned(),
        },
        Now::Stopped {
            why: "You stopped alo.".to_owned(),
        },
    ];
    for now in nows.drain(..) {
        let original = card(AtWork {
            plan: None,
            asked: "send <it>".to_owned(),
            steps: vec![step.clone(); MOST_STEPS],
            now,
            choosing: None,
        });
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
    for refused in [
        card(AtWork {
            plan: None,
            asked: String::new(),
            steps: vec![step.clone(); MOST_STEPS + 1],
            now: Now::Acting,
            choosing: None,
        }),
        card(AtWork {
            plan: None,
            asked: String::new(),
            steps: Vec::new(),
            now: Now::Stuck {
                why: String::new(),
                choices: vec!["a".to_owned(); MOST_CHOICES + 1],
            },
            choosing: None,
        }),
    ] {
        let bytes = write_to_renderer(&refused);
        assert!(read_to_renderer(&bytes).is_err(), "{refused:?} was read");
    }
}

/// Queue items 824, 825 and 826: the agent's plan, it held while the page
/// is the person's, and what a press pays cross as they were sent,
/// strangers' words intact; a plan past five steps, on no step or with a
/// step past its line, and what a press pays in more lines than are drawn
/// or a line too long, are refused.
/// Queue item 821: a turn asked with no page crosses — live as the newest
/// turn, kept anywhere — and live anywhere else, or keeping two runs, is
/// refused.
#[test]
fn a_turn_asked_with_no_page_crosses_and_live_only_as_the_newest() {
    use alo_renderer::agent_asked::Asked;
    use alo_renderer::agent_ran::Ran;
    use alo_renderer::agent_work::AtWork;
    let asked = |agent: Option<AtWork>, runs: usize| {
        Turn::Asked(Asked {
            said: "find </p> the hours".to_owned(),
            agent,
            ran: vec![Ran::starting("find </p> the hours"); runs],
        })
    };
    let start = |turns: Vec<Turn>| {
        ToRenderer::Start(Box::new(Start {
            turns,
            ..Start::default()
        }))
    };
    for original in [
        start(vec![asked(Some(AtWork::starting("x\0")), 1)]),
        start(vec![asked(None, 1), asked(Some(AtWork::starting("y")), 1)]),
        start(vec![asked(None, 0)]),
    ] {
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
    for (refused, why) in [
        (
            start(vec![asked(Some(AtWork::starting("x")), 1), asked(None, 1)]),
            "not the newest",
        ),
        (start(vec![asked(None, 2)]), "more than its one run"),
    ] {
        let bytes = write_to_renderer(&refused);
        let said = read_to_renderer(&bytes).expect_err("refused").to_string();
        assert!(said.contains(why), "{said}");
    }
}

#[test]
fn a_plan_a_pause_and_a_payment_cross_and_past_their_bounds_are_refused() {
    use alo_renderer::agent_paying::{MOST_SUMMARY, PAYING_LINE};
    use alo_renderer::agent_plan::{MOST_PLAN, PLAN_LINE};
    use alo_renderer::agent_work::{AtWork, Now, Paying, Plan};
    let card = |work: AtWork| {
        let mut card = alo_renderer::card::Card::new(
            "a page",
            alo_url::parse("https://example.com/").unwrap_or_else(|_| panic!("parses")),
        );
        card.agent = Some(work);
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Page(card)],
            ..Start::default()
        }))
    };
    let plan = Plan {
        steps: vec!["Open </li> search".to_owned(); MOST_PLAN],
        on: MOST_PLAN - 1,
    };
    let nows = vec![
        Now::Acting,
        Now::Paused,
        // Handed over to sign in (queue item 821).
        Now::SignIn,
        Now::Asking {
            what: "Pay on shop.example?".to_owned(),
            paying: Some(Box::new(Paying {
                amount: Some("UGX 45,000\0".to_owned()),
                merchant: "<b>Kampala</b> Coffee (shop.example)".to_owned(),
                lines: vec!["Flat white \u{d7} 2".to_owned(); MOST_SUMMARY],
            })),
        },
        Now::Asking {
            what: "Pay on shop.example?".to_owned(),
            paying: Some(Box::new(Paying {
                amount: None,
                merchant: "shop.example".to_owned(),
                lines: Vec::new(),
            })),
        },
    ];
    for now in nows {
        let original = card(AtWork {
            plan: Some(plan.clone()),
            now,
            ..AtWork::starting("send <it>")
        });
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
    for refused in [
        card(AtWork {
            plan: Some(Plan {
                steps: vec!["a".to_owned(); MOST_PLAN + 1],
                on: 0,
            }),
            ..AtWork::starting("x")
        }),
        card(AtWork {
            plan: Some(Plan {
                steps: vec!["a".to_owned(); 2],
                on: 2,
            }),
            ..AtWork::starting("x")
        }),
        card(AtWork {
            plan: Some(Plan {
                steps: vec!["w".repeat(PLAN_LINE + 1)],
                on: 0,
            }),
            ..AtWork::starting("x")
        }),
        card(AtWork {
            now: Now::Asking {
                what: String::new(),
                paying: Some(Box::new(Paying {
                    amount: None,
                    merchant: String::new(),
                    lines: vec!["a".to_owned(); MOST_SUMMARY + 1],
                })),
            },
            ..AtWork::starting("x")
        }),
        card(AtWork {
            now: Now::Asking {
                what: String::new(),
                paying: Some(Box::new(Paying {
                    amount: Some("9".repeat(PAYING_LINE + 1)),
                    merchant: String::new(),
                    lines: Vec::new(),
                })),
            },
            ..AtWork::starting("x")
        }),
    ] {
        let bytes = write_to_renderer(&refused);
        assert!(read_to_renderer(&bytes).is_err(), "{refused:?} was read");
    }
}

/// Queue items 798 and 810: alo's menu over a card's page and the closed
/// list the agent chooses in cross as they were sent, strangers' words
/// intact; a menu of more than four lines, a line of no kind, and a list
/// whose chosen option is past its options are refused.
#[test]
fn a_cards_menu_and_the_list_the_agent_chooses_in_cross_and_nonsense_is_refused() {
    use alo_renderer::agent_work::{AtWork, Dropdown, Now};
    use alo_renderer::card_menu::{CardMenu, MenuItem};
    let card = |menu: Option<CardMenu>, choosing: Option<Dropdown>| {
        let mut card = alo_renderer::card::Card::new(
            "a page",
            alo_url::parse("https://example.com/").unwrap_or_else(|_| panic!("parses")),
        );
        card.menu = menu;
        card.agent = Some(AtWork {
            plan: None,
            asked: "choose".to_owned(),
            steps: Vec::new(),
            now: Now::Acting,
            choosing,
        });
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Page(card)],
            ..Start::default()
        }))
    };
    let menu = CardMenu {
        at: (658, 370),
        items: vec![
            MenuItem::Copy { selected: true },
            MenuItem::CopyLink("https://a.example/\0</p>".to_owned()),
            MenuItem::OpenInNewChat("https://a.example/".to_owned()),
            MenuItem::Search("<b>words</b>\n".to_owned()),
        ],
    };
    let choosing = Dropdown {
        list: "Fruit </ul>".to_owned(),
        options: vec!["Apples".to_owned(), "Dates\0".to_owned()],
        chosen: 1,
    };
    let original = card(Some(menu.clone()), Some(choosing.clone()));
    let bytes = write_to_renderer(&original);
    let back = read_to_renderer(&bytes).unwrap_or_else(|why| panic!("did not survive: {why}"));
    assert_eq!(back, original);
    let mut five = menu.clone();
    five.items.push(MenuItem::Copy { selected: false });
    let mut past = choosing.clone();
    past.chosen = 2;
    for refused in [card(Some(five), None), card(None, Some(past))] {
        let bytes = write_to_renderer(&refused);
        assert!(read_to_renderer(&bytes).is_err(), "{refused:?} was read");
    }
}

/// Queue item 820: the runs of the agent kept with a page shown earlier
/// cross as they were sent — every kind of ending, as many steps as are
/// drawn — and more runs or steps than are drawn, or a card's runs on a
/// live card, are not carried.
#[test]
fn the_runs_kept_with_a_page_cross_and_too_many_are_refused() {
    use alo_renderer::agent_ran::{Ended, MOST_RUNS, Ran};
    use alo_renderer::agent_work::{MOST_STEPS, Step};
    let page = |ran: Vec<Ran>| {
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Visited {
                said: "a page".to_owned(),
                url: alo_url::parse("https://example.com/").unwrap_or_else(|_| panic!("parses")),
                ran,
            }],
            ..Start::default()
        }))
    };
    let step = Step {
        line: "Typing \u{201c}</li>\0\u{201d}".to_owned(),
        why: "<b>because</b>".to_owned(),
    };
    // Queue item 824: each kept with its plan as it last stood.
    let plan = alo_renderer::agent_plan::Plan {
        steps: vec!["Find </li> the form".to_owned(), "Send it".to_owned()],
        on: 1,
    };
    let ran = |ended: Ended, steps: usize| Ran {
        plan: Some(plan.clone()),
        asked: "send <it>".to_owned(),
        steps: vec![step.clone(); steps],
        ended,
    };
    let every = vec![
        ran(
            Ended::Done {
                said: "Sent.".to_owned(),
            },
            MOST_STEPS,
        ),
        ran(
            Ended::Stuck {
                why: "no form".to_owned(),
            },
            1,
        ),
        ran(
            Ended::Stopped {
                why: "You stopped alo.".to_owned(),
            },
            0,
        ),
    ];
    let original = page(every);
    let back = read_to_renderer(&write_to_renderer(&original))
        .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
    assert_eq!(back, original);
    let most = page(vec![Ran::starting("x"); MOST_RUNS]);
    assert!(read_to_renderer(&write_to_renderer(&most)).is_ok());
    for refused in [
        page(vec![Ran::starting("x"); MOST_RUNS + 1]),
        page(vec![Ran {
            plan: Some(alo_renderer::agent_plan::Plan {
                steps: vec!["a".to_owned()],
                on: 1,
            }),
            ..Ran::starting("x")
        }]),
        page(vec![ran(
            Ended::Done {
                said: String::new(),
            },
            MOST_STEPS + 1,
        )]),
    ] {
        assert!(
            read_to_renderer(&write_to_renderer(&refused)).is_err(),
            "{refused:?} was read"
        );
    }
    // A live card's runs are the browser process's: not carried.
    let mut card = alo_renderer::card::Card::new(
        "a page",
        alo_url::parse("https://example.com/").unwrap_or_else(|_| panic!("parses")),
    );
    card.ran = vec![Ran::starting("x")];
    let live = ToRenderer::Start(Box::new(Start {
        turns: vec![Turn::Page(card)],
        ..Start::default()
    }));
    let ToRenderer::Start(back) = read_to_renderer(&write_to_renderer(&live))
        .unwrap_or_else(|why| panic!("a live card did not survive: {why}"))
    else {
        panic!("not a first screen");
    };
    assert!(matches!(back.turns.first(), Some(Turn::Page(card)) if card.ran.is_empty()));
}

/// Queue item 703: a conversation of every kind of turn, oldest first,
/// scrolled either way, crosses as it was sent; and asking how far it
/// reaches, and the answer, do too.
#[test]
fn a_conversation_of_turns_and_its_reach_survive_the_crossing() {
    let site =
        |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written} parses"));
    let turns = vec![
        Turn::Searched(Results::searching("</p>\0")),
        Turn::Visited {
            said: "<b>said</b>".to_owned(),
            url: site("https://a.example/b?c#d"),
            ran: Vec::new(),
        },
        Turn::Failed {
            said: "b.example".to_owned(),
            why: "https://b.example/ could not be opened: no\0Chrome".to_owned(),
        },
        Turn::Page(
            alo_renderer::card::Card::new("c.example", site("https://c.example/"))
                .holding(b"\x89PNG\r\n\x1a\n".to_vec()),
        ),
    ];
    // With the sign of more above or below (queue item 755), each way.
    let bars = [
        None,
        Some(Bar {
            top: 240.515_63,
            length: 32.0,
            above: true,
            below: false,
            seen: 1.0,
        }),
        // Fading, as it does once the conversation has stopped (793).
        Some(Bar {
            top: 0.0,
            length: 675.5,
            above: false,
            below: true,
            seen: 0.375,
        }),
        None,
    ];
    for (down, bar) in [0.0, -1234.5, 0.015_625, 99_999.0].into_iter().zip(bars) {
        let original = ToRenderer::Start(Box::new(Start {
            turns: turns.clone(),
            down,
            bar,
            ..Start::default()
        }));
        let back = read_to_renderer(&write_to_renderer(&original))
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
    let asked = ToRenderer::ConversationReach;
    assert_eq!(read_to_renderer(&write_to_renderer(&asked)), Ok(asked));
    for keyed in [
        None,
        Some(alo_renderer::conversation_scroll::Span {
            top: 600.0,
            foot: 736.0,
        }),
    ] {
        let reached = FromRenderer::ConversationReached(
            alo_renderer::conversation_scroll::Reach {
                earlier: 1462.0,
                newest: 626.0,
            },
            keyed,
        );
        assert_eq!(
            read_from_renderer(&write_from_renderer(&reached)),
            Ok(reached)
        );
    }
}

/// Queue item 861: a field's editing and where its words were laid out
/// cross whole, and a number past any field is refused by name.
#[test]
fn a_fields_editing_and_its_laid_out_words_cross_and_hostile_ones_are_refused() {
    use alo_renderer::field::{Editing, FieldLaidOut};
    let asked = ToRenderer::FieldLaidOut;
    assert_eq!(read_to_renderer(&write_to_renderer(&asked)), Ok(asked));
    let laid = FieldLaidOut {
        left: 182.0,
        top: 270.5,
        height: 40.0,
        room: 614.0,
        scrolled: 21.5,
        stops: vec![(0, 0.0), (1, 9.25), (3, 18.0)],
    };
    for answer in [
        FromRenderer::FieldLaidOut(Some(laid.clone())),
        FromRenderer::FieldLaidOut(None),
    ] {
        assert_eq!(
            read_from_renderer(&write_from_renderer(&answer)),
            Ok(answer.clone())
        );
    }
    let refused_from = |answer: &FromRenderer| {
        read_from_renderer(&write_from_renderer(answer))
            .err()
            .map(|why| why.why)
            .unwrap_or_default()
    };
    // A length past any field, stops going back, and too many of them.
    let far = FromRenderer::FieldLaidOut(Some(FieldLaidOut {
        scrolled: 2.0e7,
        ..laid.clone()
    }));
    assert!(
        refused_from(&far).contains("scroll"),
        "{}",
        refused_from(&far)
    );
    let back = FromRenderer::FieldLaidOut(Some(FieldLaidOut {
        stops: vec![(4, 0.0), (2, 9.0)],
        ..laid.clone()
    }));
    assert!(refused_from(&back).contains("going back"));
    let many = FromRenderer::FieldLaidOut(Some(FieldLaidOut {
        stops: (0..=alo_renderer::typed::LONGEST_TYPED + 300)
            .map(|at| (at, 0.0))
            .collect(),
        ..laid
    }));
    assert!(
        refused_from(&many).contains("stops"),
        "{}",
        refused_from(&many)
    );
    // The panel's editing crosses as the first screen's does.
    let panel = ToRenderer::Panel(Box::new(Panel {
        searched: "lisbon".to_owned(),
        searching: true,
        editing: Editing::caret_at(2).selecting_from(6),
        ..Panel::default()
    }));
    assert_eq!(
        read_to_renderer(&write_to_renderer(&panel)),
        Ok(panel.clone())
    );
    let refused_to = |message: &ToRenderer| {
        read_to_renderer(&write_to_renderer(message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default()
    };
    // Scrolled less than nothing, a place past any field, and a
    // composition longer than any drawn.
    for (editing, said) in [
        (
            Editing {
                scrolled: -1.0,
                ..Editing::default()
            },
            "less than nothing",
        ),
        (Editing::caret_at(usize::MAX), "characters into a field"),
        (
            Editing {
                composing: Some(alo_renderer::field::Composing {
                    text: "あ".repeat(alo_renderer::field::LONGEST_COMPOSING + 1),
                    caret: 0,
                }),
                ..Editing::default()
            },
            "composition",
        ),
    ] {
        let start = ToRenderer::Start(Box::new(Start {
            editing,
            ..Start::default()
        }));
        let why = refused_to(&start);
        assert!(why.contains(said), "{said}: {why}");
    }
}

/// Queue item 755: a sign of more above or below placed past any room, or
/// before it, is refused by name.
#[test]
fn a_sign_of_more_placed_past_any_room_is_refused() {
    let card = Turn::Page(alo_renderer::card::Card::new(
        "a",
        alo_url::parse("https://a.example/").unwrap_or_else(|_| panic!("parses")),
    ));
    let refused = |message: &ToRenderer| {
        read_to_renderer(&write_to_renderer(message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default()
    };
    // A sign of more above or below placed past any room, or before it
    // (queue item 755).
    for (top, length) in [(2.0e6, 32.0), (-1.0, 32.0), (0.0, 2.0e6), (0.0, -0.5)] {
        let placed = ToRenderer::Start(Box::new(Start {
            turns: vec![card.clone()],
            bar: Some(Bar {
                top,
                length,
                above: true,
                below: true,
                seen: 1.0,
            }),
            ..Start::default()
        }));
        assert!(
            refused(&placed).contains("further than any room"),
            "{top} {length}: {}",
            refused(&placed)
        );
    }
    // Seen as other than a fraction (queue item 793).
    for (seen, why) in [
        (-0.01, "other than a fraction"),
        (1.5, "other than a fraction"),
        (f32::NAN, "not a number"),
        (f32::INFINITY, "not a number"),
    ] {
        let placed = ToRenderer::Start(Box::new(Start {
            turns: vec![card.clone()],
            bar: Some(Bar {
                top: 0.0,
                length: 32.0,
                above: false,
                below: true,
                seen,
            }),
            ..Start::default()
        }));
        assert!(
            refused(&placed).contains(why),
            "{seen}: {}",
            refused(&placed)
        );
    }
}

/// Queue item 703: a live card anywhere but the newest turn, a page shown
/// earlier somewhere not a site, more turns than are drawn, an unknown turn
/// and a conversation scrolled past any reach are refused by name; and a
/// renderer's reach past the most is read as the most.
#[test]
fn a_conversation_the_renderer_should_not_draw_is_refused() {
    let site =
        |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written} parses"));
    let card = || {
        Turn::Page(alo_renderer::card::Card::new(
            "a",
            site("https://a.example/"),
        ))
    };
    let start = |turns: Vec<Turn>, down: f32| {
        ToRenderer::Start(Box::new(Start {
            turns,
            down,
            ..Start::default()
        }))
    };
    let refused = |message: &ToRenderer| {
        read_to_renderer(&write_to_renderer(message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default()
    };
    let earlier = start(vec![card(), Turn::Searched(Results::searching("x"))], 0.0);
    assert!(
        refused(&earlier).contains("not the newest"),
        "{}",
        refused(&earlier)
    );
    let visited = start(
        vec![Turn::Visited {
            said: "x".to_owned(),
            url: site("file:///etc/passwd"),
            ran: Vec::new(),
        }],
        0.0,
    );
    assert!(
        refused(&visited).contains("not a site's"),
        "{}",
        refused(&visited)
    );
    let many = start(
        vec![Turn::Searched(Results::searching("x")); MOST_TURNS + 1],
        0.0,
    );
    assert!(refused(&many).contains("at most 500"), "{}", refused(&many));
    assert!(
        read_to_renderer(&write_to_renderer(&start(
            vec![Turn::Searched(Results::searching("x")); MOST_TURNS],
            0.0
        )))
        .is_ok()
    );
    let far = start(vec![card()], 2.0e6);
    assert!(
        refused(&far).contains("scrolled further"),
        "{}",
        refused(&far)
    );
    // A turn tagged as nothing this engine draws: the tag after the turns'
    // count, rewritten — back past *Settings*' bar's flag and its offset
    // (queue item 886), the bar's flag (none), the offset, the failure's
    // two empty strings and the tag itself.
    let mut bytes = write_to_renderer(&start(
        vec![Turn::Failed {
            said: String::new(),
            why: String::new(),
        }],
        0.0,
    ));
    let tag = bytes
        .len()
        .checked_sub(1 + 4 + 1 + 4 + 2 * 8 + 1)
        .unwrap_or_else(|| panic!("a turn was written"));
    let _ = bytes.get(tag).map(|written| assert_eq!(*written, 3));
    if let Some(written) = bytes.get_mut(tag) {
        *written = 9;
    }
    let why = read_to_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("a turn tagged 9"), "{why}");
}

/// A renderer's lengths, read inside the most: a reach, and where the
/// thing the keyboard is on stands (queue item 880), its foot never above
/// its top.
#[test]
fn a_renderers_reach_is_read_inside_the_most() {
    let reached = FromRenderer::ConversationReached(
        alo_renderer::conversation_scroll::Reach {
            earlier: 5.0e7,
            newest: -3.0,
        },
        Some(alo_renderer::conversation_scroll::Span {
            top: 40.0,
            foot: 10.0,
        }),
    );
    let Ok(FromRenderer::ConversationReached(read, keyed)) =
        read_from_renderer(&write_from_renderer(&reached))
    else {
        panic!("a reach crosses");
    };
    // A foot above its top is read as the top: never above it.
    assert_eq!(
        keyed.map(|span| (span.top.to_bits(), span.foot.to_bits())),
        Some((40.0_f32.to_bits(), 40.0_f32.to_bits()))
    );
    assert_eq!(
        read.earlier.to_bits(),
        alo_renderer::conversation_scroll::MOST.to_bits()
    );
    assert_eq!(read.newest.to_bits(), 0.0_f32.to_bits());
}

/// A press on the chats panel or the first screen, every claim its
/// renderer can answer with, and the renderer that holds neither part
/// (queue item 710).
#[test]
fn every_claim_on_the_conversation_survives_the_crossing() {
    let messages = vec![
        FromRenderer::Failed(Failure::NotTheConversation),
        // Every claim on a part of the conversation, each place at its
        // edges, and a press on nothing.
        FromRenderer::ConversationClaimed(None),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::NewChat)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Chat { at: 0 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Chat { at: usize::MAX })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Settings)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Site { at: 3 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::AddASite)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Waiting { at: 1 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::TheBox)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Go)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Back)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Forward)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Expand)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::CloseTheCard)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Found { at: 2 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Offer { at: 0 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Offer { at: usize::MAX })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::SearchChats)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::HideThePanel)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::ShowThePanel)),
        // Queue items 743 and 744.
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Tab { at: 5 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::MoreSites { at: 0 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::SeeAllImages { at: 1 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Image { at: usize::MAX })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::ShowMoreSites { at: 3 })),
        // Queue item 704: the agent's buttons and a stuck agent's choice.
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Stop)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Allow)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::NotNow)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Choice { at: 2 })),
        // Queue item 789: the hovered row's Delete, and the foot's Undo.
        FromRenderer::ConversationClaimed(Some(ConversationClaim::DeleteChat)),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Undo)),
        // Queue item 807: a card's shield.
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Shield)),
        // Queue item 791: an open row's ×.
        FromRenderer::ConversationClaimed(Some(ConversationClaim::CloseChat)),
        // Queue item 798: a line of alo's menu over the card's page.
        FromRenderer::ConversationClaimed(Some(ConversationClaim::MenuItem { at: 3 })),
        // Queue item 854: a face Settings offers, and its × (886).
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Face { at: 0 })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::Face { at: usize::MAX })),
        FromRenderer::ConversationClaimed(Some(ConversationClaim::LeaveSettings)),
    ];
    for original in messages {
        let bytes = write_from_renderer(&original);
        let back = read_from_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// Queue items 853 and 854: each face the interface may be set in, and
/// *Settings* on screen, cross to the panel's and the first screen's
/// renderers; a face tagged as none of the five is refused.
#[test]
fn every_face_and_settings_survive_the_crossing_and_an_unknown_face_is_refused() {
    for face in InterfaceFace::ALL {
        for settings in [false, true] {
            let messages = [
                ToRenderer::Panel(Box::new(Panel {
                    chats: vec![Chat::new("Invoice", "").current()],
                    viewport: Size::new(240.0, 820.0),
                    face,
                    settings,
                    ..Panel::default()
                })),
                ToRenderer::Start(Box::new(Start {
                    viewport: Size::new(1040.0, 820.0),
                    face,
                    settings,
                    ..Start::default()
                })),
            ];
            for original in messages {
                let bytes = write_to_renderer(&original);
                let back = read_to_renderer(&bytes)
                    .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
                assert_eq!(back, original, "{original:?} changed on the way across");
            }
        }
    }
    // The one byte the face is: found where Inter and Geist differ, and set
    // to a tag no face has.
    let of = |face| {
        write_to_renderer(&ToRenderer::Panel(Box::new(Panel {
            face,
            ..Panel::default()
        })))
    };
    let (inter, geist) = (of(InterfaceFace::Inter), of(InterfaceFace::Geist));
    let differ: Vec<usize> = (0..inter.len())
        .filter(|&at| inter.get(at) != geist.get(at))
        .collect();
    assert_eq!(differ.len(), 1, "{differ:?}");
    let mut unknown = inter.clone();
    if let Some(byte) = differ.first().and_then(|&at| unknown.get_mut(at)) {
        *byte = 5;
    }
    let refused = read_to_renderer(&unknown).map_err(|why| why.to_string());
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("a face tagged 5")),
        "{refused:?}"
    );
}

/// Every refusal a verb can meet, each of which an agent is told by name.
#[test]
fn every_refusal_survives_the_crossing() {
    let refusals = vec![
        FromRenderer::Refused(Refusal::NotFound {
            target: Target::Named("Nowhere".to_owned()),
        }),
        FromRenderer::Refused(Refusal::Ambiguous {
            target: Target::OfRole(Role::Known(KnownRole::Button)),
            candidates: vec![id(1), id(2), id(3)],
        }),
        FromRenderer::Refused(Refusal::NotOperable {
            node: id(8),
            role: Role::Presentational,
        }),
        FromRenderer::Refused(Refusal::Disabled { node: id(9) }),
        FromRenderer::Refused(Refusal::NotAField {
            node: id(10),
            role: Role::Generic,
        }),
        FromRenderer::Refused(Refusal::ReadOnly { node: id(11) }),
        FromRenderer::Refused(Refusal::DoesNotScroll { node: id(12) }),
        FromRenderer::Refused(Refusal::NotADate {
            node: id(13),
            text: "12/10/2026".to_owned(),
        }),
        FromRenderer::Refused(Refusal::Chooses { node: id(18) }),
        FromRenderer::Refused(Refusal::Unbuilt {
            node: id(14),
            kind: "time".to_owned(),
        }),
        FromRenderer::Refused(Refusal::TooLong {
            node: id(15),
            most: 3,
            length: 6,
        }),
        FromRenderer::Refused(Refusal::Unchecked {
            node: id(16),
            control: Some(id(2)),
            constraint: Constraint::PatternMismatch,
        }),
        FromRenderer::Refused(Refusal::Unchecked {
            node: id(16),
            control: None,
            constraint: Constraint::TooShort,
        }),
        FromRenderer::Refused(Refusal::NotSubmittable {
            node: id(17),
            why: "the form closes a dialog, and dialogs are not built".to_owned(),
        }),
    ];
    for original in refusals {
        let bytes = write_from_renderer(&original);
        let back = read_from_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
}

/// A role this engine does not know still reaches an agent, because a role is
/// what an interface *declared* itself to be (ADR 0002) and dropping the ones
/// we have no name for would silently narrow what an agent can see.
#[test]
fn a_role_this_engine_does_not_know_crosses_intact() {
    let message = FromRenderer::Refused(Refusal::NotOperable {
        node: id(1),
        role: Role::Declared("invented-last-tuesday".into()),
    });
    let back =
        read_from_renderer(&write_from_renderer(&message)).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(back, message);
}

/// Queue item 701: the first screen is told to offer *Go to the site* only
/// for a site's address. One for anything else does not survive the wire,
/// whoever wrote it.
#[test]
fn an_offer_to_go_somewhere_that_is_not_a_site_is_refused() {
    for written in ["file:///etc/passwd", "javascript:alert(1)", "data:,x"] {
        let Ok(url) = alo_url::parse(written) else {
            panic!("{written} parses");
        };
        let message = ToRenderer::Start(Box::new(Start {
            offers: vec![Offer::Go(url)],
            ..Start::default()
        }));
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains("not a site's"), "{written}: {why:?}");
    }
}

/// Queue item 717: a card is drawn only for a site's address, and its
/// frame only when it is a PNG — the one format Chrome is asked for.
#[test]
fn a_card_for_somewhere_not_a_site_or_holding_anything_but_a_png_is_refused() {
    let card = |written: &str, frame: &[u8]| {
        let url = alo_url::parse(written).unwrap_or_else(|_| panic!("{written} parses"));
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Page(
                alo_renderer::card::Card::new("a", url).holding(frame.to_vec()),
            )],
            ..Start::default()
        }))
    };
    let png = b"\x89PNG\r\n\x1a\n";
    for (message, refusal) in [
        (card("file:///etc/passwd", png), "not a site's"),
        (card("javascript:alert(1)", png), "not a site's"),
        (card("https://example.com/", b"GIF89a"), "not a PNG"),
        (card("https://example.com/", b"<svg/>"), "not a PNG"),
        (card("https://example.com/", b""), "not a PNG"),
    ] {
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{message:?}: {why:?}");
    }
}

/// Queue item 708: a site found is drawn as a link only for a site's
/// address, and no more sites than one page of results holds cross.
#[test]
fn a_site_found_somewhere_not_a_site_or_too_many_is_refused() {
    let found = |written: &str, count: usize| {
        let url = alo_url::parse(written).unwrap_or_else(|_| panic!("{written} parses"));
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(Results::of(
                "x".to_owned(),
                Showing::Found(vec![Found::new("t", url, "d"); count]),
            ))],
            ..Start::default()
        }))
    };
    for (message, refusal) in [
        (found("file:///etc/passwd", 1), "not a site's"),
        (found("javascript:alert(1)", 1), "not a site's"),
        (
            found(
                "https://example.com/",
                alo_renderer::results::MOST_FOUND + 1,
            ),
            "at most 20",
        ),
    ] {
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{why:?}");
    }
    assert!(read_to_renderer(&write_to_renderer(&found("https://example.com/", 20))).is_ok());
}

/// Queue items 743 and 744: a search's images — asked, found with and
/// without thumbnails, not answered — and the tab shown cross as they were
/// sent.
#[test]
fn a_searchs_images_and_tab_survive_the_crossing() {
    let site = |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written}"));
    let sites = || Showing::Found(vec![Found::new("t", site("https://a.example/"), "d")]);
    let found = Images::Found(vec![
        Pictured::new(
            "</a><img src=x>\u{202e}",
            site("https://p.example/a"),
            "p.example",
        )
        .holding(b"\x89PNG\r\n\x1a\nand the rest".to_vec()),
        Pictured::new("no picture", site("http://q.example/"), ""),
        Pictured::new("a JPEG", site("https://r.example/"), "r").holding(vec![0xff, 0xd8, 0xff, 0]),
    ]);
    for results in [
        Results::of("x", sites()).with_images(Images::Asking),
        Results::of("x", sites()).with_images(found.clone()),
        Results::of("x", sites()).with_images(found).on(Tab::Images),
        Results::of("x", sites()).on(Tab::Sites),
        Results::of("x", sites())
            .with_images(Images::Unanswered(
                "Brave Search could not answer (503).".to_owned(),
            ))
            .on(Tab::Images),
    ] {
        let original = ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(results)],
            ..Start::default()
        }));
        let back = read_to_renderer(&write_to_renderer(&original))
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
}

/// Queue item 743: an image is drawn only for a page at a site's address,
/// no more than one search holds, each thumbnail a picture this engine
/// reads and no larger than a thumbnail is, all of them within one first
/// screen's budget; and a tab is shown only when it can be (744).
#[test]
fn images_that_should_not_be_drawn_are_refused() {
    let site = |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written}"));
    let sending = |results: Results| {
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(results)],
            ..Start::default()
        }))
    };
    let png = |bytes: usize| {
        let mut picture = b"\x89PNG\r\n\x1a\n".to_vec();
        picture.resize(bytes, 0);
        picture
    };
    let with = |images: Vec<Pictured>| {
        sending(
            Results::of(
                "x",
                Showing::Found(vec![Found::new("t", site("https://a.example/"), "")]),
            )
            .with_images(Images::Found(images))
            .on(Tab::Images),
        )
    };
    let one =
        |page: &str, picture: Vec<u8>| vec![Pictured::new("t", site(page), "").holding(picture)];
    for (message, refusal) in [
        (with(one("javascript:x()", png(20))), "not a site's"),
        (
            with(one("https://p.example/", b"<svg>".to_vec())),
            "not a picture",
        ),
        (
            with(one("https://p.example/", png(LARGEST_THUMBNAIL + 1))),
            "at most 262144",
        ),
        (
            with(vec![
                Pictured::new("t", site("https://p.example/"), "");
                MOST_IMAGES + 1
            ]),
            "at most 20",
        ),
        (
            sending(
                Results::of(
                    "x",
                    Showing::Found(vec![Found::new("t", site("https://a.example/"), "")]),
                )
                .on(Tab::Images),
            ),
            "which they cannot be",
        ),
        (
            sending(Results::searching("x").on(Tab::Sites)),
            "which they cannot be",
        ),
    ] {
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{refusal}: {why:?}");
    }
    // Two searches of twenty thumbnails at the largest are more than one
    // first screen carries.
    let heavy = || {
        Turn::Searched(
            Results::of(
                "x",
                Showing::Found(vec![Found::new("t", site("https://a.example/"), "")]),
            )
            .with_images(Images::Found(
                (0..MOST_IMAGES)
                    .map(|_| {
                        Pictured::new("t", site("https://p.example/"), "")
                            .holding(png(LARGEST_THUMBNAIL))
                    })
                    .collect(),
            ))
            .on(Tab::Images),
        )
    };
    let flood = ToRenderer::Start(Box::new(Start {
        turns: vec![heavy()],
        ..Start::default()
    }));
    let why = read_to_renderer(&write_to_renderer(&flood))
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("bytes of pictures"), "{why:?}");
    // Within the budget, as the browser process trims it, it crosses.
    let trimmed = ToRenderer::Start(Box::new(Start {
        turns: alo_renderer::results::within_budget(&[heavy(), heavy()]),
        ..Start::default()
    }));
    assert!(read_to_renderer(&write_to_renderer(&trimmed)).is_ok());
}

/// Queue items 745 and 746: a search's videos and news — asked, found,
/// not answered — and their tabs cross as they were sent.
#[test]
fn a_searchs_videos_and_news_survive_the_crossing() {
    use alo_renderer::results::{Stories, Videos};
    use alo_renderer::results_media::{Story, Video};
    let site = |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written}"));
    let sites = || Showing::Found(vec![Found::new("t", site("https://a.example/"), "d")]);
    let videos = Videos::Found(vec![
        Video::new("</a><img src=x>\u{202e}", site("https://v.example/w?v=1"))
            .lasting("1:02:03")
            .from_site("YouTube", "2 days ago")
            .said_of("<b>said</b>")
            .holding(b"\x89PNG\r\n\x1a\nand the rest".to_vec()),
        Video::new("no picture", site("http://w.example/")),
    ]);
    let news = Stories::Found(vec![
        Story::new("<script>x()</script>", site("https://n.example/a"))
            .from_site("Daily", "1 week ago")
            .said_of("d")
            .holding(b"\xff\xd8\xffa photo".to_vec()),
        Story::new("no photo", site("https://m.example/")),
    ]);
    for results in [
        Results::of("x", sites())
            .with_videos(Videos::Asking)
            .on(Tab::Videos),
        Results::of("x", sites())
            .with_videos(videos.clone())
            .on(Tab::Videos),
        Results::of("x", sites())
            .with_news(news.clone())
            .on(Tab::News),
        Results::of("x", sites())
            .with_news(Stories::Asking)
            .on(Tab::News),
        Results::of("x", sites())
            .with_videos(videos)
            .with_news(news)
            .on(Tab::Sites),
        Results::of("x", sites())
            .with_news(Stories::Unanswered(
                "Brave Search could not answer (503).".to_owned(),
            ))
            .on(Tab::News),
    ] {
        let original = ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(results)],
            ..Start::default()
        }));
        let back = read_to_renderer(&write_to_renderer(&original))
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
}

/// Queue items 745, 746 and 765: a video or story is drawn only for a page at a
/// site's address, no more than one search holds, a video's length only a
/// length and its picture only a picture no larger than a thumbnail; and
/// Videos or News shown only once sites were found.
#[test]
fn videos_and_news_that_should_not_be_drawn_are_refused() {
    use alo_renderer::results::{Stories, Videos};
    use alo_renderer::results_media::{MOST_STORIES, MOST_VIDEOS, Story, Video};
    let site = |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written}"));
    let found = || Showing::Found(vec![Found::new("t", site("https://a.example/"), "")]);
    let sending = |results: Results| {
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(results)],
            ..Start::default()
        }))
    };
    let videos = |each: Vec<Video>| {
        sending(
            Results::of("x", found())
                .with_videos(Videos::Found(each))
                .on(Tab::Videos),
        )
    };
    let mut long = Video::new("t", site("https://v.example/"));
    long.length = "4m05s".to_owned();
    let mut big = b"\x89PNG\r\n\x1a\n".to_vec();
    big.resize(LARGEST_THUMBNAIL + 1, 0);
    for (message, refusal) in [
        (
            videos(vec![Video::new("t", site("javascript:x()"))]),
            "a video found on a page that is not a site's",
        ),
        (videos(vec![long]), "a video whose length is not a length"),
        (
            videos(vec![
                Video::new("t", site("https://v.example/")).holding(b"<svg>".to_vec()),
            ]),
            "not a picture",
        ),
        (
            videos(vec![
                Video::new("t", site("https://v.example/")).holding(big),
            ]),
            "at most 262144",
        ),
        (
            videos(vec![
                Video::new("t", site("https://v.example/"));
                MOST_VIDEOS + 1
            ]),
            "21 videos found, and at most 20",
        ),
        (
            sending(
                Results::of("x", found())
                    .with_news(Stories::Found(vec![Story::new("t", site("file:///x"))]))
                    .on(Tab::News),
            ),
            "a news story found on a page that is not a site's",
        ),
        (
            sending(
                Results::of("x", found())
                    .with_news(Stories::Found(vec![
                        Story::new("t", site("https://n.example/")).holding(b"<html>".to_vec()),
                    ]))
                    .on(Tab::News),
            ),
            "a thumbnail that is not a picture",
        ),
        (
            sending(
                Results::of("x", found())
                    .with_news(Stories::Found(vec![
                        Story::new(
                            "t",
                            site("https://n.example/")
                        );
                        MOST_STORIES + 1
                    ]))
                    .on(Tab::News),
            ),
            "21 news stories found, and at most 20",
        ),
        (
            sending(Results::searching("x").on(Tab::Videos)),
            "which they cannot be",
        ),
        (
            sending(Results::of("x", Showing::Found(Vec::new())).on(Tab::News)),
            "which they cannot be",
        ),
    ] {
        let why = read_to_renderer(&write_to_renderer(&message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{refusal}: {why:?}");
    }
}

// --- What a hostile renderer can put on the pipe -----------------------------

/// The line the decoder is built around. Four bytes saying "a gigabyte
/// follows" must not become a gigabyte of allocation.
#[test]
fn a_length_longer_than_the_message_is_refused_before_anything_is_reserved() {
    // `Loaded` with a count of a billion issues, and nothing after it.
    let mut bytes = vec![0u8];
    bytes.extend_from_slice(&1_000_000_000u64.to_be_bytes());
    let why = read_from_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("no room for them"), "{why:?}");

    // And a string inside a message, saying the same thing.
    let mut bytes = vec![5u8, 1u8];
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(read_from_renderer(&bytes).is_err());
}

/// The families a renderer says a page wanted are the same kind of hazard as
/// its issues, and they are a second list in the same message — so the same
/// question is asked of it separately, because a bound that was only ever
/// applied to the first list is a bound the second one does not have.
#[test]
fn a_count_of_families_larger_than_the_message_is_refused() {
    // `Loaded`, no issues, and then a billion families.
    let mut bytes = vec![0u8];
    bytes.extend_from_slice(&0u64.to_be_bytes());
    bytes.extend_from_slice(&1_000_000_000u64.to_be_bytes());
    let why = read_from_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("no room for them"), "{why:?}");
}

/// What the generics mean is a list of pairs, and a count of pairs is the same
/// hazard as a count of anything else.
///
/// Asked in **both** directions. The browser process is the trusted side, and
/// that is exactly the argument that leaves a decoder unchecked until the day
/// something else is speaking on that pipe.
#[test]
fn a_count_of_generic_families_larger_than_any_mapping_is_refused() {
    // To a renderer: tag 6, then a billion pairs.
    let mut bytes = vec![6u8];
    bytes.extend_from_slice(&1_000_000_000u64.to_be_bytes());
    assert!(read_to_renderer(&bytes).is_err());

    // A count small enough to fit in the message and larger than any honest
    // mapping — the case a length check alone would let through.
    let mut bytes = vec![6u8];
    bytes.extend_from_slice(&((MOST_PAIRS + 1) as u64).to_be_bytes());
    bytes.extend_from_slice(&[0u8; 256]);
    let why = read_to_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("more than the"), "{why:?}");

    // And back: tag 7, then a billion generics answered.
    let mut bytes = vec![7u8];
    bytes.extend_from_slice(&1_000_000_000u64.to_be_bytes());
    assert!(read_from_renderer(&bytes).is_err());

    let mut bytes = vec![7u8];
    bytes.extend_from_slice(&((MOST_PAIRS + 1) as u64).to_be_bytes());
    bytes.extend_from_slice(&[0u8; 256]);
    assert!(read_from_renderer(&bytes).is_err());
}

/// Every prefix of a mapping is a mapping that did not arrive, and half a pair
/// is the interesting prefix: a decoder that read the generic and stopped would
/// hand up a mapping saying `sans-serif` means nothing at all.
#[test]
fn a_mapping_that_stops_part_way_through_is_refused() {
    let whole = write_to_renderer(&ToRenderer::UseGenerics(Generics::stating(vec![
        ("sans-serif".to_owned(), "Helvetica Neue".to_owned()),
        ("serif".to_owned(), "Times".to_owned()),
    ])));
    for cut in 1..whole.len() {
        assert!(
            read_to_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
            "{cut} bytes of a mapping were read as a whole one"
        );
    }
    // And every single flipped bit is an answer rather than a crash.
    for at in 0..whole.len() {
        for bit in 0..8 {
            let mut flipped = whole.clone();
            if let Some(byte) = flipped.get_mut(at) {
                *byte ^= 1 << bit;
            }
            let _ = read_to_renderer(&flipped);
        }
    }
}

/// ADR 0020: where a page asked to go crosses in a load's answer and in an
/// act's, with every referrer policy and either claim of how it arose.
#[test]
fn where_a_page_asked_to_go_survives_the_crossing() {
    let policies = [
        None,
        Some(Policy::NoReferrer),
        Some(Policy::NoReferrerWhenDowngrade),
        Some(Policy::Origin),
        Some(Policy::OriginWhenCrossOrigin),
        Some(Policy::SameOrigin),
        Some(Policy::StrictOrigin),
        Some(Policy::StrictOriginWhenCrossOrigin),
        Some(Policy::UnsafeUrl),
    ];
    for (at, referrer) in policies.into_iter().enumerate() {
        let navigation = Some(Asked {
            url: "https://example.com/next".to_owned(),
            by: if at % 2 == 0 { By::Script } else { By::Browser },
            referrer,
            replaced: u32::MAX - 1,
            submission: None,
        });
        for original in [
            FromRenderer::Loaded {
                issues: Vec::new(),
                wanted: Vec::new(),
                objections: Vec::new(),
                navigation: navigation.clone(),
                fetches: Vec::new(),
                sheets: Vec::new(),
                left: Vec::new(),
                title: String::new(),
            },
            FromRenderer::Acted {
                outcome: Outcome::Followed {
                    node: id(4),
                    to: "/next".to_owned(),
                },
                issues: vec![
                    "click: uncaught: Error: a listener threw".to_owned(),
                    String::new(),
                ],
                objections: Vec::new(),
                navigation,
                fetches: Vec::new(),
                sheets: Vec::new(),
            },
        ] {
            let back = read_from_renderer(&write_from_renderer(&original));
            assert_eq!(back.as_ref(), Ok(&original), "{original:?}");
        }
    }
}

/// Every prefix of a load is a load that did not arrive.
///
/// Asked of this message in particular because it is the one that grew a second
/// list: a decoder that read the issues and then stopped would hand up a load
/// with an empty `wanted` rather than an error, which reads exactly like a page
/// that asked for nothing.
#[test]
fn a_load_that_stops_part_way_through_is_refused() {
    let whole = write_from_renderer(&FromRenderer::Loaded {
        issues: vec!["refused `float: left`".to_owned()],
        wanted: vec!["Inter".to_owned()],
        objections: vec![Objection {
            policy: 1,
            kind: Inline::Script,
            placement: Placement::Element,
        }],
        navigation: Some(Asked {
            url: "https://example.com/next".to_owned(),
            by: By::Script,
            referrer: Some(Policy::Origin),
            replaced: 1,
            submission: None,
        }),
        fetches: Vec::new(),
        sheets: Vec::new(),
        left: Vec::new(),
        title: String::new(),
    });
    for cut in 1..whole.len() {
        assert!(
            read_from_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
            "{cut} bytes of a load were read as a whole one"
        );
    }
}

/// A tree arrives as a recursive structure. A decoder that recursed as deeply
/// as it was told would run out of stack on a message — which is a crash in the
/// **browser** process, caused by the renderer, which is the one thing ADR 0005
/// says must never happen.
#[test]
fn a_tree_deeper_than_this_engine_reads_is_refused_rather_than_recursed_into() {
    let mut deepest = a_node(vec![]);
    for _ in 0..(DEEPEST_TREE + 50) {
        deepest = a_node(vec![deepest]);
    }
    let message = FromRenderer::Tree(Box::new(Snapshot {
        root: Some(deepest),
    }));
    let bytes = write_from_renderer(&message);
    let why = read_from_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(
        why.contains("deeper than"),
        "a very deep tree was read: {why:?}"
    );
}

/// The cross-check the encoding cannot do on its own: a frame whose size and
/// pixels disagree is a frame something above would read past the end of.
#[test]
fn a_frame_whose_size_and_pixels_disagree_is_refused() {
    let mut bytes = vec![1u8];
    bytes.extend_from_slice(&1000u64.to_be_bytes()); // width
    bytes.extend_from_slice(&1000u64.to_be_bytes()); // height
    bytes.extend_from_slice(&4u64.to_be_bytes()); // four bytes of pixels
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    let why = read_from_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("rather than"), "{why:?}");
}

/// Every comparison against a NaN answers false, which turns a bounds check
/// into a thing that passes.
#[test]
fn a_number_that_is_not_a_number_is_refused() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bytes = vec![1u8]; // Resize
        bytes.extend_from_slice(&bad.to_be_bytes());
        bytes.extend_from_slice(&100f32.to_be_bytes());
        assert!(
            read_to_renderer(&bytes).is_err(),
            "{bad} was accepted as a window size"
        );
    }
}

/// A screen's scale crosses with the page it is laid out at (ADR 0057 § 3).
/// One that is not a number is refused, as a window's size would be; one that
/// is a number but not a factor above zero is read as one, which is how the
/// page would have been laid out had nobody said.
#[test]
fn a_scale_that_is_not_a_scale_is_refused_or_read_as_one() {
    let written = write_to_renderer(&ToRenderer::Load(Box::new(
        Page::new("<p>hi</p>", Size::new(123.25, 456.5)).at_scale(2.0),
    )));
    let needle: Vec<u8> = [123.25_f32, 456.5, 2.0]
        .iter()
        .flat_map(|number| number.to_be_bytes())
        .collect();
    let at = written
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| at + 8)
        .expect("the size and the scale, written together");
    let read_with = |scale: f32| {
        let mut bytes = written.clone();
        bytes.splice(at..at + 4, scale.to_be_bytes());
        read_to_renderer(&bytes)
    };
    let scale_read = |scale: f32| match read_with(scale) {
        Ok(ToRenderer::Load(page)) => Some(page.scale.to_bits()),
        _ => None,
    };
    assert_eq!(scale_read(2.0), Some(2.0_f32.to_bits()));
    assert_eq!(scale_read(1.5), Some(1.5_f32.to_bits()), "not rounded");
    for not_a_factor in [0.0, -0.0, -2.0, f32::MIN] {
        assert_eq!(
            scale_read(not_a_factor),
            Some(1.0_f32.to_bits()),
            "{not_a_factor} was read as a scale"
        );
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(read_with(bad).is_err(), "{bad} was accepted as a scale");
    }
}

/// A resize carries its screen's scale after its size, read as a load's is.
#[test]
fn a_resizes_scale_that_is_not_a_scale_is_refused_or_read_as_one() {
    let resize_at = |scale: f32| {
        let mut bytes = vec![1];
        for number in [320.0_f32, 200.0, scale] {
            bytes.extend_from_slice(&number.to_be_bytes());
        }
        read_to_renderer(&bytes)
    };
    let scale_read = |scale: f32| match resize_at(scale) {
        Ok(ToRenderer::Resize(screen)) => {
            assert_eq!(screen.size, Size::new(320.0, 200.0));
            Some(screen.scale().to_bits())
        }
        _ => None,
    };
    assert_eq!(scale_read(2.0), Some(2.0_f32.to_bits()));
    assert_eq!(scale_read(1.5), Some(1.5_f32.to_bits()), "not rounded");
    for not_a_factor in [0.0, -0.0, -2.0, f32::MIN] {
        assert_eq!(
            scale_read(not_a_factor),
            Some(1.0_f32.to_bits()),
            "{not_a_factor} was read as a scale"
        );
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(resize_at(bad).is_err(), "{bad} was accepted as a scale");
    }
    // A resize that stops before its scale is refused, not read at one.
    let mut cut = vec![1];
    for number in [320.0_f32, 200.0] {
        cut.extend_from_slice(&number.to_be_bytes());
    }
    assert!(read_to_renderer(&cut).is_err());
}

/// Trailing bytes mean the two ends disagree about the message, and a decoder
/// that ignored them would let a sender append something a later version reads.
#[test]
fn anything_left_over_after_a_message_is_refused() {
    let mut bytes = write_to_renderer(&ToRenderer::Paint);
    bytes.push(0);
    let why = read_to_renderer(&bytes)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("left over"), "{why:?}");
}

#[test]
fn a_message_that_stops_in_the_middle_is_refused() {
    let whole = write_from_renderer(&FromRenderer::Acted {
        outcome: Outcome::Followed {
            node: id(4),
            to: "https://example.com/next".to_owned(),
        },
        issues: Vec::new(),
        objections: Vec::new(),
        navigation: Some(Asked {
            url: "https://example.com/next".to_owned(),
            by: By::Browser,
            referrer: None,
            replaced: 0,
            submission: None,
        }),
        fetches: Vec::new(),
        sheets: Vec::new(),
    });
    for cut in 1..whole.len() {
        assert!(
            read_from_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
            "{cut} bytes of a message were read as a whole one"
        );
    }
}

#[test]
fn a_tag_nobody_knows_is_refused_rather_than_ignored() {
    for bytes in [vec![250u8], vec![4u8, 250u8], vec![0u8]] {
        assert!(
            read_from_renderer(&bytes).is_err() || read_to_renderer(&bytes).is_err(),
            "{bytes:?} was accepted"
        );
    }
    let why = read_to_renderer(&[99u8])
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("tagged 99"), "{why:?}");
}

/// Whether a value is in question is one of three answers (ADR 0051 § 6);
/// a fourth is refused by name rather than read as one of them.
#[test]
fn an_invalid_state_nobody_knows_is_refused() {
    let tree = |invalid| {
        let mut node = a_node(Vec::new());
        node.states.invalid = invalid;
        write_from_renderer(&FromRenderer::Tree(Box::new(Snapshot { root: Some(node) })))
    };
    let unknown = tree(Some(invalid::Invalid::Unknown));
    let valid = tree(None);
    // The two differ in the one byte that says it.
    let at: Vec<usize> = (0..unknown.len())
        .filter(|at| unknown.get(*at) != valid.get(*at))
        .collect();
    assert_eq!((at.len(), unknown.len()), (1, valid.len()), "{at:?}");
    let mut wrong = unknown;
    if let Some(byte) = at.first().and_then(|at| wrong.get_mut(*at)) {
        *byte = 3;
    }
    let why = read_from_renderer(&wrong)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("an invalid state tagged 3"), "{why:?}");
}

/// Whether a page can be seen is one of two states, on its own and with a
/// page (ADR 0039 § 1); a third, or none, is refused by name.
#[test]
fn a_visibility_state_nobody_knows_is_refused() {
    let why = read_to_renderer(&[9u8, 2u8])
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("a visibility state tagged 2"), "{why:?}");
    assert!(read_to_renderer(&[9u8]).is_err(), "a state that never came");
    assert!(
        read_to_renderer(&[9u8, 1u8, 0u8]).is_err(),
        "a byte after it"
    );

    let mut load = write_to_renderer(&ToRenderer::Load(Box::new(Page::new(
        "<p>hi</p>",
        Size::new(1.0, 1.0),
    ))));
    if let Some(last) = load.last_mut() {
        *last = 0xff;
    }
    let why = read_to_renderer(&load)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("a visibility state tagged 255"), "{why:?}");
    load.pop();
    assert!(read_to_renderer(&load).is_err(), "a page that does not say");
}

/// Nothing at all is not a message.
#[test]
fn an_empty_message_is_refused() {
    assert!(read_to_renderer(&[]).is_err());
    assert!(read_from_renderer(&[]).is_err());
}

/// Queue item 237: every objection a renderer says is a report the browser
/// process posts, so a load claiming more than an honest renderer sends is
/// refused whole rather than read and trimmed — it is the page talking.
#[test]
fn a_load_claiming_more_objections_than_one_may_carry_is_refused() {
    let honest = FromRenderer::Loaded {
        issues: Vec::new(),
        wanted: Vec::new(),
        objections: vec![
            Objection {
                policy: 0,
                kind: Inline::Script,
                placement: Placement::Element,
            };
            MOST_OBJECTIONS
        ],
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
        left: Vec::new(),
        title: String::new(),
    };
    assert_eq!(
        read_from_renderer(&write_from_renderer(&honest)).as_ref(),
        Ok(&honest)
    );
    let flood = FromRenderer::Loaded {
        issues: Vec::new(),
        wanted: Vec::new(),
        objections: vec![
            Objection {
                policy: 0,
                kind: Inline::Script,
                placement: Placement::Element,
            };
            MOST_OBJECTIONS + 1
        ],
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
        left: Vec::new(),
        title: String::new(),
    };
    let refused = read_from_renderer(&write_from_renderer(&flood));
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.why.contains("objections")),
        "{refused:?}"
    );
}

/// Queue item 528: a load's title crosses whole, markup and all, up to what
/// the strip draws, and a renderer claiming a longer one is refused rather
/// than cut — an honest renderer never sends one.
#[test]
fn a_loads_title_crosses_as_text_and_no_longer_than_the_strip_draws() {
    let loaded = |title: String| FromRenderer::Loaded {
        issues: Vec::new(),
        wanted: Vec::new(),
        objections: Vec::new(),
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
        left: Vec::new(),
        title,
    };
    for title in [
        String::new(),
        "<b>bold</b> & </style><script>steal()</script>".to_owned(),
        "a\0b".to_owned(),
        "\u{202e}gnp.exe".to_owned(),
        "é".repeat(LONGEST_TITLE),
    ] {
        let honest = loaded(title);
        assert_eq!(
            read_from_renderer(&write_from_renderer(&honest)).as_ref(),
            Ok(&honest)
        );
    }
    let flood = write_from_renderer(&loaded("x".repeat(LONGEST_TITLE + 1)));
    let refused = read_from_renderer(&flood);
    assert!(
        refused.as_ref().is_err_and(|why| why.why.contains("title")),
        "{refused:?}"
    );
    // Cut anywhere, it is refused too, as every message is.
    let whole = write_from_renderer(&loaded("Invoices".to_owned()));
    for at in 0..whole.len() {
        assert!(
            whole
                .get(..at)
                .is_some_and(|cut| read_from_renderer(cut).is_err()),
            "cut at {at}"
        );
    }
}

/// Queue item 346: an act and a delivery carry what a later draw objected
/// to, across the boundary and under the bound a load's are.
#[test]
fn an_act_or_a_delivery_carries_objections_and_no_more_than_one_may() {
    let style = |many| {
        vec![
            Objection {
                policy: 2,
                kind: Inline::Style,
                placement: Placement::Attribute,
            };
            many
        ]
    };
    let acted = |objections| FromRenderer::Acted {
        outcome: Outcome::Activated {
            node: id(2),
            name: None,
        },
        issues: vec!["1 more policy objections".to_owned()],
        objections,
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
    };
    let delivered = |objections| FromRenderer::Delivered {
        issues: Vec::new(),
        objections,
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
    };
    for honest in [
        acted(style(1)),
        acted(style(MOST_OBJECTIONS)),
        delivered(style(1)),
        delivered(style(MOST_OBJECTIONS)),
    ] {
        assert_eq!(
            read_from_renderer(&write_from_renderer(&honest)).as_ref(),
            Ok(&honest)
        );
    }
    for flood in [
        acted(style(MOST_OBJECTIONS + 1)),
        delivered(style(MOST_OBJECTIONS + 1)),
    ] {
        let refused = read_from_renderer(&write_from_renderer(&flood));
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.why.contains("objections in one answer")),
            "{refused:?}"
        );
    }
    let whole = write_from_renderer(&delivered(style(1)));
    for cut in 1..whole.len() {
        assert!(
            read_from_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
            "{cut} bytes of a delivery were read as a whole one"
        );
    }
}

/// An objection is a number and two tags, and all are a stranger's: inline
/// content of a kind nobody has is refused, so is a placement nobody has,
/// and so is a place no machine could index.
#[test]
fn an_objection_that_is_not_one_is_refused() {
    let one = write_from_renderer(&FromRenderer::Loaded {
        issues: Vec::new(),
        wanted: Vec::new(),
        objections: vec![Objection {
            policy: 0,
            kind: Inline::Style,
            placement: Placement::Element,
        }],
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
        left: Vec::new(),
        title: String::new(),
    });
    // From the end: eight bytes saying the title is empty, eight counting no
    // fetches by a page left, eight counting no style sheets, eight counting
    // no fetches, one saying there is no navigation, and before them the
    // placement's tag and the kind's.
    let mut strange = one.clone();
    let at = strange.len() - 35;
    if let Some(kind) = strange.get_mut(at) {
        *kind = 9;
    }
    let refused = read_from_renderer(&strange);
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.why.contains("tagged 9")),
        "{refused:?}"
    );
    let mut misplaced = one.clone();
    let at = misplaced.len() - 34;
    if let Some(placement) = misplaced.get_mut(at) {
        *placement = 7;
    }
    let refused = read_from_renderer(&misplaced);
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.why.contains("placed by a tag 7")),
        "{refused:?}"
    );
    let mut longer = one;
    longer.push(0);
    assert!(
        read_from_renderer(&longer).is_err(),
        "a load with bytes after its objections was read as a whole one"
    );
}

/// ADR 0020: where a page asked to go crosses as a claim, and every part of
/// it is a stranger's. A cause or a referrer policy nobody has is refused, so
/// is a count no task could make; and the URL is carried as text, never
/// parsed or trusted here — the browser process does that when it decides.
#[test]
fn a_navigation_that_is_not_one_is_refused() {
    let asked = |url: &str| FromRenderer::Acted {
        outcome: Outcome::Activated {
            node: id(1),
            name: None,
        },
        issues: Vec::new(),
        objections: Vec::new(),
        navigation: Some(Asked {
            url: url.to_owned(),
            by: By::Script,
            referrer: Some(Policy::SameOrigin),
            replaced: 0,
            submission: None,
        }),
        fetches: Vec::new(),
        sheets: Vec::new(),
    };
    // Nonsense crosses as text: refusing it is the browser process's.
    let nonsense = asked("javascript:alert(1)");
    assert_eq!(
        read_from_renderer(&write_from_renderer(&nonsense)).as_ref(),
        Ok(&nonsense)
    );

    let whole = write_from_renderer(&asked("https://example.com/"));
    // From the end: eight bytes counting no style sheets, eight counting no
    // fetches, one saying it is no form's submission, then eight bytes of
    // count, one of policy, one of cause.
    let (count, policy, cause) = (whole.len() - 25, whole.len() - 26, whole.len() - 27);
    for (at, value, said) in [
        (cause, 2u8, "cause tagged 2"),
        (policy, 9, "policy tagged 9"),
    ] {
        let mut strange = whole.clone();
        if let Some(byte) = strange.get_mut(at) {
            *byte = value;
        }
        let refused = read_from_renderer(&strange);
        assert!(
            refused.as_ref().is_err_and(|why| why.why.contains(said)),
            "{refused:?}"
        );
    }
    let mut too_many = whole.clone();
    if let Some(high) = too_many.get_mut(count) {
        *high = 1;
    }
    assert!(
        read_from_renderer(&too_many).is_err(),
        "a count of replaced navigations past u32 was read"
    );
}

/// A page's address is the browser process's to state, and one that does not
/// parse is a message that is not one.
#[test]
fn a_page_whose_address_is_not_a_url_is_refused() {
    let mut bytes = vec![0u8];
    let nonsense = b"not a url";
    bytes.extend_from_slice(&(nonsense.len() as u64).to_be_bytes());
    bytes.extend_from_slice(nonsense);
    let refused = read_to_renderer(&bytes);
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.why.contains("address")),
        "{refused:?}"
    );
}

/// ADR 0051 § 5: a form's submission crosses as names and values, and every
/// part of it is a stranger's. A method or an enctype no form could make is a
/// renderer that broke the boundary, so is an entry of a kind nobody has, and
/// a count of entries the message has no room for is not believed.
#[test]
fn a_forms_submission_crosses_as_names_and_values_and_a_strange_one_is_refused() {
    let submitted = FromRenderer::Acted {
        outcome: Outcome::Submitted {
            node: id(1),
            method: Method::Post,
            to: "https://httpbin.org/post".to_owned(),
        },
        issues: Vec::new(),
        objections: Vec::new(),
        navigation: Some(Asked {
            url: "https://httpbin.org/post".to_owned(),
            by: By::Browser,
            referrer: None,
            replaced: 0,
            submission: Some(Submission {
                method: Method::Post,
                enctype: Enctype::UrlEncoded,
                entries: vec![
                    Entry {
                        name: "custname".to_owned(),
                        value: Value::Text("Ada Lovelace".to_owned()),
                    },
                    Entry {
                        name: "file".to_owned(),
                        value: Value::NoFile,
                    },
                ],
            }),
        }),
        fetches: Vec::new(),
        sheets: Vec::new(),
    };
    let whole = write_from_renderer(&submitted);
    assert_eq!(read_from_renderer(&whole).as_ref(), Ok(&submitted));

    // From the end: eight bytes counting no style sheets and eight counting
    // no fetches; before them the file entry's tag, its name ("file", eight
    // bytes of length and four of text) and so on back to the count, the
    // enctype and the method.
    let file_tag = whole.len() - 17;
    let after_custname = file_tag - 12;
    let text_tag = after_custname - (8 + 12) - 1;
    let count = text_tag - (8 + 8) - 8;
    let (enctype, method) = (count - 1, count - 2);
    for (at, value, said) in [
        (method, 2u8, "method tagged 2"),
        (enctype, 3, "enctype tagged 3"),
        (file_tag, 2, "value tagged 2"),
        (text_tag, 7, "value tagged 7"),
    ] {
        let mut strange = whole.clone();
        if let Some(byte) = strange.get_mut(at) {
            *byte = value;
        }
        let refused = read_from_renderer(&strange);
        assert!(
            refused.as_ref().is_err_and(|why| why.why.contains(said)),
            "{said}: {refused:?}"
        );
    }
    let mut too_many = whole.clone();
    if let Some(high) = too_many.get_mut(count) {
        *high = 0x7f;
    }
    assert!(
        read_from_renderer(&too_many).is_err(),
        "a count of entries no message could hold was read"
    );
    for cut in 1..whole.len() {
        assert!(
            read_from_renderer(&whole[..cut]).is_err(),
            "{cut} bytes of a submission were read as a whole one"
        );
    }
}

/// What a press on a submit button came to, each said to an agent by name
/// (ADR 0051 § 6).
#[test]
fn what_came_of_a_submission_survives_the_crossing() {
    let acted = |outcome| FromRenderer::Acted {
        outcome,
        issues: Vec::new(),
        objections: Vec::new(),
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
    };
    let messages = vec![
        acted(Outcome::Submitted {
            node: id(8),
            method: Method::Get,
            to: "https://nordwind.alosites.com/b/studio-consultation".to_owned(),
        }),
        acted(Outcome::NotSubmitted {
            node: id(9),
            invalid: vec![
                Invalid {
                    control: Some(id(4)),
                    constraint: Some(Constraint::ValueMissing),
                    message: "Please fill out this field.".to_owned(),
                },
                Invalid {
                    control: None,
                    constraint: Some(Constraint::CustomError),
                    message: "mine".to_owned(),
                },
                Invalid {
                    control: Some(id(5)),
                    constraint: None,
                    message: "words, and no constraint".to_owned(),
                },
            ],
        }),
    ];
    for original in messages {
        let bytes = write_from_renderer(&original);
        let back = read_from_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
    let mut strange = write_from_renderer(&acted(Outcome::NotSubmitted {
        node: id(9),
        invalid: vec![Invalid {
            control: None,
            constraint: Some(Constraint::TooLong),
            message: String::new(),
        }],
    }));
    // From the end: eight bytes counting no style sheets, eight counting no
    // fetches, one saying there is no navigation, eight counting no
    // objections, eight counting no issues, then the message's eight bytes
    // of length and the constraint's tag.
    let at = strange.len() - 42;
    if let Some(tag) = strange.get_mut(at) {
        *tag = 10;
    }
    let refused = read_from_renderer(&strange);
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.why.contains("constraint tagged 10")),
        "{refused:?}"
    );
}

/// Queue item 747: each site's, video's and story's mark crosses as it was
/// sent; one larger than a mark may be, or not a picture, is refused.
#[test]
fn a_sites_mark_crosses_and_one_that_is_no_mark_is_refused() {
    use alo_renderer::results::{LARGEST_MARK, Stories, Videos};
    use alo_renderer::results_media::{Story, Video};
    let site = |written: &str| alo_url::parse(written).unwrap_or_else(|_| panic!("{written}"));
    let png = |bytes: usize| {
        let mut picture = b"\x89PNG\r\n\x1a\n".to_vec();
        picture.resize(bytes, 0);
        picture
    };
    let sending = |results: Results| {
        ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Searched(results)],
            ..Start::default()
        }))
    };
    let marked = |mark: Vec<u8>| {
        Results::of(
            "x",
            Showing::Found(vec![
                Found::new("t", site("https://a.example/"), "").marked(mark.clone()),
                Found::new("u", site("https://b.example/"), ""),
            ]),
        )
        .with_videos(Videos::Found(vec![
            Video::new("v", site("https://v.example/")).marked(mark.clone()),
        ]))
        .with_news(Stories::Found(vec![
            Story::new("s", site("https://n.example/")).marked(mark),
        ]))
    };
    for tab in [Tab::All, Tab::Videos, Tab::News] {
        let original = sending(marked(png(LARGEST_MARK)).on(tab));
        let back = read_to_renderer(&write_to_renderer(&original))
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
    for (mark, refusal) in [
        (png(LARGEST_MARK + 1), "at most 16384"),
        (
            b"<svg></svg>".to_vec(),
            "a site's mark that is not a picture",
        ),
    ] {
        let why = read_to_renderer(&write_to_renderer(&sending(marked(mark))))
            .err()
            .map(|why| why.why)
            .unwrap_or_default();
        assert!(why.contains(refusal), "{refusal}: {why:?}");
    }
}

/// Queue item 807: a card's shield crosses on and off, at no count and the
/// most one holds.
#[test]
fn a_cards_shield_crosses_as_it_was_sent() {
    use alo_renderer::card::{Card, Shield};
    for shield in [
        None,
        Some(Shield {
            blocked: 0,
            on: true,
        }),
        Some(Shield {
            blocked: u32::MAX,
            on: true,
        }),
        Some(Shield {
            blocked: 3,
            on: false,
        }),
    ] {
        let mut card = Card::new(
            "a page",
            alo_url::parse("https://www.theguardian.com/international")
                .unwrap_or_else(|_| panic!("parses")),
        );
        card.shield = shield;
        let original = ToRenderer::Start(Box::new(Start {
            turns: vec![Turn::Page(card)],
            ..Start::default()
        }));
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
}

/// Queue item 869: a card drawn beneath the first screen, the question of
/// where it was left clear and its answer, and the card's page to draw,
/// each cross whole; a place of no size, past any side or too far, a
/// picture that is not a PNG, of no size or past what a frame carries
/// back, and every truncation are refused by name, never panicked on.
#[test]
fn a_cards_page_beneath_the_first_screen_crosses_and_hostile_ones_are_refused() {
    use alo_renderer::beneath::{Beneath, FURTHEST, MOST_SIDE};
    use alo_renderer::card::{Card, PictureAt};
    use alo_renderer::card_picture::CardPicture;
    let url = alo_url::parse("https://en.wikipedia.org/wiki/Kampala").expect("an address");
    let start = ToRenderer::Start(Box::new(Start {
        turns: vec![Turn::Page(Card::new("kampala", url).pictured_beneath())],
        ..Start::default()
    }));
    let crossed = read_to_renderer(&write_to_renderer(&start));
    assert_eq!(crossed, Ok(start.clone()));
    assert!(matches!(
        crossed,
        Ok(ToRenderer::Start(start)) if start.card().is_some_and(|card| card.picture_at == PictureAt::Beneath)
    ));
    let asked = ToRenderer::PictureBeneath;
    assert_eq!(read_to_renderer(&write_to_renderer(&asked)), Ok(asked));
    let png = b"\x89PNG\r\n\x1a\nthe rest".to_vec();
    let picture = |png: Vec<u8>, width: u32, height: u32| {
        ToRenderer::CardPicture(Box::new(CardPicture { png, width, height }))
    };
    let drawn = picture(png.clone(), 1756, 1040);
    assert_eq!(read_to_renderer(&write_to_renderer(&drawn)), Ok(drawn));
    let refused_to = |message: &ToRenderer| {
        read_to_renderer(&write_to_renderer(message))
            .err()
            .map(|why| why.why)
            .unwrap_or_default()
    };
    assert!(refused_to(&picture(b"GIF89a".to_vec(), 4, 4)).contains("not a PNG"));
    assert!(refused_to(&picture(png.clone(), 0, 4)).contains("no size"));
    assert!(refused_to(&picture(png.clone(), MOST_SIDE + 1, 4)).contains("past"));
    assert!(
        refused_to(&picture(png.clone(), MOST_SIDE, MOST_SIDE)).contains("past what one frame"),
        "{}",
        refused_to(&picture(png.clone(), MOST_SIDE, MOST_SIDE))
    );
    let place = Beneath {
        left: -40,
        top: 290,
        width: 1756,
        height: 1040,
    };
    for answer in [
        FromRenderer::PictureBeneath(Some(place)),
        FromRenderer::PictureBeneath(None),
    ] {
        assert_eq!(
            read_from_renderer(&write_from_renderer(&answer)),
            Ok(answer.clone())
        );
    }
    let refused_from = |place: Beneath| {
        read_from_renderer(&write_from_renderer(&FromRenderer::PictureBeneath(Some(
            place,
        ))))
        .err()
        .map(|why| why.why)
        .unwrap_or_default()
    };
    assert!(refused_from(Beneath { width: 0, ..place }).contains("no size"));
    assert!(
        refused_from(Beneath {
            height: MOST_SIDE + 1,
            ..place
        })
        .contains("past")
    );
    assert!(
        refused_from(Beneath {
            top: FURTHEST + 1,
            ..place
        })
        .contains("further")
    );
    // Cut short anywhere, each is refused rather than panicked on.
    for whole in [
        write_to_renderer(&picture(png, 1756, 1040)),
        write_to_renderer(&start),
    ] {
        for cut in 0..whole.len() {
            assert!(read_to_renderer(&whole[..cut]).is_err(), "cut at {cut}");
        }
    }
    let whole = write_from_renderer(&FromRenderer::PictureBeneath(Some(place)));
    for cut in 0..whole.len() {
        assert!(read_from_renderer(&whole[..cut]).is_err(), "cut at {cut}");
    }
}

/// Queue item 872: the ask to paint only what changed, and what changed of
/// a frame, each cross whole — rectangles of pixels (a rectangle per piece,
/// queue item 896), and nothing changed; a rectangle past the frame's side
/// or foot, pixels more or fewer than the rectangle's, more pieces than a
/// renderer sends, and every truncation are refused, never panicked on.
#[test]
fn what_changed_of_a_frame_crosses_and_hostile_ones_are_refused() {
    use alo_renderer::frame_patch::{MOST_PIECES, Patch, Piece};
    let asked = ToRenderer::PaintOver;
    assert_eq!(read_to_renderer(&write_to_renderer(&asked)), Ok(asked));
    let piece = Piece {
        left: 205,
        top: 1264,
        across: 3,
        down: 2,
        pixels: (0..24).collect(),
    };
    let far = Piece {
        left: 1900,
        top: 10,
        across: 1,
        down: 2,
        pixels: (0..8).collect(),
    };
    let patch = Patch {
        width: 2080,
        height: 1640,
        pieces: vec![piece.clone(), far.clone()],
    };
    let nothing = Patch {
        pieces: Vec::new(),
        ..patch.clone()
    };
    for answer in [
        FromRenderer::PaintedOver(patch.clone()),
        FromRenderer::PaintedOver(nothing),
    ] {
        assert_eq!(
            read_from_renderer(&write_from_renderer(&answer)),
            Ok(answer.clone())
        );
    }
    let refused = |patch: Patch| {
        read_from_renderer(&write_from_renderer(&FromRenderer::PaintedOver(patch)))
            .err()
            .map(|why| why.why)
            .unwrap_or_default()
    };
    // Queue item 896: each piece read as hostile on its own, the second as
    // much as the first.
    let with = |second: Piece| Patch {
        pieces: vec![far.clone(), second],
        ..patch.clone()
    };
    assert!(
        refused(with(Piece {
            left: 2078,
            ..piece.clone()
        }))
        .contains("a patch of 3×2 at 2078, 1264")
    );
    assert!(
        refused(with(Piece {
            top: 1639,
            ..piece.clone()
        }))
        .contains("of a 2080×1640 frame")
    );
    let mut short = piece.clone();
    short.pixels.pop();
    assert!(refused(with(short)).contains("carrying 23 bytes"));
    let mut long = piece.clone();
    long.pixels.push(0);
    assert!(refused(with(long)).contains("carrying 25 bytes"));
    assert!(
        refused(with(Piece {
            left: u32::MAX,
            across: 2,
            ..piece.clone()
        }))
        .contains("at 4294967295")
    );
    assert!(
        refused(Patch {
            pieces: vec![far; MOST_PIECES + 1],
            ..patch.clone()
        })
        .contains("more than the 8 a renderer sends")
    );
    let whole = write_from_renderer(&FromRenderer::PaintedOver(patch));
    for cut in 0..whole.len() {
        assert!(read_from_renderer(&whole[..cut]).is_err(), "cut at {cut}");
    }
}

/// Queue item 856: *Settings* and what of it the keyboard is on cross as
/// sent, in one byte, and a byte naming anything else is refused by name.
#[test]
fn settings_and_where_the_keyboard_is_on_it_cross_and_nothing_else_does() {
    let messages = vec![
        // *Settings*, and each thing of it the keyboard may be on (queue
        // item 856).
        ToRenderer::Start(Box::new(Start {
            settings: true,
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_keyed: Some(SettingsKeyed::Close),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_keyed: Some(SettingsKeyed::Font),
            face: InterfaceFace::Geist,
            ..Start::default()
        })),
        // *alo at work*'s three, and its limits (queue item 878).
        ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_keyed: Some(SettingsKeyed::Sites),
            limits: alo_renderer::agent_limits::Limits::checked(
                10,
                100,
                alo_renderer::agent_limits::Asks::Every,
            )
            .unwrap_or_default(),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_keyed: Some(SettingsKeyed::Actions),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_keyed: Some(SettingsKeyed::Asks),
            ..Start::default()
        })),
        // Each of the three choices of asking, crossing as itself (queue
        // item 884): before, as a yes or no, *Never* came across as *A site
        // not found or named* — found by hand.
        ToRenderer::Start(Box::new(Start {
            settings: true,
            limits: alo_renderer::agent_limits::Limits::checked(
                3,
                20,
                alo_renderer::agent_limits::Asks::Unnamed,
            )
            .unwrap_or_default(),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            settings: true,
            limits: alo_renderer::agent_limits::Limits::checked(
                3,
                20,
                alo_renderer::agent_limits::Asks::Never,
            )
            .unwrap_or_default(),
            ..Start::default()
        })),
    ];
    for original in messages {
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
    let keyed = |settings_keyed| {
        write_to_renderer(&ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_keyed,
            ..Start::default()
        })))
    };
    let back = keyed(Some(SettingsKeyed::Close));
    let font = keyed(Some(SettingsKeyed::Font));
    assert_eq!(back.len(), font.len(), "one byte either way");
    let differing: Vec<usize> = (0..back.len())
        .filter(|&at| back.get(at) != font.get(at))
        .collect();
    assert_eq!(differing.len(), 1, "{differing:?}");
    let at = differing.first().copied().unwrap_or_default();
    assert_eq!((back.get(at), font.get(at)), (Some(&2), Some(&3)));
    for hostile in [7_u8, 9, 255] {
        let mut bytes = font.clone();
        if let Some(byte) = bytes.get_mut(at) {
            *byte = hostile;
        }
        let refused = read_to_renderer(&bytes).expect_err("refused");
        assert!(
            refused
                .to_string()
                .contains(&format!("a Settings tagged {hostile}")),
            "{refused}"
        );
    }
}

/// Queue item 886: over a conversation scrolled its own way, *Settings*
/// scrolled in its panel with its own bar crosses as sent.
#[test]
fn settings_scrolled_in_its_panel_crosses_as_sent() {
    let original = ToRenderer::Start(Box::new(Start {
        settings: true,
        down: -120.5,
        settings_down: 238.25,
        settings_bar: Some(alo_renderer::conversation_scroll::Bar {
            top: 158.0,
            length: 312.0,
            above: true,
            below: false,
            seen: 0.5,
        }),
        ..Start::default()
    }));
    let back = read_to_renderer(&write_to_renderer(&original)).expect("read back");
    assert_eq!(back, original);
}

/// Queue item 886: *Settings* scrolled further than any page reaches is
/// refused by name, as the conversation is.
#[test]
fn settings_scrolled_past_any_reach_is_refused() {
    let refused = |down: f32| {
        let bytes = write_to_renderer(&ToRenderer::Start(Box::new(Start {
            settings: true,
            settings_down: down,
            ..Start::default()
        })));
        read_to_renderer(&bytes).expect_err("refused").to_string()
    };
    for down in [2.0e6_f32, -2.0e6] {
        assert!(
            refused(down).contains("Settings scrolled further"),
            "{down}"
        );
    }
    for down in [f32::INFINITY, f32::NAN] {
        assert!(refused(down).contains("not a number"), "{down}");
    }
}

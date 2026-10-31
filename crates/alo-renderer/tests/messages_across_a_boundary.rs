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
use alo_renderer::frame::Frame;
use alo_renderer::generic::{Generics, MOST_PAIRS};
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::snapshot::{Snapshot, SnapshotNode};
use alo_renderer::start::{KeptSite, Start, Waiting};
use alo_renderer::strip::LONGEST_TITLE;
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
            offers: alo_renderer::typed::offers("example.com")
                .into_iter()
                .chain([Offer::Search("</script>\0<b>".to_owned())])
                .collect(),
            card: None,
            results: None,
            search: SetUp::Not("no key\0<b>".to_owned()),
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

/// Queue item 708: a search's results, each state a search can be in,
/// cross as they were sent.
#[test]
fn a_searchs_results_survive_the_crossing() {
    let messages = vec![
        // Strangers' strings in every field.
        ToRenderer::Start(Box::new(Start {
            results: Some(Results {
                said: "</p><script>".to_owned(),
                showing: Showing::Found(vec![
                    Found::new(
                        "<b>Fish &amp; chips</b>\0",
                        alo_url::parse("https://fish.example/a?b#c")
                            .unwrap_or_else(|_| alo_url::Url::about_blank()),
                        "two <strong>lines</strong>",
                    )
                    .named("Fish\u{202e}"),
                ]),
            }),
            search: SetUp::Ready,
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            results: Some(Results::searching("x")),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            results: Some(Results {
                said: "x".to_owned(),
                showing: Showing::Unanswered(
                    "Brave Search could not answer just now (500).".to_owned(),
                ),
            }),
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
/// person said, hostile, the address, somewhere to go back to and a frame;
/// and a card before its first frame.
#[test]
fn a_first_screen_holding_a_card_survives_the_crossing() {
    let messages = vec![
        // A page shown as a card, with somewhere to go back to and a frame
        // (queue item 717), and one before its first frame.
        ToRenderer::Start(Box::new(Start {
            card: Some(
                alo_renderer::card::Card::new(
                    "example.com</p>\0",
                    alo_url::parse("https://example.com/a?b#c")
                        .unwrap_or_else(|_| panic!("parses")),
                )
                .holding(b"\x89PNG\r\n\x1a\nthe rest".to_vec()),
            )
            .map(|mut card| {
                card.back = true;
                card
            }),
            ..Start::default()
        })),
        ToRenderer::Start(Box::new(Start {
            card: Some(alo_renderer::card::Card::new(
                "",
                alo_url::parse("http://127.0.0.1:8765/").unwrap_or_else(|_| panic!("parses")),
            )),
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
    ];
    for original in messages {
        let bytes = write_from_renderer(&original);
        let back = read_from_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way across");
    }
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
            card: Some(alo_renderer::card::Card::new("a", url).holding(frame.to_vec())),
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
            results: Some(Results {
                said: "x".to_owned(),
                showing: Showing::Found(vec![Found::new("t", url, "d"); count]),
            }),
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

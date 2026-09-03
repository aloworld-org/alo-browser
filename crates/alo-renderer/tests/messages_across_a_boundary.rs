//! Every message the boundary carries, written down and read back — and what
//! happens when what comes back is a lie.
//!
//! ADR 0005: the browser process holds the network, the disk and the profile,
//! and a renderer is the process that parsed a hostile page. So the interesting
//! half of this file is not the round trips. It is that a message *from* a
//! renderer is treated as bytes a stranger chose, because if a page found a way
//! to steer that process, the stranger is the page.

use alo_agent::verb::{Outcome, Refusal, ScrollBy, Target, Verb};
use alo_box::role::{KnownRole, Role};
use alo_box::state::{Checked, Current, States};
use alo_box::tree::BoxId;
use alo_css::media::ColorScheme;
use alo_layout::geometry::{Point, Rect, Size};
use alo_renderer::frame::Frame;
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::snapshot::{Snapshot, SnapshotNode};
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
            invalid: false,
            hidden: false,
            level: Some(3),
            current: Some(Current::Page),
            takes_text: true,
        },
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
            html: "<p>hello</p>".to_owned(),
            sheets: vec!["p { color: red }".to_owned(), "p { margin: 0 }".to_owned()],
            viewport: Size {
                width: 900.0,
                height: 600.0,
            },
            scheme: ColorScheme::Dark,
        })),
        ToRenderer::Resize(Size {
            width: 320.5,
            height: 480.0,
        }),
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
    let messages = vec![
        FromRenderer::Loaded {
            issues: vec!["refused `float: left`".to_owned()],
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
        FromRenderer::Acted(Outcome::Activated {
            node: id(3),
            name: Some("Save".to_owned()),
        }),
        FromRenderer::Acted(Outcome::Followed {
            node: id(4),
            to: "https://example.com/next".to_owned(),
        }),
        FromRenderer::Acted(Outcome::TextPut {
            node: id(5),
            text: "12 May".to_owned(),
        }),
        FromRenderer::Acted(Outcome::Scrolled {
            node: id(6),
            by: ScrollBy::ToStart,
        }),
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
        FromRenderer::Failed(Failure::NothingLoaded),
        FromRenderer::Failed(Failure::Unpaintable {
            why: "a window of no size".to_owned(),
        }),
    ];
    for original in messages {
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
    let whole = write_from_renderer(&FromRenderer::Acted(Outcome::Followed {
        node: id(4),
        to: "https://example.com/next".to_owned(),
    }));
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

/// Nothing at all is not a message.
#[test]
fn an_empty_message_is_refused() {
    assert!(read_to_renderer(&[]).is_err());
    assert!(read_from_renderer(&[]).is_err());
}

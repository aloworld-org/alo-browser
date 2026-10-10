/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 348: a page's linked style sheet crosses the boundary (ADR
//! 0035 §§ 1 and 4).
//!
//! *Malformed, truncated and adversarial bytes in either direction refused,
//! never a panic.* The asks come from a renderer, which parsed a stranger's
//! page, and every mode `alo-net` names is carried so that the browser
//! process — not the decoder — refuses one no `<link>` asks in. The answers
//! go to a renderer, and carry bytes or nothing.

use alo_agent::verb::Outcome;
use alo_box::tree::BoxId;
use alo_net::cors::{Credentials, Mode};
use alo_net::referrer::Policy;
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::sheet::{SheetAnswer, SheetAsk};
use alo_renderer::wire::{
    read_from_renderer, read_to_renderer, sheet_answer_size, write_from_renderer, write_to_renderer,
};

/// One ask of every mode, credentials mode and referrer policy, with and
/// without a nonce.
fn asks() -> Vec<SheetAsk> {
    let modes = [Mode::NoCors, Mode::Cors, Mode::SameOrigin, Mode::Navigate];
    let credentials = [
        Credentials::Include,
        Credentials::SameOrigin,
        Credentials::Omit,
    ];
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
    let mut modes = modes.iter().cycle();
    let mut credentials = credentials.iter().cycle();
    let mut policies = policies.iter().cycle();
    (0..12u8)
        .filter_map(|at| {
            Some(SheetAsk {
                number: u64::MAX - u64::from(at),
                url: format!("https://cdn.example/{at}.css"),
                mode: *modes.next()?,
                credentials: *credentials.next()?,
                referrer: *policies.next()?,
                nonce: (at % 2 == 0).then(|| format!("n{at}")),
            })
        })
        .collect()
}

fn answers() -> Vec<FromRenderer> {
    vec![
        FromRenderer::Loaded {
            issues: vec!["a line".to_owned()],
            wanted: Vec::new(),
            objections: Vec::new(),
            navigation: None,
            fetches: Vec::new(),
            sheets: asks(),
            left: Vec::new(),
        },
        FromRenderer::Acted {
            outcome: Outcome::Activated {
                node: BoxId::from_wire(4),
                name: Some("Add".to_owned()),
            },
            issues: Vec::new(),
            objections: Vec::new(),
            navigation: None,
            fetches: Vec::new(),
            sheets: asks(),
        },
        FromRenderer::Delivered {
            issues: vec!["the style sheet at \"x\" did not arrive".to_owned()],
            objections: Vec::new(),
            navigation: None,
            fetches: Vec::new(),
            sheets: asks(),
        },
    ]
}

fn deliveries() -> Vec<ToRenderer> {
    vec![
        ToRenderer::Sheet(Box::new(SheetAnswer::failed(0))),
        ToRenderer::Sheet(Box::new(SheetAnswer {
            number: u64::MAX,
            bytes: Some(b"p { color: red }".to_vec()),
        })),
        ToRenderer::Sheet(Box::new(SheetAnswer {
            number: 7,
            bytes: Some(Vec::new()),
        })),
        ToRenderer::Sheet(Box::new(SheetAnswer {
            number: 8,
            bytes: Some(vec![0xFF, 0xFE, 0, 0xEF, 0xBB, 0xBF]),
        })),
    ]
}

#[test]
fn every_ask_and_every_answer_survives_the_crossing() {
    for original in answers() {
        let back = read_from_renderer(&write_from_renderer(&original));
        assert_eq!(back.as_ref(), Ok(&original));
    }
    for original in deliveries() {
        let back = read_to_renderer(&write_to_renderer(&original));
        assert_eq!(back.as_ref(), Ok(&original));
    }
}

/// `sheet_make` asks how large an answer is before it sends a sheet, so the
/// arithmetic has to be the encoding's exactly.
#[test]
fn an_answers_size_is_what_it_writes() {
    for sent in deliveries() {
        let ToRenderer::Sheet(answer) = &sent else {
            continue;
        };
        assert_eq!(
            sheet_answer_size(answer),
            write_to_renderer(&sent).len(),
            "{answer}"
        );
    }
}

/// Every prefix of every message is a message that did not arrive.
#[test]
fn a_message_that_stops_part_way_through_is_refused_in_either_direction() {
    for original in answers() {
        let whole = write_from_renderer(&original);
        for cut in 1..whole.len() {
            assert!(
                read_from_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
                "{cut} bytes of {original:?} were read as a whole one"
            );
        }
    }
    for original in deliveries() {
        let whole = write_to_renderer(&original);
        for cut in 1..whole.len() {
            assert!(
                read_to_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
                "{cut} bytes of {original} were read as a whole one"
            );
        }
    }
}

/// Every byte of every message changed to every value, read again: whatever
/// comes back, nothing panics.
#[test]
fn every_byte_of_every_message_changed_is_read_or_refused_never_a_panic() {
    for original in answers() {
        let whole = write_from_renderer(&original);
        for at in 0..whole.len() {
            for value in [0u8, 1, 2, 3, 4, 9, 0x7f, 0xff] {
                let mut changed = whole.clone();
                if let Some(byte) = changed.get_mut(at) {
                    *byte = value;
                }
                let _ = read_from_renderer(&changed);
            }
        }
    }
    for original in deliveries() {
        let whole = write_to_renderer(&original);
        for at in 0..whole.len() {
            for value in [0u8, 1, 2, 3, 4, 9, 0x7f, 0xff] {
                let mut changed = whole.clone();
                if let Some(byte) = changed.get_mut(at) {
                    *byte = value;
                }
                let _ = read_to_renderer(&changed);
            }
        }
    }
}

/// The last ask's tags, counted back from the end of a `Delivered` carrying
/// one with no nonce, whose absence is the last byte: referrer, credentials,
/// mode.
#[test]
fn an_ask_tagged_with_something_nobody_has_is_refused_by_name() {
    let mut one = asks().remove(1);
    one.nonce = None;
    one.referrer = None;
    let whole = write_from_renderer(&FromRenderer::Delivered {
        issues: Vec::new(),
        objections: Vec::new(),
        navigation: None,
        fetches: Vec::new(),
        sheets: vec![one],
    });
    let end = whole.len() - 1;
    for (from_end, value, said) in [
        (1, 10u8, "style sheet's referrer policy tagged 9"),
        (2, 3, "style sheet's credentials tagged 3"),
        (3, 4, "style sheet's mode tagged 4"),
    ] {
        let mut strange = whole.clone();
        if let Some(byte) = strange.get_mut(end - from_end) {
            *byte = value;
        }
        let refused = read_from_renderer(&strange);
        assert!(
            refused.as_ref().is_err_and(|why| why.why.contains(said)),
            "{said}: {refused:?}"
        );
    }
}

/// A count of asks no message could hold is refused before anything is made
/// room for, and an answer that is neither bytes nor nothing is not one.
#[test]
fn a_count_or_an_answer_no_honest_sender_writes_is_refused() {
    let honest = write_from_renderer(&FromRenderer::Delivered {
        issues: Vec::new(),
        objections: Vec::new(),
        navigation: None,
        fetches: Vec::new(),
        sheets: Vec::new(),
    });
    let mut flood = honest.get(..honest.len() - 8).unwrap_or_default().to_vec();
    flood.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(
        read_from_renderer(&flood)
            .as_ref()
            .is_err_and(|why| why.why.contains("no room")),
        "{:?}",
        read_from_renderer(&flood)
    );

    let mut bytes = vec![8u8];
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.push(2);
    assert!(
        read_to_renderer(&bytes)
            .as_ref()
            .is_err_and(|why| why.why.contains("style sheet's answer tagged 2"))
    );
    // Bytes that say they are longer than the message.
    let mut bytes = vec![8u8];
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(read_to_renderer(&bytes).is_err());
    // And a byte left over after a whole one.
    let mut trailing = write_to_renderer(&ToRenderer::Sheet(Box::new(SheetAnswer::failed(3))));
    trailing.push(0);
    assert!(read_to_renderer(&trailing).is_err());
}

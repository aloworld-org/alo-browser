/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 297: the tab strip's state crosses the boundary (ADR 0024 § 4).
//!
//! It goes from the browser process to the strip's renderer, which is
//! confined and reads it as it reads anything: *malformed, truncated and
//! adversarial bytes refused, never a panic*. Its titles are pages' strings,
//! and they cross whole — markup, a NUL, a right-to-left override, a hundred
//! thousand characters — as text.

use alo_layout::geometry::Size;
use alo_renderer::message::ToRenderer;
use alo_renderer::strip::{Condition, Strip, StripTab};
use alo_renderer::wire::{read_to_renderer, write_to_renderer};

fn hostile_titles() -> Vec<String> {
    vec![
        "<b>bold</b>".to_owned(),
        "</style><script>steal()</script>".to_owned(),
        "a\0b".to_owned(),
        "x".repeat(100_000),
        "\u{202e}gnp.exe".to_owned(),
        String::new(),
    ]
}

fn a_strip() -> Strip {
    let mut tabs: Vec<StripTab> = hostile_titles()
        .into_iter()
        .map(|title| StripTab::new(title, "https://example.com/"))
        .collect();
    tabs.push(
        StripTab::new("Invoices", "https://books.example/")
            .selected()
            .in_condition(Condition::Loading),
    );
    tabs.push(StripTab::new("Calendar", "https://cal.example/").in_condition(Condition::Gone));
    Strip {
        tabs,
        viewport: Size {
            width: 1000.0,
            height: 36.0,
        },
        scale: 1.5,
    }
}

#[test]
fn a_strip_survives_the_crossing_with_every_title_as_it_was() {
    for strip in [a_strip(), Strip::default()] {
        let original = ToRenderer::Strip(Box::new(strip));
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
}

#[test]
fn a_strip_that_stops_anywhere_is_refused() {
    let bytes = write_to_renderer(&ToRenderer::Strip(Box::new(a_strip())));
    // Every prefix but the whole: a message cut at any byte is refused,
    // whichever field it was cut in. Stepping keeps the long title's
    // hundred thousand prefixes from being a hundred thousand reads.
    let mut cut = 0;
    while cut < bytes.len() {
        let prefix = bytes.get(..cut).unwrap_or_default();
        assert!(read_to_renderer(prefix).is_err(), "cut at {cut} was read");
        cut += if (64..bytes.len() - 64).contains(&cut) {
            997
        } else {
            1
        };
    }
}

#[test]
fn a_count_of_tabs_larger_than_the_message_is_refused_before_anything_is_reserved() {
    let mut bytes = vec![11];
    bytes.extend_from_slice(&640f32.to_be_bytes());
    bytes.extend_from_slice(&36f32.to_be_bytes());
    bytes.extend_from_slice(&1f32.to_be_bytes());
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(read_to_renderer(&bytes).is_err());
}

#[test]
fn a_size_that_is_not_a_number_is_refused() {
    for width in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bytes = vec![11];
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&36f32.to_be_bytes());
        bytes.extend_from_slice(&1f32.to_be_bytes());
        bytes.extend_from_slice(&0u64.to_be_bytes());
        assert!(read_to_renderer(&bytes).is_err(), "{width}");
    }
}

/// No tabs, at 640 by 36, on a screen of `scale`, written by hand.
fn no_tabs_at(scale: f32) -> Vec<u8> {
    let mut bytes = vec![11];
    bytes.extend_from_slice(&640f32.to_be_bytes());
    bytes.extend_from_slice(&36f32.to_be_bytes());
    bytes.extend_from_slice(&scale.to_be_bytes());
    bytes.extend_from_slice(&0u64.to_be_bytes());
    bytes
}

#[test]
fn a_scale_that_is_not_a_scale_is_refused_or_read_as_one() {
    // Not a number refuses the message, as a size's does (ADR 0057 § 3).
    for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(read_to_renderer(&no_tabs_at(scale)).is_err(), "{scale}");
    }
    // A number that is not a factor above zero is one.
    for scale in [0.0, -0.0, -2.0, f32::MIN] {
        let Ok(ToRenderer::Strip(strip)) = read_to_renderer(&no_tabs_at(scale)) else {
            panic!("a strip at {scale} was not read");
        };
        assert_eq!(strip.scale.to_bits(), 1f32.to_bits(), "{scale}");
    }
    // A factor is kept as it was sent, unrounded.
    let Ok(ToRenderer::Strip(strip)) = read_to_renderer(&no_tabs_at(1.5)) else {
        panic!("a strip at 1.5 was not read");
    };
    assert_eq!(strip.scale.to_bits(), 1.5f32.to_bits());
}

/// One tab, written by hand: its title, address, selection and condition.
fn one_tab(selected: u8, condition: u8, title: &[u8]) -> Vec<u8> {
    let mut bytes = vec![11];
    bytes.extend_from_slice(&640f32.to_be_bytes());
    bytes.extend_from_slice(&36f32.to_be_bytes());
    bytes.extend_from_slice(&2f32.to_be_bytes());
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.extend_from_slice(&(title.len() as u64).to_be_bytes());
    bytes.extend_from_slice(title);
    bytes.extend_from_slice(&0u64.to_be_bytes());
    bytes.push(selected);
    bytes.push(condition);
    bytes
}

#[test]
fn a_tab_written_by_hand_is_read_and_a_strange_one_is_refused() {
    assert_eq!(
        read_to_renderer(&one_tab(1, 2, b"ok")),
        Ok(ToRenderer::Strip(Box::new(Strip {
            tabs: vec![
                StripTab::new("ok", "")
                    .selected()
                    .in_condition(Condition::Gone)
            ],
            viewport: Size {
                width: 640.0,
                height: 36.0,
            },
            scale: 2.0,
        })))
    );
    assert!(
        read_to_renderer(&one_tab(2, 0, b"ok")).is_err(),
        "a selection of 2"
    );
    assert!(
        read_to_renderer(&one_tab(0, 3, b"ok")).is_err(),
        "a condition of 3"
    );
    assert!(
        read_to_renderer(&one_tab(0, 0, &[0xff, 0xfe])).is_err(),
        "a title that is not UTF-8"
    );
    let mut longer = one_tab(0, 0, b"ok");
    longer.push(0);
    assert!(read_to_renderer(&longer).is_err(), "a byte left over");
}

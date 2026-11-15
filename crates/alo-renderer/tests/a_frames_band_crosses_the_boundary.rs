/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 991: the frame's band crosses the boundary as the tab strip
//! does (ADR 0024 § 4), tag 24 — malformed, truncated and adversarial bytes
//! refused, never a panic, and a page's title crossing whole as text.

use alo_layout::geometry::Size;
use alo_renderer::band::Band;
use alo_renderer::message::ToRenderer;
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

#[test]
fn a_band_survives_the_crossing_with_its_title_as_it_was() {
    for title in hostile_titles() {
        let band = Band::new(title, "https://example.com/", Size::new(980.0, 54.0)).at_scale(1.5);
        let original = ToRenderer::Band(Box::new(band));
        let bytes = write_to_renderer(&original);
        let back = read_to_renderer(&bytes)
            .unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original);
    }
}

#[test]
fn a_band_that_stops_anywhere_is_refused() {
    let band = Band::new(
        "Invoices <b>2026</b>",
        "https://example.com/",
        Size::new(980.0, 54.0),
    );
    let bytes = write_to_renderer(&ToRenderer::Band(Box::new(band)));
    for cut in 0..bytes.len() {
        let prefix = bytes.get(..cut).unwrap_or_default();
        assert!(read_to_renderer(prefix).is_err(), "cut at {cut} was read");
    }
}

/// A band written by hand: its size, scale, title and address.
fn by_hand(width: f32, scale: f32, title: &[u8]) -> Vec<u8> {
    let mut bytes = vec![24];
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&54f32.to_be_bytes());
    bytes.extend_from_slice(&scale.to_be_bytes());
    bytes.extend_from_slice(&(title.len() as u64).to_be_bytes());
    bytes.extend_from_slice(title);
    bytes.extend_from_slice(&0u64.to_be_bytes());
    bytes
}

#[test]
fn a_band_written_by_hand_is_read_and_a_strange_one_is_refused() {
    assert_eq!(
        read_to_renderer(&by_hand(640.0, 2.0, b"ok")),
        Ok(ToRenderer::Band(Box::new(
            Band::new("ok", "", Size::new(640.0, 54.0)).at_scale(2.0)
        )))
    );
    for width in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            read_to_renderer(&by_hand(width, 1.0, b"ok")).is_err(),
            "{width}"
        );
    }
    // A number that is not a factor above zero is one.
    let Ok(ToRenderer::Band(band)) = read_to_renderer(&by_hand(640.0, 0.0, b"ok")) else {
        panic!("a band at 0 was not read");
    };
    assert_eq!(band.scale.to_bits(), 1f32.to_bits());
    assert!(
        read_to_renderer(&by_hand(640.0, 1.0, &[0xff, 0xfe])).is_err(),
        "a title that is not UTF-8"
    );
    let mut longer = by_hand(640.0, 1.0, b"ok");
    longer.push(0);
    assert!(read_to_renderer(&longer).is_err(), "a byte left over");
}

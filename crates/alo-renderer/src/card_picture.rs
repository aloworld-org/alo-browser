/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The card's page drawn on its own, for the window to draw beneath the
//! first screen (queue item 869, ADR 0100).
//!
//! Chrome's newest frame of the page is a stranger's PNG, decoded only in a
//! confined renderer (ADR 0076 B). It was decoded and drawn as the card's
//! `<img>` in the first screen's document, so every frame painted the whole
//! first screen again and an edit in the box waited behind it. Now a
//! renderer of its own is sent the frame and the place the first screen
//! left clear for it ([`crate::beneath`]), and answers the picture drawn
//! into exactly that many device pixels — through the one picture path
//! every `<img>` takes ([`alo_paint::drawn_picture::draw_picture`]), so a
//! frame at the place's own size is its pixels, and one at another size is
//! smoothed as the `<img>` would have smoothed it.

use crate::Frame;
use crate::message::Failure;
use crate::site::Site;
use alo_layout::Rect;
use alo_paint::Canvas;
use alo_paint::picture_filter::Uploads;
use alo_value::Matrix;
use std::sync::Arc;

/// What the card's picture's renderer is said to be at.
pub const ADDRESS: &str = "alo:card-picture";

/// A site for the card's picture's renderer that no page can name, minted
/// on every call, as the first screen's is.
pub fn site() -> Site {
    Site::of(&alo_url::parse(ADDRESS).unwrap_or_else(|_| alo_url::Url::about_blank()))
}

/// What a card's picture is drawn from: the frame's bytes and the size of
/// the place it goes, in device pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardPicture {
    /// Chrome's frame of the page: a PNG's bytes.
    pub png: Vec<u8>,
    /// How wide the place is.
    pub width: u32,
    /// How tall.
    pub height: u32,
}

/// The picture drawn into a canvas of its place's size: the frame decoded
/// and drawn over nothing, filling it.
///
/// # Errors
///
/// [`Failure::Unpaintable`] for a frame this engine does not read, or a
/// place of no size.
pub fn draw(picture: &CardPicture) -> Result<Frame, Failure> {
    if picture.width == 0 || picture.height == 0 {
        return Err(Failure::Unpaintable {
            why: "a card's picture for a place of no size".to_owned(),
        });
    }
    let decoded = alo_paint::picture::read(&picture.png).map_err(|why| Failure::Unpaintable {
        why: format!("the card's frame is not a picture this engine reads: {why}"),
    })?;
    let mut canvas = Canvas::new(picture.width, picture.height, alo_value::Rgba::TRANSPARENT);
    let side = |pixels: u32| f32::from(u16::try_from(pixels).unwrap_or(u16::MAX));
    alo_paint::drawn_picture::draw_picture(
        &mut canvas,
        &Arc::new(decoded),
        Rect::new(0.0, 0.0, side(picture.width), side(picture.height)),
        Matrix::IDENTITY,
        &mut Uploads::default(),
    );
    Ok(Frame::from_canvas(&canvas))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::frame_to_png;

    /// A frame of `width` by `height`, each pixel its own colour.
    fn frame(width: u32, height: u32) -> Frame {
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let byte = |seed: u32| u8::try_from((x * 31 + y * 17 + seed) % 256).unwrap_or(0);
                pixels.extend_from_slice(&[byte(0), byte(90), byte(180), 255]);
            }
        }
        Frame {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn a_frame_at_its_places_size_is_its_own_pixels() {
        let made = frame(12, 7);
        let png = frame_to_png(&made).expect("a frame is written as a PNG");
        let drawn = draw(&CardPicture {
            png,
            width: 12,
            height: 7,
        });
        assert_eq!(drawn, Ok(made));
    }

    #[test]
    fn a_frame_at_another_size_fills_its_place() {
        let png = frame_to_png(&frame(6, 4)).expect("a frame is written as a PNG");
        let drawn = draw(&CardPicture {
            png,
            width: 12,
            height: 8,
        })
        .expect("drawn");
        assert_eq!((drawn.width, drawn.height), (12, 8));
        assert!(
            drawn
                .pixels
                .chunks(4)
                .all(|pixel| pixel.get(3) == Some(&255)),
            "every pixel of the place is the page's"
        );
    }

    #[test]
    fn bytes_that_are_no_picture_and_a_place_of_no_size_are_said() {
        let png = frame_to_png(&frame(2, 2)).expect("a frame is written as a PNG");
        assert!(matches!(
            draw(&CardPicture {
                png: b"\x89PNG\r\n\x1a\nnonsense".to_vec(),
                width: 4,
                height: 4,
            }),
            Err(Failure::Unpaintable { .. })
        ));
        assert!(matches!(
            draw(&CardPicture {
                png,
                width: 0,
                height: 4,
            }),
            Err(Failure::Unpaintable { .. })
        ));
    }
}

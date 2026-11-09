/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Damage cut into a few rectangles, each painted again on its own (queue
//! item 891, ADR 0101 § 3 amended).
//!
//! Tab from *Close* to *Font* in *Settings* puts one ring away at the
//! panel's top right and draws another half way down its left: two small
//! places far apart, whose bounding rectangle held the card's picture, the
//! veil's blur and a score of runs of text, all painted again for nothing.
//! So the places [`crate::damage`] finds are joined only where joining
//! costs less than painting a piece more: a piece is the whole list drawn
//! again onto a canvas confined to it ([`crate::repaint()`]), and each
//! costs its pixels and what every confined drawing costs however small —
//! every run of text outlined to be passed over, every clip in force
//! pushed.
//!
//! # Why painting pieces one after another is right
//!
//! A piece is painted as ADR 0101 § 3 paints a region: put back to the
//! colour a new canvas starts in, confined, and drawn whole. Every pixel
//! inside it comes out as drawn whole, so long as everything drawn there
//! reads only the pixel it writes — and the one thing that reads others, a
//! backdrop, reads its own crop (ADR 0104 § 2). A backdrop whose blur is
//! kept reads nothing. One whose crop was damaged reads every pixel of its
//! crop as drawn before it, which only a piece holding the whole crop has
//! drawn: so **every piece that meets such a crop is joined with it**, and
//! no other piece meets it. A pixel two pieces share is painted twice, each
//! time as drawn whole. Pixels in no piece are where the lists draw alike.

use crate::region::Region;

/// What a piece costs beyond its pixels, in pixels: every confined drawing
/// passes over the whole list, and outlines every run of text to do it —
/// about 2 ms at two for *Settings*' 41 runs, when a pixel painted under
/// the veil, the card's picture and the panel costs about 15 ns.
pub const PIECE_PIXELS: u64 = 1 << 17;

/// The most pieces a damage is cut into: past this the two whose joining
/// wastes least are joined.
pub const MOST_PIECES: usize = 8;

/// `places`, joined where that costs no more than a piece does, cut into
/// at most [`MOST_PIECES`], with every piece that meets one of `crops`
/// joined into the one holding it: the pieces to paint, in no particular
/// order. Nothing for no places.
pub fn cut(places: &[Region], crops: &[Region]) -> Vec<Region> {
    let mut pieces: Vec<Region> = Vec::new();
    for &place in places {
        pieces.push(place);
        settle(&mut pieces, crops);
    }
    while pieces.len() > MOST_PIECES {
        let Some((one, two)) = cheapest(&pieces) else {
            break;
        };
        join(&mut pieces, one, two);
        settle(&mut pieces, crops);
    }
    pieces
}

/// Join pieces of `pieces` until no two are worth joining and no crop is met
/// by more than one: every pair joined is replaced by its join, which may be
/// worth joining with another in turn.
fn settle(pieces: &mut Vec<Region>, crops: &[Region]) {
    loop {
        let Some((one, two)) = worth_joining(pieces, crops) else {
            return;
        };
        join(pieces, one, two);
    }
}

/// Two pieces, by their places in `pieces`, to join: two that share a crop,
/// or whose joining wastes no more than a piece costs.
fn worth_joining(pieces: &[Region], crops: &[Region]) -> Option<(usize, usize)> {
    for (one, &a) in pieces.iter().enumerate() {
        for (two, &b) in pieces.iter().enumerate().skip(one + 1) {
            let share_a_crop = crops
                .iter()
                .any(|crop| crop.meeting(a).is_some() && crop.meeting(b).is_some());
            if share_a_crop || waste(a, b) <= PIECE_PIXELS {
                return Some((one, two));
            }
        }
    }
    None
}

/// The two pieces whose joining wastes least.
fn cheapest(pieces: &[Region]) -> Option<(usize, usize)> {
    let mut best: Option<(u64, usize, usize)> = None;
    for (one, &a) in pieces.iter().enumerate() {
        for (two, &b) in pieces.iter().enumerate().skip(one + 1) {
            let wasted = waste(a, b);
            if best.is_none_or(|(least, _, _)| wasted < least) {
                best = Some((wasted, one, two));
            }
        }
    }
    best.map(|(_, one, two)| (one, two))
}

/// The pixels painted again by joining `a` and `b` that neither holds.
fn waste(a: Region, b: Region) -> u64 {
    let shared = a.meeting(b).map_or(0, Region::area);
    let held = a.area() + b.area() - shared;
    a.joined(b).area().saturating_sub(held)
}

/// The pieces at `one` and `two` replaced by their join.
fn join(pieces: &mut Vec<Region>, one: usize, two: usize) {
    let (Some(&a), Some(&b)) = (pieces.get(one), pieces.get(two)) else {
        return;
    };
    pieces.swap_remove(two.max(one));
    pieces.swap_remove(two.min(one));
    pieces.push(a.joined(b));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(left: u32, top: u32, right: u32, bottom: u32) -> Region {
        Region::new(left, top, right, bottom).expect("pixels")
    }

    #[test]
    fn two_far_places_are_two_pieces_and_two_near_ones_one() {
        let ring = at(1800, 100, 1900, 180);
        let other = at(400, 900, 1100, 1300);
        let mut pieces = cut(&[ring, other], &[]);
        pieces.sort_by_key(|piece| piece.left);
        assert_eq!(pieces, vec![other, ring]);
        let beside = at(1905, 100, 1960, 180);
        assert_eq!(cut(&[ring, beside], &[]), vec![ring.joined(beside)]);
    }

    #[test]
    fn a_place_inside_another_is_painted_once() {
        let large = at(0, 0, 500, 500);
        assert_eq!(cut(&[large, at(10, 10, 20, 20)], &[]), vec![large]);
    }

    #[test]
    fn every_piece_that_meets_a_damaged_crop_is_joined_with_it() {
        // A crop whose blur reads all of it, a strip from it down into a
        // far place, and two far places each under an end of it: whatever
        // meets the crop is one piece holding it whole, and no other piece
        // meets it.
        let crop = at(0, 0, 2000, 40);
        let left = at(0, 1000, 300, 1300);
        let right = at(1700, 1000, 2000, 1300);
        let strip = at(0, 30, 10, 1100);
        for places in [
            vec![crop, strip, left, right],
            vec![left, strip, right, crop],
        ] {
            let pieces = cut(&places, &[crop]);
            let meeting: Vec<&Region> = pieces
                .iter()
                .filter(|piece| piece.meeting(crop).is_some())
                .collect();
            assert_eq!(meeting.len(), 1, "{pieces:?}");
            assert!(
                meeting
                    .iter()
                    .all(|piece| piece.meeting(crop) == Some(crop))
            );
            assert!(
                meeting
                    .iter()
                    .all(|piece| piece.meeting(strip) == Some(strip))
            );
            for place in &places {
                assert!(
                    pieces
                        .iter()
                        .any(|piece| piece.meeting(*place) == Some(*place))
                );
            }
        }
    }

    #[test]
    fn never_more_than_the_most_pieces() {
        let places: Vec<Region> = (0..20)
            .map(|k| at(k * 400, k * 400, k * 400 + 50, k * 400 + 50))
            .collect();
        let pieces = cut(&places, &[]);
        assert_eq!(pieces.len(), MOST_PIECES);
        for place in &places {
            assert!(
                pieces
                    .iter()
                    .any(|piece| piece.meeting(*place) == Some(*place)),
                "{place:?} held whole"
            );
        }
        assert!(cut(&[], &[]).is_empty());
    }
}

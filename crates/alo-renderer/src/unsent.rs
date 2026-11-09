/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What every drawing since the last frame was sent painted again (queue
//! items 872 and 896, ADR 0102 § 3): what an asker holding that frame is
//! sent ([`crate::frame_patch`]).
//!
//! A drawing painted in pieces ([`alo_paint::pieces`]) changed the pixels of
//! those pieces and no others, so the drawings since a frame changed the
//! pixels of all their pieces together — kept as pieces, not the rectangle
//! holding them, and joined as a drawing's are once there are more than a
//! patch carries.

use alo_paint::{Region, Repainted};

/// The pixels painted again since the last frame was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unsent {
    /// None.
    Nothing,
    /// These pieces' pixels, and no others.
    Pieces(Vec<Region>),
    /// Every pixel: no frame sent yet, or a drawing painted whole.
    Whole,
}

impl Unsent {
    /// What was unsent, and a drawing that `repainted` in `pieces` — the
    /// pieces it was painted in, or none when it gives only the region
    /// holding them.
    #[must_use]
    pub(crate) fn joined(self, repainted: Repainted, pieces: &[Region]) -> Self {
        let painted: Vec<Region> = match repainted {
            Repainted::Nothing => return self,
            Repainted::Whole => return Unsent::Whole,
            Repainted::Within(bounds) if pieces.is_empty() => vec![bounds],
            Repainted::Within(_) => pieces.to_vec(),
        };
        match self {
            Unsent::Whole => Unsent::Whole,
            Unsent::Nothing => Unsent::Pieces(painted),
            Unsent::Pieces(mut was) => {
                was.extend(painted);
                if was.len() > crate::frame_patch::MOST_PIECES {
                    was = alo_paint::pieces::cut(&was, &[]);
                }
                Unsent::Pieces(was)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(left: u32, top: u32, right: u32, bottom: u32) -> Region {
        Region::new(left, top, right, bottom).expect("a region")
    }

    #[test]
    fn drawings_since_a_frame_are_their_pieces_together() {
        let (one, two) = (region(0, 0, 4, 4), region(900, 900, 910, 910));
        let unsent = Unsent::Nothing.joined(Repainted::Within(one.joined(two)), &[one, two]);
        assert_eq!(unsent, Unsent::Pieces(vec![one, two]));
        let three = region(400, 0, 410, 10);
        let unsent = unsent.joined(Repainted::Within(three), &[]);
        assert_eq!(unsent, Unsent::Pieces(vec![one, two, three]));
        assert_eq!(
            unsent.clone().joined(Repainted::Nothing, &[]),
            unsent,
            "nothing painted adds nothing"
        );
        assert_eq!(unsent.joined(Repainted::Whole, &[]), Unsent::Whole);
        assert_eq!(
            Unsent::Whole.joined(Repainted::Within(one), &[one]),
            Unsent::Whole
        );
    }

    #[test]
    fn more_pieces_than_a_patch_carries_are_joined_and_hold_every_one() {
        let mut unsent = Unsent::Nothing;
        let mut all = Vec::new();
        for k in 0..30 {
            let piece = region(k * 100, 0, k * 100 + 3, 3);
            all.push(piece);
            unsent = unsent.joined(Repainted::Within(piece), &[piece]);
        }
        let Unsent::Pieces(pieces) = unsent else {
            panic!("pieces");
        };
        assert!(pieces.len() <= crate::frame_patch::MOST_PIECES);
        for each in all {
            assert!(
                pieces.iter().any(|piece| piece.joined(each) == *piece),
                "{each:?} held"
            );
        }
    }
}

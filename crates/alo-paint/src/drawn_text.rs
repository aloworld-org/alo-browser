/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A run of text drawn: outlined once, and passed over where it cannot be
//! seen (queue item 888).
//!
//! Measured in the first screen's renderer at two (release, this Mac), a
//! run costs about 0.05 ms to shape and outline and about 0.59 ms to fill:
//! the filling is the run. Every run of the screen was filled on every
//! drawing, whatever region was painted again, so a Tab in *Settings* over
//! a card spent 29 of its 30 ms of paint filling 41 runs of which a ring's
//! region saw none.
//!
//! So, on a confined canvas ([`crate::repaint`]), a run is outlined and the
//! bound of its ink is asked of the outlines themselves
//! ([`crate::extent::letters_reach`], the very bound a changed run is
//! damaged by): **never too small**, since it is the shape that would be
//! filled, whole pixels outwards and one more, and each shadow by its offset
//! and the whole reach of its blur. A run that cannot reach what the clips
//! leave open of the region is not filled.
//!
//! Its letters are not kept between drawings as a box's fill is
//! ([`crate::kept`]): measured opening *Settings*, 37 of the 41 runs filled
//! were the panel's own, new to that drawing, so keeping them would have
//! saved about 2 ms for the memory of every run on the screen.
//!
//! But inside one drawing they are (queue item 895): a repaint cut into
//! pieces draws the whole list once per piece ([`crate::pieces`]), and every
//! run was outlined and bounded again for each, about 2 ms at two for
//! *Settings*' 41. A run's outlines and its bound are a function of the run
//! and the transform it is drawn under, the same in every piece of the
//! drawing — so each is made once for the drawing, by the item's place in
//! its list, and taken by every other piece ([`Kept::run`]).

use crate::canvas::Canvas;
use crate::clip::Clip;
use crate::display::DisplayItem;
use crate::extent::{Reach, letters_reach};
use crate::kept::{Kept, Made, Run};
use crate::paint::Paint;
use crate::path::{Path, Point};
use crate::raster::fill;
use crate::render::{draw_coverage, draw_letters_shadow, moved, outlined_glyphs_at};
use crate::synthesis::leaned;
use alo_text::Font;
use alo_value::Matrix;

/// What a run is set as: its words, face, size, the room after each
/// character, where its pen starts and how far its letters lean.
pub(crate) struct Set<'a> {
    pub text: &'a str,
    pub font: &'a Font,
    pub size: f32,
    pub letter_spacing: f32,
    pub origin: (f32, f32),
    pub oblique: f32,
}

/// A run's glyphs as they are drawn, each where it is drawn, on a screen of
/// `scale`: those with an outline, each leaned as the run is. What is
/// filled, and what its ink is bounded by.
pub(crate) fn run_glyphs(set: &Set<'_>, scale: f32) -> Vec<Path> {
    outlined_glyphs_at(
        set.text,
        set.font,
        set.size,
        scale,
        set.letter_spacing,
        set.origin,
    )
    .iter()
    .filter(|glyph| !glyph.is_empty())
    .map(|glyph| leaned(glyph, set.oblique, set.origin.1))
    .collect()
}

/// A text item and the shadows it casts, under the transform in force, its
/// letters set as they are on a screen at `scale` — or nothing, when
/// `misses` says what the run reaches is not to be painted: asked with how
/// to bound it, which it bounds only on a confined canvas, so a drawing
/// painted whole spends nothing on it. A run filled is
/// counted when coverage is kept ([`crate::kept::Making::letters`]).
/// Anything that is not a text item draws nothing.
///
/// With coverage kept, the run's outlines and the bound of its ink are made
/// once a drawing, for the item at `index` of its list, and taken by every
/// other piece the drawing is painted in.
pub(crate) fn draw_text(
    target: &mut Canvas,
    (index, item): (usize, &DisplayItem),
    (current, scale): (Matrix, f32),
    clip: Option<&Clip>,
    misses: &dyn Fn(&dyn Fn() -> Reach) -> bool,
    mut kept: Option<&mut Kept>,
) {
    let DisplayItem::Text {
        text,
        origin,
        font,
        oblique,
        size,
        letter_spacing,
        color,
        shadows,
        ..
    } = item
    else {
        return;
    };
    let set = Set {
        text,
        font,
        size: *size,
        letter_spacing: *letter_spacing,
        origin: *origin,
        oblique: *oblique,
    };
    // The run is outlined once and drawn once per shadow plus once for
    // itself: shaping and outlining are cheap beside filling, and a shadow
    // is the same letters somewhere else. A lean is part of the letters, so
    // a shadow leans with them. Each glyph casts its own shadow, softened on
    // its own (queue item 831); the letters are filled as one shape.
    let page = (target.width(), target.height());
    let run = match kept.as_deref_mut() {
        Some(kept) => kept.run((index, current, scale), &set, || run_glyphs(&set, scale)),
        None => Run::of(run_glyphs(&set, scale)),
    };
    let glyphs = run.glyphs();
    // The bound, as this drawing first made it or made now and kept for its
    // other pieces: asked only on a confined canvas.
    let made = core::cell::Cell::new(None);
    let passed_over = misses(&|| {
        let reach = run
            .reach()
            .unwrap_or_else(|| letters_reach(glyphs, shadows, current, page));
        made.set(Some(reach));
        reach
    });
    if let (Some(kept), Some(reach)) = (kept.as_deref_mut(), made.get()) {
        kept.run_reach((index, current, scale), reach);
    }
    if passed_over {
        return;
    }
    // Furthest back first, so the first shadow written ends up on top —
    // which is the order CSS draws them in.
    for shadow in shadows.iter().rev() {
        let by = Point::new(shadow.offset.0, shadow.offset.1);
        let placed: Vec<Path> = glyphs.iter().map(|glyph| glyph.translated(by)).collect();
        draw_letters_shadow(target, &placed, (shadow.blur, shadow.color), current, clip);
    }
    let mut outlined = Path::new();
    for glyph in glyphs {
        outlined.extend(glyph);
    }
    if let Some(kept) = kept {
        kept.count(Made::Letters);
    }
    draw_coverage(
        target,
        &fill(&moved(&outlined, current)),
        &Paint::Solid(*color),
        current,
        clip,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DisplayList, TextShadow};
    use crate::fill_rule::FillRule;
    use crate::region::Region;
    use crate::render::render;
    use crate::repaint::{Repainted, repaint};
    use alo_box::BoxId;
    use alo_text::{Slant, Weight};
    use alo_value::Rgba;

    fn face(family: &str, weight: Weight, bytes: &[u8]) -> Font {
        let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) else {
            panic!("the faces this crate is tested with");
        };
        font
    }

    fn faces() -> Vec<Font> {
        vec![
            face("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
            face("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
            face("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
        ]
    }

    fn run(text: &str, font: &Font, size: f32, origin: (f32, f32)) -> DisplayItem {
        DisplayItem::Text {
            box_id: BoxId::from_index_for_tests(0),
            text: text.to_owned(),
            origin,
            font: font.clone(),
            oblique: 0.0,
            size,
            letter_spacing: 0.0,
            color: Rgba::BLACK,
            shadows: Vec::new(),
        }
    }

    fn rectangle(x: f32, y: f32, width: f32, height: f32, color: Rgba) -> DisplayItem {
        DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, y, width, height),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        }
    }

    /// A transform a quarter of the way round and grown, as a box's own.
    fn turned() -> Matrix {
        let (sin, cos) = 0.5_f32.sin_cos();
        Matrix {
            a: cos * 1.3,
            b: sin * 1.3,
            c: -sin * 1.3,
            d: cos * 1.3,
            e: 60.0,
            f: -10.0,
        }
    }

    /// The bound is never too small: whatever face, size, scale, spacing,
    /// lean, shadows and transform, every pixel a run changes drawn alone is
    /// inside the region its outlines bound it by.
    #[test]
    fn a_runs_ink_and_its_shadows_never_reach_past_their_bound() {
        let shadows = [
            Vec::new(),
            vec![TextShadow {
                offset: (7.0, -5.0),
                blur: 4.0,
                color: Rgba::BLACK,
            }],
            vec![
                TextShadow {
                    offset: (-3.0, 9.0),
                    blur: 0.0,
                    color: Rgba::BLACK,
                },
                TextShadow {
                    offset: (0.0, 0.0),
                    blur: 11.0,
                    color: Rgba::BLACK,
                },
            ],
        ];
        let mut checked = 0;
        for font in faces() {
            for size in [13.337, 40.0] {
                for scale in [1.0, 1.5, 2.0] {
                    for (letter_spacing, oblique) in [(0.0, 0.0), (3.5, 14.0), (-1.0, -20.0)] {
                        for cast in &shadows {
                            for own in [Matrix::IDENTITY, Matrix::translation(3.25, 1.5), turned()]
                            {
                                let item = DisplayItem::Text {
                                    box_id: BoxId::from_index_for_tests(0),
                                    text: "Ágj fly ÿ — Wq\u{301}".to_owned(),
                                    origin: (30.0, 70.0),
                                    font: font.clone(),
                                    oblique,
                                    size,
                                    letter_spacing,
                                    color: Rgba::BLACK,
                                    shadows: cast.clone(),
                                };
                                let current = own.then(Matrix::scaling(scale));
                                let size_px = (900, 400);
                                let mut canvas =
                                    Canvas::new(size_px.0, size_px.1, Rgba::TRANSPARENT);
                                draw_text(
                                    &mut canvas,
                                    (0, &item),
                                    (current, scale),
                                    None,
                                    &|_| false,
                                    None,
                                );
                                let set = Set {
                                    text: "Ágj fly ÿ — Wq\u{301}",
                                    font: &font,
                                    size,
                                    letter_spacing,
                                    origin: (30.0, 70.0),
                                    oblique,
                                };
                                let glyphs = run_glyphs(&set, scale);
                                let Reach::Within(bound) =
                                    letters_reach(&glyphs, cast, current, size_px)
                                else {
                                    panic!("a run on the canvas reaches a region");
                                };
                                let mut inked = 0;
                                for y in 0..size_px.1 {
                                    for x in 0..size_px.0 {
                                        let touched =
                                            canvas.at(x, y).is_some_and(|pixel| pixel.alpha > 0.0);
                                        if touched {
                                            inked += 1;
                                            assert!(
                                                bound.holds(x, y),
                                                "{} {size} at {scale}, spacing {letter_spacing}, lean {oblique}, {} shadows, {own:?}: ({x}, {y}) inked outside {bound:?}",
                                                font.family(),
                                                cast.len()
                                            );
                                        }
                                    }
                                }
                                assert!(inked > 0, "the run drew something");
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 3 * 2 * 3 * 3 * 3 * 3);
    }

    /// Rows of words, a mark among them, and that mark changed: the region
    /// painted again is around the mark, every pixel comes out as the new
    /// list drawn whole, and only the runs that could reach it are filled —
    /// one whose own letters are far from it but whose shadow falls in it
    /// among them.
    #[test]
    fn a_run_that_cannot_reach_the_region_painted_again_is_not_filled() {
        let font = &faces()[0];
        let list = |mark: Rgba| {
            let mut items = vec![rectangle(0.0, 0.0, 400.0, 300.0, Rgba::WHITE)];
            for row in 0..12u8 {
                let y = 20.0 + f32::from(row) * 22.0;
                items.push(run("The capital and largest city", font, 16.0, (10.0, y)));
            }
            // Its letters well above the mark, its shadow cast onto it.
            let mut casting = run("cast", font, 16.0, (300.0, 40.0));
            if let DisplayItem::Text { shadows, .. } = &mut casting {
                shadows.push(TextShadow {
                    offset: (0.0, 200.0),
                    blur: 2.0,
                    color: Rgba::BLACK,
                });
            }
            items.push(casting);
            items.push(rectangle(300.0, 240.0, 20.0, 20.0, mark));
            DisplayList::from_parts(items, Vec::new(), 2.0)
        };
        let (before, after) = (list(Rgba::BLACK), list(Rgba::WHITE));
        let size = (800, 600);
        for (pass, old) in [(1, &before), (2, &after)] {
            let mut canvas = Canvas::new(size.0, size.1, Rgba::WHITE);
            let mut kept = Kept::new();
            crate::render::render_keeping(old, &mut canvas, &mut kept);
            assert_eq!(kept.made().letters, 13, "drawn whole, every run is filled");
            let repainted = repaint(old, &after, &mut canvas, Rgba::WHITE, &mut kept);
            let mut whole = Canvas::new(size.0, size.1, Rgba::WHITE);
            render(&after, &mut whole);
            assert!(canvas == whole, "pass {pass}: as the list drawn whole");
            if pass == 2 {
                assert_eq!(repainted, Repainted::Nothing);
                continue;
            }
            let Repainted::Within(region) = repainted else {
                panic!("the mark's change is painted again within a region: {repainted:?}");
            };
            assert!(
                region.holds(610, 490)
                    && !region.holds(20, 490)
                    && region
                        .meeting(Region::new(0, 0, 800, 460).unwrap_or(region))
                        .is_none(),
                "the region is around the mark: {region:?}"
            );
            assert_eq!(
                kept.made().letters,
                1,
                "only the run whose shadow falls on the mark is filled"
            );
        }
    }
    /// Rows of words and two marks far apart, both changed: two pieces,
    /// each drawing the whole list — and every run outlined once for both
    /// (queue item 895), and not at all by the next drawing, which sets
    /// them alike; every pixel as the new list drawn whole.
    #[test]
    fn a_run_is_outlined_once_however_many_pieces_a_drawing_is_painted_in() {
        let font = &faces()[0];
        let list = |mark: Rgba, words: &str| {
            let mut items = vec![rectangle(0.0, 0.0, 1200.0, 900.0, Rgba::WHITE)];
            for row in 0..30u8 {
                let y = 20.0 + f32::from(row) * 28.0;
                items.push(run(words, font, 16.0, (400.0, y)));
            }
            items.push(rectangle(10.0, 10.0, 20.0, 20.0, mark));
            items.push(rectangle(1150.0, 850.0, 20.0, 20.0, mark));
            DisplayList::from_parts(items, Vec::new(), 1.0)
        };
        let words = "The capital and largest city";
        let before = list(Rgba::BLACK, words);
        let after = list(Rgba::from_rgba8(200, 30, 30, 255), words);
        let size = (1200, 900);
        let mut canvas = Canvas::new(size.0, size.1, Rgba::WHITE);
        render(&before, &mut canvas);
        let mut kept = Kept::new();
        for (was, now, outlined) in [(&before, &after, 30), (&after, &before, 0)] {
            repaint(was, now, &mut canvas, Rgba::WHITE, &mut kept);
            let mut whole = Canvas::new(size.0, size.1, Rgba::WHITE);
            render(now, &mut whole);
            assert!(canvas == whole, "as the list drawn whole");
            let made = kept.made();
            assert_eq!(made.pieces, 2, "{made:?}");
            assert_eq!(made.outlined, outlined, "{made:?}");
            assert_eq!(made.letters, 0, "no run reaches either mark: {made:?}");
        }
        // Set otherwise — another word — a run is outlined again.
        let other = list(Rgba::BLACK, "The capital and largest town");
        let mut whole = Canvas::new(size.0, size.1, Rgba::WHITE);
        render(&other, &mut whole);
        repaint(&before, &other, &mut canvas, Rgba::WHITE, &mut kept);
        assert!(canvas == whole, "as the list drawn whole");
        assert_eq!(kept.made().outlined, 30, "{:?}", kept.made());
    }
}

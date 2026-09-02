//! A letter, from a font to coverage.
//!
//! Everything before this could be checked as numbers. This is the first thing
//! in the repository with a *shape*, so it is checked as one: an `l` is a
//! vertical bar, an `H` has a gap in the middle of its top row and none in its
//! middle row, and a space is nothing at all.
//!
//! It is deliberately not an image comparison. `CLAUDE.md` asks for a
//! reference render for anything visual and item 7 is where one arrives, with
//! a canvas to render onto; a coverage mask on its own is better asserted by
//! saying what shape it is than by committing a picture of it.

use alo_paint::{Coverage, fill, outline};
use alo_text::{Direction, Font, Slant, Weight, shape};

fn sans() -> Option<Font> {
    Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    )
}

/// The coverage of one character, drawn at a size.
///
/// Empty when the font could not be loaded at all, which the assertion that
/// asked will report — this helper does not know what the caller wanted.
fn coverage_of(character: char, size: f32) -> Coverage {
    let Some(font) = sans() else {
        return Coverage::empty();
    };
    let run = shape(&character.to_string(), &font, size, Direction::LeftToRight);
    let Some(id) = run.glyphs.first().map(|glyph| glyph.glyph_id) else {
        return Coverage::empty();
    };
    match outline(&font, id, size) {
        Some(glyph) => fill(&glyph.path),
        None => Coverage::empty(),
    }
}

/// The mask as text, one character a pixel: `#` solid, `+` partly, `.` not.
fn picture(coverage: &Coverage) -> String {
    let mut out = String::new();
    for y in 0..coverage.height() {
        for x in 0..coverage.width() {
            out.push(match coverage.at(x, y) {
                0..=32 => '.',
                33..=223 => '+',
                _ => '#',
            });
        }
        out.push('\n');
    }
    out
}

#[test]
fn an_l_is_a_vertical_bar() {
    let coverage = coverage_of('l', 40.0);
    assert!(!coverage.is_empty());
    assert!(
        coverage.height() > coverage.width() * 3,
        "an l is much taller than it is wide: {}×{}",
        coverage.width(),
        coverage.height(),
    );

    // Every row of it is inked, all the way down.
    for y in 0..coverage.height() {
        let inked = (0..coverage.width()).any(|x| coverage.at(x, y) > 32);
        assert!(inked, "row {y} of an l has no ink:\n{}", picture(&coverage));
    }
}

#[test]
fn an_h_has_two_uprights_and_a_bar_between_them() {
    let coverage = coverage_of('H', 40.0);
    let picture = picture(&coverage);

    // Near the top there is a gap between the uprights; in the middle the
    // crossbar fills it.
    let inked_in_row = |y: u32| -> Vec<u32> {
        (0..coverage.width())
            .filter(|x| coverage.at(*x, y) > 32)
            .collect()
    };
    let top = inked_in_row(1);
    let middle = inked_in_row(coverage.height() / 2);

    assert!(!top.is_empty() && !middle.is_empty(), "\n{picture}");
    let gap_at_top = top.windows(2).any(|pair| pair[1] - pair[0] > 1);
    let gap_in_middle = middle.windows(2).any(|pair| pair[1] - pair[0] > 1);

    assert!(gap_at_top, "the top of an H is two uprights:\n{picture}");
    assert!(
        !gap_in_middle,
        "and its middle is joined by the crossbar:\n{picture}",
    );
}

#[test]
fn a_space_covers_nothing() {
    assert!(coverage_of(' ', 40.0).is_empty());
}

#[test]
fn a_letter_sits_above_the_baseline_and_a_descender_below_it() {
    // The mask's origin is where the ink starts, measured from the pen — which
    // sits on the baseline with `y` going down the screen.
    let (_, top) = coverage_of('H', 40.0).origin();
    assert!(top < 0, "an H is entirely above the baseline: {top}");

    let tail = coverage_of('p', 40.0);
    let (_, tail_top) = tail.origin();
    let bottom = tail_top + i32::try_from(tail.height()).unwrap_or(i32::MAX);
    assert!(bottom > 0, "and the tail of a p goes below it: {bottom}");
}

#[test]
fn a_bigger_size_is_a_bigger_letter() {
    let small = coverage_of('H', 20.0);
    let large = coverage_of('H', 40.0);
    assert!(large.width() > small.width());
    assert!(large.height() > small.height());
}

#[test]
fn the_edges_of_a_letter_are_anti_aliased_rather_than_on_or_off() {
    let coverage = coverage_of('o', 40.0);
    let partial = (0..coverage.height())
        .flat_map(|y| (0..coverage.width()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let value = coverage.at(*x, *y);
            value > 0 && value < 255
        })
        .count();
    assert!(
        partial > 20,
        "a round letter has many partly-covered pixels, found {partial}:\n{}",
        picture(&coverage),
    );
}

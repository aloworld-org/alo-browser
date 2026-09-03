//! Text, laid into lines.
//!
//! Shaping says how wide a run is; breaking says where a line may end; this
//! decides where it does end, and that decision is ours. The rule is the one
//! every engine uses and it is worth stating: take break opportunities in
//! order, keep the last one that still fits, and if none fits then overflow
//! rather than break a word in half — because a word cut at an arbitrary point
//! is harder to read than one that sticks out.
//!
//! # What this is not
//!
//! It is not inline formatting. A line here holds one paragraph of text;
//! putting *several inline boxes* on one line, with their baselines aligned,
//! is queue item 16 and lives in layout, where the boxes are. What is here is
//! the half that item needs first: how wide a piece of text is, and where it
//! wraps.

use crate::database::FontDatabase;
use crate::font::FontRequest;
use crate::linebreak::opportunities;
use crate::run::{TextRun, split};
use crate::shape::{ShapedRun, shape};

/// One line of laid-out text.
#[derive(Debug, Clone)]
pub struct Line {
    /// The runs on it, in the order they are drawn.
    pub runs: Vec<ShapedRun>,
    /// How wide the line is, in CSS pixels.
    pub width: f32,
    /// How far above the baseline the tallest thing on it reaches.
    pub ascender: f32,
    /// How far below the baseline the deepest thing on it reaches.
    pub descender: f32,
}

impl Line {
    /// How tall the line is: everything above the baseline plus everything
    /// below it.
    pub fn height(&self) -> f32 {
        self.ascender + self.descender
    }
}

/// A paragraph of text, laid into lines.
#[derive(Debug, Clone, Default)]
pub struct Paragraph {
    /// The lines, top to bottom.
    pub lines: Vec<Line>,
}

impl Paragraph {
    /// How wide the widest line is.
    pub fn width(&self) -> f32 {
        self.lines.iter().map(|line| line.width).fold(0.0, f32::max)
    }

    /// How tall all the lines are together.
    pub fn height(&self) -> f32 {
        self.lines.iter().map(Line::height).sum()
    }

    /// How many lines there are.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Lay text into lines no wider than `available_width`.
///
/// `available_width` of [`None`] is the max-content question — "how wide would
/// this like to be" — and gives one line however long it is. A mandatory break
/// still ends a line whatever the width.
pub fn lay_out(
    text: &str,
    database: &FontDatabase,
    request: &FontRequest,
    size: f32,
    available_width: Option<f32>,
    letter_spacing: f32,
) -> Paragraph {
    if text.is_empty() {
        return Paragraph::default();
    }
    let mut paragraph = Paragraph::default();
    for piece in split_at_mandatory_breaks(text) {
        lay_out_one(
            piece,
            database,
            request,
            size,
            available_width,
            letter_spacing,
            &mut paragraph,
        );
    }
    paragraph
}

/// A paragraph on one line, however wide that is — the max-content width.
pub fn measure_unwrapped(
    text: &str,
    database: &FontDatabase,
    request: &FontRequest,
    size: f32,
) -> Paragraph {
    lay_out(text, database, request, size, None, 0.0)
}

/// The pieces between the breaks that must happen.
///
/// A newline ends a line whatever the width, so the text is cut there first
/// and each piece is wrapped on its own.
fn split_at_mandatory_breaks(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0usize;
    for point in opportunities(text) {
        if !point.mandatory {
            continue;
        }
        if let Some(piece) = text.get(start..point.offset) {
            pieces.push(piece);
        }
        start = point.offset;
    }
    if start < text.len()
        && let Some(rest) = text.get(start..)
    {
        pieces.push(rest);
    }
    if pieces.is_empty() {
        pieces.push(text);
    }
    pieces
}

/// Wrap one piece — text with no mandatory break in it — into lines.
fn lay_out_one(
    text: &str,
    database: &FontDatabase,
    request: &FontRequest,
    size: f32,
    available_width: Option<f32>,
    letter_spacing: f32,
    out: &mut Paragraph,
) {
    let trimmed = text.trim_end_matches(['\n', '\r']);
    let Some(width) = available_width else {
        out.lines
            .push(shape_line(trimmed, database, request, size, letter_spacing));
        return;
    };
    if trimmed.is_empty() {
        // A blank line between two paragraphs is a line, and it is as tall as
        // the font would have been. Dropping it would close the gap the author
        // asked for.
        out.lines
            .push(shape_line("", database, request, size, letter_spacing));
        return;
    }

    let points = opportunities(trimmed);
    let mut start = 0usize;
    let mut index = 0usize;
    let mut last_fitting: Option<usize> = None;

    while index < points.len() {
        let Some(offset) = points.get(index).map(|point| point.offset) else {
            break;
        };
        if offset <= start {
            index += 1;
            continue;
        }
        let Some(candidate) = trimmed.get(start..offset) else {
            index += 1;
            continue;
        };
        if shape_line(
            candidate.trim_end(),
            database,
            request,
            size,
            letter_spacing,
        )
        .width
            <= width
        {
            last_fitting = Some(offset);
            index += 1;
            continue;
        }
        if let Some(end) = last_fitting {
            if let Some(line) = trimmed.get(start..end) {
                out.lines.push(shape_line(
                    line.trim_end(),
                    database,
                    request,
                    size,
                    letter_spacing,
                ));
            }
            start = end;
            last_fitting = None;
            // Deliberately not advancing: this opportunity has not been
            // considered from the new start yet, and skipping it is how a line
            // ends up wider than the width it was given.
        } else {
            // One unbreakable piece is wider than the whole line. It goes on a
            // line of its own and sticks out, because a word cut at an
            // arbitrary point is harder to read than one that overflows.
            out.lines.push(shape_line(
                candidate.trim_end(),
                database,
                request,
                size,
                letter_spacing,
            ));
            start = offset;
            index += 1;
        }
    }

    if start < trimmed.len()
        && let Some(rest) = trimmed.get(start..)
    {
        out.lines.push(shape_line(
            rest.trim_end(),
            database,
            request,
            size,
            letter_spacing,
        ));
    }
}

/// Shape one line's worth of text into its runs.
fn shape_line(
    text: &str,
    database: &FontDatabase,
    request: &FontRequest,
    size: f32,
    letter_spacing: f32,
) -> Line {
    let mut runs = Vec::new();
    let mut width = 0.0;
    let mut ascender: f32 = 0.0;
    let mut descender: f32 = 0.0;

    for TextRun {
        text,
        direction,
        font,
        ..
    } in split(text, database, request)
    {
        let Some(font) = font else {
            // Nothing has these characters. They take no room rather than
            // being drawn as something they are not; `Font::has_glyph` said so
            // and the caller can see the gap.
            continue;
        };
        let shaped = crate::shape::spaced(shape(text, &font, size, direction), letter_spacing);
        width += shaped.width;
        ascender = ascender.max(shaped.ascender);
        descender = descender.max(shaped.descender);
        runs.push(shaped);
    }

    // An empty line is still a line, and it is as tall as the font would have
    // been — which is what keeps a blank line in a paragraph from vanishing.
    if runs.is_empty()
        && let Some(font) = database.chain(request).first()
    {
        let metrics = font.metrics(size);
        ascender = metrics.ascender;
        descender = metrics.descender;
    }

    Line {
        runs,
        width,
        ascender,
        descender,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Font, Slant, Weight};

    fn database() -> FontDatabase {
        let mut database = FontDatabase::new();
        if let Some(font) = Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        ) {
            database.add(font);
        }
        database
    }

    fn request() -> FontRequest {
        FontRequest::family("DejaVu Sans")
    }

    fn wrapped(text: &str, width: f32) -> Vec<String> {
        lay_out(text, &database(), &request(), 16.0, Some(width), 0.0)
            .lines
            .iter()
            .map(|line| {
                line.runs
                    .iter()
                    .flat_map(|run| run.glyphs.iter())
                    .count()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn nothing_lays_out_to_nothing() {
        let paragraph = lay_out("", &database(), &request(), 16.0, Some(100.0), 0.0);
        assert!(paragraph.is_empty());
        assert!((paragraph.width() - 0.0).abs() < f32::EPSILON);
        assert!((paragraph.height() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn text_that_fits_is_one_line() {
        let paragraph = lay_out("one two", &database(), &request(), 16.0, Some(1000.0), 0.0);
        assert_eq!(paragraph.len(), 1);
        assert!(paragraph.width() > 0.0);
        assert!(paragraph.height() > 0.0);
    }

    #[test]
    fn the_unwrapped_width_is_the_width_of_the_whole_thing_on_one_line() {
        let one_line = measure_unwrapped("one two three", &database(), &request(), 16.0);
        assert_eq!(one_line.len(), 1);

        let narrow = lay_out(
            "one two three",
            &database(),
            &request(),
            16.0,
            Some(one_line.width() / 2.0),
            0.0,
        );
        assert!(narrow.len() > 1, "and half that width takes more lines");
        assert!(narrow.width() <= one_line.width());
    }

    #[test]
    fn every_line_fits_inside_the_width_it_was_given() {
        let width = 80.0;
        let paragraph = lay_out(
            "the quick brown fox jumps over the lazy dog",
            &database(),
            &request(),
            16.0,
            Some(width),
            0.0,
        );
        assert!(paragraph.len() > 1);
        for line in &paragraph.lines {
            assert!(
                line.width <= width + 0.001,
                "a line of {} in a width of {width}",
                line.width,
            );
        }
    }

    #[test]
    fn a_word_wider_than_the_line_overflows_rather_than_being_cut_in_half() {
        let paragraph = lay_out(
            "extraordinarily",
            &database(),
            &request(),
            16.0,
            Some(10.0),
            0.0,
        );
        assert_eq!(paragraph.len(), 1, "one word is one line");
        assert!(
            paragraph.width() > 10.0,
            "and it sticks out, which is easier to read than a word cut anywhere",
        );
    }

    #[test]
    fn a_newline_ends_a_line_however_wide_the_room_is() {
        let paragraph = lay_out(
            "one\ntwo",
            &database(),
            &request(),
            16.0,
            Some(10000.0),
            0.0,
        );
        assert_eq!(paragraph.len(), 2);
    }

    #[test]
    fn a_line_is_as_tall_as_the_font_reaches_above_and_below_the_baseline() {
        let paragraph = lay_out("x", &database(), &request(), 16.0, Some(1000.0), 0.0);
        let line = paragraph.lines.first().expect("one line");
        assert!(line.ascender > 0.0);
        assert!(line.descender > 0.0);
        assert!((line.height() - (line.ascender + line.descender)).abs() < 0.001);
    }

    #[test]
    fn a_blank_line_is_still_a_line_with_a_height() {
        let paragraph = lay_out(
            "one\n\ntwo",
            &database(),
            &request(),
            16.0,
            Some(1000.0),
            0.0,
        );
        assert_eq!(paragraph.len(), 3);
        for line in &paragraph.lines {
            assert!(line.height() > 0.0, "including the empty one");
        }
    }

    #[test]
    fn right_to_left_text_is_measured_the_same_way_as_any_other() {
        let paragraph = measure_unwrapped("مرحبا بالعالم", &database(), &request(), 16.0);
        assert_eq!(paragraph.len(), 1);
        assert!(paragraph.width() > 0.0);
        let line = paragraph.lines.first().expect("one line");
        assert!(!line.runs.is_empty());
    }

    #[test]
    fn a_wider_size_makes_a_wider_paragraph() {
        let small = measure_unwrapped("hello", &database(), &request(), 16.0);
        let large = measure_unwrapped("hello", &database(), &request(), 32.0);
        assert!((large.width() - small.width() * 2.0).abs() < 0.01);
    }

    #[test]
    fn wrapping_keeps_every_glyph_the_text_had() {
        let all_on_one_line: usize =
            measure_unwrapped("the quick brown fox", &database(), &request(), 16.0)
                .lines
                .iter()
                .flat_map(|line| line.runs.iter())
                .map(|run| run.glyphs.len())
                .sum();

        let wrapped_count: usize = wrapped("the quick brown fox", 60.0)
            .iter()
            .filter_map(|count| count.parse::<usize>().ok())
            .sum();

        // Wrapping drops the spaces at the ends of lines and nothing else.
        assert!(wrapped_count <= all_on_one_line);
        assert!(wrapped_count >= all_on_one_line - 4);
    }
}

//! Where a grid item goes: `grid-row` and `grid-column`.

use alo_value::{is_keyword, parse_number};
use core::fmt;

/// One end of a grid item's placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridLine {
    /// Wherever the grid's flow puts it.
    #[default]
    Auto,
    /// A numbered line. Negative numbers count from the far end, which is how
    /// `-1` means "the last line" without knowing how many there are.
    Line(i16),
    /// How many tracks to cover, rather than where to start or stop.
    Span(u16),
}

impl GridLine {
    /// Read one end.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || is_keyword(text, "auto") {
            return Some(GridLine::Auto);
        }
        if let Some(count) = text
            .strip_prefix("span")
            .or_else(|| text.strip_prefix("SPAN"))
            .filter(|rest| rest.starts_with(char::is_whitespace) || rest.is_empty())
        {
            let count = parse_number(count.trim())?;
            if count < 1.0 || count.fract() != 0.0 || count > f32::from(u16::MAX) {
                return None;
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked above to be a whole number in range"
            )]
            return Some(GridLine::Span(count as u16));
        }
        let line = parse_number(text)?;
        // Line zero does not exist: CSS counts from one in both directions.
        if line == 0.0 || line.fract() != 0.0 || line.abs() > f32::from(i16::MAX) {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "checked above to be a whole number in range"
        )]
        Some(GridLine::Line(line as i16))
    }
}

impl fmt::Display for GridLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridLine::Auto => f.write_str("auto"),
            GridLine::Line(line) => write!(f, "{line}"),
            GridLine::Span(count) => write!(f, "span {count}"),
        }
    }
}

/// A whole `grid-row` or `grid-column`: where it starts and where it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPlacement {
    /// Where it starts.
    pub start: GridLine,
    /// Where it stops.
    pub end: GridLine,
}

impl GridPlacement {
    /// Read a `grid-row` or `grid-column`, with or without its `/`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        match text.split_once('/') {
            Some((start, end)) => Some(Self {
                start: GridLine::parse(start)?,
                end: GridLine::parse(end)?,
            }),
            // One value places the start and leaves the end to the flow, which
            // is what CSS says a single value means.
            None => Some(Self {
                start: GridLine::parse(text)?,
                end: GridLine::Auto,
            }),
        }
    }
}

impl fmt::Display for GridPlacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} / {}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numbered_line_is_read_as_a_number() {
        assert_eq!(GridLine::parse("1"), Some(GridLine::Line(1)));
        assert_eq!(GridLine::parse("-1"), Some(GridLine::Line(-1)));
        assert_eq!(GridLine::parse("  3  "), Some(GridLine::Line(3)));
    }

    #[test]
    fn line_zero_does_not_exist_because_css_counts_from_one_both_ways() {
        assert_eq!(GridLine::parse("0"), None);
        assert_eq!(GridLine::parse("1.5"), None);
    }

    #[test]
    fn a_span_says_how_many_rather_than_where() {
        assert_eq!(GridLine::parse("span 2"), Some(GridLine::Span(2)));
        assert_eq!(GridLine::parse("SPAN 3"), Some(GridLine::Span(3)));
        assert_eq!(
            GridLine::parse("span 0"),
            None,
            "a span of nothing is not one"
        );
        assert_eq!(GridLine::parse("span"), None);
        assert_eq!(
            GridLine::parse("spanish"),
            None,
            "the word has to be the word",
        );
    }

    #[test]
    fn auto_and_nothing_are_both_auto() {
        assert_eq!(GridLine::parse("auto"), Some(GridLine::Auto));
        assert_eq!(GridLine::parse(""), Some(GridLine::Auto));
        assert_eq!(GridLine::default(), GridLine::Auto);
    }

    #[test]
    fn a_placement_with_a_slash_has_two_ends_and_without_one_has_one() {
        assert_eq!(
            GridPlacement::parse("1 / 3"),
            Some(GridPlacement {
                start: GridLine::Line(1),
                end: GridLine::Line(3),
            }),
        );
        assert_eq!(
            GridPlacement::parse("2"),
            Some(GridPlacement {
                start: GridLine::Line(2),
                end: GridLine::Auto,
            }),
        );
        assert_eq!(
            GridPlacement::parse("1 / span 2"),
            Some(GridPlacement {
                start: GridLine::Line(1),
                end: GridLine::Span(2),
            }),
        );
    }

    #[test]
    fn a_placement_this_engine_cannot_read_is_refused_whole() {
        for text in ["header", "1 / banana", "banana / 1", "1 / 2 / 3"] {
            assert_eq!(GridPlacement::parse(text), None, "{text}");
        }
    }

    #[test]
    fn a_placement_writes_itself_back_out() {
        assert_eq!(
            GridPlacement::parse("1 / span 2")
                .expect("placement")
                .to_string(),
            "1 / span 2",
        );
        assert_eq!(GridPlacement::default().to_string(), "auto / auto");
    }
}

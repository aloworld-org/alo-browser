//! Media queries: the two things `docs/features.md` asks for in stage 1.
//!
//! **Width, and `prefers-color-scheme`.** The workspace already ships a light
//! and a dark theme, and its breakpoints are widths, so those two carry the
//! whole of alo's own styling. Everything else in the media grammar is
//! recognised as *not understood* and recorded, rather than guessed at.
//!
//! When a query is not understood, CSS says to replace it with `not all`, and
//! that is what happens here: it never matches. Failing closed matters more in
//! a media query than almost anywhere else, because the alternative is a dark
//! theme's rules leaking into a light one, which looks like a rendering bug and
//! is not one.

use crate::issue::{IssueKind, Location, StyleIssue};
use core::fmt;
use cssparser::{BasicParseError, Parser as CssParser, ParserInput, Token};

/// Which of the two themes the person is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    /// `prefers-color-scheme: light`.
    #[default]
    Light,
    /// `prefers-color-scheme: dark`.
    Dark,
}

impl ColorScheme {
    /// The keyword, as written in CSS.
    pub fn as_str(self) -> &'static str {
        match self {
            ColorScheme::Light => "light",
            ColorScheme::Dark => "dark",
        }
    }
}

impl fmt::Display for ColorScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the device is, when a media query asks.
///
/// This is the whole of the device state stage 1 has. It is a plain struct
/// rather than a trait because there is exactly one implementation and there
/// is no benefit in pretending otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaContext {
    /// The viewport width, in CSS pixels.
    pub width: f32,
    /// The viewport height, in CSS pixels.
    ///
    /// Not asked by any media query this engine evaluates yet — it is here
    /// because `vh` needs it, and a window with a width and no height is not
    /// a window.
    pub height: f32,
    /// The theme being asked for.
    pub color_scheme: ColorScheme,
}

impl MediaContext {
    /// A context, from its parts.
    pub fn new(width: f32, color_scheme: ColorScheme) -> Self {
        Self {
            width,
            // The proportion an ordinary window has, so that `vh` means
            // something for a caller that only said how wide the page is.
            height: width * 0.625,
            color_scheme,
        }
    }

    /// A window of both dimensions.
    #[must_use]
    pub fn sized(width: f32, height: f32, color_scheme: ColorScheme) -> Self {
        Self {
            width,
            height,
            color_scheme,
        }
    }
}

impl Default for MediaContext {
    /// A 1280-pixel light screen: an ordinary window, and the theme a document
    /// gets when nobody has said otherwise.
    fn default() -> Self {
        Self::new(1280.0, ColorScheme::Light)
    }
}

/// The kind of device a query is asking about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaType {
    /// `all`, and what a query with no type means.
    All,
    /// `screen`. This engine is one.
    Screen,
    /// `print`. This engine is not one.
    Print,
    /// Any other media type. Never matches: naming a device we are not is
    /// exactly the case where guessing would be wrong.
    Other(Box<str>),
}

impl MediaType {
    fn from_name(name: &str) -> Self {
        if name.eq_ignore_ascii_case("all") {
            MediaType::All
        } else if name.eq_ignore_ascii_case("screen") {
            MediaType::Screen
        } else if name.eq_ignore_ascii_case("print") {
            MediaType::Print
        } else {
            MediaType::Other(name.to_ascii_lowercase().into())
        }
    }

    fn matches(&self) -> bool {
        matches!(self, MediaType::All | MediaType::Screen)
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaType::All => f.write_str("all"),
            MediaType::Screen => f.write_str("screen"),
            MediaType::Print => f.write_str("print"),
            MediaType::Other(name) => f.write_str(name),
        }
    }
}

/// How a width feature compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// `min-width`: the viewport is at least this wide.
    AtLeast,
    /// `max-width`: the viewport is at most this wide.
    AtMost,
    /// `width`: the viewport is exactly this wide.
    Exactly,
}

/// One `(feature: value)` test.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaCondition {
    /// A width in CSS pixels, compared.
    Width {
        /// How to compare.
        comparison: Comparison,
        /// The width being compared against, in CSS pixels.
        pixels: f32,
    },
    /// `prefers-color-scheme`.
    ColorScheme(ColorScheme),
}

impl MediaCondition {
    /// Whether this condition holds for a device.
    pub fn matches(&self, context: &MediaContext) -> bool {
        match self {
            MediaCondition::Width { comparison, pixels } => match comparison {
                Comparison::AtLeast => context.width >= *pixels,
                Comparison::AtMost => context.width <= *pixels,
                #[allow(clippy::float_cmp)]
                Comparison::Exactly => context.width == *pixels,
            },
            MediaCondition::ColorScheme(scheme) => context.color_scheme == *scheme,
        }
    }
}

impl fmt::Display for MediaCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaCondition::Width { comparison, pixels } => {
                let name = match comparison {
                    Comparison::AtLeast => "min-width",
                    Comparison::AtMost => "max-width",
                    Comparison::Exactly => "width",
                };
                write!(f, "({name}: {pixels}px)")
            }
            MediaCondition::ColorScheme(scheme) => {
                write!(f, "(prefers-color-scheme: {scheme})")
            }
        }
    }
}

/// One media query: a device type and a run of conditions joined by `and`.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaQuery {
    /// A query this engine understands.
    Understood {
        /// Whether the query was written with `not`.
        negated: bool,
        /// The device type being asked about.
        media_type: MediaType,
        /// The conditions, all of which must hold.
        conditions: Vec<MediaCondition>,
    },
    /// A query outside the grammar this engine implements. CSS says to replace
    /// such a query with `not all`, so it never matches — and the text is kept
    /// so a later stage can implement it without re-parsing the sheet.
    Unsupported {
        /// The query, as written.
        source: String,
    },
}

impl MediaQuery {
    /// Whether this query holds for a device.
    pub fn matches(&self, context: &MediaContext) -> bool {
        match self {
            MediaQuery::Understood {
                negated,
                media_type,
                conditions,
            } => {
                let holds = media_type.matches()
                    && conditions
                        .iter()
                        .all(|condition| condition.matches(context));
                holds != *negated
            }
            MediaQuery::Unsupported { .. } => false,
        }
    }
}

impl fmt::Display for MediaQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaQuery::Understood {
                negated,
                media_type,
                conditions,
            } => {
                if *negated {
                    f.write_str("not ")?;
                }
                let type_is_implied = *media_type == MediaType::All && !conditions.is_empty();
                let mut wrote_something = false;
                if !type_is_implied || *negated {
                    write!(f, "{media_type}")?;
                    wrote_something = true;
                }
                for condition in conditions {
                    if wrote_something {
                        f.write_str(" and ")?;
                    }
                    write!(f, "{condition}")?;
                    wrote_something = true;
                }
                Ok(())
            }
            MediaQuery::Unsupported { source } => f.write_str(source),
        }
    }
}

/// A comma-separated list of media queries, as an `@media` prelude is written.
///
/// The list matches if **any** query in it matches, which is what the comma
/// means.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaQueryList {
    queries: Vec<MediaQuery>,
}

impl MediaQueryList {
    /// Parse a media query list, recording every query it could not understand.
    ///
    /// Never fails: an unparseable query becomes one that never matches, per
    /// CSS's own error handling, and the issue says which one and why.
    pub fn parse(input: &mut CssParser<'_, '_>, issues: &mut Vec<StyleIssue>) -> Self {
        if input.is_exhausted() {
            // No prelude at all is `all`, which an empty list already means.
            return Self::default();
        }
        let queries = input.parse_comma_separated_ignoring_errors(
            |input| -> Result<MediaQuery, cssparser::ParseError<'_, ()>> {
                let at = input.current_source_location();
                let start = input.position();
                if let Ok(query) = input.try_parse(parse_one_query) {
                    return Ok(query);
                }
                // Drain the rest of this query so the list moves on to the
                // next comma rather than giving up on the whole prelude.
                while input.next().is_ok() {}
                let source = input.slice_from(start).trim().to_owned();
                issues.push(StyleIssue {
                    kind: IssueKind::UnknownMediaCondition,
                    source: source.clone(),
                    at: Location {
                        line: at.line + 1,
                        column: at.column,
                    },
                });
                Ok(MediaQuery::Unsupported { source })
            },
        );
        Self { queries }
    }

    /// Parse a media query list from text.
    pub fn parse_text(text: &str, issues: &mut Vec<StyleIssue>) -> Self {
        let mut input = ParserInput::new(text);
        Self::parse(&mut CssParser::new(&mut input), issues)
    }

    /// Whether this list applies to a device.
    ///
    /// An empty list is `all` — a query list with nothing in it constrains
    /// nothing — and so it always matches.
    pub fn matches(&self, context: &MediaContext) -> bool {
        self.queries.is_empty() || self.queries.iter().any(|query| query.matches(context))
    }

    /// The queries in the list, in the order they were written.
    pub fn iter(&self) -> core::slice::Iter<'_, MediaQuery> {
        self.queries.iter()
    }

    /// How many queries the list holds.
    pub fn len(&self) -> usize {
        self.queries.len()
    }

    /// Whether the list holds nothing, which means `all`.
    pub fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }
}

impl<'a> IntoIterator for &'a MediaQueryList {
    type Item = &'a MediaQuery;
    type IntoIter = core::slice::Iter<'a, MediaQuery>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for MediaQueryList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, query) in self.queries.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{query}")?;
        }
        Ok(())
    }
}

/// `[not | only]? <media-type> [and <condition>]*`, or
/// `<condition> [and <condition>]*`.
fn parse_one_query<'i>(
    input: &mut CssParser<'i, '_>,
) -> Result<MediaQuery, cssparser::ParseError<'i, ()>> {
    let negated = input
        .try_parse(|input| input.expect_ident_matching("not"))
        .is_ok();

    // `only` exists so that CSS2 user agents skip the query. It says nothing
    // to an engine that understands media queries at all, so it is accepted
    // and has no effect.
    let _ = input.try_parse(|input| input.expect_ident_matching("only"));

    let media_type = input
        .try_parse(CssParser::expect_ident_cloned)
        .ok()
        .map(|name| MediaType::from_name(&name));

    let mut conditions = Vec::new();
    // With a media type, every condition is joined to it by `and`. Without
    // one, the query must start with a condition and `and` joins the rest.
    if media_type.is_none() && !input.is_exhausted() {
        conditions.push(parse_one_condition(input)?);
    }
    while input
        .try_parse(|input| input.expect_ident_matching("and"))
        .is_ok()
    {
        conditions.push(parse_one_condition(input)?);
    }

    input
        .expect_exhausted()
        .map_err(cssparser::ParseError::from)?;
    Ok(MediaQuery::Understood {
        negated,
        media_type: media_type.unwrap_or(MediaType::All),
        conditions,
    })
}

/// `(<feature>: <value>)`.
fn parse_one_condition<'i>(
    input: &mut CssParser<'i, '_>,
) -> Result<MediaCondition, cssparser::ParseError<'i, ()>> {
    input.expect_parenthesis_block()?;
    input.parse_nested_block(|input| {
        let name = input.expect_ident_cloned()?;
        input.expect_colon()?;
        if name.eq_ignore_ascii_case("prefers-color-scheme") {
            let value = input.expect_ident_cloned()?;
            let scheme = if value.eq_ignore_ascii_case("dark") {
                ColorScheme::Dark
            } else if value.eq_ignore_ascii_case("light") {
                ColorScheme::Light
            } else {
                return Err(input.new_custom_error(()));
            };
            return Ok(MediaCondition::ColorScheme(scheme));
        }

        let comparison = if name.eq_ignore_ascii_case("min-width") {
            Comparison::AtLeast
        } else if name.eq_ignore_ascii_case("max-width") {
            Comparison::AtMost
        } else if name.eq_ignore_ascii_case("width") {
            Comparison::Exactly
        } else {
            return Err(input.new_custom_error(()));
        };
        Ok(MediaCondition::Width {
            comparison,
            pixels: expect_pixels(input)?,
        })
    })
}

/// A length in CSS pixels.
///
/// Only `px` and a bare zero. A breakpoint written in `em` depends on a font
/// size that is not settled until the cascade runs, and answering it with a
/// guessed 16 pixels would be a wrong answer that looks like a right one.
fn expect_pixels<'i>(input: &mut CssParser<'i, '_>) -> Result<f32, cssparser::ParseError<'i, ()>> {
    let location = input.current_source_location();
    match input.next()? {
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => Ok(*value),
        #[allow(clippy::float_cmp)]
        Token::Number { value, .. } if *value == 0.0 => Ok(0.0),
        token => {
            let token = token.clone();
            Err(cssparser::ParseError::from(BasicParseError {
                kind: cssparser::BasicParseErrorKind::UnexpectedToken(token),
                location,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> (MediaQueryList, Vec<StyleIssue>) {
        let mut issues = Vec::new();
        let list = MediaQueryList::parse_text(text, &mut issues);
        (list, issues)
    }

    fn understood(text: &str) -> MediaQueryList {
        let (list, issues) = parse(text);
        assert!(issues.is_empty(), "{text} produced {issues:?}");
        list
    }

    const NARROW_LIGHT: MediaContext = MediaContext {
        width: 400.0,
        height: 800.0,
        color_scheme: ColorScheme::Light,
    };
    const WIDE_DARK: MediaContext = MediaContext {
        width: 1600.0,
        height: 800.0,
        color_scheme: ColorScheme::Dark,
    };

    #[test]
    fn a_width_query_compares_the_viewport() {
        let at_least = understood("(min-width: 600px)");
        assert!(!at_least.matches(&NARROW_LIGHT));
        assert!(at_least.matches(&WIDE_DARK));

        let at_most = understood("(max-width: 600px)");
        assert!(at_most.matches(&NARROW_LIGHT));
        assert!(!at_most.matches(&WIDE_DARK));

        let exactly = understood("(width: 400px)");
        assert!(exactly.matches(&NARROW_LIGHT));
        assert!(!exactly.matches(&WIDE_DARK));
    }

    #[test]
    fn a_boundary_width_is_inclusive_at_both_ends() {
        let exactly_600 = MediaContext::new(600.0, ColorScheme::Light);
        assert!(understood("(min-width: 600px)").matches(&exactly_600));
        assert!(understood("(max-width: 600px)").matches(&exactly_600));
    }

    #[test]
    fn a_colour_scheme_query_picks_the_theme() {
        assert!(understood("(prefers-color-scheme: light)").matches(&NARROW_LIGHT));
        assert!(!understood("(prefers-color-scheme: dark)").matches(&NARROW_LIGHT));
        assert!(understood("(prefers-color-scheme: dark)").matches(&WIDE_DARK));
    }

    #[test]
    fn conditions_joined_by_and_must_all_hold() {
        let list = understood("(min-width: 600px) and (prefers-color-scheme: dark)");
        assert!(list.matches(&WIDE_DARK));
        assert!(!list.matches(&NARROW_LIGHT));
        assert!(!list.matches(&MediaContext::new(1600.0, ColorScheme::Light)));
    }

    #[test]
    fn a_comma_separated_list_matches_if_any_query_does() {
        let list = understood("(max-width: 500px), (prefers-color-scheme: dark)");
        assert_eq!(list.len(), 2);
        assert!(list.matches(&NARROW_LIGHT));
        assert!(list.matches(&WIDE_DARK));
        assert!(!list.matches(&MediaContext::new(1600.0, ColorScheme::Light)));
    }

    #[test]
    fn a_media_type_decides_whether_we_are_the_device() {
        assert!(understood("screen").matches(&NARROW_LIGHT));
        assert!(understood("all").matches(&NARROW_LIGHT));
        assert!(
            !understood("print").matches(&NARROW_LIGHT),
            "this engine draws to a screen",
        );
        assert!(!understood("speech").matches(&NARROW_LIGHT));
        assert!(understood("screen and (min-width: 100px)").matches(&NARROW_LIGHT));
    }

    #[test]
    fn not_inverts_the_whole_query() {
        assert!(!understood("not screen").matches(&NARROW_LIGHT));
        assert!(understood("not print").matches(&NARROW_LIGHT));
        assert!(understood("not all and (min-width: 600px)").matches(&NARROW_LIGHT));
        assert!(!understood("not all and (min-width: 600px)").matches(&WIDE_DARK));
    }

    #[test]
    fn only_is_accepted_and_does_nothing() {
        assert!(understood("only screen").matches(&NARROW_LIGHT));
        assert!(understood("only screen and (max-width: 500px)").matches(&NARROW_LIGHT));
    }

    #[test]
    fn an_empty_list_is_all() {
        let list = understood("");
        assert!(list.is_empty());
        assert!(list.matches(&NARROW_LIGHT));
        assert!(list.matches(&WIDE_DARK));
    }

    #[test]
    fn what_we_do_not_understand_never_matches_and_is_recorded() {
        for text in [
            "(min-resolution: 2dppx)",
            "(width >= 600px)",
            "(min-width: 40em)",
            "(orientation: landscape)",
            "(prefers-color-scheme: sepia)",
        ] {
            let (list, issues) = parse(text);
            assert!(!list.matches(&NARROW_LIGHT), "{text} should not match");
            assert!(!list.matches(&WIDE_DARK), "{text} should not match");
            assert_eq!(issues.len(), 1, "{text} should be recorded once");
            assert_eq!(issues[0].kind, IssueKind::UnknownMediaCondition);
            assert!(!issues[0].source.is_empty());
        }
    }

    #[test]
    fn one_bad_query_does_not_take_the_list_down_with_it() {
        let (list, issues) = parse("(min-resolution: 2dppx), (prefers-color-scheme: dark)");
        assert_eq!(list.len(), 2);
        assert_eq!(issues.len(), 1);
        assert!(
            list.matches(&WIDE_DARK),
            "the half we understood still works"
        );
        assert!(!list.matches(&NARROW_LIGHT));
    }

    #[test]
    fn a_query_writes_itself_back_out() {
        for text in [
            "screen",
            "print",
            "not screen",
            "(min-width: 600px)",
            "(max-width: 600px) and (prefers-color-scheme: dark)",
            "screen and (min-width: 600px)",
            "(min-width: 600px), (prefers-color-scheme: dark)",
        ] {
            assert_eq!(understood(text).to_string(), text);
        }
    }

    #[test]
    fn an_unsupported_query_keeps_the_text_it_was_written_with() {
        let (list, _) = parse("(min-resolution: 2dppx)");
        assert_eq!(list.to_string(), "(min-resolution: 2dppx)");
    }

    #[test]
    fn zero_is_a_length_without_a_unit() {
        assert!(understood("(min-width: 0)").matches(&NARROW_LIGHT));
    }

    #[test]
    fn the_default_context_is_an_ordinary_light_window() {
        let context = MediaContext::default();
        assert_eq!(context.color_scheme, ColorScheme::Light);
        assert!(understood("(min-width: 1000px)").matches(&context));
    }
}

//! Reading a value: text in, numbers out.
//!
//! What arrives here is what the cascade produced — specified values, as text,
//! with `var()` already substituted. What leaves is something with a number in
//! it, or nothing at all.
//!
//! **Nothing at all is a real answer.** A value this engine cannot read is
//! refused rather than approximated, and the caller falls back to the
//! property's initial value, which is what CSS does with a value it cannot
//! parse. A guessed length is a wrong pixel, and law 3 says a wrong pixel is a
//! bug.

use crate::calc::{CalcNode, Kind};
use crate::color::{Color, Rgba, from_hsl};
use crate::length::{Length, LengthPercentage};
use crate::unit::Unit;
use cssparser::{Parser as CssParser, ParserInput, Token};

/// Read a whole value as a length, refusing anything left over.
///
/// `16px` is a length; `16px 4px` is two of them and not one, so it is refused
/// rather than silently becoming the first.
pub fn parse_length(text: &str) -> Option<Length> {
    match parse_length_percentage(text)? {
        LengthPercentage::Length(length) => Some(length),
        LengthPercentage::Percentage(_) | LengthPercentage::Calc(_) => None,
    }
}

/// Read a whole value as a length or a percentage, `calc()` included.
pub fn parse_length_percentage(text: &str) -> Option<LengthPercentage> {
    entirely(text, |input| {
        let value = one_length_percentage(input)?;
        Some(value)
    })
}

/// Read a whole value as a plain number.
///
/// `line-height: 1.5` and `flex-grow: 2` are numbers, and so is
/// `calc(3 / 2)`.
pub fn parse_number(text: &str) -> Option<f32> {
    entirely(text, |input| {
        if let Ok(number) = input.try_parse(CssParser::expect_number) {
            return Some(number);
        }
        let node = one_calc(input)?;
        if node.kind()? != Kind::Number {
            return None;
        }
        Some(node.evaluate(crate::length::FontMetrics::default(), 0.0))
    })
}

/// Read a whole value as a colour.
///
/// [`None`] for anything this engine does not implement — `oklch`, `lab`,
/// `color()`, `color-mix()`. Those are different colour spaces, and a colour
/// converted by guesswork is a wrong pixel that looks nearly right, which law
/// 3 calls a bug rather than a task.
pub fn parse_color(text: &str) -> Option<Color> {
    entirely(text, one_color)
}

fn one_color<'i>(input: &mut CssParser<'i, '_>) -> Option<Color> {
    let token = input.next().ok()?.clone();
    match token {
        // `#101014` and its three-, four- and eight-digit relatives. The table
        // is `cssparser`'s, because it is a table.
        Token::Hash(value) | Token::IDHash(value) => {
            let (red, green, blue, alpha) =
                cssparser::color::parse_hash_color(value.as_bytes()).ok()?;
            Some(Color::Rgba(Rgba::from_rgba8(
                red,
                green,
                blue,
                to_byte(alpha),
            )))
        }
        Token::Ident(name) => {
            if name.eq_ignore_ascii_case("transparent") {
                return Some(Color::Rgba(Rgba::TRANSPARENT));
            }
            if name.eq_ignore_ascii_case("currentcolor") {
                return Some(Color::CurrentColor);
            }
            let (red, green, blue) =
                cssparser::color::parse_named_color(&name.to_ascii_lowercase()).ok()?;
            Some(Color::Rgba(Rgba::from_rgba8(red, green, blue, 255)))
        }
        Token::Function(name) => {
            let name = name.to_ascii_lowercase();
            input
                .parse_nested_block(
                    |arguments| -> Result<Option<Color>, cssparser::ParseError<'i, ()>> {
                        Ok(color_function(&name, arguments))
                    },
                )
                .ok()?
        }
        _ => None,
    }
}

/// `rgb()`, `rgba()`, `hsl()` and `hsla()`, in both the modern
/// space-separated form and the legacy comma-separated one.
fn color_function(name: &str, input: &mut CssParser<'_, '_>) -> Option<Color> {
    let is_rgb = name == "rgb" || name == "rgba";
    let is_hsl = name == "hsl" || name == "hsla";
    if !is_rgb && !is_hsl {
        return None;
    }

    let first = channel(input, is_hsl)?;
    // The legacy form separates with commas and the modern one with spaces;
    // both are current CSS, and a sheet written for either has to work.
    let legacy = input.try_parse(CssParser::expect_comma).is_ok();
    let second = channel(input, false)?;
    if legacy {
        input.expect_comma().ok()?;
    }
    let third = channel(input, false)?;

    let alpha = if legacy {
        if input.try_parse(CssParser::expect_comma).is_ok() {
            alpha_channel(input)?
        } else {
            1.0
        }
    } else if input.try_parse(|input| input.expect_delim('/')).is_ok() {
        alpha_channel(input)?
    } else {
        1.0
    };
    input.expect_exhausted().ok()?;

    if is_rgb {
        // `rgb()` takes numbers out of 255 or percentages; both end up here as
        // a fraction of one.
        Some(Color::Rgba(Rgba::new(first, second, third, alpha)))
    } else {
        Some(Color::Rgba(from_hsl(first, second, third, alpha)))
    }
}

/// A fraction from zero to one, as a byte.
fn to_byte(value: f32) -> u8 {
    let scaled = (value * 255.0).round().clamp(0.0, 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to zero..255, so it is a whole number in range"
    )]
    let byte = scaled as u8;
    byte
}

/// One channel: a number, or a percentage.
///
/// `as_angle` is for the hue, which is a number of degrees rather than a
/// fraction of anything — the one channel that is not out of 255 or out of a
/// hundred.
fn channel(input: &mut CssParser<'_, '_>, as_angle: bool) -> Option<f32> {
    if let Ok(percentage) = input.try_parse(CssParser::expect_percentage) {
        return Some(percentage);
    }
    let number = input.try_parse(CssParser::expect_number).ok()?;
    if as_angle {
        return Some(number);
    }
    Some(number / 255.0)
}

/// The alpha channel, which is a fraction rather than out of 255.
fn alpha_channel(input: &mut CssParser<'_, '_>) -> Option<f32> {
    if let Ok(percentage) = input.try_parse(CssParser::expect_percentage) {
        return Some(percentage);
    }
    input.try_parse(CssParser::expect_number).ok()
}

/// Whether a value is exactly this keyword, whatever its case.
///
/// A keyword is not a value with a number in it, so it does not belong to the
/// rest of this file — but asking "is this `auto`" is the question every
/// caller asks first, and every caller asking it slightly differently is how
/// `AUTO` stops working in one place only.
pub fn is_keyword(text: &str, keyword: &str) -> bool {
    text.trim().eq_ignore_ascii_case(keyword)
}

/// Run a parser over the whole of a value, refusing anything left over.
fn entirely<T>(text: &str, parse: impl FnOnce(&mut CssParser<'_, '_>) -> Option<T>) -> Option<T> {
    let mut input = ParserInput::new(text);
    let mut parser = CssParser::new(&mut input);
    let value = parse(&mut parser)?;
    parser.expect_exhausted().ok()?;
    Some(value)
}

/// One length, percentage or `calc()`.
fn one_length_percentage(input: &mut CssParser<'_, '_>) -> Option<LengthPercentage> {
    if let Ok(value) = input.try_parse(|input| one_dimension(input).ok_or(())) {
        return Some(value);
    }
    let node = one_calc(input)?;
    if node.kind()? != Kind::Length {
        return None;
    }
    Some(match node {
        CalcNode::Length(length) => LengthPercentage::Length(length),
        CalcNode::Percentage(percent) => LengthPercentage::Percentage(percent),
        other => LengthPercentage::Calc(Box::new(other)),
    })
}

/// One dimension or percentage token — no expression.
fn one_dimension(input: &mut CssParser<'_, '_>) -> Option<LengthPercentage> {
    let location = input.current_source_location();
    let _ = location;
    match input.next().ok()? {
        Token::Dimension { value, unit, .. } => Some(LengthPercentage::Length(Length {
            value: *value,
            unit: Unit::parse(unit)?,
        })),
        Token::Percentage { unit_value, .. } => {
            Some(LengthPercentage::Percentage(unit_value * 100.0))
        }
        // A bare zero is a length in CSS, and only a zero is: `width: 5` is
        // not five pixels, it is a mistake.
        Token::Number { value, .. } if *value == 0.0 => {
            Some(LengthPercentage::Length(Length::ZERO))
        }
        _ => None,
    }
}

/// A `calc()` function, parsed and type-checked.
fn one_calc(input: &mut CssParser<'_, '_>) -> Option<CalcNode> {
    input
        .try_parse(|input| {
            let name = input.expect_function()?.clone();
            if name.eq_ignore_ascii_case("calc") {
                Ok(())
            } else {
                Err(input.new_basic_unexpected_token_error(Token::Function(name)))
            }
        })
        .ok()?;
    let node = input
        .parse_nested_block(
            |arguments| -> Result<Option<CalcNode>, cssparser::ParseError<'_, ()>> {
                let node = sum(arguments, 0);
                if node.is_some() && arguments.expect_exhausted().is_err() {
                    return Ok(None);
                }
                Ok(node)
            },
        )
        .ok()??;
    // Type-check once, here, so that evaluating later cannot fail.
    node.kind()?;
    Some(node)
}

/// How deep `calc(calc(calc(…)))` may nest.
///
/// A finite limit on a finite input, so that a pathological value costs a
/// refusal rather than the stack.
const MAX_DEPTH: u8 = 32;

/// `<product> ([+-] <product>)*`
///
/// The spaces around `+` and `-` are required by CSS, and for a good reason:
/// `-2px` is a single token, so without the space `calc(1px -2px)` would be
/// two values sitting next to each other rather than a subtraction. The
/// requirement is checked here rather than assumed, which is why the
/// whitespace is read as a token instead of skipped.
fn sum(input: &mut CssParser<'_, '_>, depth: u8) -> Option<CalcNode> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut terms = vec![product(input, depth)?];
    loop {
        // Whitespace, operator and whitespace succeed or fail together, so a
        // missing space rewinds the whole thing rather than eating the space
        // and then failing.
        let Ok(sign) = input.try_parse(|input| {
            input.expect_whitespace()?;
            let token = input.next_including_whitespace()?.clone();
            let sign = match token {
                Token::Delim('+') => '+',
                Token::Delim('-') => '-',
                other => return Err(input.new_basic_unexpected_token_error(other)),
            };
            input.expect_whitespace()?;
            Ok(sign)
        }) else {
            return Some(if terms.len() == 1 {
                terms.pop()?
            } else {
                CalcNode::Sum(terms)
            });
        };
        let term = product(input, depth)?;
        terms.push(if sign == '-' {
            CalcNode::Negate(Box::new(term))
        } else {
            term
        });
    }
}

/// `<value> ([*/] <value>)*`
///
/// Multiplication and division need no spaces around them: `*` and `/` cannot
/// begin a number, so there is nothing for them to be confused with.
fn product(input: &mut CssParser<'_, '_>, depth: u8) -> Option<CalcNode> {
    let mut factors = vec![value(input, depth)?];
    loop {
        let Ok(operator) = input.try_parse(|input| {
            let _ = input.try_parse(CssParser::expect_whitespace);
            let token = input.next_including_whitespace()?.clone();
            match token {
                Token::Delim('*') => Ok('*'),
                Token::Delim('/') => Ok('/'),
                other => Err(input.new_basic_unexpected_token_error(other)),
            }
        }) else {
            return Some(if factors.len() == 1 {
                factors.pop()?
            } else {
                CalcNode::Product(factors)
            });
        };
        let factor = value(input, depth)?;
        factors.push(if operator == '/' {
            CalcNode::Invert(Box::new(factor))
        } else {
            factor
        });
    }
}

/// A number, a dimension, a percentage, a nested `calc()`, or a parenthesised
/// expression.
fn value(input: &mut CssParser<'_, '_>, depth: u8) -> Option<CalcNode> {
    let _ = input.try_parse(CssParser::expect_whitespace);
    if let Ok(value) = input.try_parse(|input| one_dimension(input).ok_or(())) {
        return Some(match value {
            LengthPercentage::Length(length) => CalcNode::Length(length),
            LengthPercentage::Percentage(percent) => CalcNode::Percentage(percent),
            LengthPercentage::Calc(node) => *node,
        });
    }
    if let Ok(number) = input.try_parse(CssParser::expect_number) {
        return Some(CalcNode::Number(number));
    }
    // `(` opens a sub-expression, and a nested `calc(` is the same thing with
    // a name in front of it.
    let opened = input.try_parse(|input| {
        let token = input.next()?.clone();
        match token {
            Token::ParenthesisBlock => Ok(()),
            Token::Function(ref name) if name.eq_ignore_ascii_case("calc") => Ok(()),
            other => Err(input.new_basic_unexpected_token_error(other)),
        }
    });
    opened.ok()?;
    input
        .parse_nested_block(
            |inner| -> Result<Option<CalcNode>, cssparser::ParseError<'_, ()>> {
                let node = sum(inner, depth + 1);
                if node.is_some() && inner.expect_exhausted().is_err() {
                    return Ok(None);
                }
                Ok(node)
            },
        )
        .ok()?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::length::FontMetrics;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    /// A value, in pixels, with a twenty-pixel font and a four-hundred-pixel
    /// basis for percentages.
    fn px(text: &str) -> Option<f32> {
        let metrics = FontMetrics::estimated(20.0, 16.0);
        Some(parse_length_percentage(text)?.to_px(metrics, 400.0))
    }

    #[test]
    fn a_plain_length_in_every_unit_becomes_a_number() {
        assert!(close(px("16px").expect("px"), 16.0));
        assert!(close(px("1in").expect("in"), 96.0));
        assert!(close(px("12pt").expect("pt"), 16.0));
        assert!(close(px("1pc").expect("pc"), 16.0));
        assert!(close(px("2em").expect("em"), 40.0));
        assert!(close(px("2rem").expect("rem"), 32.0));
        assert!(close(px("-4px").expect("negative"), -4.0));
        assert!(close(px("1.5px").expect("fractional"), 1.5));
    }

    #[test]
    fn a_percentage_is_resolved_against_the_basis_it_is_given() {
        assert!(close(px("50%").expect("percentage"), 200.0));
        assert!(close(px("0%").expect("percentage"), 0.0));
        assert!(
            parse_length_percentage("50%")
                .expect("percentage")
                .is_percentage(),
        );
    }

    #[test]
    fn a_bare_zero_is_a_length_and_a_bare_anything_else_is_not() {
        assert!(close(px("0").expect("zero"), 0.0));
        assert_eq!(px("5"), None, "`width: 5` is a mistake, not five pixels");
        assert_eq!(px("5.0"), None);
    }

    #[test]
    fn a_unit_this_engine_does_not_have_is_refused_rather_than_guessed_at() {
        for text in ["50vw", "10vh", "3fr", "1cqw", "2s", "45deg"] {
            assert_eq!(px(text), None, "{text} should be refused");
        }
    }

    #[test]
    fn a_value_with_something_left_over_is_refused() {
        assert_eq!(px("16px 4px"), None, "that is two values, not one");
        assert_eq!(px("16px auto"), None);
        assert_eq!(px("auto"), None);
        assert_eq!(px(""), None);
        assert_eq!(px("   "), None);
    }

    #[test]
    fn whitespace_around_a_value_is_not_part_of_it() {
        assert!(close(px("  16px  ").expect("padded"), 16.0));
    }

    #[test]
    fn calc_adds_and_subtracts_lengths() {
        assert!(close(px("calc(8px + 4px)").expect("sum"), 12.0));
        assert!(close(px("calc(8px - 4px)").expect("difference"), 4.0));
        assert!(close(px("calc(1em + 2px)").expect("mixed units"), 22.0));
        assert!(close(px("calc(8px + 4px + 2px)").expect("three"), 14.0));
    }

    #[test]
    fn calc_multiplies_and_divides_by_numbers() {
        assert!(close(px("calc(8px * 2)").expect("product"), 16.0));
        assert!(close(px("calc(2 * 8px)").expect("either order"), 16.0));
        assert!(close(px("calc(8px / 2)").expect("quotient"), 4.0));
        assert_eq!(
            px("calc(8px / 2px)"),
            None,
            "a length divided by a length is not a length",
        );
    }

    #[test]
    fn calc_mixes_percentages_with_lengths() {
        assert!(close(px("calc(50% - 10px)").expect("mixed"), 190.0));
        assert!(close(px("calc(100% / 3)").expect("thirds"), 400.0 / 3.0));
        assert!(
            parse_length_percentage("calc(50% - 10px)")
                .expect("mixed")
                .is_percentage(),
        );
    }

    #[test]
    fn calc_respects_the_order_operations_are_done_in() {
        assert!(close(px("calc(2px + 3px * 2)").expect("precedence"), 8.0));
        assert!(close(
            px("calc((2px + 3px) * 2)").expect("parentheses"),
            10.0
        ));
        assert!(close(
            px("calc(calc(2px + 3px) * 2)").expect("nested"),
            10.0
        ));
    }

    #[test]
    fn calc_refuses_arithmetic_that_does_not_mean_anything() {
        for text in [
            "calc(1px + 2)",
            "calc(2px * 3px)",
            "calc(8px / 2px)",
            "calc()",
            "calc(1px +)",
            "calc(1px + )",
        ] {
            assert_eq!(px(text), None, "{text} should be refused");
        }
    }

    #[test]
    fn calc_requires_the_spaces_the_specification_requires() {
        assert_eq!(
            px("calc(1px+2px)"),
            None,
            "without spaces this is two values, and CSS says so",
        );
        assert_eq!(px("calc(1px -2px)"), None);
        assert!(close(px("calc(1px - 2px)").expect("spaced"), -1.0));
        assert!(close(
            px("calc(8px*2)").expect("multiplication needs no spaces"),
            16.0
        ),);
    }

    #[test]
    fn a_number_is_read_as_a_number_and_a_length_is_not() {
        assert!(close(parse_number("1.5").expect("number"), 1.5));
        assert!(close(parse_number("2").expect("number"), 2.0));
        assert!(close(parse_number("calc(3 / 2)").expect("calc"), 1.5));
        assert_eq!(parse_number("16px"), None);
        assert_eq!(parse_number("calc(1px * 2)"), None);
        assert_eq!(parse_number("auto"), None);
    }

    #[test]
    fn a_length_that_is_a_percentage_is_not_a_length() {
        assert!(parse_length("16px").is_some());
        assert_eq!(parse_length("50%"), None);
        assert_eq!(parse_length("calc(50% - 1px)"), None);
    }

    fn colour(text: &str) -> Option<(u8, u8, u8, u8)> {
        Some(parse_color(text)?.resolve(crate::Rgba::BLACK).to_rgba8())
    }

    #[test]
    fn hex_is_read_in_every_length_css_allows() {
        assert_eq!(colour("#000"), Some((0, 0, 0, 255)));
        assert_eq!(colour("#fff"), Some((255, 255, 255, 255)));
        assert_eq!(colour("#101014"), Some((16, 16, 20, 255)));
        assert_eq!(colour("#10101480"), Some((16, 16, 20, 128)));
        assert_eq!(colour("#0000"), Some((0, 0, 0, 0)));
        assert_eq!(
            colour("#FFF"),
            Some((255, 255, 255, 255)),
            "however written"
        );
    }

    #[test]
    fn the_named_colours_are_the_ones_css_names() {
        assert_eq!(colour("red"), Some((255, 0, 0, 255)));
        assert_eq!(colour("rebeccapurple"), Some((102, 51, 153, 255)));
        assert_eq!(colour("WHITE"), Some((255, 255, 255, 255)));
        assert_eq!(colour("notacolour"), None);
    }

    #[test]
    fn transparent_and_current_colour_are_both_real_values() {
        assert_eq!(colour("transparent"), Some((0, 0, 0, 0)));
        assert_eq!(
            parse_color("currentColor"),
            Some(crate::Color::CurrentColor),
            "carried as itself: there is no element here to ask",
        );
        assert_eq!(
            parse_color("currentcolor").map(|c| c.resolve(crate::Rgba::WHITE).to_rgba8()),
            Some((255, 255, 255, 255)),
        );
    }

    #[test]
    fn rgb_is_read_in_both_the_modern_and_the_legacy_form() {
        assert_eq!(colour("rgb(255 0 0)"), Some((255, 0, 0, 255)));
        assert_eq!(colour("rgb(255, 0, 0)"), Some((255, 0, 0, 255)));
        assert_eq!(colour("rgba(255, 0, 0, 0.5)"), Some((255, 0, 0, 128)));
        assert_eq!(colour("rgb(255 0 0 / 0.5)"), Some((255, 0, 0, 128)));
        assert_eq!(colour("rgb(100% 0% 0%)"), Some((255, 0, 0, 255)));
        assert_eq!(colour("rgb(255 0 0 / 50%)"), Some((255, 0, 0, 128)));
    }

    #[test]
    fn hsl_is_read_the_same_two_ways() {
        assert_eq!(colour("hsl(0 100% 50%)"), Some((255, 0, 0, 255)));
        assert_eq!(colour("hsl(120, 100%, 50%)"), Some((0, 255, 0, 255)));
        assert_eq!(colour("hsla(240, 100%, 50%, 0.5)"), Some((0, 0, 255, 128)));
        assert_eq!(colour("hsl(240 100% 50% / 50%)"), Some((0, 0, 255, 128)));
    }

    #[test]
    fn a_channel_outside_its_range_is_brought_back_into_it() {
        assert_eq!(colour("rgb(300 -20 0)"), Some((255, 0, 0, 255)));
        assert_eq!(colour("rgb(0 0 0 / 2)"), Some((0, 0, 0, 255)));
    }

    #[test]
    fn a_colour_space_this_engine_does_not_have_is_refused_rather_than_guessed_at() {
        for text in [
            "oklch(70% 0.1 200)",
            "lab(50% 20 -30)",
            "color(display-p3 1 0 0)",
            "color-mix(in oklab, red 50%, blue)",
            "hwb(0 0% 0%)",
        ] {
            assert_eq!(parse_color(text), None, "{text} should be refused");
        }
    }

    #[test]
    fn something_that_is_not_a_colour_at_all_is_not_a_colour() {
        for text in [
            "",
            "  ",
            "16px",
            "rgb(1 2)",
            "rgb(1 2 3 4)",
            "red blue",
            "#12345",
        ] {
            assert_eq!(parse_color(text), None, "{text:?} should be refused");
        }
    }

    #[test]
    fn asking_whether_a_value_is_a_keyword_ignores_case_and_padding() {
        assert!(is_keyword("auto", "auto"));
        assert!(is_keyword("  AUTO  ", "auto"));
        assert!(!is_keyword("auto auto", "auto"));
        assert!(!is_keyword("automatic", "auto"));
    }
}

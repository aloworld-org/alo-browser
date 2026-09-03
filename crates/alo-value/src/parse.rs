/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

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
use crate::gradient::{Angle, Gradient, Stop};
use crate::length::{Length, LengthPercentage};
use crate::shadow::Shadow;
use crate::transform::{Function, Matrix, Transform};
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

/// Read a whole value as a gradient.
///
/// [`None`] for anything this engine does not implement: `conic-gradient`, the
/// repeating forms, interpolation hints, and any colour space but sRGB. Each is
/// a different curve through colour, and drawing one as another is a wrong
/// pixel that looks nearly right.
pub fn parse_gradient(text: &str) -> Option<Gradient> {
    entirely(text, |input| {
        let radial = input
            .try_parse(|input| {
                let name = input.expect_function()?.clone();
                if name.eq_ignore_ascii_case("linear-gradient") {
                    Ok(false)
                } else if name.eq_ignore_ascii_case("radial-gradient") {
                    Ok(true)
                } else {
                    Err(input.new_basic_unexpected_token_error(Token::Function(name)))
                }
            })
            .ok()?;
        input
            .parse_nested_block(
                |arguments| -> Result<Option<Gradient>, cssparser::ParseError<'_, ()>> {
                    Ok(gradient_arguments(arguments, radial))
                },
            )
            .ok()?
    })
}

/// What is inside the brackets: a direction, then the stops.
fn gradient_arguments(input: &mut CssParser<'_, '_>, radial: bool) -> Option<Gradient> {
    let mut angle = Angle::DOWN;
    if !radial {
        // A leading angle or `to <side>`; without one the gradient runs down.
        if let Ok(degrees) =
            input.try_parse(|input| -> Result<f32, cssparser::ParseError<'_, ()>> {
                let start = input.position();
                let degrees = match input.next()? {
                    Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("deg") => {
                        *value
                    }
                    Token::Ident(_) => {
                        // `to bottom right` — read the words up to the comma.
                        while input.try_parse(CssParser::expect_comma).is_err()
                            && !input.is_exhausted()
                        {
                            if input.next().is_err() {
                                break;
                            }
                        }
                        let phrase = input.slice_from(start);
                        let phrase = phrase.trim_end_matches(',').trim();
                        return match Angle::from_sides(phrase) {
                            Some(angle) => Ok(angle.0),
                            None => Err(input.new_custom_error(())),
                        };
                    }
                    _ => return Err(input.new_custom_error(())),
                };
                input.expect_comma()?;
                Ok(degrees)
            })
        {
            angle = Angle(degrees);
        }
    }

    let mut stops = Vec::new();
    loop {
        stops.push(one_stop(input)?);
        if input.try_parse(CssParser::expect_comma).is_err() {
            break;
        }
    }
    input.expect_exhausted().ok()?;
    if stops.len() < 2 {
        // One stop is a flat colour written the long way round, and CSS calls
        // it invalid rather than guessing which end it is.
        return None;
    }
    Some(if radial {
        Gradient::Radial { stops }
    } else {
        Gradient::Linear { angle, stops }
    })
}

/// One colour, and where it sits if it says.
fn one_stop(input: &mut CssParser<'_, '_>) -> Option<Stop> {
    let color = input.try_parse(|input| one_color(input).ok_or(())).ok()?;
    let position = input
        .try_parse(CssParser::expect_percentage)
        .ok()
        .map(|fraction| fraction.clamp(0.0, 1.0));
    Some(Stop { color, position })
}

/// Read a whole `box-shadow`: a comma-separated list, front to back.
///
/// `none` is an empty list rather than a refusal — the author said there are
/// no shadows, which is a different answer from "this could not be read".
pub fn parse_box_shadows(text: &str) -> Option<Vec<Shadow>> {
    parse_shadows(text, true)
}

/// Read a whole `text-shadow`.
///
/// The same grammar without `inset` and without a spread: there is no sensible
/// way to grow a letter before blurring it, and CSS does not offer one.
pub fn parse_text_shadows(text: &str) -> Option<Vec<Shadow>> {
    parse_shadows(text, false)
}

fn parse_shadows(text: &str, boxes: bool) -> Option<Vec<Shadow>> {
    if text.trim().is_empty() {
        return None;
    }
    if is_keyword(text, "none") {
        return Some(Vec::new());
    }
    entirely(text, |input| {
        let mut shadows = Vec::new();
        loop {
            shadows.push(one_shadow(input, boxes)?);
            if input.try_parse(CssParser::expect_comma).is_err() {
                break;
            }
        }
        input.expect_exhausted().ok()?;
        Some(shadows)
    })
}

/// One shadow: two lengths, then a blur, then a spread, in that order, with a
/// colour and — for a box shadow — the word `inset` anywhere among them.
fn one_shadow(input: &mut CssParser<'_, '_>, boxes: bool) -> Option<Shadow> {
    let mut lengths: Vec<Length> = Vec::new();
    let mut color: Option<Color> = None;
    let mut inset = false;

    loop {
        if boxes
            && !inset
            && input
                .try_parse(|input| input.expect_ident_matching("inset"))
                .is_ok()
        {
            inset = true;
            continue;
        }
        if lengths.len() < if boxes { 4 } else { 3 }
            && let Ok(length) = input.try_parse(|input| one_length(input).ok_or(()))
        {
            lengths.push(length);
            continue;
        }
        if color.is_none()
            && let Ok(read) = input.try_parse(|input| one_color(input).ok_or(()))
        {
            color = Some(read);
            continue;
        }
        break;
    }

    // Two offsets are the whole of what a shadow must say; everything else has
    // a default, and nothing else can stand in for them.
    let (Some(x), Some(y)) = (lengths.first().copied(), lengths.get(1).copied()) else {
        return None;
    };
    Some(Shadow {
        offset: (x, y),
        blur: lengths.get(2).copied().unwrap_or(Length::ZERO),
        spread: lengths.get(3).copied().unwrap_or(Length::ZERO),
        color,
        inset,
    })
}

/// One length from the stream: a dimension, or a bare zero.
fn one_length(input: &mut CssParser<'_, '_>) -> Option<Length> {
    match input.next().ok()? {
        Token::Dimension { value, unit, .. } => Some(Length {
            value: *value,
            unit: Unit::parse(unit)?,
        }),
        // `0` needs no unit, and is the only number a length may be written as.
        Token::Number { value, .. } if *value == 0.0 => Some(Length::ZERO),
        _ => None,
    }
}

/// Read a whole `transform`.
///
/// `none` is a transform that moves nothing, rather than a refusal. A value
/// with a function this engine does not implement — anything with a third
/// dimension in it — is refused whole, because applying half of a transform
/// puts a box somewhere the author never asked for.
pub fn parse_transform(text: &str) -> Option<Transform> {
    if is_keyword(text, "none") {
        return Some(Transform::default());
    }
    entirely(text, |input| {
        let mut functions = Vec::new();
        while !input.is_exhausted() {
            functions.push(one_transform_function(input)?);
        }
        if functions.is_empty() {
            return None;
        }
        Some(Transform { functions })
    })
}

/// One `translate(…)`, `scale(…)`, `rotate(…)`, `skew(…)` or `matrix(…)`.
fn one_transform_function(input: &mut CssParser<'_, '_>) -> Option<Function> {
    let name = input
        .try_parse(|input| input.expect_function().cloned())
        .ok()?
        .to_ascii_lowercase();
    input
        .parse_nested_block(
            |arguments| -> Result<Option<Function>, cssparser::ParseError<'_, ()>> {
                Ok(transform_arguments(&name, arguments))
            },
        )
        .ok()?
}

fn transform_arguments(name: &str, input: &mut CssParser<'_, '_>) -> Option<Function> {
    let function = match name {
        "translate" => {
            let across = one_length_percentage(input)?;
            let down = second(input, one_length_percentage)
                .unwrap_or(LengthPercentage::Length(Length::ZERO));
            Function::Translate(across, down)
        }
        "translatex" => Function::Translate(
            one_length_percentage(input)?,
            LengthPercentage::Length(Length::ZERO),
        ),
        "translatey" => Function::Translate(
            LengthPercentage::Length(Length::ZERO),
            one_length_percentage(input)?,
        ),
        "scale" => {
            let across = one_factor(input)?;
            // `scale(2)` is two in both directions, which is the one place CSS
            // repeats an argument rather than defaulting it to nothing.
            let down = second(input, one_factor).unwrap_or(across);
            Function::Scale(across, down)
        }
        "scalex" => Function::Scale(one_factor(input)?, 1.0),
        "scaley" => Function::Scale(1.0, one_factor(input)?),
        "rotate" => Function::Rotate(one_angle(input)?),
        "skew" => {
            let across = one_angle(input)?;
            let down = second(input, one_angle).unwrap_or(0.0);
            Function::Skew(across, down)
        }
        "skewx" => Function::Skew(one_angle(input)?, 0.0),
        "skewy" => Function::Skew(0.0, one_angle(input)?),
        "matrix" => {
            let mut numbers = [0.0; 6];
            for (index, slot) in numbers.iter_mut().enumerate() {
                if index > 0 {
                    input.expect_comma().ok()?;
                }
                *slot = one_number(input)?;
            }
            Function::Matrix(Matrix {
                a: numbers[0],
                b: numbers[1],
                c: numbers[2],
                d: numbers[3],
                e: numbers[4],
                f: numbers[5],
            })
        }
        _ => return None,
    };
    input.expect_exhausted().ok()?;
    Some(function)
}

/// A second argument, after the comma that introduces it.
fn second<T>(
    input: &mut CssParser<'_, '_>,
    read: impl Fn(&mut CssParser<'_, '_>) -> Option<T>,
) -> Option<T> {
    input.expect_comma().ok()?;
    read(input)
}

/// A bare number: a scale factor, or one of `matrix()`'s six.
fn one_number(input: &mut CssParser<'_, '_>) -> Option<f32> {
    match input.next().ok()? {
        Token::Number { value, .. } => Some(*value),
        _ => None,
    }
}

/// A scale factor: a number, or a percentage of it.
fn one_factor(input: &mut CssParser<'_, '_>) -> Option<f32> {
    match input.next().ok()? {
        Token::Number { value, .. } => Some(*value),
        Token::Percentage { unit_value, .. } => Some(*unit_value),
        _ => None,
    }
}

/// An angle, in degrees. Every unit CSS has for one, because a turn and a
/// radian are the same value written differently.
fn one_angle(input: &mut CssParser<'_, '_>) -> Option<f32> {
    match input.next().ok()? {
        Token::Dimension { value, unit, .. } => match unit.to_ascii_lowercase().as_str() {
            "deg" => Some(*value),
            "grad" => Some(value * 0.9),
            "rad" => Some(value.to_degrees()),
            "turn" => Some(value * 360.0),
            _ => None,
        },
        // A bare zero is an angle, and only a zero is.
        Token::Number { value, .. } if *value == 0.0 => Some(0.0),
        _ => None,
    }
}

/// Read a whole `transform-origin`, as a fraction of the box in each
/// direction and a length beside it.
///
/// The initial value is `50% 50%` — the middle — which is why `rotate` turns a
/// box about itself rather than about the corner of the page.
pub fn parse_transform_origin(text: &str) -> Option<(LengthPercentage, LengthPercentage)> {
    entirely(text, |input| {
        let first = origin_part(input)?;
        let second = if input.is_exhausted() {
            Part {
                value: LengthPercentage::Percentage(50.0),
                axis: None,
            }
        } else {
            origin_part(input)?
        };
        // `top left` says the same thing as `left top`, and CSS allows it when
        // both halves are words — a word says which axis it belongs to, so
        // there is nothing to be ambiguous about.
        let (across, down) = match (first.axis, second.axis) {
            (Some(Axis::Down), Some(Axis::Across)) => (second, first),
            (Some(Axis::Down), None) | (None, Some(Axis::Across)) => return None,
            (Some(Axis::Across), Some(Axis::Across)) | (Some(Axis::Down), Some(Axis::Down)) => {
                return None;
            }
            _ => (first, second),
        };
        Some((across.value, down.value))
    })
}

/// Which way a `transform-origin` keyword points, when it points at all.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Axis {
    Across,
    Down,
}

/// One half of a `transform-origin`, and which axis it insisted on.
struct Part {
    value: LengthPercentage,
    axis: Option<Axis>,
}

/// One half of a `transform-origin`: a keyword, or a length or percentage.
fn origin_part(input: &mut CssParser<'_, '_>) -> Option<Part> {
    if let Ok(keyword) = input.try_parse(|input| input.expect_ident().cloned()) {
        let (fraction, axis) = match keyword.to_ascii_lowercase().as_str() {
            "center" => (50.0, None),
            "left" => (0.0, Some(Axis::Across)),
            "right" => (100.0, Some(Axis::Across)),
            "top" => (0.0, Some(Axis::Down)),
            "bottom" => (100.0, Some(Axis::Down)),
            _ => return None,
        };
        return Some(Part {
            value: LengthPercentage::Percentage(fraction),
            axis,
        });
    }
    Some(Part {
        value: one_length_percentage(input)?,
        axis: None,
    })
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
    let node = math_function(input, 0)?;
    // Type-check once, here, so that evaluating later cannot fail.
    node.kind()?;
    Some(node)
}

/// `calc()`, `min()`, `max()` or `clamp()` — the four that are one family.
///
/// They are the same thing spelled four ways: an expression that has to be
/// worked out before it means anything. Parsing them together is what stops
/// `clamp(1rem, 4vw, min(3rem, 5vw))` from being three special cases that do
/// not nest.
fn math_function(input: &mut CssParser<'_, '_>, depth: u8) -> Option<CalcNode> {
    if depth > 16 {
        // A depth nobody writes, and a bound so that a pathological value is
        // refused rather than running out of stack.
        return None;
    }
    let name = input
        .try_parse(|input| input.expect_function().cloned())
        .ok()?
        .to_ascii_lowercase();
    input
        .parse_nested_block(
            |arguments| -> Result<Option<CalcNode>, cssparser::ParseError<'_, ()>> {
                Ok(math_arguments(&name, arguments, depth))
            },
        )
        .ok()?
}

/// What is inside a math function's brackets.
fn math_arguments(name: &str, input: &mut CssParser<'_, '_>, depth: u8) -> Option<CalcNode> {
    let node = match name {
        "calc" => sum(input, depth)?,
        "min" | "max" => {
            let terms = comma_separated(input, depth)?;
            if terms.is_empty() {
                return None;
            }
            if name == "min" {
                CalcNode::Min(terms)
            } else {
                CalcNode::Max(terms)
            }
        }
        "clamp" => {
            let terms = comma_separated(input, depth)?;
            let [low, middle, high] = <[CalcNode; 3]>::try_from(terms).ok()?;
            CalcNode::Clamp(Box::new(low), Box::new(middle), Box::new(high))
        }
        _ => return None,
    };
    input.expect_exhausted().ok()?;
    Some(node)
}

/// One or more expressions, separated by commas.
fn comma_separated(input: &mut CssParser<'_, '_>, depth: u8) -> Option<Vec<CalcNode>> {
    let mut out = Vec::new();
    loop {
        out.push(sum(input, depth + 1)?);
        if input.try_parse(CssParser::expect_comma).is_err() {
            return Some(out);
        }
    }
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
    // A math function nested inside another one — which is how
    // `clamp(2.4rem, 4vw, 3.5rem)` is written in a real sheet, and how
    // `min()` ends up inside a `calc()`.
    if let Ok(nested) = input.try_parse(|input| math_function(input, depth + 1).ok_or(())) {
        return Some(nested);
    }
    // `(` opens a sub-expression.
    input
        .try_parse(|input| {
            let token = input.next()?.clone();
            match token {
                Token::ParenthesisBlock => Ok(()),
                other => Err(input.new_basic_unexpected_token_error(other)),
            }
        })
        .ok()?;
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
        for text in ["3fr", "1cqw", "2s", "45deg", "10dvh"] {
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

    fn gradient(text: &str) -> Option<crate::Gradient> {
        parse_gradient(text)
    }

    fn stop_colours(gradient: &crate::Gradient) -> Vec<(u8, u8, u8, u8)> {
        gradient
            .stops()
            .iter()
            .map(|stop| stop.color.resolve(crate::Rgba::BLACK).to_rgba8())
            .collect()
    }

    #[test]
    fn the_simplest_gradient_is_two_colours() {
        let found = gradient("linear-gradient(red, blue)").expect("two stops");
        assert_eq!(
            stop_colours(&found),
            vec![(255, 0, 0, 255), (0, 0, 255, 255)],
        );
        match found {
            crate::Gradient::Linear { angle, .. } => {
                assert!(
                    (angle.0 - 180.0).abs() < f32::EPSILON,
                    "downwards by default"
                );
            }
            other @ crate::Gradient::Radial { .. } => {
                panic!("expected a linear gradient, got {other}")
            }
        }
    }

    #[test]
    fn a_direction_may_be_an_angle_or_a_side() {
        for (text, degrees) in [
            ("linear-gradient(90deg, red, blue)", 90.0),
            ("linear-gradient(to right, red, blue)", 90.0),
            ("linear-gradient(to top left, red, blue)", 315.0),
            ("linear-gradient(0deg, red, blue)", 0.0),
        ] {
            match gradient(text).unwrap_or_else(|| panic!("{text} should parse")) {
                crate::Gradient::Linear { angle, .. } => {
                    assert!((angle.0 - degrees).abs() < 0.001, "{text}");
                }
                other @ crate::Gradient::Radial { .. } => {
                    panic!("{text}: expected linear, got {other}")
                }
            }
        }
    }

    #[test]
    fn a_stop_may_say_where_it_sits() {
        let found = gradient("linear-gradient(red 0%, blue 25%, red 100%)").expect("stops");
        let positions: Vec<Option<f32>> = found.stops().iter().map(|stop| stop.position).collect();
        assert_eq!(positions, vec![Some(0.0), Some(0.25), Some(1.0)]);
    }

    #[test]
    fn a_radial_gradient_is_read_as_one() {
        let found = gradient("radial-gradient(white, black)").expect("two stops");
        assert!(matches!(found, crate::Gradient::Radial { .. }));
    }

    #[test]
    fn a_gradient_of_one_colour_is_not_a_gradient() {
        assert_eq!(gradient("linear-gradient(red)"), None);
        assert_eq!(gradient("linear-gradient(to right, red)"), None);
    }

    #[test]
    fn what_this_engine_does_not_implement_is_refused() {
        for text in [
            "conic-gradient(red, blue)",
            "repeating-linear-gradient(red, blue)",
            "linear-gradient(in oklab, red, blue)",
            "linear-gradient(red, 50%, blue)",
            "linear-gradient()",
            "red",
            "",
        ] {
            assert_eq!(gradient(text), None, "{text} should be refused");
        }
    }

    fn drawn(text: &str) -> Vec<String> {
        parse_box_shadows(text)
            .unwrap_or_else(|| panic!("{text} should parse"))
            .iter()
            .map(|shadow| {
                shadow
                    .drawn(crate::FontMetrics::estimated(16.0, 16.0), Rgba::BLACK)
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn a_shadow_is_two_offsets_and_whatever_else_it_says() {
        assert_eq!(
            drawn("0 2px 4px rgba(0, 0, 0, 0.5)"),
            vec!["0 2 blur 4 spread 0 rgb(0 0 0 / 0.5)"],
        );
        assert_eq!(
            drawn("1px 2px 3px 4px black"),
            vec!["1 2 blur 3 spread 4 rgb(0 0 0)"],
        );
    }

    #[test]
    fn the_offsets_are_the_only_part_a_shadow_must_say() {
        assert_eq!(drawn("2px 2px"), vec!["2 2 blur 0 spread 0 rgb(0 0 0)"]);
        assert_eq!(parse_box_shadows("2px"), None);
        assert_eq!(parse_box_shadows("red"), None);
    }

    #[test]
    fn a_colour_may_come_first_or_last_and_inset_anywhere() {
        assert_eq!(
            drawn("white 0 1px 2px"),
            vec!["0 1 blur 2 spread 0 rgb(255 255 255)"],
        );
        assert_eq!(
            drawn("inset 0 1px 2px black"),
            vec!["inset 0 1 blur 2 spread 0 rgb(0 0 0)"],
        );
        assert_eq!(
            drawn("0 1px 2px black inset"),
            vec!["inset 0 1 blur 2 spread 0 rgb(0 0 0)"],
        );
    }

    #[test]
    fn shadows_are_a_list_and_stay_in_the_order_they_were_written() {
        assert_eq!(
            drawn("0 1px 1px black, 0 4px 8px white"),
            vec![
                "0 1 blur 1 spread 0 rgb(0 0 0)",
                "0 4 blur 8 spread 0 rgb(255 255 255)",
            ],
        );
    }

    #[test]
    fn no_shadows_is_an_answer_and_nonsense_is_not() {
        assert_eq!(parse_box_shadows("none"), Some(Vec::new()));
        assert_eq!(parse_box_shadows("NONE"), Some(Vec::new()));
        assert_eq!(parse_box_shadows(""), None);
        assert_eq!(parse_box_shadows("0 1px 2px black, "), None);
        assert_eq!(parse_box_shadows("0 1px 2px black extra"), None);
    }

    #[test]
    fn a_text_shadow_has_no_spread_and_is_never_inset() {
        let shadows = parse_text_shadows("0 1px 2px black").expect("one shadow");
        assert_eq!(shadows.len(), 1);
        let one = shadows.first().expect("one shadow");
        assert_eq!(one.spread, Length::ZERO);
        assert!(!one.inset);

        assert_eq!(parse_text_shadows("inset 0 1px 2px black"), None);
        assert_eq!(parse_text_shadows("0 1px 2px 3px black"), None);
    }

    fn moved(text: &str, x: f32, y: f32) -> (f32, f32) {
        parse_transform(text)
            .unwrap_or_else(|| panic!("{text} should parse"))
            .matrix(
                crate::FontMetrics::estimated(16.0, 16.0),
                (100.0, 100.0),
                (0.0, 0.0),
            )
            .apply(x, y)
    }

    fn near(left: (f32, f32), right: (f32, f32)) -> bool {
        (left.0 - right.0).abs() < 0.001 && (left.1 - right.1).abs() < 0.001
    }

    #[test]
    fn every_transform_this_engine_draws_is_read() {
        assert!(near(moved("translate(10px, 20px)", 0.0, 0.0), (10.0, 20.0)));
        assert!(near(moved("translate(10px)", 0.0, 0.0), (10.0, 0.0)));
        assert!(near(moved("translateX(10px)", 0.0, 0.0), (10.0, 0.0)));
        assert!(near(moved("translateY(10px)", 0.0, 0.0), (0.0, 10.0)));
        assert!(near(moved("scale(2)", 3.0, 4.0), (6.0, 8.0)));
        assert!(near(moved("scale(2, 3)", 1.0, 1.0), (2.0, 3.0)));
        assert!(near(moved("scaleX(2)", 1.0, 1.0), (2.0, 1.0)));
        assert!(near(moved("scaleY(2)", 1.0, 1.0), (1.0, 2.0)));
        assert!(near(moved("rotate(90deg)", 1.0, 0.0), (0.0, 1.0)));
        assert!(near(moved("skewX(45deg)", 0.0, 1.0), (1.0, 1.0)));
        assert!(near(moved("skewY(45deg)", 1.0, 0.0), (1.0, 1.0)));
        assert!(near(
            moved("matrix(1, 0, 0, 1, 5, 6)", 0.0, 0.0),
            (5.0, 6.0),
        ));
    }

    #[test]
    fn an_angle_may_be_written_in_any_unit_css_has_for_one() {
        for text in [
            "rotate(90deg)",
            "rotate(100grad)",
            "rotate(0.25turn)",
            "rotate(1.5707963rad)",
        ] {
            assert!(near(moved(text, 1.0, 0.0), (0.0, 1.0)), "{text}");
        }
        assert!(near(moved("rotate(0)", 1.0, 0.0), (1.0, 0.0)));
    }

    #[test]
    fn several_functions_apply_in_each_others_coordinates() {
        // Turned a quarter, then ten along an axis that is now downwards.
        assert!(near(
            moved("rotate(90deg) translateX(10px)", 0.0, 0.0),
            (0.0, 10.0),
        ));
    }

    #[test]
    fn no_transform_is_an_answer_and_half_a_transform_is_not() {
        assert_eq!(parse_transform("none"), Some(Transform::default()));
        assert_eq!(parse_transform("NONE"), Some(Transform::default()));
        // A function with a third dimension in it refuses the whole value
        // rather than applying the part it understood.
        assert_eq!(parse_transform("translateX(10px) translateZ(5px)"), None);
        assert_eq!(parse_transform("rotate3d(1, 1, 1, 45deg)"), None);
        assert_eq!(parse_transform("perspective(500px)"), None);
        assert_eq!(parse_transform("rotate(45)"), None, "an angle needs a unit");
        assert_eq!(parse_transform("translate(10px, 20px, 30px)"), None);
        assert_eq!(parse_transform("matrix(1, 0, 0, 1, 5)"), None);
        assert_eq!(parse_transform(""), None);
        assert_eq!(parse_transform("10px"), None);
    }

    #[test]
    fn an_origin_may_be_said_in_words_or_in_numbers() {
        let middle = (
            LengthPercentage::Percentage(50.0),
            LengthPercentage::Percentage(50.0),
        );
        assert_eq!(parse_transform_origin("center"), Some(middle.clone()));
        assert_eq!(
            parse_transform_origin("center center"),
            Some(middle.clone())
        );
        assert_eq!(parse_transform_origin("50% 50%"), Some(middle));
        assert_eq!(
            parse_transform_origin("left top"),
            Some((
                LengthPercentage::Percentage(0.0),
                LengthPercentage::Percentage(0.0),
            )),
        );
        assert_eq!(
            parse_transform_origin("right bottom"),
            Some((
                LengthPercentage::Percentage(100.0),
                LengthPercentage::Percentage(100.0),
            )),
        );
        assert_eq!(
            parse_transform_origin("10px"),
            Some((
                LengthPercentage::Length(Length::px(10.0)),
                LengthPercentage::Percentage(50.0),
            )),
            "one value leaves the other in the middle",
        );
    }

    #[test]
    fn an_origin_that_names_the_wrong_axis_is_refused() {
        assert_eq!(
            parse_transform_origin("top left"),
            parse_transform_origin("left top"),
            "two words say which axis each belongs to, so the order is free",
        );
        assert_eq!(parse_transform_origin("left left"), None);
        assert_eq!(parse_transform_origin("top top"), None);
        assert_eq!(parse_transform_origin("top 10px"), None);
        assert_eq!(parse_transform_origin("sideways"), None);
        assert_eq!(parse_transform_origin(""), None);
    }

    /// A length resolved in a thousand-pixel-wide window.
    fn in_a_window(text: &str) -> Option<f32> {
        let metrics = crate::FontMetrics::estimated(16.0, 16.0)
            .in_viewport(crate::Viewport::new(1000.0, 800.0));
        Some(parse_length_percentage(text)?.to_px(metrics, 0.0))
    }

    #[test]
    fn a_viewport_unit_is_a_hundredth_of_the_window() {
        assert_eq!(in_a_window("4vw"), Some(40.0));
        assert_eq!(in_a_window("50vh"), Some(400.0));
        assert_eq!(in_a_window("10vmin"), Some(80.0), "the shorter side");
        assert_eq!(in_a_window("10vmax"), Some(100.0), "the longer side");
    }

    #[test]
    fn a_viewport_unit_with_no_window_is_nothing_rather_than_a_guess() {
        let metrics = crate::FontMetrics::estimated(16.0, 16.0);
        let length = parse_length_percentage("4vw").expect("a length");
        assert!(length.to_px(metrics, 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_headline_on_alos_sign_in_screen_resolves() {
        // `clamp(2.4rem, 4vw, 3.5rem)` at the thousand pixels the corpus case
        // renders at. This is the value that stood substituted in that case
        // for as long as the engine had neither piece.
        assert_eq!(in_a_window("clamp(2.4rem, 4vw, 3.5rem)"), Some(40.0));
    }

    #[test]
    fn clamp_holds_a_value_between_its_bounds() {
        assert_eq!(in_a_window("clamp(10px, 4vw, 30px)"), Some(30.0), "capped");
        assert_eq!(in_a_window("clamp(60px, 4vw, 90px)"), Some(60.0), "floored");
        assert_eq!(in_a_window("clamp(10px, 4vw, 90px)"), Some(40.0), "between");
    }

    #[test]
    fn when_the_bounds_cross_the_lower_one_wins() {
        // CSS defines `clamp(a, b, c)` as `max(a, min(b, c))`, so a minimum
        // above the maximum is the answer. Not Rust's `clamp`, which refuses.
        assert_eq!(in_a_window("clamp(80px, 10px, 20px)"), Some(80.0));
    }

    #[test]
    fn min_and_max_take_as_many_arguments_as_they_are_given() {
        assert_eq!(in_a_window("min(10px, 4vw, 2rem)"), Some(10.0));
        assert_eq!(in_a_window("max(10px, 4vw, 2rem)"), Some(40.0));
        assert_eq!(in_a_window("min(5rem)"), Some(80.0), "one is a minimum too");
    }

    #[test]
    fn the_family_nests_in_itself_and_in_calc() {
        assert_eq!(in_a_window("calc(min(10px, 4vw) * 2)"), Some(20.0));
        assert_eq!(in_a_window("clamp(1rem, min(4vw, 30px), 5rem)"), Some(30.0));
        assert_eq!(in_a_window("max(calc(1rem + 2px), 4px)"), Some(18.0));
    }

    #[test]
    fn a_math_function_that_does_not_type_check_is_refused() {
        // The same rule `calc()` already had: there is no answer to the
        // smaller of a length and a number.
        assert_eq!(parse_length_percentage("min(10px, 4)"), None);
        assert_eq!(parse_length_percentage("clamp(1px, 2, 3px)"), None);
        assert_eq!(parse_length_percentage("clamp(1px, 2px)"), None);
        assert_eq!(parse_length_percentage("clamp(1px, 2px, 3px, 4px)"), None);
        assert_eq!(parse_length_percentage("min()"), None);
    }

    #[test]
    fn asking_whether_a_value_is_a_keyword_ignores_case_and_padding() {
        assert!(is_keyword("auto", "auto"));
        assert!(is_keyword("  AUTO  ", "auto"));
        assert!(!is_keyword("auto auto", "auto"));
        assert!(!is_keyword("automatic", "auto"));
    }
}

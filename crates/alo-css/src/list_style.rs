/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `list-style`, split into its three longhands as it is written (ADR 0044
//! § 2).
//!
//! The one shorthand split here whose parts are told apart by **kind**
//! rather than by counting ([`crate::shorthand`]), and it is split for the
//! reason [`DeclarationBlock::push`](crate::DeclarationBlock::push) gives:
//! left whole, the user agent's `list-style-type: disc` and an author's
//! `list-style: none` are different properties that never meet in the
//! cascade, and every link in alo Sites' navigation, which its sheet takes
//! the markers off that way, would be drawn with a bullet.
//!
//! # The grammar, CSS Lists 3 § 3.4
//!
//! `<'list-style-position'> || <'list-style-image'> || <'list-style-type'>`,
//! each at most once, in any order; one left out is reset to its initial
//! value — `outside`, `none` and `disc`. A **position** is `inside` or
//! `outside`. An **image** is a function that makes one, `url()` or a
//! gradient. A **type** is a string, `symbols()`, or any other name.
//!
//! `none` is the word two of them share. One `none` with a type beside it
//! is the image's, with an image beside it the type's, and alone it is
//! both, which is how `list-style: none` takes a marker off. Two are both.
//!
//! # What is left whole
//!
//! A value of no shape this grammar allows — a position twice, three
//! `none`s, a fourth part — and a value holding `var()`, which could be any
//! of the three until it is substituted. Each is left whole and split into
//! nothing, as [`crate::shorthand`] leaves a value it cannot split, and
//! nothing reads the shorthand itself. A CSS-wide keyword alone sets all
//! three to it.

/// The longhands `list-style` sets, as CSSOM asks for them.
pub(crate) static LONGHANDS: [&str; 3] =
    ["list-style-position", "list-style-image", "list-style-type"];

/// The three longhands with the value each takes, or nothing when the value
/// cannot be split.
pub(crate) fn split(value: &str) -> Vec<(String, String)> {
    let value = value.trim();
    let [position, image, kind] = LONGHANDS;
    if is_wide_keyword(value) {
        return LONGHANDS
            .iter()
            .map(|longhand| ((*longhand).to_owned(), value.to_owned()))
            .collect();
    }
    if value.to_ascii_lowercase().contains("var(") {
        return Vec::new();
    }
    let Some(parts) = parts(value) else {
        return Vec::new();
    };
    let mut found = Found::default();
    let mut nones = 0usize;
    for part in parts {
        let taken = if part.eq_ignore_ascii_case("none") {
            nones += 1;
            true
        } else if part.eq_ignore_ascii_case("inside") || part.eq_ignore_ascii_case("outside") {
            set(&mut found.position, part)
        } else if is_image(&part) {
            set(&mut found.image, part)
        } else if is_wide_keyword(&part) {
            false
        } else {
            set(&mut found.kind, part)
        };
        if !taken {
            return Vec::new();
        }
    }
    // Each `none` goes to whichever of the two that take one is still free.
    for _ in 0..nones {
        let free = if found.image.is_none() && found.kind.is_some() {
            &mut found.image
        } else if found.kind.is_none() && found.image.is_some() {
            &mut found.kind
        } else if found.image.is_none() {
            &mut found.image
        } else {
            return Vec::new();
        };
        *free = Some("none".to_owned());
    }
    // A lone `none` left the type free: it is both, so the type is `none`.
    if nones > 0 && found.kind.is_none() {
        found.kind = Some("none".to_owned());
    }
    vec![
        (
            position.to_owned(),
            found.position.unwrap_or_else(|| "outside".to_owned()),
        ),
        (
            image.to_owned(),
            found.image.unwrap_or_else(|| "none".to_owned()),
        ),
        (
            kind.to_owned(),
            found.kind.unwrap_or_else(|| "disc".to_owned()),
        ),
    ]
}

/// What has been found of each longhand so far.
#[derive(Default)]
struct Found {
    position: Option<String>,
    image: Option<String>,
    kind: Option<String>,
}

/// Put `part` in `slot`, or say it was already taken.
fn set(slot: &mut Option<String>, part: String) -> bool {
    if slot.is_some() {
        return false;
    }
    *slot = Some(part);
    true
}

/// Whether a part is an image: a function that makes one.
fn is_image(part: &str) -> bool {
    let Some((name, _)) = part.split_once('(') else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    name == "url"
        || name == "image-set"
        || name == "image"
        || name == "cross-fade"
        || name == "element"
        || name.ends_with("gradient")
}

/// Whether a value is one of the keywords every property takes.
fn is_wide_keyword(value: &str) -> bool {
    ["inherit", "initial", "unset", "revert", "revert-layer"]
        .iter()
        .any(|keyword| value.eq_ignore_ascii_case(keyword))
}

/// The parts of a value, split on the spaces outside functions and
/// strings, or [`None`] when a string or a function is never closed.
fn parts(value: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for letter in value.chars() {
        if let Some(open) = quote {
            current.push(letter);
            if escaped {
                escaped = false;
            } else if letter == '\\' {
                escaped = true;
            } else if letter == open {
                quote = None;
            }
            continue;
        }
        match letter {
            '"' | '\'' => {
                quote = Some(letter);
                current.push(letter);
            }
            '(' => {
                depth = depth.checked_add(1)?;
                current.push(letter);
            }
            ')' => {
                depth = depth.checked_sub(1)?;
                current.push(letter);
            }
            letter if letter.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    parts.push(core::mem::take(&mut current));
                }
            }
            letter => current.push(letter),
        }
    }
    if quote.is_some() || depth != 0 {
        return None;
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three longhands' values, position, image and type, or `None`.
    fn split_into(value: &str) -> Option<[String; 3]> {
        let split = split(value);
        let values: Vec<String> = split.into_iter().map(|(_, value)| value).collect();
        values.try_into().ok()
    }

    fn three(position: &str, image: &str, kind: &str) -> [String; 3] {
        [position.to_owned(), image.to_owned(), kind.to_owned()]
    }

    #[test]
    fn the_longhands_come_out_in_order_with_their_names() {
        let names: Vec<String> = split("square").into_iter().map(|(name, _)| name).collect();
        assert_eq!(
            names,
            ["list-style-position", "list-style-image", "list-style-type"]
        );
    }

    #[test]
    fn none_takes_the_marker_off() {
        assert_eq!(split_into("none"), Some(three("outside", "none", "none")));
        assert_eq!(split_into("NONE"), Some(three("outside", "none", "none")));
        assert_eq!(
            split_into("none none"),
            Some(three("outside", "none", "none"))
        );
        assert_eq!(
            split_into("inside none"),
            Some(three("inside", "none", "none"))
        );
    }

    #[test]
    fn none_beside_a_type_is_the_image_and_beside_an_image_the_type() {
        assert_eq!(
            split_into("square none"),
            Some(three("outside", "none", "square"))
        );
        assert_eq!(
            split_into("none url(dot.png)"),
            Some(three("outside", "url(dot.png)", "none"))
        );
    }

    #[test]
    fn each_part_is_told_apart_by_its_kind_in_any_order() {
        assert_eq!(
            split_into("square"),
            Some(three("outside", "none", "square"))
        );
        assert_eq!(
            split_into("inside lower-roman"),
            Some(three("inside", "none", "lower-roman"))
        );
        assert_eq!(
            split_into("url( 'a b.png' ) circle outside"),
            Some(three("outside", "url( 'a b.png' )", "circle"))
        );
        assert_eq!(
            split_into("linear-gradient(red, blue) inside"),
            Some(three("inside", "linear-gradient(red, blue)", "disc"))
        );
        assert_eq!(
            split_into("\"→ \" inside"),
            Some(three("inside", "none", "\"→ \""))
        );
        assert_eq!(
            split_into("symbols(cyclic '*')"),
            Some(three("outside", "none", "symbols(cyclic '*')"))
        );
        // Anything else is a counter style's name, which the marker reads.
        assert_eq!(
            split_into("my-style"),
            Some(three("outside", "none", "my-style"))
        );
    }

    #[test]
    fn a_wide_keyword_sets_all_three_and_is_nothing_beside_another_part() {
        assert_eq!(
            split_into("inherit"),
            Some(three("inherit", "inherit", "inherit"))
        );
        assert_eq!(split_into("disc initial"), None);
    }

    #[test]
    fn a_value_of_no_shape_or_holding_a_variable_is_left_whole() {
        for value in [
            "inside outside",
            "disc square",
            "none none none",
            "url(a) url(b)",
            "disc none url(a)",
            "var(--marker)",
            "disc VAR(--x)",
            "url(a",
            "\"open",
            "a)",
        ] {
            assert_eq!(split_into(value), None, "{value}");
        }
    }

    #[test]
    fn a_hostile_value_is_split_or_left_whole_without_a_panic() {
        let deep = format!("{}disc{}", "symbols(".repeat(10_000), ")".repeat(10_000));
        assert!(split_into(&deep).is_some());
        let long = "x".repeat(100_000);
        assert_eq!(
            split_into(&long).map(|[_, _, kind]| kind.len()),
            Some(100_000)
        );
        assert_eq!(split_into(&"none ".repeat(10_000)), None);
        assert_eq!(split_into(&")".repeat(1000)), None);
        assert_eq!(split_into("'\\'"), None);
    }
}

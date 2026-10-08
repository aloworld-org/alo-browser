/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Shorthands a declaration block splits into longhands as it is written.
//!
//! [`DeclarationBlock::push`](crate::DeclarationBlock::push) says why this has
//! to happen before the cascade. This file says *which* shorthands are split
//! and by what rule, and it holds only the ones that split by **counting** —
//! the position of a value says which longhand it is. Two families do:
//!
//! - **One value per side** ([`SIDED`]): `margin`, `padding` and the three
//!   border shorthands that are one value per side.
//! - **One value per axis** ([`PAIRED`]): `place-items`, `place-self` and
//!   `place-content`, block axis first.
//!
//! A shorthand whose parts are told apart by *kind* — `border`, `background`,
//! `font` — is not here: that is parsing, and it is `alo_value::shorthand`'s.

use crate::declaration::Declaration;

/// The shorthands that are one value per side, and the longhands each becomes.
///
/// Only the shorthands that are **one value per side**, because splitting them
/// is one rule — one value is every side, two are vertical then horizontal, and
/// so on — and because they are the ones a user-agent sheet sets and an author
/// overrides.
///
/// `border` itself, `background` and `font` are **not** here and have the same
/// shape of problem: expanding them means parsing values rather than splitting
/// on spaces, since `red solid 1px` and `1px solid red` are the same border.
/// They are read where they are used instead, longhand first.
///
/// `border-radius` is one value per *corner* and pairs the diagonals rather
/// than opposite sides, so it is not one of these however much it looks like
/// one. It is `alo_paint::corner`'s.
pub(crate) static SIDED: [(&str, [&str; 4]); 5] = [
    (
        "margin",
        ["margin-top", "margin-right", "margin-bottom", "margin-left"],
    ),
    (
        "padding",
        [
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ],
    ),
    // The three border shorthands arrived when the user-agent sheet first set
    // one — `border-color` on a disabled control, queue item 182. Until then
    // the sheet set none of them, so nothing collided; the comment that said
    // so is what named the day they should be added.
    (
        "border-width",
        [
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ],
    ),
    (
        "border-style",
        [
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
        ],
    ),
    (
        "border-color",
        [
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        ],
    ),
];

/// A shorthand's longhands, each with its value.
///
/// Empty for anything that is not a shorthand this file splits, or for a value
/// whose shape it cannot split — which is left whole, to be refused where it is
/// read.
pub(crate) fn expand(declaration: &Declaration) -> Vec<(String, String)> {
    let name = declaration.name.as_str();
    let value = declaration.value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    if let Some((_, longhands)) = SIDED.iter().find(|(shorthand, _)| *shorthand == name) {
        return sides(longhands, value);
    }
    if let Some((_, axes)) = PAIRED.iter().find(|(shorthand, _)| *shorthand == name) {
        return axes_of(axes, value);
    }
    Vec::new()
}

/// A one-value-per-side shorthand's longhands, with the value each side takes.
///
/// # `var()` is split too, and it took a wrong turn to learn why
///
/// The first version of this refused to expand a value containing `var()`, on
/// the reasoning that a custom property may hold several values and so which
/// side each part belongs to is not knowable until substitution. That is true
/// and it made things **worse**: an author's `padding: var(--a) var(--b)` was
/// then the only shorthand left unexpanded, so it lost to the user agent's
/// expanded `padding-left`, and every control on every alo screen lost its
/// padding. A picture showed it.
///
/// So a `var()` is one part like any other function, and splitting respects
/// parentheses. A custom property holding several values is still not handled —
/// but it is now a rare wrong answer rather than a common one.
fn sides(longhands: &[&str; 4], value: &str) -> Vec<(String, String)> {
    let parts = top_level_parts(value);
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    // One value is every side; two are vertical then horizontal; three add a
    // separate bottom; four are top, right, bottom, left. Anything else is not
    // a shorthand this engine can split, and is left whole to be refused where
    // it is read.
    let sides: [&str; 4] = match parts.as_slice() {
        [all] => [all, all, all, all],
        [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
        [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
        [top, right, bottom, left] => [top, right, bottom, left],
        _ => return Vec::new(),
    };
    longhands
        .iter()
        .zip(sides)
        .map(|(longhand, side)| ((*longhand).to_owned(), side.to_owned()))
        .collect()
}

/// The shorthands that are one value per axis, and the longhands each becomes:
/// the block axis (`align-*`) first, the inline axis (`justify-*`) second.
///
/// `display: grid; place-items: center` is how alo's offline screen centres
/// itself, and with the shorthand left whole the screen was laid out at the
/// top of its window.
pub(crate) static PAIRED: [(&str, [&str; 2]); 3] = [
    ("place-items", ["align-items", "justify-items"]),
    ("place-self", ["align-self", "justify-self"]),
    ("place-content", ["align-content", "justify-content"]),
];

/// A one-value-per-axis shorthand's two longhands, block axis first.
///
/// One value is both axes; two are block then inline. A value can be more
/// than one word — `safe center`, `last baseline`, `legacy left` — so it is
/// the *values* that are counted, not the words: `safe center start` is two.
///
/// Two exceptions, both CSS's. `place-content` with one value that is a
/// baseline does not copy it, because `justify-content` has no baseline: its
/// inline axis is `start`. And `legacy` belongs to the inline axis alone, so a
/// value that puts it first is not one of these and is left whole.
///
/// A `var()` is one value, as it is for [`SIDED`], with the same rare wrong
/// answer when the variable holds two.
fn axes_of(axes: &[&str; 2], value: &str) -> Vec<(String, String)> {
    let parts = top_level_parts(value);
    let Some((block, rest)) = first_value(&parts) else {
        return Vec::new();
    };
    if has_word(&block, "legacy") {
        return Vec::new();
    }
    let [align, justify] = axes;
    let inline = match rest {
        [] if *align == "align-content" && has_word(&block, "baseline") => "start".to_owned(),
        [] => block.clone(),
        [only] => only.clone(),
        [first, second] if belong_together(first, second) => format!("{first} {second}"),
        _ => return Vec::new(),
    };
    vec![
        ((*align).to_owned(), block),
        ((*justify).to_owned(), inline),
    ]
}

/// The first value of a block-axis alignment, and the parts after it.
///
/// `safe` and `unsafe` take the word after them, and `first` and `last` take
/// `baseline`; anything else is one word.
fn first_value(parts: &[String]) -> Option<(String, &[String])> {
    match parts {
        [first, second, rest @ ..] if is_one_of(first, &["safe", "unsafe"]) => {
            Some((format!("{first} {second}"), rest))
        }
        [first, second, rest @ ..]
            if is_one_of(first, &["first", "last"]) && is_one_of(second, &["baseline"]) =>
        {
            Some((format!("{first} {second}"), rest))
        }
        [first, rest @ ..] => Some((first.clone(), rest)),
        [] => None,
    }
}

/// Whether two words are one alignment value: an overflow position and what
/// it qualifies, a baseline and which one, or `legacy` and its direction in
/// either order.
fn belong_together(first: &str, second: &str) -> bool {
    const DIRECTIONS: &[&str] = &["left", "right", "center"];
    is_one_of(first, &["safe", "unsafe"])
        || (is_one_of(first, &["first", "last"]) && is_one_of(second, &["baseline"]))
        || (is_one_of(first, &["legacy"]) && is_one_of(second, DIRECTIONS))
        || (is_one_of(first, DIRECTIONS) && is_one_of(second, &["legacy"]))
}

/// Whether `word` is one of `options`, as CSS compares keywords.
fn is_one_of(word: &str, options: &[&str]) -> bool {
    options
        .iter()
        .any(|option| word.eq_ignore_ascii_case(option))
}

/// Whether a value of one or more words has `word` among them.
fn has_word(value: &str, word: &str) -> bool {
    value.split(' ').any(|part| part.eq_ignore_ascii_case(word))
}

/// Split on the spaces between values, not the ones inside them.
///
/// `1px calc(2px + 3px) var(--a, 4px 5px)` is three values, and a split on
/// whitespace makes it six — which would put `calc(2px` on one side and
/// `+ 3px)` on another.
fn top_level_parts(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for letter in value.chars() {
        match letter {
            '(' => {
                depth += 1;
                current.push(letter);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(letter);
            }
            letter if letter.is_ascii_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            letter => current.push(letter),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declaration::Importance;

    fn sides(value: &str) -> Vec<(String, String)> {
        expand(&Declaration::new("margin", value, Importance::Normal))
    }

    /// One value is every side; two are vertical then horizontal; three add a
    /// separate bottom; four are top, right, bottom, left.
    #[test]
    fn a_shorthand_becomes_the_four_sides_it_means() {
        assert_eq!(
            sides("1px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "1px", "1px", "1px"]
        );
        assert_eq!(
            sides("1px 2px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "2px", "1px", "2px"]
        );
        assert_eq!(
            sides("1px 2px 3px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "2px", "3px", "2px"]
        );
        assert_eq!(
            sides("1px 2px 3px 4px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "2px", "3px", "4px"]
        );
    }

    #[test]
    fn the_longhands_are_named_for_their_sides() {
        assert_eq!(
            sides("0")
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["margin-top", "margin-right", "margin-bottom", "margin-left"]
        );
    }

    /// A `var()` is one part like any other function. The first version of
    /// this refused to expand them, which left an author's
    /// `padding: var(--a) var(--b)` as the only unexpanded shorthand — so it
    /// lost to the user agent's expanded `padding-left`, and every control on
    /// every alo screen lost its padding.
    #[test]
    fn a_shorthand_holding_a_variable_is_still_split_by_side() {
        assert_eq!(
            sides("var(--gap) 2px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["var(--gap)", "2px", "var(--gap)", "2px"]
        );
    }

    /// A split on whitespace would make three values into six, putting
    /// `calc(2px` on one side and `+ 3px)` on another.
    #[test]
    fn the_spaces_inside_a_value_are_not_the_spaces_between_them() {
        assert_eq!(
            sides("1px calc(2px + 3px) var(--a, 4px 5px)")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec![
                "1px",
                "calc(2px + 3px)",
                "var(--a, 4px 5px)",
                "calc(2px + 3px)"
            ]
        );
    }

    #[test]
    fn a_shorthand_of_no_shape_this_engine_knows_is_left_whole() {
        assert!(sides("1px 2px 3px 4px 5px").is_empty());
        assert!(sides("").is_empty());
    }

    /// The three border shorthands that are one value per side split like the
    /// box ones, because they *are* box ones: the rule is the same.
    #[test]
    fn the_border_shorthands_that_are_one_value_per_side_split_too() {
        for (shorthand, first) in [
            ("border-width", "border-top-width"),
            ("border-style", "border-top-style"),
            ("border-color", "border-top-color"),
        ] {
            let split = expand(&Declaration::new(shorthand, "red blue", Importance::Normal));
            assert_eq!(split.len(), 4, "{shorthand} is four sides");
            assert_eq!(split.first().map(|(name, _)| name.as_str()), Some(first));
            assert_eq!(
                split
                    .iter()
                    .map(|(_, value)| value.as_str())
                    .collect::<Vec<_>>(),
                vec!["red", "blue", "red", "blue"],
                "{shorthand} pairs vertical then horizontal",
            );
        }
    }

    /// Only the shorthands that are one value per side. `border` itself takes
    /// its parts in any order — `red solid 1px` is the same border as
    /// `1px solid red` — so splitting it means parsing rather than counting,
    /// and it is read where it is used instead.
    ///
    /// `border-radius` is one value per **corner** and pairs the diagonals, so
    /// it is not one of these however much it looks like one.
    #[test]
    fn only_the_shorthands_this_engine_expands_are_expanded() {
        assert!(
            expand(&Declaration::new(
                "border",
                "1px solid red",
                Importance::Normal
            ))
            .is_empty()
        );
        assert!(
            expand(&Declaration::new(
                "border-radius",
                "4px 8px",
                Importance::Normal
            ))
            .is_empty()
        );
        assert!(expand(&Declaration::new("color", "red", Importance::Normal)).is_empty());
    }

    fn axes(shorthand: &str, value: &str) -> Vec<(String, String)> {
        expand(&Declaration::new(shorthand, value, Importance::Normal))
    }

    fn pair(align: &str, block: &str, justify: &str, inline: &str) -> Vec<(String, String)> {
        vec![
            (align.to_owned(), block.to_owned()),
            (justify.to_owned(), inline.to_owned()),
        ]
    }

    /// One value is both axes; two are block then inline — for all three.
    #[test]
    fn a_place_shorthand_becomes_its_block_and_inline_longhands() {
        for (shorthand, align, justify) in [
            ("place-items", "align-items", "justify-items"),
            ("place-self", "align-self", "justify-self"),
            ("place-content", "align-content", "justify-content"),
        ] {
            assert_eq!(
                axes(shorthand, "center"),
                pair(align, "center", justify, "center"),
                "{shorthand} with one value",
            );
            assert_eq!(
                axes(shorthand, "end start"),
                pair(align, "end", justify, "start"),
                "{shorthand} with two values",
            );
            assert!(
                axes(shorthand, "start center end").is_empty(),
                "{shorthand} with three values is no shape it has",
            );
        }
    }

    /// A value can be two words, and it is values that are counted.
    #[test]
    fn a_value_of_two_words_is_one_value() {
        assert_eq!(
            axes("place-items", "safe center unsafe end"),
            pair("align-items", "safe center", "justify-items", "unsafe end"),
        );
        assert_eq!(
            axes("place-self", "last baseline"),
            pair(
                "align-self",
                "last baseline",
                "justify-self",
                "last baseline"
            ),
        );
        assert_eq!(
            axes("place-items", "first baseline start"),
            pair("align-items", "first baseline", "justify-items", "start"),
        );
        assert_eq!(
            axes("place-content", "safe end space-between"),
            pair(
                "align-content",
                "safe end",
                "justify-content",
                "space-between"
            ),
        );
        assert_eq!(
            axes("place-items", "center legacy left"),
            pair("align-items", "center", "justify-items", "legacy left"),
            "`center legacy` is not a block-axis value, so `legacy left` is the inline one",
        );
        assert_eq!(
            axes("place-items", "start right legacy"),
            pair("align-items", "start", "justify-items", "right legacy"),
        );
        assert!(axes("place-items", "start end center").is_empty());
    }

    /// `justify-content` has no baseline, so `place-content` does not copy
    /// one there: the inline axis is `start`. The other two copy it.
    #[test]
    fn place_content_with_only_a_baseline_starts_the_inline_axis() {
        assert_eq!(
            axes("place-content", "baseline"),
            pair("align-content", "baseline", "justify-content", "start"),
        );
        assert_eq!(
            axes("place-content", "last baseline"),
            pair("align-content", "last baseline", "justify-content", "start"),
        );
        assert_eq!(
            axes("place-items", "baseline"),
            pair("align-items", "baseline", "justify-items", "baseline"),
        );
    }

    /// `legacy` is the inline axis's alone, so a value that leads with it is
    /// left whole rather than handing it to `align-items`.
    #[test]
    fn legacy_in_the_block_axis_is_left_whole() {
        assert!(axes("place-items", "legacy").is_empty());
        assert!(axes("place-items", "legacy left").is_empty());
        assert!(axes("place-items", "").is_empty());
    }

    /// A `var()` is one value, as it is for the sided shorthands.
    #[test]
    fn a_place_shorthand_holding_a_variable_copies_it() {
        assert_eq!(
            axes("place-items", "var(--where)"),
            pair(
                "align-items",
                "var(--where)",
                "justify-items",
                "var(--where)"
            ),
        );
    }

    /// In a block, the longhands sit at the shorthand's position, so a
    /// longhand written after it still wins.
    #[test]
    fn a_longhand_after_a_place_shorthand_still_wins() {
        let mut block = crate::DeclarationBlock::new();
        block.push(Declaration::new(
            "place-items",
            "center",
            Importance::Normal,
        ));
        block.push(Declaration::new(
            "justify-items",
            "start",
            Importance::Normal,
        ));
        let value = |name: &str| {
            block
                .get(&crate::PropertyName::parse(name))
                .map(|declaration| declaration.value.clone())
        };
        assert_eq!(value("align-items").as_deref(), Some("center"));
        assert_eq!(value("justify-items").as_deref(), Some("start"));
    }
}

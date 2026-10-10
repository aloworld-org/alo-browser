/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The properties this engine acts on, and the crate that acts on each
//! (ADR 0033 § 4).
//!
//! A property is **supported when some stage of this engine acts on it** —
//! style, boxes, layout, paint, SVG or the renderer's choice of fonts — and
//! not because a specification lists it. A script asks this list when it
//! writes `el.style.backgroundColor` or tests `'backdropFilter' in el.style`,
//! so a page that checks before using a feature reads the truth
//! (`docs/features.md`).
//!
//! # Kept true from both ends
//!
//! A test here holds the list sorted, with no name twice and every entry
//! naming a crate. A test **in each reading crate** holds the other end: it
//! scans that crate's own source for the names passed to its readers
//! ([`named_in`]) and asserts each one is listed with that crate, and that
//! every entry naming the crate is one it reads. So a property cannot be
//! read without being listed, or listed without being read.
//!
//! A property read **only to be refused** — an SVG `filter`, said not
//! applied and drawn as if absent — is not acted on, and is not here.
//! `alo-svg`'s test names those and asserts they stay off the list.
//!
//! Custom properties (`--anything`) are not listed and are always
//! supported, through `setProperty` and `getPropertyValue` only, as CSSOM
//! says.

/// A crate that reads a property's value from a computed style and acts on
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// `alo-style`: the font size, line height and colour it settles while
    /// cascading.
    Style,
    /// `alo-box`: which boxes there are.
    Boxes,
    /// `alo-layout`: where they go and how large they are.
    Layout,
    /// `alo-paint`: how they are drawn.
    Paint,
    /// `alo-svg`: how an SVG's shapes are drawn.
    Svg,
    /// `alo-renderer`: which fonts a page needs.
    Renderer,
}

impl Reader {
    /// The crate, by its name.
    pub const fn crate_name(self) -> &'static str {
        match self {
            Self::Style => "alo-style",
            Self::Boxes => "alo-box",
            Self::Layout => "alo-layout",
            Self::Paint => "alo-paint",
            Self::Svg => "alo-svg",
            Self::Renderer => "alo-renderer",
        }
    }
}

use Reader::{Boxes, Layout, Paint, Renderer, Style, Svg};

/// Every property this engine acts on, sorted, with the crates that read it.
pub static SUPPORTED: &[(&str, &[Reader])] = &[
    ("accent-color", &[Paint]),
    ("align-content", &[Layout]),
    ("align-items", &[Layout]),
    ("align-self", &[Layout]),
    ("aspect-ratio", &[Layout]),
    ("background", &[Paint]),
    ("background-color", &[Paint]),
    ("background-image", &[Paint]),
    ("border", &[Layout, Paint]),
    ("border-bottom", &[Layout, Paint]),
    ("border-bottom-color", &[Paint]),
    ("border-bottom-left-radius", &[Paint]),
    ("border-bottom-right-radius", &[Paint]),
    ("border-bottom-style", &[Paint]),
    ("border-bottom-width", &[Layout]),
    ("border-left", &[Layout, Paint]),
    ("border-left-color", &[Paint]),
    ("border-left-style", &[Paint]),
    ("border-left-width", &[Layout]),
    ("border-radius", &[Paint]),
    ("border-right", &[Layout, Paint]),
    ("border-right-color", &[Paint]),
    ("border-right-style", &[Paint]),
    ("border-right-width", &[Layout]),
    ("border-top", &[Layout, Paint]),
    ("border-top-color", &[Paint]),
    ("border-top-left-radius", &[Paint]),
    ("border-top-right-radius", &[Paint]),
    ("border-top-style", &[Paint]),
    ("border-top-width", &[Layout]),
    ("bottom", &[Layout]),
    ("box-shadow", &[Paint]),
    ("box-sizing", &[Layout]),
    ("color", &[Style, Paint]),
    ("column-gap", &[Layout]),
    ("display", &[Boxes, Svg]),
    ("fill", &[Svg]),
    ("fill-opacity", &[Svg]),
    ("fill-rule", &[Svg]),
    ("flex-basis", &[Layout]),
    ("flex-direction", &[Layout]),
    ("flex-grow", &[Layout]),
    ("flex-shrink", &[Layout]),
    ("flex-wrap", &[Layout]),
    ("font-family", &[Layout, Paint, Renderer]),
    ("font-size", &[Style]),
    ("font-style", &[Layout, Paint]),
    ("font-weight", &[Layout, Paint]),
    ("gap", &[Layout]),
    ("grid-auto-columns", &[Layout]),
    ("grid-auto-flow", &[Layout]),
    ("grid-auto-rows", &[Layout]),
    ("grid-column", &[Layout]),
    ("grid-row", &[Layout]),
    ("grid-template-columns", &[Layout]),
    ("grid-template-rows", &[Layout]),
    ("height", &[Layout]),
    ("justify-content", &[Layout]),
    ("justify-items", &[Layout]),
    ("justify-self", &[Layout]),
    ("left", &[Layout]),
    ("letter-spacing", &[Layout, Paint]),
    ("line-height", &[Style]),
    ("margin", &[Layout]),
    ("margin-bottom", &[Layout]),
    ("margin-left", &[Layout]),
    ("margin-right", &[Layout]),
    ("margin-top", &[Layout]),
    ("max-height", &[Layout]),
    ("max-width", &[Layout]),
    ("min-height", &[Layout]),
    ("min-width", &[Layout]),
    ("opacity", &[Paint, Svg]),
    ("overflow", &[Layout, Paint]),
    ("overflow-x", &[Layout, Paint]),
    ("overflow-y", &[Layout, Paint]),
    ("padding", &[Layout]),
    ("padding-bottom", &[Layout]),
    ("padding-left", &[Layout]),
    ("padding-right", &[Layout]),
    ("padding-top", &[Layout]),
    ("position", &[Boxes, Layout, Paint]),
    ("right", &[Layout]),
    ("row-gap", &[Layout]),
    ("stroke", &[Svg]),
    ("stroke-dasharray", &[Svg]),
    ("stroke-dashoffset", &[Svg]),
    ("stroke-linecap", &[Svg]),
    ("stroke-linejoin", &[Svg]),
    ("stroke-miterlimit", &[Svg]),
    ("stroke-opacity", &[Svg]),
    ("stroke-width", &[Svg]),
    ("text-align", &[Layout]),
    ("text-decoration", &[Paint]),
    ("text-decoration-color", &[Paint]),
    ("text-decoration-line", &[Paint]),
    ("text-shadow", &[Paint]),
    ("top", &[Layout]),
    ("transform", &[Layout, Paint, Svg]),
    ("transform-box", &[Svg]),
    ("transform-origin", &[Paint, Svg]),
    ("vertical-align", &[Layout]),
    ("visibility", &[Svg]),
    ("white-space", &[Boxes, Layout]),
    ("width", &[Layout]),
    ("z-index", &[Paint]),
];

/// Whether this engine acts on the property `name`, which must already be
/// lowercased: an entry in [`SUPPORTED`]. A custom property is not asked
/// about here.
pub fn is_supported(name: &str) -> bool {
    SUPPORTED
        .binary_search_by(|(listed, _)| (*listed).cmp(name))
        .is_ok()
}

/// Every property `reader` is listed as reading, in order.
pub fn read_by(reader: Reader) -> impl Iterator<Item = &'static str> {
    SUPPORTED
        .iter()
        .filter(move |(_, readers)| readers.contains(&reader))
        .map(|(name, _)| *name)
}

/// The four sides a `{side}` in a name a reader builds stands for.
const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// The property names a crate's source passes to its readers, sorted and
/// each once — what a reading crate's test compares with [`read_by`].
///
/// `source` is the crate's code. Only what comes before its first
/// `#[cfg(test)]` is read, and a line that is a `//` comment is skipped.
/// `readers` are the names of the functions and methods that take a
/// property's name — `get`, `color`, a crate's own `keyword` — and every
/// string literal inside the parentheses of a call to one of them is a
/// name it reads, a turbofish between name and parentheses allowed. A
/// literal with `{side}` in it is a name built for each side, as
/// `format!("border-{side}-width")` is, and stands for all four.
///
/// A name that reaches a reader through a variable — a loop over an array
/// of names — is not seen. The reading crate's test lists those by hand.
pub fn named_in(source: &str, readers: &[&str]) -> Vec<String> {
    let code: String = source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut names = Vec::new();
    for reader in readers {
        let mut rest = code.as_str();
        while let Some(found) = rest.find(reader) {
            let before = rest.get(..found).and_then(|text| text.chars().next_back());
            let after = rest
                .get(found.saturating_add(reader.len())..)
                .unwrap_or_default();
            rest = after;
            if before.is_some_and(|letter| letter.is_alphanumeric() || letter == '_') {
                continue;
            }
            let Some(arguments) = call_arguments(after) else {
                continue;
            };
            for literal in literals(arguments) {
                if literal.contains("{side}") {
                    names.extend(SIDES.iter().map(|side| literal.replace("{side}", side)));
                } else {
                    names.push(literal.to_owned());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The text inside the parentheses of a call whose name `after` follows:
/// what is between `(` and its matching `)`, with an optional `::<…>`
/// before them. [`None`] when `after` is not a call.
fn call_arguments(after: &str) -> Option<&str> {
    let mut text = after;
    if let Some(generic) = text.strip_prefix("::<") {
        text = generic.get(generic.find('>')?.saturating_add(1)..)?;
    }
    let inside = text.strip_prefix('(')?;
    let mut depth = 1usize;
    let mut in_string = false;
    for (at, letter) in inside.char_indices() {
        match letter {
            '"' => in_string = !in_string,
            '(' if !in_string => depth = depth.saturating_add(1),
            ')' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return inside.get(..at);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every `"…"` literal in `text`, without its quotes.
fn literals(text: &str) -> Vec<&str> {
    text.split('"').skip(1).step_by(2).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_sorted_and_names_each_property_once() {
        assert!(
            SUPPORTED.windows(2).all(|pair| match pair {
                [(first, _), (second, _)] => first < second,
                _ => false,
            }),
            "sorted, which is also what lets is_supported search it"
        );
    }

    #[test]
    fn every_entry_names_a_crate_that_reads_it_and_names_it_once() {
        for (name, readers) in SUPPORTED {
            assert!(!readers.is_empty(), "{name} names no crate");
            for (at, reader) in readers.iter().enumerate() {
                assert!(
                    !readers
                        .iter()
                        .skip(at.saturating_add(1))
                        .any(|r| r == reader),
                    "{name} names {} twice",
                    reader.crate_name()
                );
            }
        }
    }

    #[test]
    fn every_name_is_a_lowercase_property_name() {
        for (name, _) in SUPPORTED {
            assert!(!name.starts_with('-'), "{name}: no custom or vendor names");
            assert!(
                name.chars()
                    .all(|letter| letter.is_ascii_lowercase() || letter == '-'),
                "{name}"
            );
        }
    }

    #[test]
    fn a_supported_property_is_one_listed() {
        assert!(is_supported("background"));
        assert!(is_supported("accent-color"));
        assert!(is_supported("z-index"));
        assert!(!is_supported("cursor"));
        assert!(!is_supported("pointer-events"));
        assert!(
            !is_supported("font"),
            "read by no stage: font longhands are"
        );
        assert!(!is_supported("Background"), "the caller lowercases");
        assert!(!is_supported("--gap"));
        assert!(!is_supported(""));
    }

    #[test]
    fn a_reader_reads_what_it_is_listed_with() {
        let boxes: Vec<_> = read_by(Reader::Boxes).collect();
        assert_eq!(boxes, ["display", "position", "white-space"]);
        assert_eq!(Reader::Svg.crate_name(), "alo-svg");
    }

    #[test]
    fn a_source_is_scanned_for_the_names_passed_to_its_readers() {
        let source = r#"
            fn f(style: &S) {
                // style.get("in-a-comment")
                let a = style.get("width").or(style.color("color"));
                let b = reader.value_or::<Length>("row-gap", None);
                let c = reader.shorthand_side("margin", "margin-top", 0);
                let d = style.get(&format!("border-{side}-width"));
                let e = budget("not-a-reader") + forget("nor-this");
                let f = style.get(name);
            }
            #[cfg(test)]
            mod tests { fn g() { style.get("in-a-test"); } }
        "#;
        assert_eq!(
            named_in(source, &["get", "color", "value_or", "shorthand_side"]),
            [
                "border-bottom-width",
                "border-left-width",
                "border-right-width",
                "border-top-width",
                "color",
                "margin",
                "margin-top",
                "row-gap",
                "width",
            ]
        );
    }
}

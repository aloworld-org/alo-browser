/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a list item makes a `::marker`, and what it says (ADR 0044 §§ 1
//! and 3).
//!
//! An element **makes a marker** when its computed `display` is a list item
//! and its `list-style-type` is not `none`. That is decided here, where its
//! style is, and held once in the style tree: the style tree computes the
//! marker's style only for an element that makes one, and the box tree and
//! the agent tree read the same answer.
//!
//! # What it says
//!
//! CSS Counter Styles 3's representation of the item's ordinal value
//! ([`crate::ordinal`]), with its suffix:
//!
//! | `list-style-type` | text |
//! |---|---|
//! | `disc` | `• ` |
//! | `circle` | `◦ ` |
//! | `square` | `▪ ` |
//! | `decimal` | the number, `-` before a negative one, then `. ` |
//!
//! A name no specification defines is `decimal`, as Counter Styles says. A
//! predefined style this engine has not built — `lower-roman`,
//! `upper-alpha` and the rest — a `<string>`, `symbols()`, an `inside`
//! position and an image draw **no marker** and are said, because a
//! decimal "4." where a page asked for "iv." is approximate, and absent
//! beats approximate (ADR 0013 § 3; items 404–406).

use crate::computed::ComputedStyle;
use crate::display::Display;

/// What a list item's marker is, once it is known to make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marker {
    /// Drawn, saying this.
    Drawn(String),
    /// Not drawn, for the reason given, which names what was asked for.
    Refused(String),
}

/// Whether an element with this style makes a `::marker`: it is a list item
/// and its `list-style-type` is not `none` (ADR 0044 § 1).
pub fn makes_one(style: &ComputedStyle) -> bool {
    let is_list_item = style
        .get("display")
        .and_then(Display::parse)
        .is_some_and(Display::is_list_item);
    is_list_item
        && !style
            .get("list-style-type")
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("none"))
}

/// The marker an item that makes one draws, numbered `ordinal`, or why it
/// draws none.
pub fn marker(style: &ComputedStyle, ordinal: i32) -> Marker {
    if let Some(position) = style
        .get("list-style-position")
        .filter(|position| !position.trim().eq_ignore_ascii_case("outside"))
    {
        return Marker::Refused(format!(
            "list-style-position: {position} — only an outside marker is drawn (item 404)"
        ));
    }
    if let Some(image) = style
        .get("list-style-image")
        .filter(|image| !image.trim().eq_ignore_ascii_case("none"))
    {
        return Marker::Refused(format!(
            "list-style-image: {image} — a picture marker is not drawn (item 405)"
        ));
    }
    let kind = style.get("list-style-type").map_or("disc", str::trim);
    match text(kind, ordinal) {
        Some(text) => Marker::Drawn(text),
        None => Marker::Refused(format!(
            "list-style-type: {kind} — a counter style this engine does not draw (item 406)"
        )),
    }
}

/// What `disc`, `circle` and `square` say: a symbol and a space.
const SYMBOLS: [(&str, &str); 3] = [("disc", "• "), ("circle", "◦ "), ("square", "▪ ")];

/// Whether a marker's text is a symbol — a disc, a circle or a square —
/// rather than a number, which decides where it stands beside its line
/// (ADR 0044 § 4).
pub fn is_symbol(text: &str) -> bool {
    SYMBOLS.iter().any(|(_, symbol)| *symbol == text)
}

/// A counter style's representation of `ordinal` with its suffix, or
/// [`None`] for one this engine does not draw.
fn text(kind: &str, ordinal: i32) -> Option<String> {
    let named = |name: &str| kind.eq_ignore_ascii_case(name);
    if let Some((_, symbol)) = SYMBOLS.iter().find(|(name, _)| named(name)) {
        return Some((*symbol).to_owned());
    }
    if named("decimal") {
        return Some(format!("{ordinal}. "));
    }
    let is_a_name = kind
        .chars()
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '-' || first == '_')
        && !kind.contains(['(', '"', '\'', ' ']);
    if !is_a_name || PREDEFINED.iter().any(|predefined| named(predefined)) {
        return None;
    }
    // A name nothing defines is `decimal`.
    Some(format!("{ordinal}. "))
}

/// CSS Counter Styles 3's predefined styles beyond the four drawn, and the
/// names CSS Lists reserves. Each draws no marker until a page asks (item
/// 406).
const PREDEFINED: &[&str] = &[
    "decimal-leading-zero",
    "arabic-indic",
    "armenian",
    "upper-armenian",
    "lower-armenian",
    "bengali",
    "cambodian",
    "khmer",
    "cjk-decimal",
    "devanagari",
    "georgian",
    "gujarati",
    "gurmukhi",
    "hebrew",
    "kannada",
    "lao",
    "malayalam",
    "mongolian",
    "myanmar",
    "oriya",
    "persian",
    "lower-roman",
    "upper-roman",
    "tamil",
    "telugu",
    "thai",
    "tibetan",
    "lower-alpha",
    "lower-latin",
    "upper-alpha",
    "upper-latin",
    "lower-greek",
    "hiragana",
    "hiragana-iroha",
    "katakana",
    "katakana-iroha",
    "disclosure-open",
    "disclosure-closed",
    "cjk-earthly-branch",
    "cjk-heavenly-stem",
    "japanese-informal",
    "japanese-formal",
    "korean-hangul-formal",
    "korean-hanja-informal",
    "korean-hanja-formal",
    "simp-chinese-informal",
    "simp-chinese-formal",
    "trad-chinese-informal",
    "trad-chinese-formal",
    "ethiopic-numeric",
    "cjk-ideographic",
    "inherit",
    "initial",
    "unset",
    "revert",
    "revert-layer",
    "default",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascade::SourcedSheet;
    use crate::origin::Origin;
    use alo_css::{MediaContext, parse_stylesheet};

    /// The computed style of the element whose `id` is `wanted`.
    fn style(html: &str, author: &str, wanted: &str) -> ComputedStyle {
        let document = alo_dom::parse_document(html);
        let agent = parse_stylesheet(crate::user_agent::USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(author);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = crate::resolve(&document, &sheets, &MediaContext::default());
        document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))
            .and_then(|id| styles.get(id))
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn the_four_drawn_styles_say_what_counter_styles_says() {
        assert_eq!(text("disc", 1).as_deref(), Some("• "));
        assert_eq!(text("circle", 1).as_deref(), Some("◦ "));
        assert_eq!(text("square", 1).as_deref(), Some("▪ "));
        assert_eq!(text("decimal", 3).as_deref(), Some("3. "));
        assert_eq!(text("decimal", -2).as_deref(), Some("-2. "));
        assert_eq!(text("decimal", i32::MIN).as_deref(), Some("-2147483648. "));
        assert_eq!(text("DISC", 1).as_deref(), Some("• "), "any case");
        assert!(is_symbol("• ") && is_symbol("◦ ") && is_symbol("▪ "));
        assert!(!is_symbol("3. ") && !is_symbol("•"));
    }

    #[test]
    fn a_name_nothing_defines_is_decimal_and_an_unbuilt_one_is_nothing() {
        assert_eq!(text("my-own-style", 4).as_deref(), Some("4. "));
        for refused in [
            "lower-roman",
            "Upper-Alpha",
            "\"→ \"",
            "'x'",
            "symbols(cyclic '*')",
            "",
            "4",
        ] {
            assert_eq!(text(refused, 4), None, "{refused}");
        }
    }

    #[test]
    fn a_list_item_makes_one_unless_its_type_is_none() {
        let html = "<ul><li id=a>a</ul><ol><li id=b>b</ol><ul class=bare><li id=c>c</ul>\
                    <div id=d>d</div><div id=e class=item>e</div><ul><li id=f class=gone>f</ul>";
        let sheet = ".bare { list-style: none } .item { display: list-item } \
                     .gone { list-style-type: NONE }";
        for (id, makes) in [
            ("a", true),
            ("b", true),
            ("c", false),
            ("d", false),
            ("e", true),
            ("f", false),
        ] {
            assert_eq!(makes_one(&style(html, sheet, id)), makes, "{id}");
        }
    }

    #[test]
    fn inside_an_image_and_an_unbuilt_style_are_refused_by_name() {
        let html = "<ol><li id=a>a<li id=b>b<li id=c>c<li id=d>d</ol>";
        let sheet = "#a { list-style-position: inside } #b { list-style: url(dot.png) } \
                     #c { list-style-type: lower-roman } #d { list-style: square outside }";
        let refused = |id: &str| match marker(&style(html, sheet, id), 1) {
            Marker::Refused(why) => why,
            Marker::Drawn(text) => format!("drawn {text:?}"),
        };
        assert!(refused("a").contains("inside"), "{}", refused("a"));
        assert!(refused("b").contains("url(dot.png)"), "{}", refused("b"));
        assert!(refused("c").contains("lower-roman"), "{}", refused("c"));
        assert_eq!(
            marker(&style(html, sheet, "d"), 1),
            Marker::Drawn("▪ ".to_owned())
        );
    }
}

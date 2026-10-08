/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An element's inline declaration block, as a script reads and edits it
//! (ADR 0033 §§ 3–5): what `element.style` does to the `style` attribute,
//! with no heap and no document in sight.
//!
//! The attribute is the only copy of the block. A script's every read
//! parses it ([`InlineStyle::parse`]), and every write parses it, edits
//! what was parsed and serialises the result ([`InlineStyle::serialize`])
//! to be written back. Nothing here is kept between the two.
//!
//! # What the block holds
//!
//! **The declarations that were written, one per property.** Parsing is the
//! style sheet's own ([`crate::parse_declaration_list`]); then, as CSSOM's
//! *parse a CSS declaration block* does, a property written twice is held
//! once — by the declaration the cascade would pick between the two, an
//! important one over a normal one and otherwise the later, at the later
//! one's place. The longhands a counted shorthand implies are not held,
//! since nobody wrote them; they are still read ([`InlineStyle::value`]).
//!
//! # Where this differs from CSSOM, and why
//!
//! CSSOM parses a value against its property's grammar when it is set, holds
//! a shorthand as its longhands and reads a value back in canonical form.
//! This engine keeps a value as written and does not split `background`,
//! `border` or `font` (ADR 0033, *Why this is a decision*). So:
//!
//! - **A value is accepted when a style sheet would keep it** as one
//!   declaration's value ([`InlineStyle::set`]), and refused where it is
//!   read if it cannot be used, as a sheet's is.
//! - **A value reads back as written**: `#c7bfb2`, not `rgb(199, 191, 178)`.
//! - **A shorthand read by kind stays one declaration.** Setting any
//!   shorthand first removes the written declarations of its longhands
//!   ([`crate::longhand`]), and `background-color` after `background: red`
//!   reads `""`.
//! - **`length` counts what was written**, so `margin: 0` is one
//!   declaration, where other engines count its four sides.
//!
//! # A value that would swallow what follows it
//!
//! A value is accepted only when `name: value` followed by another
//! declaration still reads as two: an unclosed string, comment or bracket
//! would carry everything after it into its value once serialised. A
//! declaration the attribute already held that fails the same test — one
//! left unclosed at the attribute's very end — is not held, so a script's
//! first write leaves it out of the attribute rather than corrupting
//! everything written after it. CSSOM would have closed it; this engine
//! keeps values as written and cannot.

use crate::declaration::{Declaration, DeclarationBlock, Importance, PropertyName};
use crate::longhand;
use crate::parse::parse_declaration_list;
use crate::properties;

/// The declaration a value is tested against: written after it, it must
/// still be read as itself.
const SENTINEL: &str = "--alo-inline-end";

/// The word a value is tested against: written after it, it must still be
/// part of it.
const MARK: &str = "alo-inline-end";

/// An element's inline declaration block, as parsed from its `style`
/// attribute.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InlineStyle {
    declarations: Vec<Declaration>,
}

/// What a `setProperty` or `removeProperty` did to the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// It changed: the block is to be serialised and written back.
    Changed,
    /// Nothing changed — the same value and priority were already held, or
    /// nothing of that name was there to remove — so nothing is written.
    Unchanged,
    /// CSSOM's steps returned before changing anything: a property this
    /// engine does not act on, a priority that is neither empty nor
    /// `important`, or a value a style sheet would not keep.
    Ignored,
}

impl InlineStyle {
    /// The block a `style` attribute's text holds.
    pub fn parse(attribute: &str) -> Self {
        let (block, _dropped) = parse_declaration_list(attribute);
        let mut style = Self::default();
        for declaration in block.written() {
            if survives_what_follows(declaration) {
                style.hold(declaration.clone());
            }
        }
        style
    }

    /// Hold `declaration` as parsing a block does: in place of a held one
    /// of its name unless that one is important and it is not, and at the
    /// end.
    fn hold(&mut self, declaration: Declaration) {
        if let Some(at) = self.position(&declaration.name) {
            let held_wins = self
                .declarations
                .get(at)
                .is_some_and(|held| held.importance.is_important())
                && !declaration.importance.is_important();
            if held_wins {
                return;
            }
            self.declarations.remove(at);
        }
        self.declarations.push(declaration);
    }

    /// Where the declaration of `name` is.
    fn position(&self, name: &PropertyName) -> Option<usize> {
        self.declarations
            .iter()
            .position(|declaration| &declaration.name == name)
    }

    /// How many declarations it holds: CSSOM's `length`.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// The name of the declaration at `index`: CSSOM's `item()`.
    pub fn item(&self, index: usize) -> Option<&str> {
        self.declarations
            .get(index)
            .map(|declaration| declaration.name.as_str())
    }

    /// The declaration that decides `name`: among those held and the
    /// longhands their counted shorthands imply, an important one over a
    /// normal one, and otherwise the last.
    fn deciding(&self, name: &str) -> Option<Declaration> {
        let name = PropertyName::parse(name);
        let mut block = DeclarationBlock::new();
        for declaration in &self.declarations {
            block.push(declaration.clone());
        }
        let mut of_name = block.iter().filter(|declaration| declaration.name == name);
        let last = of_name.clone().next_back().cloned();
        of_name
            .rfind(|declaration| declaration.importance.is_important())
            .cloned()
            .or(last)
    }

    /// CSSOM's `getPropertyValue`: the value as written of the declaration
    /// that decides `name`, or `""`. An ordinary name is compared without
    /// regard to case and a custom one with it.
    pub fn value(&self, name: &str) -> String {
        self.deciding(name)
            .map(|declaration| declaration.value)
            .unwrap_or_default()
    }

    /// CSSOM's `getPropertyPriority`: `"important"` when the declaration
    /// that decides `name` is, and otherwise `""`.
    pub fn priority(&self, name: &str) -> &'static str {
        match self.deciding(name) {
            Some(declaration) if declaration.importance.is_important() => "important",
            _ => "",
        }
    }

    /// CSSOM's `setProperty(name, value, priority)`, its step 6 replaced by
    /// ADR 0033 § 5.
    ///
    /// An ordinary name is lowercased and must be one this engine acts on
    /// ([`properties::is_supported`]); a custom one is taken as written. An
    /// empty value is a removal. The priority must be empty or `important`,
    /// in any case. The value must be one a style sheet would keep as one
    /// declaration's: something once trimmed, carrying no `!important` and
    /// no `;` of its own, that does not swallow a declaration written after
    /// it. Then the written declarations of the property's longhands are
    /// removed, if it is a shorthand, and it is set in place or at the end.
    pub fn set(&mut self, name: &str, value: &str, priority: &str) -> Edit {
        let name = PropertyName::parse(name);
        if !name.is_custom() && !properties::is_supported(name.as_str()) {
            return Edit::Ignored;
        }
        if value.is_empty() {
            return self.remove(name.as_str());
        }
        let importance = match priority {
            "" => Importance::Normal,
            _ if priority.eq_ignore_ascii_case("important") => Importance::Important,
            _ => return Edit::Ignored,
        };
        let Some(value) = kept_value(&name, value) else {
            return Edit::Ignored;
        };
        let mut edit = Edit::Unchanged;
        if let Some(longhands) = longhand::longhands(name.as_str()) {
            let before = self.declarations.len();
            self.declarations
                .retain(|held| !longhands.contains(&held.name.as_str()));
            if self.declarations.len() != before {
                edit = Edit::Changed;
            }
        }
        let at = self.position(&name);
        if let Some(held) = at.and_then(|at| self.declarations.get_mut(at)) {
            if held.value != value || held.importance != importance {
                held.value = value;
                held.importance = importance;
                edit = Edit::Changed;
            }
        } else {
            self.declarations.push(Declaration {
                name,
                value,
                importance,
            });
            edit = Edit::Changed;
        }
        edit
    }

    /// CSSOM's `removeProperty(name)`: the declaration of `name` removed,
    /// and those of its longhands if it is a shorthand. Any name may be
    /// removed, acted on or not.
    pub fn remove(&mut self, name: &str) -> Edit {
        let name = PropertyName::parse(name);
        let longhands = longhand::longhands(name.as_str()).unwrap_or_default();
        let before = self.declarations.len();
        self.declarations.retain(|held| {
            held.name != name && (name.is_custom() || !longhands.contains(&held.name.as_str()))
        });
        if self.declarations.len() == before {
            Edit::Unchanged
        } else {
            Edit::Changed
        }
    }

    /// CSSOM's *serialize a CSS declaration block*, without its
    /// shorthand-combining step, since a shorthand is held whole: each
    /// declaration as written — `name: value`, then ` !important` if it is —
    /// with a `;` after it, joined by one space. `cssText`, and what is
    /// written back to the attribute.
    pub fn serialize(&self) -> String {
        let mut text = String::new();
        for declaration in &self.declarations {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&declaration.to_string());
            text.push(';');
        }
        text
    }
}

/// The value `value` holds as `name`'s, as a style sheet would keep it — or
/// [`None`] when a sheet would not keep it as one declaration's value.
///
/// Two readings decide. Followed by another declaration, it must read as a
/// normal declaration of `name` and leave the other alone: that refuses an
/// `!important` of its own, a second declaration and anything unclosed.
/// Followed by a word, the word must still be part of its value: that
/// refuses a `;` of its own, which the first reading cannot see, since an
/// empty declaration reads as nothing.
fn kept_value(name: &PropertyName, value: &str) -> Option<String> {
    let probe = Declaration::new(name.as_str(), value, Importance::Normal);
    if probe.value.is_empty() {
        return None;
    }
    let (block, dropped) = parse_declaration_list(&format!("{name}: {value}; {SENTINEL}: 0"));
    if !dropped.is_empty() {
        return None;
    }
    let kept = match block.written().collect::<Vec<_>>().as_slice() {
        [read, sentinel]
            if read.name == *name
                && read.importance == Importance::Normal
                && !read.value.is_empty()
                && sentinel.name.as_str() == SENTINEL =>
        {
            read.value.clone()
        }
        _ => return None,
    };
    let (block, dropped) = parse_declaration_list(&format!("{name}: {value} {MARK}"));
    let one_value = dropped.is_empty()
        && matches!(
            block.written().collect::<Vec<_>>().as_slice(),
            [read] if read.name == *name && read.value.ends_with(MARK)
        );
    one_value.then_some(kept)
}

/// Whether `declaration`, serialised and followed by another, still reads
/// as itself and leaves the other alone.
fn survives_what_follows(declaration: &Declaration) -> bool {
    let (block, _dropped) = parse_declaration_list(&format!("{declaration}; {SENTINEL}: 0"));
    match block.written().collect::<Vec<_>>().as_slice() {
        [read, sentinel] => *read == declaration && sentinel.name.as_str() == SENTINEL,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(attribute: &str, name: &str, value: &str, priority: &str) -> (Edit, String) {
        let mut style = InlineStyle::parse(attribute);
        let edit = style.set(name, value, priority);
        (edit, style.serialize())
    }

    #[test]
    fn a_block_is_what_was_written_one_declaration_per_property() {
        let style = InlineStyle::parse("color: red; margin: 0 4px; COLOR: blue");
        assert_eq!(style.len(), 2);
        assert_eq!(style.item(0), Some("margin"));
        assert_eq!(style.item(1), Some("color"));
        assert_eq!(style.item(2), None);
        assert_eq!(style.serialize(), "margin: 0 4px; color: blue;");
    }

    #[test]
    fn an_important_declaration_is_held_over_a_later_normal_one() {
        let style = InlineStyle::parse("color: red !important; color: blue");
        assert_eq!(style.serialize(), "color: red !important;");
        assert_eq!(style.value("color"), "red");
        assert_eq!(style.priority("color"), "important");
    }

    #[test]
    fn what_cannot_be_read_is_not_held() {
        let style = InlineStyle::parse("12px; color: red; : x; width");
        assert_eq!(style.serialize(), "color: red;");
    }

    #[test]
    fn a_value_reads_back_as_written() {
        let style = InlineStyle::parse("background: #C7BFB2; --Gap: 8px");
        assert_eq!(style.value("BACKGROUND"), "#C7BFB2");
        assert_eq!(style.value("--Gap"), "8px");
        assert_eq!(style.value("--gap"), "", "a custom name keeps its case");
        assert_eq!(style.value("color"), "");
        assert_eq!(style.priority("background"), "");
    }

    #[test]
    fn a_counted_shorthands_longhands_are_read_but_not_held() {
        let style = InlineStyle::parse("margin: 0 4px !important; margin-top: 9px");
        assert_eq!(style.value("margin-left"), "4px");
        assert_eq!(
            style.value("margin-top"),
            "0",
            "the shorthand's important side beats the later normal one"
        );
        assert_eq!(style.priority("margin-right"), "important");
        assert_eq!(style.len(), 2);
    }

    #[test]
    fn a_shorthand_read_by_kind_answers_nothing_for_its_longhands() {
        let style = InlineStyle::parse("background: red");
        assert_eq!(style.value("background-color"), "");
    }

    #[test]
    fn setting_appends_or_replaces_in_place() {
        assert_eq!(
            set("", "background", "#c7bfb2", ""),
            (Edit::Changed, "background: #c7bfb2;".to_owned()),
        );
        assert_eq!(
            set("color: red; width: 1px", "Color", "blue", "IMPORTANT"),
            (
                Edit::Changed,
                "color: blue !important; width: 1px;".to_owned()
            ),
        );
        assert_eq!(
            set("--a: 1", "--a", " 2 ", ""),
            (Edit::Changed, "--a: 2;".to_owned()),
        );
    }

    #[test]
    fn setting_what_is_already_held_changes_nothing() {
        assert_eq!(
            set("color:red", "color", "red", ""),
            (Edit::Unchanged, "color: red;".to_owned()),
        );
        assert_eq!(
            set("color: red", "color", "red", "important").0,
            Edit::Changed,
            "the priority is part of what is held"
        );
    }

    #[test]
    fn setting_a_shorthand_removes_its_written_longhands() {
        assert_eq!(
            set(
                "background-color: red; color: red; background-image: none",
                "background",
                "blue",
                ""
            ),
            (Edit::Changed, "color: red; background: blue;".to_owned()),
        );
        assert_eq!(
            set("margin-left: 1px; margin: 2px", "margin", "2px", ""),
            (Edit::Changed, "margin: 2px;".to_owned()),
            "the same value, but a longhand went"
        );
        assert_eq!(
            set("background: blue", "background-color", "red", ""),
            (
                Edit::Changed,
                "background: blue; background-color: red;".to_owned()
            ),
            "a longhand leaves its shorthand alone and goes after it"
        );
    }

    #[test]
    fn a_property_this_engine_does_not_act_on_is_ignored() {
        assert_eq!(
            set("color: red", "cursor", "default", ""),
            (Edit::Ignored, "color: red;".to_owned())
        );
        assert_eq!(set("", "pointer-events", "none", "").0, Edit::Ignored);
        assert_eq!(set("", "pointerEvents", "none", "").0, Edit::Ignored);
        assert_eq!(set("", "", "red", "").0, Edit::Ignored);
    }

    #[test]
    fn a_priority_must_be_empty_or_important() {
        assert_eq!(set("", "color", "red", "!important").0, Edit::Ignored);
        assert_eq!(set("", "color", "red", "true").0, Edit::Ignored);
    }

    #[test]
    fn an_empty_value_removes() {
        assert_eq!(
            set("color: red; width: 1px", "color", "", ""),
            (Edit::Changed, "width: 1px;".to_owned())
        );
        assert_eq!(set("width: 1px", "color", "", "").0, Edit::Unchanged);
    }

    #[test]
    fn removing_takes_a_shorthands_longhands_with_it() {
        let mut style = InlineStyle::parse(
            "margin-top: 1px; color: red; margin: 0; cursor: default; --margin-top: 2",
        );
        assert_eq!(style.remove("MARGIN"), Edit::Changed);
        assert_eq!(
            style.serialize(),
            "color: red; cursor: default; --margin-top: 2;"
        );
        assert_eq!(
            style.remove("cursor"),
            Edit::Changed,
            "any name may be removed"
        );
        assert_eq!(style.remove("cursor"), Edit::Unchanged);
        assert_eq!(style.remove("--MARGIN-TOP"), Edit::Unchanged);
        assert_eq!(style.remove("--margin-top"), Edit::Changed);
        assert_eq!(style.serialize(), "color: red;");
    }

    #[test]
    fn a_value_a_sheet_would_not_keep_as_one_declarations_is_refused() {
        for value in [
            "   ",
            "red; width: 1px",
            "red;",
            "red !important",
            "red ! IMPORTANT",
            "rgb(1, 2",
            "\"open",
            "url(open",
            "red /* open",
            "[",
            "{",
            "12px; --alo-inline-end: 0",
        ] {
            assert_eq!(
                set("width: 1px", "color", value, ""),
                (Edit::Ignored, "width: 1px;".to_owned()),
                "{value:?}"
            );
        }
    }

    #[test]
    fn a_value_a_sheet_would_keep_is_kept_however_odd() {
        for (value, kept) in [
            ("red}", "red}"),
            ("red )", "red )"),
            ("red /* note */", "red"),
            ("bogus garbage 12", "bogus garbage 12"),
            ("\0", "\0"),
        ] {
            assert_eq!(
                set("", "color", value, ""),
                (Edit::Changed, format!("color: {kept};")),
                "{value:?}"
            );
        }
    }

    #[test]
    fn an_unclosed_declaration_at_the_attributes_end_is_not_held() {
        let style = InlineStyle::parse("width: 1px; color: \"open");
        assert_eq!(style.serialize(), "width: 1px;");
        let style = InlineStyle::parse("width: 1px; background: url(a.png");
        assert_eq!(style.serialize(), "width: 1px;");
    }

    #[test]
    fn hostile_attributes_and_values_are_answered_never_panicked_on() {
        let deep = "(".repeat(100_000);
        let braces = "{".repeat(100_000);
        let long = "a".repeat(1 << 20);
        for attribute in [
            deep.as_str(),
            braces.as_str(),
            "}}}}",
            "\u{feff}color: red",
            "color: red\0; width: \0",
            "@media x { color: red }",
            "a { color: red }",
            ";;;;",
        ] {
            let mut style = InlineStyle::parse(attribute);
            let _ = style.serialize();
            let _ = style.set("color", "blue", "");
            let _ = style.remove("color");
        }
        for value in [deep.as_str(), braces.as_str()] {
            assert_eq!(set("", "color", value, "").0, Edit::Ignored);
        }
        let (edit, text) = set("", "color", &long, "");
        assert_eq!(edit, Edit::Changed);
        assert_eq!(text.len(), "color: ;".len() + long.len());
        let mut style = InlineStyle::parse(&text);
        assert_eq!(style.value("color").len(), long.len());
        assert_eq!(style.remove("color"), Edit::Changed);
    }

    #[test]
    fn a_serialised_block_parses_back_to_itself() {
        let mut style = InlineStyle::parse("margin: 0 4px; color: red !important");
        let _ = style.set("background", "url(\"a b.png\") no-repeat", "");
        let _ = style.set("--x", "{ a: b }", "");
        let text = style.serialize();
        assert_eq!(InlineStyle::parse(&text), style, "{text}");
    }
}

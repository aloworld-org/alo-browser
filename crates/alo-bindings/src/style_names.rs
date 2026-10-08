/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The names a `CSSStyleDeclaration` answers a property under (CSSOM § 6.7,
//! ADR 0033 § 4, queue item 342).
//!
//! For every property this engine acts on ([`alo_css::properties`]),
//! CSSOM puts accessors on `CSSStyleDeclaration.prototype` under up to
//! three names:
//!
//! - the **camel-cased attribute**, `backgroundColor`, for every property;
//! - the **WebKit-cased attribute**, `webkitTransform` for
//!   `-webkit-transform`, for every property spelled with that prefix;
//! - the **dashed attribute**, `background-color`, for every property with
//!   a `-` in its name.
//!
//! Each is made from the property's name by CSSOM's *CSS property to IDL
//! attribute* algorithm ([`attribute_of`]). Nothing else is named: a page
//! that writes `el.style.cursor` writes an ordinary property of the object,
//! because no stage of this engine acts on `cursor` (ADR 0033 § 4).
//!
//! The list is made once, the first time a page's prototypes are, and kept
//! for as long as the process runs, so a member's native can be named by
//! it without anything being leaked or copied ([`ATTRIBUTES`]).
//!
//! Nothing here touches a heap; [`crate::interface::css_style_declaration`]
//! puts the accessors on the prototype.

use std::sync::LazyLock;

use alo_css::properties::SUPPORTED;

/// One accessor a `CSSStyleDeclaration` has for a property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Named {
    /// The name it is under on the prototype.
    pub attribute: String,
    /// The property it reads and writes, as CSS spells it.
    pub property: &'static str,
}

/// Every accessor, for every property in [`SUPPORTED`]: all the
/// camel-cased attributes in the list's order, then the WebKit-cased ones,
/// then the dashed ones — the order of CSSOM's three partial interfaces.
pub(crate) static ATTRIBUTES: LazyLock<Vec<Named>> =
    LazyLock::new(|| attributes(SUPPORTED.iter().map(|(name, _)| *name)));

/// The prefix a property has for its WebKit-cased attribute.
const WEBKIT: &str = "-webkit-";

/// The accessors `properties` are named by, in [`ATTRIBUTES`]' order.
fn attributes(properties: impl Iterator<Item = &'static str> + Clone) -> Vec<Named> {
    let camel = properties.clone().map(|property| Named {
        attribute: attribute_of(property, false),
        property,
    });
    let webkit = properties
        .clone()
        .filter(|property| property.starts_with(WEBKIT))
        .map(|property| Named {
            attribute: attribute_of(property, true),
            property,
        });
    let dashed = properties
        .filter(|property| property.contains('-'))
        .map(|property| Named {
            attribute: property.to_owned(),
            property,
        });
    camel.chain(webkit).chain(dashed).collect()
}

/// CSSOM's *CSS property to IDL attribute*: every `-` dropped and the
/// character after it uppercased — after the first character is dropped,
/// when `lowercase_first` asks for the WebKit-cased name.
pub(crate) fn attribute_of(property: &str, lowercase_first: bool) -> String {
    let mut characters = property.chars();
    if lowercase_first {
        characters.next();
    }
    let mut out = String::with_capacity(property.len());
    let mut uppercase_next = false;
    for character in characters {
        if character == '-' {
            uppercase_next = true;
        } else if uppercase_next {
            uppercase_next = false;
            out.push(character.to_ascii_uppercase());
        } else {
            out.push(character);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_is_camel_cased_as_cssom_says() {
        assert_eq!(attribute_of("background-color", false), "backgroundColor");
        assert_eq!(attribute_of("color", false), "color");
        assert_eq!(
            attribute_of("border-top-left-radius", false),
            "borderTopLeftRadius"
        );
        // A leading dash is uppercased into the name, unless the WebKit
        // spelling drops it first.
        assert_eq!(attribute_of("-webkit-transform", false), "WebkitTransform");
        assert_eq!(attribute_of("-webkit-transform", true), "webkitTransform");
    }

    #[test]
    fn each_property_has_its_camel_webkit_and_dashed_names_in_order() {
        let made = attributes(["-webkit-mask", "color", "font-size"].into_iter());
        let names: Vec<(&str, &str)> = made
            .iter()
            .map(|named| (named.attribute.as_str(), named.property))
            .collect();
        assert_eq!(
            names,
            [
                ("WebkitMask", "-webkit-mask"),
                ("color", "color"),
                ("fontSize", "font-size"),
                ("webkitMask", "-webkit-mask"),
                ("-webkit-mask", "-webkit-mask"),
                ("font-size", "font-size"),
            ]
        );
    }

    #[test]
    fn the_engines_list_names_what_the_page_writes_and_not_what_it_does_not() {
        let has = |attribute: &str| ATTRIBUTES.iter().any(|named| named.attribute == attribute);
        assert!(has("background"));
        assert!(has("backgroundColor"));
        assert!(has("background-color"));
        // No stage acts on these, so `alo-downloads`' writes to them are
        // ordinary properties of the object.
        assert!(!has("cursor"));
        assert!(!has("pointerEvents"));
        assert!(!has("pointer-events"));
        // `float` is not acted on (law 1), so neither it nor `cssFloat` is
        // named.
        assert!(!has("float") && !has("cssFloat"));
        // No name is given twice: a property without a dash has one name,
        // which is both its camel-cased and its dashed one.
        let mut seen: Vec<&str> = ATTRIBUTES
            .iter()
            .map(|named| named.attribute.as_str())
            .collect();
        let count = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), count);
    }
}

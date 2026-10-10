/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a pseudo-element's style is made from (ADR 0043 §§ 1 and 3).
//!
//! A pseudo-element is styled **as its originating element's child**: from
//! the rules that name it and match that element, by the same cascade,
//! inheriting from the element's computed style — custom properties
//! included, so `var(--text-tertiary)` on `::placeholder` reads the one the
//! field sees. [`crate::computed`] does the computing, once the element's own
//! style is finished; this says which declarations take part.
//!
//! # Which are read
//!
//! Each pseudo-element is read for a few of the properties that apply to
//! it, beside the custom properties every one takes. A declaration of one
//! that applies and is not read is **recorded** as
//! [`IssueKind::PropertyNotReadOnPseudoElement`] and left out, so the
//! pseudo-element keeps what it inherits from its element. A property that
//! does not apply to it at all — `width`, `margin`, `display` — is left out
//! without a word, as CSS ignores it.
//!
//! - **`::placeholder`** takes the properties `::first-line` takes — fonts,
//!   colour, backgrounds, spacing, decorations — and this engine reads one
//!   of them, `color`. Each of the others either moves the hint's run or
//!   paints a box around it, and no page has asked for one (item 392).
//! - **`::marker`** takes the font properties, `color`, `white-space`,
//!   `content`, `direction`, `unicode-bidi`, `text-combine-upright` and the
//!   animations and transitions (CSS Lists 3 § 3.2), and this engine reads
//!   `color` and the font properties text is set in: `font-family`,
//!   `font-size`, `font-style` and `font-weight` (ADR 0044 § 2). The rest,
//!   `content` among them, waits for a page (items 406 and 408).

use crate::cascade::{Applicable, SourcedSheet};
use alo_css::{IssueKind, MatchContext, MediaContext, PropertyName, PseudoElement, StyleIssue};
use alo_dom::NodeId;

/// The declarations that take part in the style of `pseudo` on the element
/// `id`, with every refusal recorded in `issues`.
pub(crate) fn applicable<'a>(
    sheets: &[SourcedSheet<'a>],
    device: &MediaContext,
    matcher: &mut MatchContext<'_>,
    id: NodeId,
    pseudo: PseudoElement,
    issues: &mut Vec<StyleIssue>,
) -> Applicable<'a> {
    let mut applicable = Applicable::gather_pseudo(sheets, device, matcher, id, pseudo);
    applicable.retain(
        |name| name.is_custom() || is_read(pseudo, name),
        |name, winner| {
            if applies(pseudo, name) {
                issues.push(StyleIssue {
                    kind: IssueKind::PropertyNotReadOnPseudoElement,
                    source: format!(
                        "{}: {} on {} (of {id})",
                        name.as_str(),
                        winner.declaration.value,
                        pseudo.as_str()
                    ),
                    at: winner.at,
                });
            }
        },
    );
    applicable
}

/// The font properties a run of text is set in, which a marker reads.
const FONT: [&str; 4] = ["font-family", "font-size", "font-style", "font-weight"];

/// Whether this engine reads `name` on `pseudo`.
fn is_read(pseudo: PseudoElement, name: &PropertyName) -> bool {
    let name = name.as_str();
    match pseudo {
        PseudoElement::Marker => name == "color" || FONT.contains(&name),
        _ => name == "color",
    }
}

/// Whether a property applies to `pseudo` at all.
fn applies(pseudo: PseudoElement, name: &PropertyName) -> bool {
    match pseudo {
        PseudoElement::Marker => applies_to_marker(name),
        _ => applies_to_placeholder(name),
    }
}

/// Whether a property applies to `::marker`: CSS Lists 3 § 3.2's list.
fn applies_to_marker(name: &PropertyName) -> bool {
    let name = name.as_str();
    ["font", "animation", "transition"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || [
            "color",
            "white-space",
            "content",
            "direction",
            "unicode-bidi",
            "text-combine-upright",
        ]
        .contains(&name)
}

/// Whether a property applies to `::placeholder`: CSS Pseudo-Elements 4
/// gives it `::first-line`'s list (§ 2.1.1), which is the font, colour and
/// background properties, opacity, the typesetting ones and the text
/// decorations.
fn applies_to_placeholder(name: &PropertyName) -> bool {
    let name = name.as_str();
    ["font", "background", "text-decoration", "text-emphasis"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || [
            "color",
            "opacity",
            "letter-spacing",
            "word-spacing",
            "line-height",
            "text-transform",
            "text-shadow",
            "text-underline-position",
            "text-underline-offset",
            "vertical-align",
            "ruby-position",
        ]
        .contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_properties_apply_and_box_properties_do_not() {
        for name in [
            "font-style",
            "font",
            "font-size",
            "background-color",
            "background",
            "text-decoration-line",
            "letter-spacing",
            "text-transform",
            "line-height",
            "opacity",
        ] {
            assert!(applies_to_placeholder(&PropertyName::parse(name)), "{name}");
        }
        for name in [
            "width",
            "margin",
            "display",
            "padding-left",
            "border",
            "position",
        ] {
            assert!(
                !applies_to_placeholder(&PropertyName::parse(name)),
                "{name}"
            );
        }
    }

    #[test]
    fn a_marker_reads_its_colour_and_font_and_applies_to_little_else() {
        let marker = PseudoElement::Marker;
        for name in [
            "color",
            "font-family",
            "font-size",
            "font-style",
            "font-weight",
        ] {
            assert!(is_read(marker, &PropertyName::parse(name)), "{name}");
        }
        for name in ["content", "white-space", "font-variant", "animation-name"] {
            let name = PropertyName::parse(name);
            assert!(
                applies(marker, &name) && !is_read(marker, &name),
                "{name:?}"
            );
        }
        for name in ["width", "margin", "background-color", "padding-left"] {
            assert!(!applies(marker, &PropertyName::parse(name)), "{name}");
        }
        // A placeholder still reads its colour and nothing of its font.
        let placeholder = PseudoElement::Placeholder;
        assert!(is_read(placeholder, &PropertyName::parse("color")));
        assert!(!is_read(placeholder, &PropertyName::parse("font-size")));
    }
}

//! What a document says about an element's state.
//!
//! A pseudo-class like `:disabled` is not a fact about CSS; it is a fact about
//! HTML, and HTML defines it carefully enough that approximating it produces a
//! renderer that is wrong in ways nobody can predict. A disabled `<fieldset>`
//! disables its controls but not the ones inside its first `<legend>`; a
//! `readonly` attribute means nothing on a checkbox. So these are written out
//! rather than guessed at.
//!
//! **Everything here is determinable from a static tree.** Stage 1 has no
//! scripting and no input, so there is no state anywhere except what the
//! markup says. The pseudo-classes that describe a person acting right now —
//! `:hover`, `:focus`, `:active` — are not here, because there is nothing
//! true to say about them yet; [`crate::selector::PseudoClass`] names them as
//! interaction states and [`crate::matching`] refuses them.

use alo_dom::{Document, Element, NodeId};

/// Elements that a `disabled` attribute means something on.
const CAN_BE_DISABLED: &[&str] = &[
    "button", "input", "select", "textarea", "optgroup", "option", "fieldset",
];

/// The form controls a disabled ancestor `<fieldset>` reaches.
const DISABLED_BY_FIELDSET: &[&str] = &["button", "input", "select", "textarea"];

/// Elements a `required` attribute means something on.
const CAN_BE_REQUIRED: &[&str] = &["input", "select", "textarea"];

/// `<input>` types that cannot be required, however the attribute is written.
const NEVER_REQUIRED: &[&str] = &[
    "hidden", "range", "color", "submit", "image", "reset", "button",
];

/// `<input>` types a person types into, and so the only ones `readonly` means
/// anything on.
const TEXT_ENTRY_TYPES: &[&str] = &[
    "text",
    "search",
    "url",
    "tel",
    "email",
    "password",
    "date",
    "month",
    "week",
    "time",
    "datetime-local",
    "number",
];

fn is_one_of(element: &Element, names: &[&str]) -> bool {
    names.iter().any(|name| element.name.is_html(name))
}

/// The `type` of an `<input>`, lowercased. Missing or unknown is `text`, which
/// is what HTML says.
fn input_type(element: &Element) -> String {
    element
        .attr("type")
        .map_or_else(|| "text".to_owned(), str::to_ascii_lowercase)
}

/// Whether an element is a link — `:link` and `:any-link`.
///
/// `:link` and `:any-link` are the same thing here, because `:visited` never
/// matches: see [`crate::selector::PseudoClass::Visited`].
pub fn is_link(element: &Element) -> bool {
    (element.name.is_html("a") || element.name.is_html("area")) && element.attr("href").is_some()
}

/// Whether an element is one that a `disabled` attribute applies to at all.
pub fn can_be_disabled(element: &Element) -> bool {
    is_one_of(element, CAN_BE_DISABLED)
}

/// Whether an element matches `:disabled`.
///
/// A control is disabled if it says so, or if it is inside a disabled
/// `<fieldset>` — except inside that fieldset's **first** `<legend>`, which
/// stays usable so that a person can still read and operate the group's own
/// controls. An `<option>` is also disabled when its `<optgroup>` is.
pub fn is_disabled(document: &Document, id: NodeId, element: &Element) -> bool {
    if !can_be_disabled(element) {
        return false;
    }
    if element.attr("disabled").is_some() {
        return true;
    }
    if element.name.is_html("option") {
        return document
            .parent(id)
            .and_then(|parent| document.element(parent))
            .is_some_and(|parent| {
                parent.name.is_html("optgroup") && parent.attr("disabled").is_some()
            });
    }
    if !is_one_of(element, DISABLED_BY_FIELDSET) {
        return false;
    }
    disabling_fieldset(document, id).is_some()
}

/// The nearest ancestor `<fieldset>` that disables this element, if there is
/// one.
fn disabling_fieldset(document: &Document, id: NodeId) -> Option<NodeId> {
    let mut child = id;
    let mut ancestor = document.parent(id);
    while let Some(current) = ancestor {
        if let Some(element) = document.element(current)
            && element.name.is_html("fieldset")
            && element.attr("disabled").is_some()
            && !is_inside_first_legend(document, current, child)
        {
            return Some(current);
        }
        child = current;
        ancestor = document.parent(current);
    }
    None
}

/// Whether the path from a fieldset down to a descendant passes through the
/// fieldset's first `<legend>` child.
fn is_inside_first_legend(
    document: &Document,
    fieldset: NodeId,
    child_of_fieldset: NodeId,
) -> bool {
    document
        .children(fieldset)
        .find(|candidate| {
            document
                .element(*candidate)
                .is_some_and(|element| element.name.is_html("legend"))
        })
        .is_some_and(|first_legend| first_legend == child_of_fieldset)
}

/// Whether an element matches `:enabled`: it is a thing that could be
/// disabled, and is not.
pub fn is_enabled(document: &Document, id: NodeId, element: &Element) -> bool {
    can_be_disabled(element) && !is_disabled(document, id, element)
}

/// Whether an element matches `:checked`.
///
/// With no scripting, checkedness is what the markup says: the `checked`
/// attribute on a checkbox or radio, and `selected` on an `<option>`.
pub fn is_checked(element: &Element) -> bool {
    if element.name.is_html("option") {
        return element.attr("selected").is_some();
    }
    if !element.name.is_html("input") {
        return false;
    }
    let kind = input_type(element);
    (kind == "checkbox" || kind == "radio") && element.attr("checked").is_some()
}

/// Whether a `required` attribute means anything on this element.
fn can_be_required(element: &Element) -> bool {
    if !is_one_of(element, CAN_BE_REQUIRED) {
        return false;
    }
    if element.name.is_html("input") {
        let kind = input_type(element);
        return !NEVER_REQUIRED.contains(&&*kind);
    }
    true
}

/// Whether an element matches `:required`.
pub fn is_required(element: &Element) -> bool {
    can_be_required(element) && element.attr("required").is_some()
}

/// Whether an element matches `:optional`: it could have been required, and
/// is not.
pub fn is_optional(element: &Element) -> bool {
    can_be_required(element) && element.attr("required").is_none()
}

/// Whether an element matches `:read-write`.
///
/// A text entry that is neither `readonly` nor disabled, or anything a person
/// may edit because `contenteditable` is in force on it or on an ancestor.
/// `:read-only` is everything else, which is what HTML says and is why there
/// is only one function here.
pub fn is_read_write(document: &Document, id: NodeId, element: &Element) -> bool {
    if is_text_entry(element) {
        return element.attr("readonly").is_none() && !is_disabled(document, id, element);
    }
    is_editable(document, id)
}

/// Whether an element is one a person types into: a `<textarea>`, or an
/// `<input>` of a type that takes text.
///
/// `readonly` means nothing on a checkbox, and `:read-only` matching every
/// checkbox in a document is the bug this predicate exists to prevent. It is
/// public so that the box tree asks the same question rather than asking its
/// own — two implementations of "is this a field" would eventually disagree.
pub fn is_text_entry(element: &Element) -> bool {
    element.name.is_html("textarea")
        || (element.name.is_html("input") && TEXT_ENTRY_TYPES.contains(&&*input_type(element)))
}

/// Whether `contenteditable` is in force on this element.
///
/// The attribute inherits: `contenteditable="false"` on a descendant turns it
/// off again, so the nearest ancestor that says anything is the one that
/// decides.
fn is_editable(document: &Document, id: NodeId) -> bool {
    let mut current = Some(id);
    while let Some(node) = current {
        if let Some(value) = document
            .element(node)
            .and_then(|element| element.attr("contenteditable"))
        {
            if value.is_empty() || value.eq_ignore_ascii_case("true") {
                return true;
            }
            if value.eq_ignore_ascii_case("false") {
                return false;
            }
        }
        current = document.parent(node);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// The element with this id attribute, and the document it is in.
    fn find(html: &str, wanted: &str) -> (Document, NodeId) {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })
            .unwrap_or_else(|| panic!("no element with id={wanted}"));
        (document, id)
    }

    fn disabled(html: &str, wanted: &str) -> bool {
        let (document, id) = find(html, wanted);
        let element = document.element(id).expect("an element");
        is_disabled(&document, id, element)
    }

    #[test]
    fn a_link_is_an_anchor_with_somewhere_to_go() {
        let (document, id) = find("<a id=x href='/'>go</a>", "x");
        assert!(is_link(document.element(id).expect("an element")));

        let (document, id) = find("<a id=x>nowhere</a>", "x");
        assert!(!is_link(document.element(id).expect("an element")));

        let (document, id) = find("<span id=x>text</span>", "x");
        assert!(!is_link(document.element(id).expect("an element")));
    }

    #[test]
    fn the_disabled_attribute_disables_what_it_is_written_on() {
        assert!(disabled("<input id=x disabled>", "x"));
        assert!(!disabled("<input id=x>", "x"));
        assert!(disabled("<button id=x disabled>b</button>", "x"));
        assert!(
            !disabled("<span id=x disabled>s</span>", "x"),
            "a span is not a thing that can be disabled",
        );
    }

    #[test]
    fn a_disabled_fieldset_disables_the_controls_inside_it() {
        let html = "<fieldset disabled><input id=inner></fieldset>";
        assert!(disabled(html, "inner"));

        let nested = "<fieldset disabled><div><select id=deep></select></div></fieldset>";
        assert!(disabled(nested, "deep"));

        let enabled = "<fieldset><input id=inner></fieldset>";
        assert!(!disabled(enabled, "inner"));
    }

    #[test]
    fn the_first_legend_of_a_disabled_fieldset_stays_usable() {
        let html = "<fieldset disabled>\
             <legend><input id=first></legend>\
             <legend><input id=second></legend>\
             <input id=body>\
             </fieldset>";
        assert!(!disabled(html, "first"), "the first legend is exempt");
        assert!(disabled(html, "second"), "a second legend is not");
        assert!(disabled(html, "body"));
    }

    #[test]
    fn an_option_is_disabled_by_its_optgroup() {
        let html = "<select><optgroup disabled><option id=x>a</option></optgroup></select>";
        assert!(disabled(html, "x"));

        let free = "<select><optgroup><option id=x>a</option></optgroup></select>";
        assert!(!disabled(free, "x"));
    }

    #[test]
    fn enabled_is_the_complement_but_only_for_things_that_could_be_disabled() {
        let (document, id) = find("<input id=x>", "x");
        let element = document.element(id).expect("an element");
        assert!(is_enabled(&document, id, element));

        let (document, id) = find("<input id=x disabled>", "x");
        let element = document.element(id).expect("an element");
        assert!(!is_enabled(&document, id, element));

        let (document, id) = find("<span id=x>s</span>", "x");
        let element = document.element(id).expect("an element");
        assert!(
            !is_enabled(&document, id, element),
            "a span is neither enabled nor disabled",
        );
    }

    #[test]
    fn checkedness_is_what_the_markup_says() {
        let cases = [
            ("<input id=x type=checkbox checked>", true),
            ("<input id=x type=checkbox>", false),
            ("<input id=x type=radio checked>", true),
            ("<input id=x type=text checked>", false),
            ("<select><option id=x selected>a</option></select>", true),
            ("<select><option id=x>a</option></select>", false),
        ];
        for (html, expected) in cases {
            let (document, id) = find(html, "x");
            let element = document.element(id).expect("an element");
            assert_eq!(is_checked(element), expected, "{html}");
        }
    }

    #[test]
    fn required_applies_only_where_it_could_mean_something() {
        let cases = [
            ("<input id=x required>", true, false),
            ("<input id=x>", false, true),
            ("<input id=x type=range required>", false, false),
            ("<textarea id=x required></textarea>", true, false),
            ("<select id=x></select>", false, true),
            ("<div id=x required>d</div>", false, false),
        ];
        for (html, required, optional) in cases {
            let (document, id) = find(html, "x");
            let element = document.element(id).expect("an element");
            assert_eq!(is_required(element), required, "{html} :required");
            assert_eq!(is_optional(element), optional, "{html} :optional");
        }
    }

    #[test]
    fn read_write_is_a_text_entry_a_person_can_actually_type_into() {
        let cases = [
            ("<input id=x>", true),
            ("<input id=x readonly>", false),
            ("<input id=x disabled>", false),
            ("<input id=x type=checkbox>", false),
            ("<textarea id=x></textarea>", true),
            ("<textarea id=x readonly></textarea>", false),
            ("<p id=x>text</p>", false),
        ];
        for (html, expected) in cases {
            let (document, id) = find(html, "x");
            let element = document.element(id).expect("an element");
            assert_eq!(is_read_write(&document, id, element), expected, "{html}");
        }
    }

    #[test]
    fn contenteditable_makes_a_subtree_read_write_until_it_is_turned_off() {
        let html = "<div contenteditable><p id=inside>a</p>\
             <div contenteditable=false><p id=off>b</p></div></div>";
        let (document, id) = find(html, "inside");
        assert!(is_read_write(
            &document,
            id,
            document.element(id).expect("e")
        ));

        let (document, id) = find(html, "off");
        assert!(!is_read_write(
            &document,
            id,
            document.element(id).expect("e")
        ));

        let (document, id) = find(
            "<div contenteditable=true><p id=inside>a</p></div>",
            "inside",
        );
        assert!(is_read_write(
            &document,
            id,
            document.element(id).expect("e")
        ));
    }
}

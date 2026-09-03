//! What state a box is in.
//!
//! The other half of ADR 0002's "invoice list, twelve rows, row three
//! selected". The role says what a box is; this says what is true of it right
//! now, and it comes from the same place: something the author declared.
//!
//! Two sources, and `aria-*` wins because it is the more explicit of the two.
//! A `<input disabled>` is disabled because HTML says so; a
//! `<div role=checkbox aria-checked=true>` is checked because the author said
//! so. Both are declarations. Neither is a guess about how the box looks.
//!
//! The HTML half is not re-derived here: [`alo_css::state`] already works it
//! out for selector matching, and two implementations of "is this disabled"
//! would eventually disagree — which is the same argument ADR 0002 makes about
//! two trees.

use alo_dom::{Document, Element, NodeId};
use core::fmt;

/// Whether something is checked, and the third answer a checkbox can give.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checked {
    /// Not checked.
    No,
    /// Checked.
    Yes,
    /// Partly — `aria-checked="mixed"`, the state a "select all" box is in
    /// when some of its rows are selected.
    Mixed,
}

impl fmt::Display for Checked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Checked::No => "false",
            Checked::Yes => "true",
            Checked::Mixed => "mixed",
        })
    }
}

/// Which thing in a set is the current one.
///
/// `aria-current` is not a boolean: a nav item can be the current *page*, a
/// wizard step the current *step*, a cell the current *date*. Keeping the word
/// the author used is the difference between an agent knowing which section of
/// Settings is open and knowing only that something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Current {
    /// `aria-current="true"`, and what any word this engine does not know
    /// means — ARIA says an unrecognised value is `true` rather than nothing.
    Yes,
    /// The current page in a set of them.
    Page,
    /// The current step.
    Step,
    /// The current location.
    Location,
    /// The current date.
    Date,
    /// The current time.
    Time,
}

impl fmt::Display for Current {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Current::Yes => "true",
            Current::Page => "page",
            Current::Step => "step",
            Current::Location => "location",
            Current::Date => "date",
            Current::Time => "time",
        })
    }
}

/// What is true of a box right now.
///
/// Every field that can be absent is an [`Option`], and the distinction
/// matters: `expanded: None` is "this is not a thing that opens", which is not
/// the same as `Some(false)`, "this opens and is closed". An agent told the
/// second can act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
// Clippy reads a struct of many `bool`s as a state machine that wants an enum.
// This one is not: these states are independently true, a box can be disabled
// and required and busy at once, and an enum would have to enumerate the
// combinations. A bit set would compile to the same thing and read worse.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags, not a state machine"
)]
pub struct States {
    /// The box cannot be operated.
    pub disabled: bool,
    /// The box is checked, if it is a thing that can be.
    pub checked: Option<Checked>,
    /// The box is selected, if it is a thing that can be.
    pub selected: Option<bool>,
    /// The box is open, if it is a thing that opens.
    pub expanded: Option<bool>,
    /// The box is held down, if it is a thing that presses.
    pub pressed: Option<bool>,
    /// A value is required before the form it is in can be submitted.
    pub required: bool,
    /// The box can be read but not typed into.
    pub read_only: bool,
    /// The box is being updated and what it says may be stale.
    pub busy: bool,
    /// The value in the box is not acceptable.
    pub invalid: bool,
    /// `aria-hidden`: the author asking that this box be read through, even
    /// though it is on screen.
    pub hidden: bool,
    /// A heading's level, one to six.
    pub level: Option<u8>,
    /// This is the current one of its kind, if its kind has a current one.
    pub current: Option<Current>,
    /// Text can be put into this box.
    ///
    /// A **capability**, not a state a person would read out — which is why it
    /// is not in the outline. It exists because the two questions come apart
    /// on exactly one element: ARIA deliberately gives
    /// `<input type=password>` **no role**, so that assistive technology does
    /// not read a password back, and a browser must nonetheless be able to
    /// type into one. Role says what a thing *is*; this says what can be done
    /// to it.
    pub takes_text: bool,
}

impl States {
    /// What is true of an element.
    pub fn of(document: &Document, id: NodeId, element: &Element) -> Self {
        Self {
            disabled: aria_flag(element, "aria-disabled")
                .unwrap_or_else(|| alo_css::state::is_disabled(document, id, element)),
            checked: checked(document, id, element),
            selected: selected(element),
            expanded: aria_flag(element, "aria-expanded").or_else(|| open_flag(element)),
            pressed: aria_flag(element, "aria-pressed"),
            required: aria_flag(element, "aria-required")
                .unwrap_or_else(|| alo_css::state::is_required(element)),
            read_only: aria_flag(element, "aria-readonly").unwrap_or_else(|| {
                // Only of a field a person types into. `readonly` means
                // nothing on a checkbox, and saying every checkbox in a
                // document is read-only would be noise an agent has to ignore.
                alo_css::state::is_text_entry(element)
                    && !alo_css::state::is_read_write(document, id, element)
            }),
            busy: aria_flag(element, "aria-busy").unwrap_or(false),
            invalid: aria_flag(element, "aria-invalid").unwrap_or(false),
            hidden: aria_flag(element, "aria-hidden").unwrap_or(false),
            level: heading_level(element),
            current: current(element),
            takes_text: alo_css::state::is_text_entry(element),
        }
    }

    /// Whether anything at all is true of this box.
    ///
    /// A box in no particular state does not need to be described as being in
    /// none, which keeps the agent tree readable.
    pub fn is_unremarkable(self) -> bool {
        // A capability is not a state to read out, so it is not one that makes
        // a box worth mentioning. Without this, every field on every page
        // would print an empty `[]`.
        States {
            takes_text: false,
            ..self
        } == States::default()
    }
}

impl fmt::Display for States {
    /// The states that are true, as `name=value`, in a fixed order.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut written = 0;
        let mut write_one = |f: &mut fmt::Formatter<'_>, text: &str| -> fmt::Result {
            if written > 0 {
                f.write_str(" ")?;
            }
            written += 1;
            f.write_str(text)
        };
        if self.disabled {
            write_one(f, "disabled")?;
        }
        if let Some(checked) = self.checked {
            write_one(f, &format!("checked={checked}"))?;
        }
        if let Some(selected) = self.selected {
            write_one(f, &format!("selected={selected}"))?;
        }
        if let Some(expanded) = self.expanded {
            write_one(f, &format!("expanded={expanded}"))?;
        }
        if let Some(pressed) = self.pressed {
            write_one(f, &format!("pressed={pressed}"))?;
        }
        if self.required {
            write_one(f, "required")?;
        }
        if self.read_only {
            write_one(f, "read-only")?;
        }
        if self.busy {
            write_one(f, "busy")?;
        }
        if self.invalid {
            write_one(f, "invalid")?;
        }
        if self.hidden {
            write_one(f, "hidden")?;
        }
        if let Some(level) = self.level {
            write_one(f, &format!("level={level}"))?;
        }
        if let Some(current) = self.current {
            write_one(f, &format!("current={current}"))?;
        }
        Ok(())
    }
}

/// An `aria-*` attribute whose value is `true` or `false`.
///
/// Anything else — including an empty value — is not a declaration of
/// anything, so it is [`None`] and the HTML answer stands.
fn aria_flag(element: &Element, name: &str) -> Option<bool> {
    let value = element.attr(name)?;
    if value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn checked(document: &Document, id: NodeId, element: &Element) -> Option<Checked> {
    if let Some(value) = element.attr("aria-checked") {
        if value.eq_ignore_ascii_case("mixed") {
            return Some(Checked::Mixed);
        }
        if value.eq_ignore_ascii_case("true") {
            return Some(Checked::Yes);
        }
        if value.eq_ignore_ascii_case("false") {
            return Some(Checked::No);
        }
    }
    let _ = (document, id);
    if !is_checkable(element) {
        return None;
    }
    Some(if alo_css::state::is_checked(element) {
        Checked::Yes
    } else {
        Checked::No
    })
}

/// Which of `aria-current`'s words the author used.
///
/// `false` is the absence of the state rather than a state of its own, which
/// is what ARIA says and what keeps an agent from being told about every item
/// in a navigation that is not the current one.
fn current(element: &Element) -> Option<Current> {
    let value = element.attr("aria-current")?.trim().to_ascii_lowercase();
    match value.as_str() {
        "" | "false" => None,
        "page" => Some(Current::Page),
        "step" => Some(Current::Step),
        "location" => Some(Current::Location),
        "date" => Some(Current::Date),
        "time" => Some(Current::Time),
        // ARIA: anything it does not recognise means `true`.
        _ => Some(Current::Yes),
    }
}

fn selected(element: &Element) -> Option<bool> {
    if let Some(declared) = aria_flag(element, "aria-selected") {
        return Some(declared);
    }
    if element.name.is_html("option") {
        return Some(element.attr("selected").is_some());
    }
    None
}

/// `<details open>` and `<dialog open>` are the two HTML elements that say
/// outright whether they are open.
fn open_flag(element: &Element) -> Option<bool> {
    if element.name.is_html("details") || element.name.is_html("dialog") {
        return Some(element.attr("open").is_some());
    }
    None
}

fn is_checkable(element: &Element) -> bool {
    if element.name.is_html("option") {
        return false;
    }
    if !element.name.is_html("input") {
        return false;
    }
    let kind = element
        .attr("type")
        .map_or_else(|| "text".to_owned(), str::to_ascii_lowercase);
    kind == "checkbox" || kind == "radio"
}

fn heading_level(element: &Element) -> Option<u8> {
    if let Some(declared) = element
        .attr("aria-level")
        .and_then(|value| value.trim().parse::<u8>().ok())
        && (1..=6).contains(&declared)
    {
        return Some(declared);
    }
    for (name, level) in [
        ("h1", 1),
        ("h2", 2),
        ("h3", 3),
        ("h4", 4),
        ("h5", 5),
        ("h6", 6),
    ] {
        if element.name.is_html(name) {
            return Some(level);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn states_of(html: &str, wanted: &str) -> States {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })
            .unwrap_or_else(|| panic!("no element with id={wanted}"));
        let element = document.element(id).expect("an element");
        States::of(&document, id, element)
    }

    fn described(html: &str, wanted: &str) -> String {
        states_of(html, wanted).to_string()
    }

    #[test]
    fn a_box_in_no_particular_state_says_nothing() {
        assert!(states_of("<div id=x>t</div>", "x").is_unremarkable());
        assert_eq!(described("<div id=x>t</div>", "x"), "");
    }

    #[test]
    fn html_state_is_read_without_the_author_repeating_it() {
        assert!(states_of("<input id=x disabled>", "x").disabled);
        assert!(states_of("<input id=x required>", "x").required);
        assert_eq!(
            states_of("<input id=x type=checkbox checked>", "x").checked,
            Some(Checked::Yes),
        );
        assert_eq!(
            states_of("<input id=x type=checkbox>", "x").checked,
            Some(Checked::No),
        );
    }

    #[test]
    fn a_box_that_cannot_be_checked_says_nothing_about_checkedness() {
        assert_eq!(states_of("<div id=x>t</div>", "x").checked, None);
        assert_eq!(states_of("<input id=x type=text>", "x").checked, None);
        assert_eq!(
            states_of("<div id=x>t</div>", "x").expanded,
            None,
            "not a thing that opens is not the same as closed",
        );
    }

    #[test]
    fn aria_wins_over_html_because_it_is_the_more_explicit_declaration() {
        assert!(!states_of("<input id=x disabled aria-disabled=false>", "x").disabled);
        assert!(states_of("<div id=x aria-disabled=true>t</div>", "x").disabled);
        assert_eq!(
            states_of("<input id=x type=checkbox aria-checked=mixed>", "x").checked,
            Some(Checked::Mixed),
        );
    }

    #[test]
    fn an_aria_value_that_is_not_true_or_false_declares_nothing() {
        assert!(
            states_of("<input id=x disabled aria-disabled=''>", "x").disabled,
            "an empty value leaves the HTML answer standing",
        );
        assert!(states_of("<input id=x disabled aria-disabled=maybe>", "x").disabled);
    }

    #[test]
    fn a_disclosure_says_whether_it_is_open() {
        assert_eq!(
            states_of("<details id=x open><summary>s</summary></details>", "x").expanded,
            Some(true),
        );
        assert_eq!(
            states_of("<details id=x><summary>s</summary></details>", "x").expanded,
            Some(false),
        );
        assert_eq!(
            states_of("<div id=x role=button aria-expanded=false>b</div>", "x").expanded,
            Some(false),
        );
    }

    #[test]
    fn an_option_says_whether_it_is_selected() {
        assert_eq!(
            states_of("<select><option id=x selected>a</option></select>", "x").selected,
            Some(true),
        );
        assert_eq!(
            states_of("<select><option id=x>a</option></select>", "x").selected,
            Some(false),
        );
        assert_eq!(
            states_of("<div id=x role=row aria-selected=true>r</div>", "x").selected,
            Some(true),
        );
    }

    #[test]
    fn a_heading_carries_its_level() {
        assert_eq!(states_of("<h1 id=x>t</h1>", "x").level, Some(1));
        assert_eq!(states_of("<h4 id=x>t</h4>", "x").level, Some(4));
        assert_eq!(states_of("<p id=x>t</p>", "x").level, None);
        assert_eq!(
            states_of("<div id=x role=heading aria-level=3>t</div>", "x").level,
            Some(3),
        );
        assert_eq!(
            states_of("<div id=x role=heading aria-level=9>t</div>", "x").level,
            None,
            "a level outside one to six is not a level",
        );
    }

    #[test]
    fn a_field_that_cannot_be_typed_into_is_read_only() {
        assert!(states_of("<input id=x readonly>", "x").read_only);
        assert!(!states_of("<input id=x>", "x").read_only);
        assert!(
            !states_of("<p id=x>t</p>", "x").read_only,
            "a paragraph is not a field that happens to be read only",
        );
    }

    #[test]
    fn every_state_that_is_true_is_described_and_the_others_are_not() {
        assert_eq!(
            described(
                "<input id=x type=checkbox checked disabled required aria-busy=true>",
                "x",
            ),
            "disabled checked=true required busy",
        );
        assert_eq!(described("<h2 id=x>t</h2>", "x"), "level=2");
        assert_eq!(
            described("<div id=x aria-hidden=true aria-invalid=true>t</div>", "x"),
            "invalid hidden",
        );
    }
}

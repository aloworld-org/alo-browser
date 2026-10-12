/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a box's value is one its form would refuse, as the agent tree
//! says it (ADR 0051 § 6).
//!
//! Two sources, as for every other state in [`crate::state`], and the
//! author's `aria-invalid` wins. Without it, a control is `invalid` when it
//! fails one of HTML's constraints, which [`alo_dom::constraint`] already
//! works out for submission: a second answer to "is this valid" would be the
//! one that disagrees with what pressing the button does.
//!
//! # Measured, not read
//!
//! What follows is Chrome 154's accessibility tree, read over the
//! `DevTools` protocol (iteration 301); its source was not read.
//!
//! - **A control reads as invalid from the moment it is, not from the moment
//!   somebody pressed.** The frozen booking page's empty required date field
//!   is `invalid` as it loads, stays so after *See free times* is pressed,
//!   and stops being so once a date is in it. A press changes nothing in the
//!   tree; only the value does.
//! - **Except an empty text field.** A required `text`, `search`, `email`,
//!   `password`, `tel`, `url` or `number` field, or a `<textarea>`, whose only
//!   fault is that nothing is in it is **not** `invalid` — not as it loads,
//!   not after a person typed and erased, not after a press. A required date,
//!   time, checkbox, radio group, `<select>` or file is. A text field failing
//!   anything else (a bad email address) is.
//! - **A control barred from validation is never invalid**: disabled,
//!   read-only, or in a `datalist`. `novalidate` on the form changes nothing,
//!   and a control with no form is judged as one with a form.
//! - **`aria-invalid` wins either way.** `false`, in any case, makes a
//!   failing control read as valid; an empty value is no declaration, and the
//!   control's own answer stands; any other word — `true`, `grammar`,
//!   `spelling`, `banana`, ` false` with its space — reads as `invalid`.
//! - **No message is in the tree.** The field carries no description; the
//!   message is said by the press, which [`alo_dom::constraint`] already
//!   words as Chrome does and the agent is told as `NotSubmitted`.
//!
//! # A constraint this engine does not check
//!
//! A control that could fail a constraint not built yet
//! ([`Problem::Unchecked`]) is neither: saying `invalid` or saying nothing
//! would both be guesses an agent would act on. It reads
//! [`Invalid::Unknown`], which is what pressing it would say too — a refusal
//! by name rather than an answer.

use alo_dom::constraint::{self, Constraint, Problem};
use alo_dom::kind::{self, Kind};
use alo_dom::{Document, Element, NodeId};
use core::fmt;

/// What the tree says of a value that is in question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// Its form would refuse it, or its author says it is invalid.
    Yes,
    /// It could fail a constraint this engine does not check, so nobody here
    /// can say.
    Unknown,
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Invalid::Yes => "invalid",
            Invalid::Unknown => "invalid=unknown",
        })
    }
}

/// Whether the element `id` reads as invalid, and how sure that is.
pub fn of(document: &Document, id: NodeId, element: &Element) -> Option<Invalid> {
    if let Some(declared) = element
        .attr("aria-invalid")
        .filter(|value| !value.is_empty())
    {
        return (!declared.eq_ignore_ascii_case("false")).then_some(Invalid::Yes);
    }
    match constraint::problem(document, id)? {
        Problem::Unchecked { .. } => Some(Invalid::Unknown),
        Problem::Fails {
            constraint: Constraint::ValueMissing,
            ..
        } if is_a_text_field(element) => None,
        Problem::Fails { .. } => Some(Invalid::Yes),
    }
}

/// A field a person types a line or more of text into: the one kind of
/// control an empty value does not make invalid in the tree.
///
/// When `setCustomValidity` is built (queue item 471), an empty text field
/// with a custom error must read as invalid, as Chrome's does: then a
/// missing value is no longer the only thing [`constraint::problem`] can be
/// hiding behind it.
fn is_a_text_field(element: &Element) -> bool {
    element.name.is_html("textarea")
        || (element.name.is_html("input")
            && kind::of(element).is_some_and(|kind| kind.is_typed_text() || kind == Kind::Number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// What each control named by its `id` reads as, on the markup Chrome
    /// 154 was measured on.
    fn read(html: &str, wanted: &str) -> Option<Invalid> {
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
        of(&document, id, element)
    }

    fn each(html: &str, expected: &[(&str, Option<Invalid>)]) {
        for (wanted, answer) in expected {
            assert_eq!(read(html, wanted), *answer, "{wanted}");
        }
    }

    #[test]
    fn a_failing_control_is_invalid_before_anybody_pressed() {
        each(
            "<form><input id=d type=date required><input id=ti type=time required>\
             <input id=mo type=month required><input id=we type=week required>\
             <input id=dl type=datetime-local required>\
             <input id=c type=checkbox required><input id=cc type=checkbox required checked>\
             <input id=r1 type=radio name=r required><input id=r2 type=radio name=r>\
             <select id=s required><option value=''>-</option><option>x</option></select>\
             <select id=sc required><option value=''>-</option><option selected>x</option>\
             </select><select id=sm multiple required><option>x</option></select>\
             <input id=f type=file required><input id=e type=email value=nope>\
             <input id=u type=url value=nope><input id=b type=date value=2026-10-20 required>\
             </form>",
            &[
                ("d", Some(Invalid::Yes)),
                ("ti", Some(Invalid::Yes)),
                ("mo", Some(Invalid::Yes)),
                ("we", Some(Invalid::Yes)),
                ("dl", Some(Invalid::Yes)),
                ("c", Some(Invalid::Yes)),
                ("cc", None),
                ("r1", Some(Invalid::Yes)),
                ("r2", Some(Invalid::Yes)),
                ("s", Some(Invalid::Yes)),
                ("sc", None),
                ("sm", Some(Invalid::Yes)),
                ("f", Some(Invalid::Yes)),
                ("e", Some(Invalid::Yes)),
                ("u", Some(Invalid::Yes)),
                ("b", None),
            ],
        );
    }

    #[test]
    fn an_empty_text_field_is_not_invalid_for_being_empty() {
        each(
            "<form><input id=t required><input id=em type=email required>\
             <input id=pw type=password required><input id=se type=search required>\
             <input id=te type=tel required><input id=ur type=url required>\
             <input id=nu type=number required><textarea id=ta required></textarea>\
             <input id=sp required value=' '><input id=pe required pattern=a></form>\
             <input id=nf required>",
            &[
                ("t", None),
                ("em", None),
                ("pw", None),
                ("se", None),
                ("te", None),
                ("ur", None),
                ("nu", None),
                ("ta", None),
                ("sp", None),
                ("pe", None),
                ("nf", None),
            ],
        );
    }

    #[test]
    fn a_barred_control_is_never_invalid() {
        each(
            "<form><input id=ro type=date required readonly>\
             <input id=di type=date required disabled>\
             <fieldset disabled><input id=fs type=date required></fieldset>\
             <datalist><input id=dl type=email value=nope></datalist>\
             <input id=h type=hidden required><button id=b>go</button></form>\
             <form novalidate><input id=nv type=email value=nope></form>",
            &[
                ("ro", None),
                ("di", None),
                ("fs", None),
                ("dl", None),
                ("h", None),
                ("b", None),
                ("nv", Some(Invalid::Yes)),
            ],
        );
    }

    #[test]
    fn the_authors_aria_invalid_wins_either_way() {
        each(
            "<form><input id=f type=email value=nope aria-invalid=false>\
             <input id=fc type=checkbox required aria-invalid=fAlSe>\
             <input id=sp type=email value=nope aria-invalid=' false'>\
             <input id=em type=email value=nope aria-invalid=''>\
             <input id=ev aria-invalid=''>\
             <input id=t aria-invalid=TRUE><input id=g aria-invalid=grammar>\
             <input id=s aria-invalid=spelling><input id=w aria-invalid=banana>\
             <input id=pt pattern=[0-9] value=a aria-invalid=false></form>\
             <div id=d aria-invalid=true>t</div>",
            &[
                ("f", None),
                ("fc", None),
                ("sp", Some(Invalid::Yes)),
                ("em", Some(Invalid::Yes)),
                ("ev", None),
                ("t", Some(Invalid::Yes)),
                ("g", Some(Invalid::Yes)),
                ("s", Some(Invalid::Yes)),
                ("w", Some(Invalid::Yes)),
                ("pt", None),
                ("d", Some(Invalid::Yes)),
            ],
        );
    }

    #[test]
    fn a_constraint_not_built_is_unknown_rather_than_guessed() {
        each(
            "<form><input id=pt pattern=[0-9]+ value=ab><input id=pe pattern=[0-9]+>\
             <input id=mn minlength=5 value=ab><input id=nm type=number min=5 value=1>\
             <input id=e type=email value='a@é'></form>",
            &[
                ("pt", Some(Invalid::Unknown)),
                ("pe", None),
                ("mn", Some(Invalid::Unknown)),
                ("nm", Some(Invalid::Unknown)),
                ("e", Some(Invalid::Unknown)),
            ],
        );
    }

    #[test]
    fn it_is_written_as_the_outline_writes_it() {
        assert_eq!(Invalid::Yes.to_string(), "invalid");
        assert_eq!(Invalid::Unknown.to_string(), "invalid=unknown");
    }
}

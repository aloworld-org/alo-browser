/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ Acting on the interface, through verbs that never take a coordinate.
//!
//! ADR 0002: *"Acting goes through typed verbs… **No verb takes a
//! coordinate**, because a coordinate is a guess about a layout that may have
//! changed between the reading and the acting."*
//!
//! So there is no function here that accepts a point, and there is no way to
//! write one without changing this file. A verb names what it wants the way a
//! person would — "the Save button", "the row called Invoice 12" — and the
//! engine finds it in the tree it just drew.
//!
//! # Refusing is a result
//!
//! Every verb can be refused, and a refusal says why. Two of them are worth
//! naming:
//!
//! - **Ambiguous.** Two things called the same name is not a reason to pick
//!   one. Acting on the wrong row is worse than acting on none, and the caller
//!   can narrow the request or ask for it by id.
//! - **Disabled.** A control that says it cannot be operated is not operated,
//!   even though nothing physically prevents it. An agent that pressed a
//!   disabled button would be doing something a person cannot.
//!
//! # What a verb does, and what it does not
//!
//! A verb **validates and reports** here rather than changing the document.
//! `Activate` on a link comes back with where it goes; `PutText` comes back
//! with the field and the text. Carrying that into the document is the
//! host's — [`apply`](crate::apply::apply) on a page that never ran script, the page's own
//! listeners on one that has (ADR 0018) — and the outcome is the record of
//! what was asked for, which is the guarantee `alo-os` rests on. A page that
//! runs script may cancel text it is offered, and the host then answers
//! [`Outcome::TextCanceled`] instead.
//!
//! # Whichever renderer drew the page
//!
//! A verb is decided against a [`Reading`](crate::reading::Reading), not
//! against this engine's tree as such: Chrome, behind the same boundary
//! (ADR 0058 § 4), is read into one too, so a refusal is reached by the same
//! rule on a page either renderer drew. Carrying a decision out is the
//! renderer's own — `apply` or the page's listeners here, Chrome's `Input`
//! domain there.
//!
//! # Submitting a form
//!
//! There is no verb for it (ADR 0051 § 6). An agent submits as a keyboard
//! user does, by activating the form's submit button, and the host says
//! what came of it: [`Outcome::Submitted`] with where it asked to go,
//! [`Outcome::NotSubmitted`] with each control that failed and the
//! browser's words for why, or [`Refusal::Unchecked`] and
//! [`Refusal::NotSubmittable`] when what stopped it is something this
//! engine does not build yet.

use crate::reading::{Candidate, Reading, is_called};
use alo_box::{BoxId, KnownRole, Role};
use alo_dom::constraint::Constraint;
use alo_dom::submission::Method;
use core::fmt;

/// What a verb is aimed at.
///
/// Every one of these is a *description*, not a position. That is the whole
/// point: a description survives the page moving, and a point does not.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// What it is called.
    Named(String),
    /// What it is, when there is only one.
    OfRole(Role),
    /// Both, which is how a caller narrows an ambiguous name.
    NamedOfRole {
        /// What it is.
        role: Role,
        /// What it is called.
        name: String,
    },
    /// A node the caller already read, by the id it came back with.
    ///
    /// The only form that does not describe: it *names*, and `alo_box::BoxId`
    /// is allocated once and never reused (ADR 0003), so an id that no longer
    /// names anything says so rather than naming something else.
    Node(BoxId),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Named(name) => write!(f, "{name:?}"),
            Target::OfRole(role) => write!(f, "the {role}"),
            Target::NamedOfRole { role, name } => write!(f, "the {role} {name:?}"),
            Target::Node(id) => write!(f, "{id}"),
        }
    }
}

/// How far to scroll, and which way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollBy {
    /// Down by this many CSS pixels, or up when negative.
    Pixels(f32),
    /// To the very start.
    ToStart,
    /// To the very end.
    ToEnd,
}

impl fmt::Display for ScrollBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScrollBy::Pixels(pixels) => write!(f, "by {pixels}px"),
            ScrollBy::ToStart => f.write_str("to the start"),
            ScrollBy::ToEnd => f.write_str("to the end"),
        }
    }
}

/// Something to do to a thing.
#[derive(Debug, Clone, PartialEq)]
pub enum Verb {
    /// Press it, follow it, choose it — whatever operating it means.
    Activate,
    /// Put text into it, replacing what is there.
    PutText(String),
    /// Scroll it.
    Scroll(ScrollBy),
}

/// What happened, in enough detail to be a record.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// A control was operated.
    Activated {
        /// Which node.
        node: BoxId,
        /// What it is called, as it was when the verb ran.
        name: Option<String>,
    },
    /// A link was followed.
    Followed {
        /// Which node.
        node: BoxId,
        /// Where it goes, as written.
        to: String,
    },
    /// Text was put into a field.
    TextPut {
        /// Which node.
        node: BoxId,
        /// What was put in it.
        text: String,
    },
    /// The page cancelled the `beforeinput` that asked whether this text
    /// may go into a field, and the field is as it was (ADR 0018 § 5).
    ///
    /// Only a page that runs script can answer this way. It is not a
    /// [`Refusal`]: the verb was carried out — the page was asked, as it
    /// is asked when a person types — and the page said no.
    TextCanceled {
        /// Which node.
        node: BoxId,
        /// What the page would not take.
        text: String,
    },
    /// Something was scrolled.
    Scrolled {
        /// Which node.
        node: BoxId,
        /// How far.
        by: ScrollBy,
    },
    /// A submit button was pressed and its form asked to be sent (ADR 0051
    /// § 6). Where the tab then goes is the browser process's to decide.
    Submitted {
        /// The button.
        node: BoxId,
        /// How the form is sent.
        method: Method,
        /// Where to, resolved against the document.
        to: String,
    },
    /// A submit button was pressed and its form was not sent, because these
    /// controls failed its checks.
    ///
    /// Not a [`Refusal`]: the verb was carried out — the button was pressed,
    /// as a person presses it — and the form said no, in the browser's
    /// words.
    NotSubmitted {
        /// The button.
        node: BoxId,
        /// Each control that failed, in tree order.
        invalid: Vec<Invalid>,
    },
}

/// A control that failed a form's checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    /// Its box, or [`None`] for a control nothing draws, which an agent
    /// cannot name but which stopped the form all the same.
    pub control: Option<BoxId>,
    /// Which constraint, or [`None`] when the renderer said why in words
    /// and not which: Chrome's bubble names no constraint, and its words
    /// cannot be taken for one, since a page may write any words it likes
    /// with `setCustomValidity` (ADR 0051, amendment of 25 October 2026).
    pub constraint: Option<Constraint>,
    /// What the browser says about it, word for word as Chrome does.
    pub message: String,
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.control {
            Some(control) => write!(f, "{control}")?,
            None => f.write_str("a control nothing draws")?,
        }
        if let Some(constraint) = self.constraint {
            write!(f, " {constraint}")?;
        }
        write!(f, ": {:?}", self.message)
    }
}

impl Outcome {
    /// The node it happened to.
    pub fn node(&self) -> BoxId {
        match self {
            Outcome::Activated { node, .. }
            | Outcome::Followed { node, .. }
            | Outcome::TextPut { node, .. }
            | Outcome::TextCanceled { node, .. }
            | Outcome::Scrolled { node, .. }
            | Outcome::Submitted { node, .. }
            | Outcome::NotSubmitted { node, .. } => *node,
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Activated { node, name } => match name {
                Some(name) => write!(f, "activated {node} {name:?}"),
                None => write!(f, "activated {node}"),
            },
            Outcome::Followed { node, to } => write!(f, "followed {node} to {to:?}"),
            Outcome::TextPut { node, text } => write!(f, "put {text:?} into {node}"),
            Outcome::TextCanceled { node, text } => {
                write!(f, "the page cancelled putting {text:?} into {node}")
            }
            Outcome::Scrolled { node, by } => write!(f, "scrolled {node} {by}"),
            Outcome::Submitted { node, method, to } => {
                write!(
                    f,
                    "pressed {node}, and its form asked to {} {to:?}",
                    method.name()
                )
            }
            Outcome::NotSubmitted { node, invalid } => {
                write!(f, "pressed {node}, and its form was not sent:")?;
                for (at, one) in invalid.iter().enumerate() {
                    let joint = if at == 0 { " " } else { "; " };
                    write!(f, "{joint}{one}")?;
                }
                Ok(())
            }
        }
    }
}

/// Why a verb was not carried out.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Nothing on the page matches.
    NotFound {
        /// What was asked for.
        target: Target,
    },
    /// Several things match, and choosing one would be a guess.
    Ambiguous {
        /// What was asked for.
        target: Target,
        /// Everything that matched, so the caller can narrow it.
        candidates: Vec<BoxId>,
    },
    /// It is not a thing that can be operated.
    NotOperable {
        /// Which node.
        node: BoxId,
        /// What it is.
        role: Role,
    },
    /// It says it cannot be operated.
    Disabled {
        /// Which node.
        node: BoxId,
    },
    /// It is not a thing text goes into.
    NotAField {
        /// Which node.
        node: BoxId,
        /// What it is.
        role: Role,
    },
    /// It is a field, and it cannot be typed into.
    ReadOnly {
        /// Which node.
        node: BoxId,
    },
    /// It is a date field, and the text is not a date it can hold.
    ///
    /// Refused by name rather than put in: HTML would sanitise it to the
    /// empty string, and an agent whose text the field silently emptied
    /// would believe it had chosen a day (ADR 0042 § 4). A person typing it
    /// is stopped by the field's segments; this is the same stop.
    NotADate {
        /// Which node.
        node: BoxId,
        /// The text that is not a date.
        text: String,
    },
    /// It is a field of a kind this browser cannot put text into yet: on
    /// this engine's page a `time`, `month`, `week` or `datetime-local`
    /// field, whose text nothing draws; on Chrome's, those and a `date`
    /// field and a `<select>`, which typing's text does not reach (queue
    /// item 621).
    ///
    /// Refused by name rather than put in: an agent told it had put a time
    /// in would believe a person could see it.
    Unbuilt {
        /// Which node.
        node: BoxId,
        /// The field's type, as HTML names it.
        kind: String,
    },
    /// It has no more content than room, so there is nothing to scroll.
    DoesNotScroll {
        /// Which node.
        node: BoxId,
    },
    /// The text is longer than the field's `maxlength`.
    ///
    /// Refused rather than cut short: a person cannot type past it, and an
    /// agent told its text went in when part of it did not would hold a
    /// false record (ADR 0051 § 3).
    TooLong {
        /// Which node.
        node: BoxId,
        /// The most the field takes, in UTF-16 code units as HTML counts.
        most: usize,
        /// How long the text is, counted the same way.
        length: usize,
    },
    /// The button's form was not sent: a control in it could fail a
    /// constraint this engine does not check yet (ADR 0051 § 3).
    Unchecked {
        /// The button.
        node: BoxId,
        /// The control, or [`None`] for one nothing draws.
        control: Option<BoxId>,
        /// Which constraint.
        constraint: Constraint,
    },
    /// The button's form was not sent, for a reason this engine does not
    /// build yet, in words (ADR 0051 § 4).
    NotSubmittable {
        /// The button.
        node: BoxId,
        /// Why.
        why: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotFound { target } => write!(f, "nothing on the page is {target}"),
            Refusal::Ambiguous { target, candidates } => write!(
                f,
                "{} things are {target}; ask for one of {candidates:?} instead",
                candidates.len(),
            ),
            Refusal::NotOperable { node, role } => {
                write!(f, "{node} is a {role}, which is not a thing to operate")
            }
            Refusal::Disabled { node } => write!(f, "{node} says it is disabled"),
            Refusal::NotAField { node, role } => {
                write!(f, "{node} is a {role}, which is not a thing text goes into")
            }
            Refusal::ReadOnly { node } => write!(f, "{node} cannot be typed into"),
            Refusal::NotADate { node, text } => write!(
                f,
                "{text:?} is not a date {node} can hold; a date is written yyyy-mm-dd",
            ),
            Refusal::Unbuilt { node, kind } => write!(
                f,
                "{node} is a {kind} field, and this browser cannot put text into one yet",
            ),
            Refusal::DoesNotScroll { node } => write!(f, "{node} has nothing to scroll"),
            Refusal::TooLong { node, most, length } => write!(
                f,
                "{node} takes at most {most} characters, and the text is {length} long",
            ),
            Refusal::Unchecked {
                node,
                control,
                constraint,
            } => {
                let control = control.map_or_else(
                    || "a control nothing draws".to_owned(),
                    |control| control.to_string(),
                );
                write!(
                    f,
                    "{node}'s form was not sent: {control} could fail {constraint}, which this \
                     engine does not check yet",
                )
            }
            Refusal::NotSubmittable { node, why } => {
                write!(f, "{node}'s form was not sent: {why}")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// Do something to a thing on the page.
///
/// Decided against `page`, whichever renderer drew it ([`Reading`]): the
/// same refusals, by the same rules, for this engine's tree and for
/// Chrome's.
///
/// # Errors
///
/// A [`Refusal`] saying why not — including when several things match, because
/// acting on the wrong one is worse than acting on none.
pub fn perform<R: Reading + ?Sized>(
    page: &R,
    target: &Target,
    verb: &Verb,
) -> Result<Outcome, Refusal> {
    let node = find(page, target)?;
    match verb {
        Verb::Activate => activate(page, &node),
        Verb::PutText(text) => put_text(page, &node, text),
        Verb::Scroll(by) => scroll(&node, *by),
    }
}

/// The one thing `target` names on `page`, or why it is not one thing.
///
/// # Errors
///
/// [`Refusal::NotFound`] when nothing matches, and [`Refusal::Ambiguous`]
/// when several things do.
pub fn find<R: Reading + ?Sized>(page: &R, target: &Target) -> Result<Candidate, Refusal> {
    let matches: Vec<Candidate> = page
        .candidates()
        .into_iter()
        .filter(|node| match target {
            Target::Named(name) => is_called(node.name.as_deref(), name),
            Target::OfRole(role) => node.role == *role,
            Target::NamedOfRole { role, name } => {
                node.role == *role && is_called(node.name.as_deref(), name)
            }
            Target::Node(id) => node.id == *id,
        })
        .collect();
    match matches.len() {
        0 => Err(Refusal::NotFound {
            target: target.clone(),
        }),
        1 => matches.into_iter().next().ok_or(Refusal::NotFound {
            target: target.clone(),
        }),
        _ => Err(Refusal::Ambiguous {
            target: target.clone(),
            candidates: matches.iter().map(|node| node.id).collect(),
        }),
    }
}

/// The roles that can be operated.
///
/// A list rather than a rule, because "can this be pressed" is a fact about
/// each role. A paragraph cannot, however much it looks like a button.
fn is_operable(role: &Role) -> bool {
    matches!(
        role,
        Role::Known(
            KnownRole::Button
                | KnownRole::Link
                | KnownRole::CheckBox
                | KnownRole::Radio
                | KnownRole::Switch
                | KnownRole::Option
                | KnownRole::Tab
                | KnownRole::MenuItem
                | KnownRole::Summary
                | KnownRole::ListItem
                | KnownRole::Row
                | KnownRole::Cell
        )
    )
}

fn activate<R: Reading + ?Sized>(page: &R, node: &Candidate) -> Result<Outcome, Refusal> {
    let role = node.role.clone();
    if !is_operable(&role) {
        return Err(Refusal::NotOperable {
            node: node.id,
            role,
        });
    }
    if node.states.disabled {
        return Err(Refusal::Disabled { node: node.id });
    }
    // A link goes somewhere, and where it goes is part of what happened.
    if role == Role::Known(KnownRole::Link)
        && let Some(destination) = page.href_of(node.id)
    {
        return Ok(Outcome::Followed {
            node: node.id,
            to: destination,
        });
    }
    Ok(Outcome::Activated {
        node: node.id,
        name: node.name.clone(),
    })
}

/// The roles text goes into.
fn is_a_field(role: &Role) -> bool {
    matches!(
        role,
        Role::Known(
            KnownRole::TextBox | KnownRole::SearchBox | KnownRole::ComboBox | KnownRole::Date
        )
    )
}

fn put_text<R: Reading + ?Sized>(
    page: &R,
    node: &Candidate,
    text: &str,
) -> Result<Outcome, Refusal> {
    let role = node.role.clone();
    let states = node.states;
    // Either the role says it is a field, or the box says text goes into it.
    // The two come apart on `<input type=password>`, which ARIA deliberately
    // gives no role so that a screen reader does not read a password back —
    // and which a browser must nonetheless be able to type into.
    if !is_a_field(&role) && !states.takes_text {
        return Err(Refusal::NotAField {
            node: node.id,
            role,
        });
    }
    if states.disabled {
        return Err(Refusal::Disabled { node: node.id });
    }
    if states.read_only {
        return Err(Refusal::ReadOnly { node: node.id });
    }
    // A date field holds a valid date string or nothing (ADR 0042 § 4).
    // The empty text is nothing, which is how a date is cleared. Asked
    // first: a text that is not a date is the agent's to correct whether or
    // not this browser can put a date in, which on Chrome's page it cannot
    // yet (`Refusal::Unbuilt`).
    if page.is_a_date_field(node.id)
        && !text.is_empty()
        && !alo_dom::date::is_valid_date_string(text)
    {
        return Err(Refusal::NotADate {
            node: node.id,
            text: text.to_owned(),
        });
    }
    // A field nothing draws the text of is not told it took some.
    if let Some(kind) = page.unbuilt_field(node.id) {
        return Err(Refusal::Unbuilt {
            node: node.id,
            kind: kind.to_owned(),
        });
    }
    // A person cannot type past `maxlength`, and an agent is not let past
    // it either, rather than told it put in text that was cut (ADR 0051 § 3).
    if let Some(most) = page.max_length(node.id) {
        let length = length_in_a_field(text);
        if length > most {
            return Err(Refusal::TooLong {
                node: node.id,
                most,
                length,
            });
        }
    }
    Ok(Outcome::TextPut {
        node: node.id,
        text: text.to_owned(),
    })
}

/// How long `text` is as `maxlength` counts it: in UTF-16 code units, a
/// line break of CR LF or a lone CR counting as one, as a field holds it.
fn length_in_a_field(text: &str) -> usize {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .encode_utf16()
        .count()
}

fn scroll(node: &Candidate, by: ScrollBy) -> Result<Outcome, Refusal> {
    if !node.scrolls {
        return Err(Refusal::DoesNotScroll { node: node.id });
    }
    Ok(Outcome::Scrolled { node: node.id, by })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::States;

    #[test]
    fn a_target_says_what_it_is_asking_for() {
        assert_eq!(Target::Named("Save".to_owned()).to_string(), "\"Save\"");
        assert_eq!(
            Target::OfRole(Role::Known(KnownRole::Button)).to_string(),
            "the button",
        );
        assert_eq!(
            Target::NamedOfRole {
                role: Role::Known(KnownRole::ListItem),
                name: "Invoice 12".to_owned(),
            }
            .to_string(),
            "the listitem \"Invoice 12\"",
        );
    }

    #[test]
    fn the_things_that_can_be_operated_are_the_things_a_person_operates() {
        assert!(is_operable(&Role::Known(KnownRole::Button)));
        assert!(is_operable(&Role::Known(KnownRole::Link)));
        assert!(is_operable(&Role::Known(KnownRole::ListItem)));
        assert!(!is_operable(&Role::Known(KnownRole::Paragraph)));
        assert!(!is_operable(&Role::Known(KnownRole::Heading)));
        assert!(!is_operable(&Role::Generic));
    }

    #[test]
    fn the_things_text_goes_into_are_fields() {
        assert!(is_a_field(&Role::Known(KnownRole::TextBox)));
        assert!(is_a_field(&Role::Known(KnownRole::SearchBox)));
        assert!(!is_a_field(&Role::Known(KnownRole::Button)));
        assert!(!is_a_field(&Role::Known(KnownRole::CheckBox)));
        assert!(is_a_field(&Role::Known(KnownRole::Date)));
    }

    #[test]
    fn an_outcome_reads_as_a_record_of_what_was_done() {
        let node = BoxId::from_index_for_tests(7);
        assert_eq!(
            Outcome::Activated {
                node,
                name: Some("Save".to_owned()),
            }
            .to_string(),
            "activated box#7 \"Save\"",
        );
        assert_eq!(
            Outcome::Followed {
                node,
                to: "/three".to_owned(),
            }
            .to_string(),
            "followed box#7 to \"/three\"",
        );
        assert_eq!(
            Outcome::TextPut {
                node,
                text: "12.50".to_owned(),
            }
            .to_string(),
            "put \"12.50\" into box#7",
        );
        assert_eq!(
            Outcome::TextCanceled {
                node,
                text: "12.50".to_owned(),
            }
            .to_string(),
            "the page cancelled putting \"12.50\" into box#7",
        );
        assert_eq!(
            Outcome::Scrolled {
                node,
                by: ScrollBy::Pixels(40.0),
            }
            .to_string(),
            "scrolled box#7 by 40px",
        );
        assert_eq!(Outcome::Activated { node, name: None }.node(), node,);
    }

    #[test]
    fn a_refusal_says_why_and_what_to_do_about_it() {
        let target = Target::Named("Invoice".to_owned());
        let candidates = vec![
            BoxId::from_index_for_tests(3),
            BoxId::from_index_for_tests(4),
        ];
        let ambiguous = Refusal::Ambiguous {
            target,
            candidates: candidates.clone(),
        };
        assert!(ambiguous.to_string().contains("2 things"));
        assert!(
            ambiguous.to_string().contains("instead"),
            "and says how to narrow it: {ambiguous}",
        );

        let disabled = Refusal::Disabled {
            node: BoxId::from_index_for_tests(1),
        };
        assert_eq!(disabled.to_string(), "box#1 says it is disabled");

        let unbuilt = Refusal::Unbuilt {
            node: BoxId::from_index_for_tests(4),
            kind: "time".to_owned(),
        };
        assert_eq!(
            unbuilt.to_string(),
            "box#4 is a time field, and this browser cannot put text into one yet",
        );

        let not_a_date = Refusal::NotADate {
            node: BoxId::from_index_for_tests(2),
            text: "12/10/2026".to_owned(),
        };
        assert_eq!(
            not_a_date.to_string(),
            "\"12/10/2026\" is not a date box#2 can hold; a date is written yyyy-mm-dd",
        );
    }

    #[test]
    fn a_submission_reads_as_a_record_of_what_came_of_the_press() {
        let node = BoxId::from_index_for_tests(9);
        assert_eq!(
            Outcome::Submitted {
                node,
                method: Method::Post,
                to: "https://httpbin.org/post".to_owned(),
            }
            .to_string(),
            "pressed box#9, and its form asked to POST \"https://httpbin.org/post\"",
        );
        let invalid = Outcome::NotSubmitted {
            node,
            invalid: vec![
                Invalid {
                    control: Some(BoxId::from_index_for_tests(4)),
                    constraint: Some(Constraint::ValueMissing),
                    message: "Please fill out this field.".to_owned(),
                },
                Invalid {
                    control: None,
                    constraint: Some(Constraint::TypeMismatch),
                    message: "Please enter a URL.".to_owned(),
                },
            ],
        };
        assert_eq!(
            invalid.to_string(),
            "pressed box#9, and its form was not sent: box#4 valueMissing: \
             \"Please fill out this field.\"; a control nothing draws typeMismatch: \
             \"Please enter a URL.\"",
        );
        // Through Chrome, whose bubble gives the words and not the
        // constraint.
        let unnamed = Outcome::NotSubmitted {
            node,
            invalid: vec![Invalid {
                control: Some(BoxId::from_index_for_tests(4)),
                constraint: None,
                message: "Fill it in, please".to_owned(),
            }],
        };
        assert_eq!(
            unnamed.to_string(),
            "pressed box#9, and its form was not sent: box#4: \"Fill it in, please\"",
        );
        assert_eq!(invalid.node(), node);
        assert_eq!(
            Refusal::TooLong {
                node,
                most: 3,
                length: 6
            }
            .to_string(),
            "box#9 takes at most 3 characters, and the text is 6 long",
        );
        assert_eq!(
            Refusal::Unchecked {
                node,
                control: Some(BoxId::from_index_for_tests(2)),
                constraint: Constraint::PatternMismatch,
            }
            .to_string(),
            "box#9's form was not sent: box#2 could fail patternMismatch, which this engine \
             does not check yet",
        );
    }

    #[test]
    fn a_fields_length_is_counted_as_maxlength_counts_it() {
        assert_eq!(length_in_a_field("abc"), 3);
        assert_eq!(length_in_a_field("a\r\nb"), 3);
        assert_eq!(length_in_a_field("a\rb"), 3);
        assert_eq!(length_in_a_field("é"), 1);
        assert_eq!(length_in_a_field("😀"), 2, "two UTF-16 code units");
    }

    /// A page described rather than drawn, as another renderer's reading
    /// arrives: each node, and the facts about the fields and links in it.
    struct Described {
        nodes: Vec<Candidate>,
        dates: Vec<BoxId>,
        unbuilt: Vec<BoxId>,
        most: Vec<(BoxId, usize)>,
        links: Vec<(BoxId, String)>,
    }

    impl Reading for Described {
        fn candidates(&self) -> Vec<Candidate> {
            self.nodes.clone()
        }
        fn href_of(&self, id: BoxId) -> Option<String> {
            self.links
                .iter()
                .find(|(link, _)| *link == id)
                .map(|(_, to)| to.clone())
        }
        fn is_a_date_field(&self, id: BoxId) -> bool {
            self.dates.contains(&id)
        }
        fn unbuilt_field(&self, id: BoxId) -> Option<&'static str> {
            self.unbuilt.contains(&id).then_some("time")
        }
        fn max_length(&self, id: BoxId) -> Option<usize> {
            self.most
                .iter()
                .find(|(field, _)| *field == id)
                .map(|(_, most)| *most)
        }
    }

    fn described() -> Described {
        let id = BoxId::from_index_for_tests;
        let node = |at: usize, role: KnownRole, name: &str, states: States| Candidate {
            id: id(at),
            role: Role::Known(role),
            name: Some(name.to_owned()),
            states,
            scrolls: at == 9,
        };
        let field = States {
            takes_text: true,
            ..States::default()
        };
        Described {
            nodes: vec![
                node(1, KnownRole::Button, "Twice", States::default()),
                node(2, KnownRole::Button, "Twice", States::default()),
                node(3, KnownRole::Paragraph, "Words", States::default()),
                node(
                    4,
                    KnownRole::Button,
                    "Off",
                    States {
                        disabled: true,
                        ..States::default()
                    },
                ),
                node(
                    5,
                    KnownRole::TextBox,
                    "Fixed",
                    States {
                        read_only: true,
                        ..field
                    },
                ),
                node(6, KnownRole::Date, "Day", field),
                node(7, KnownRole::TextBox, "When", field),
                node(8, KnownRole::TextBox, "Short", field),
                node(9, KnownRole::Region, "Long", States::default()),
                node(10, KnownRole::Link, "Next", States::default()),
            ],
            dates: vec![id(6)],
            unbuilt: vec![id(7)],
            most: vec![(id(8), 3)],
            links: vec![(id(10), "/next".to_owned())],
        }
    }

    #[test]
    fn every_refusal_is_reached_on_a_page_another_renderer_described() {
        let page = described();
        let named = |name: &str| Target::Named(name.to_owned());
        let put = |text: &str| Verb::PutText(text.to_owned());
        let kind = |target: Target, verb: Verb| match perform(&page, &target, &verb) {
            Ok(outcome) => format!("done: {outcome}"),
            Err(refusal) => format!("{refusal:?}")
                .split([' ', '{'])
                .next()
                .unwrap_or_default()
                .to_owned(),
        };
        assert_eq!(kind(named("Nothing"), Verb::Activate), "NotFound");
        assert_eq!(kind(named("twice"), Verb::Activate), "Ambiguous");
        assert_eq!(kind(named("Words"), Verb::Activate), "NotOperable");
        assert_eq!(kind(named("Off"), Verb::Activate), "Disabled");
        assert_eq!(kind(named("Words"), put("x")), "NotAField");
        assert_eq!(kind(named("Fixed"), put("x")), "ReadOnly");
        assert_eq!(kind(named("Day"), put("12/10/2026")), "NotADate");
        assert_eq!(kind(named("When"), put("12:00")), "Unbuilt");
        assert_eq!(kind(named("Short"), put("four")), "TooLong");
        assert_eq!(
            kind(named("Words"), Verb::Scroll(ScrollBy::ToEnd)),
            "DoesNotScroll"
        );
        // And what is not refused is decided as it is on this engine's page.
        assert_eq!(
            kind(named("Next"), Verb::Activate),
            "done: followed box#10 to \"/next\""
        );
        assert_eq!(
            kind(named("Day"), put("2026-10-12")),
            "done: put \"2026-10-12\" into box#6"
        );
        assert_eq!(
            kind(named("Short"), put("abc")),
            "done: put \"abc\" into box#8"
        );
        assert_eq!(
            kind(named("Long"), Verb::Scroll(ScrollBy::ToEnd)),
            "done: scrolled box#9 to the end"
        );
        assert_eq!(
            kind(
                Target::NamedOfRole {
                    role: Role::Known(KnownRole::Button),
                    name: "Twice".to_owned(),
                },
                Verb::Activate
            ),
            "Ambiguous"
        );
        assert_eq!(
            kind(Target::Node(BoxId::from_index_for_tests(2)), Verb::Activate),
            "done: activated box#2 \"Twice\""
        );
    }

    #[test]
    fn how_far_to_scroll_reads_as_a_direction() {
        assert_eq!(ScrollBy::Pixels(-20.0).to_string(), "by -20px");
        assert_eq!(ScrollBy::ToStart.to_string(), "to the start");
        assert_eq!(ScrollBy::ToEnd.to_string(), "to the end");
    }
}

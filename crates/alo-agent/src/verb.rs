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
//! Stage 1 has no scripting and no mutation — `docs/features.md` puts both in
//! stage 2 — so a verb **validates and reports** rather than changing the
//! document. `Activate` on a link comes back with where it goes; `PutText`
//! comes back with the field and the text. Applying that is the host's, and
//! the outcome is the record of what was asked for, which is the guarantee
//! `alo-os` rests on.

use crate::tree::{AgentNode, AgentTree};
use alo_box::{BoxId, KnownRole, Role};
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
    /// Something was scrolled.
    Scrolled {
        /// Which node.
        node: BoxId,
        /// How far.
        by: ScrollBy,
    },
}

impl Outcome {
    /// The node it happened to.
    pub fn node(&self) -> BoxId {
        match self {
            Outcome::Activated { node, .. }
            | Outcome::Followed { node, .. }
            | Outcome::TextPut { node, .. }
            | Outcome::Scrolled { node, .. } => *node,
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
            Outcome::Scrolled { node, by } => write!(f, "scrolled {node} {by}"),
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
    /// It has no more content than room, so there is nothing to scroll.
    DoesNotScroll {
        /// Which node.
        node: BoxId,
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
            Refusal::DoesNotScroll { node } => write!(f, "{node} has nothing to scroll"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Do something to a thing on the page.
///
/// # Errors
///
/// A [`Refusal`] saying why not — including when several things match, because
/// acting on the wrong one is worse than acting on none.
pub fn perform(tree: &AgentTree<'_>, target: &Target, verb: &Verb) -> Result<Outcome, Refusal> {
    let node = find(tree, target)?;
    match verb {
        Verb::Activate => activate(tree, &node),
        Verb::PutText(text) => put_text(&node, text),
        Verb::Scroll(by) => scroll(&node, *by),
    }
}

/// The one thing a target names, or why it is not one thing.
fn find<'a>(tree: &AgentTree<'a>, target: &Target) -> Result<AgentNode<'a>, Refusal> {
    let matches: Vec<AgentNode<'a>> = match target {
        Target::Named(name) => tree.named(name),
        Target::OfRole(role) => tree.with_role(role),
        Target::NamedOfRole { role, name } => tree
            .named(name)
            .into_iter()
            .filter(|node| node.role() == *role)
            .collect(),
        Target::Node(id) => tree
            .nodes()
            .into_iter()
            .filter(|node| node.id() == *id)
            .collect(),
    };
    match matches.len() {
        0 => Err(Refusal::NotFound {
            target: target.clone(),
        }),
        1 => matches.into_iter().next().ok_or(Refusal::NotFound {
            target: target.clone(),
        }),
        _ => Err(Refusal::Ambiguous {
            target: target.clone(),
            candidates: matches.iter().map(AgentNode::id).collect(),
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

fn activate(tree: &AgentTree<'_>, node: &AgentNode<'_>) -> Result<Outcome, Refusal> {
    let role = node.role();
    if !is_operable(&role) {
        return Err(Refusal::NotOperable {
            node: node.id(),
            role,
        });
    }
    if node.states().disabled {
        return Err(Refusal::Disabled { node: node.id() });
    }
    // A link goes somewhere, and where it goes is part of what happened.
    if role == Role::Known(KnownRole::Link)
        && let Some(destination) = tree.href_of(node.id())
    {
        return Ok(Outcome::Followed {
            node: node.id(),
            to: destination,
        });
    }
    Ok(Outcome::Activated {
        node: node.id(),
        name: node.name(),
    })
}

/// The roles text goes into.
fn is_a_field(role: &Role) -> bool {
    matches!(
        role,
        Role::Known(KnownRole::TextBox | KnownRole::SearchBox | KnownRole::ComboBox)
    )
}

fn put_text(node: &AgentNode<'_>, text: &str) -> Result<Outcome, Refusal> {
    let role = node.role();
    let states = node.states();
    // Either the role says it is a field, or the box says text goes into it.
    // The two come apart on `<input type=password>`, which ARIA deliberately
    // gives no role so that a screen reader does not read a password back —
    // and which a browser must nonetheless be able to type into.
    if !is_a_field(&role) && !states.takes_text {
        return Err(Refusal::NotAField {
            node: node.id(),
            role,
        });
    }
    if states.disabled {
        return Err(Refusal::Disabled { node: node.id() });
    }
    if states.read_only {
        return Err(Refusal::ReadOnly { node: node.id() });
    }
    Ok(Outcome::TextPut {
        node: node.id(),
        text: text.to_owned(),
    })
}

fn scroll(node: &AgentNode<'_>, by: ScrollBy) -> Result<Outcome, Refusal> {
    if !node.scrolls() {
        return Err(Refusal::DoesNotScroll { node: node.id() });
    }
    Ok(Outcome::Scrolled {
        node: node.id(),
        by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn how_far_to_scroll_reads_as_a_direction() {
        assert_eq!(ScrollBy::Pixels(-20.0).to_string(), "by -20px");
        assert_eq!(ScrollBy::ToStart.to_string(), "to the start");
        assert_eq!(ScrollBy::ToEnd.to_string(), "to the end");
    }
}

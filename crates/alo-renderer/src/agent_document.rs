/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent at work, built into the card's document under its page (queue
//! item 704, ADR 0096, `docs/design/shell.md`, *The agent at work*), the
//! way the rest of the first screen is built ([`crate::start_document`]):
//! every element made with `alo-dom`'s operations, every stranger's string
//! — what the person asked, what the model wrote of a step — a text node.
//!
//! # What is drawn
//!
//! Under the page, inside the card, a column the card's width:
//!
//! - While it works, a terracotta line — [`WORKING`] — with *Stop* at its
//!   end, and the page outlined in terracotta ([`ring`]): the one place the
//!   acting colour is used (ADR 0074 § 2).
//! - What the person asked, in the ink.
//! - One line per step, oldest first, in a person's words, with why under
//!   it when it chose; the step under way, while it works, in terracotta.
//! - What it is doing now: asking the person — the question, with *Allow*
//!   and *Not now* — stuck, with why and its choices as buttons, the closest
//!   to what was asked first; done, *Done in N steps* and the result in
//!   full; or stopped, and why.
//!
//! # What an agent reads
//!
//! ADR 0002: a `region` named [`AT_WORK`] holding the working line as a
//! paragraph and *Stop* as a button; what was asked as a paragraph; a list
//! named [`STEPS`] of each step as a list item; the question as a `group`
//! named [`ASKS`] holding it and *Allow* and *Not now* as buttons; a stuck
//! agent's why as a paragraph and each choice as a button named by its
//! words; *Done in N steps* and the result as paragraphs. The outline over
//! the page is decoration, `aria-hidden`.
//!
//! # What a press is on
//!
//! *Stop*, *Allow*, *Not now* are each one of a kind, known by their class
//! (`abtn stop`, `abtn allow`, `abtn notnow`), and a choice by its place
//! among the choices drawn (`achoice`): claimed by the renderer, decided by
//! the browser process against what it last painted
//! ([`crate::conversation_decide`]) — so a press on *Allow* allows only the
//! question the person was shown.

use crate::agent_work::{AtWork, Now, WORKING};
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the agent at work is named, to an agent reading the screen.
pub const AT_WORK: &str = "alo at work";

/// What its list of steps is named.
pub const STEPS: &str = "Steps";

/// What the question it asks before acting is named.
pub const ASKS: &str = "alo asks you first";

/// The words of the button that lets it do what it asked.
pub const ALLOW: &str = "Allow";

/// The words of the button that refuses it.
pub const NOT_NOW: &str = "Not now";

/// The words of the button that stops it.
pub const STOP: &str = "Stop";

/// The column for `work`, made for the card to put under its page.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no agent's words
/// can cause.
pub fn make(document: &mut Document, work: &AtWork) -> Result<NodeId, Unbuilt> {
    let column = element(document, "div")?;
    attribute(document, column, "class", "agent")?;
    attribute(document, column, "role", "region")?;
    attribute(document, column, "aria-label", AT_WORK)?;
    if work.is_working() {
        let row = element(document, "div")?;
        attribute(document, row, "class", "working")?;
        let line = words(document, "p", "said", WORKING)?;
        append(document, row, line)?;
        let stop = button(document, "abtn stop", STOP)?;
        append(document, row, stop)?;
        append(document, column, row)?;
    }
    let asked = words(document, "p", "asked", &work.asked)?;
    append(document, column, asked)?;
    if !work.steps.is_empty() {
        let list = element(document, "ol")?;
        attribute(document, list, "class", "steps")?;
        attribute(document, list, "aria-label", STEPS)?;
        let newest = work.steps.len() - 1;
        for (place, step) in work.steps.iter().enumerate() {
            let item = element(document, "li")?;
            let under_way = place == newest && matches!(work.now, Now::Acting);
            attribute(
                document,
                item,
                "class",
                if under_way { "step now" } else { "step" },
            )?;
            let line = words(document, "span", "line", &step.line)?;
            append(document, item, line)?;
            if !step.why.trim().is_empty() {
                let why = words(document, "span", "why", &step.why)?;
                append(document, item, why)?;
            }
            append(document, list, item)?;
        }
        append(document, column, list)?;
    }
    if let Some(now) = now(document, work)? {
        append(document, column, now)?;
    }
    Ok(column)
}

/// The outline drawn over the page while the agent works: decoration over
/// the page box, never a thing a press or an agent finds.
///
/// # Errors
///
/// As [`make`].
pub fn ring(document: &mut Document) -> Result<NodeId, Unbuilt> {
    let ring = element(document, "div")?;
    attribute(document, ring, "class", "ring")?;
    attribute(document, ring, "aria-hidden", "true")?;
    Ok(ring)
}

/// What it is doing now, if anything is drawn for it: the question and its
/// two buttons, being stuck and its choices, done, or stopped.
fn now(document: &mut Document, work: &AtWork) -> Result<Option<NodeId>, Unbuilt> {
    let made = match &work.now {
        Now::Acting => return Ok(None),
        Now::Asking { what } => {
            let asks = element(document, "div")?;
            attribute(document, asks, "class", "asks")?;
            attribute(document, asks, "role", "group")?;
            attribute(document, asks, "aria-label", ASKS)?;
            let question = words(document, "p", "question", what)?;
            append(document, asks, question)?;
            let row = element(document, "div")?;
            attribute(document, row, "class", "answers")?;
            for (class, said) in [("abtn allow", ALLOW), ("abtn notnow", NOT_NOW)] {
                let pressed = button(document, class, said)?;
                append(document, row, pressed)?;
            }
            append(document, asks, row)?;
            asks
        }
        Now::Stuck { why, choices } => {
            let stuck = element(document, "div")?;
            attribute(document, stuck, "class", "stuck")?;
            let said = words(document, "p", "because", why)?;
            append(document, stuck, said)?;
            for choice in choices {
                let pressed = button(document, "achoice", choice)?;
                append(document, stuck, pressed)?;
            }
            stuck
        }
        Now::Done { said } => {
            let done = element(document, "div")?;
            attribute(document, done, "class", "finished")?;
            let line = words(document, "p", "done", &work.done_in())?;
            append(document, done, line)?;
            let result = words(document, "p", "result", said)?;
            append(document, done, result)?;
            done
        }
        Now::Stopped { why } => words(document, "p", "stopped", why)?,
    };
    Ok(Some(made))
}

/// An element `name` of `class` holding `said` as a text node.
fn words(document: &mut Document, name: &str, class: &str, said: &str) -> Result<NodeId, Unbuilt> {
    let made = element(document, name)?;
    attribute(document, made, "class", class)?;
    let text = document.create_text_node(said);
    append(document, made, text)?;
    Ok(made)
}

/// A button of `class` named by `said`.
fn button(document: &mut Document, class: &str, said: &str) -> Result<NodeId, Unbuilt> {
    let made = words(document, "button", class, said)?;
    attribute(document, made, "type", "button")?;
    Ok(made)
}

#[cfg(test)]
mod tests {
    use crate::agent_work::{AtWork, Now, Step};
    use crate::card::Card;
    use crate::start::Start;
    use crate::start_document::build;
    use alo_dom::{Document, NodeKind};

    fn built(work: AtWork) -> Document {
        let mut card = Card::new("a page", alo_url::parse("https://example.com/").unwrap());
        card.agent = Some(work);
        build(&Start {
            greeting: "Hello.".to_owned(),
            turns: vec![crate::turn::Turn::Page(card)],
            ..Start::default()
        })
        .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    fn of_class(document: &Document, class: &str) -> Vec<String> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .map(|node| document.text_content(node))
            .collect()
    }

    fn step(line: &str, why: &str) -> Step {
        Step {
            line: line.to_owned(),
            why: why.to_owned(),
        }
    }

    #[test]
    fn while_it_acts_the_page_is_outlined_and_stop_is_drawn() {
        let mut work = AtWork::starting("send my name");
        work.steps = vec![
            step("Typing \u{201c}Ada\u{201d} into Name", ""),
            step("Pressing Send", "it sends the form"),
        ];
        let document = built(work);
        assert_eq!(of_class(&document, "card acting").len(), 1);
        assert_eq!(of_class(&document, "ring").len(), 1);
        assert_eq!(of_class(&document, "said"), vec![super::WORKING]);
        assert_eq!(of_class(&document, "abtn stop"), vec!["Stop"]);
        assert_eq!(of_class(&document, "asked"), vec!["send my name"]);
        assert_eq!(
            of_class(&document, "step"),
            vec!["Typing \u{201c}Ada\u{201d} into Name"]
        );
        assert_eq!(
            of_class(&document, "step now"),
            vec!["Pressing Sendit sends the form"]
        );
        assert!(of_class(&document, "abtn allow").is_empty());
    }

    #[test]
    fn asking_draws_the_question_with_allow_and_not_now_and_still_stop() {
        let mut work = AtWork::starting("send it");
        work.steps = vec![step("Pressing Send", "")];
        work.now = Now::Asking {
            what: "Press \u{201c}Send\u{201d}? It sends the form.".to_owned(),
        };
        let document = built(work);
        assert_eq!(
            of_class(&document, "question"),
            vec!["Press \u{201c}Send\u{201d}? It sends the form."]
        );
        assert_eq!(of_class(&document, "abtn allow"), vec!["Allow"]);
        assert_eq!(of_class(&document, "abtn notnow"), vec!["Not now"]);
        assert_eq!(of_class(&document, "abtn stop").len(), 1);
        // Not under way while it waits on the person.
        assert!(of_class(&document, "step now").is_empty());
        assert_eq!(of_class(&document, "card acting").len(), 1);
    }

    #[test]
    fn stuck_done_and_stopped_draw_no_outline_and_no_stop() {
        for (now, class, said) in [
            (
                Now::Stuck {
                    why: "The page asks for a password.".to_owned(),
                    choices: vec!["Try again".to_owned()],
                },
                "achoice",
                "Try again",
            ),
            (
                Now::Done {
                    said: "Sent.".to_owned(),
                },
                "done",
                "Done in 0 steps",
            ),
            (
                Now::Stopped {
                    why: "You stopped alo.".to_owned(),
                },
                "stopped",
                "You stopped alo.",
            ),
        ] {
            let mut work = AtWork::starting("x");
            work.now = now;
            let document = built(work);
            assert_eq!(of_class(&document, class), vec![said]);
            assert!(of_class(&document, "abtn stop").is_empty());
            assert!(of_class(&document, "ring").is_empty());
            assert_eq!(of_class(&document, "card").len(), 1);
        }
    }

    #[test]
    fn what_was_asked_and_each_step_stay_text() {
        let hostile = "</p><script>x()</script>";
        let mut work = AtWork::starting(hostile);
        work.steps = vec![step(hostile, hostile)];
        work.now = Now::Stuck {
            why: hostile.to_owned(),
            choices: vec![hostile.to_owned()],
        };
        let document = built(work);
        let scripts = document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .filter(|element| element.name.local.as_ref() == "script")
            .count();
        assert_eq!(scripts, 0);
        let texts = document
            .descendants(document.root())
            .filter(|&node| matches!(document.kind(node), Some(NodeKind::Text(data)) if data == hostile))
            .count();
        assert_eq!(texts, 5);
    }
}

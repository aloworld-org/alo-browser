/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Each run of alo's agent kept with a page shown earlier, built into the
//! first screen's document under the page's line (queue item 820, ADR 0085,
//! ADR 0092 as amended), the way the rest of the first screen is built
//! ([`crate::start_document`]): every element made with `alo-dom`'s
//! operations, every stranger's string a text node.
//!
//! # What is drawn
//!
//! The run as turns of the conversation: what the person asked, as their
//! bubble at the turn's right ([`REQUEST`] beside `you`); then what alo
//! did, on a white box rounded 12 — one line per step, in the quiet
//! colour, why under it when it chose, as the live card draws them — and
//! how it ended: *Done in N steps* in the done colour and the result in
//! full, or why it was stuck or stopped. Nothing is pressable: a stuck
//! run's choices were buttons on a live card, and are not kept.
//!
//! # What an agent reads
//!
//! ADR 0002: the person's words as a paragraph, then a `region` named
//! [`RAN`] holding a list named [`STEPS`] of each step and the outcome as
//! paragraphs — a result as its paragraphs and lists, as on the live card
//! ([`crate::result_document`], queue item 883).

use crate::agent_document::{STEPS, words};
use crate::agent_ran::{Ended, Ran};
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What a kept run is named, to an agent reading the screen.
pub const RAN: &str = "What alo did";

/// The class of the person's bubble asking a run, beside `you`.
pub const REQUEST: &str = "request";

/// Append each of `runs` to `turn`: the person's bubble, then what alo did.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no run's words can
/// cause.
pub fn append_runs(document: &mut Document, turn: NodeId, runs: &[Ran]) -> Result<(), Unbuilt> {
    for ran in runs {
        let asked = words(document, "p", &format!("you {REQUEST}"), &ran.asked)?;
        append(document, turn, asked)?;
        let made = kept(document, ran)?;
        append(document, turn, made)?;
    }
    Ok(())
}

/// What alo did in `ran`: its steps and how it ended — for a turn whose
/// bubble already says what was asked (queue item 821), on its own.
///
/// # Errors
///
/// As [`append_runs`].
pub fn kept(document: &mut Document, ran: &Ran) -> Result<NodeId, Unbuilt> {
    let made = element(document, "div")?;
    attribute(document, made, "class", "ran")?;
    attribute(document, made, "role", "region")?;
    attribute(document, made, "aria-label", RAN)?;
    // Its plan as it last stood (queue item 824), as the column drew it.
    if let Some(plan) = &ran.plan {
        let plan = crate::agent_document::planned(document, plan, ran.is_finished())?;
        append(document, made, plan)?;
    }
    if !ran.steps.is_empty() {
        let list = element(document, "ol")?;
        attribute(document, list, "class", "steps")?;
        attribute(document, list, "aria-label", STEPS)?;
        for step in &ran.steps {
            let item = element(document, "li")?;
            attribute(document, item, "class", "step")?;
            let line = words(document, "span", "line", &step.line)?;
            append(document, item, line)?;
            if !step.why.trim().is_empty() {
                let why = words(document, "span", "why", &step.why)?;
                append(document, item, why)?;
            }
            append(document, list, item)?;
        }
        append(document, made, list)?;
    }
    let ended = match &ran.ended {
        Ended::Done { said } => {
            let done = element(document, "div")?;
            attribute(document, done, "class", "finished")?;
            let line = words(document, "p", "done", &ran.done_in())?;
            append(document, done, line)?;
            crate::result_document::append_result(document, done, said)?;
            done
        }
        Ended::Stuck { why } | Ended::Stopped { why } => words(document, "p", "stopped", why)?,
    };
    append(document, made, ended)?;
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_ran::CLOSED;
    use crate::agent_work::Step;
    use crate::start::Start;
    use crate::start_document::build;
    use crate::turn::Turn;
    use alo_dom::NodeKind;

    fn of_class(document: &Document, class: &str) -> Vec<String> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .map(|node| document.text_content(node))
            .collect()
    }

    fn visited(ran: Vec<Ran>) -> Document {
        build(&Start {
            greeting: "Hello.".to_owned(),
            turns: vec![Turn::Visited {
                said: "en.wikipedia.org".to_owned(),
                url: alo_url::parse("https://en.wikipedia.org/wiki/Kampala")
                    .unwrap_or_else(|_| alo_url::Url::about_blank()),
                ran,
            }],
            ..Start::default()
        })
        .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    #[test]
    fn each_run_is_the_persons_bubble_then_what_alo_did() {
        let document = visited(vec![
            Ran {
                plan: None,
                asked: "Search Wikipedia for Kampala".to_owned(),
                steps: vec![
                    Step {
                        line: "Typing Kampala into Search Wikipedia".to_owned(),
                        why: String::new(),
                    },
                    Step {
                        line: "Pressing Search".to_owned(),
                        why: "it sends the search".to_owned(),
                    },
                ],
                ended: Ended::Done {
                    said: "Opened the article on Kampala.".to_owned(),
                },
            },
            Ran::starting("Find its population"),
        ]);
        assert_eq!(
            of_class(&document, "you"),
            vec!["en.wikipedia.org"],
            "the turn's own words"
        );
        assert_eq!(
            of_class(&document, "you request"),
            vec!["Search Wikipedia for Kampala", "Find its population"]
        );
        assert_eq!(of_class(&document, "ran").len(), 2);
        assert_eq!(
            of_class(&document, "line"),
            vec!["Typing Kampala into Search Wikipedia", "Pressing Search"]
        );
        assert_eq!(of_class(&document, "why"), vec!["it sends the search"]);
        assert_eq!(of_class(&document, "done"), vec!["Done in 2 steps"]);
        assert_eq!(
            of_class(&document, "result"),
            vec!["Opened the article on Kampala."]
        );
        assert_eq!(of_class(&document, "stopped"), vec![CLOSED]);
        // Nothing in the conversation pressable but the page's line.
        let thread = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some("thread"))
            .expect("the conversation");
        let buttons = document
            .descendants(thread)
            .filter_map(|node| document.element(node))
            .filter(|element| element.name.local.as_ref() == "button")
            .count();
        assert_eq!(buttons, 0);
    }

    #[test]
    fn a_runs_words_stay_text() {
        let hostile = "</p><script>x()</script><b>b</b>";
        let document = visited(vec![Ran {
            plan: None,
            asked: hostile.to_owned(),
            steps: vec![Step {
                line: hostile.to_owned(),
                why: hostile.to_owned(),
            }],
            ended: Ended::Stuck {
                why: hostile.to_owned(),
            },
        }]);
        let named = |name: &str| {
            document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .filter(|element| element.name.local.as_ref() == name)
                .count()
        };
        assert_eq!(named("script") + named("b"), 0);
        let texts = document
            .descendants(document.root())
            .filter(|&node| matches!(document.kind(node), Some(NodeKind::Text(data)) if data == hostile))
            .count();
        assert_eq!(texts, 4);
    }
}

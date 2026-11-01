/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a chat keeps of a run of alo's agent (queue item 820, ADR 0092 as
//! amended): what the person asked, each step in words, and how it ended —
//! **never what was typed into a field** unless the person's own words
//! said it.
//!
//! The person was shown each step's line and why as the model wrote them
//! (ADR 0096 § 7), and a typing step's line says what it typed: *Typing
//! "ada@example.com" into Email*. What was typed may be anything the page
//! or the person gave the agent; the file keeps none of it. So, from the
//! run as it was heard ([`Heard`]):
//!
//! - **What the person asked** is kept as they said it, as a search's
//!   words are (ADR 0092 § 2).
//! - **A typing step** whose text the person's own words hold — *search
//!   Wikipedia for Kampala*, typing *Kampala* — keeps its line and why, as
//!   every other step does, since the file holds those words already. Any
//!   other typing step is kept as [`TYPED`], and no why.
//! - **Every other line and why, and the result or why it stopped**, keeps
//!   the model's words with each text typed that the person did not say
//!   replaced by `…` wherever it stands as whole words, any case — so a
//!   later step's *Pressing Sign up for ada@example.com* keeps no address.
//!
//! **Its limit**, said: a model that writes a typed text otherwise than as
//! it typed it — spelled out, split, translated — is not recognised; and
//! what the page itself says is kept as far as the steps already said it.
//! Nothing of the page is added: no tree, no field's value, no address but
//! the page's own.

use alo_renderer::agent_ran::{Ended, Ran, TYPED};
use alo_renderer::agent_work::Step;

/// One step as the run said it: its line, why, and what it typed, if it
/// typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    /// The step's line, as shown.
    pub line: String,
    /// Why, as shown; empty when it did not choose.
    pub why: String,
    /// The text a typing step put into a field — the browser process's
    /// own, never shown and never kept but as the person's words hold it.
    pub typed: Option<String>,
}

/// The run asked `asked`, heard as `steps`, ended as `ended` — or, with
/// none, still at work, kept as alo closing stops it ([`Ran::starting`]) —
/// as the chat keeps it.
pub fn kept(asked: &str, steps: &[Heard], ended: Option<&Ended>) -> Ran {
    // The texts typed that the person did not say, longest first, so a
    // longer one is replaced before a shorter one inside it.
    let mut secret: Vec<&str> = steps
        .iter()
        .filter_map(|step| step.typed.as_deref())
        .map(str::trim)
        .filter(|typed| !typed.is_empty() && find(asked, typed).is_none())
        .collect();
    secret.sort_by_key(|typed| std::cmp::Reverse(typed.chars().count()));
    secret.dedup();
    let scrub = |said: &str| {
        secret
            .iter()
            .fold(said.to_owned(), |said, typed| replaced(&said, typed))
    };
    let steps = steps
        .iter()
        .map(|step| match step.typed.as_deref().map(str::trim) {
            Some(typed) if typed.is_empty() || find(asked, typed).is_none() => Step {
                line: TYPED.to_owned(),
                why: String::new(),
            },
            _ => Step {
                line: scrub(&step.line),
                why: scrub(&step.why),
            },
        })
        .collect();
    let mut ran = Ran::starting(asked);
    ran.steps = steps;
    if let Some(ended) = ended {
        ran.ended = match ended {
            Ended::Done { said } => Ended::Done { said: scrub(said) },
            Ended::Stuck { why } => Ended::Stuck { why: scrub(why) },
            Ended::Stopped { why } => Ended::Stopped { why: scrub(why) },
        };
    }
    ran
}

/// `said` with every whole-word occurrence of `typed`, in any case,
/// replaced by `…`.
fn replaced(said: &str, typed: &str) -> String {
    let mut kept = String::with_capacity(said.len());
    let mut rest = said;
    while let Some((from, to)) = find(rest, typed) {
        kept.push_str(rest.get(..from).unwrap_or_default());
        kept.push('\u{2026}');
        rest = rest.get(to..).unwrap_or_default();
    }
    kept.push_str(rest);
    kept
}

/// Where `typed` first stands in `said` as whole words, in any case: the
/// byte range it takes, its neighbours on either side not letters or
/// digits.
fn find(said: &str, typed: &str) -> Option<(usize, usize)> {
    let wanted: Vec<char> = typed.chars().flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        return None;
    }
    let letters: Vec<(usize, char)> = said.char_indices().collect();
    for start in 0..letters.len() {
        if start > 0
            && letters
                .get(start - 1)
                .is_some_and(|&(_, before)| before.is_alphanumeric())
        {
            continue;
        }
        let mut lowered = Vec::new();
        let mut end = start;
        while lowered.len() < wanted.len() {
            let Some(&(_, letter)) = letters.get(end) else {
                break;
            };
            lowered.extend(letter.to_lowercase());
            end += 1;
        }
        if lowered != wanted {
            continue;
        }
        if letters
            .get(end)
            .is_some_and(|&(_, after)| after.is_alphanumeric())
        {
            continue;
        }
        let from = letters.get(start).map_or(said.len(), |&(at, _)| at);
        let to = letters.get(end).map_or(said.len(), |&(at, _)| at);
        return Some((from, to));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::agent_ran::CLOSED;

    fn heard(line: &str, why: &str, typed: Option<&str>) -> Heard {
        Heard {
            line: line.to_owned(),
            why: why.to_owned(),
            typed: typed.map(str::to_owned),
        }
    }

    #[test]
    fn a_text_the_person_said_is_kept_as_its_steps_say_it() {
        let ran = kept(
            "Search Wikipedia for Kampala and open the article",
            &[
                heard("Typing Kampala into Search Wikipedia", "", Some("Kampala")),
                heard("Pressing Search", "it sends the search for Kampala", None),
            ],
            Some(&Ended::Done {
                said: "Opened the article on Kampala.".to_owned(),
            }),
        );
        assert_eq!(
            ran.steps
                .iter()
                .map(|step| (step.line.as_str(), step.why.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("Typing Kampala into Search Wikipedia", ""),
                ("Pressing Search", "it sends the search for Kampala"),
            ]
        );
        assert_eq!(
            ran.ended,
            Ended::Done {
                said: "Opened the article on Kampala.".to_owned()
            }
        );
    }

    #[test]
    fn a_text_typed_that_the_person_did_not_say_is_never_kept() {
        let ran = kept(
            "Sign me up for the newsletter",
            &[
                heard(
                    "Typing \u{201c}ada@example.com\u{201d} into Email",
                    "your address from the page",
                    Some(" ada@example.com "),
                ),
                heard(
                    "Pressing Sign up",
                    "ADA@EXAMPLE.COM is typed, and adalovelace stays",
                    None,
                ),
            ],
            Some(&Ended::Stopped {
                why: "Signed up ada@example.com.".to_owned(),
            }),
        );
        let typing = ran.steps.first().expect("a typing step");
        assert_eq!((typing.line.as_str(), typing.why.as_str()), (TYPED, ""));
        let pressing = ran.steps.get(1).expect("a press");
        assert_eq!(
            pressing.why, "\u{2026} is typed, and adalovelace stays",
            "replaced in any case, and only as whole words"
        );
        assert_eq!(
            ran.ended,
            Ended::Stopped {
                why: "Signed up \u{2026}.".to_owned()
            }
        );
        let all = format!("{ran:?}").to_lowercase();
        assert!(!all.contains("ada@example.com"), "{all}");
    }

    #[test]
    fn a_longer_text_is_replaced_before_one_inside_it_and_a_run_at_work_reads_as_closed() {
        let ran = kept(
            "Fill the form",
            &[
                heard("Typing", "", Some("Ada")),
                heard("Typing", "", Some("Ada Lovelace")),
                heard("Pressing Send for Ada Lovelace, Ada", "", None),
            ],
            None,
        );
        assert_eq!(
            ran.steps.get(2).map(|step| step.line.as_str()),
            Some("Pressing Send for \u{2026}, \u{2026}")
        );
        assert_eq!(
            ran.ended,
            Ended::Stopped {
                why: CLOSED.to_owned()
            }
        );
    }

    #[test]
    fn whole_words_are_found_in_any_case_and_nowhere_else() {
        assert_eq!(find("Typing Ada into Name", "ada"), Some((7, 10)));
        assert_eq!(find("Adam", "Ada"), None);
        assert_eq!(find("é Ångström.", "ångström"), Some((3, 13)));
        assert_eq!(find("x", ""), None);
        assert_eq!(replaced("a1 a 1a a", "a"), "a1 \u{2026} 1a \u{2026}");
    }
}

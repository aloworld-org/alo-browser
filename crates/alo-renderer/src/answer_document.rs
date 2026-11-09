/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's answer, built into the first screen's document (queue item
//! 707, `docs/design/shell.md` item 2 and *The answer*): the short answer
//! at the top of **All**, and the **Answer** tab's answer in full with its
//! sources. Every word the model wrote, and every site's name, address and
//! title, enters as a text node; the structure is ours, made with
//! `alo-dom`'s operations.
//!
//! # What an agent reads
//!
//! ADR 0002: a `region` named [`REGION`]. On All, a paragraph of two or
//! three sentences, each followed by the numbers of the sources that say it
//! as text; then *Full answer ›*, a button, and how many sites the answer
//! drew on. On the Answer tab, the one-line answer as a paragraph, the
//! short points as a list, each followed by its numbers, then a `region`
//! named [`SOURCES`] holding *N sources* and, for each number cited, the
//! number, the site's name, its address and its page's title — so the
//! sources are read by name — and one line saying what was sent to have it
//! written, and to whom (ADR 0081 § 5), as it does under *I couldn't find a
//! reliable answer*. Where there is no answer to draw, one
//! sentence: that it is being written, *I couldn't find a reliable answer*
//! and what is offered instead, or why it was not answered.
//!
//! # A press
//!
//! *Full answer ›* is of the class `full`, claimed by its place among the
//! *Full answer ›* drawn ([`crate::conversation_claim`]), and opens that
//! search's Answer tab. Nothing else here claims a press: a source is read,
//! not opened, in this cut (queue item 813).

use crate::results::{Found, Results};
use crate::results_answer::{self, Answer, Cited, FULL_ANSWER, NO_RELIABLE_ANSWER, Written};
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the answer is named, to an agent.
pub const REGION: &str = "alo\u{2019}s answer";

/// What the Answer tab's list of sources is named, to an agent.
pub const SOURCES: &str = "Sources";

/// The short answer at the top of All, in `made`, when an answer was asked
/// for and the sites were found: its sentences, *Full answer ›* and how
/// many sites it drew on; or one sentence in its place.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no answer can
/// cause.
pub fn short(document: &mut Document, made: NodeId, results: &Results) -> Result<(), Unbuilt> {
    if matches!(results.answer, Answer::NotAsked) || results.note().is_some() {
        return Ok(());
    }
    let region = element(document, "section")?;
    attribute(document, region, "class", "ans")?;
    attribute(document, region, "aria-label", REGION)?;
    append(document, made, region)?;
    let Answer::Written(written) = &results.answer else {
        return said_instead(document, region, results, "are below");
    };
    let said = element(document, "p")?;
    attribute(document, said, "class", "ansp")?;
    for (at, cited) in written.sentences.iter().enumerate() {
        if at > 0 {
            let between = document.create_text_node(" ");
            append(document, said, between)?;
        }
        claim(document, said, cited)?;
    }
    append(document, region, said)?;
    let foot = element(document, "div")?;
    attribute(document, foot, "class", "ansf")?;
    let full = element(document, "button")?;
    attribute(document, full, "class", "full")?;
    attribute(document, full, "type", "button")?;
    let words = document.create_text_node(FULL_ANSWER);
    append(document, full, words)?;
    append(document, foot, full)?;
    let drew = element(document, "span")?;
    attribute(document, drew, "class", "ansn")?;
    let words = document.create_text_node(&format!(
        "Drew on {}",
        results_answer::sites(written.used().len())
    ));
    append(document, drew, words)?;
    append(document, foot, drew)?;
    append(document, region, foot)
}

/// The Answer tab, in `made`: the answer in one line, its short points and
/// its sources; or one sentence in its place.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no answer can
/// cause.
pub fn full(document: &mut Document, made: NodeId, results: &Results) -> Result<(), Unbuilt> {
    let region = element(document, "section")?;
    attribute(document, region, "class", "answer")?;
    attribute(document, region, "aria-label", REGION)?;
    append(document, made, region)?;
    let Answer::Written(written) = &results.answer else {
        said_instead(document, region, results, "are on the Sites tab")?;
        if matches!(results.answer, Answer::NotFound) {
            sent(document, region, results)?;
        }
        return Ok(());
    };
    let line = element(document, "p")?;
    attribute(document, line, "class", "ansline")?;
    let words = document.create_text_node(&written.line);
    append(document, line, words)?;
    append(document, region, line)?;
    if !written.points.is_empty() {
        let list = element(document, "ul")?;
        attribute(document, list, "class", "pts")?;
        for cited in &written.points {
            let point = element(document, "li")?;
            attribute(document, point, "class", "pt")?;
            claim(document, point, cited)?;
            append(document, list, point)?;
        }
        append(document, region, list)?;
    }
    sources(document, region, written, results.sources())?;
    sent(document, region, results)
}

/// What was sent to have the answer written, in `region`, in one quiet
/// line (ADR 0081 § 5).
fn sent(document: &mut Document, region: NodeId, results: &Results) -> Result<(), Unbuilt> {
    let said = element(document, "p")?;
    attribute(document, said, "class", "sent")?;
    let words = document.create_text_node(&results_answer::sent(results.sources().len()));
    append(document, said, words)?;
    append(document, region, said)
}

/// The sources an answer cited, in `region`: *N sources*, then each by its
/// number, its site's name and address, and its page's title.
fn sources(
    document: &mut Document,
    region: NodeId,
    written: &Written,
    given: &[Found],
) -> Result<(), Unbuilt> {
    let used = written.used();
    let listed = element(document, "section")?;
    attribute(document, listed, "class", "srcs")?;
    attribute(document, listed, "aria-label", SOURCES)?;
    let heading = element(document, "p")?;
    attribute(document, heading, "class", "srch")?;
    let words = document.create_text_node(&results_answer::sources(used.len()));
    append(document, heading, words)?;
    append(document, listed, heading)?;
    for number in used {
        // A number past the sites given never reaches here: an answer
        // citing one is refused on the wire (`Written::check`).
        let Some(found) = number.checked_sub(1).and_then(|place| given.get(place)) else {
            continue;
        };
        let source = element(document, "div")?;
        attribute(document, source, "class", "srci")?;
        number_of(document, source, number)?;
        for (class, words) in [
            ("sname", found.site()),
            ("saddr", found.address()),
            ("stitle", found.title.clone()),
        ] {
            let part = element(document, "span")?;
            attribute(document, part, "class", class)?;
            // A stranger's name, address or title, as a text node's data.
            let text = document.create_text_node(&words);
            append(document, part, text)?;
            append(document, source, part)?;
        }
        append(document, listed, source)?;
    }
    append(document, region, listed)
}

/// One sentence or point, in `into`: what it says, then each source's
/// number after a space.
fn claim(document: &mut Document, into: NodeId, cited: &Cited) -> Result<(), Unbuilt> {
    // The model's words, as a text node's data.
    let text = document.create_text_node(&cited.text);
    append(document, into, text)?;
    for &number in &cited.sources {
        let space = document.create_text_node(" ");
        append(document, into, space)?;
        number_of(document, into, number)?;
    }
    Ok(())
}

/// A source's number, in `into`.
fn number_of(document: &mut Document, into: NodeId, number: usize) -> Result<(), Unbuilt> {
    let cite = element(document, "span")?;
    attribute(document, cite, "class", "cite")?;
    let words = document.create_text_node(&number.to_string());
    append(document, cite, words)?;
    append(document, into, cite)
}

/// What is said in `region` where an answer would be: that it is being
/// written, why it was not answered, or [`NO_RELIABLE_ANSWER`] and that the
/// search's sites `where_they_are`.
fn said_instead(
    document: &mut Document,
    region: NodeId,
    results: &Results,
    where_they_are: &str,
) -> Result<(), Unbuilt> {
    let note = match &results.answer {
        Answer::NotFound => format!(
            "{NO_RELIABLE_ANSWER} The sites found for \u{201c}{}\u{201d} {where_they_are}.",
            results.said
        ),
        other => other.note().unwrap_or_default(),
    };
    let said = element(document, "p")?;
    attribute(document, said, "class", "ansnote")?;
    // Why, in words, perhaps holding what was typed, as a text node's data.
    let text = document.create_text_node(&note);
    append(document, said, text)?;
    append(document, region, said)
}

#[cfg(test)]
mod tests {
    use crate::results::{Found, Results, Showing, Tab};
    use crate::results_answer::{Answer, Cited, Written};
    use crate::start::Start;
    use crate::start_document::build;
    use alo_dom::{Document, NodeKind};

    fn found(title: &str, address: &str) -> Found {
        Found::new(title, alo_url::parse(address).unwrap(), "Two lines of it.")
    }

    fn results(answer: Answer) -> Results {
        Results::of(
            "ev charger",
            Showing::Found(vec![
                found("One", "https://one.example/a").named("One Guide"),
                found("Two", "https://two.example/"),
                found("Three", "https://three.example/"),
                found("Four", "https://four.example/"),
            ]),
        )
        .with_answer(answer)
    }

    fn written() -> Answer {
        Answer::Written(Written {
            line: "About a day.".to_owned(),
            sentences: vec![
                Cited::new("A fitting takes a day.", [1, 4]),
                Cited::new("A new circuit takes longer.", [4]),
            ],
            points: vec![
                Cited::new("Book an electrician.", [1]),
                Cited::new("Ask about the fuse box.", [4]),
            ],
        })
    }

    fn built(results: Results) -> Document {
        build(&Start {
            greeting: "Hello.".to_owned(),
            turns: vec![crate::turn::Turn::Searched(results)],
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

    #[test]
    fn all_puts_the_short_answer_over_the_sites_and_marks_those_it_used() {
        let document = built(results(written()));
        assert_eq!(
            of_class(&document, "ansp"),
            vec!["A fitting takes a day. 1 4 A new circuit takes longer. 4"]
        );
        assert_eq!(of_class(&document, "cite"), vec!["1", "4", "4"]);
        assert_eq!(of_class(&document, "full"), vec![super::FULL_ANSWER]);
        assert_eq!(of_class(&document, "ansn"), vec!["Drew on 2 sites"]);
        // All draws three sites; the first was used, the fourth not drawn.
        assert_eq!(of_class(&document, "used"), vec!["alo used this"]);
        let marked: Vec<bool> = of_class(&document, "found")
            .iter()
            .map(|site| site.contains("alo used this"))
            .collect();
        assert_eq!(marked, vec![true, false, false], "on the site it used");
        assert_eq!(of_class(&document, "rt on"), vec!["All"]);
        assert!(of_class(&document, "rt").contains(&"Answer".to_owned()));
        let order: Vec<&str> = document
            .descendants(document.root())
            .filter_map(|node| document.attribute_by_name(node, "class"))
            .filter(|class| matches!(*class, "ans" | "res" | "rtabs"))
            .collect();
        assert_eq!(order, vec!["rtabs", "ans", "res"]);
    }

    #[test]
    fn the_answer_tab_holds_the_line_the_points_and_the_sources_by_name() {
        let document = built(results(written()).on(Tab::Answer));
        assert_eq!(of_class(&document, "ansline"), vec!["About a day."]);
        assert_eq!(
            of_class(&document, "pt"),
            vec!["Book an electrician. 1", "Ask about the fuse box. 4"]
        );
        assert_eq!(of_class(&document, "srch"), vec!["2 sources"]);
        assert_eq!(
            of_class(&document, "sname"),
            vec!["One Guide", "four.example"]
        );
        assert_eq!(of_class(&document, "stitle"), vec!["One", "Four"]);
        assert!(of_class(&document, "found").is_empty(), "no sites drawn");
        assert!(of_class(&document, "ansp").is_empty(), "no short answer");
        assert_eq!(of_class(&document, "rt on"), vec!["Answer"]);
        assert_eq!(
            of_class(&document, "sent"),
            vec![
                "Your question and what the search said of its first 4 sites were sent to \
                 the AI service chosen in Settings to write this answer."
            ]
        );
    }

    #[test]
    fn with_no_answer_to_draw_one_sentence_stands_in_its_place() {
        for (answer, on_all, on_answer) in [
            (
                Answer::Asking,
                "alo is writing an answer from these sites\u{2026}",
                "alo is writing an answer from these sites\u{2026}",
            ),
            (
                Answer::NotFound,
                "I couldn\u{2019}t find a reliable answer. The sites found for \u{201c}ev \
                 charger\u{201d} are below.",
                "I couldn\u{2019}t find a reliable answer. The sites found for \u{201c}ev \
                 charger\u{201d} are on the Sites tab.",
            ),
            (
                Answer::Unanswered("OpenAI could not answer just now (503).".into()),
                "OpenAI could not answer just now (503).",
                "OpenAI could not answer just now (503).",
            ),
        ] {
            let document = built(results(answer.clone()));
            assert_eq!(of_class(&document, "ansnote"), vec![on_all]);
            assert!(of_class(&document, "full").is_empty());
            assert!(of_class(&document, "used").is_empty());
            let document = built(results(answer).on(Tab::Answer));
            assert_eq!(of_class(&document, "ansnote"), vec![on_answer]);
        }
        // Never asked: nothing, and the Answer tab disabled.
        let document = built(results(Answer::NotAsked));
        assert!(of_class(&document, "ans").is_empty());
        assert!(of_class(&document, "rt off").contains(&"Answer".to_owned()));
    }

    #[test]
    fn the_models_words_stay_text() {
        let hostile = "<b>Fish &amp; chips</b><script>x()</script>";
        let answer = Answer::Written(Written {
            line: hostile.to_owned(),
            sentences: vec![Cited::new(hostile, [1])],
            points: vec![Cited::new(hostile, [1])],
        });
        for tab in [Tab::All, Tab::Answer] {
            let document = built(results(answer.clone()).on(tab));
            let scripts = document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .filter(|element| matches!(element.name.local.as_ref(), "script" | "b"))
                .count();
            assert_eq!(scripts, 0);
            let texts = document
                .descendants(document.root())
                .filter(|&node| {
                    matches!(document.kind(node), Some(NodeKind::Text(data)) if data == hostile)
                })
                .count();
            assert_eq!(texts, if tab == Tab::All { 1 } else { 2 }, "{tab:?}");
        }
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's answer, written by the model from the sites the search found
//! (queue item 707, ADR 0081, ADR 0095): what the browser process tells the
//! first screen's renderer of it, as data.
//!
//! The browser process asks the model — `OpenAI`'s, on a thread of its own
//! (`alo-window`'s `conversation_answer`) — giving it the question and the
//! first [`MOST_SOURCES`] sites the search found, each by its number, its
//! place among them counted from one; and reads back an answer whose every
//! sentence names the numbers of the sites that say it. This is what it
//! then holds: whether the answer is asked, written, honestly not found, or
//! not answered and why.
//!
//! # Checked by construction
//!
//! A [`Written`] answer is never drawn unless every number it cites is one
//! of the sites given ([`Written::check`]): the browser process refuses an
//! answer that names a source it was not given, and the renderer refuses
//! one on the wire too. *N sources* counts only the numbers cited
//! ([`Written::used`]), and a site the answer drew on says *alo used this*.

/// The most sites given to the model as sources: the search's first.
pub const MOST_SOURCES: usize = 8;

/// The most sentences the short answer at the top of All holds.
pub const MOST_SENTENCES: usize = 3;

/// The most short points the Answer tab holds.
pub const MOST_POINTS: usize = 6;

/// The longest sentence or point kept, in characters.
pub const LONGEST_CLAIM: usize = 600;

/// What is said when the sources do not answer the question.
pub const NO_RELIABLE_ANSWER: &str = "I couldn\u{2019}t find a reliable answer.";

/// What *Full answer ›* says.
pub const FULL_ANSWER: &str = "Full answer \u{203a}";

/// What a site the answer drew on says.
pub const USED: &str = "alo used this";

/// Who writes the answer, in words a person reads.
pub const ANSWERER: &str = "OpenAI";

/// What the Answer tab says was sent to write it (ADR 0081 § 5: an answer
/// says what it sent): the person's question and what the search provider
/// said of the first `given` sites.
pub fn sent(given: usize) -> String {
    format!(
        "Your question and what {} said of its first {} were sent to {ANSWERER} to write this \
         answer.",
        crate::results::PROVIDER,
        sites(given)
    )
}

/// Where a search's answer is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Answer {
    /// Not asked: the search found no sites, or is still under way.
    #[default]
    NotAsked,
    /// Asked of the model, and not yet answered.
    Asking,
    /// Written, every sentence citing the sites that say it.
    Written(Written),
    /// The model said the sites do not reliably answer the question:
    /// [`NO_RELIABLE_ANSWER`], and nothing made up in its place.
    NotFound,
    /// Not answered, and why, in a sentence a person reads: no key, no
    /// connection, a refusal, a status, an answer that could not be read.
    Unanswered(String),
}

/// An answer written from a search's sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The answer in one line: the Answer tab's first line.
    pub line: String,
    /// Two or three sentences: the short answer at the top of All.
    pub sentences: Vec<Cited>,
    /// Short points: the Answer tab's, under its line.
    pub points: Vec<Cited>,
}

/// A sentence or a point, and the sites that say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cited {
    /// What it says, as plain text.
    pub text: String,
    /// The numbers of the sites that say it, each its place among the
    /// sources given counted from one, in the order written.
    pub sources: Vec<usize>,
}

impl Cited {
    /// `text`, said by the sites numbered `sources`.
    pub fn new(text: impl Into<String>, sources: impl Into<Vec<usize>>) -> Self {
        Self {
            text: text.into(),
            sources: sources.into(),
        }
    }
}

impl Written {
    /// Every number cited, in order, each once: the sites the answer used.
    pub fn used(&self) -> Vec<usize> {
        let mut used: Vec<usize> = self
            .sentences
            .iter()
            .chain(&self.points)
            .flat_map(|cited| cited.sources.iter().copied())
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }

    /// Whether the answer is one that may be drawn, given `given` sources:
    /// a line and at least one sentence; at most [`MOST_SENTENCES`]
    /// sentences and [`MOST_POINTS`] points, none longer than
    /// [`LONGEST_CLAIM`]; and every sentence and point citing at least one
    /// source, each one of the `given`.
    ///
    /// # Errors
    ///
    /// Why not, in words: the first thing wrong.
    pub fn check(&self, given: usize) -> Result<(), String> {
        if self.line.trim().is_empty() {
            return Err("the answer has no line".to_owned());
        }
        if self.sentences.is_empty() {
            return Err("the answer has no sentence".to_owned());
        }
        if self.sentences.len() > MOST_SENTENCES {
            return Err(format!(
                "the answer has {} sentences, and at most {MOST_SENTENCES} are drawn",
                self.sentences.len()
            ));
        }
        if self.points.len() > MOST_POINTS {
            return Err(format!(
                "the answer has {} points, and at most {MOST_POINTS} are drawn",
                self.points.len()
            ));
        }
        if self.line.chars().count() > LONGEST_CLAIM {
            return Err(format!(
                "the answer's line is longer than {LONGEST_CLAIM} characters"
            ));
        }
        for cited in self.sentences.iter().chain(&self.points) {
            if cited.text.trim().is_empty() {
                return Err("the answer has an empty sentence".to_owned());
            }
            if cited.text.chars().count() > LONGEST_CLAIM {
                return Err(format!(
                    "the answer has a sentence longer than {LONGEST_CLAIM} characters"
                ));
            }
            if cited.sources.is_empty() {
                return Err("the answer has a sentence that cites no source".to_owned());
            }
            if cited.sources.len() > MOST_SOURCES {
                return Err(format!(
                    "the answer has a sentence citing more than {MOST_SOURCES} sources"
                ));
            }
            if let Some(stranger) = cited
                .sources
                .iter()
                .find(|&&number| number == 0 || number > given)
            {
                return Err(format!(
                    "the answer cites source {stranger}, and it was given {given}"
                ));
            }
        }
        Ok(())
    }
}

impl Answer {
    /// Whether the site at `place` among the search's sites, counted from
    /// zero, is one the answer used: it then says [`USED`].
    pub fn used(&self, place: usize) -> bool {
        match self {
            Answer::Written(written) => written.used().contains(&(place + 1)),
            Answer::NotAsked | Answer::Asking | Answer::NotFound | Answer::Unanswered(_) => false,
        }
    }

    /// What is said where an answer would be, when there is none to draw:
    /// that it is being written, that none was found, or why not; [`None`]
    /// when it is written, or was never asked.
    pub fn note(&self) -> Option<String> {
        match self {
            Answer::NotAsked | Answer::Written(_) => None,
            Answer::Asking => Some("alo is writing an answer from these sites\u{2026}".to_owned()),
            Answer::NotFound => Some(NO_RELIABLE_ANSWER.to_owned()),
            Answer::Unanswered(why) => Some(why.clone()),
        }
    }
}

/// How many sites `count` is, in words: *1 site*, *3 sites*.
pub fn sites(count: usize) -> String {
    if count == 1 {
        "1 site".to_owned()
    } else {
        format!("{count} sites")
    }
}

/// How many sources `count` is, in words: *1 source*, *3 sources*.
pub fn sources(count: usize) -> String {
    if count == 1 {
        "1 source".to_owned()
    } else {
        format!("{count} sources")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written() -> Written {
        Written {
            line: "About a day.".to_owned(),
            sentences: vec![
                Cited::new("A fitting takes a day.", [1, 3]),
                Cited::new("A new circuit may take longer.", [3]),
            ],
            points: vec![Cited::new("Book an electrician.", [2])],
        }
    }

    #[test]
    fn the_sources_used_are_every_number_cited_once() {
        assert_eq!(written().used(), vec![1, 2, 3]);
        let answer = Answer::Written(written());
        assert!(answer.used(0) && answer.used(2) && !answer.used(3));
        assert!(!Answer::NotFound.used(0));
        assert_eq!(sources(1), "1 source");
        assert_eq!(sources(3), "3 sources");
        assert_eq!(sites(1), "1 site");
        assert_eq!(sites(2), "2 sites");
    }

    #[test]
    fn an_answer_citing_a_source_not_given_is_refused() {
        assert_eq!(written().check(3), Ok(()));
        assert_eq!(
            written().check(2),
            Err("the answer cites source 3, and it was given 2".to_owned())
        );
        let mut zero = written();
        zero.points[0].sources = vec![0];
        assert_eq!(
            zero.check(8),
            Err("the answer cites source 0, and it was given 8".to_owned())
        );
        let mut uncited = written();
        uncited.sentences[1].sources.clear();
        assert_eq!(
            uncited.check(8),
            Err("the answer has a sentence that cites no source".to_owned())
        );
    }

    #[test]
    fn an_answer_too_long_or_empty_is_refused_by_name() {
        let mut many = written();
        many.sentences = vec![Cited::new("x", [1]); MOST_SENTENCES + 1];
        assert!(many.check(8).is_err_and(|why| why.contains("4 sentences")));
        let mut points = written();
        points.points = vec![Cited::new("x", [1]); MOST_POINTS + 1];
        assert!(points.check(8).is_err_and(|why| why.contains("7 points")));
        let mut long = written();
        long.sentences[0].text = "x".repeat(LONGEST_CLAIM + 1);
        assert!(long.check(8).is_err());
        let mut lineless = written();
        lineless.line = " ".to_owned();
        assert_eq!(lineless.check(8), Err("the answer has no line".to_owned()));
        let mut none = written();
        none.sentences.clear();
        assert_eq!(none.check(8), Err("the answer has no sentence".to_owned()));
    }

    #[test]
    fn an_answer_not_drawn_says_why_in_its_place() {
        assert_eq!(Answer::NotAsked.note(), None);
        assert_eq!(Answer::Written(written()).note(), None);
        assert_eq!(
            Answer::NotFound.note().as_deref(),
            Some("I couldn\u{2019}t find a reliable answer.")
        );
        assert_eq!(
            Answer::Unanswered("OpenAI could not answer just now (503).".into())
                .note()
                .as_deref(),
            Some("OpenAI could not answer just now (503).")
        );
        assert!(
            Answer::Asking
                .note()
                .is_some_and(|said| said.ends_with('\u{2026}'))
        );
    }
}

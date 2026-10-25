/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person watching is told of an agent's verbs, and for how long.
//!
//! Queue items 623 and 625. A verb that ran leaves the sentence its agent
//! was answered with; a verb that was refused leaves the refusal's — `nothing
//! on the page is "A pony"` — because a refusal is the most convincing thing
//! this browser does and a person who is not told of it sees an agent that
//! seems to have done nothing at all.
//!
//! The window has no clock (ADR 0024 § 2: it shows what it was last sent), so
//! "how long" is counted in verbs:
//!
//! - **a sentence about a verb that ran is kept until the next verb runs;**
//! - **a refusal is kept until the second verb runs after it,** so the
//!   sentence of the verb that finally worked never hides why the ones
//!   before it did not — a person needs longer to read why nothing happened
//!   than to see that something did;
//! - **a refusal never replaces anything:** two refusals in a row are two
//!   sentences, and a refusal after a verb that ran leaves that verb's
//!   sentence where it was.
//!
//! At most [`MOST_AGENT_LINES`] are kept, the oldest given up first, so an
//! agent refused over and over fills a few lines of the window and never the
//! page.

/// The most lines the window keeps of what agents did.
pub const MOST_AGENT_LINES: usize = 3;

/// One sentence the window shows of an agent's verb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLine {
    /// The sentence the agent was answered with, word for word.
    pub sentence: String,
    /// Whether the verb was refused, so nothing was done.
    pub refused: bool,
}

/// The lines the window shows of agents' verbs, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentLines {
    lines: Vec<AgentLine>,
}

impl AgentLines {
    /// A verb ran and was answered with `sentence`: the verb that ran before
    /// it, and every refusal before that, are given up.
    pub fn did(&mut self, sentence: String) {
        let since = self
            .lines
            .iter()
            .rposition(|line| !line.refused)
            .map_or(0, |last| last + 1);
        self.lines.drain(..since);
        self.push(sentence, false);
    }

    /// A verb was refused with `sentence`: added below what is there.
    pub fn refused(&mut self, sentence: String) {
        self.push(sentence, true);
    }

    /// The lines, oldest first.
    pub fn lines(&self) -> &[AgentLine] {
        &self.lines
    }

    fn push(&mut self, sentence: String, refused: bool) {
        self.lines.push(AgentLine { sentence, refused });
        let over = self.lines.len().saturating_sub(MOST_AGENT_LINES);
        self.lines.drain(..over);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines as (sentence, refused) pairs.
    fn told(lines: &AgentLines) -> Vec<(&str, bool)> {
        lines
            .lines()
            .iter()
            .map(|line| (line.sentence.as_str(), line.refused))
            .collect()
    }

    #[test]
    fn a_verb_that_ran_is_replaced_by_the_next_that_runs() {
        let mut lines = AgentLines::default();
        assert!(lines.lines().is_empty());
        lines.did("put \"Ada\" into box#3".to_owned());
        assert_eq!(told(&lines), [("put \"Ada\" into box#3", false)]);
        lines.did("activated box#5".to_owned());
        assert_eq!(told(&lines), [("activated box#5", false)]);
    }

    #[test]
    fn two_refusals_in_a_row_are_two_sentences() {
        let mut lines = AgentLines::default();
        lines.refused("nothing on the page is \"A pony\"".to_owned());
        lines.refused("box#4 says it is disabled".to_owned());
        assert_eq!(
            told(&lines),
            [
                ("nothing on the page is \"A pony\"", true),
                ("box#4 says it is disabled", true),
            ]
        );
    }

    #[test]
    fn a_refusal_leaves_the_verb_that_ran_before_it() {
        let mut lines = AgentLines::default();
        lines.did("activated box#5".to_owned());
        lines.refused("box#4 says it is disabled".to_owned());
        assert_eq!(
            told(&lines),
            [
                ("activated box#5", false),
                ("box#4 says it is disabled", true),
            ]
        );
    }

    #[test]
    fn a_refusal_outlasts_the_next_verb_that_runs_and_not_the_one_after() {
        let mut lines = AgentLines::default();
        lines.did("activated box#5".to_owned());
        lines.refused("box#4 says it is disabled".to_owned());
        // The verb that ran next takes the one before the refusal away, and
        // leaves the refusal above its own sentence.
        lines.did("activated box#6".to_owned());
        assert_eq!(
            told(&lines),
            [
                ("box#4 says it is disabled", true),
                ("activated box#6", false),
            ]
        );
        // The second takes the refusal too.
        lines.did("activated box#7".to_owned());
        assert_eq!(told(&lines), [("activated box#7", false)]);
    }

    #[test]
    fn refusals_before_any_verb_ran_outlast_the_first_that_runs() {
        let mut lines = AgentLines::default();
        lines.refused("nothing on the page is \"Nme\"".to_owned());
        lines.did("put \"Ada\" into box#3".to_owned());
        assert_eq!(
            told(&lines),
            [
                ("nothing on the page is \"Nme\"", true),
                ("put \"Ada\" into box#3", false),
            ]
        );
    }

    #[test]
    fn at_most_three_are_kept_and_the_oldest_goes_first() {
        let mut lines = AgentLines::default();
        lines.did("activated box#5".to_owned());
        for node in 1..=4 {
            lines.refused(format!("box#{node} says it is disabled"));
            assert!(lines.lines().len() <= MOST_AGENT_LINES);
        }
        assert_eq!(
            told(&lines),
            [
                ("box#2 says it is disabled", true),
                ("box#3 says it is disabled", true),
                ("box#4 says it is disabled", true),
            ]
        );
        // However many, a verb that runs keeps only the refusals since the
        // last that ran — here all of them, less the oldest.
        lines.did("activated box#6".to_owned());
        assert_eq!(
            told(&lines),
            [
                ("box#3 says it is disabled", true),
                ("box#4 says it is disabled", true),
                ("activated box#6", false),
            ]
        );
    }
}

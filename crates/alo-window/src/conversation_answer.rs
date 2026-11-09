/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's answer, asked of the model (queue item 707, ADR 0081, ADR
//! 0095): a thread of the browser process that asks `OpenAI` and tells the
//! window what came back.
//!
//! Once a search has found sites, the search's thread hands this thread the
//! person's words and the first sites found ([`Answerer::ask`]) and returns
//! at once; this thread asks the model ([`crate::openai_answer`]) through
//! this repository's own HTTP stack, `alo-net`, and tells the window
//! [`Came::Answer`] as an event like any other. Never a renderer: a renderer
//! has no network (ADR 0005), and is sent only the answer, as data.
//!
//! # Once a search, only after it
//!
//! Nothing is asked while a person types, nor for a search that found no
//! sites: one answer for one search, asked once its sites have come. A
//! search answered from the provider's results alone sends nothing the
//! person did not already send Brave, but to `OpenAI`.
//!
//! # Not set up
//!
//! With no key ([`crate::answer_key`]) there is no thread: an answer asked
//! for is told unanswered at once, saying why, in its place on the screen.
//!
//! # Newest wins
//!
//! Of the answers waiting when the thread is free, only the newest search's
//! is asked; each older one is told it was not asked, so its turn does not
//! wait for ever.

use crate::ai_service::{AI_SERVICE, AiService};
use crate::answer_protocol::Spent;
use crate::conversation_search::{Came, Searched, TellSearched};
use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::results::Found;
use alo_renderer::results_answer::{Answer, MOST_SOURCES};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the model is waited for before an answer is said not had.
pub const PATIENCE: Duration = Duration::from_secs(30);

/// What the thread is handed.
enum Work {
    /// Answer these words from these sites, for the search numbered so.
    Ask(u64, String, Vec<Found>),
    /// The window is closing: end.
    Finish,
}

/// A thread asking the model for each search's answer, one at a time.
pub struct Answerer {
    /// Why answers are not set up, if they are not.
    not_set_up: Option<String>,
    /// Who answers, for the terminal alone, when they are.
    named: Option<String>,
    /// Where work is sent; [`None`] with no provider, or once finished.
    asked: Option<Sender<Work>>,
    thread: Option<JoinHandle<()>>,
    /// How the window is told, while there is no thread to tell it.
    tell: Option<TellSearched>,
}

impl core::fmt::Debug for Answerer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Answerer")
            .field("not_set_up", &self.not_set_up)
            .finish_non_exhaustive()
    }
}

impl Answerer {
    /// Start a thread asking `provider` and telling the window with `tell`;
    /// or, with no provider and why, none, every answer told unanswered
    /// with why.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(provider: Result<AiService, String>, tell: TellSearched) -> std::io::Result<Self> {
        let provider = match provider {
            Ok(provider) => provider,
            Err(why) => {
                return Ok(Self {
                    not_set_up: Some(why),
                    named: None,
                    asked: None,
                    thread: None,
                    tell: Some(tell),
                });
            }
        };
        let named = Some(provider.named());
        let (asked, asking) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-conversation-answer".to_owned())
            .spawn(move || answer(&provider, &asking, &tell))?;
        Ok(Self {
            not_set_up: None,
            named,
            asked: Some(asked),
            thread: Some(thread),
            tell: None,
        })
    }

    /// Who answers, for the terminal alone: [`None`] when answers are not
    /// set up.
    pub fn named(&self) -> Option<&str> {
        self.named.as_deref()
    }

    /// Ask for the answer to `words`, the search numbered `number`, from
    /// the first [`MOST_SOURCES`] of `found`. Returns at once: what came of
    /// it is told to the window as [`Came::Answer`].
    pub fn ask(&self, number: u64, words: &str, found: &[Found]) {
        let sources: Vec<Found> = found
            .iter()
            .take(MOST_SOURCES)
            // Its mark is a picture, never sent anywhere: the words alone.
            .map(|site| Found {
                mark: None,
                ..site.clone()
            })
            .collect();
        match (&self.asked, &self.not_set_up, &self.tell) {
            (Some(asked), _, _) => {
                // A thread that has gone is a window that is closing.
                let _ = asked.send(Work::Ask(number, words.to_owned(), sources));
            }
            (None, Some(why), Some(tell)) => {
                let _ = tell(told(
                    number,
                    words,
                    Answer::Unanswered(not_set_up(why)),
                    None,
                ));
            }
            _ => {}
        }
    }

    /// Stop the thread, after the answer it is asking, and wait for it:
    /// whether it ended rather than panicked.
    pub fn finish(mut self) -> bool {
        if let Some(asked) = self.asked.take() {
            let _ = asked.send(Work::Finish);
        }
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

impl Drop for Answerer {
    /// Never finished: the thread is told to end, and is not waited for.
    fn drop(&mut self) {
        if let Some(asked) = self.asked.take() {
            let _ = asked.send(Work::Finish);
        }
    }
}

/// What an answer is told when answers are not set up, and why.
pub fn not_set_up(why: &str) -> String {
    format!("alo\u{2019}s answers are not set up yet: {why}.")
}

/// What the terminal is told of the answer to `words`, written `by` (*`OpenAI`'s
/// gpt-6.1-terra*, [`Answerer::named`]), which cost `spent`: who wrote it
/// from how many sites, that none reliable was found, or why not — nothing
/// while it is asked, or never was.
pub fn said(by: &str, words: &str, answer: &Answer, spent: Option<Spent>) -> Option<String> {
    let cost = spent.map_or_else(String::new, |spent| {
        format!(" ({} tokens in, {} out)", spent.input, spent.output)
    });
    match answer {
        Answer::Written(written) => Some(format!(
            "{by} answered \u{201c}{words}\u{201d} from {}{cost}",
            alo_renderer::results_answer::sites(written.used().len())
        )),
        Answer::NotFound => Some(format!(
            "{by} found no reliable answer to \u{201c}{words}\u{201d} in its sites{cost}"
        )),
        Answer::Unanswered(why) => Some(why.clone()),
        Answer::NotAsked | Answer::Asking => None,
    }
}

/// What the window is told of the answer to the search `number` for
/// `words`.
fn told(number: u64, words: &str, answer: Answer, spent: Option<Spent>) -> Searched {
    Searched {
        number,
        words: words.to_owned(),
        came: Came::Answer { answer, spent },
    }
}

/// Ask `provider` for each answer `asking` sends — of those waiting, only
/// the newest — telling the window what came of each, until told to finish.
fn answer(provider: &AiService, asking: &Receiver<Work>, tell: &TellSearched) {
    let mut pool: Option<Pool> = None;
    let mut identities = Identities::default();
    loop {
        let Ok(first) = asking.recv() else {
            return;
        };
        let waiting: Vec<Work> = core::iter::once(first).chain(asking.try_iter()).collect();
        if waiting.iter().any(|work| matches!(work, Work::Finish)) {
            return;
        }
        let mut asks: Vec<(u64, String, Vec<Found>)> = waiting
            .into_iter()
            .filter_map(|work| match work {
                Work::Ask(number, words, sources) => Some((number, words, sources)),
                Work::Finish => None,
            })
            .collect();
        let Some((number, words, sources)) = asks.pop() else {
            continue;
        };
        for (older, words, _) in asks {
            let passed = Answer::Unanswered(format!(
                "alo did not ask {AI_SERVICE} to answer \u{201c}{words}\u{201d}: a newer search \
                 came first."
            ));
            if !tell(told(older, &words, passed, None)) {
                return;
            }
        }
        if pool.is_none() {
            match Pool::from_this_machine() {
                Ok(made) => pool = Some(made.patient_for(PATIENCE)),
                Err(why) => {
                    let why = format!(
                        "alo could not write an answer: {AI_SERVICE} could not be reached: this \
                         Mac's certificates could not be read ({why})"
                    );
                    if !tell(told(number, &words, Answer::Unanswered(why), None)) {
                        return;
                    }
                    continue;
                }
            }
        }
        let Some(pool) = pool.as_mut() else {
            continue;
        };
        let asked = provider.answer(pool, &mut identities, &words, &sources);
        if !tell(told(number, &words, asked.answer, asked.spent)) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(n: usize) -> Vec<Found> {
        (0..n)
            .map(|n| {
                Found::new(
                    "t",
                    alo_url::parse(&format!("https://s{n}.example/")).unwrap(),
                    "",
                )
                .marked(vec![0x89, b'P', b'N', b'G'])
            })
            .collect()
    }

    #[test]
    fn with_no_key_an_answer_asked_for_says_why_at_once() {
        let (tell, told) = channel();
        let answerer = Answerer::start(
            Err("add a key for the AI service in Settings".to_owned()),
            Box::new(move |searched| tell.send(searched).is_ok()),
        )
        .unwrap();
        answerer.ask(3, "x", &found(2));
        assert_eq!(
            told.try_recv().ok(),
            Some(Searched {
                number: 3,
                words: "x".to_owned(),
                came: Came::Answer {
                    answer: Answer::Unanswered(
                        "alo\u{2019}s answers are not set up yet: add a key for the AI \
                         service in Settings."
                            .to_owned()
                    ),
                    spent: None,
                },
            })
        );
        assert!(answerer.finish());
    }

    #[test]
    fn a_thread_set_up_ends_when_told_to() {
        let (tell, _told) = channel();
        let provider = crate::openai_answer::Provider::at(
            "http://127.0.0.1:9/v1/responses",
            crate::search_key::Key::of("not-a-real-key").unwrap(),
        )
        .unwrap();
        let answerer = Answerer::start(
            Ok(provider.into()),
            Box::new(move |searched| tell.send(searched).is_ok()),
        )
        .unwrap();
        assert!(answerer.finish());
    }
}

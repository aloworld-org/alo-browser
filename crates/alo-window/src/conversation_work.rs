/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent at work on the card's page, as the conversation holds it
//! (queue item 704, ADR 0096): whether it can be asked, which run is at
//! work and which question it asks the person, by the browser process's
//! own numbers — never a renderer's — and what each run said, put on the
//! live card ([`alo_renderer::card::Card::agent`]) for the first screen to
//! draw.
//!
//! # Set up only on a card
//!
//! The agent works on the page open as a card, and nowhere else in this
//! cut: asking alo is set up while a card is open and the model can be
//! asked, and says why not otherwise — no key, or no page open (an agent
//! that opens its own page is queue item 821).
//!
//! # A run's words, kept to its card
//!
//! What a run says is put on the card it was started on, and only while it
//! is the run at work: a run stopped — by the person, by a press on the
//! page, by its card going — is said to be stopped at once, here, and
//! anything it says after is not shown.
//!
//! # Taken over, and carried on
//!
//! A person's press on the page while a run works takes the page over
//! (queue item 825): the run is held, not ended — [`Work::pause`] — its
//! card saying so with *Let alo continue*, and [`Work::carry_on`] sets it
//! acting again. While held, only how it ended — stopped, by its limit or
//! the person — and its plan are put on its card: a step it was about to
//! say when the page was taken over is not done, and not drawn.
//!
//! # And kept with its page
//!
//! Every run is kept on its card as the chat keeps it
//! ([`alo_renderer::card::Card::ran`], queue item 820) — what was asked,
//! its steps in words, how it ended, and never a text typed that the
//! person did not say ([`crate::agent_kept`]) — made again from what was
//! heard each time the run says something, so the chat's file holds it as
//! it stands. Until it ends it is kept as alo closing would stop it; one
//! replaced by a newer run is kept as stopped by it ([`REPLACED`]). At
//! most [`MOST_RUNS`] are kept with one page, the newest.

use crate::agent_kept::{self, Heard};
use crate::conversation_agent::{AgentSaid, Said};
use alo_renderer::agent_plan::Plan;
use alo_renderer::agent_ran::{Ended, MOST_RUNS, Ran};
use alo_renderer::agent_work::{AtWork, MOST_STEPS, Now, Step};
use alo_renderer::card::Card;
use alo_renderer::results::SetUp;

/// What a run is kept as having come to when a newer one replaced it at
/// work.
pub const REPLACED: &str = "You asked alo something else, so it stopped.";

/// What a person is told when asking alo needs a page open.
pub const NO_PAGE: &str = "open a page first: alo works on the page open in the conversation";

/// The agent, as the conversation holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// Whether the model can be asked, and why not.
    set_up: SetUp,
    /// How many runs were started: the newest's number.
    runs: u64,
    /// The run at work on the card, if one is.
    run: Option<u64>,
    /// The question that run asks the person now, by its number.
    question: Option<u64>,
    /// A run started here and not yet handed to the window: its number,
    /// and what the person asked.
    to_start: Option<(u64, String)>,
    /// What the person asked the run at work, and each step as it was
    /// heard, what it typed with it: what its card's kept run is made
    /// from (queue item 820).
    keeping: (String, Vec<Heard>),
    /// Its plan as it stands, kept with the run (queue item 824).
    plan: Option<Plan>,
}

impl Default for Work {
    /// Not set up, for no reason given; no run.
    fn default() -> Self {
        Self {
            set_up: SetUp::Not(String::new()),
            runs: 0,
            run: None,
            question: None,
            to_start: None,
            keeping: (String::new(), Vec::new()),
            plan: None,
        }
    }
}

impl Work {
    /// Whether the model can be asked is now `set_up`.
    pub fn set_up(&mut self, set_up: SetUp) {
        self.set_up = set_up;
    }

    /// Whether asking alo can be done, there being a card or not: only
    /// with one, and the model set up.
    pub fn asking(&self, card: bool) -> SetUp {
        match &self.set_up {
            SetUp::Ready if card => SetUp::Ready,
            SetUp::Ready => SetUp::Not(NO_PAGE.to_owned()),
            not @ SetUp::Not(_) => not.clone(),
        }
    }

    /// Start a run on `card` for what the person `asked`: whether it
    /// started, which it does only when asking is set up. A run at work
    /// before it is stopped first, said so on the card.
    pub fn start(&mut self, card: &mut Card, asked: &str) -> bool {
        if self.asking(true) != SetUp::Ready {
            return false;
        }
        if self.at_work(Some(card)).is_some() {
            self.keep(
                card,
                Some(&Ended::Stopped {
                    why: REPLACED.to_owned(),
                }),
            );
        }
        self.runs += 1;
        self.run = Some(self.runs);
        self.question = None;
        self.to_start = Some((self.runs, asked.to_owned()));
        card.agent = Some(AtWork::starting(asked));
        self.keeping = (asked.to_owned(), Vec::new());
        self.plan = None;
        card.ran.push(Ran::starting(asked));
        if card.ran.len() > MOST_RUNS {
            card.ran.remove(0);
        }
        true
    }

    /// The run at work kept on `card` as it stands — ended as `ended`, or
    /// still at work — in place of the newest kept there.
    fn keep(&self, card: &mut Card, ended: Option<&Ended>) {
        let (asked, heard) = &self.keeping;
        if let Some(newest) = card.ran.last_mut() {
            *newest = agent_kept::kept(asked, heard, self.plan.as_ref(), ended);
        }
    }

    /// The run started and not yet handed to the window, once.
    pub fn take_start(&mut self) -> Option<(u64, String)> {
        self.to_start.take()
    }

    /// The run at work on `card`, if one is: what *Stop* stops.
    pub fn working(&self, card: Option<&Card>) -> Option<u64> {
        let working = card
            .and_then(|card| card.agent.as_ref())
            .is_some_and(AtWork::is_working);
        self.run.filter(|_| working)
    }

    /// The run held on `card` while its page is the person's, if one is:
    /// what *Let alo continue* carries on (queue item 825).
    pub fn paused(&self, card: Option<&Card>) -> Option<u64> {
        let paused = card
            .and_then(|card| card.agent.as_ref())
            .is_some_and(AtWork::is_paused);
        self.run.filter(|_| paused)
    }

    /// The run on `card` the window keeps a thread for: at work, or held
    /// while the page is the person's.
    pub fn at_work(&self, card: Option<&Card>) -> Option<u64> {
        self.working(card).or_else(|| self.paused(card))
    }

    /// The person took the page over (queue item 825): the run at work on
    /// `card` is held, its question dropped, its card saying so with *Let
    /// alo continue*. Its number, if one was at work.
    pub fn pause(&mut self, card: Option<&mut Card>) -> Option<u64> {
        let card = card?;
        let run = self.working(Some(card))?;
        if let Some(work) = card.agent.as_mut() {
            work.now = Now::Paused;
            work.choosing = None;
        }
        self.question = None;
        Some(run)
    }

    /// *Let alo continue* for the run numbered `run`: held on `card`, it
    /// acts again. Its number, if it was the one held.
    pub fn carry_on(&mut self, card: Option<&mut Card>, run: u64) -> Option<u64> {
        let card = card?;
        let run = self.paused(Some(card)).filter(|held| *held == run)?;
        if let Some(work) = card.agent.as_mut() {
            work.now = Now::Acting;
        }
        Some(run)
    }

    /// The question asked of the person now, if one is: what *Allow* and
    /// *Not now* answer.
    pub fn asking_now(&self, card: Option<&Card>) -> Option<u64> {
        let asking = card
            .and_then(|card| card.agent.as_ref())
            .is_some_and(|work| matches!(work.now, Now::Asking { .. }));
        self.question
            .filter(|_| asking && self.working(card).is_some())
    }

    /// The choices a stuck run offers on `card`.
    pub fn choices(card: Option<&Card>) -> Vec<String> {
        match card
            .and_then(|card| card.agent.as_ref())
            .map(|work| &work.now)
        {
            Some(Now::Stuck { choices, .. }) => choices.clone(),
            _ => Vec::new(),
        }
    }

    /// What the run said, put on `card` when it is the run at work there:
    /// whether the card shows something else.
    pub fn heard(&mut self, card: Option<&mut Card>, said: AgentSaid) -> bool {
        let Some(card) = card else {
            return false;
        };
        let Some(work) = card.agent.as_mut() else {
            return false;
        };
        if self.run != Some(said.run) || !(work.is_working() || work.is_paused()) {
            return false;
        }
        // Held while the page is the person's: only how it ended and its
        // plan are its card's (queue item 825).
        if work.is_paused() && !(said.said.ends() || matches!(said.said, Said::Planned(_))) {
            return false;
        }
        let ended = match &said.said {
            Said::Stuck { why, .. } => Some(Ended::Stuck { why: why.clone() }),
            Said::Done { result } => Some(Ended::Done {
                said: result.clone(),
            }),
            Said::Stopped { why } => Some(Ended::Stopped { why: why.clone() }),
            Said::Step { .. }
            | Said::Choosing(_)
            | Said::Asking { .. }
            | Said::Allowed
            | Said::Planned(_)
            | Said::Continued { .. }
            | Said::Spent(_) => None,
        };
        match said.said {
            Said::Step { line, why, typed } => {
                if work.steps.len() < MOST_STEPS {
                    self.keeping.1.push(Heard {
                        line: line.clone(),
                        why: why.clone(),
                        typed,
                    });
                    work.steps.push(Step { line, why });
                }
                work.now = Now::Acting;
                work.choosing = None;
                self.question = None;
            }
            // The closed list the step chooses in (queue item 810), drawn
            // under it while it is under way.
            Said::Choosing(choosing) => work.choosing = Some(choosing),
            Said::Asking {
                question,
                what,
                paying,
            } => {
                work.now = Now::Asking {
                    what,
                    paying: paying.map(Box::new),
                };
                self.question = Some(question);
            }
            // Its plan as it stands (queue item 824): drawn, and kept.
            Said::Planned(plan) => {
                work.plan = Some(plan.clone());
                self.plan = Some(plan);
            }
            Said::Allowed => {
                work.now = Now::Acting;
                self.question = None;
            }
            Said::Stuck { why, choices } => {
                work.now = Now::Stuck { why, choices };
                self.ended();
            }
            Said::Done { result } => {
                work.now = Now::Done { said: result };
                self.ended();
            }
            Said::Stopped { why } => {
                work.now = Now::Stopped { why };
                self.ended();
            }
            Said::Spent(_) | Said::Continued { .. } => return false,
        }
        self.keep(card, ended.as_ref());
        true
    }

    /// Stop the run at work on `card`, said there as `why`: its number, if
    /// one was at work.
    pub fn stop(&mut self, card: Option<&mut Card>, why: &str) -> Option<u64> {
        let run = self.run?;
        if let Some(card) = card
            && card
                .agent
                .as_ref()
                .is_some_and(|work| work.is_working() || work.is_paused())
        {
            if let Some(work) = card.agent.as_mut() {
                work.now = Now::Stopped {
                    why: why.to_owned(),
                };
            }
            self.keep(
                card,
                Some(&Ended::Stopped {
                    why: why.to_owned(),
                }),
            );
        }
        self.ended();
        Some(run)
    }

    /// No run at work any more.
    fn ended(&mut self) {
        self.run = None;
        self.question = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card::new("a page", alo_url::parse("https://example.com/").unwrap())
    }

    fn said(run: u64, said: Said) -> AgentSaid {
        AgentSaid { run, said }
    }

    #[test]
    fn asking_is_set_up_only_with_a_card_and_the_model() {
        let mut work = Work::default();
        assert_eq!(work.asking(true), SetUp::Not(String::new()));
        let mut card = card();
        assert!(!work.start(&mut card, "x"));
        work.set_up(SetUp::Ready);
        assert_eq!(work.asking(true), SetUp::Ready);
        assert_eq!(work.asking(false), SetUp::Not(NO_PAGE.to_owned()));
        assert!(work.start(&mut card, "send it"));
        assert_eq!(work.take_start(), Some((1, "send it".to_owned())));
        assert_eq!(work.take_start(), None);
        assert_eq!(work.working(Some(&card)), Some(1));
    }

    #[test]
    fn a_runs_words_go_on_its_card_and_a_question_is_numbered() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "send it"));
        let step = Said::Step {
            line: "Pressing Send".to_owned(),
            why: String::new(),
            typed: None,
        };
        assert!(work.heard(Some(&mut card), said(1, step.clone())));
        assert!(work.heard(
            Some(&mut card),
            said(
                1,
                Said::Asking {
                    question: 4,
                    what: "Press Send?".to_owned(),
                    paying: None,
                }
            )
        ));
        assert_eq!(work.asking_now(Some(&card)), Some(4));
        // Another run's words are not this card's.
        assert!(!work.heard(Some(&mut card), said(2, step)));
        assert!(work.heard(Some(&mut card), said(1, Said::Allowed)));
        assert_eq!(work.asking_now(Some(&card)), None);
        assert!(work.heard(
            Some(&mut card),
            said(
                1,
                Said::Done {
                    result: "Sent.".to_owned()
                }
            )
        ));
        assert_eq!(work.working(Some(&card)), None);
        let agent = card.agent.clone().unwrap_or_else(|| AtWork::starting(""));
        assert_eq!(agent.steps.len(), 1);
        assert_eq!(
            agent.now,
            Now::Done {
                said: "Sent.".to_owned()
            }
        );
    }

    #[test]
    fn a_run_stopped_is_said_at_once_and_says_nothing_after() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "send it"));
        assert_eq!(work.stop(Some(&mut card), "You stopped alo."), Some(1));
        assert_eq!(work.stop(Some(&mut card), "again"), None);
        assert!(!work.heard(
            Some(&mut card),
            said(
                1,
                Said::Done {
                    result: "Sent.".to_owned()
                }
            )
        ));
        assert_eq!(
            card.agent.map(|agent| agent.now),
            Some(Now::Stopped {
                why: "You stopped alo.".to_owned()
            })
        );
    }

    #[test]
    fn every_run_is_kept_with_its_page_as_it_stands_and_never_what_was_typed() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "Sign me up"));
        assert_eq!(card.ran, vec![Ran::starting("Sign me up")]);
        let typing = Said::Step {
            line: "Typing ada@example.com into Email".to_owned(),
            why: String::new(),
            typed: Some("ada@example.com".to_owned()),
        };
        assert!(work.heard(Some(&mut card), said(1, typing)));
        // Shown on the card as the model wrote it; kept as alo's words.
        assert_eq!(
            card.agent
                .as_ref()
                .map(|agent| agent.steps[0].line.as_str()),
            Some("Typing ada@example.com into Email")
        );
        assert_eq!(card.ran[0].steps[0].line, alo_renderer::agent_ran::TYPED);
        assert_eq!(
            card.ran[0].ended,
            Ended::Stopped {
                why: alo_renderer::agent_ran::CLOSED.to_owned()
            },
            "until it ends, as alo closing would stop it"
        );
        assert!(work.heard(
            Some(&mut card),
            said(
                1,
                Said::Done {
                    result: "Signed up ada@example.com.".to_owned()
                }
            )
        ));
        assert_eq!(
            card.ran[0].ended,
            Ended::Done {
                said: "Signed up \u{2026}.".to_owned()
            }
        );
        // A second run stopped by the person; a third replacing a fourth.
        assert!(work.start(&mut card, "Again"));
        assert_eq!(work.stop(Some(&mut card), "You stopped alo."), Some(2));
        assert_eq!(
            card.ran[1].ended,
            Ended::Stopped {
                why: "You stopped alo.".to_owned()
            }
        );
        assert!(work.start(&mut card, "Third"));
        assert!(work.start(&mut card, "Fourth"));
        assert_eq!(
            card.ran[2].ended,
            Ended::Stopped {
                why: REPLACED.to_owned()
            }
        );
        assert_eq!(card.ran.len(), 4);
        for more in 0..MOST_RUNS {
            assert!(work.start(&mut card, &format!("more {more}")));
        }
        assert_eq!(card.ran.len(), MOST_RUNS, "the newest kept");
        assert_eq!(card.ran[0].asked, "more 0");
    }

    #[test]
    fn taken_over_the_run_is_held_its_question_dropped_and_carried_on_after() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "pay for it"));
        assert!(work.heard(
            Some(&mut card),
            said(
                1,
                Said::Asking {
                    question: 1,
                    what: "Press Pay?".to_owned(),
                    paying: None,
                }
            )
        ));
        assert_eq!(work.pause(Some(&mut card)), Some(1));
        assert_eq!(work.pause(Some(&mut card)), None, "held once");
        assert_eq!(
            card.agent.as_ref().map(|agent| &agent.now),
            Some(&Now::Paused)
        );
        assert_eq!(work.asking_now(Some(&card)), None, "its question dropped");
        assert_eq!(work.working(Some(&card)), None, "the page the person's");
        assert_eq!(work.paused(Some(&card)), Some(1));
        assert_eq!(work.at_work(Some(&card)), Some(1), "its thread kept");
        // A step it was about to say is not its card's while held; how it
        // ends is.
        let step = Said::Step {
            line: "Pressing Pay".to_owned(),
            why: String::new(),
            typed: None,
        };
        assert!(!work.heard(Some(&mut card), said(1, step)));
        assert_eq!(work.carry_on(Some(&mut card), 2), None, "another run's");
        assert_eq!(work.carry_on(Some(&mut card), 1), Some(1));
        assert_eq!(work.carry_on(Some(&mut card), 1), None);
        assert_eq!(work.working(Some(&card)), Some(1));
        assert_eq!(work.pause(Some(&mut card)), Some(1));
        assert!(work.heard(
            Some(&mut card),
            said(
                1,
                Said::Stopped {
                    why: "alo stopped: the page was yours for 10 minutes.".to_owned()
                }
            )
        ));
        assert_eq!(work.at_work(Some(&card)), None);
        // Stopped while held: by the card closing, another page, a new ask.
        assert!(work.start(&mut card, "again"));
        assert_eq!(work.pause(Some(&mut card)), Some(2));
        assert_eq!(
            work.stop(Some(&mut card), "alo stopped: you opened another page."),
            Some(2)
        );
        assert!(matches!(
            card.agent.as_ref().map(|agent| &agent.now),
            Some(Now::Stopped { .. })
        ));
    }

    #[test]
    fn its_plan_is_drawn_and_kept_with_the_run() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "find it"));
        let plan = Plan {
            steps: vec!["Search".to_owned(), "Open it".to_owned()],
            on: 0,
        };
        assert!(work.heard(Some(&mut card), said(1, Said::Planned(plan.clone()))));
        assert_eq!(
            card.agent.as_ref().and_then(|agent| agent.plan.clone()),
            Some(plan.clone())
        );
        assert_eq!(card.ran[0].plan, Some(plan));
        // A new run begins with none.
        assert!(work.start(&mut card, "again"));
        assert_eq!(card.ran[1].plan, None);
    }

    #[test]
    fn a_stuck_runs_choices_are_what_a_press_chooses_among() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "x"));
        let stuck = Said::Stuck {
            why: "No form.".to_owned(),
            choices: vec!["Look for one".to_owned()],
        };
        assert!(work.heard(Some(&mut card), said(1, stuck)));
        assert_eq!(Work::choices(Some(&card)), vec!["Look for one".to_owned()]);
        assert_eq!(work.working(Some(&card)), None);
    }
}

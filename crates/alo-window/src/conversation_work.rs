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
//! # On a card, or with no page open
//!
//! Asking alo is set up while the model can be asked, and says why not
//! otherwise — no key. With a card open the run works on its page; with
//! none (queue item 821) it is held in a turn of its own,
//! [`alo_renderer::agent_asked::Asked`], until it opens a page — and then
//! on that page's card, moved there as the conversation opens it. Either
//! holds a run the same way ([`Holds`]).
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
//!
//! # A press from assistive technology that pays
//!
//! ADR 0115, *Answered* 1 (queue item 988): a press from the platform's
//! accessibility bus on a card's page that the person's gate says pays is
//! held for the person's yes as alo's agent's is — the same question, with
//! what it pays, *Pay* and *Not now*, on the same card ([`Work::hold`]). It
//! is no run: nothing is started, nothing is kept with the page, and a run
//! at work is never interrupted by it. Its question is numbered down from
//! the top of the numbers, so it is never a run's, which count up from one.

use crate::agent_kept::{self, Heard};
use crate::conversation_agent::{AgentSaid, Said};
use alo_renderer::agent_asked::Asked;
use alo_renderer::agent_plan::Plan;
use alo_renderer::agent_ran::{Ended, MOST_RUNS, Ran};
use alo_renderer::agent_work::{AtWork, MOST_STEPS, Now, Step};
use alo_renderer::card::Card;
use alo_renderer::results::SetUp;

/// What a run is kept as having come to when a newer one replaced it at
/// work.
pub const REPLACED: &str = "You asked alo something else, so it stopped.";

/// What holds a run of the agent: the live card, or a turn asked with no
/// page open (queue item 821).
pub trait Holds {
    /// The agent at work there, if it is.
    fn agent(&self) -> Option<&AtWork>;
    /// The same, to change.
    fn agent_mut(&mut self) -> &mut Option<AtWork>;
    /// The runs kept there, oldest first.
    fn ran_mut(&mut self) -> &mut Vec<Ran>;
}

impl Holds for Card {
    fn agent(&self) -> Option<&AtWork> {
        self.agent.as_ref()
    }

    fn agent_mut(&mut self) -> &mut Option<AtWork> {
        &mut self.agent
    }

    fn ran_mut(&mut self) -> &mut Vec<Ran> {
        &mut self.ran
    }
}

impl Holds for Asked {
    fn agent(&self) -> Option<&AtWork> {
        self.agent.as_ref()
    }

    fn agent_mut(&mut self) -> &mut Option<AtWork> {
        &mut self.agent
    }

    fn ran_mut(&mut self) -> &mut Vec<Ran> {
        &mut self.ran
    }
}

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
    /// A press from assistive technology held for the person's yes (queue
    /// item 988), and how many were.
    held: (Option<HeldForTheBus>, u64),
}

/// A press from assistive technology held on the card for the person's
/// yes (ADR 0115, *Answered* 1).
#[derive(Debug, Clone, PartialEq, Eq)]
struct HeldForTheBus {
    /// Its question's number.
    question: u64,
    /// What the card showed of alo's agent before it was asked, put back
    /// once it is answered.
    before: Option<AtWork>,
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
            held: (None, 0),
        }
    }
}

impl Work {
    /// Whether the model can be asked is now `set_up`.
    pub fn set_up(&mut self, set_up: SetUp) {
        self.set_up = set_up;
    }

    /// Whether asking alo can be done: the model set up — with a card open
    /// or none (queue item 821).
    pub fn asking(&self) -> SetUp {
        self.set_up.clone()
    }

    /// Start a run on `card` — a live card, or a turn asked with no page —
    /// for what the person `asked`: whether it started, which it does only
    /// when asking is set up. A run at work before it is stopped first,
    /// said so on the card.
    pub fn start<H: Holds + ?Sized>(&mut self, card: &mut H, asked: &str) -> bool {
        if self.asking() != SetUp::Ready {
            return false;
        }
        if self.at_work(Some(&*card)).is_some() {
            self.keep(
                card,
                Some(&Ended::Stopped {
                    why: REPLACED.to_owned(),
                }),
            );
        }
        // A press held for the bus is not done once a run replaces it.
        self.held.0 = None;
        self.runs += 1;
        self.run = Some(self.runs);
        self.question = None;
        self.to_start = Some((self.runs, asked.to_owned()));
        *card.agent_mut() = Some(AtWork::starting(asked));
        self.keeping = (asked.to_owned(), Vec::new());
        self.plan = None;
        let ran = card.ran_mut();
        ran.push(Ran::starting(asked));
        if ran.len() > MOST_RUNS {
            ran.remove(0);
        }
        true
    }

    /// The run at work kept on `card` as it stands — ended as `ended`, or
    /// still at work — in place of the newest kept there.
    fn keep<H: Holds + ?Sized>(&self, card: &mut H, ended: Option<&Ended>) {
        let (asked, heard) = &self.keeping;
        if let Some(newest) = card.ran_mut().last_mut() {
            *newest = agent_kept::kept(asked, heard, self.plan.as_ref(), ended);
        }
    }

    /// The run started and not yet handed to the window, once.
    pub fn take_start(&mut self) -> Option<(u64, String)> {
        self.to_start.take()
    }

    /// The run at work on `card`, if one is: what *Stop* stops.
    pub fn working<H: Holds + ?Sized>(&self, card: Option<&H>) -> Option<u64> {
        let working = card
            .and_then(|card| card.agent())
            .is_some_and(AtWork::is_working);
        self.run.filter(|_| working)
    }

    /// The run held on `card` while its page is the person's, if one is:
    /// what *Let alo continue* carries on (queue item 825).
    pub fn paused<H: Holds + ?Sized>(&self, card: Option<&H>) -> Option<u64> {
        let paused = card
            .and_then(|card| card.agent())
            .is_some_and(AtWork::is_paused);
        self.run.filter(|_| paused)
    }

    /// The run on `card` the window keeps a thread for: at work, or held
    /// while the page is the person's.
    pub fn at_work<H: Holds + ?Sized>(&self, card: Option<&H>) -> Option<u64> {
        self.working(card).or_else(|| self.paused(card))
    }

    /// The person took the page over (queue item 825): the run at work on
    /// `card` is held, its question dropped, its card saying so with *Let
    /// alo continue*. Its number, if one was at work.
    pub fn pause<H: Holds + ?Sized>(&mut self, card: Option<&mut H>) -> Option<u64> {
        self.hand_over(card, Now::Paused)
    }

    /// The page the run reached asks the person to sign in (queue item
    /// 821): the run is held as when they take the page over, its card
    /// saying so with *Let alo continue*. Its number, if one was at work.
    pub fn sign_in<H: Holds + ?Sized>(&mut self, card: Option<&mut H>) -> Option<u64> {
        self.hand_over(card, Now::SignIn)
    }

    /// The run at work on `card` held, the page the person's, saying
    /// `now`: its number, if one was at work.
    fn hand_over<H: Holds + ?Sized>(&mut self, card: Option<&mut H>, now: Now) -> Option<u64> {
        let card = card?;
        let run = self.working(Some(&*card))?;
        if let Some(work) = card.agent_mut().as_mut() {
            work.now = now;
            work.choosing = None;
        }
        self.question = None;
        Some(run)
    }

    /// *Let alo continue* for the run numbered `run`: held on `card`, it
    /// acts again. Its number, if it was the one held.
    pub fn carry_on<H: Holds + ?Sized>(&mut self, card: Option<&mut H>, run: u64) -> Option<u64> {
        let card = card?;
        let run = self.paused(Some(&*card)).filter(|held| *held == run)?;
        if let Some(work) = card.agent_mut().as_mut() {
            work.now = Now::Acting;
        }
        Some(run)
    }

    /// The question asked of the person now, if one is: what *Allow* and
    /// *Not now* answer.
    pub fn asking_now<H: Holds + ?Sized>(&self, card: Option<&H>) -> Option<u64> {
        let asking = card
            .and_then(|card| card.agent())
            .is_some_and(|work| matches!(work.now, Now::Asking { .. }));
        self.question
            .filter(|_| asking && self.working(card).is_some())
            .or_else(|| self.held_question().filter(|_| asking))
    }

    /// Hold a press from assistive technology on `card` for the person's
    /// yes (ADR 0115, *Answered* 1): the card asks `what`, with what it
    /// pays, `asked` its line — what the bus pressed. Its question's
    /// number; none while a run of alo's agent is at work or held there,
    /// or another such press already waits.
    pub fn hold(
        &mut self,
        card: &mut Card,
        asked: &str,
        what: String,
        paying: Option<alo_renderer::agent_work::Paying>,
    ) -> Option<u64> {
        if self.at_work(Some(&*card)).is_some() || self.held.0.is_some() {
            return None;
        }
        self.held.1 += 1;
        let question = u64::MAX - self.held.1;
        let before = card.agent.take();
        card.agent = Some(AtWork {
            now: Now::Asking {
                what,
                paying: paying.map(Box::new),
            },
            ..AtWork::starting(asked)
        });
        self.held.0 = Some(HeldForTheBus { question, before });
        Some(question)
    }

    /// The question a press from assistive technology waits on, if one
    /// does.
    pub fn held_question(&self) -> Option<u64> {
        self.held.0.as_ref().map(|held| held.question)
    }

    /// The person answered the question `question` — *Allow*, *Not now*,
    /// *Stop* — or the card it was asked on went: whether it was the one a
    /// press from assistive technology waits on, which then waits no more,
    /// and `card`, if it is still there, shows what it showed of alo's
    /// agent before.
    pub fn answer_held(&mut self, card: Option<&mut Card>, question: u64) -> bool {
        if self.held_question() != Some(question) {
            return false;
        }
        if let Some(held) = self.held.0.take()
            && let Some(card) = card
        {
            card.agent = held.before;
        }
        true
    }

    /// The choices a stuck run offers on `card`.
    pub fn choices<H: Holds + ?Sized>(card: Option<&H>) -> Vec<String> {
        match card.and_then(|card| card.agent()).map(|work| &work.now) {
            Some(Now::Stuck { choices, .. }) => choices.clone(),
            _ => Vec::new(),
        }
    }

    /// What the run said, put on `card` when it is the run at work there:
    /// whether the card shows something else.
    pub fn heard<H: Holds + ?Sized>(&mut self, card: Option<&mut H>, said: AgentSaid) -> bool {
        let Some(card) = card else {
            return false;
        };
        let Some(work) = card.agent_mut().as_mut() else {
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
            | Said::Opening { .. }
            | Said::SignIn
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
            // The page it reached asks the person to sign in (queue item
            // 821): handed over, as when they take it over.
            Said::SignIn => {
                work.now = Now::SignIn;
                work.choosing = None;
                self.question = None;
            }
            // A page it opens is the window's to open (queue item 821).
            Said::Spent(_) | Said::Continued { .. } | Said::Opening { .. } => return false,
        }
        self.keep(card, ended.as_ref());
        true
    }

    /// Stop the run at work on `card`, said there as `why`: its number, if
    /// one was at work.
    pub fn stop<H: Holds + ?Sized>(&mut self, card: Option<&mut H>, why: &str) -> Option<u64> {
        let run = self.run?;
        if let Some(card) = card
            && card
                .agent()
                .is_some_and(|work| work.is_working() || work.is_paused())
        {
            if let Some(work) = card.agent_mut().as_mut() {
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
    fn asking_is_set_up_with_the_model_on_a_card_or_with_no_page() {
        let mut work = Work::default();
        assert_eq!(work.asking(), SetUp::Not(String::new()));
        let mut card = card();
        assert!(!work.start(&mut card, "x"));
        work.set_up(SetUp::Ready);
        assert_eq!(work.asking(), SetUp::Ready);
        // With no page open (queue item 821): held in a turn of its own.
        let mut asked = Asked::of("find the museum's hours");
        assert!(work.start(&mut asked, "find the museum's hours"));
        assert_eq!(
            work.take_start(),
            Some((1, "find the museum's hours".to_owned()))
        );
        assert_eq!(work.working(Some(&asked)), Some(1));
        assert_eq!(asked.ran.len(), 1);
        assert!(work.start(&mut card, "send it"));
        assert_eq!(work.take_start(), Some((2, "send it".to_owned())));
        assert_eq!(work.take_start(), None);
        assert_eq!(work.working(Some(&card)), Some(2));
    }

    #[test]
    fn a_sign_in_page_hands_the_run_over_and_it_carries_on_after() {
        // Queue item 821.
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "read my orders"));
        assert!(work.heard(Some(&mut card), said(1, Said::SignIn)));
        assert_eq!(
            card.agent.as_ref().map(|agent| &agent.now),
            Some(&Now::SignIn)
        );
        assert_eq!(work.working(Some(&card)), None, "the page the person's");
        assert_eq!(work.paused(Some(&card)), Some(1));
        assert_eq!(work.carry_on(Some(&mut card), 1), Some(1));
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

    #[test]
    fn a_press_from_the_bus_that_pays_is_asked_on_the_card_and_put_back_once_answered() {
        let mut work = Work::default();
        let mut card = card();
        let paying = alo_renderer::agent_work::Paying {
            amount: Some("UGX 45,000".to_owned()),
            merchant: "Kampala Coffee (shop.example)".to_owned(),
            lines: Vec::new(),
        };
        // Held with no model set up: it is no run.
        let question = work
            .hold(
                &mut card,
                "Assistive technology pressed Pay",
                "Pay UGX 45,000?".to_owned(),
                Some(paying.clone()),
            )
            .expect("held");
        assert!(question > u64::MAX / 2, "never a run's number");
        assert_eq!(work.asking_now(Some(&card)), Some(question));
        assert!(matches!(
            card.agent.as_ref().map(|work| &work.now),
            Some(Now::Asking { paying: Some(shown), .. }) if **shown == paying
        ));
        assert_eq!(work.take_start(), None, "nothing started");
        assert!(card.ran.is_empty(), "nothing kept with the page");
        // A second waits for none.
        assert_eq!(work.hold(&mut card, "again", "Pay?".to_owned(), None), None);
        // Another question's answer answers nothing.
        assert!(!work.answer_held(Some(&mut card), question - 1));
        assert!(work.answer_held(Some(&mut card), question));
        assert_eq!(card.agent, None, "what it showed before");
        assert_eq!(work.asking_now(Some(&card)), None);
        assert!(!work.answer_held(Some(&mut card), question), "once");
    }

    #[test]
    fn a_press_from_the_bus_is_never_held_over_a_run_at_work() {
        let mut work = Work::default();
        work.set_up(SetUp::Ready);
        let mut card = card();
        assert!(work.start(&mut card, "book a table"));
        assert_eq!(
            work.hold(&mut card, "pressed", "Pay?".to_owned(), None),
            None
        );
    }
}

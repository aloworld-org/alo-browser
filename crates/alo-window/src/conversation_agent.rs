/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ The agent at work on a card's page (queue item 704, ADR 0096): a
//! thread of the browser process that asks `OpenAI` for one step at a time
//! and carries each out **through the same road an agent outside this
//! browser takes** — no other.
//!
//! # The same verbs, the same gate
//!
//! Every read and every verb is an [`Order::Agent`] sent to the
//! conversation's relay ([`crate::conversation_agents`]), the very orders
//! the agent socket sends to (ADR 0055): answered by the thread serving the
//! live card, kept to the sites a person named, refused by the same kinds,
//! and written into the same record, as [`Caller::ALO`]. There is no
//! privileged path: nothing here reaches Chrome, the tab or a renderer but
//! through those orders, and nothing a verb is given is a position (ADR
//! 0002) — the model names a node by the number the tree gave it.
//!
//! # One step
//!
//! 1. Read the page: the one tab the relay answers ([`Ask::Tabs`]), then
//!    its tree ([`Ask::Read`]).
//! 2. Ask the model ([`crate::agent_protocol`]) with what the person asked,
//!    the tree as text ([`crate::agent_page`]) and the steps so far; it
//!    answers with one tool call.
//! 3. A verb: the window is told the step's line, the page read again, and
//!    the verb judged by **the person's gate** ([`crate::agent_gate`]): a
//!    press that pays, sends, deletes or sends a form is asked of the person
//!    and waits for their press on *Allow* — *Not now* ends the run, and
//!    nothing is pressed. Then the verb is sent; what came of it is the
//!    step's outcome, told to the model next time.
//! 4. `done` or `stuck` ends the run, said in the model's words.
//!
//! # Its limits
//!
//! At most [`MOST_STEPS`] verbs, and [`MOST_TOKENS`] tokens as `OpenAI`'s
//! `usage` counts them; [`PATIENCE`] for each answer of the model's and
//! [`VERB_PATIENCE`] for each of the page's. Past any, the run stops and
//! says which.
//!
//! # Stop
//!
//! [`Run::stop`] — *Stop*, a press on the page (*click anywhere to take
//! over*), the card closed or another opened — is seen before every order
//! is sent and every model's answer acted on: a verb already sent is
//! finished by the page, and nothing after it is done. A model's answer in
//! flight is read and thrown away.
//!
//! # A password, and a card's details
//!
//! Never typed: the card refuses text put into a password field for every
//! agent (`chrome_act`, [`Refused::Password`]), and a card's number,
//! security code or expiry, typed or chosen, whatever the page names the
//! field ([`Refused::CardDetails`], queue item 826, ADR 0099); the run then
//! says it is stuck — signing in and paying details are the person's.
//!
//! # Its plan
//!
//! With its first action the model writes two to five short steps of the
//! whole task, and with each action the step of the plan it is for; a new
//! plan replaces the old (queue item 824). The window is told the plan as
//! it stands ([`Said::Planned`]), and the model is sent it with every
//! request, since no request names another.
//!
//! # Taken over, and carried on
//!
//! [`Run::pause`] — a person's press on the page while it works — holds the
//! run where [`Run::stop`] would end it: a verb already sent is finished by
//! the page, a model's answer in flight is thrown away, a question waiting
//! on the person is dropped, and the thread waits. [`Run::carry_on`] — *Let
//! alo continue* — reads the page anew and tells the model, as a step of
//! the person's, what they changed since the run last read it
//! ([`crate::agent_changes`]), then goes on: its steps and tokens counted
//! across the pause, the person's gate holding as before. Held past
//! [`MOST_PAUSE`], the run stops and says so (queue item 825).
//!
//! # Paying
//!
//! A press the gate holds because it pays is asked with what it pays,
//! read off the page by the browser process ([`crate::agent_paying`],
//! queue item 826), and pressed only after *Pay* — and only if the page,
//! read again, would ask exactly the same, amount and all.

use crate::agent_changes::{self, Reached};
use crate::agent_gate::{self, Question};
use crate::agent_page;
use crate::agent_protocol::{self, Asking, Call, PERSON, Taken};
use crate::agent_record::Caller;
use crate::answer_protocol::Spent;
use crate::asking::{Answer, Ask, Refused};
use crate::inbox::Orders;
use crate::message::Order;
use crate::openai_answer::{ANSWERER, Provider, why_unanswered};
use alo_agent::{Outcome, Target, Verb};
use alo_box::BoxId;
use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::Snapshot;
use alo_renderer::agent_paying::Paying;
use alo_renderer::agent_plan::Plan;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The most verbs one run carries out before it stops.
pub const MOST_STEPS: usize = 20;

/// The most tokens one run spends, sent and written, as `OpenAI` counts
/// them.
pub const MOST_TOKENS: u64 = 400_000;

/// How long the model is waited for, for one step.
pub const PATIENCE: Duration = Duration::from_secs(60);

/// How long the page is waited for, for one read or verb.
pub const VERB_PATIENCE: Duration = Duration::from_secs(90);

/// How long the page is given to go where a verb sent it before it is
/// read again.
pub const SETTLE: Duration = Duration::from_millis(1200);

/// How long a run is held while the page is the person's before it stops
/// (queue item 825).
pub const MOST_PAUSE: Duration = Duration::from_secs(10 * 60);

/// How often a thread waiting on the person looks whether it was stopped.
const LOOK: Duration = Duration::from_millis(50);

/// What the window is told of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSaid {
    /// The run's number, the window's own.
    pub run: u64,
    /// What it said.
    pub said: Said,
}

/// What a run says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A step begun: its line, and why when it chose.
    Step {
        /// What it is doing.
        line: String,
        /// Why; empty when it did not choose.
        why: String,
        /// What a typing step puts into its field: the browser process's
        /// own, kept out of the chat's file unless the person said it
        /// (queue item 820, [`crate::agent_kept`]), never drawn and never
        /// said on the terminal.
        typed: Option<String>,
    },
    /// It waits on the person: the question numbered so, and its words.
    Asking {
        /// The question's number, this run's own.
        question: u64,
        /// What the person is asked.
        what: String,
        /// What the press pays, when it pays (queue item 826).
        paying: Option<Paying>,
    },
    /// Its plan, as it stands now (queue item 824): written, replaced, or
    /// on another of its steps.
    Planned(Plan),
    /// It carries on after the person let it (queue item 825): what it
    /// told the model the person changed, said on the terminal.
    Continued {
        /// What the model was told.
        changed: String,
    },
    /// The step begun chooses in a closed list: its options, shown in the
    /// agent's column (queue item 810).
    Choosing(alo_renderer::agent_work::Dropdown),
    /// The person allowed it, and it goes on.
    Allowed,
    /// It cannot go on: why, and its choices.
    Stuck {
        /// Why.
        why: String,
        /// What it offers.
        choices: Vec<String>,
    },
    /// It finished: the result.
    Done {
        /// The result, in full.
        result: String,
    },
    /// It stopped before finishing, and why.
    Stopped {
        /// Why, in a person's words.
        why: String,
    },
    /// What one step cost, as `OpenAI` counted it: said on the terminal.
    Spent(Spent),
}

impl Said {
    /// Whether the run is over once this is said.
    pub fn ends(&self) -> bool {
        matches!(
            self,
            Said::Stuck { .. } | Said::Done { .. } | Said::Stopped { .. }
        )
    }
}

/// How the window is told: whether it was listening.
pub type TellAgent = Arc<dyn Fn(AgentSaid) -> bool + Send + Sync>;

/// What a person answered a question with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Person {
    /// *Allow*, for the question numbered so.
    Allow(u64),
    /// *Not now*, for the question numbered so.
    NotNow(u64),
    /// *Let alo continue*, once the page was the person's (queue item 825).
    Continue,
}

/// The agent, as the window holds it: the model it asks, and the orders it
/// reaches the card by — the agent socket's own.
#[derive(Debug, Clone)]
pub struct Agent {
    provider: Provider,
    orders: Orders,
    /// How long a run is held while the page is the person's.
    most_pause: Duration,
}

impl Agent {
    /// The agent asking `provider`, reaching the card by `orders`: the
    /// relay's ([`crate::conversation_agents::relay`]).
    pub fn new(provider: Provider, orders: Orders) -> Self {
        Self {
            provider,
            orders,
            most_pause: MOST_PAUSE,
        }
    }

    /// The same agent, a run of it held at most `most` while the page is
    /// the person's — [`MOST_PAUSE`] unless a test says otherwise.
    #[must_use]
    pub fn held_at_most(mut self, most: Duration) -> Self {
        self.most_pause = most;
        self
    }

    /// Start run `run` of the agent on what the person `asked`, telling the
    /// window with `tell`. Returns at once.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(&self, run: u64, asked: String, tell: TellAgent) -> std::io::Result<Run> {
        let stopped = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let (person, answers) = channel();
        let running = Running {
            agent: self.clone(),
            run,
            asked,
            stopped: Arc::clone(&stopped),
            paused: Arc::clone(&paused),
            answers,
            tell,
            steps: Vec::new(),
            plan: None,
            left: None,
            spent: Spent::default(),
            questions: 0,
            identities: Identities::default(),
        };
        let thread = std::thread::Builder::new()
            .name("alo-conversation-agent".to_owned())
            .spawn(move || running.run())?;
        Ok(Run {
            number: run,
            stopped,
            paused,
            person,
            thread: Some(thread),
        })
    }
}

/// One run of the agent, as the window holds it.
#[derive(Debug)]
pub struct Run {
    number: u64,
    stopped: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    person: Sender<Person>,
    thread: Option<JoinHandle<()>>,
}

impl Run {
    /// Its number.
    pub fn number(&self) -> u64 {
        self.number
    }

    /// Stop it: nothing more is sent to the page, and nothing it says after
    /// is acted on.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    /// Hold it while the page is the person's (queue item 825): nothing
    /// more is sent to the page and nothing it was saying is acted on, until
    /// [`Run::carry_on`] or [`Run::stop`].
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// *Let alo continue*: it reads the page as the person left it, and
    /// carries on.
    pub fn carry_on(&self) {
        let _ = self.person.send(Person::Continue);
    }

    /// The person allowed question `question`.
    pub fn allow(&self, question: u64) {
        // A run that has ended needs no answer.
        let _ = self.person.send(Person::Allow(question));
    }

    /// The person said *Not now* to question `question`.
    pub fn not_now(&self, question: u64) {
        let _ = self.person.send(Person::NotNow(question));
    }

    /// Stop it and wait for its thread: whether it ended rather than
    /// panicked.
    pub fn finish(mut self) -> bool {
        self.stop();
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

impl Drop for Run {
    /// Never finished: stopped, and not waited for.
    fn drop(&mut self) {
        self.stop();
    }
}

/// A run, on its thread.
struct Running {
    agent: Agent,
    run: u64,
    asked: String,
    stopped: Arc<AtomicBool>,
    /// Set while the page is the person's (queue item 825).
    paused: Arc<AtomicBool>,
    answers: Receiver<Person>,
    tell: TellAgent,
    steps: Vec<Taken>,
    /// Its plan as it stands (queue item 824).
    plan: Option<Plan>,
    /// The page as the run last read it: what the person's changes are
    /// told against when they let it carry on.
    left: Option<(String, Snapshot)>,
    spent: Spent,
    questions: u64,
    identities: Identities,
}

/// Why a run ends where it does: what it says last, or nothing when it was
/// stopped and the window already said so.
type Ended = Option<Said>;

impl Running {
    /// Run to its end, and say why it ended.
    fn run(mut self) {
        let ended = self.steps_until_the_end();
        if let Some(said) = ended
            && !self.is_stopped()
        {
            let _ = self.say(said);
        }
    }

    fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Whether it should send nothing and act on nothing now: stopped, or
    /// held while the page is the person's.
    fn is_halted(&self) -> bool {
        self.is_stopped() || self.is_paused()
    }

    /// How many verbs it has carried out: the person's steps are not its.
    fn verbs(&self) -> usize {
        self.steps.iter().filter(|taken| !taken.person).count()
    }

    fn say(&self, said: Said) -> bool {
        (self.tell)(AgentSaid {
            run: self.run,
            said,
        })
    }

    /// Every step, until one ends the run.
    fn steps_until_the_end(&mut self) -> Ended {
        let mut pool = match Pool::from_this_machine() {
            Ok(pool) => pool.patient_for(PATIENCE),
            Err(why) => {
                return Some(stopped(format!(
                    "alo could not go on: {ANSWERER} could not be reached: this Mac's \
                     certificates could not be read ({why})"
                )));
            }
        };
        loop {
            if self.is_stopped() {
                return None;
            }
            // The page the person's: held until they let it carry on
            // (queue item 825).
            if self.is_paused()
                && let Err(ended) = self.hold()
            {
                return ended;
            }
            if self.verbs() >= MOST_STEPS {
                return Some(stopped(format!(
                    "alo stopped after {MOST_STEPS} steps without finishing. Ask again to go on \
                     from what the page shows now."
                )));
            }
            let (tab, snapshot) = match self.read() {
                Ok(read) => read,
                Err(ended) => match self.held(ended) {
                    Some(ended) => return ended,
                    None => continue,
                },
            };
            let call = match self.choose(&mut pool, &tab, &snapshot) {
                Ok(call) => call,
                Err(ended) => match self.held(ended) {
                    Some(ended) => return ended,
                    None => continue,
                },
            };
            match call {
                Call::Done { result } => return Some(Said::Done { result }),
                Call::Stuck { why, choices } => return Some(Said::Stuck { why, choices }),
                Call::Act {
                    node,
                    verb,
                    step,
                    why,
                    plan,
                    on,
                } => {
                    self.planned(plan.as_deref(), on);
                    if let Err(ended) = self.act(node, verb, step, why)
                        && let Some(ended) = self.held(ended)
                    {
                        return ended;
                    }
                }
            }
        }
    }

    /// Whether `ended` ends the run, or was the run held while the page is
    /// the person's — then [`None`], and it carries on from the top once
    /// they let it.
    fn held(&self, ended: Ended) -> Option<Ended> {
        if ended.is_none() && self.is_paused() && !self.is_stopped() {
            return None;
        }
        Some(ended)
    }

    /// The plan the model gave with its step, if it gave one — the first,
    /// or a new one replacing the old — and the step of it `on`: the window
    /// told when either changed (queue item 824).
    fn planned(&mut self, given: Option<&[String]>, on: usize) {
        let now = match (given.and_then(|steps| Plan::of(steps, on)), &self.plan) {
            (Some(plan), _) => Some(plan),
            (None, Some(plan)) => Plan::of(&plan.steps, on),
            (None, None) => None,
        };
        if now.is_some() && now != self.plan {
            self.plan = now;
            if let Some(plan) = &self.plan {
                let _ = self.say(Said::Planned(plan.clone()));
            }
        }
    }

    /// Wait while the page is the person's, then read it as they left it
    /// and tell the model, as a step of theirs, what they changed (queue
    /// item 825). Ends the run when it is stopped, or held past
    /// [`MOST_PAUSE`].
    fn hold(&mut self) -> Result<(), Ended> {
        let since = Instant::now();
        loop {
            if self.is_stopped() {
                return Err(None);
            }
            if since.elapsed() >= self.agent.most_pause {
                return Err(Some(stopped(held_too_long(self.agent.most_pause))));
            }
            match self.answers.recv_timeout(LOOK) {
                Ok(Person::Continue) => break,
                // An answer to a question dropped as the person took over
                // answers nothing.
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(None),
            }
        }
        self.paused.store(false, Ordering::SeqCst);
        let before = self.left.take();
        let (address, now) = self.read()?;
        let changed = match &before {
            Some((was_at, was)) => agent_changes::said(
                agent_changes::Read {
                    address: was_at,
                    tree: was,
                },
                agent_changes::Read {
                    address: &address,
                    tree: &now,
                },
                Reached {
                    steps: self.verbs(),
                    plan: self.plan.as_ref(),
                },
            ),
            None => "alo had not read the page before the person took it over.".to_owned(),
        };
        self.steps.push(Taken {
            step: PERSON.to_owned(),
            came_to: changed.clone(),
            person: true,
        });
        if !self.say(Said::Continued { changed }) {
            return Err(None);
        }
        Ok(())
    }

    /// Ask the page through the relay, as an agent on the socket asks it.
    fn ask(&self, ask: Ask) -> Result<Answer, Ended> {
        if self.is_halted() {
            return Err(None);
        }
        let (reply, answered) = channel();
        self.agent
            .orders
            .send(Order::Agent {
                caller: Caller::ALO,
                ask: Ok(ask),
                reply,
            })
            .map_err(|_| {
                Some(stopped(
                    "alo stopped: the conversation has closed.".to_owned(),
                ))
            })?;
        answered.recv_timeout(VERB_PATIENCE).map_err(|_| {
            Some(stopped(format!(
                "alo stopped: the page did not answer within {} seconds.",
                VERB_PATIENCE.as_secs()
            )))
        })
    }

    /// The page as it is now: the one tab the relay answers, and its tree —
    /// kept as the page the run last read.
    fn read(&mut self) -> Result<(String, Box<Snapshot>), Ended> {
        let gone = || {
            Some(stopped(
                "alo stopped: the page is no longer open.".to_owned(),
            ))
        };
        let tab = match self.ask(Ask::Tabs)? {
            Answer::Tabs(tabs) => tabs.into_iter().next().ok_or_else(gone)?,
            _ => return Err(gone()),
        };
        match self.ask(Ask::Read { tab: tab.clone() })? {
            Answer::Tree { snapshot, .. } => {
                self.left = Some((tab.clone(), (*snapshot).clone()));
                Ok((tab, snapshot))
            }
            Answer::Refused(refused) => Err(Some(stopped(format!(
                "alo stopped: the page could not be read: {refused}."
            )))),
            _ => Err(gone()),
        }
    }

    /// Ask the model for the next step on the page at `tab`, read as
    /// `snapshot`.
    fn choose(&mut self, pool: &mut Pool, tab: &str, snapshot: &Snapshot) -> Result<Call, Ended> {
        let tree = agent_page::text(snapshot);
        let body = agent_protocol::request_body(
            self.agent.provider.model(),
            &Asking {
                asked: &self.asked,
                plan: self.plan.as_ref(),
                address: tab,
                tree: &tree,
                steps: &self.steps,
                left: MOST_STEPS.saturating_sub(self.verbs()),
            },
        );
        let request = self
            .agent
            .provider
            .post(body, agent_protocol::LARGEST, &mut self.identities);
        let fetched = pool.fetch(&request);
        if self.is_halted() {
            // An answer in flight is thrown away — stopped, or the page
            // taken over while it was written.
            return Err(None);
        }
        let response = fetched.map_err(|why| {
            Some(stopped(format!(
                "alo could not go on: {ANSWERER} could not be reached: {why}"
            )))
        })?;
        if response.status.0 != 200 {
            return Err(Some(stopped(format!(
                "alo could not go on: {}",
                why_unanswered(response.status.0, self.agent.provider.held())
            ))));
        }
        let read = agent_protocol::read(&response.body).map_err(|why| {
            Some(stopped(format!(
                "alo could not go on: {ANSWERER}'s step could not be read: {why}."
            )))
        })?;
        if let Some(spent) = read.spent {
            self.spent.input += spent.input;
            self.spent.output += spent.output;
            let _ = self.say(Said::Spent(spent));
        }
        if self.spent.input + self.spent.output > MOST_TOKENS {
            return Err(Some(stopped(format!(
                "alo stopped: this task has used the {MOST_TOKENS} tokens alo lets one task use."
            ))));
        }
        Ok(read.call)
    }

    /// Do `verb` to `node` on the page as it is now, as the step `line`
    /// says, once the person's gate lets it.
    fn act(&mut self, node: BoxId, verb: Verb, line: String, why: String) -> Result<(), Ended> {
        let line = if line.is_empty() {
            let doing = match verb {
                Verb::Activate => "Pressing",
                Verb::PutText(_) => "Typing into",
                Verb::Scroll(_) => "Scrolling",
            };
            format!("{doing} node {}", node.as_usize())
        } else {
            line
        };
        let typed = match &verb {
            Verb::PutText(text) => Some(text.clone()),
            Verb::Activate | Verb::Scroll(_) => None,
        };
        if !self.say(Said::Step {
            line: line.clone(),
            why,
            typed,
        }) {
            return Err(None);
        }
        // Judged on the page as it is now, not as the model was sent it.
        let (now_at, now) = self.read()?;
        // An option of a closed list: its options in the column, never a
        // menu opened where nobody sees it (queue item 810).
        if matches!(verb, Verb::Activate)
            && let Some(choosing) = crate::agent_dropdown::of(&now, node)
            && !self.say(Said::Choosing(choosing))
        {
            return Err(None);
        }
        let host = alo_url::parse(&now_at)
            .map(|url| alo_url::reading::host(&url))
            .unwrap_or_default();
        if let Some(question) = agent_gate::asked(&now, node, &verb, &host) {
            self.ask_the_person(&question, &verb, node, &host)?;
        }
        let answer = self.ask(Ask::Act {
            tab: now_at.clone(),
            target: Target::Node(node),
            verb,
        })?;
        let came_to = match &answer {
            Answer::Done { outcome, .. } => {
                if matches!(
                    outcome,
                    Outcome::Followed { .. } | Outcome::Submitted { .. }
                ) {
                    std::thread::sleep(SETTLE);
                }
                outcome.to_string()
            }
            Answer::Refused(Refused::Password { .. }) => {
                return Err(Some(Said::Stuck {
                    why: "This page asks for a password, and alo never types one: signing in is \
                          yours. Sign in on the page, then ask alo again."
                        .to_owned(),
                    choices: Vec::new(),
                }));
            }
            Answer::Refused(Refused::CardDetails { .. }) => {
                return Err(Some(Said::Stuck {
                    why: CARD_DETAILS.to_owned(),
                    choices: Vec::new(),
                }));
            }
            other => other.said().unwrap_or_default(),
        };
        self.steps.push(Taken {
            step: line,
            came_to,
            person: false,
        });
        Ok(())
    }

    /// Ask the person `what`, and wait for their answer: go on after
    /// *Allow*, once the page still asks the same; end after *Not now*, or
    /// once stopped.
    fn ask_the_person(
        &mut self,
        asked: &Question,
        verb: &Verb,
        node: BoxId,
        host: &str,
    ) -> Result<(), Ended> {
        let what = asked.what.as_str();
        self.questions += 1;
        let question = self.questions;
        if !self.say(Said::Asking {
            question,
            what: what.to_owned(),
            paying: asked.paying.clone(),
        }) {
            return Err(None);
        }
        loop {
            // Stopped, or the page taken over: the question is dropped.
            if self.is_halted() {
                return Err(None);
            }
            match self.answers.recv_timeout(LOOK) {
                Ok(Person::Allow(answered)) if answered == question => break,
                Ok(Person::NotNow(answered)) if answered == question => {
                    return Err(Some(stopped(format!(
                        "Not done: you said not now. {}",
                        what.split('?')
                            .next()
                            .map_or(String::new(), |asked| format!(
                                "alo did not {}.",
                                lowered(asked)
                            ))
                    ))));
                }
                // An answer to another question is no answer to this one.
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(None),
            }
        }
        // Allowed: only what the person was shown. The page is read again,
        // and a page that would now be asked something else — the button
        // renamed, gone — is not pressed.
        let (_, now) = self.read()?;
        let still = agent_gate::asked(&now, node, verb, host);
        // The same question, and for a payment the same amount, merchant
        // and summary: a total changed while the person read is not paid.
        if still.as_ref() != Some(asked) {
            return Err(Some(stopped(
                "alo stopped: the page changed while you were asked, so nothing was pressed."
                    .to_owned(),
            )));
        }
        let _ = self.say(Said::Allowed);
        Ok(())
    }
}

/// What a run says when the page asks for a card's details (queue item
/// 826, ADR 0099).
pub const CARD_DETAILS: &str = "This page asks for a card's details, and alo never types or \
chooses a card's number, security code or expiry: paying details are yours. Fill them in on the \
page, then ask alo again.";

/// What a run held past `most` says as it stops (queue item 825).
pub fn held_too_long(most: Duration) -> String {
    let minutes = most.as_secs() / 60;
    let held = match minutes {
        0 => format!("{} seconds", most.as_secs()),
        1 => "1 minute".to_owned(),
        minutes => format!("{minutes} minutes"),
    };
    format!(
        "alo stopped: the page was yours for {held}. Ask again to go on from what the page shows now."
    )
}

/// A run stopped for `why`.
fn stopped(why: String) -> Said {
    Said::Stopped { why }
}

/// `asked` — *Press “Send” on 127.0.0.1* — as what was not done: *press
/// “Send” on 127.0.0.1*.
fn lowered(asked: &str) -> String {
    let mut letters = asked.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(letters).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{self, Arrival};
    use crate::search_key::Key;
    use alo_box::{KnownRole, Role, States};
    use alo_layout::Rect;
    use alo_renderer::SnapshotNode;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::time::Instant;

    fn node(
        id: usize,
        role: KnownRole,
        name: Option<&str>,
        children: Vec<SnapshotNode>,
    ) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: name.map(str::to_owned),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    /// The page the fake card answers: a field *Name*, *Say hello* and,
    /// in a form, *Send*.
    fn page() -> Snapshot {
        Snapshot {
            root: Some(node(
                1,
                KnownRole::Document,
                None,
                vec![
                    node(2, KnownRole::TextBox, Some("Name"), Vec::new()),
                    node(3, KnownRole::Button, Some("Say hello"), Vec::new()),
                    node(
                        4,
                        KnownRole::Form,
                        None,
                        vec![node(5, KnownRole::Button, Some("Send"), Vec::new())],
                    ),
                ],
            )),
        }
    }

    const TAB: &str = "http://127.0.0.1:9/page.html";

    /// Each verb a fake card was asked to carry out.
    type Acted = Arc<Mutex<Vec<(Target, Verb)>>>;

    /// The page a fake card answers with, which a test may change.
    type Shown = Arc<Mutex<Snapshot>>;

    /// The node a fake card refuses as a card's details (queue item 826).
    const CARD_FIELD: usize = 77;

    /// A card's thread answering every order with the page `shown` holds,
    /// and keeping each verb it was asked to carry out; text put into
    /// [`CARD_FIELD`] is refused as a card's details are.
    fn card_of(shown: Shown) -> (Orders, Acted) {
        let (orders, _, inbox) = inbox::inbox();
        let acted = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&acted);
        std::thread::spawn(move || {
            while let Ok(Arrival::Order(Order::Agent { caller, ask, reply })) = inbox.recv() {
                assert_eq!(caller, Caller::ALO);
                let answer = match ask {
                    Ok(Ask::Tabs) => Answer::Tabs(vec![TAB.to_owned()]),
                    Ok(Ask::Read { tab }) => Answer::Tree {
                        tab,
                        snapshot: Box::new(
                            shown.lock().map(|page| page.clone()).unwrap_or_default(),
                        ),
                    },
                    Ok(Ask::Act { tab, target, verb }) => {
                        if target == Target::Node(BoxId::from_wire(CARD_FIELD)) {
                            let _ = reply.send(Answer::Refused(Refused::CardDetails {
                                tab,
                                node: BoxId::from_wire(CARD_FIELD),
                            }));
                            continue;
                        }
                        if let Ok(mut acted) = kept.lock() {
                            acted.push((target.clone(), verb.clone()));
                        }
                        let Target::Node(node) = target else {
                            return;
                        };
                        Answer::Done {
                            tab,
                            outcome: Outcome::Activated { node, name: None },
                            issues: Vec::new(),
                        }
                    }
                    _ => return,
                };
                let _ = reply.send(answer);
            }
        });
        (orders, acted)
    }

    /// A model on this machine answering each request with the next of
    /// `script` — a status and a body — and keeping each request's body.
    fn model(script: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&bodies);
        std::thread::spawn(move || {
            let mut script = script.into_iter();
            for socket in listener.incoming() {
                let Ok(mut socket) = socket else { return };
                let body = read_request(&mut socket);
                if let Ok(mut bodies) = kept.lock() {
                    bodies.push(body);
                }
                let (status, reply) = script.next().unwrap_or((500, String::new()));
                let _ = socket.write_all(
                    format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                        reply.len()
                    )
                    .as_bytes(),
                );
            }
        });
        (format!("http://127.0.0.1:{port}/v1/responses"), bodies)
    }

    fn read_request(socket: &mut std::net::TcpStream) -> String {
        let mut read = Vec::new();
        let mut block = [0u8; 4096];
        while let Ok(got) = socket.read(&mut block) {
            if got == 0 {
                break;
            }
            read.extend_from_slice(&block[..got]);
            let text = String::from_utf8_lossy(&read).to_string();
            if let Some(head) = text.find("\r\n\r\n") {
                let length = text[..head]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|n| n.trim().parse::<usize>().unwrap_or(0))
                    })
                    .unwrap_or(0);
                if read.len() >= head + 4 + length {
                    return text[head + 4..].to_owned();
                }
            }
        }
        String::from_utf8_lossy(&read).to_string()
    }

    /// A response calling `name` with `arguments`, in `OpenAI`'s shape.
    fn calling(name: &str, arguments: &str) -> (u16, String) {
        let arguments = arguments.replace('\\', "\\\\").replace('"', "\\\"");
        (
            200,
            format!(
                "{{\"object\":\"response\",\"status\":\"completed\",\"output\":[{{\"type\":\"function_call\",\"call_id\":\"c\",\"name\":\"{name}\",\"arguments\":\"{arguments}\"}}],\"usage\":{{\"input_tokens\":10,\"output_tokens\":2}}}}"
            ),
        )
    }

    /// Run the agent on `asked` with the model answering `script`: what it
    /// said, the verbs the card was asked for, and the model's requests;
    /// `person` answers each question it is asked.
    fn run(
        script: Vec<(u16, String)>,
        person: impl Fn(&Run, u64) + Send + 'static,
    ) -> (Vec<Said>, Vec<(Target, Verb)>, Vec<String>) {
        let shown = Arc::new(Mutex::new(page()));
        run_on(script, &shown, Agent::new, move |running, said| {
            if let Said::Asking { question, .. } = said {
                person(running, *question);
            }
        })
    }

    /// [`run`] on the page `shown` holds, the agent made by `agent`, and
    /// `react` told everything the run says as it says it.
    fn run_on(
        script: Vec<(u16, String)>,
        shown: &Shown,
        agent: impl Fn(Provider, Orders) -> Agent,
        mut react: impl FnMut(&Run, &Said),
    ) -> (Vec<Said>, Vec<(Target, Verb)>, Vec<String>) {
        let (orders, acted) = card_of(Arc::clone(shown));
        let (endpoint, bodies) = model(script);
        let provider = Provider::at(&endpoint, Key::of("test-key-not-real").unwrap()).unwrap();
        let agent = agent(provider, orders);
        let (tell, heard) = channel();
        let tell: TellAgent = Arc::new(move |said: AgentSaid| tell.send(said).is_ok());
        let running = agent.start(1, "send my name".to_owned(), tell).unwrap();
        let mut said: Vec<Said> = Vec::new();
        let by = Instant::now() + Duration::from_secs(30);
        let mut quiet = Instant::now();
        while Instant::now() < by {
            let Ok(heard) = heard.recv_timeout(Duration::from_millis(200)) else {
                // A run stopped while it asks says nothing more.
                if matches!(said.last(), Some(Said::Asking { .. }))
                    && quiet.elapsed() > Duration::from_secs(3)
                {
                    break;
                }
                continue;
            };
            quiet = Instant::now();
            assert_eq!(heard.run, 1);
            react(&running, &heard.said);
            let ends = heard.said.ends();
            said.push(heard.said);
            if ends {
                break;
            }
        }
        assert!(running.finish());
        let acted = acted.lock().map(|acted| acted.clone()).unwrap_or_default();
        let bodies = bodies
            .lock()
            .map(|bodies| bodies.clone())
            .unwrap_or_default();
        (said, acted, bodies)
    }

    fn steps_of(said: &[Said]) -> Vec<String> {
        said.iter()
            .filter_map(|said| match said {
                Said::Step { line, .. } => Some(line.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn it_fills_a_field_presses_a_button_and_finishes_through_the_relay() {
        let (said, acted, bodies) = run(
            vec![
                calling(
                    "put_text",
                    r#"{"node":2,"text":"Ada","step":"Typing Ada into Name","why":""}"#,
                ),
                calling(
                    "activate",
                    r#"{"node":3,"step":"Pressing Say hello","why":""}"#,
                ),
                calling("done", r#"{"result":"Said hello as Ada."}"#),
            ],
            |_, _| {},
        );
        assert_eq!(
            steps_of(&said),
            ["Typing Ada into Name", "Pressing Say hello"]
        );
        assert_eq!(
            said.last(),
            Some(&Said::Done {
                result: "Said hello as Ada.".to_owned()
            })
        );
        assert_eq!(
            acted,
            vec![
                (
                    Target::Node(BoxId::from_wire(2)),
                    Verb::PutText("Ada".to_owned())
                ),
                (Target::Node(BoxId::from_wire(3)), Verb::Activate),
            ]
        );
        // Every request is whole: the page, and the steps so far.
        assert_eq!(bodies.len(), 3);
        assert!(bodies.iter().all(|body| body.contains("\"store\":false")));
        assert!(bodies[2].contains("Pressing Say hello"), "{}", bodies[2]);
        assert!(!bodies.iter().any(|body| body.contains("test-key")));
    }

    #[test]
    fn a_press_that_sends_a_form_waits_for_allow_and_is_done_only_after_it() {
        let (said, acted, _) = run(
            vec![
                calling("activate", r#"{"node":5,"step":"Pressing Send","why":""}"#),
                calling("done", r#"{"result":"Sent."}"#),
            ],
            Run::allow,
        );
        assert!(
            said.contains(&Said::Asking {
                question: 1,
                what: "Press \u{201c}Send\u{201d} on 127.0.0.1:9? It sends something from you."
                    .to_owned(),
                paying: None,
            })
        );
        assert!(said.contains(&Said::Allowed));
        assert_eq!(
            acted,
            vec![(Target::Node(BoxId::from_wire(5)), Verb::Activate)]
        );
    }

    #[test]
    fn not_now_presses_nothing_and_ends_the_run() {
        let (said, acted, bodies) = run(
            vec![calling(
                "activate",
                r#"{"node":5,"step":"Pressing Send","why":""}"#,
            )],
            Run::not_now,
        );
        assert!(acted.is_empty(), "{acted:?}");
        assert!(
            matches!(said.last(), Some(Said::Stopped { why }) if why.starts_with("Not done: you said not now.")),
            "{said:?}"
        );
        assert_eq!(bodies.len(), 1, "the model is not asked again");
    }

    #[test]
    fn an_answer_to_another_question_allows_nothing() {
        let (said, acted, _) = run(
            vec![calling(
                "activate",
                r#"{"node":5,"step":"Pressing Send","why":""}"#,
            )],
            |running, question| {
                running.allow(question + 1);
                running.not_now(question);
            },
        );
        assert!(acted.is_empty(), "{acted:?}");
        assert!(matches!(said.last(), Some(Said::Stopped { .. })));
    }

    #[test]
    fn its_step_limit_stops_it_and_says_so() {
        let script = (0..MOST_STEPS + 2)
            .map(|_| {
                calling(
                    "activate",
                    r#"{"node":3,"step":"Pressing Say hello","why":""}"#,
                )
            })
            .collect();
        let (said, acted, bodies) = run(script, |_, _| {});
        assert_eq!(acted.len(), MOST_STEPS);
        assert_eq!(bodies.len(), MOST_STEPS);
        assert_eq!(
            said.last(),
            Some(&Said::Stopped {
                why: "alo stopped after 20 steps without finishing. Ask again to go on from \
                      what the page shows now."
                    .to_owned()
            })
        );
    }

    #[test]
    fn a_model_that_cannot_answer_or_answers_nonsense_is_said() {
        let (said, acted, _) = run(vec![(500, String::new())], |_, _| {});
        assert!(acted.is_empty());
        assert_eq!(
            said.last(),
            Some(&Said::Stopped {
                why: "alo could not go on: OpenAI could not answer just now (500).".to_owned()
            })
        );
        let (said, _, _) = run(vec![(200, "nonsense".to_owned())], |_, _| {});
        assert!(
            matches!(said.last(), Some(Said::Stopped { why }) if why.starts_with("alo could not go on: OpenAI's step could not be read: it is not JSON")),
            "{said:?}"
        );
        let (said, acted, _) = run(vec![calling("click_at", r#"{"x":1,"y":2}"#)], |_, _| {});
        assert!(acted.is_empty());
        assert!(
            matches!(said.last(), Some(Said::Stopped { why }) if why.contains("a tool it was not given")),
            "{said:?}"
        );
    }

    #[test]
    fn stuck_says_why_and_offers_its_choices() {
        let (said, acted, _) = run(
            vec![calling(
                "stuck",
                r#"{"why":"There is no form here.","choices":["Look for one","Stop here"]}"#,
            )],
            |_, _| {},
        );
        assert!(acted.is_empty());
        assert_eq!(
            said.last(),
            Some(&Said::Stuck {
                why: "There is no form here.".to_owned(),
                choices: vec!["Look for one".to_owned(), "Stop here".to_owned()],
            })
        );
    }

    #[test]
    fn a_run_stopped_while_the_person_is_asked_does_nothing_more() {
        let (said, acted, _) = run(
            vec![calling(
                "activate",
                r#"{"node":5,"step":"Pressing Send","why":""}"#,
            )],
            |running, _| running.stop(),
        );
        assert!(acted.is_empty(), "{acted:?}");
        // Stopped by the window, which says so itself: the run says nothing
        // more.
        assert!(matches!(said.last(), Some(Said::Asking { .. })), "{said:?}");
    }

    #[test]
    fn its_plan_is_told_to_the_window_and_sent_with_every_request_as_it_stands() {
        let (said, acted, bodies) = run(
            vec![
                calling(
                    "put_text",
                    r#"{"node":2,"text":"Ada","step":"Typing Ada into Name","why":"","plan":["Fill in your name","Say hello"],"on":1}"#,
                ),
                calling(
                    "activate",
                    r#"{"node":3,"step":"Pressing Say hello","why":"","plan":null,"on":2}"#,
                ),
                calling("done", r#"{"result":"Said hello as Ada."}"#),
            ],
            |_, _| {},
        );
        let plans: Vec<Plan> = said
            .iter()
            .filter_map(|said| match said {
                Said::Planned(plan) => Some(plan.clone()),
                _ => None,
            })
            .collect();
        let steps = vec!["Fill in your name".to_owned(), "Say hello".to_owned()];
        assert_eq!(
            plans,
            [
                Plan {
                    steps: steps.clone(),
                    on: 0
                },
                Plan { steps, on: 1 }
            ]
        );
        assert_eq!(acted.len(), 2);
        // The first request is sent no plan; the next ones the plan as it
        // stood, the step it was on.
        assert!(bodies[0].contains(r#"\"plan\":null"#), "{}", bodies[0]);
        assert!(
            bodies[1].contains(r#"{\"state\":\"now\",\"step\":\"Fill in your name\"}"#),
            "{}",
            bodies[1]
        );
        assert!(
            bodies[2].contains(r#"{\"state\":\"done\",\"step\":\"Fill in your name\"}"#),
            "{}",
            bodies[2]
        );
    }

    #[test]
    fn taken_over_while_it_asks_it_holds_then_carries_on_telling_what_the_person_changed() {
        let mut empty = page();
        if let Some(name) = empty
            .root
            .as_mut()
            .and_then(|root| root.children.first_mut())
        {
            name.states.takes_text = true;
        }
        let shown = Arc::new(Mutex::new(empty));
        let changing = Arc::clone(&shown);
        let mut questions = 0;
        let (said, acted, bodies) = run_on(
            vec![
                calling(
                    "activate",
                    r#"{"node":5,"step":"Pressing Send","why":"","plan":["Send the form"],"on":1}"#,
                ),
                calling(
                    "activate",
                    r#"{"node":5,"step":"Pressing Send","why":"","plan":null,"on":1}"#,
                ),
                calling("done", r#"{"result":"Sent."}"#),
            ],
            &shown,
            Agent::new,
            move |running, said| {
                if let Said::Asking { question, .. } = said {
                    questions += 1;
                    if questions == 1 {
                        // The person takes the page over and types their
                        // name, then lets alo continue.
                        running.pause();
                        std::thread::sleep(Duration::from_millis(300));
                        if let Ok(mut page) = changing.lock()
                            && let Some(root) = page.root.as_mut()
                        {
                            let mut field = node(
                                2,
                                KnownRole::TextBox,
                                Some("Name"),
                                vec![node(20, KnownRole::Text, Some("Grace"), Vec::new())],
                            );
                            field.states.takes_text = true;
                            root.children[0] = field;
                        }
                        running.carry_on();
                    } else {
                        // The person's gate holds after: asked again.
                        running.allow(*question);
                    }
                }
            },
        );
        assert!(matches!(said.last(), Some(Said::Done { .. })), "{said:?}");
        // The question dropped as the page was taken over: Send pressed once,
        // after the second question's Allow.
        assert_eq!(
            acted,
            vec![(Target::Node(BoxId::from_wire(5)), Verb::Activate)]
        );
        let asked: Vec<u64> = said
            .iter()
            .filter_map(|said| match said {
                Said::Asking { question, .. } => Some(*question),
                _ => None,
            })
            .collect();
        assert_eq!(asked, [1, 2]);
        let continued = said.iter().find_map(|said| match said {
            Said::Continued { changed } => Some(changed.clone()),
            _ => None,
        });
        assert_eq!(
            continued.as_deref(),
            Some(
                "The person typed \u{201c}Grace\u{201d} into the field \u{201c}Name\u{201d}. alo \
                 had taken 0 steps, and was on step 1 of its plan, \u{201c}Send the form\u{201d}."
            )
        );
        // The model told, as a step of the person's; and that step not
        // counted against the run's: 20 left still.
        assert_eq!(bodies.len(), 3);
        assert!(bodies[1].contains(PERSON), "{}", bodies[1]);
        assert!(bodies[1].contains("Grace"), "{}", bodies[1]);
        assert!(bodies[1].contains(r#"\"steps_left\":20"#), "{}", bodies[1]);
    }

    #[test]
    fn held_past_its_most_it_stops_and_says_so_having_done_nothing() {
        let shown = Arc::new(Mutex::new(page()));
        let (said, acted, bodies) = run_on(
            vec![calling(
                "activate",
                r#"{"node":5,"step":"Pressing Send","why":"","plan":null,"on":1}"#,
            )],
            &shown,
            |provider, orders| {
                Agent::new(provider, orders).held_at_most(Duration::from_millis(400))
            },
            |running, said| {
                if matches!(said, Said::Asking { .. }) {
                    running.pause();
                }
            },
        );
        assert!(acted.is_empty(), "{acted:?}");
        assert_eq!(bodies.len(), 1);
        assert_eq!(
            said.last(),
            Some(&Said::Stopped {
                why: held_too_long(Duration::from_millis(400))
            })
        );
        assert_eq!(
            held_too_long(MOST_PAUSE),
            "alo stopped: the page was yours for 10 minutes. Ask again to go on from what the \
             page shows now."
        );
    }

    #[test]
    fn a_press_that_pays_is_asked_with_what_it_pays_and_a_cards_details_are_never_typed() {
        let mut checkout = page();
        if let Some(root) = checkout.root.as_mut() {
            root.name = Some("Kampala Coffee".to_owned());
            root.children.push(node(
                8,
                KnownRole::Text,
                Some("Total UGX 45,000"),
                Vec::new(),
            ));
            root.children
                .push(node(9, KnownRole::Button, Some("Pay now"), Vec::new()));
        }
        let shown = Arc::new(Mutex::new(checkout));
        let (said, acted, _) = run_on(
            vec![calling(
                "activate",
                r#"{"node":9,"step":"Pressing Pay now","why":"","plan":null,"on":1}"#,
            )],
            &shown,
            Agent::new,
            |running, said| {
                if let Said::Asking { question, .. } = said {
                    running.not_now(*question);
                }
            },
        );
        let paying = said.iter().find_map(|said| match said {
            Said::Asking { paying, .. } => paying.clone(),
            _ => None,
        });
        assert_eq!(
            paying,
            Some(Paying {
                amount: Some("UGX 45,000".to_owned()),
                merchant: "Kampala Coffee (127.0.0.1:9)".to_owned(),
                lines: Vec::new(),
            })
        );
        assert!(acted.is_empty(), "Not now pays nothing");
        let (said, acted, _) = run(
            vec![calling(
                "put_text",
                &format!(
                    r#"{{"node":{CARD_FIELD},"text":"4242 4242 4242 4242","step":"Typing the card","why":"","plan":null,"on":1}}"#
                ),
            )],
            |_, _| {},
        );
        assert!(acted.is_empty(), "{acted:?}");
        assert_eq!(
            said.last(),
            Some(&Said::Stuck {
                why: CARD_DETAILS.to_owned(),
                choices: Vec::new(),
            })
        );
    }
}

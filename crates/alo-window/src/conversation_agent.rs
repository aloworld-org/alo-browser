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
//! At most [`Limits::actions`] steps — [`MOST_STEPS`] unless the person set
//! otherwise — and [`MOST_TOKENS`] tokens as `OpenAI`'s
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
//!
//! # Beginning with no page, and another site
//!
//! With no page open (queue item 821, ADR 0103) the model is sent none and
//! begins with two more steps: `search`, asked through the box's own search
//! ([`SearchService::search`]), its sites told to the model as words
//! ([`crate::agent_found`]); and `open`, a result by its number or an
//! address, judged by the browser process ([`crate::agent_opening`]) —
//! opened as if the person had, asked of them first, or refused, and at
//! most [`Limits::sites`] sites a run. A page it opens is opened by the
//! window ([`Said::Opening`]) as a card, through the road a person's
//! address takes; once the card is up ([`Run::opened`]) the run reads it
//! through the relay as any page. A page it reaches that asks the person
//! to sign in ([`crate::agent_signin`]) — or that refuses it a password —
//! hands the page over ([`Said::SignIn`]), as taking it over does, until
//! *Let alo continue*.
//!
//! # Its limits, the person's
//!
//! How many actions a run takes and how many sites it opens, and whether
//! it asks before every site, are the person's ([`Limits`], queue item
//! 878), read by the window as the run starts and kept for the whole run.

use crate::agent_changes::{self, Reached};
use crate::agent_found::FoundSites;
use crate::agent_gate::{self, Question};
use crate::agent_opening::{self, Against, Judged, Visited};
use crate::agent_page;
use crate::agent_protocol::{Asking, Begin, Call, PERSON, Taken};
use crate::agent_record::Caller;
use crate::agent_sites::AgentSites;
use crate::ai_service::{AI_SERVICE, AiService};
use crate::answer_protocol::Spent;
use crate::asking::{Answer, Ask, Refused};
use crate::chrome_pin::Pinned;
use crate::inbox::Orders;
use crate::message::Order;
use crate::search_service::SearchService;
use alo_agent::{Outcome, Target, Verb};
use alo_box::BoxId;
use alo_net::Pool;
use alo_net::cause::Identities;
use alo_net::resolve::{Reach, Resolver, Unresolved};
use alo_renderer::Snapshot;
use alo_renderer::agent_limits::Limits;
use alo_renderer::agent_paying::Paying;
use alo_renderer::agent_plan::Plan;
use alo_url::Url;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The most steps one run takes before it stops unless the person set
/// otherwise (queue item 878): ADR 0096 § 4's twenty.
pub const MOST_STEPS: usize = alo_renderer::agent_limits::DEFAULT_ACTIONS as usize;

/// What the model is told of a step of the person's after a page asked
/// them to sign in (queue item 821).
pub const SIGNED_IN: &str = "The page asked the person to sign in, and alo handed it to them; \
they let alo continue";

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
    /// It opens the page at `url` (queue item 821): the window opens it as
    /// a card answering `said`, the run moved onto it, and says when it is
    /// up ([`Run::opened`]) or why it could not be opened
    /// ([`Run::unopened`]).
    Opening {
        /// Where: boxed, as few runs open a page and every word a run says
        /// is carried in a `Said`.
        url: Box<Url>,
        /// What the card's bubble says.
        said: String,
        /// Its name as the address the browser process resolved it to,
        /// which the card's Chromium takes rather than resolving it again
        /// (queue item 881, ADR 0103 § 7); none for an address or a site
        /// the person named.
        pinned: Option<Pinned>,
    },
    /// The page it reached asks the person to sign in (queue item 821): it
    /// is handed over, and the run held, until *Let alo continue*.
    SignIn,
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
#[derive(Debug, Clone, PartialEq, Eq)]
enum Person {
    /// *Allow*, for the question numbered so.
    Allow(u64),
    /// *Not now*, for the question numbered so.
    NotNow(u64),
    /// *Let alo continue*, once the page was the person's (queue item 825).
    Continue,
    /// The page the run asked to open is up as a card (queue item 821).
    Opened,
    /// It could not be opened, and why.
    Unopened(String),
}

/// How a name is resolved before a site is opened — the addresses it may
/// be reached at — the machine's resolver unless a test says otherwise
/// (queue item 821).
pub type Resolving = Arc<dyn Fn(&str, u16, Reach) -> Result<Vec<IpAddr>, Unresolved> + Send + Sync>;

/// The machine's resolver, as every request's names are resolved (ADR
/// 0008).
fn the_machines(host: &str, port: u16, reach: Reach) -> Result<Vec<IpAddr>, Unresolved> {
    Resolver::new()
        .resolve(host, port, reach)
        .map(|found| found.iter().map(std::net::SocketAddr::ip).collect())
}

/// The agent, as the window holds it: the model it asks, and the orders it
/// reaches the card by — the agent socket's own.
#[derive(Clone)]
pub struct Agent {
    provider: AiService,
    orders: Orders,
    /// How long a run is held while the page is the person's.
    most_pause: Duration,
    /// What a run searches with, the box's own search, if it is set up
    /// (queue item 821).
    search: Option<SearchService>,
    /// The sites agents are kept to, which a site it opens must be one of.
    sites: AgentSites,
    /// How a name is resolved before a site is opened.
    resolve: Resolving,
}

impl core::fmt::Debug for Agent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Agent")
            .field("provider", &self.provider)
            .field("most_pause", &self.most_pause)
            .field("search", &self.search.is_some())
            .field("sites", &self.sites)
            .finish_non_exhaustive()
    }
}

impl Agent {
    /// The agent asking `provider`, reaching the card by `orders`: the
    /// relay's ([`crate::conversation_agents::relay`]).
    pub fn new(provider: impl Into<AiService>, orders: Orders) -> Self {
        Self {
            provider: provider.into(),
            orders,
            most_pause: MOST_PAUSE,
            search: None,
            sites: AgentSites::Every,
            resolve: Arc::new(the_machines),
        }
    }

    /// The same agent, searching with `search` — the box's own provider —
    /// when a run needs to find a site (queue item 821); none, and a run
    /// told searching is not set up.
    #[must_use]
    pub fn searching_with(mut self, search: Option<SearchService>) -> Self {
        self.search = search;
        self
    }

    /// Who chooses its steps, for the terminal alone.
    pub fn named(&self) -> String {
        self.provider.named()
    }

    /// The same agent, opening only sites of `sites` — those a person named
    /// with `--agent-site`, which every agent's ask is kept to.
    #[must_use]
    pub fn kept_to(mut self, sites: AgentSites) -> Self {
        self.sites = sites;
        self
    }

    /// The same agent, resolving names with `resolve` rather than the
    /// machine's resolver: a Rust constructor, for a test that answers
    /// names itself, as `openai_answer::Provider::at` is for one that answers as the
    /// model.
    #[must_use]
    pub fn resolving_with(mut self, resolve: Resolving) -> Self {
        self.resolve = resolve;
        self
    }

    /// The same agent, a run of it held at most `most` while the page is
    /// the person's — [`MOST_PAUSE`] unless a test says otherwise.
    #[must_use]
    pub fn held_at_most(mut self, most: Duration) -> Self {
        self.most_pause = most;
        self
    }

    /// Start run `run` of the agent on what the person `asked`, telling the
    /// window with `tell`, kept for the whole run to `limits` — the
    /// person's, read as it starts (queue item 878). Returns at once.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(
        &self,
        run: u64,
        asked: String,
        tell: TellAgent,
        limits: Limits,
    ) -> std::io::Result<Run> {
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
            limits,
            found: FoundSites::default(),
            visited: Visited::default(),
            had_page: false,
            signing_in: false,
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

    /// The page it asked to open is up as a card (queue item 821).
    pub fn opened(&self) {
        let _ = self.person.send(Person::Opened);
    }

    /// The page it asked to open could not be opened, for `why`.
    pub fn unopened(&self, why: String) {
        let _ = self.person.send(Person::Unopened(why));
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
    /// How far it may go, the person's, read as it started (queue item
    /// 878).
    limits: Limits,
    /// What its searches found, by number (queue item 821).
    found: FoundSites,
    /// The sites it has been on, and how many it opened.
    visited: Visited,
    /// Whether it has read a page: once it has, no page open is a page
    /// gone.
    had_page: bool,
    /// Whether it is held because a page asked the person to sign in.
    signing_in: bool,
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
                    "alo could not go on: {AI_SERVICE} could not be reached: this Mac's \
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
            let most = usize::from(self.limits.actions);
            if self.verbs() >= most {
                return Some(stopped(format!(
                    "alo stopped after {most} steps without finishing. Ask again to go on \
                     from what the page shows now."
                )));
            }
            // The page as it is now — or, before any is open, none (queue
            // item 821).
            let page = match self.read_if_open() {
                Ok(page) => page,
                Err(ended) => match self.held(ended) {
                    Some(ended) => return ended,
                    None => continue,
                },
            };
            let page = page
                .as_ref()
                .map(|(tab, snapshot)| (tab.as_str(), &**snapshot));
            let call = match self.choose(&mut pool, page) {
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
                Call::Begin {
                    begin,
                    step,
                    why,
                    plan,
                    on,
                } => {
                    self.planned(plan.as_deref(), on);
                    if let Err(ended) = self.begin(&mut pool, &begin, step, why)
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
        let step = if core::mem::replace(&mut self.signing_in, false) {
            SIGNED_IN
        } else {
            PERSON
        };
        self.steps.push(Taken {
            step: step.to_owned(),
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
                self.had_page = true;
                // Where it is now: a site it is on, never counted as one it
                // opened (queue item 821).
                if let Ok(url) = alo_url::parse(&tab) {
                    self.visited.been_on(&url);
                }
                Ok((tab, snapshot))
            }
            Answer::Refused(refused) => Err(Some(stopped(format!(
                "alo stopped: the page could not be read: {refused}."
            )))),
            _ => Err(gone()),
        }
    }

    /// The page as it is now, as [`Running::read`] reads it — or, before
    /// the run has read any (queue item 821), [`None`] when no page is open.
    fn read_if_open(&mut self) -> Result<Option<(String, Box<Snapshot>)>, Ended> {
        if !self.had_page {
            match self.ask(Ask::Tabs)? {
                Answer::Tabs(tabs) if tabs.is_empty() => return Ok(None),
                Answer::Tabs(_) => {}
                _ => return Ok(None),
            }
        }
        self.read().map(Some)
    }

    /// Ask the model for the next step on `page` — its address, and its
    /// tree — or with no page open.
    fn choose(&mut self, pool: &mut Pool, page: Option<(&str, &Snapshot)>) -> Result<Call, Ended> {
        let (tab, tree) = page.map_or((String::new(), String::new()), |(tab, snapshot)| {
            (tab.to_owned(), agent_page::text(snapshot))
        });
        let asking = Asking {
            asked: &self.asked,
            plan: self.plan.as_ref(),
            address: &tab,
            tree: &tree,
            steps: &self.steps,
            left: usize::from(self.limits.actions).saturating_sub(self.verbs()),
            sites_left: usize::from(self.limits.sites).saturating_sub(self.visited.opened_count()),
        };
        let stepped = self
            .agent
            .provider
            .step(pool, &mut self.identities, &asking);
        if self.is_halted() {
            // An answer in flight is thrown away — stopped, or the page
            // taken over while it was written.
            return Err(None);
        }
        let read = stepped.map_err(|why| Some(stopped(format!("alo could not go on: {why}"))))?;
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
        let mut went = false;
        let came_to = match &answer {
            Answer::Done { outcome, .. } => {
                if matches!(
                    outcome,
                    Outcome::Followed { .. } | Outcome::Submitted { .. }
                ) {
                    std::thread::sleep(SETTLE);
                    went = true;
                }
                outcome.to_string()
            }
            // The agent trying to sign in (queue item 821): the page is the
            // person's, as a sign-in page reached is.
            Answer::Refused(Refused::Password { .. }) => {
                self.steps.push(Taken {
                    step: line,
                    came_to: "The page asked for a password, which alo never types: alo handed \
                              the page to the person to sign in."
                        .to_owned(),
                    person: false,
                });
                self.sign_in();
                return Ok(());
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
        // Gone to another page by a link or a form: one that asks the
        // person to sign in is handed to them (queue item 821).
        if went {
            let (_, now) = self.read()?;
            if crate::agent_signin::asks_to_sign_in(&now) {
                self.sign_in();
            }
        }
        Ok(())
    }

    /// The page is the person's to sign in on (queue item 821): held, as
    /// when they take it over, until *Let alo continue*.
    fn sign_in(&mut self) {
        self.signing_in = true;
        self.paused.store(true, Ordering::SeqCst);
        let _ = self.say(Said::SignIn);
    }

    /// A step that begins (queue item 821): a search, or a site opened —
    /// its line said, what came of it told to the model next time.
    fn begin(
        &mut self,
        pool: &mut Pool,
        begin: &Begin,
        line: String,
        why: String,
    ) -> Result<(), Ended> {
        let line = if line.is_empty() {
            match begin {
                Begin::Search(words) => format!("Searching for \u{201c}{words}\u{201d}"),
                Begin::Result(number) => format!("Opening result {number}"),
                Begin::Address(address) => format!("Opening {address}"),
            }
        } else {
            line
        };
        if !self.say(Said::Step {
            line: line.clone(),
            why,
            typed: None,
        }) {
            return Err(None);
        }
        let came_to = match begin {
            Begin::Search(words) => self.search(pool, words)?,
            Begin::Result(_) | Begin::Address(_) => self.open(begin)?,
        };
        self.steps.push(Taken {
            step: line,
            came_to,
            person: false,
        });
        Ok(())
    }

    /// Search for `words` through the box's own search: what came of it,
    /// in words, its sites kept by number.
    fn search(&mut self, pool: &mut Pool, words: &str) -> Result<String, Ended> {
        if self.is_halted() {
            return Err(None);
        }
        let Some(provider) = self.agent.search.clone() else {
            return Ok(
                "alo cannot search: searching is not set up in this browser. Open a site by \
                 its address instead."
                    .to_owned(),
            );
        };
        let answered = provider.search(pool, &mut self.identities, words);
        if self.is_halted() {
            return Err(None);
        }
        Ok(self.found.searched(words, answered.showing))
    }

    /// Open what `begin` names, once the browser process's judgement
    /// (`crate::agent_opening`) lets it — and the person, when it asks
    /// them: what came of it, in words.
    fn open(&mut self, begin: &Begin) -> Result<String, Ended> {
        let resolve = Arc::clone(&self.agent.resolve);
        let mut resolving = move |host: &str, port: u16, reach: Reach| resolve(host, port, reach);
        let judged = agent_opening::judge(
            begin,
            &Against {
                asked: &self.asked,
                found: &self.found,
                visited: &self.visited,
                limits: self.limits,
                agent_sites: &self.agent.sites,
            },
            &mut resolving,
        );
        let (url, said, pinned) = match judged {
            Judged::Refused(why) => return Ok(format!("Not opened: {why}.")),
            Judged::Ask {
                url,
                said,
                what,
                pinned,
            } => {
                let host = alo_url::reading::host(&url);
                self.wait_for_the_person(&what, None, &format!("alo did not open {host}."))?;
                let _ = self.say(Said::Allowed);
                (url, said, pinned)
            }
            Judged::Open { url, said, pinned } => (url, said, pinned),
        };
        if self.is_halted() {
            return Err(None);
        }
        if !self.say(Said::Opening {
            url: Box::new(url.clone()),
            said,
            pinned,
        }) {
            return Err(None);
        }
        let since = Instant::now();
        loop {
            if self.is_halted() {
                return Err(None);
            }
            if since.elapsed() >= VERB_PATIENCE {
                return Err(Some(stopped(format!(
                    "alo stopped: the page did not open within {} seconds.",
                    VERB_PATIENCE.as_secs()
                ))));
            }
            match self.answers.recv_timeout(LOOK) {
                Ok(Person::Opened) => break,
                Ok(Person::Unopened(why)) => {
                    return Ok(format!("{} could not be opened: {why}", url.serialised));
                }
                // An answer to a question no longer asked answers nothing.
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(None),
            }
        }
        self.visited.opened(&url);
        // Given the time a page takes to arrive before it is read.
        std::thread::sleep(SETTLE * 2);
        let (at, now) = self.read()?;
        if crate::agent_signin::asks_to_sign_in(&now) {
            self.sign_in();
        }
        Ok(format!(
            "Opened {at}: it is the page you are given now, a card in the person's conversation."
        ))
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
        let not_done = what.split('?').next().map_or(String::new(), |asked| {
            format!("alo did not {}.", lowered(asked))
        });
        self.wait_for_the_person(what, asked.paying.clone(), &not_done)?;
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

impl Running {
    /// Ask the person `what` — paying `paying`, when it pays — and wait for
    /// their answer: [`Ok`] once allowed; ended, saying `not_done`, after
    /// *Not now*, or once stopped or taken over.
    fn wait_for_the_person(
        &mut self,
        what: &str,
        paying: Option<Paying>,
        not_done: &str,
    ) -> Result<(), Ended> {
        self.questions += 1;
        let question = self.questions;
        if !self.say(Said::Asking {
            question,
            what: what.to_owned(),
            paying,
        }) {
            return Err(None);
        }
        loop {
            // Stopped, or the page taken over: the question is dropped.
            if self.is_halted() {
                return Err(None);
            }
            match self.answers.recv_timeout(LOOK) {
                Ok(Person::Allow(answered)) if answered == question => return Ok(()),
                Ok(Person::NotNow(answered)) if answered == question => {
                    return Err(Some(stopped(format!(
                        "Not done: you said not now. {not_done}"
                    ))));
                }
                // An answer to another question is no answer to this one.
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(None),
            }
        }
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
    use crate::openai_answer::Provider;
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
        let running = agent
            .start(1, "send my name".to_owned(), tell, Limits::default())
            .unwrap();
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
                why: "alo could not go on: the AI service could not answer just now (500)."
                    .to_owned()
            })
        );
        let (said, _, _) = run(vec![(200, "nonsense".to_owned())], |_, _| {});
        assert!(
            matches!(said.last(), Some(Said::Stopped { why }) if why.starts_with("alo could not go on: the AI service's step could not be read: it is not JSON")),
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

    /// Where a fake world's one card is, if one is open: no page until the
    /// run's own open is answered (queue item 821).
    type OpenAt = Arc<Mutex<Option<String>>>;

    /// A card's thread answering as [`card_of`] does, but with no tab until
    /// `open_at` holds one, and then that tab.
    fn world_of(shown: Shown, open_at: OpenAt) -> (Orders, Acted) {
        let (orders, _, inbox) = inbox::inbox();
        let acted = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&acted);
        std::thread::spawn(move || {
            while let Ok(Arrival::Order(Order::Agent { ask, reply, .. })) = inbox.recv() {
                let at = open_at.lock().map(|at| at.clone()).unwrap_or_default();
                let answer = match (ask, at) {
                    (Ok(Ask::Tabs), at) => Answer::Tabs(at.into_iter().collect()),
                    (Ok(Ask::Read { tab }), Some(_)) => Answer::Tree {
                        tab,
                        snapshot: Box::new(
                            shown.lock().map(|page| page.clone()).unwrap_or_default(),
                        ),
                    },
                    (Ok(Ask::Act { tab, target, verb }), Some(_)) => {
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
                    (Ok(Ask::Read { tab } | Ask::Act { tab, .. }), None) => {
                        Answer::Refused(Refused::NoSuchTab { tab })
                    }
                    _ => return,
                };
                let _ = reply.send(answer);
            }
        });
        (orders, acted)
    }

    /// Brave Search's answer, written by hand in its shape, finding the
    /// Uganda Museum, a page on this machine and Visit Uganda.
    const FOUND: &str = r#"{"type":"search","web":{"type":"search","results":[{"title":"Uganda Museum","url":"https://ugandamuseum.go.ug/","description":"Open daily from 10."},{"title":"A page here","url":"http://192.168.1.20/","description":""},{"title":"Visit Uganda","url":"https://visit.example/museum","description":""}]}}"#;

    /// What a run begun with no page came to, as [`run_on`] gives it, and
    /// the addresses the window was asked to open, and the searches asked.
    struct Begun {
        said: Vec<Said>,
        acted: Vec<(Target, Verb)>,
        bodies: Vec<String>,
        opened: Vec<String>,
        searched: Vec<String>,
    }

    /// Run the agent with no page open on what the person `asked`, the
    /// model answering `script`, a search answering [`FOUND`], kept to
    /// `limits`; the page it opens is `shown`, and `person` answers each
    /// question.
    fn begun(
        asked: &str,
        script: Vec<(u16, String)>,
        shown: Snapshot,
        limits: Limits,
        person: impl Fn(&Run, u64) + Send + 'static,
    ) -> Begun {
        let open_at: OpenAt = Arc::new(Mutex::new(None));
        let (orders, acted) = world_of(Arc::new(Mutex::new(shown)), Arc::clone(&open_at));
        let (endpoint, bodies) = model(script);
        let (search_at, searched) = model(vec![(200, FOUND.to_owned()); 4]);
        let provider = Provider::at(&endpoint, Key::of("test-key-not-real").unwrap()).unwrap();
        let search = crate::brave_search::Provider::at(
            &search_at.replace("/v1/responses", "/res/v1/web/search"),
            Key::of("search-key-not-real").unwrap(),
        )
        .unwrap();
        // Names resolve to a public address but this test's own.
        let agent = Agent::new(provider, orders)
            .searching_with(Some(search.into()))
            .resolving_with(Arc::new(|_: &str, _: u16, _: Reach| {
                Ok(vec![IpAddr::from([93, 184, 215, 14])])
            }));
        let (tell, heard) = channel();
        let tell: TellAgent = Arc::new(move |said: AgentSaid| tell.send(said).is_ok());
        let running = agent.start(1, asked.to_owned(), tell, limits).unwrap();
        let mut said: Vec<Said> = Vec::new();
        let mut opened = Vec::new();
        let by = Instant::now() + Duration::from_secs(40);
        while Instant::now() < by {
            let Ok(heard) = heard.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            match &heard.said {
                // The window opens it as a card, and says it is up.
                Said::Opening { url, .. } => {
                    opened.push(url.serialised.clone());
                    if let Ok(mut at) = open_at.lock() {
                        *at = Some(url.serialised.clone());
                    }
                    running.opened();
                }
                Said::Asking { question, .. } => person(&running, *question),
                // The person signs in, and lets alo continue.
                Said::SignIn => running.carry_on(),
                _ => {}
            }
            let ends = heard.said.ends();
            said.push(heard.said);
            if ends {
                break;
            }
        }
        assert!(running.finish());
        let take = |kept: &Arc<Mutex<Vec<String>>>| {
            kept.lock().map(|kept| kept.clone()).unwrap_or_default()
        };
        Begun {
            said,
            acted: acted.lock().map(|acted| acted.clone()).unwrap_or_default(),
            bodies: take(&bodies),
            opened,
            searched: take(&searched),
        }
    }

    #[test]
    fn with_no_page_it_searches_opens_a_result_with_no_question_and_works_on_it() {
        // Queue item 821.
        let ran = begun(
            "find the opening hours of the Uganda Museum",
            vec![
                calling(
                    "search",
                    r#"{"words":"Uganda Museum opening hours","step":"Searching for the museum","why":"","plan":["Find the museum's site","Read its hours"],"on":1}"#,
                ),
                calling(
                    "open",
                    r#"{"result":1,"address":null,"step":"Opening the Uganda Museum's site","why":"","plan":null,"on":1}"#,
                ),
                calling("done", r#"{"result":"It opens daily from 10."}"#),
            ],
            page(),
            Limits::default(),
            |_, _| panic!("a search's result is opened with no question"),
        );
        assert_eq!(
            steps_of(&ran.said),
            [
                "Searching for the museum",
                "Opening the Uganda Museum's site"
            ]
        );
        assert_eq!(ran.opened, ["https://ugandamuseum.go.ug/"]);
        assert_eq!(ran.searched.len(), 1, "one search");
        assert!(ran.acted.is_empty());
        assert!(
            matches!(ran.said.last(), Some(Said::Done { .. })),
            "{:?}",
            ran.said
        );
        // No page sent at first; the results as words; then the page.
        assert!(
            ran.bodies[0].contains(r#"\"page\":null"#),
            "{}",
            ran.bodies[0]
        );
        assert!(
            ran.bodies[0].contains(r#"\"sites_left\":3"#),
            "{}",
            ran.bodies[0]
        );
        assert!(
            ran.bodies[1].contains("1. Uganda Museum \u{2014} https://ugandamuseum.go.ug/"),
            "{}",
            ran.bodies[1]
        );
        assert!(
            ran.bodies[2].contains(r#"\"address\":\"https://ugandamuseum.go.ug/\""#),
            "{}",
            ran.bodies[2]
        );
        assert!(
            ran.bodies[2].contains(r#"\"sites_left\":2"#),
            "{}",
            ran.bodies[2]
        );
    }

    /// Queue item 884: by default, *Never* — the site neither found nor
    /// named opens with no question, the person never asked.
    #[test]
    fn by_default_a_site_neither_found_nor_named_opens_with_no_question() {
        let ran = begun(
            "go straight to the Wikipedia article on the Uganda Museum",
            vec![calling(
                "open",
                r#"{"result":null,"address":"wikipedia.org","step":"Opening Wikipedia","why":"","plan":null,"on":1}"#,
            )],
            page(),
            Limits::default(),
            Run::not_now,
        );
        assert_eq!(ran.opened, vec!["https://wikipedia.org/".to_owned()]);
        assert!(
            !ran.said
                .iter()
                .any(|said| matches!(said, Said::Asking { .. })),
            "{:?}",
            ran.said
        );
    }

    #[test]
    fn a_site_neither_found_nor_named_is_asked_and_not_now_opens_nothing() {
        let ran = begun(
            "find the opening hours of the Uganda Museum",
            vec![calling(
                "open",
                r#"{"result":null,"address":"wikipedia.org","step":"Opening Wikipedia","why":"","plan":null,"on":1}"#,
            )],
            page(),
            Limits::checked(3, 20, alo_renderer::agent_limits::Asks::Unnamed).unwrap(),
            Run::not_now,
        );
        assert!(ran.opened.is_empty(), "{:?}", ran.opened);
        assert!(
            ran.said.contains(&Said::Asking {
                question: 1,
                what: "alo wants to open wikipedia.org. It is not a site a search found or one \
                       you named."
                    .to_owned(),
                paying: None,
            }),
            "{:?}",
            ran.said
        );
        assert_eq!(
            ran.said.last(),
            Some(&Said::Stopped {
                why: "Not done: you said not now. alo did not open wikipedia.org.".to_owned()
            })
        );
        assert_eq!(ran.bodies.len(), 1, "the model is not asked again");
    }

    #[test]
    fn this_machines_address_is_refused_unless_named_and_the_sites_are_counted() {
        // A result on this machine's network is refused, the model told.
        let ran = begun(
            "find the museum",
            vec![
                calling(
                    "search",
                    r#"{"words":"museum","step":"","why":"","plan":null,"on":1}"#,
                ),
                calling(
                    "open",
                    r#"{"result":2,"address":null,"step":"","why":"","plan":null,"on":1}"#,
                ),
                calling(
                    "open",
                    r#"{"result":null,"address":"localhost:8080","step":"","why":"","plan":null,"on":1}"#,
                ),
                calling("stuck", r#"{"why":"Nothing to open.","choices":[]}"#),
            ],
            page(),
            Limits::default(),
            |_, _| panic!("nothing is asked"),
        );
        assert!(ran.opened.is_empty(), "{:?}", ran.opened);
        for body in &ran.bodies[2..4] {
            assert!(body.contains("on this machine or its network"), "{body}");
        }
        // Named by the person, it opens; and a second site past a limit of
        // one is refused.
        let one = Limits::checked(1, 20, alo_renderer::agent_limits::Asks::Unnamed).unwrap();
        let ran = begun(
            "read 127.0.0.1:8080/page.html, then the museum",
            vec![
                calling(
                    "open",
                    r#"{"result":null,"address":"127.0.0.1:8080/page.html","step":"","why":"","plan":null,"on":1}"#,
                ),
                calling(
                    "search",
                    r#"{"words":"museum","step":"","why":"","plan":null,"on":1}"#,
                ),
                calling(
                    "open",
                    r#"{"result":1,"address":null,"step":"","why":"","plan":null,"on":1}"#,
                ),
                calling("done", r#"{"result":"Read it."}"#),
            ],
            page(),
            one,
            |_, _| panic!("nothing is asked"),
        );
        assert_eq!(ran.opened, ["http://127.0.0.1:8080/page.html"]);
        assert!(
            ran.bodies[3].contains("alo has opened the 1 site one task may open"),
            "{}",
            ran.bodies[3]
        );
    }

    #[test]
    fn its_limits_are_the_persons_read_as_it_starts() {
        // Queue item 878: five actions, then it stops.
        let five = Limits::checked(3, 5, alo_renderer::agent_limits::Asks::Unnamed).unwrap();
        let script = (0..8)
            .map(|_| {
                calling(
                    "search",
                    r#"{"words":"museum","step":"Searching","why":"","plan":null,"on":1}"#,
                )
            })
            .collect();
        let ran = begun("find it", script, page(), five, |_, _| {});
        assert_eq!(ran.searched.len(), 5, "five searches, one an action each");
        assert_eq!(steps_of(&ran.said).len(), 5);
        assert_eq!(
            ran.said.last(),
            Some(&Said::Stopped {
                why: "alo stopped after 5 steps without finishing. Ask again to go on from \
                      what the page shows now."
                    .to_owned()
            })
        );
        // Asking before every site: even a search's result is asked.
        let every = Limits::checked(3, 20, alo_renderer::agent_limits::Asks::Every).unwrap();
        let ran = begun(
            "find it",
            vec![
                calling(
                    "search",
                    r#"{"words":"museum","step":"","why":"","plan":null,"on":1}"#,
                ),
                calling(
                    "open",
                    r#"{"result":1,"address":null,"step":"","why":"","plan":null,"on":1}"#,
                ),
                calling("done", r#"{"result":"Opened."}"#),
            ],
            page(),
            every,
            Run::allow,
        );
        assert!(ran.said.iter().any(|said| matches!(said, Said::Asking { what, .. } if what == "alo wants to open ugandamuseum.go.ug.")));
        assert_eq!(ran.opened, ["https://ugandamuseum.go.ug/"]);
    }

    #[test]
    fn a_sign_in_page_reached_hands_the_page_over_and_it_carries_on() {
        let mut signing_in = page();
        if let Some(root) = signing_in.root.as_mut() {
            let mut field = node(30, KnownRole::TextBox, Some("Password"), Vec::new());
            field.states.takes_text = true;
            root.children.push(field);
        }
        let ran = begun(
            "read my orders on shop.example",
            vec![
                calling(
                    "open",
                    r#"{"result":null,"address":"shop.example/orders","step":"Opening the shop","why":"","plan":null,"on":1}"#,
                ),
                calling("done", r#"{"result":"Read them."}"#),
            ],
            signing_in,
            Limits::default(),
            |_, _| panic!("a site named is opened with no question"),
        );
        assert_eq!(ran.opened, ["https://shop.example/orders"]);
        assert!(ran.said.contains(&Said::SignIn), "{:?}", ran.said);
        assert!(matches!(ran.said.last(), Some(Said::Done { .. })));
        // The model told, as a step of the person's.
        assert!(ran.bodies[1].contains(SIGNED_IN), "{}", ran.bodies[1]);
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

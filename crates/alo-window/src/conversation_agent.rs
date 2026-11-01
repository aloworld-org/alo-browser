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
//! # A password
//!
//! Never typed: the card refuses text put into a password field for every
//! agent (`chrome_act`, [`Refused::Password`]), and the run then says it is
//! stuck — signing in is the person's.

use crate::agent_gate;
use crate::agent_page;
use crate::agent_protocol::{self, Asking, Call, Taken};
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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

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
    },
    /// It waits on the person: the question numbered so, and its words.
    Asking {
        /// The question's number, this run's own.
        question: u64,
        /// What the person is asked.
        what: String,
    },
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
}

/// The agent, as the window holds it: the model it asks, and the orders it
/// reaches the card by — the agent socket's own.
#[derive(Debug, Clone)]
pub struct Agent {
    provider: Provider,
    orders: Orders,
}

impl Agent {
    /// The agent asking `provider`, reaching the card by `orders`: the
    /// relay's ([`crate::conversation_agents::relay`]).
    pub fn new(provider: Provider, orders: Orders) -> Self {
        Self { provider, orders }
    }

    /// Start run `run` of the agent on what the person `asked`, telling the
    /// window with `tell`. Returns at once.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(&self, run: u64, asked: String, tell: TellAgent) -> std::io::Result<Run> {
        let stopped = Arc::new(AtomicBool::new(false));
        let (person, answers) = channel();
        let running = Running {
            agent: self.clone(),
            run,
            asked,
            stopped: Arc::clone(&stopped),
            answers,
            tell,
            steps: Vec::new(),
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
    answers: Receiver<Person>,
    tell: TellAgent,
    steps: Vec<Taken>,
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
            if self.steps.len() >= MOST_STEPS {
                return Some(stopped(format!(
                    "alo stopped after {MOST_STEPS} steps without finishing. Ask again to go on \
                     from what the page shows now."
                )));
            }
            let (tab, snapshot) = match self.read() {
                Ok(read) => read,
                Err(ended) => return ended,
            };
            let call = match self.choose(&mut pool, &tab, &snapshot) {
                Ok(call) => call,
                Err(ended) => return ended,
            };
            match call {
                Call::Done { result } => return Some(Said::Done { result }),
                Call::Stuck { why, choices } => return Some(Said::Stuck { why, choices }),
                Call::Act {
                    node,
                    verb,
                    step,
                    why,
                } => {
                    if let Err(ended) = self.act(node, verb, step, why) {
                        return ended;
                    }
                }
            }
        }
    }

    /// Ask the page through the relay, as an agent on the socket asks it.
    fn ask(&self, ask: Ask) -> Result<Answer, Ended> {
        if self.is_stopped() {
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

    /// The page as it is now: the one tab the relay answers, and its tree.
    fn read(&self) -> Result<(String, Box<Snapshot>), Ended> {
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
            Answer::Tree { snapshot, .. } => Ok((tab, snapshot)),
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
                address: tab,
                tree: &tree,
                steps: &self.steps,
                left: MOST_STEPS - self.steps.len(),
            },
        );
        let request = self
            .agent
            .provider
            .post(body, agent_protocol::LARGEST, &mut self.identities);
        let fetched = pool.fetch(&request);
        if self.is_stopped() {
            // An answer in flight is thrown away.
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
        if !self.say(Said::Step {
            line: line.clone(),
            why,
        }) {
            return Err(None);
        }
        // Judged on the page as it is now, not as the model was sent it.
        let (now_at, now) = self.read()?;
        let host = alo_url::parse(&now_at)
            .map(|url| alo_url::reading::host(&url))
            .unwrap_or_default();
        if let Some(question) = agent_gate::asked(&now, node, &verb, &host) {
            self.ask_the_person(&question.what, &verb, node, &host)?;
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
            other => other.said().unwrap_or_default(),
        };
        self.steps.push(Taken {
            step: line,
            came_to,
        });
        Ok(())
    }

    /// Ask the person `what`, and wait for their answer: go on after
    /// *Allow*, once the page still asks the same; end after *Not now*, or
    /// once stopped.
    fn ask_the_person(
        &mut self,
        what: &str,
        verb: &Verb,
        node: BoxId,
        host: &str,
    ) -> Result<(), Ended> {
        self.questions += 1;
        let question = self.questions;
        if !self.say(Said::Asking {
            question,
            what: what.to_owned(),
        }) {
            return Err(None);
        }
        loop {
            if self.is_stopped() {
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
        if still.as_ref().map(|still| still.what.as_str()) != Some(what) {
            return Err(Some(stopped(
                "alo stopped: the page changed while you were asked, so nothing was pressed."
                    .to_owned(),
            )));
        }
        let _ = self.say(Said::Allowed);
        Ok(())
    }
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

    /// A card's thread answering every order with [`page`], and keeping
    /// each verb it was asked to carry out.
    fn card() -> (Orders, Acted) {
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
                        snapshot: Box::new(page()),
                    },
                    Ok(Ask::Act { tab, target, verb }) => {
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
        let (orders, acted) = card();
        let (endpoint, bodies) = model(script);
        let provider = Provider::at(&endpoint, Key::of("test-key-not-real").unwrap()).unwrap();
        let agent = Agent::new(provider, orders);
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
            if let Said::Asking { question, .. } = &heard.said {
                person(&running, *question);
            }
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
        assert!(said.contains(&Said::Asking {
            question: 1,
            what:
                "Press \u{201c}Send\u{201d} on 127.0.0.1:9? It sends something from you.".to_owned()
        }));
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
}

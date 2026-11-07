/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 707 (ADR 0081, ADR 0095): a search's answer, asked of the
//! model once its sites have come and drawn — through **the window's own
//! road**: the functions `alo --conversation`'s event loop calls for each
//! event ([`ConversationWindow`]), the two confined renderers, the thread
//! that searches and the thread that asks for the answer, every thread's
//! news given back as the event loop gives it.
//!
//! **Never the real APIs, and never the network.** One server on
//! `127.0.0.1` answers hand-written replies in Brave's shape and in
//! `OpenAI`'s Responses API's shape, given to the threads by `Provider::at`,
//! with keys that are not keys. It records what it was asked, and the test
//! asserts what was sent: a `POST` carrying the key in its one header, the
//! person's words and the sites' words, `store: false`, and nothing else.
//!
//! Each asserted:
//!
//! 1. Return on words: the sites, then the answer, drawn at the top of All
//!    — its rule painted — and *Full answer ›* pressed opens the Answer tab.
//! 2. A model saying the sites do not answer: *I couldn't find a reliable
//!    answer*, and nothing made up.
//! 3. A model citing a source it was not given: refused, said in words,
//!    never drawn.
//! 4. The key refused (401): said in words where the answer would be, with
//!    no key in them.
//! 5. A search that found no sites: no answer asked, nothing sent.
//! 6. With no key for the model: said where the answer would be, and
//!    nothing sent.
//! 7. Kept in its chat (queue item 814, ADR 0092 as amended): the answer
//!    written to the chat's file in words, and after a second launch on the
//!    same directory the chat pressed under *Earlier* shows it on the
//!    Answer tab as it was — **with no request to the model**.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Showing, Tab};
use alo_renderer::results_answer::{Answer, NO_RELIABLE_ANSWER};
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::chat_seal::Key as ChatKey;
use alo_window::chat_store::ChatStore;
use alo_window::conversation_answer::Answerer;
use alo_window::conversation_chrome::ChromeOpener;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_search::Searcher;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::openai_answer::{self, MODEL};
use alo_window::scale::Scale;
use alo_window::search_key::{Held, Key as SearchKey};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// Where the first screen begins in the window, at one: right of the
/// panel's 240.
const PANEL: f64 = 240.0;

/// The box under results, at one: pinned 24 above the window's foot, 56
/// tall (queue item 750).
const THE_BOX_UNDER_RESULTS: (f64, f64) = (640.0, 768.0);

/// What the test's search provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-search";

/// What the test's model is given as a key: not a key.
const NOT_A_MODEL_KEY: &str = "test-key-for-a-local-model";

/// Three sites, in Brave's shape, for any words but `nothing`.
const SITES: &str = r#"{"type":"search","web":{"type":"search","results":[
    {"title":"Installing a charger","url":"https://evhomeguide.org/a",
     "description":"Most fittings take half a day to a day.","profile":{"name":"EV Home Guide"}},
    {"title":"Chargers compared","url":"https://compare.example/"},
    {"title":"A new circuit","url":"https://electricians.example/c",
     "description":"A new circuit can add a day."}
  ]}}"#;

/// A response in the Responses API's shape holding `text`.
fn response(text: &str) -> String {
    format!(
        r#"{{"id":"resp_test","object":"response","status":"completed","model":"{MODEL}",
            "output":[{{"type":"message","role":"assistant","content":[
              {{"type":"output_text","text":{},"annotations":[]}}]}}],
            "usage":{{"input_tokens":700,"output_tokens":90,"total_tokens":790}}}}"#,
        json_string(text)
    )
}

/// `text` as a JSON string, written by hand: no JSON crate in a test.
fn json_string(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            c => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

/// What the model answers, by what the person's words were.
fn answered(body: &str) -> (&'static str, String) {
    if body.contains("unclear question") {
        return (
            "200 OK",
            response(r#"{"answered":false,"line":"","sentences":[],"points":[]}"#),
        );
    }
    if body.contains("lying model") {
        return (
            "200 OK",
            response(
                r#"{"answered":true,"line":"Made up.","sentences":[{"text":"Made up.","sources":[9]}],"points":[]}"#,
            ),
        );
    }
    if body.contains("key refused") {
        return (
            "401 Unauthorized",
            r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#
                .to_owned(),
        );
    }
    (
        "200 OK",
        response(
            r#"{"answered":true,"line":"About a day.","sentences":[{"text":"Fitting takes half a day to a day.","sources":[1]},{"text":"A new circuit adds more.","sources":[3]}],"points":[{"text":"Half a day to a day.","sources":[1]}]}"#,
        ),
    )
}

/// What a provider was asked: the request's head and its body.
type Asked = (String, String);

/// A server on this machine answering both providers: its port. Each
/// request is told to `asked`.
fn provide(asked: Sender<Asked>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let asked = asked.clone();
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut read = Vec::new();
                let mut block = [0u8; 4096];
                let mut ends = None;
                while let Ok(got) = socket.read(&mut block) {
                    read.extend_from_slice(block.get(..got).unwrap_or_default());
                    if ends.is_none() {
                        ends = read
                            .windows(4)
                            .position(|end| end == b"\r\n\r\n")
                            .map(|at| at + 4);
                    }
                    let whole = ends.is_some_and(|ends| {
                        let head = String::from_utf8_lossy(read.get(..ends).unwrap_or_default())
                            .to_ascii_lowercase();
                        let length = head
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .and_then(|length| length.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        read.len() >= ends + length
                    });
                    if got == 0 || whole {
                        break;
                    }
                }
                let ends = ends.unwrap_or(read.len());
                let head =
                    String::from_utf8_lossy(read.get(..ends).unwrap_or_default()).into_owned();
                let body =
                    String::from_utf8_lossy(read.get(ends..).unwrap_or_default()).into_owned();
                let target = head.split(' ').nth(1).unwrap_or_default().to_owned();
                let (status, reply) = if target == "/v1/responses" {
                    answered(&body)
                } else if target.ends_with("q=nothing") {
                    ("200 OK", r#"{"type":"search"}"#.to_owned())
                } else {
                    ("200 OK", SITES.to_owned())
                };
                let _ = asked.send((head, body));
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    /// Every line the window said on the terminal.
    said: Vec<String>,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let until = Instant::now() + AT_MOST;
        while !done(self) {
            let left = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("{what} did not happen; the window said {:?}", self.said))?;
            let Ok(heard) = self
                .heard
                .recv_timeout(left.min(Duration::from_millis(100)))
            else {
                continue;
            };
            if let Heard::Drawn(Drawn::Said(part, said)) = &heard {
                return Err(format!("{part} said {said}"));
            }
            self.window.heard(heard);
            self.said.extend(self.window.take_said());
            let _ = self.window.take_redraw();
        }
        Ok(())
    }

    /// A person's click at `at`, in the window's device pixels.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// `text` typed, as the window reads keys, then Return.
    fn type_and_return(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.window.key(&Press::of(Key::Return), None);
    }

    /// The first screen as its renderer last painted it.
    fn painted(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
    }
}

/// The first screen, as the window asks it drawn now.
fn start(window: &ConversationWindow) -> alo_renderer::start::Start {
    window
        .conversation()
        .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The conversation's window at one, both parts painted, searching through
/// the server on `port` and asking it for answers — or, with `model` an
/// error, with answers not set up.
fn the_window(
    port: u16,
    model: Result<(), String>,
    chats: Option<&Path>,
) -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let drawn = tell.clone();
    let told = tell.clone();
    let answers = tell.clone();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |d| drawn.send(Heard::Drawn(d)).is_ok()),
    )
    .map_err(|why| format!("no drawing thread: {why}"))?;
    // Never asked here: no address is chosen.
    let chrome = ChromeOpener::start(
        Chromium::Machines,
        Box::new(move |w| tell.send(Heard::Chrome(w)).is_ok()),
    )
    .map_err(|why| format!("no Chrome thread: {why}"))?;
    let key = SearchKey::of(NOT_A_KEY).ok_or("the test's key is not one a header carries")?;
    let provider = Provider::at(&format!("http://127.0.0.1:{port}/res/v1/web/search"), key)?;
    let model_key =
        SearchKey::of(NOT_A_MODEL_KEY).ok_or("the test's key is not one a header carries")?;
    let model = model.and_then(|()| {
        openai_answer::Provider::at(&format!("http://127.0.0.1:{port}/v1/responses"), model_key)
    });
    let answerer = Answerer::start(
        model,
        Box::new(move |s| answers.send(Heard::Searched(s)).is_ok()),
    )
    .map_err(|why| format!("no answer thread: {why}"))?;
    let searcher = Searcher::answering(
        Ok(provider),
        Some(answerer),
        Box::new(move |s| told.send(Heard::Searched(s)).is_ok()),
    )
    .map_err(|why| format!("no search thread: {why}"))?;
    let mut window =
        ConversationWindow::new(Some(Scale::ONE), drawing, chrome).with_search(searcher);
    if let Some(chats) = chats {
        let (store, opened) = ChatStore::open(chats, test_key())?;
        window = window.with_chats(store, opened);
    }
    window.measured_as(Measured {
        size: WINDOW,
        scale_factor: 1.0,
        outer: Some((100, 100)),
    });
    window.focused(true);
    window.ask_drawn();
    let mut driven = Driven {
        window,
        heard,
        said: Vec::new(),
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        start(&driven.window).focused
    })?;
    Ok(driven)
}

/// The answer of the newest search.
fn answer(driven: &Driven) -> Option<Answer> {
    driven
        .window
        .conversation()
        .results()
        .map(|results| results.answer.clone())
}

/// Search for `words` and wait until its answer is no longer asked for.
fn asked_for(driven: &mut Driven, words: &str) -> Result<Answer, String> {
    driven.type_and_return(words);
    driven.until(&format!("the answer to {words}"), |driven| {
        matches!(
            driven.window.conversation().results(),
            Some(results) if results.said == words
                && matches!(results.showing, Showing::Found(_))
                && !matches!(results.answer, Answer::Asking)
        )
    })?;
    answer(driven).ok_or_else(|| "no results".to_owned())
}

/// Every request `asked` holds now, after waiting a moment for stragglers.
fn every(asked: &Receiver<Asked>) -> Vec<Asked> {
    std::thread::sleep(Duration::from_millis(200));
    asked.try_iter().collect()
}

/// What `requests` sent the model: once, a `POST` with the key in its one
/// header and JSON, no cookie, referrer, origin or organisation, and a body
/// holding the person's words, the sites' words, the model and `store:
/// false` — and neither key. Why not, if it is not so.
fn sent_once_and_only_what_it_may(requests: &[Asked]) -> Result<(), String> {
    let to_the_model: Vec<&Asked> = requests
        .iter()
        .filter(|(head, _)| head.starts_with("POST /v1/responses "))
        .collect();
    let [(head, body)] = to_the_model.as_slice() else {
        return Err(format!("not one answer for one search: {to_the_model:?}"));
    };
    let lower = head.to_ascii_lowercase();
    if !lower.contains(&format!("\r\nauthorization: bearer {NOT_A_MODEL_KEY}\r\n"))
        || !lower.contains("\r\ncontent-type: application/json\r\n")
    {
        return Err(format!("the key or the type not in their headers: {head}"));
    }
    for never in [
        "cookie:",
        "referer:",
        "origin:",
        "openai-organization:",
        NOT_A_KEY,
    ] {
        if lower.contains(never) {
            return Err(format!("{never} was sent: {head}"));
        }
    }
    for words in [
        "\"store\":false",
        &format!("\"model\":\"{MODEL}\""),
        "ev charger install time",
        "Installing a charger",
        "Most fittings take half a day to a day.",
        "https://electricians.example/c",
    ] {
        if !body.contains(words) {
            return Err(format!("{words} not sent: {body}"));
        }
    }
    if body.contains(NOT_A_MODEL_KEY) || body.contains(NOT_A_KEY) {
        return Err(format!("a key in the body: {body}"));
    }
    Ok(())
}

#[test]
fn a_searchs_answer_is_asked_once_its_sites_come_and_drawn_or_refused() -> Result<(), String> {
    let (asking, asked) = channel();
    let port = provide(asking)?;
    let mut driven = the_window(port, Ok(()), None)?;

    // 1. Answered, drawn, and Full answer opens the Answer tab.
    let answer = asked_for(&mut driven, "ev charger install time")?;
    let Answer::Written(written) = &answer else {
        return Err(format!("not written: {answer:?}"));
    };
    assert_eq!(written.used(), vec![1, 3]);
    assert_eq!(written.line, "About a day.");
    driven.until("the answer's rule painted", |driven| {
        driven.painted().is_some_and(|frame| {
            (150..400).any(|y| pixel(&frame, 740, y) == Some([0xe1, 0xda, 0xce, 0xff]))
        })
    })?;
    sent_once_and_only_what_it_may(&every(&asked))?;
    assert!(
        driven.said.iter().any(|said| said
            == &format!(
                "OpenAI's {MODEL} answered \u{201c}ev charger install time\u{201d} from 2 sites \
                 (700 tokens in, 90 out)"
            )),
        "{:?}",
        driven.said
    );
    // Full answer ›, 10 under one line of sentences: 164 + 26 + 10.
    driven.click((PANEL + 110.0, 209.0));
    driven.until("the Answer tab shown", |driven| {
        driven
            .window
            .conversation()
            .results()
            .is_some_and(|results| results.tab == Tab::Answer)
    })?;
    // The press took the keyboard from the box: given back.
    driven.click(THE_BOX_UNDER_RESULTS);
    driven.until("the box given the keyboard again", |driven| {
        start(&driven.window).focused
    })?;

    // 2. No reliable answer: said, nothing made up.
    let answer = asked_for(&mut driven, "unclear question")?;
    assert_eq!(answer, Answer::NotFound);
    assert_eq!(answer.note().as_deref(), Some(NO_RELIABLE_ANSWER));

    // 3. A citation of a source not given: refused, said, never drawn.
    let answer = asked_for(&mut driven, "lying model")?;
    assert_eq!(
        answer,
        Answer::Unanswered(
            "alo could not write an answer: OpenAI's answer could not be read: it was refused: \
             the answer cites source 9, and it was given 3."
                .to_owned()
        )
    );

    // 4. The key refused: said, with no key in it.
    let answer = asked_for(&mut driven, "key refused")?;
    assert_eq!(
        answer,
        Answer::Unanswered(openai_answer::unanswered(401, Held::Keychain))
    );
    let _ = every(&asked);

    // 5. No sites: no answer asked.
    driven.type_and_return("nothing");
    driven.until("nothing found", |driven| {
        matches!(
            driven.window.conversation().results(),
            Some(results) if results.said == "nothing" && matches!(&results.showing, Showing::Found(found) if found.is_empty())
        )
    })?;
    assert_eq!(answer_of_newest(&driven), Some(Answer::NotAsked));
    let requests = every(&asked);
    assert!(
        requests.iter().all(|(head, _)| !head.starts_with("POST")),
        "{requests:?}"
    );
    assert!(
        driven
            .said
            .iter()
            .all(|said| !said.contains(NOT_A_KEY) && !said.contains(NOT_A_MODEL_KEY)),
        "{:?}",
        driven.said
    );

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}

/// The answer of the newest search, as [`answer`].
fn answer_of_newest(driven: &Driven) -> Option<Answer> {
    answer(driven)
}

#[test]
fn with_no_key_for_the_model_the_answer_says_why_and_nothing_is_sent() -> Result<(), String> {
    let (asking, asked) = channel();
    let port = provide(asking)?;
    let mut driven = the_window(
        port,
        Err("no OpenAI key in this Mac's keychain (alo-openai)".to_owned()),
        None,
    )?;
    let answer = asked_for(&mut driven, "ev charger install time")?;
    assert_eq!(
        answer,
        Answer::Unanswered(
            "alo\u{2019}s answers are not set up yet: no OpenAI key in this Mac's keychain \
             (alo-openai)."
                .to_owned()
        )
    );
    let requests = every(&asked);
    assert!(
        requests.iter().all(|(head, _)| !head.starts_with("POST")),
        "{requests:?}"
    );
    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}

/// The first row under *Earlier* with one chat open, at one: the open row
/// at 178 and the earlier ones from 254, each 38 tall.
const FIRST_EARLIER: (f64, f64) = (120.0, 273.0);

/// A directory of this test's own, never a person's.
fn scratch() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("alo-an-answer-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// The rows of `frame` from 100 to 600 — the tab row, the answer and its
/// sources — as bytes, to be compared with another launch's.
fn answer_rows(frame: &Frame) -> Option<Vec<u8>> {
    let row = usize::try_from(frame.width).ok()? * 4;
    frame.pixels.get(100 * row..600 * row).map(<[u8]>::to_vec)
}

#[test]
fn a_searchs_answer_is_kept_in_its_chat_and_read_back_without_asking_again() -> Result<(), String> {
    let (asking, asked) = channel();
    let port = provide(asking)?;
    let chats = scratch();
    let mut driven = the_window(port, Ok(()), Some(&chats))?;
    let answer = asked_for(&mut driven, "ev charger install time")?;
    assert!(matches!(answer, Answer::Written(_)), "{answer:?}");
    driven.click((PANEL + 110.0, 209.0));
    // The Answer tab shown, and the frame held the one drawn of it: the
    // window holds painted what it last asked drawn (queue item 862). A
    // frame of All has the line's colour at 740 too, under its answer —
    // which is how a fixed wait once took All's picture as the Answer
    // tab's under load.
    driven.until("the Answer tab shown and painted", |driven| {
        driven
            .window
            .conversation()
            .results()
            .is_some_and(|results| results.tab == Tab::Answer)
            && driven.window.drawn_as_asked()
            && driven.painted().is_some_and(|frame| {
                (150..400).any(|y| pixel(&frame, 740, y) == Some([0xe1, 0xda, 0xce, 0xff]))
            })
    })?;
    let before = driven
        .painted()
        .and_then(|frame| answer_rows(&frame))
        .ok_or("the Answer tab not painted")?;
    driven.window.finish();
    if let Some(why) = driven.window.unfinished() {
        return Err(why.to_owned());
    }
    let written = kept_words(&chats, 0)?;
    for words in [
        "\"line\":\"About a day.\"",
        "{\"sources\":[1],\"text\":\"Fitting takes half a day to a day.\"}",
        "\"tab\":\"answer\"",
    ] {
        assert!(written.contains(words), "{words} not kept: {written}");
    }
    let _ = every(&asked);

    // The second launch: the chat pressed under Earlier, its answer drawn
    // on the Answer tab as it was, and nothing asked of the model.
    let mut driven = the_window(port, Ok(()), Some(&chats))?;
    driven.click(FIRST_EARLIER);
    driven.until("the kept chat's answer drawn", |driven| {
        driven
            .window
            .conversation()
            .results()
            .is_some_and(|results| results.tab == Tab::Answer && results.answer == answer)
            && driven.window.drawn_as_asked()
            && driven
                .painted()
                .and_then(|frame| answer_rows(&frame))
                .is_some_and(|rows| rows == before)
    })?;
    driven.window.finish();
    let requests = every(&asked);
    assert!(
        requests
            .iter()
            .all(|(head, _)| !head.starts_with("POST /v1/responses ")),
        "the model was asked again: {requests:?}"
    );
    let _ = std::fs::remove_dir_all(&chats);
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}

/// The key this test's chats are sealed under: its own, never the
/// keychain's (ADR 0098).
fn test_key() -> ChatKey {
    ChatKey::of([0x2a; 32])
}

/// The words of the chat numbered `number` kept in `chats`, opened under
/// the test's key.
fn kept_words(chats: &std::path::Path, number: u64) -> Result<String, String> {
    ChatStore::open(chats, test_key())?.0.words(number)
}

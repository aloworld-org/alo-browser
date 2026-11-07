/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A chat as it is written to its file and read back (queue item 757, ADR
//! 0092 §§ 2–3): its name and its turns, as text.
//!
//! # What is written
//!
//! JSON, one object: `"format": "alo chat"`, `"version": 1`, the chat's
//! `"name"` (empty while it is *New chat*) and its `"turns"`, oldest first.
//! Each turn is what the conversation shows of it, in words and addresses:
//!
//! - **A search** — what the person said, the tab shown, and each of its
//!   four searches as it stands: the sites' titles, addresses, descriptions
//!   and names; the images' titles, pages and sources; the videos' titles,
//!   pages, lengths, sites, ages and descriptions; the stories' titles,
//!   pages, sites, ages and descriptions; or why one was not answered.
//! - **A page** — live or shown earlier — as what was said and the address
//!   it was last at: a live card is written as the page shown earlier it
//!   becomes when anything newer comes (ADR 0085 § 2). With it, **each run
//!   of alo's agent on it** (queue item 820): what was asked, each step's
//!   line and why, and how it ended, as the card keeps them — never a text
//!   typed that the person did not say (`crate::agent_kept`); a run still
//!   at work is written as alo closing stops it.
//! - **A page that could not be opened** — what was said, and why not.
//!
//! - **A search's answer** (queue item 814), in words: a written one's
//!   line, sentences and points, each with the numbers of the sites it
//!   cites; or that none reliable was found; or why it was not answered; or
//!   that it was never asked. A chat written before 814 holds none, and is
//!   read as never asked.
//!
//! **Never written:** a picture's bytes — a thumbnail, a site's mark, a
//! card's frame — nor anything typed and not yet chosen, how far the chat
//! was scrolled, a provider's key, or anything of a Chromium's profile.
//! Pictures are bytes a provider sent for one showing; written, they would
//! make a chat's file hundreds of times the size of its words (ADR 0092
//! § 2).
//!
//! # What is read back
//!
//! The same, or nothing: a file that is not this format and version, or
//! has a field missing or of the wrong kind, or an address that is not an
//! `http` or `https` one where a site's must be, is **not read at all**
//! ([`Unread`]), never read in part — and the store leaves it where it is
//! and says so (`crate::chat_store`). What was under way when alo closed
//! cannot be under way after it: a search still searching is read as
//! unanswered, saying why ([`NOT_ANSWERED`]), and a tab's results still
//! being asked for as never asked, so opening the tab asks again (ADR 0087).
//! An answer is read back **checked as the wire checks it**
//! (`alo_renderer::wire`): none to a search that found no sites, and a
//! written one only if [`Written::check`] passes against the first
//! [`MOST_SOURCES`] sites read with it — every number cited one of them,
//! no more sentences or points than are drawn — or the whole file is not
//! read. An answer still being written when alo closed is read as not
//! written, saying why ([`ANSWER_NOT_WRITTEN`]); **a chat read back never
//! asks the model again**: what it shows is what was written.
//! A search's sites past its first page are the further pages a person
//! asked for (queue item 761), counted again from how many there are; no
//! further page is offered for a chat read back, since whether there is
//! one is the provider's to say, and more than [`MOST_SITES`] are not read.
//! Nothing read asks anything of the network.

use alo_renderer::agent_plan::Plan;
use alo_renderer::agent_ran::{Ended, MOST_RUNS, Ran};
use alo_renderer::agent_work::{MOST_STEPS, Step};
use alo_renderer::results::{
    Asked, Found, MOST_FOUND, MOST_SITES, Pictured, Results, Showing, Tab,
};
use alo_renderer::results_answer::{Answer, Cited, MOST_SOURCES, Written};
use alo_renderer::results_media::{self, Story, Video};
use alo_renderer::turn::{MOST_TURNS, Turn};
use alo_url::Url;
use serde_json::{Map, Value};

/// What every chat's file says it is.
pub const FORMAT: &str = "alo chat";

/// The version of [`FORMAT`] this browser writes, and the only one it
/// reads.
pub const VERSION: u64 = 1;

/// The largest file read as a chat: past any chat of [`MOST_TURNS`] turns
/// written in words.
pub const LARGEST: u64 = 16 * 1024 * 1024;

/// Why a search kept while it was still searching is not answered.
pub const NOT_ANSWERED: &str = "This search was not answered before alo was closed.";

/// Why a search's answer still being written when alo closed is not
/// there (queue item 814): it is never asked again for a chat read back.
pub const ANSWER_NOT_WRITTEN: &str = "This answer was not written before alo was closed.";

/// A chat as it is kept: its name and its turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// What the chat is called; empty while it is *New chat*.
    pub name: String,
    /// Every turn, oldest first. None is a live card.
    pub turns: Vec<Turn>,
}

/// Why a file was not read as a chat, in a sentence's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread(pub String);

/// The text written for a chat called `name` holding `turns`.
pub fn write(name: &str, turns: &[Turn]) -> String {
    let mut chat = Map::new();
    chat.insert("format".to_owned(), Value::from(FORMAT));
    chat.insert("version".to_owned(), Value::from(VERSION));
    chat.insert("name".to_owned(), Value::from(name));
    chat.insert(
        "turns".to_owned(),
        Value::Array(turns.iter().map(turn).collect()),
    );
    let mut written = Value::Object(chat).to_string();
    written.push('\n');
    written
}

/// An object of `fields`.
fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

/// One turn, written.
fn turn(turn: &Turn) -> Value {
    match turn {
        Turn::Searched(results) => object([("searched", searched(results))]),
        Turn::Page(card) => visited(&card.said, &card.url, &card.ran),
        Turn::Visited { said, url, ran } => visited(said, url, ran),
        Turn::Failed { said, why } => object([(
            "failed",
            object([
                ("said", Value::from(said.as_str())),
                ("why", Value::from(why.as_str())),
            ]),
        )]),
    }
}

/// A page, written: what was said for it, where it last was, and each run
/// of the agent kept with it (queue item 820).
fn visited(said: &str, url: &Url, ran: &[Ran]) -> Value {
    object([(
        "visited",
        object([
            ("said", Value::from(said)),
            ("url", Value::from(url.to_string())),
            ("ran", ran.iter().map(run).collect()),
        ]),
    )])
}

/// A run of the agent, written as the chat keeps it: what was asked, its
/// plan as it last stood if it wrote one (queue item 824), each step's
/// line and why, and how it ended.
fn run(ran: &Ran) -> Value {
    let ended = match &ran.ended {
        Ended::Done { said } => object([("done", Value::from(said.as_str()))]),
        Ended::Stuck { why } => object([("stuck", Value::from(why.as_str()))]),
        Ended::Stopped { why } => object([("stopped", Value::from(why.as_str()))]),
    };
    let mut written = object([
        ("asked", Value::from(ran.asked.as_str())),
        (
            "steps",
            ran.steps
                .iter()
                .map(|step| {
                    object([
                        ("line", Value::from(step.line.as_str())),
                        ("why", Value::from(step.why.as_str())),
                    ])
                })
                .collect(),
        ),
        ("ended", ended),
    ]);
    if let (Some(plan), Value::Object(fields)) = (&ran.plan, &mut written) {
        fields.insert(
            "plan".to_owned(),
            object([
                (
                    "steps",
                    plan.steps
                        .iter()
                        .map(|step| Value::from(step.as_str()))
                        .collect(),
                ),
                ("on", Value::from(plan.on)),
            ]),
        );
    }
    written
}

/// The name a tab is written by.
fn tab_name(tab: Tab) -> &'static str {
    match tab {
        Tab::All => "all",
        Tab::Answer => "answer",
        Tab::Sites => "sites",
        Tab::Images => "images",
        Tab::Videos => "videos",
        Tab::News => "news",
    }
}

/// A search's results, written: its words, its tab, and its four searches.
fn searched(results: &Results) -> Value {
    let sites = match &results.showing {
        Showing::Searching => Value::from("searching"),
        Showing::Found(found) => object([("found", found.iter().map(site).collect())]),
        Showing::Unanswered(why) => object([("unanswered", Value::from(why.as_str()))]),
    };
    object([
        ("said", Value::from(results.said.as_str())),
        ("tab", Value::from(tab_name(results.tab))),
        ("sites", sites),
        ("images", asked(&results.images, image)),
        ("videos", asked(&results.videos, video)),
        ("news", asked(&results.news, story)),
        ("answer", answer(&results.answer)),
    ])
}

/// A search's answer, written in words (queue item 814): its line,
/// sentences and points with the numbers they cite, or what came instead.
fn answer(answer: &Answer) -> Value {
    let claims = |claims: &[Cited]| -> Value {
        claims
            .iter()
            .map(|cited| {
                object([
                    ("text", Value::from(cited.text.as_str())),
                    (
                        "sources",
                        cited
                            .sources
                            .iter()
                            .map(|&number| Value::from(number))
                            .collect(),
                    ),
                ])
            })
            .collect()
    };
    match answer {
        Answer::NotAsked => Value::from("not asked"),
        Answer::Asking => Value::from("asking"),
        Answer::Written(written) => object([(
            "written",
            object([
                ("line", Value::from(written.line.as_str())),
                ("sentences", claims(&written.sentences)),
                ("points", claims(&written.points)),
            ]),
        )]),
        Answer::NotFound => Value::from("not found"),
        Answer::Unanswered(why) => object([("unanswered", Value::from(why.as_str()))]),
    }
}

/// A further search, written, each thing found by `each`.
fn asked<T>(asked: &Asked<T>, each: fn(&T) -> Value) -> Value {
    match asked {
        Asked::NotAsked => Value::from("not asked"),
        Asked::Asking => Value::from("asking"),
        Asked::Found(found) => object([("found", found.iter().map(each).collect())]),
        Asked::Unanswered(why) => object([("unanswered", Value::from(why.as_str()))]),
    }
}

/// A site found, in words and its address; its mark never.
fn site(found: &Found) -> Value {
    object([
        ("title", Value::from(found.title.as_str())),
        ("url", Value::from(found.url.to_string())),
        ("description", Value::from(found.description.as_str())),
        ("name", Value::from(found.name.as_str())),
    ])
}

/// An image found, in words and its page's address; its picture and mark
/// never.
fn image(image: &Pictured) -> Value {
    object([
        ("title", Value::from(image.title.as_str())),
        ("page", Value::from(image.page.to_string())),
        ("source", Value::from(image.source.as_str())),
    ])
}

/// A video found, in words and its page's address; its picture and mark
/// never.
fn video(video: &Video) -> Value {
    object([
        ("title", Value::from(video.title.as_str())),
        ("page", Value::from(video.page.to_string())),
        ("length", Value::from(video.length.as_str())),
        ("site", Value::from(video.site.as_str())),
        ("age", Value::from(video.age.as_str())),
        ("description", Value::from(video.description.as_str())),
    ])
}

/// A news story found, in words and its page's address; its photo and
/// mark never.
fn story(story: &Story) -> Value {
    object([
        ("title", Value::from(story.title.as_str())),
        ("page", Value::from(story.page.to_string())),
        ("site", Value::from(story.site.as_str())),
        ("age", Value::from(story.age.as_str())),
        ("description", Value::from(story.description.as_str())),
    ])
}

/// The chat `bytes` hold, if they are one this browser wrote.
///
/// # Errors
///
/// [`Unread`], saying why, if they are not: too large, not JSON, another
/// format or version, a field missing or of the wrong kind, more turns
/// than a conversation holds, or an address that is not a site's.
pub fn read(bytes: &[u8]) -> Result<Record, Unread> {
    if !u64::try_from(bytes.len()).is_ok_and(|length| length <= LARGEST) {
        return Err(Unread(format!("it is larger than {LARGEST} bytes")));
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|why| Unread(format!("it is not JSON ({why})")))?;
    let chat = value
        .as_object()
        .ok_or_else(|| Unread("it is not an object".to_owned()))?;
    if chat.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(Unread(format!(
            "it does not say it is an \u{201c}{FORMAT}\u{201d}"
        )));
    }
    match chat.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        Some(other) => {
            return Err(Unread(format!(
                "it is version {other}, and this browser reads version {VERSION}"
            )));
        }
        None => return Err(Unread("it says no version".to_owned())),
    }
    let name = text(chat, "name")?;
    let turns = chat
        .get("turns")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("turns"))?;
    if turns.len() > MOST_TURNS {
        return Err(Unread(format!(
            "it holds {} turns, more than the {MOST_TURNS} a conversation holds",
            turns.len()
        )));
    }
    let turns = turns
        .iter()
        .enumerate()
        .map(|(place, each)| {
            read_turn(each).map_err(|Unread(why)| Unread(format!("turn {}: {why}", place + 1)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Record { name, turns })
}

/// That `field` is missing or not what it should be.
fn missing(field: &str) -> Unread {
    Unread(format!(
        "\u{201c}{field}\u{201d} is missing or not what it should be"
    ))
}

/// The string `field` of `object`.
fn text(object: &Map<String, Value>, field: &str) -> Result<String, Unread> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| missing(field))
}

/// The address `field` of `object`: an `http` or `https` one.
fn address(object: &Map<String, Value>, field: &str) -> Result<Url, Unread> {
    let written = text(object, field)?;
    match alo_url::parse(&written) {
        Ok(url) if matches!(url.scheme.as_str(), "http" | "https") => Ok(url),
        _ => Err(Unread(format!(
            "\u{201c}{field}\u{201d} is not a site\u{2019}s address"
        ))),
    }
}

/// One turn, read.
fn read_turn(value: &Value) -> Result<Turn, Unread> {
    let turn = value
        .as_object()
        .filter(|turn| turn.len() == 1)
        .ok_or_else(|| Unread("it is not one kind of turn".to_owned()))?;
    if let Some(Value::Object(results)) = turn.get("searched") {
        return read_searched(results).map(Turn::Searched);
    }
    if let Some(Value::Object(visited)) = turn.get("visited") {
        return Ok(Turn::Visited {
            said: text(visited, "said")?,
            url: address(visited, "url")?,
            ran: read_runs(visited)?,
        });
    }
    if let Some(Value::Object(failed)) = turn.get("failed") {
        return Ok(Turn::Failed {
            said: text(failed, "said")?,
            why: text(failed, "why")?,
        });
    }
    Err(Unread(
        "it is no kind of turn this browser keeps".to_owned(),
    ))
}

/// A search's results, read: a search still searching as unanswered, and a
/// further search still asking as never asked.
fn read_searched(object: &Map<String, Value>) -> Result<Results, Unread> {
    let said = text(object, "said")?;
    let showing = match object.get("sites") {
        Some(Value::String(state)) if state == "searching" => {
            Showing::Unanswered(NOT_ANSWERED.to_owned())
        }
        Some(Value::Object(sites)) if sites.len() == 1 => {
            if let Some(why) = sites.get("unanswered").and_then(Value::as_str) {
                Showing::Unanswered(why.to_owned())
            } else {
                Showing::Found(each_of(sites, read_site)?)
            }
        }
        _ => return Err(missing("sites")),
    };
    if let Showing::Found(found) = &showing
        && found.len() > MOST_SITES
    {
        return Err(Unread(format!(
            "it holds {} sites for one search, more than the {MOST_SITES} a search holds",
            found.len()
        )));
    }
    let mut results = Results::of(said, showing);
    // Sites past a first page were further pages a person asked for (queue
    // item 761): as many as they fill. Whether more could be asked is the
    // provider's to say, and is not kept: none is offered.
    if let Showing::Found(found) = &results.showing {
        results.pages.added = found.len().saturating_sub(1) / MOST_FOUND;
    }
    results.images = read_asked(object, "images", read_image)?;
    results.videos = read_asked(object, "videos", read_video)?;
    results.news = read_asked(object, "news", read_story)?;
    results.answer = read_answer(object, &results.showing)?;
    let tab = match object.get("tab").and_then(Value::as_str) {
        Some("all") => Tab::All,
        Some("answer") => Tab::Answer,
        Some("sites") => Tab::Sites,
        Some("images") => Tab::Images,
        Some("videos") => Tab::Videos,
        Some("news") => Tab::News,
        _ => return Err(missing("tab")),
    };
    // A tab that cannot be chosen as read, or whose results were still
    // being asked for — never asked again until it is opened — is not
    // shown: All is.
    let unasked = match tab {
        Tab::Videos => matches!(results.videos, Asked::NotAsked),
        Tab::News => matches!(results.news, Asked::NotAsked),
        // Never asked — a chat written before its answer was kept (queue
        // item 814) — its tab cannot be chosen.
        Tab::Answer => matches!(results.answer, Answer::NotAsked),
        Tab::All | Tab::Sites | Tab::Images => false,
    };
    if results.open(tab) && !unasked {
        results.tab = tab;
    }
    Ok(results)
}

/// The runs of the agent kept with a page, `"ran"` of `visited`: none in
/// a chat kept before runs were (queue item 820); at most [`MOST_RUNS`],
/// each of at most [`MOST_STEPS`] steps, as the wire reads them.
fn read_runs(visited: &Map<String, Value>) -> Result<Vec<Ran>, Unread> {
    let Some(runs) = visited.get("ran") else {
        return Ok(Vec::new());
    };
    let runs = runs.as_array().ok_or_else(|| missing("ran"))?;
    if runs.len() > MOST_RUNS {
        return Err(Unread(format!(
            "it holds {} runs of the agent on one page, more than the {MOST_RUNS} kept",
            runs.len()
        )));
    }
    runs.iter()
        .map(|ran| {
            let ran = ran.as_object().ok_or_else(|| missing("ran"))?;
            let steps = ran
                .get("steps")
                .and_then(Value::as_array)
                .ok_or_else(|| missing("steps"))?;
            if steps.len() > MOST_STEPS {
                return Err(Unread(format!(
                    "it holds a run of {} steps, more than the {MOST_STEPS} drawn",
                    steps.len()
                )));
            }
            let steps = steps
                .iter()
                .map(|step| {
                    let step = step.as_object().ok_or_else(|| missing("steps"))?;
                    Ok(Step {
                        line: text(step, "line")?,
                        why: text(step, "why")?,
                    })
                })
                .collect::<Result<Vec<_>, Unread>>()?;
            let ended = ran
                .get("ended")
                .and_then(Value::as_object)
                .filter(|ended| ended.len() == 1)
                .ok_or_else(|| missing("ended"))?;
            let ended = if let Some(said) = ended.get("done").and_then(Value::as_str) {
                Ended::Done {
                    said: said.to_owned(),
                }
            } else if let Some(why) = ended.get("stuck").and_then(Value::as_str) {
                Ended::Stuck {
                    why: why.to_owned(),
                }
            } else if let Some(why) = ended.get("stopped").and_then(Value::as_str) {
                Ended::Stopped {
                    why: why.to_owned(),
                }
            } else {
                return Err(missing("ended"));
            };
            Ok(Ran {
                asked: text(ran, "asked")?,
                plan: read_plan(ran)?,
                steps,
                ended,
            })
        })
        .collect()
}

/// A run's plan, `"plan"` of `ran` (queue item 824): none in a run kept
/// without one, and refused past what is drawn, as the wire refuses it.
fn read_plan(ran: &Map<String, Value>) -> Result<Option<Plan>, Unread> {
    let Some(plan) = ran.get("plan") else {
        return Ok(None);
    };
    let plan = plan.as_object().ok_or_else(|| missing("plan"))?;
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("plan"))?
        .iter()
        .map(|step| {
            step.as_str()
                .map(str::to_owned)
                .ok_or_else(|| missing("plan"))
        })
        .collect::<Result<Vec<_>, Unread>>()?;
    let on = plan
        .get("on")
        .and_then(Value::as_u64)
        .and_then(|on| usize::try_from(on).ok())
        .ok_or_else(|| missing("plan"))?;
    let plan = Plan { steps, on };
    if !plan.is_bounded() {
        return Err(Unread(format!(
            "it holds a plan of {} steps on its step {}, past what is drawn",
            plan.steps.len(),
            plan.on
        )));
    }
    Ok(Some(plan))
}

/// A search's answer, `"answer"` of `object`, checked against the sites
/// `showing` holds as the wire checks it: none to a search that found no
/// sites, and a written one citing only the first [`MOST_SOURCES`] of
/// them. One still being written is read as not written; none at all, a
/// chat kept before answers were (queue item 814), as never asked.
fn read_answer(object: &Map<String, Value>, showing: &Showing) -> Result<Answer, Unread> {
    let answer = match object.get("answer") {
        None => return Ok(Answer::NotAsked),
        Some(Value::String(state)) => match state.as_str() {
            "not asked" => Answer::NotAsked,
            "asking" => Answer::Unanswered(ANSWER_NOT_WRITTEN.to_owned()),
            "not found" => Answer::NotFound,
            _ => return Err(missing("answer")),
        },
        Some(Value::Object(answer)) if answer.len() == 1 => {
            if let Some(why) = answer.get("unanswered").and_then(Value::as_str) {
                Answer::Unanswered(why.to_owned())
            } else {
                let written = answer
                    .get("written")
                    .and_then(Value::as_object)
                    .ok_or_else(|| missing("answer"))?;
                Answer::Written(Written {
                    line: text(written, "line")?,
                    sentences: read_claims(written, "sentences")?,
                    points: read_claims(written, "points")?,
                })
            }
        }
        Some(_) => return Err(missing("answer")),
    };
    let given = match showing {
        Showing::Found(found) => found.len().min(MOST_SOURCES),
        Showing::Searching | Showing::Unanswered(_) => 0,
    };
    if given == 0 && answer != Answer::NotAsked {
        return Err(Unread(
            "it holds an answer to a search that found no sites".to_owned(),
        ));
    }
    if let Answer::Written(written) = &answer {
        written
            .check(given)
            .map_err(|why| Unread(format!("its answer is refused: {why}")))?;
    }
    Ok(answer)
}

/// An answer's sentences or points, `field` of `written`: each its text
/// and the numbers it cites. How many, and which numbers, are checked by
/// [`Written::check`].
fn read_claims(written: &Map<String, Value>, field: &str) -> Result<Vec<Cited>, Unread> {
    written
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| missing(field))?
        .iter()
        .map(|claim| {
            let claim = claim.as_object().ok_or_else(|| missing(field))?;
            let sources = claim
                .get("sources")
                .and_then(Value::as_array)
                .ok_or_else(|| missing("sources"))?
                .iter()
                .map(|number| {
                    number
                        .as_u64()
                        .and_then(|number| usize::try_from(number).ok())
                        .ok_or_else(|| missing("sources"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Cited::new(text(claim, "text")?, sources))
        })
        .collect()
}

/// Each thing `found` of an object holding only that, read by `each`.
fn each_of<T>(
    holding: &Map<String, Value>,
    each: fn(&Map<String, Value>) -> Result<T, Unread>,
) -> Result<Vec<T>, Unread> {
    holding
        .get("found")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("found"))?
        .iter()
        .map(|one| {
            one.as_object()
                .ok_or_else(|| missing("found"))
                .and_then(each)
        })
        .collect()
}

/// A further search, `field` of `object`, each thing found read by `each`:
/// one still asking as never asked.
fn read_asked<T>(
    object: &Map<String, Value>,
    field: &str,
    each: fn(&Map<String, Value>) -> Result<T, Unread>,
) -> Result<Asked<T>, Unread> {
    match object.get(field) {
        Some(Value::String(state)) if state == "not asked" || state == "asking" => {
            Ok(Asked::NotAsked)
        }
        Some(Value::Object(asked)) if asked.len() == 1 => {
            if let Some(why) = asked.get("unanswered").and_then(Value::as_str) {
                Ok(Asked::Unanswered(why.to_owned()))
            } else {
                each_of(asked, each).map(Asked::Found)
            }
        }
        _ => Err(missing(field)),
    }
}

/// A site found, read.
fn read_site(object: &Map<String, Value>) -> Result<Found, Unread> {
    Ok(Found::new(
        text(object, "title")?,
        address(object, "url")?,
        text(object, "description")?,
    )
    .named(text(object, "name")?))
}

/// An image found, read, with no picture.
fn read_image(object: &Map<String, Value>) -> Result<Pictured, Unread> {
    Ok(Pictured::new(
        text(object, "title")?,
        address(object, "page")?,
        text(object, "source")?,
    ))
}

/// A video found, read, with no picture; a length that is not one is
/// read as none.
fn read_video(object: &Map<String, Value>) -> Result<Video, Unread> {
    let mut video = Video::new(text(object, "title")?, address(object, "page")?);
    let length = text(object, "length")?;
    if results_media::is_length(&length) {
        video.length = length;
    }
    video.site = text(object, "site")?;
    video.age = text(object, "age")?;
    video.description = text(object, "description")?;
    Ok(video)
}

/// A news story found, read.
fn read_story(object: &Map<String, Value>) -> Result<Story, Unread> {
    let mut story = Story::new(text(object, "title")?, address(object, "page")?);
    story.site = text(object, "site")?;
    story.age = text(object, "age")?;
    story.description = text(object, "description")?;
    Ok(story)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::card::Card;

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap_or_else(|_| Url::about_blank())
    }

    /// A search with every kind of thing found, its pictures and marks
    /// held.
    fn a_search() -> Results {
        let mut site = Found::new(
            "Chargers \u{201c}near\u{201d} <b>you</b>",
            url("https://a.example/x?y=1"),
            "Fast ones.",
        )
        .named("A");
        site.mark = Some(vec![1, 2, 3]);
        let mut results = Results::of("ev chargers", Showing::Found(vec![site]));
        let mut image = Pictured::new("A plug", url("https://p.example/"), "p.example");
        image.picture = Some(vec![9; 400]);
        image.mark = Some(vec![4]);
        results.images = Asked::Found(vec![image]);
        let mut video = Video::new("How to charge", url("https://v.example/w"));
        video.length = "4:05".to_owned();
        video.site = "V".to_owned();
        video.age = "2 days ago".to_owned();
        video.description = "A video.".to_owned();
        video.picture = Some(vec![7; 100]);
        results.videos = Asked::Found(vec![video]);
        results.news = Asked::Unanswered("No news.".to_owned());
        results.tab = Tab::Videos;
        results
    }

    #[test]
    fn a_chat_read_back_is_what_was_written_but_its_pictures() {
        let mut card = Card::new("example.com", url("https://example.com/now"));
        card.frame = Some(vec![0; 1000]);
        card.loading = true;
        let turns = vec![
            Turn::Searched(a_search()),
            Turn::Visited {
                said: "Lifetimes".to_owned(),
                url: url("https://l.example/"),
                ran: Vec::new(),
            },
            Turn::Failed {
                said: "Nope".to_owned(),
                why: "https://n.example/ could not be opened: no".to_owned(),
            },
            Turn::Page(card),
        ];
        let written = write("ev chargers", &turns);
        assert!(!written.contains("\"picture\"") && !written.contains("\"mark\""));
        assert!(!written.contains("frame"), "{written}");
        let read = read(written.as_bytes()).expect("read back");
        assert_eq!(read.name, "ev chargers");
        let mut expected = a_search();
        if let Showing::Found(found) = &mut expected.showing {
            for site in found {
                site.mark = None;
            }
        }
        if let Asked::Found(images) = &mut expected.images {
            for image in images {
                image.picture = None;
                image.mark = None;
            }
        }
        if let Asked::Found(videos) = &mut expected.videos {
            for video in videos {
                video.picture = None;
            }
        }
        assert_eq!(read.turns.first(), Some(&Turn::Searched(expected)));
        assert_eq!(read.turns.get(1..3), turns.get(1..3));
        assert_eq!(
            read.turns.get(3),
            Some(&Turn::Visited {
                said: "example.com".to_owned(),
                url: url("https://example.com/now"),
                ran: Vec::new(),
            }),
            "a live card is kept as the page shown earlier it becomes"
        );
        // Written again, the same text.
        assert_eq!(
            write(&read.name, &read.turns),
            write("ev chargers", &read.turns)
        );
    }

    #[test]
    fn a_searchs_further_pages_are_read_back_as_pages_with_none_offered() {
        let sites = |count: usize| {
            Showing::Found(
                (0..count)
                    .map(|n| Found::new("S", url(&format!("https://s{n}.example/")), ""))
                    .collect(),
            )
        };
        let mut paged = Results::of("ev", sites(45)).on(Tab::Sites);
        paged.pages.added = 2;
        paged.pages.next = alo_renderer::results::Next::Offered;
        let kept = read(write("ev", &[Turn::Searched(paged)]).as_bytes()).expect("read");
        let Some(Turn::Searched(results)) = kept.turns.first() else {
            panic!("{kept:?}");
        };
        assert_eq!(results.showing, sites(45));
        assert_eq!(results.tab, Tab::Sites);
        // 45 sites fill two pages past the first; no more is offered.
        assert_eq!(results.pages.added, 2);
        assert_eq!(results.pages.next, alo_renderer::results::Next::None);
        let one = read(write("ev", &[Turn::Searched(Results::of("ev", sites(20)))]).as_bytes())
            .expect("read");
        assert!(
            matches!(one.turns.first(), Some(Turn::Searched(r)) if r.pages.added == 0),
            "{one:?}"
        );
        let too_many = write(
            "ev",
            &[Turn::Searched(Results::of("ev", sites(MOST_SITES + 1)))],
        );
        assert!(
            read(too_many.as_bytes())
                .err()
                .is_some_and(|Unread(why)| why.contains("201 sites")),
            "more than a search holds is not read"
        );
    }

    #[test]
    fn what_was_under_way_is_read_as_not_answered_or_never_asked() {
        let mut results = Results::searching("ev");
        results.images = Asked::Asking;
        let written = write("", &[Turn::Searched(results)]);
        let kept = read(written.as_bytes()).expect("read");
        let Some(Turn::Searched(results)) = kept.turns.first() else {
            panic!("{kept:?}");
        };
        assert_eq!(
            results.showing,
            Showing::Unanswered(NOT_ANSWERED.to_owned())
        );
        assert_eq!(results.images, Asked::NotAsked);
        // A tab whose results were still asked for is not shown.
        let mut asking = a_search();
        asking.videos = Asked::Asking;
        let kept = read(write("x", &[Turn::Searched(asking)]).as_bytes()).expect("read");
        assert!(
            matches!(kept.turns.first(), Some(Turn::Searched(r)) if r.tab == Tab::All && r.videos == Asked::NotAsked)
        );
    }

    /// A written answer citing the search's first and third sites.
    fn written() -> alo_renderer::results_answer::Written {
        Written {
            line: "About a day, \u{201c}quoted\u{201d}.".to_owned(),
            sentences: vec![
                Cited::new("A fitting takes a day.", [1]),
                Cited::new("A circuit adds one.", [1, 3]),
            ],
            points: vec![Cited::new("Book early.", [3])],
        }
    }

    /// A search that found `count` sites.
    fn found(count: usize) -> Results {
        Results::of(
            "ev",
            Showing::Found(
                (0..count)
                    .map(|n| Found::new("S", url(&format!("https://s{n}.example/")), ""))
                    .collect(),
            ),
        )
    }

    #[test]
    fn a_searchs_answer_is_kept_in_words_and_read_back_on_its_tab() {
        let answered = found(3)
            .with_answer(Answer::Written(written()))
            .on(Tab::Answer);
        let text = write("x", &[Turn::Searched(answered.clone())]);
        assert!(text.contains("\"line\":\"About a day"), "{text}");
        assert!(text.contains("\"sources\":[1,3]"), "{text}");
        let kept = read(text.as_bytes()).expect("read");
        assert_eq!(kept.turns, vec![Turn::Searched(answered)]);
        // Each kind of answer, round trip; the Answer tab shown with each.
        for answer in [
            Answer::NotFound,
            Answer::Unanswered("OpenAI could not answer just now (503).".to_owned()),
        ] {
            let results = found(2).with_answer(answer).on(Tab::Answer);
            let kept =
                read(write("x", &[Turn::Searched(results.clone())]).as_bytes()).expect("read");
            assert_eq!(kept.turns, vec![Turn::Searched(results)]);
        }
        let never = found(2);
        let kept = read(write("x", &[Turn::Searched(never.clone())]).as_bytes()).expect("read");
        assert_eq!(kept.turns, vec![Turn::Searched(never)]);
    }

    #[test]
    fn an_answer_being_written_is_read_as_not_written_and_never_asked_again() {
        let asking = found(2).with_answer(Answer::Asking).on(Tab::Answer);
        let kept = read(write("x", &[Turn::Searched(asking)]).as_bytes()).expect("read");
        assert!(
            matches!(
                kept.turns.first(),
                Some(Turn::Searched(r)) if r.answer == Answer::Unanswered(ANSWER_NOT_WRITTEN.to_owned())
                    && r.tab == Tab::Answer
            ),
            "{kept:?}"
        );
    }

    #[test]
    fn a_chat_kept_before_answers_were_is_read_with_none_on_all() {
        let text = write("x", &[Turn::Searched(found(2).on(Tab::Answer))])
            .replace("\"answer\":\"not asked\",", "");
        assert!(!text.contains("answer\":"), "{text}");
        let kept = read(text.as_bytes()).expect("read");
        assert!(
            matches!(
                kept.turns.first(),
                Some(Turn::Searched(r)) if r.tab == Tab::All && r.answer == Answer::NotAsked
            ),
            "{kept:?}"
        );
    }

    #[test]
    fn an_answer_is_read_back_checked_as_the_wire_checks_it() {
        let why = |results: Results| {
            read(write("x", &[Turn::Searched(results)]).as_bytes())
                .err()
                .map(|Unread(why)| why)
                .unwrap_or_default()
        };
        // Citing the third site of a search that found two.
        assert_eq!(
            why(found(2).with_answer(Answer::Written(written()))),
            "turn 1: its answer is refused: the answer cites source 3, and it was given 2"
        );
        // Past the first eight sites, a source is not one given.
        let mut ninth = written();
        ninth.points = vec![Cited::new("Far down.", [9])];
        assert!(
            why(found(20).with_answer(Answer::Written(ninth)))
                .ends_with("the answer cites source 9, and it was given 8")
        );
        // An answer to a search that found nothing.
        assert!(
            why(Results::of("ev", Showing::Found(Vec::new())).with_answer(Answer::NotFound))
                .contains("an answer to a search that found no sites")
        );
        // More sentences than are drawn.
        let mut many = written();
        many.sentences = vec![Cited::new("x", [1]); 4];
        assert!(why(found(3).with_answer(Answer::Written(many))).contains("4 sentences"));
        // A number that is not one, and an answer of no kind.
        let good = write(
            "x",
            &[Turn::Searched(
                found(3).with_answer(Answer::Written(written())),
            )],
        );
        let bad = |text: String| {
            read(text.as_bytes())
                .err()
                .map(|Unread(why)| why)
                .unwrap_or_default()
        };
        assert!(bad(good.replace("[1,3]", "[1,-3]")).contains("\u{201c}sources\u{201d}"));
        assert!(bad(good.replace("[1,3]", "[1,\"3\"]")).contains("\u{201c}sources\u{201d}"));
        let never = write("x", &[Turn::Searched(found(3))]);
        assert!(
            bad(never.replace("\"answer\":\"not asked\"", "\"answer\":\"thought\""))
                .contains("\u{201c}answer\u{201d}"),
            "{never}"
        );
    }

    /// A run done in two steps, and one stopped with none.
    fn runs() -> Vec<Ran> {
        vec![
            Ran {
                plan: Some(Plan {
                    steps: vec![
                        "Search for Kampala".to_owned(),
                        "Open the article".to_owned(),
                    ],
                    on: 1,
                }),
                asked: "Search Wikipedia for Kampala".to_owned(),
                steps: vec![
                    Step {
                        line: "Typing Kampala into Search Wikipedia".to_owned(),
                        why: String::new(),
                    },
                    Step {
                        line: "Pressing Search".to_owned(),
                        why: "it sends the search".to_owned(),
                    },
                ],
                ended: Ended::Done {
                    said: "Opened the article.".to_owned(),
                },
            },
            Ran::starting("Find its population"),
        ]
    }

    #[test]
    fn the_agents_runs_are_kept_with_their_page_live_or_shown_earlier() {
        let mut card = Card::new(
            "en.wikipedia.org",
            url("https://en.wikipedia.org/wiki/Kampala"),
        );
        card.ran = runs();
        card.agent = Some(alo_renderer::agent_work::AtWork::starting(
            "Find its population",
        ));
        let text = write("x", &[Turn::Page(card)]);
        assert!(
            text.contains("\"asked\":\"Search Wikipedia for Kampala\""),
            "{text}"
        );
        assert!(
            text.contains("\"ended\":{\"done\":\"Opened the article.\"}"),
            "{text}"
        );
        assert!(
            text.contains(
                "\"plan\":{\"on\":1,\"steps\":[\"Search for Kampala\",\"Open the article\"]}"
            ),
            "{text}"
        );
        let kept = read(text.as_bytes()).expect("read");
        assert_eq!(
            kept.turns,
            vec![Turn::Visited {
                said: "en.wikipedia.org".to_owned(),
                url: url("https://en.wikipedia.org/wiki/Kampala"),
                ran: runs(),
            }]
        );
        // Written again from what was read, the same text.
        assert_eq!(write("x", &kept.turns), text);
        // A chat kept before runs were: none.
        let before = text.replace(
            &text[text.find("\"ran\":").expect("ran")..text.find("\"said\":").expect("said")],
            "",
        );
        assert!(!before.contains("\"ran\""), "{before}");
        assert!(
            matches!(read(before.as_bytes()).expect("read").turns.first(), Some(Turn::Visited { ran, .. }) if ran.is_empty())
        );
    }

    #[test]
    fn runs_past_what_is_drawn_or_of_no_kind_are_not_read() {
        let why = |turns: &[Turn]| {
            read(write("x", turns).as_bytes())
                .err()
                .map(|Unread(why)| why)
                .unwrap_or_default()
        };
        let visited = |ran: Vec<Ran>| Turn::Visited {
            said: "s".to_owned(),
            url: url("https://s.example/"),
            ran,
        };
        assert!(
            why(&[visited(vec![Ran::starting("x"); MOST_RUNS + 1])]).contains("21 runs"),
            "more runs than are kept"
        );
        let mut planned = Ran::starting("x");
        planned.plan = Some(Plan {
            steps: vec!["a".to_owned(); 6],
            on: 0,
        });
        assert!(
            why(&[visited(vec![planned])]).contains("a plan of 6 steps"),
            "a plan past what is drawn"
        );
        let mut long = Ran::starting("x");
        long.steps = vec![
            Step {
                line: "s".to_owned(),
                why: String::new()
            };
            MOST_STEPS + 1
        ];
        assert!(why(&[visited(vec![long])]).contains("41 steps"));
        let good = write("x", &[visited(runs())]);
        let bad = |text: String| {
            read(text.as_bytes())
                .err()
                .map(|Unread(why)| why)
                .unwrap_or_default()
        };
        assert!(bad(good.replace("{\"done\":", "{\"thought\":")).contains("\u{201c}ended\u{201d}"));
        assert!(bad(good.replace("\"why\":\"\"", "\"why\":1")).contains("\u{201c}why\u{201d}"));
    }

    #[test]
    fn a_file_that_is_not_a_chat_is_not_read_at_all_and_says_why() {
        let why = |bytes: &[u8]| read(bytes).err().map(|Unread(why)| why).unwrap_or_default();
        assert!(why(b"").contains("not JSON"));
        assert!(
            why(b"{\"format\":\"alo chat\",\"version\":1,\"name\":\"a\",\"tu").contains("not JSON")
        );
        assert!(why(b"[]").contains("not an object"));
        assert!(why(b"{\"version\":1}").contains("does not say"));
        assert!(
            why(b"{\"format\":\"alo chat\",\"version\":2,\"name\":\"\",\"turns\":[]}")
                .contains("version 2")
        );
        assert!(
            why(b"{\"format\":\"alo chat\",\"version\":1,\"turns\":[]}")
                .contains("\u{201c}name\u{201d}")
        );
        let good = write(
            "a",
            &[Turn::Visited {
                said: "a".to_owned(),
                url: url("https://a.example/"),
                ran: Vec::new(),
            }],
        );
        assert!(read(good.as_bytes()).is_ok());
        let not_a_site = good.replace("https://a.example/", "file:///etc/passwd");
        assert!(why(not_a_site.as_bytes()).contains("turn 1: \u{201c}url\u{201d} is not a site"));
        let unknown = good.replace("\"visited\"", "\"thought\"");
        assert!(why(unknown.as_bytes()).contains("turn 1: it is no kind"));
        let too_many = write(
            "",
            &vec![
                Turn::Failed {
                    said: String::new(),
                    why: String::new(),
                };
                MOST_TURNS + 1
            ],
        );
        assert!(why(too_many.as_bytes()).contains("more than the 500"));
        assert!(
            read(
                write(
                    "",
                    &vec![
                        Turn::Failed {
                            said: String::new(),
                            why: String::new()
                        };
                        MOST_TURNS
                    ]
                )
                .as_bytes()
            )
            .is_ok()
        );
    }
}

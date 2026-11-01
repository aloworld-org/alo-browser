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
//!   becomes when anything newer comes (ADR 0085 § 2).
//! - **A page that could not be opened** — what was said, and why not.
//!
//! **Not written yet:** a search's answer (queue item 707) — a chat read
//! back shows its sites with no answer, and the Answer tab is not open; a
//! chat kept on it is read back on All (queue item 814).
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
//! A search's sites past its first page are the further pages a person
//! asked for (queue item 761), counted again from how many there are; no
//! further page is offered for a chat read back, since whether there is
//! one is the provider's to say, and more than [`MOST_SITES`] are not read.
//! Nothing read asks anything of the network.

use alo_renderer::results::{
    Asked, Found, MOST_FOUND, MOST_SITES, Pictured, Results, Showing, Tab,
};
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
        Turn::Page(card) => object([(
            "visited",
            object([
                ("said", Value::from(card.said.as_str())),
                ("url", Value::from(card.url.to_string())),
            ]),
        )]),
        Turn::Visited { said, url } => object([(
            "visited",
            object([
                ("said", Value::from(said.as_str())),
                ("url", Value::from(url.to_string())),
            ]),
        )]),
        Turn::Failed { said, why } => object([(
            "failed",
            object([
                ("said", Value::from(said.as_str())),
                ("why", Value::from(why.as_str())),
            ]),
        )]),
    }
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
    ])
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
        // A search's answer is not kept (queue item 814): its tab is not
        // shown, and cannot be chosen.
        Tab::Answer => true,
        Tab::All | Tab::Sites | Tab::Images => false,
    };
    if results.open(tab) && !unasked {
        results.tab = tab;
    }
    Ok(results)
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

    #[test]
    fn a_searchs_answer_is_not_kept_and_its_tab_is_read_back_as_all() {
        use alo_renderer::results_answer::{Answer, Cited, Written};
        let answered = a_search()
            .with_answer(Answer::Written(Written {
                line: "A secret only the model wrote.".to_owned(),
                sentences: vec![Cited::new("A secret only the model wrote.", [1])],
                points: Vec::new(),
            }))
            .on(Tab::Answer);
        let written = write("x", &[Turn::Searched(answered)]);
        assert!(!written.contains("secret"), "{written}");
        assert!(written.contains("\"tab\":\"answer\""), "{written}");
        let kept = read(written.as_bytes()).expect("read");
        assert!(
            matches!(
                kept.turns.first(),
                Some(Turn::Searched(r)) if r.tab == Tab::All && r.answer == Answer::NotAsked
            ),
            "{kept:?}"
        );
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

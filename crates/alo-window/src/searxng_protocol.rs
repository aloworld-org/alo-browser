/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `SearXNG`'s answer to a search in its JSON format, read (ADR 0106 § 2,
//! queue item 904).
//!
//! A protocol read from a stranger — the instance is whoever's address a
//! person gave — and so a file `serde_json` may be named in
//! (`scripts/gate.sh`'s boundary list), a reason to change of its own.
//!
//! # What is read
//!
//! Of the document `SearXNG` writes for `format=json` (`searx/webutils.py`,
//! `get_json_response`: `query`, `results`, `answers`, `corrections`,
//! `infoboxes`, `suggestions`, `unresponsive_engines`), only `results`:
//!
//! ```json
//! { "query": "…",
//!   "results": [ { "url": "https://…", "title": "…", "content": "…",
//!                  "publishedDate": "2026-11-08T09:30:00" } ] }
//! ```
//!
//! In this format a result's `title` and `content` are plain text — the
//! HTML escaping and highlighting are its HTML template's alone — so they
//! are read as written: control characters dropped, runs of white space
//! made one space, trimmed. A web result's site is not named (each is drawn
//! with its address's host), and a story's site is its address's host, its
//! age the date `publishedDate` begins with. `img_src`, `thumbnail_src` and
//! `thumbnail` are **never read**: they are the site's own addresses, and
//! alo fetches no picture from a site (ADR 0086, ADR 0088).
//!
//! # What is refused, by name
//!
//! Before parsing: more than [`LARGEST`] bytes, or nesting deeper than
//! [`MOST_NESTING`] ([`crate::json_depth`]). Then the whole answer, as an
//! [`Unread`]: not JSON, not an object, `results` missing or not a list. A
//! result that is not an object, has no `title` or no `url` as text, or a
//! `url` that is not an `http` or `https` site's, is left out and counted;
//! the rest are kept, at most [`MOST_FOUND`] or [`MOST_STORIES`]. Nothing
//! here panics on any input.

use crate::json_depth;
use crate::search_service::StoryFound;
use alo_renderer::results::{Found, MOST_FOUND};
use alo_renderer::results_media::{MOST_STORIES, Story};
use alo_url::Url;
use core::fmt;
use serde_json::{Map, Value};

/// The largest answer read: a page of a `SearXNG`'s merged results is tens
/// of kilobytes.
pub const LARGEST: usize = 4 * 1024 * 1024;

/// The deepest an answer may nest before it is refused unparsed.
pub const MOST_NESTING: usize = 32;

/// What was read of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read<T> {
    /// What was found, best first.
    pub found: Vec<T>,
    /// How many results were left out.
    pub left_out: usize,
}

/// Why a whole answer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// Larger than [`LARGEST`].
    TooLarge {
        /// Its size in bytes.
        bytes: usize,
    },
    /// Nested deeper than [`MOST_NESTING`].
    TooDeep {
        /// How deep.
        depth: usize,
    },
    /// Not JSON, as the parser said.
    NotJson(String),
    /// JSON, and not an object.
    NotAnObject,
    /// An object with no `results`.
    NoResults,
    /// Its `results` is not a list.
    ResultsNotAList,
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unread::TooLarge { bytes } => {
                write!(f, "it is {bytes} bytes, and at most {LARGEST} are read")
            }
            Unread::TooDeep { depth } => write!(
                f,
                "it nests {depth} deep, and at most {MOST_NESTING} is read"
            ),
            Unread::NotJson(why) => write!(f, "it is not JSON ({why})"),
            Unread::NotAnObject => f.write_str("it is not a JSON object"),
            Unread::NoResults => f.write_str("it holds no list of results"),
            Unread::ResultsNotAList => f.write_str("its results are not a list"),
        }
    }
}

/// Read `answer`, a `SearXNG`'s answer to a web search: its sites.
///
/// # Errors
///
/// [`Unread`], naming what the whole answer lacked.
pub fn read(answer: &[u8]) -> Result<Read<Found>, Unread> {
    read_each(answer, MOST_FOUND, |result, url, title| {
        Found::new(title, url, text_of(result, "content"))
    })
}

/// Read `answer`, a `SearXNG`'s answer to a search of its `news` category:
/// its stories, none with a photo to fetch.
///
/// # Errors
///
/// [`Unread`], naming what the whole answer lacked.
pub fn read_news(answer: &[u8]) -> Result<Read<StoryFound>, Unread> {
    read_each(answer, MOST_STORIES, |result, url, title| {
        let site = host_of(&url);
        let age = published(result);
        StoryFound {
            story: Story::new(title, url)
                .from_site(site, age)
                .said_of(text_of(result, "content")),
            thumbnail: None,
            mark: None,
        }
    })
}

/// Each result of `answer` that is a site's, made by `made` from the
/// result, its address and its title — at most `most`.
fn read_each<T>(
    answer: &[u8],
    most: usize,
    made: impl Fn(&Map<String, Value>, Url, String) -> T,
) -> Result<Read<T>, Unread> {
    let value = parsed(answer)?;
    let document = value.as_object().ok_or(Unread::NotAnObject)?;
    let results = document
        .get("results")
        .ok_or(Unread::NoResults)?
        .as_array()
        .ok_or(Unread::ResultsNotAList)?;
    let mut read = Read {
        found: Vec::new(),
        left_out: 0,
    };
    for result in results {
        if read.found.len() >= most {
            break;
        }
        let kept = result.as_object().and_then(|result| {
            let title = Some(text_of(result, "title")).filter(|title| !title.is_empty())?;
            let url = result
                .get("url")
                .and_then(Value::as_str)
                .and_then(site_address)?;
            Some(made(result, url, title))
        });
        match kept {
            Some(found) => read.found.push(found),
            None => read.left_out += 1,
        }
    }
    Ok(read)
}

/// `answer` parsed as JSON, once it is known to be small and shallow
/// enough to parse.
fn parsed(answer: &[u8]) -> Result<Value, Unread> {
    if answer.len() > LARGEST {
        return Err(Unread::TooLarge {
            bytes: answer.len(),
        });
    }
    let depth = json_depth::of(answer);
    if depth > MOST_NESTING {
        return Err(Unread::TooDeep { depth });
    }
    serde_json::from_slice::<Value>(answer).map_err(|why| Unread::NotJson(why.to_string()))
}

/// The text at `key` of `result`, read as plain text: control characters
/// dropped, white space made one space, trimmed; none if it is not text.
fn text_of(result: &Map<String, Value>, key: &str) -> String {
    let Some(written) = result.get(key).and_then(Value::as_str) else {
        return String::new();
    };
    written
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `written`, if it is an `http` or `https` address with a host.
fn site_address(written: &str) -> Option<Url> {
    alo_url::parse(written)
        .ok()
        .filter(|url| matches!(url.scheme.as_str(), "http" | "https") && url.host.is_some())
}

/// The host of `url`, without a leading `www.`.
fn host_of(url: &Url) -> String {
    let host = url
        .host
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    host.strip_prefix("www.").map(str::to_owned).unwrap_or(host)
}

/// The day a result's `publishedDate` begins with, `YYYY-MM-DD`, if it is
/// one; otherwise none.
fn published(result: &Map<String, Value>) -> String {
    let Some(date) = result.get("publishedDate").and_then(Value::as_str) else {
        return String::new();
    };
    let day: String = date.chars().take(10).collect();
    let shaped = day.len() == 10
        && day.char_indices().all(|(at, c)| match at {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    if shaped { day } else { String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An answer in `SearXNG`'s shape for `format=json`, written by hand after
    /// its `get_json_response`: never a real one.
    const ANSWER: &str = r#"{
      "query": "ev charger",
      "number_of_results": 0,
      "results": [
        { "url": "https://www.example.org/install", "title": "  Installing an\n EV charger ",
          "content": "Half a day, &amp; a permit\u0007 in most places.",
          "engine": "duckduckgo", "engines": ["duckduckgo", "wikipedia"],
          "parsed_url": ["https", "www.example.org", "/install", "", "", ""],
          "img_src": "https://www.example.org/picture.jpg",
          "thumbnail": "https://www.example.org/thumb.jpg",
          "template": "default.html", "positions": [1, 2], "score": 3.5, "category": "general" },
        { "url": "ftp://files.example.org/", "title": "Not a site's" },
        { "url": "https://no-title.example.org/" },
        { "title": "No address" },
        "a string",
        { "url": "https://second.example.org/", "title": "Second", "content": 42 }
      ],
      "answers": [], "corrections": [], "infoboxes": [], "suggestions": ["ev charger cost"],
      "unresponsive_engines": [["bing", "timeout"]]
    }"#;

    #[test]
    fn a_web_answer_reads_its_sites_as_plain_text_and_leaves_out_the_rest() {
        let read = read(ANSWER.as_bytes()).unwrap();
        assert_eq!(read.left_out, 4);
        let titles: Vec<&str> = read.found.iter().map(|site| site.title.as_str()).collect();
        assert_eq!(titles, ["Installing an EV charger", "Second"]);
        let first = &read.found[0];
        assert_eq!(first.url.serialised, "https://www.example.org/install");
        assert_eq!(
            first.description, "Half a day, &amp; a permit in most places.",
            "plain text, as written, its control character dropped"
        );
        assert_eq!(first.name, "", "the site is not named");
        assert!(first.mark.is_none());
        assert_eq!(
            read.found[1].description, "",
            "content that is not text is none"
        );
    }

    #[test]
    fn a_news_answer_reads_its_stories_with_their_day_and_site_and_no_photo() {
        let read = read_news(
            br#"{ "results": [
                { "url": "https://www.news.example/a", "title": "A", "content": "Said.",
                  "publishedDate": "2026-11-08T09:30:00", "thumbnail": "https://www.news.example/p.jpg" },
                { "url": "https://news.example/b", "title": "B", "publishedDate": "yesterday" },
                { "url": "https://news.example/c", "title": "C", "publishedDate": null }
            ] }"#,
        )
        .unwrap();
        let stories: Vec<(&str, &str, &str)> = read
            .found
            .iter()
            .map(|found| {
                (
                    found.story.title.as_str(),
                    found.story.site.as_str(),
                    found.story.age.as_str(),
                )
            })
            .collect();
        assert_eq!(
            stories,
            [
                ("A", "news.example", "2026-11-08"),
                ("B", "news.example", ""),
                ("C", "news.example", "")
            ]
        );
        assert!(
            read.found
                .iter()
                .all(|found| found.thumbnail.is_none() && found.mark.is_none()),
            "a picture's address is the site's, and is never read"
        );
    }

    #[test]
    fn a_whole_answer_that_is_not_one_is_refused_by_name_and_nothing_panics() {
        assert_eq!(
            read(b"<html>").err().map(|why| why.to_string()),
            Some("it is not JSON (expected value at line 1 column 1)".to_owned())
        );
        assert_eq!(read(b"[]").err(), Some(Unread::NotAnObject));
        assert_eq!(read(b"{}").err(), Some(Unread::NoResults));
        assert_eq!(
            read(br#"{"results":{}}"#).err(),
            Some(Unread::ResultsNotAList)
        );
        let deep = format!("{}{}", "[".repeat(40), "]".repeat(40));
        assert!(matches!(read(deep.as_bytes()), Err(Unread::TooDeep { .. })));
        let large = vec![b' '; LARGEST + 1];
        assert_eq!(
            read(&large).err(),
            Some(Unread::TooLarge { bytes: LARGEST + 1 })
        );
        // Truncated, hostile and odd bytes: an error or a reading, never a
        // panic.
        for at in 0..ANSWER.len() {
            let _ = read(ANSWER.as_bytes().get(..at).unwrap_or_default());
            let _ = read_news(ANSWER.as_bytes().get(..at).unwrap_or_default());
        }
        for odd in [
            &b"\xff\xfe"[..],
            br#"{"results":[{"url":"https://a.example/","title":"\ud800"}]}"#,
            br#"{"results":[{"url":"https://a.example/","title":"x","publishedDate":"2026-1\u00e9-08"}]}"#,
        ] {
            let _ = read(odd);
            let _ = read_news(odd);
        }
        // At most twenty sites.
        let many: Vec<String> = (0..30)
            .map(|n| format!(r#"{{"url":"https://s{n}.example/","title":"S{n}"}}"#))
            .collect();
        let many = format!(r#"{{"results":[{}]}}"#, many.join(","));
        assert_eq!(
            read(many.as_bytes()).map(|read| read.found.len()),
            Ok(MOST_FOUND)
        );
    }
}

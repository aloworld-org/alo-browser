/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The Brave Search API's answer to a web search, read (ADR 0083 § 4, queue
//! item 708).
//!
//! A third protocol read from a stranger, beside the agent's lines
//! ([`crate::protocol`]) and Chrome's messages ([`crate::chrome_protocol`]),
//! and so the third file `serde_json` may be named in: each is a reason to
//! change of its own.
//!
//! # What is read
//!
//! Of Brave's `WebSearchApiResponse`, only what the All tab draws:
//!
//! ```json
//! { "type": "search",
//!   "web": { "type": "search",
//!            "results": [ { "title": "…", "url": "https://…",
//!                           "description": "…",
//!                           "profile": { "name": "…" } } ] } }
//! ```
//!
//! Everything else Brave sends is left unread. A document with no `web` is
//! one with no web results, which Brave leaves out rather than sending
//! empty.
//!
//! # What is refused, by name
//!
//! Before parsing: more than [`LARGEST`] bytes, or nesting deeper than
//! [`MOST_NESTING`] ([`crate::json_depth`]). Then the whole answer, as an
//! [`Unread`]: not JSON, not an object, a `type` other than `search`, a
//! `web` that is not an object, `web.results` missing or not a list. A
//! result that is not an object, or has no `title` or no `url` as text, or
//! a `url` that is not an `http` or `https` address, is **left out** and
//! named ([`LeftOut`]); the rest are kept. A `description` or a profile's
//! `name` that is missing or not text is read as none. Nothing here panics
//! on any input.
//!
//! Titles and descriptions are read as plain text ([`crate::search_text`]).
//!
//! # An image search's answer (queue item 743, ADR 0086)
//!
//! Of Brave's `ImageSearchApiResponse`, only what the row and the grid draw:
//!
//! ```json
//! { "type": "images",
//!   "results": [ { "title": "…", "url": "https://… (the page)",
//!                  "source": "site.example",
//!                  "thumbnail": { "src": "https://imgs.search.brave.com/…" } } ] }
//! ```
//!
//! Bounded as a web search's is, and refused whole by the same names — a
//! `type` other than `images`, `results` missing or not a list. An image
//! with no `title` or no `url` as text, or a `url` that is not a site's, is
//! left out and named; a `thumbnail.src` that is missing, not text or not
//! an `http` or `https` address leaves the image with no thumbnail to
//! fetch; a `source` that is not text is none. At most [`MOST_IMAGES`] are
//! kept. The original picture's own address (`properties.url`) is never
//! read: alo does not fetch from the site that holds it.
//!
//! # A video search's answer (queue item 745, ADR 0087)
//!
//! Of Brave's `VideoSearchApiResponse`, only what the Videos tab draws:
//!
//! ```json
//! { "type": "videos",
//!   "results": [ { "title": "…", "url": "https://… (the page)",
//!                  "description": "…", "age": "…",
//!                  "video": { "duration": "04:05", "publisher": "…", "creator": "…" },
//!                  "meta_url": { "hostname": "…" },
//!                  "thumbnail": { "src": "https://imgs.search.brave.com/…" } } ] }
//! ```
//!
//! Refused whole as an image search's answer is, its `type` `videos`
//! ([`Unread::NotVideos`]). A video with no `title` or `url` as text, or a
//! `url` not a site's, is left out and named; a `duration` that is not a
//! length ([`alo_renderer::results_media::is_length`]) is none; the site is
//! the `publisher`, else the `meta_url`'s `hostname`, else none; a
//! `thumbnail.src` not a site's address leaves it with no picture to fetch.
//! At most [`MOST_VIDEOS`] are kept.
//!
//! # A news search's answer (queue item 746, ADR 0087)
//!
//! Of Brave's `NewsSearchApiResponse`, only what the News tab draws:
//!
//! ```json
//! { "type": "news",
//!   "results": [ { "title": "…", "url": "https://…", "description": "…",
//!                  "age": "2 days ago", "profile": { "name": "…" },
//!                  "meta_url": { "hostname": "…" } } ] }
//! ```
//!
//! Refused whole by the same names, its `type` `news` ([`Unread::NotNews`]);
//! a story with no `title` or `url`, or a `url` not a site's, is left out
//! and named; the site is the `profile`'s `name`, else the `meta_url`'s
//! `hostname`. A story's `thumbnail` is not read (queue item 765). At most
//! [`MOST_STORIES`] are kept.

use crate::json_depth;
use crate::search_text::plain;
use alo_renderer::results::{Found, MOST_FOUND, MOST_IMAGES, Pictured};
use alo_renderer::results_media::{MOST_STORIES, MOST_VIDEOS, Story, Video};
use alo_url::Url;
use core::fmt;
use serde_json::{Map, Value};

/// The largest answer read: Brave's twenty results with every extra it
/// can add are a few hundred kilobytes.
pub const LARGEST: usize = 4 * 1024 * 1024;

/// The deepest an answer may nest before it is refused unparsed.
pub const MOST_NESTING: usize = 32;

/// What was read of an answer: the sites found, best first, and every
/// result left out, with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// The sites, at most [`MOST_FOUND`].
    pub found: Vec<Found>,
    /// The results left out.
    pub left_out: Vec<LeftOut>,
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
    /// An object whose `type` is not `search`, or is missing.
    NotASearch {
        /// What its `type` was, if it was text.
        said: Option<String>,
    },
    /// An image search's answer whose `type` is not `images`, or is
    /// missing.
    NotImages {
        /// What its `type` was, if it was text.
        said: Option<String>,
    },
    /// A video search's answer whose `type` is not `videos`, or is
    /// missing.
    NotVideos {
        /// What its `type` was, if it was text.
        said: Option<String>,
    },
    /// A news search's answer whose `type` is not `news`, or is missing.
    NotNews {
        /// What its `type` was, if it was text.
        said: Option<String>,
    },
    /// Its `web` is not an object.
    WebNotAnObject,
    /// Its `web` has no `results`.
    NoResults,
    /// Its `web.results` is not a list.
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
            Unread::NotASearch { said: Some(said) } => {
                write!(f, "its type is {said:?}, not \"search\"")
            }
            Unread::NotASearch { said: None } => f.write_str("it does not say it is a search"),
            Unread::NotImages { said: Some(said) } => {
                write!(f, "its type is {said:?}, not \"images\"")
            }
            Unread::NotImages { said: None } => {
                f.write_str("it does not say it is an image search")
            }
            Unread::NotVideos { said: Some(said) } => {
                write!(f, "its type is {said:?}, not \"videos\"")
            }
            Unread::NotVideos { said: None } => f.write_str("it does not say it is a video search"),
            Unread::NotNews { said: Some(said) } => {
                write!(f, "its type is {said:?}, not \"news\"")
            }
            Unread::NotNews { said: None } => f.write_str("it does not say it is a news search"),
            Unread::WebNotAnObject => f.write_str("its web results are not an object"),
            Unread::NoResults => f.write_str("its web results hold no list of results"),
            Unread::ResultsNotAList => f.write_str("its web results are not a list"),
        }
    }
}

/// Why one result was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOut {
    /// Its place among the results, counting from zero.
    pub at: usize,
    /// Why.
    pub why: Missing,
}

/// What a result left out lacked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// It is not an object.
    NotAnObject,
    /// It has no `title` that is text.
    Title,
    /// It has no `url` that is text.
    Url,
    /// Its `url` is not an `http` or `https` address.
    NotASite,
}

impl fmt::Display for LeftOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = self.at;
        match self.why {
            Missing::NotAnObject => write!(f, "the result at {at} is not an object"),
            Missing::Title => write!(f, "the result at {at} has no title"),
            Missing::Url => write!(f, "the result at {at} has no url"),
            Missing::NotASite => write!(f, "the result at {at} is not at a site's address"),
        }
    }
}

/// What was read of an image search's answer: the images found, best
/// first, and every one left out, with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadImages {
    /// The images, at most [`MOST_IMAGES`], each with the address of its
    /// thumbnail if Brave gave one.
    pub found: Vec<ImageFound>,
    /// The images left out.
    pub left_out: Vec<LeftOut>,
}

/// One image an image search found, as read: what is drawn of it, and
/// where Brave says its thumbnail is — which the browser process alone
/// fetches, and never sends to a renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFound {
    /// The image, with no picture yet.
    pub pictured: Pictured,
    /// Its thumbnail's address, if Brave gave one that is a site's.
    pub thumbnail: Option<Url>,
}

/// Read `answer`, the body of Brave's answer to an image search (queue item
/// 743).
///
/// # Errors
///
/// [`Unread`], naming what the whole answer lacked.
pub fn read_images(answer: &[u8]) -> Result<ReadImages, Unread> {
    let value = parsed(answer)?;
    let document = value.as_object().ok_or(Unread::NotAnObject)?;
    match document.get("type").and_then(Value::as_str) {
        Some("images") => {}
        said => {
            return Err(Unread::NotImages {
                said: said.map(str::to_owned),
            });
        }
    }
    let results = document
        .get("results")
        .ok_or(Unread::NoResults)?
        .as_array()
        .ok_or(Unread::ResultsNotAList)?;
    let mut read = ReadImages {
        found: Vec::new(),
        left_out: Vec::new(),
    };
    for (at, result) in results.iter().enumerate() {
        if read.found.len() >= MOST_IMAGES {
            break;
        }
        match image(result) {
            Ok(found) => read.found.push(found),
            Err(why) => read.left_out.push(LeftOut { at, why }),
        }
    }
    Ok(read)
}

/// What was read of a video search's answer: the videos found, best
/// first, and every one left out, with why (queue item 745).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadVideos {
    /// The videos, at most [`MOST_VIDEOS`], each with the address of its
    /// thumbnail if Brave gave one.
    pub found: Vec<VideoFound>,
    /// The videos left out.
    pub left_out: Vec<LeftOut>,
}

/// One video a video search found, as read, and where Brave says its
/// thumbnail is — which the browser process alone fetches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFound {
    /// The video, with no picture yet.
    pub video: Video,
    /// Its thumbnail's address, if Brave gave one that is a site's.
    pub thumbnail: Option<Url>,
}

/// What was read of a news search's answer: the stories found, best first,
/// and every one left out, with why (queue item 746).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadNews {
    /// The stories, at most [`MOST_STORIES`].
    pub found: Vec<Story>,
    /// The stories left out.
    pub left_out: Vec<LeftOut>,
}

/// The results of an answer of `kind`, once it is known to be one: refused
/// whole by `refused` when its `type` is not `kind`, by name when it holds
/// no list of results.
fn results_of<'a>(
    document: &'a Map<String, Value>,
    kind: &str,
    refused: fn(Option<String>) -> Unread,
) -> Result<&'a Vec<Value>, Unread> {
    match document.get("type").and_then(Value::as_str) {
        Some(said) if said == kind => {}
        said => return Err(refused(said.map(str::to_owned))),
    }
    document
        .get("results")
        .ok_or(Unread::NoResults)?
        .as_array()
        .ok_or(Unread::ResultsNotAList)
}

/// Read `answer`, the body of Brave's answer to a video search (queue item
/// 745).
///
/// # Errors
///
/// [`Unread`], naming what the whole answer lacked.
pub fn read_videos(answer: &[u8]) -> Result<ReadVideos, Unread> {
    let value = parsed(answer)?;
    let document = value.as_object().ok_or(Unread::NotAnObject)?;
    let results = results_of(document, "videos", |said| Unread::NotVideos { said })?;
    let mut read = ReadVideos {
        found: Vec::new(),
        left_out: Vec::new(),
    };
    for (at, result) in results.iter().enumerate() {
        if read.found.len() >= MOST_VIDEOS {
            break;
        }
        match video(result) {
            Ok(found) => read.found.push(found),
            Err(why) => read.left_out.push(LeftOut { at, why }),
        }
    }
    Ok(read)
}

/// One video result, as a video found.
fn video(result: &Value) -> Result<VideoFound, Missing> {
    let result = result.as_object().ok_or(Missing::NotAnObject)?;
    let title = text(result, "title").ok_or(Missing::Title)?;
    let page = text(result, "url").ok_or(Missing::Url)?;
    let page = site_address(page).ok_or(Missing::NotASite)?;
    let about = result.get("video").and_then(Value::as_object);
    let length = about
        .and_then(|about| text(about, "duration"))
        .unwrap_or_default();
    let site = about
        .and_then(|about| text(about, "publisher"))
        .filter(|publisher| !publisher.trim().is_empty())
        .or_else(|| hostname(result))
        .map(plain)
        .unwrap_or_default();
    let age = text(result, "age").map(plain).unwrap_or_default();
    let description = text(result, "description").map(plain).unwrap_or_default();
    let thumbnail = result
        .get("thumbnail")
        .and_then(Value::as_object)
        .and_then(|thumbnail| text(thumbnail, "src"))
        .and_then(site_address);
    Ok(VideoFound {
        video: Video::new(plain(title), page)
            .lasting(length)
            .from_site(site, age)
            .said_of(description),
        thumbnail,
    })
}

/// Read `answer`, the body of Brave's answer to a news search (queue item
/// 746).
///
/// # Errors
///
/// [`Unread`], naming what the whole answer lacked.
pub fn read_news(answer: &[u8]) -> Result<ReadNews, Unread> {
    let value = parsed(answer)?;
    let document = value.as_object().ok_or(Unread::NotAnObject)?;
    let results = results_of(document, "news", |said| Unread::NotNews { said })?;
    let mut read = ReadNews {
        found: Vec::new(),
        left_out: Vec::new(),
    };
    for (at, result) in results.iter().enumerate() {
        if read.found.len() >= MOST_STORIES {
            break;
        }
        match story(result) {
            Ok(found) => read.found.push(found),
            Err(why) => read.left_out.push(LeftOut { at, why }),
        }
    }
    Ok(read)
}

/// One news result, as a story found.
fn story(result: &Value) -> Result<Story, Missing> {
    let result = result.as_object().ok_or(Missing::NotAnObject)?;
    let title = text(result, "title").ok_or(Missing::Title)?;
    let page = text(result, "url").ok_or(Missing::Url)?;
    let page = site_address(page).ok_or(Missing::NotASite)?;
    let site = result
        .get("profile")
        .and_then(Value::as_object)
        .and_then(|profile| text(profile, "name"))
        .filter(|name| !name.trim().is_empty())
        .or_else(|| hostname(result))
        .map(plain)
        .unwrap_or_default();
    let age = text(result, "age").map(plain).unwrap_or_default();
    let description = text(result, "description").map(plain).unwrap_or_default();
    Ok(Story::new(plain(title), page)
        .from_site(site, age)
        .said_of(description))
}

/// The `hostname` of a result's `meta_url`, if it is text.
fn hostname(result: &Map<String, Value>) -> Option<&str> {
    result
        .get("meta_url")
        .and_then(Value::as_object)
        .and_then(|meta| text(meta, "hostname"))
}

/// One image result, as an image found.
fn image(result: &Value) -> Result<ImageFound, Missing> {
    let result = result.as_object().ok_or(Missing::NotAnObject)?;
    let title = text(result, "title").ok_or(Missing::Title)?;
    let page = text(result, "url").ok_or(Missing::Url)?;
    let page = site_address(page).ok_or(Missing::NotASite)?;
    let source = text(result, "source").map(plain).unwrap_or_default();
    let thumbnail = result
        .get("thumbnail")
        .and_then(Value::as_object)
        .and_then(|thumbnail| text(thumbnail, "src"))
        .and_then(site_address);
    Ok(ImageFound {
        pictured: Pictured::new(plain(title), page, source),
        thumbnail,
    })
}

/// `written`, if it is an `http` or `https` address with a host.
fn site_address(written: &str) -> Option<Url> {
    alo_url::parse(written)
        .ok()
        .filter(|url| matches!(url.scheme.as_str(), "http" | "https") && url.host.is_some())
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

/// Read `answer`, the body of Brave's answer to a web search.
///
/// # Errors
///
/// [`Unread`], naming what the whole answer lacked.
pub fn read(answer: &[u8]) -> Result<Read, Unread> {
    let value = parsed(answer)?;
    let document = value.as_object().ok_or(Unread::NotAnObject)?;
    match document.get("type").and_then(Value::as_str) {
        Some("search") => {}
        said => {
            return Err(Unread::NotASearch {
                said: said.map(str::to_owned),
            });
        }
    }
    let Some(web) = document.get("web") else {
        return Ok(Read {
            found: Vec::new(),
            left_out: Vec::new(),
        });
    };
    let web = web.as_object().ok_or(Unread::WebNotAnObject)?;
    let results = web
        .get("results")
        .ok_or(Unread::NoResults)?
        .as_array()
        .ok_or(Unread::ResultsNotAList)?;
    let mut read = Read {
        found: Vec::new(),
        left_out: Vec::new(),
    };
    for (at, result) in results.iter().enumerate() {
        if read.found.len() >= MOST_FOUND {
            break;
        }
        match site(result) {
            Ok(found) => read.found.push(found),
            Err(why) => read.left_out.push(LeftOut { at, why }),
        }
    }
    Ok(read)
}

/// One result, as a site found.
fn site(result: &Value) -> Result<Found, Missing> {
    let result = result.as_object().ok_or(Missing::NotAnObject)?;
    let title = text(result, "title").ok_or(Missing::Title)?;
    let written = text(result, "url").ok_or(Missing::Url)?;
    let url = site_address(written).ok_or(Missing::NotASite)?;
    let description = text(result, "description").map(plain).unwrap_or_default();
    let name = result
        .get("profile")
        .and_then(Value::as_object)
        .and_then(|profile| text(profile, "name"))
        .map(plain)
        .unwrap_or_default();
    Ok(Found::new(plain(title), url, description).named(name))
}

/// The text at `key` of `object`, if it is text.
fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An answer in Brave's shape, written by hand: never a real one.
    const ANSWER: &str = r#"{
      "type": "search",
      "query": { "original": "rust borrow checker", "more_results_available": true },
      "web": {
        "type": "search",
        "results": [
          { "title": "The <strong>Borrow Checker</strong> &amp; you",
            "url": "https://doc.example.org/book/ch04-02-references.html",
            "description": "How <strong>references</strong> work: it&#x27;s checked at compile time.",
            "profile": { "name": "Example Docs", "url": "https://doc.example.org" },
            "meta_url": { "hostname": "doc.example.org", "path": "› book" } },
          { "title": "No description", "url": "http://plain.example/" },
          { "title": "Not a site", "url": "javascript:alert(1)" },
          { "url": "https://untitled.example/" },
          { "title": "No address" },
          "a string",
          { "title": 7, "url": "https://seven.example/" },
          { "title": "Bad profile", "url": "https://p.example/", "profile": "x", "description": 3 }
        ]
      }
    }"#;

    #[test]
    fn a_brave_answer_is_read_as_sites_with_plain_text() {
        let read = read(ANSWER.as_bytes()).unwrap();
        let titles: Vec<&str> = read.found.iter().map(|f| f.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["The Borrow Checker & you", "No description", "Bad profile"]
        );
        let first = &read.found[0];
        assert_eq!(first.name, "Example Docs");
        assert_eq!(
            first.description,
            "How references work: it's checked at compile time."
        );
        assert_eq!(
            first.url.to_string(),
            "https://doc.example.org/book/ch04-02-references.html"
        );
        assert_eq!(read.found[1].description, "");
        assert_eq!(read.found[1].name, "");
        assert_eq!(
            read.found[2].description, "",
            "a description not text is none"
        );
        let left: Vec<String> = read.left_out.iter().map(ToString::to_string).collect();
        assert_eq!(
            left,
            vec![
                "the result at 2 is not at a site's address",
                "the result at 3 has no title",
                "the result at 4 has no url",
                "the result at 5 is not an object",
                "the result at 6 has no title",
            ]
        );
    }

    #[test]
    fn no_web_is_no_sites_and_a_title_with_markup_stays_its_characters() {
        let read = read(br#"{"type":"search","query":{"original":"x"}}"#).unwrap();
        assert!(read.found.is_empty() && read.left_out.is_empty());
        let read = super::read(
            br#"{"type":"search","web":{"results":[{"title":"<b>Fish &amp; chips</b>","url":"https://a.example/"}]}}"#,
        )
        .unwrap();
        assert_eq!(read.found[0].title, "<b>Fish & chips</b>");
    }

    #[test]
    fn a_whole_answer_is_refused_by_name() {
        let refused = |answer: &[u8]| read(answer).err();
        assert!(matches!(refused(b"<html>"), Some(Unread::NotJson(_))));
        assert!(matches!(refused(b""), Some(Unread::NotJson(_))));
        assert!(matches!(
            refused(br#"{"type":"search""#),
            Some(Unread::NotJson(_))
        ));
        assert_eq!(refused(b"[1,2]"), Some(Unread::NotAnObject));
        assert_eq!(refused(b"null"), Some(Unread::NotAnObject));
        assert_eq!(refused(b"{}"), Some(Unread::NotASearch { said: None }));
        assert_eq!(
            refused(br#"{"type":"ErrorResponse","error":{"code":"RATE_LIMITED"}}"#),
            Some(Unread::NotASearch {
                said: Some("ErrorResponse".to_owned())
            })
        );
        assert_eq!(
            refused(br#"{"type":"search","web":[]}"#),
            Some(Unread::WebNotAnObject)
        );
        assert_eq!(
            refused(br#"{"type":"search","web":{}}"#),
            Some(Unread::NoResults)
        );
        assert_eq!(
            refused(br#"{"type":"search","web":{"results":{}}}"#),
            Some(Unread::ResultsNotAList)
        );
        let deep = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
        assert_eq!(
            refused(deep.as_bytes()),
            Some(Unread::TooDeep { depth: 10_000 })
        );
        let large = vec![b' '; LARGEST + 1];
        assert_eq!(
            refused(&large),
            Some(Unread::TooLarge { bytes: LARGEST + 1 })
        );
    }

    /// An image search's answer in Brave's shape, written by hand.
    const IMAGES: &str = r#"{
      "type": "images",
      "query": { "original": "ev charger" },
      "results": [
        { "type": "image_result", "title": "A wall charger &amp; its <strong>cable</strong>",
          "url": "https://evhomeguide.org/chargers", "source": "evhomeguide.org",
          "thumbnail": { "src": "https://imgs.search.brave.com/abc/rs:fit:500:0:0:0/g:ce/xyz", "width": 500, "height": 375 },
          "properties": { "url": "https://evhomeguide.org/wall.jpg", "placeholder": "https://imgs.search.brave.com/p" } },
        { "title": "No thumbnail", "url": "https://a.example/" },
        { "title": "A thumbnail not at a site", "url": "https://b.example/",
          "thumbnail": { "src": "data:image/png;base64,AAAA" }, "source": 7 },
        { "title": "Not a site", "url": "ftp://c.example/" },
        { "url": "https://untitled.example/" },
        { "title": "No page" },
        [],
        { "title": "A thumbnail that is no object", "url": "https://d.example/", "thumbnail": "x" }
      ]
    }"#;

    #[test]
    fn an_image_answer_is_read_as_images_with_their_thumbnails() {
        let read = read_images(IMAGES.as_bytes()).unwrap();
        let titles: Vec<&str> = read
            .found
            .iter()
            .map(|found| found.pictured.title.as_str())
            .collect();
        assert_eq!(
            titles,
            vec![
                "A wall charger & its cable",
                "No thumbnail",
                "A thumbnail not at a site",
                "A thumbnail that is no object"
            ]
        );
        let first = &read.found[0];
        assert_eq!(
            first.pictured.page.to_string(),
            "https://evhomeguide.org/chargers"
        );
        assert_eq!(first.pictured.source, "evhomeguide.org");
        assert_eq!(first.pictured.picture, None);
        assert_eq!(
            first.thumbnail.as_ref().map(ToString::to_string).as_deref(),
            Some("https://imgs.search.brave.com/abc/rs:fit:500:0:0:0/g:ce/xyz")
        );
        assert!(
            read.found[1..]
                .iter()
                .all(|found| found.thumbnail.is_none())
        );
        assert_eq!(
            read.found[2].pictured.source, "",
            "a source not text is none"
        );
        let left: Vec<String> = read.left_out.iter().map(ToString::to_string).collect();
        assert_eq!(
            left,
            vec![
                "the result at 3 is not at a site's address",
                "the result at 4 has no title",
                "the result at 5 has no url",
                "the result at 6 is not an object",
            ]
        );
    }

    #[test]
    fn an_image_answer_is_refused_whole_by_name_and_never_panics() {
        let refused = |answer: &[u8]| read_images(answer).err();
        assert!(matches!(refused(b"<html>"), Some(Unread::NotJson(_))));
        assert_eq!(refused(b"7"), Some(Unread::NotAnObject));
        assert_eq!(
            refused(br#"{"type":"search","web":{"results":[]}}"#),
            Some(Unread::NotImages {
                said: Some("search".to_owned())
            })
        );
        assert_eq!(refused(b"{}"), Some(Unread::NotImages { said: None }));
        assert_eq!(refused(br#"{"type":"images"}"#), Some(Unread::NoResults));
        assert_eq!(
            refused(br#"{"type":"images","results":"x"}"#),
            Some(Unread::ResultsNotAList)
        );
        let deep = format!("{}{}", "{\"a\":".repeat(100), "}".repeat(100));
        assert_eq!(
            refused(deep.as_bytes()),
            Some(Unread::TooDeep { depth: 100 })
        );
        let large = vec![b' '; LARGEST + 1];
        assert_eq!(
            refused(&large),
            Some(Unread::TooLarge { bytes: LARGEST + 1 })
        );
        assert_eq!(
            Unread::NotImages { said: None }.to_string(),
            "it does not say it is an image search"
        );
        let many: Vec<String> = (0..60)
            .map(|n| format!(r#"{{"title":"{n}","url":"https://s{n}.example/"}}"#))
            .collect();
        let answer = format!(r#"{{"type":"images","results":[{}]}}"#, many.join(","));
        assert_eq!(
            read_images(answer.as_bytes()).unwrap().found.len(),
            MOST_IMAGES
        );
        for end in 0..IMAGES.len() {
            let _ = read_images(IMAGES.as_bytes().get(..end).unwrap_or_default());
        }
        let _ = read_images(&[0xff, b'{', 0x00]);
    }

    /// A video search's answer in Brave's shape, written by hand.
    const VIDEOS: &str = r#"{
      "type": "videos",
      "query": { "original": "ev charger" },
      "results": [
        { "type": "video_result", "title": "Fitting a <strong>charger</strong> &amp; more",
          "url": "https://www.youtube.example/watch?v=abc", "description": "Start to finish.",
          "age": "2 days ago",
          "video": { "duration": "04:05", "views": 1200, "creator": "Home Energy",
                     "publisher": "YouTube" },
          "meta_url": { "hostname": "www.youtube.example" },
          "thumbnail": { "src": "https://imgs.search.brave.com/v/abc", "original": "https://i.ytimg.example/a.jpg" } },
        { "title": "No publisher", "url": "https://vimeo.example/1",
          "video": { "duration": "PT4M" }, "meta_url": { "hostname": "vimeo.example" } },
        { "title": "Nothing else", "url": "https://plain.example/v",
          "video": "x", "thumbnail": { "src": "javascript:x()" }, "age": 3 },
        { "title": "Not a site", "url": "file:///etc/passwd" },
        { "url": "https://untitled.example/" },
        { "title": "No page" },
        7
      ]
    }"#;

    #[test]
    fn a_video_answer_is_read_as_videos_with_their_lengths_and_thumbnails() {
        let read = read_videos(VIDEOS.as_bytes()).unwrap();
        let videos: Vec<&Video> = read.found.iter().map(|found| &found.video).collect();
        assert_eq!(videos.len(), 3);
        let first = &videos[0];
        assert_eq!(first.title, "Fitting a charger & more");
        assert_eq!(first.length, "04:05");
        assert_eq!(first.site, "YouTube", "the publisher, not the creator");
        assert_eq!(first.age, "2 days ago");
        assert_eq!(first.description, "Start to finish.");
        assert_eq!(first.picture, None);
        assert_eq!(
            read.found[0]
                .thumbnail
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            Some("https://imgs.search.brave.com/v/abc"),
            "the proxy's thumbnail, never the original"
        );
        assert_eq!(videos[1].length, "", "a duration not a length is none");
        assert_eq!(
            videos[1].site, "vimeo.example",
            "the hostname with no publisher"
        );
        assert_eq!(videos[2].site, "");
        assert_eq!(videos[2].age, "", "an age not text is none");
        assert!(
            read.found[1..]
                .iter()
                .all(|found| found.thumbnail.is_none())
        );
        let left: Vec<String> = read.left_out.iter().map(ToString::to_string).collect();
        assert_eq!(
            left,
            vec![
                "the result at 3 is not at a site's address",
                "the result at 4 has no title",
                "the result at 5 has no url",
                "the result at 6 is not an object",
            ]
        );
    }

    /// A news search's answer in Brave's shape, written by hand.
    const NEWS: &str = r#"{
      "type": "news",
      "query": { "original": "ev charger" },
      "results": [
        { "type": "news_result", "title": "More electricians &#x27;trained&#x27;",
          "url": "https://energy.example/a", "description": "Waiting times are <strong>shorter</strong>.",
          "age": "2 days ago", "page_age": "2026-10-29T08:00:00",
          "profile": { "name": "Consumer energy report", "url": "https://energy.example" },
          "meta_url": { "hostname": "energy.example" },
          "thumbnail": { "src": "https://imgs.search.brave.com/n/a" } },
        { "title": "Only a hostname", "url": "https://daily.example/b",
          "profile": { "name": "  " }, "meta_url": { "hostname": "daily.example" } },
        { "title": "Bare", "url": "http://bare.example/", "profile": 1, "age": null },
        { "title": "Not a site", "url": "mailto:a@b.example" },
        { "title": 4, "url": "https://four.example/" },
        []
      ]
    }"#;

    #[test]
    fn a_news_answer_is_read_as_stories_with_their_sites_and_ages() {
        let read = read_news(NEWS.as_bytes()).unwrap();
        let titles: Vec<&str> = read
            .found
            .iter()
            .map(|story| story.title.as_str())
            .collect();
        assert_eq!(
            titles,
            vec!["More electricians 'trained'", "Only a hostname", "Bare"]
        );
        let first = &read.found[0];
        assert_eq!(first.site, "Consumer energy report");
        assert_eq!(first.age, "2 days ago");
        assert_eq!(first.description, "Waiting times are shorter.");
        assert_eq!(first.page.to_string(), "https://energy.example/a");
        assert_eq!(
            read.found[1].site, "daily.example",
            "an empty name, then the hostname"
        );
        assert_eq!(
            (read.found[2].site.as_str(), read.found[2].age.as_str()),
            ("", "")
        );
        let left: Vec<String> = read.left_out.iter().map(ToString::to_string).collect();
        assert_eq!(
            left,
            vec![
                "the result at 3 is not at a site's address",
                "the result at 4 has no title",
                "the result at 5 is not an object",
            ]
        );
    }

    #[test]
    fn video_and_news_answers_are_refused_whole_by_name_and_never_panic() {
        assert_eq!(
            read_videos(br#"{"type":"news","results":[]}"#).err(),
            Some(Unread::NotVideos {
                said: Some("news".to_owned())
            })
        );
        assert_eq!(
            read_news(br#"{"type":"videos","results":[]}"#).err(),
            Some(Unread::NotNews {
                said: Some("videos".to_owned())
            })
        );
        assert_eq!(
            read_videos(b"{}").err(),
            Some(Unread::NotVideos { said: None })
        );
        assert_eq!(read_news(b"{}").err(), Some(Unread::NotNews { said: None }));
        assert_eq!(
            read_videos(br#"{"type":"videos"}"#).err(),
            Some(Unread::NoResults)
        );
        assert_eq!(
            read_news(br#"{"type":"news","results":{}}"#).err(),
            Some(Unread::ResultsNotAList)
        );
        assert_eq!(read_news(b"[]").err(), Some(Unread::NotAnObject));
        assert!(matches!(read_videos(b"<html>"), Err(Unread::NotJson(_))));
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert_eq!(
            read_news(deep.as_bytes()).err(),
            Some(Unread::TooDeep { depth: 200 })
        );
        let large = vec![b' '; LARGEST + 1];
        assert_eq!(
            read_videos(&large).err(),
            Some(Unread::TooLarge { bytes: LARGEST + 1 })
        );
        assert_eq!(
            Unread::NotVideos { said: None }.to_string(),
            "it does not say it is a video search"
        );
        assert_eq!(
            Unread::NotNews {
                said: Some("x".to_owned())
            }
            .to_string(),
            "its type is \"x\", not \"news\""
        );
        let many = |kind: &str| {
            let each: Vec<String> = (0..60)
                .map(|n| format!(r#"{{"title":"{n}","url":"https://s{n}.example/"}}"#))
                .collect();
            format!(r#"{{"type":"{kind}","results":[{}]}}"#, each.join(","))
        };
        assert_eq!(
            read_videos(many("videos").as_bytes()).unwrap().found.len(),
            MOST_VIDEOS
        );
        assert_eq!(
            read_news(many("news").as_bytes()).unwrap().found.len(),
            MOST_STORIES
        );
        for end in 0..VIDEOS.len() {
            let _ = read_videos(VIDEOS.as_bytes().get(..end).unwrap_or_default());
        }
        for end in 0..NEWS.len() {
            let _ = read_news(NEWS.as_bytes().get(..end).unwrap_or_default());
        }
        let _ = read_videos(&[0xff, b'{', 0x00]);
        let _ = read_news(
            br#"{"type":"news","results":[{"title":"\ud800","url":"https://a.example/"}]}"#,
        );
    }

    #[test]
    fn at_most_twenty_sites_are_kept_and_no_bytes_make_it_panic() {
        let many: Vec<String> = (0..50)
            .map(|n| format!(r#"{{"title":"{n}","url":"https://s{n}.example/"}}"#))
            .collect();
        let answer = format!(
            r#"{{"type":"search","web":{{"results":[{}]}}}}"#,
            many.join(",")
        );
        assert_eq!(read(answer.as_bytes()).unwrap().found.len(), MOST_FOUND);
        // Every cut of a real-shaped answer, and some bytes that are not
        // text at all: refused or read, never a panic.
        for end in 0..ANSWER.len() {
            let _ = read(ANSWER.as_bytes().get(..end).unwrap_or_default());
        }
        let _ = read(&[0xff, 0xfe, b'{', 0x00, b'"']);
        let _ = read(br#"{"type":"search","web":{"results":[{"title":"\ud800","url":"https://a.example/"}]}}"#);
    }
}

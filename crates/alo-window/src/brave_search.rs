/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One web search asked of the Brave Search API, and what its answer comes
//! to (ADR 0082 § 0, ADR 0083, queue item 708).
//!
//! # What is sent
//!
//! ADR 0083 § 3: a `GET` of [`ENDPOINT`] with **the words typed, trimmed, as
//! `q`, and nothing else in the address** — no count, no country, no
//! language, no partner code — and the key as `X-Subscription-Token`,
//! `Accept: application/json`, and what `alo-net` sends with every request
//! (`Host`, alo's `User-Agent`, `Accept-Encoding`). No cookie, no `Referer`.
//! It is asked as the person's own (`Cause::Person`), since a person chose
//! it.
//!
//! # Where it goes
//!
//! [`Provider::brave`] asks [`ENDPOINT`], always. A test gives another
//! address with [`Provider::at`] — a server on this machine answering
//! hand-written replies in Brave's shape. That is a Rust constructor and
//! nothing else: no page, renderer, agent, argument or environment variable
//! can change where a search goes.
//!
//! # Images (queue item 743, ADR 0086)
//!
//! Once a web search has found sites, the same words alone are asked of
//! [`IMAGES_ENDPOINT`] ([`images`]), and each image's thumbnail is fetched
//! only from Brave's own image proxy, [`THUMBNAILS`]
//! ([`crate::search_thumbnails`]): showing the results contacts no site but
//! Brave's. A test's provider names its own two addresses
//! ([`Provider::with_images`]); one made with [`Provider::at`] alone asks
//! no images.
//!
//! # Videos and news (queue items 745, 746, ADR 0087)
//!
//! Asked only when a person opens the Videos or News tab of a turn, once
//! for that turn: the same words alone, of [`VIDEOS_ENDPOINT`] or
//! [`NEWS_ENDPOINT`] ([`videos`], [`news`]). A video's thumbnail is fetched
//! as an image's is, only from [`THUMBNAILS`]. A test's provider names its
//! own addresses ([`Provider::with_videos`], [`Provider::with_news`]); one
//! that names none is not asked, and says so.
//!
//! # What the person is told
//!
//! ADR 0083 § 5's sentences, by status ([`unanswered`]). None says the key.

use crate::search_key::Key;
use crate::search_protocol::{self, ImageFound, VideoFound};
use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Request};
use alo_renderer::results::{PROVIDER, Showing};
use alo_renderer::results_media::Story;
use alo_url::Url;

/// Where a web search is asked.
pub const ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

/// Where an image search is asked (queue item 743).
pub const IMAGES_ENDPOINT: &str = "https://api.search.brave.com/res/v1/images/search";

/// Where a video search is asked (queue item 745).
pub const VIDEOS_ENDPOINT: &str = "https://api.search.brave.com/res/v1/videos/search";

/// Where a news search is asked (queue item 746).
pub const NEWS_ENDPOINT: &str = "https://api.search.brave.com/res/v1/news/search";

/// Brave's own image proxy: the one place a thumbnail is fetched from.
pub const THUMBNAILS: &str = "https://imgs.search.brave.com/";

/// Where searches go and with what key.
#[derive(Debug, Clone)]
pub struct Provider {
    /// The web search's address, with no query.
    endpoint: Url,
    /// The image search's address, with no query, and the one origin its
    /// thumbnails are fetched from: [`None`] where images are not asked.
    images: Option<(Url, Url)>,
    /// The video search's address, with no query: [`None`] where videos
    /// are not asked.
    videos: Option<Url>,
    /// The news search's address, with no query: [`None`] where news is
    /// not asked.
    news: Option<Url>,
    /// The key.
    key: Key,
}

impl Provider {
    /// The Brave Search API, asked with `key`.
    ///
    /// # Errors
    ///
    /// Only if the URL parser stopped reading [`ENDPOINT`], which a test pins.
    pub fn brave(key: Key) -> Result<Self, String> {
        Self::at(ENDPOINT, key)?
            .with_images(IMAGES_ENDPOINT, THUMBNAILS)?
            .with_videos(VIDEOS_ENDPOINT)?
            .with_news(NEWS_ENDPOINT)
    }

    /// A provider answering at `endpoint` in Brave's shape, asked with
    /// `key`: for a test's server on this machine.
    ///
    /// # Errors
    ///
    /// When `endpoint` is not an `http` or `https` address.
    pub fn at(endpoint: &str, key: Key) -> Result<Self, String> {
        let endpoint = site(endpoint)?;
        Ok(Self {
            endpoint,
            images: None,
            videos: None,
            news: None,
            key,
        })
    }

    /// The same provider, asking image searches at `endpoint` and fetching
    /// thumbnails only from the origin of `thumbnails` (queue item 743).
    ///
    /// # Errors
    ///
    /// When either is not an `http` or `https` address.
    pub fn with_images(mut self, endpoint: &str, thumbnails: &str) -> Result<Self, String> {
        self.images = Some((site(endpoint)?, site(thumbnails)?));
        Ok(self)
    }

    /// The same provider, asking video searches at `endpoint` (queue item
    /// 745); their thumbnails come from the origin [`Provider::with_images`]
    /// names, or none are fetched.
    ///
    /// # Errors
    ///
    /// When it is not an `http` or `https` address.
    pub fn with_videos(mut self, endpoint: &str) -> Result<Self, String> {
        self.videos = Some(site(endpoint)?);
        Ok(self)
    }

    /// The same provider, asking news searches at `endpoint` (queue item
    /// 746).
    ///
    /// # Errors
    ///
    /// When it is not an `http` or `https` address.
    pub fn with_news(mut self, endpoint: &str) -> Result<Self, String> {
        self.news = Some(site(endpoint)?);
        Ok(self)
    }

    /// The request for a video search of `words`, where videos are asked.
    ///
    /// # Errors
    ///
    /// When videos are not asked of this provider, or the words make no
    /// address.
    pub fn videos_request(
        &self,
        words: &str,
        identities: &mut Identities,
    ) -> Result<Request, String> {
        let endpoint = self
            .videos
            .as_ref()
            .ok_or_else(|| format!("{PROVIDER} is not asked for videos here"))?;
        self.asking(endpoint, words, identities)
    }

    /// The request for a news search of `words`, where news is asked.
    ///
    /// # Errors
    ///
    /// When news is not asked of this provider, or the words make no
    /// address.
    pub fn news_request(
        &self,
        words: &str,
        identities: &mut Identities,
    ) -> Result<Request, String> {
        let endpoint = self
            .news
            .as_ref()
            .ok_or_else(|| format!("{PROVIDER} is not asked for news here"))?;
        self.asking(endpoint, words, identities)
    }

    /// Whether images are asked of this provider.
    pub fn asks_images(&self) -> bool {
        self.images.is_some()
    }

    /// The one origin thumbnails are fetched from, where images are asked.
    pub fn thumbnails(&self) -> Option<&Url> {
        self.images.as_ref().map(|(_, thumbnails)| thumbnails)
    }

    /// The request for a search of `words`, made as the person's in `tab`.
    ///
    /// # Errors
    ///
    /// When the words make no address, which percent-encoding rules out.
    pub fn request(&self, words: &str, identities: &mut Identities) -> Result<Request, String> {
        self.asking(&self.endpoint, words, identities)
    }

    /// The request for an image search of `words`, made as the person's,
    /// where images are asked: the words alone, as a web search's.
    ///
    /// # Errors
    ///
    /// When images are not asked of this provider, or the words make no
    /// address.
    pub fn images_request(
        &self,
        words: &str,
        identities: &mut Identities,
    ) -> Result<Request, String> {
        let (endpoint, _) = self
            .images
            .as_ref()
            .ok_or_else(|| format!("{PROVIDER} is not asked for images here"))?;
        self.asking(endpoint, words, identities)
    }

    /// `endpoint` asked for `words` alone, with the key.
    fn asking(
        &self,
        endpoint: &Url,
        words: &str,
        identities: &mut Identities,
    ) -> Result<Request, String> {
        let query = alo_url::form::urlencoded([("q", words.trim())]);
        let url = alo_url::with_query(endpoint, &query)
            .map_err(|why| format!("the search could not be addressed: {why}"))?;
        let mut request = Request::get(
            url,
            Cause::Person {
                tab: identities.a_tab(),
            },
        );
        request.headers.add("Accept", "application/json");
        request
            .headers
            .add("X-Subscription-Token", self.key.header_value());
        Ok(request)
    }
}

/// Ask `provider` for `words` through `pool`: the sites found, or why the
/// search was not answered, in a sentence a person reads.
pub fn search(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Showing {
    let request = match provider.request(words, identities) {
        Ok(request) => request,
        Err(why) => return Showing::Unanswered(why),
    };
    match pool.fetch(&request) {
        Ok(response) if response.status.0 == 200 => answered(&response.body),
        Ok(response) => Showing::Unanswered(unanswered(response.status.0)),
        Err(why) => Showing::Unanswered(format!("{PROVIDER} could not be reached: {why}")),
    }
}

/// Ask `provider` for images of `words` through `pool`: the images found,
/// each with its thumbnail's address, or why they were not, in a sentence a
/// person reads.
///
/// # Errors
///
/// That sentence: not asked here, a status that is not 200, no connection,
/// or an answer that could not be read.
pub fn images(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Result<Vec<ImageFound>, String> {
    let request = provider.images_request(words, identities)?;
    match pool.fetch(&request) {
        Ok(response) if response.status.0 == 200 => search_protocol::read_images(&response.body)
            .map(|read| read.found)
            .map_err(|why| format!("{PROVIDER}'s answer could not be read: {why}")),
        Ok(response) => Err(unanswered(response.status.0)),
        Err(why) => Err(format!("{PROVIDER} could not be reached: {why}")),
    }
}

/// Ask `provider` for videos of `words` through `pool`: the videos found,
/// each with its thumbnail's address, or why they were not, in a sentence a
/// person reads.
///
/// # Errors
///
/// That sentence: not asked here, a status that is not 200, no connection,
/// or an answer that could not be read.
pub fn videos(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Result<Vec<VideoFound>, String> {
    let request = provider.videos_request(words, identities)?;
    answer_of(pool, &request, |body| {
        search_protocol::read_videos(body).map(|read| read.found)
    })
}

/// Ask `provider` for news of `words` through `pool`: the stories found, or
/// why they were not, in a sentence a person reads.
///
/// # Errors
///
/// That sentence: not asked here, a status that is not 200, no connection,
/// or an answer that could not be read.
pub fn news(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Result<Vec<Story>, String> {
    let request = provider.news_request(words, identities)?;
    answer_of(pool, &request, |body| {
        search_protocol::read_news(body).map(|read| read.found)
    })
}

/// `request` fetched through `pool`, and its body of 200 read by `read`:
/// what was read, or why not, in a sentence a person reads.
fn answer_of<T>(
    pool: &mut Pool,
    request: &Request,
    read: impl FnOnce(&[u8]) -> Result<T, search_protocol::Unread>,
) -> Result<T, String> {
    match pool.fetch(request) {
        Ok(response) if response.status.0 == 200 => read(&response.body)
            .map_err(|why| format!("{PROVIDER}'s answer could not be read: {why}")),
        Ok(response) => Err(unanswered(response.status.0)),
        Err(why) => Err(format!("{PROVIDER} could not be reached: {why}")),
    }
}

/// `written`, if it is an `http` or `https` address.
fn site(written: &str) -> Result<Url, String> {
    alo_url::parse(written)
        .ok()
        .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
        .ok_or_else(|| format!("{written} is not a search provider's address"))
}

/// What an answer of 200 with `body` comes to.
pub fn answered(body: &[u8]) -> Showing {
    match search_protocol::read(body) {
        Ok(read) => Showing::Found(read.found),
        Err(why) => Showing::Unanswered(format!("{PROVIDER}'s answer could not be read: {why}")),
    }
}

/// What a person is told when the answer's status is `status`, not 200.
pub fn unanswered(status: u16) -> String {
    match status {
        401 | 403 => format!(
            "{PROVIDER} refused alo's key ({status}). The key in this Mac's keychain may be wrong \
             or revoked."
        ),
        429 => format!(
            "{PROVIDER} says alo has searched too often, or its allowance is used up ({status}). \
             Try again later."
        ),
        500..=599 => format!("{PROVIDER} could not answer just now ({status})."),
        _ => format!("{PROVIDER} answered {status}, which alo does not read."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Key {
        Key::of("test-key-not-real").unwrap()
    }

    #[test]
    fn the_endpoint_is_brave_and_only_https_or_http_is_a_provider() {
        let brave = Provider::brave(key()).unwrap();
        assert_eq!(brave.endpoint.to_string(), ENDPOINT);
        assert!(Provider::at("http://127.0.0.1:9/res/v1/web/search", key()).is_ok());
        assert!(Provider::at("file:///etc/passwd", key()).is_err());
        assert!(Provider::at("not an address", key()).is_err());
        assert!(brave.asks_images());
        assert_eq!(
            brave.thumbnails().map(ToString::to_string).as_deref(),
            Some(THUMBNAILS)
        );
        let at = Provider::at("http://127.0.0.1:9/res/v1/web/search", key()).unwrap();
        assert!(!at.asks_images() && at.thumbnails().is_none());
        assert!(
            at.images_request("x", &mut Identities::default()).is_err(),
            "no image search where none is named"
        );
        assert!(at.clone().with_images("javascript:x", THUMBNAILS).is_err());
    }

    #[test]
    fn an_image_search_carries_the_words_alone_and_the_key_in_its_header() {
        let brave = Provider::brave(key()).unwrap();
        let request = brave
            .images_request(" ev charger ", &mut Identities::default())
            .unwrap();
        assert_eq!(
            request.url.to_string(),
            "https://api.search.brave.com/res/v1/images/search?q=ev+charger"
        );
        let headers: Vec<(String, String)> = request
            .headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        assert_eq!(
            headers,
            vec![
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "X-Subscription-Token".to_owned(),
                    "test-key-not-real".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn videos_and_news_are_asked_with_the_words_alone_where_the_provider_names_them() {
        let brave = Provider::brave(key()).unwrap();
        let mut identities = Identities::default();
        let videos = brave
            .videos_request(" ev charger ", &mut identities)
            .unwrap();
        assert_eq!(
            videos.url.to_string(),
            "https://api.search.brave.com/res/v1/videos/search?q=ev+charger"
        );
        let news = brave.news_request("ev & more", &mut identities).unwrap();
        assert_eq!(
            news.url.to_string(),
            "https://api.search.brave.com/res/v1/news/search?q=ev+%26+more"
        );
        for request in [videos, news] {
            let headers: Vec<(String, String)> = request
                .headers
                .iter()
                .map(|header| (header.name.clone(), header.value.clone()))
                .collect();
            assert_eq!(
                headers,
                vec![
                    ("Accept".to_owned(), "application/json".to_owned()),
                    (
                        "X-Subscription-Token".to_owned(),
                        "test-key-not-real".to_owned()
                    ),
                ]
            );
            assert!(matches!(request.cause, Cause::Person { .. }));
        }
        let at = Provider::at("http://127.0.0.1:9/res/v1/web/search", key()).unwrap();
        assert_eq!(
            at.videos_request("x", &mut identities).err().as_deref(),
            Some("Brave Search is not asked for videos here")
        );
        assert_eq!(
            at.news_request("x", &mut identities).err().as_deref(),
            Some("Brave Search is not asked for news here")
        );
        assert!(at.clone().with_videos("ftp://x.example/").is_err());
        assert!(at.with_news("not an address").is_err());
    }

    #[test]
    fn the_request_carries_the_words_alone_and_the_key_in_its_header() {
        let brave = Provider::brave(key()).unwrap();
        let request = brave
            .request("  rust borrow & checker?  ", &mut Identities::default())
            .unwrap();
        assert_eq!(
            request.url.to_string(),
            "https://api.search.brave.com/res/v1/web/search?q=rust+borrow+%26+checker%3F"
        );
        assert_eq!(request.method, "GET");
        assert!(matches!(request.cause, Cause::Person { .. }));
        let headers: Vec<(String, String)> = request
            .headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        assert_eq!(
            headers,
            vec![
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "X-Subscription-Token".to_owned(),
                    "test-key-not-real".to_owned()
                ),
            ]
        );
        assert!(request.body.is_empty());
        assert!(request.initiator.is_none());
    }

    #[test]
    fn every_status_is_said_in_words_and_none_says_the_key() {
        for (status, says) in [
            (401, "refused alo's key (401)"),
            (403, "refused alo's key (403)"),
            (429, "allowance is used up (429)"),
            (500, "could not answer just now (500)"),
            (503, "could not answer just now (503)"),
            (422, "answered 422, which alo does not read"),
            (302, "answered 302"),
        ] {
            let said = unanswered(status);
            assert!(said.contains(says), "{status}: {said}");
            assert!(said.starts_with("Brave Search"), "{said}");
            assert!(!said.contains("test-key"), "{said}");
        }
    }

    #[test]
    fn an_answer_is_sites_or_says_why_it_could_not_be_read() {
        assert_eq!(
            answered(br#"{"type":"search"}"#),
            Showing::Found(Vec::new())
        );
        assert_eq!(
            answered(b"<html>"),
            Showing::Unanswered(
                "Brave Search's answer could not be read: it is not JSON (expected value at \
                 line 1 column 1)"
                    .to_owned()
            )
        );
    }
}

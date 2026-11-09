/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search asked of a `SearXNG` — open source, run by whoever gives its
//! address, asked with no key — and what its answer comes to (ADR 0106 §§
//! 1–2, queue item 904): one connector of the search service
//! ([`crate::search_service`]).
//!
//! # What is sent
//!
//! `SearXNG`'s search API (`docs.searxng.org/dev/search_api.html`): a `GET`
//! of the instance's `/search` with **the words typed, trimmed, as `q`**,
//! and `format=json` — then, only where it is needed, `categories=news`
//! for the News tab and `pageno` for a further page (2 to 10: alo's
//! further pages 1 to [`MOST_PAGES`], one after the first). Nothing else:
//! no language, no time range, no safe-search, nothing of the person or
//! the machine. `Accept: application/json`, and what `alo-net` sends with
//! every request (`Host`, alo's `User-Agent`, `Accept-Encoding`). No
//! cookie, no `Referer`, no key. It is asked as the person's own.
//!
//! # What it searches
//!
//! The web, its further pages, and news. **Not images or videos**: its
//! JSON names each picture at the site holding it, and alo fetches no
//! picture from a site (ADR 0086, ADR 0088), so [`crate::search_service`]
//! says plainly that the service chosen does not search them. A story has
//! no photo and a site no mark: each is drawn with its letter.
//!
//! # Further pages
//!
//! `SearXNG` says nothing of whether there are more. A page that found
//! sites offers the next, up to [`MOST_PAGES`]; one that found none ends
//! them.
//!
//! # What the person is told
//!
//! [`unanswered`]'s sentences, by status. A 403 is what `SearXNG` answers
//! when its JSON format is not allowed (*"Requesting an unset format will
//! return a 403 Forbidden error"*), and is said so. None names the service.

use crate::search_service::{
    Answered, ImageFound, SearchConnector, SearchService, Searched, StoryFound, VideoFound,
};
use crate::searxng_protocol;
use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Request};
use alo_renderer::results::{MOST_PAGES, SEARCH_SERVICE, Showing};
use alo_url::Url;

/// A `SearXNG`, asked at its own `/search`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// Its search's address, with no query.
    search: Url,
}

impl Provider {
    /// The `SearXNG` at `base` — its address as a person gave it, which may
    /// have a path of its own (`https://example.org/searx`): asked at
    /// `/search` under it.
    ///
    /// # Errors
    ///
    /// When `base` is not an `http` or `https` site's address, or holds a
    /// query or a fragment.
    pub fn at(base: &Url) -> Result<Self, String> {
        let not = || format!("{} is not a search service's address", base.serialised);
        if !matches!(base.scheme.as_str(), "http" | "https")
            || base.host.is_none()
            || base.query.is_some()
            || base.fragment.is_some()
        {
            return Err(not());
        }
        let written = format!("{}/search", base.serialised.trim_end_matches('/'));
        let search = alo_url::parse(&written).map_err(|_| not())?;
        Ok(Self { search })
    }

    /// Where its searches are asked.
    pub fn address(&self) -> &Url {
        &self.search
    }

    /// The request for a web search of `words`.
    ///
    /// # Errors
    ///
    /// When the words make no address, which percent-encoding rules out.
    pub fn request(&self, words: &str, identities: &mut Identities) -> Result<Request, String> {
        self.asking(&[("q", words.trim()), ("format", "json")], identities)
    }

    /// The request for the further page at `offset` (1 to [`MOST_PAGES`])
    /// of a web search of `words`: its `pageno` is one more.
    ///
    /// # Errors
    ///
    /// When the offset is not one a person can ask, or the words make no
    /// address.
    pub fn page_request(
        &self,
        words: &str,
        offset: usize,
        identities: &mut Identities,
    ) -> Result<Request, String> {
        if !(1..=MOST_PAGES).contains(&offset) {
            return Err(format!(
                "a further page at {offset} is not one alo asks; it asks 1 to {MOST_PAGES}"
            ));
        }
        let page = (offset + 1).to_string();
        self.asking(
            &[("q", words.trim()), ("format", "json"), ("pageno", &page)],
            identities,
        )
    }

    /// The request for a news search of `words`.
    ///
    /// # Errors
    ///
    /// When the words make no address.
    pub fn news_request(
        &self,
        words: &str,
        identities: &mut Identities,
    ) -> Result<Request, String> {
        self.asking(
            &[
                ("q", words.trim()),
                ("format", "json"),
                ("categories", "news"),
            ],
            identities,
        )
    }

    /// Its search asked with `query`.
    fn asking(
        &self,
        query: &[(&str, &str)],
        identities: &mut Identities,
    ) -> Result<Request, String> {
        let query = alo_url::form::urlencoded(query.iter().copied());
        let url = alo_url::with_query(&self.search, &query)
            .map_err(|why| format!("the search could not be addressed: {why}"))?;
        let mut request = Request::get(
            url,
            Cause::Person {
                tab: identities.a_tab(),
            },
        );
        request.headers.add("Accept", "application/json");
        Ok(request)
    }
}

/// What a web search's `request` came to through `pool`, the further page
/// offered while this one found sites.
fn sites(pool: &mut Pool, request: &Request) -> Answered {
    match pool.fetch(request) {
        Ok(response) if response.status.0 == 200 => answered(&response.body),
        Ok(response) => Answered::not(unanswered(response.status.0)),
        Err(why) => Answered::not(format!("{SEARCH_SERVICE} could not be reached: {why}")),
    }
}

/// What an answer of 200 with `body` comes to: its sites, none with a mark
/// to fetch, and more offered while it found some.
pub fn answered(body: &[u8]) -> Answered {
    match searxng_protocol::read(body) {
        Ok(read) => Answered {
            more: !read.found.is_empty(),
            marks: vec![None; read.found.len()],
            showing: Showing::Found(read.found),
        },
        Err(why) => Answered::not(format!(
            "{SEARCH_SERVICE}'s answer could not be read: {why}"
        )),
    }
}

/// What a person is told when the answer's status is `status`, not 200.
pub fn unanswered(status: u16) -> String {
    match status {
        403 => format!(
            "{SEARCH_SERVICE} refused alo's search ({status}). It may not answer programs: its \
             settings must allow the json format."
        ),
        429 => {
            format!("{SEARCH_SERVICE} says alo has searched too often ({status}). Try again later.")
        }
        500..=599 => format!("{SEARCH_SERVICE} could not answer just now ({status})."),
        _ => format!("{SEARCH_SERVICE} answered {status}, which alo does not read."),
    }
}

impl SearchConnector for Provider {
    fn searches(&self, what: Searched) -> bool {
        matches!(what, Searched::Web | Searched::Pages | Searched::News)
    }

    /// None: its pictures are the sites' own.
    fn pictures(&self) -> Option<&Url> {
        None
    }

    fn search(&self, pool: &mut Pool, identities: &mut Identities, words: &str) -> Answered {
        match self.request(words, identities) {
            Ok(request) => sites(pool, &request),
            Err(why) => Answered::not(why),
        }
    }

    fn page(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
        offset: usize,
    ) -> Answered {
        let answered = match self.page_request(words, offset, identities) {
            Ok(request) => sites(pool, &request),
            Err(why) => Answered::not(why),
        };
        // The last page alo asks offers none after it.
        Answered {
            more: answered.more && offset < MOST_PAGES,
            ..answered
        }
    }

    fn images(&self, _: &mut Pool, _: &mut Identities, _: &str) -> Result<Vec<ImageFound>, String> {
        Err(crate::search_service::unsupported(Searched::Images))
    }

    fn videos(&self, _: &mut Pool, _: &mut Identities, _: &str) -> Result<Vec<VideoFound>, String> {
        Err(crate::search_service::unsupported(Searched::Videos))
    }

    fn news(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<StoryFound>, String> {
        let request = self.news_request(words, identities)?;
        match pool.fetch(&request) {
            Ok(response) if response.status.0 == 200 => searxng_protocol::read_news(&response.body)
                .map(|read| read.found)
                .map_err(|why| format!("{SEARCH_SERVICE}'s answer could not be read: {why}")),
            Ok(response) => Err(unanswered(response.status.0)),
            Err(why) => Err(format!("{SEARCH_SERVICE} could not be reached: {why}")),
        }
    }
}

impl From<Provider> for SearchService {
    fn from(provider: Provider) -> Self {
        Self::of(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(written: &str) -> Provider {
        Provider::at(&alo_url::parse(written).unwrap()).unwrap()
    }

    fn headers(request: &Request) -> Vec<(String, String)> {
        request
            .headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect()
    }

    #[test]
    fn its_search_is_under_the_address_given_with_a_path_or_none() {
        assert_eq!(
            at("https://search.example.org").address().serialised,
            "https://search.example.org/search"
        );
        assert_eq!(
            at("https://example.org/searx/").address().serialised,
            "https://example.org/searx/search"
        );
        assert_eq!(
            at("http://127.0.0.1:8888").address().serialised,
            "http://127.0.0.1:8888/search"
        );
        for not in [
            "file:///etc/passwd",
            "https://a.example/?q=1",
            "https://a.example/#x",
        ] {
            assert!(
                Provider::at(&alo_url::parse(not).unwrap()).is_err(),
                "{not}"
            );
        }
    }

    #[test]
    fn a_search_carries_the_words_and_the_format_alone_and_no_key() {
        let searxng = at("https://search.example.org");
        let mut identities = Identities::default();
        let request = searxng
            .request("  rust borrow & checker?  ", &mut identities)
            .unwrap();
        assert_eq!(
            request.url.to_string(),
            "https://search.example.org/search?q=rust+borrow+%26+checker%3F&format=json"
        );
        assert_eq!(request.method, "GET");
        assert!(matches!(request.cause, Cause::Person { .. }));
        assert_eq!(
            headers(&request),
            vec![("Accept".to_owned(), "application/json".to_owned())]
        );
        assert!(request.body.is_empty() && request.initiator.is_none());
        let news = searxng.news_request("ev", &mut identities).unwrap();
        assert_eq!(
            news.url.to_string(),
            "https://search.example.org/search?q=ev&format=json&categories=news"
        );
        let first = searxng.page_request("ev", 1, &mut identities).unwrap();
        assert_eq!(
            first.url.to_string(),
            "https://search.example.org/search?q=ev&format=json&pageno=2"
        );
        let last = searxng
            .page_request("ev", MOST_PAGES, &mut identities)
            .unwrap();
        assert!(last.url.to_string().ends_with("&pageno=10"));
        for never in [0, MOST_PAGES + 1] {
            assert!(searxng.page_request("ev", never, &mut identities).is_err());
        }
    }

    #[test]
    fn it_searches_the_web_its_pages_and_news_and_names_no_picture_origin() {
        let searxng = SearchService::from(at("https://search.example.org"));
        assert!(searxng.searches(Searched::Web));
        assert!(searxng.searches(Searched::Pages));
        assert!(searxng.searches(Searched::News));
        assert!(!searxng.searches(Searched::Images));
        assert!(!searxng.searches(Searched::Videos));
        assert!(searxng.pictures().is_none() && !searxng.asks_images());
    }

    #[test]
    fn an_answer_is_sites_with_more_offered_while_some_were_found() {
        let found =
            answered(br#"{"results":[{"url":"https://a.example/","title":"A","content":"a"}]}"#);
        assert!(found.more);
        assert_eq!(found.marks, vec![None]);
        assert!(matches!(&found.showing, Showing::Found(sites) if sites.len() == 1));
        let none = answered(br#"{"results":[]}"#);
        assert!(!none.more);
        assert_eq!(none.showing, Showing::Found(Vec::new()));
        assert_eq!(
            answered(b"<html>").showing,
            Showing::Unanswered(
                "The search service's answer could not be read: it is not JSON (expected value \
                 at line 1 column 1)"
                    .to_owned()
            )
        );
    }

    #[test]
    fn every_status_is_said_in_words_and_none_names_the_service() {
        for (status, says) in [
            (
                403,
                "refused alo's search (403). It may not answer programs",
            ),
            (429, "searched too often (429)"),
            (500, "could not answer just now (500)"),
            (502, "could not answer just now (502)"),
            (404, "answered 404, which alo does not read"),
        ] {
            let said = unanswered(status);
            assert!(said.contains(says), "{status}: {said}");
            assert!(said.starts_with(SEARCH_SERVICE), "{said}");
            assert!(!said.to_lowercase().contains("searxng"), "{said}");
        }
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's videos and news stories, as the browser process tells the
//! first screen's renderer of them (queue items 745, 746; ADR 0087).
//!
//! Each is asked of the provider only when a person opens its tab, once
//! for a turn ([`crate::results::Results::videos`],
//! [`crate::results::Results::news`]); what comes is held here. Every
//! string is a stranger's, and enters the first screen's document as a text
//! node ([`crate::videos_document`], [`crate::news_document`]).
//!
//! # A video's picture
//!
//! A video's thumbnail is fetched, as an image's is, by the browser process
//! from the provider's own image proxy alone and bounded
//! ([`crate::results::LARGEST_THUMBNAIL`]), and carried here as the bytes
//! that arrived — decoded only in the confined renderer that draws it.
//!
//! A news story's photo is not fetched (queue item 765).

use alo_url::Url;

/// The most videos a search's results are kept with: Brave's own default
/// for one page of video results.
pub const MOST_VIDEOS: usize = 20;

/// The most news stories a search's results are kept with: Brave's own
/// default for one page of news results.
pub const MOST_STORIES: usize = 20;

/// The longest a video's length is, written as the provider writes it:
/// `HH:MM:SS`.
pub const LONGEST_LENGTH: usize = 8;

/// One video a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    /// What the provider calls the video, as plain text.
    pub title: String,
    /// The page the video is on: an `http` or `https` address, which a
    /// press on its title opens in the card.
    pub page: Url,
    /// How long it plays, as the provider writes it — `4:05`, `1:02:03` —
    /// or nothing when it gave none that is a length ([`is_length`]).
    pub length: String,
    /// The site the provider says it is on, perhaps nothing.
    pub site: String,
    /// How old the provider says it is, in its words, perhaps nothing.
    pub age: String,
    /// What the provider says of it, as plain text, perhaps nothing.
    pub description: String,
    /// The thumbnail's bytes as they arrived, if one did and was kept:
    /// decoded only by the renderer that draws it.
    pub picture: Option<Vec<u8>>,
    /// Its site's own mark, as a site found has one
    /// ([`crate::results::Found::mark`], queue item 747).
    pub mark: Option<Vec<u8>>,
}

/// One news story a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    /// The story's headline, as plain text.
    pub title: String,
    /// The story's page: an `http` or `https` address, which a press on its
    /// title opens in the card.
    pub page: Url,
    /// The site the provider says published it, perhaps nothing.
    pub site: String,
    /// How old the provider says it is, in its words — `2 days ago` —
    /// perhaps nothing.
    pub age: String,
    /// What the provider says of it, as plain text, perhaps nothing.
    pub description: String,
    /// Its site's own mark, as a site found has one
    /// ([`crate::results::Found::mark`], queue item 747).
    pub mark: Option<Vec<u8>>,
}

/// Whether `written` is a length as the provider writes one: one to three
/// groups of digits split by `:`, at most [`LONGEST_LENGTH`] characters,
/// every group after the first two digits.
pub fn is_length(written: &str) -> bool {
    if written.is_empty() || written.len() > LONGEST_LENGTH {
        return false;
    }
    let groups: Vec<&str> = written.split(':').collect();
    if groups.len() > 3 {
        return false;
    }
    groups.iter().enumerate().all(|(at, group)| {
        let digits = !group.is_empty() && group.bytes().all(|byte| byte.is_ascii_digit());
        digits && (at == 0 || group.len() == 2)
    })
}

impl Video {
    /// A video called `title`, on the page at `page`, with nothing else
    /// known of it yet.
    pub fn new(title: impl Into<String>, page: Url) -> Self {
        Self {
            title: title.into(),
            page,
            length: String::new(),
            site: String::new(),
            age: String::new(),
            description: String::new(),
            picture: None,
            mark: None,
        }
    }

    /// The same video, `length` long — kept only when it is a length.
    #[must_use]
    pub fn lasting(mut self, length: &str) -> Self {
        self.length = if is_length(length) {
            length.to_owned()
        } else {
            String::new()
        };
        self
    }

    /// The same video, on the site called `site`, `age` old.
    #[must_use]
    pub fn from_site(mut self, site: impl Into<String>, age: impl Into<String>) -> Self {
        self.site = site.into();
        self.age = age.into();
        self
    }

    /// The same video, said of as `description`.
    #[must_use]
    pub fn said_of(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// The same video, its thumbnail `bytes`.
    #[must_use]
    pub fn holding(mut self, bytes: Vec<u8>) -> Self {
        self.picture = Some(bytes);
        self
    }

    /// The same video, its site's mark `bytes` (queue item 747).
    #[must_use]
    pub fn marked(mut self, bytes: Vec<u8>) -> Self {
        self.mark = Some(bytes);
        self
    }

    /// The site it is on: the provider's word for it, or its page's host.
    pub fn site(&self) -> String {
        named_or_host(&self.site, &self.page)
    }

    /// The site's mark, as a site found has one.
    pub fn letter(&self) -> Option<String> {
        crate::results::letter(&self.site())
    }

    /// The mark's tint, as a site found's, by its page's host.
    pub fn tint(&self) -> usize {
        crate::results::tint(&self.page)
    }
}

impl Story {
    /// A story headed `title`, at `page`, with nothing else known of it yet.
    pub fn new(title: impl Into<String>, page: Url) -> Self {
        Self {
            title: title.into(),
            page,
            site: String::new(),
            age: String::new(),
            description: String::new(),
            mark: None,
        }
    }

    /// The same story, from the site called `site`, `age` old.
    #[must_use]
    pub fn from_site(mut self, site: impl Into<String>, age: impl Into<String>) -> Self {
        self.site = site.into();
        self.age = age.into();
        self
    }

    /// The same story, said of as `description`.
    #[must_use]
    pub fn said_of(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// The same story, its site's mark `bytes` (queue item 747).
    #[must_use]
    pub fn marked(mut self, bytes: Vec<u8>) -> Self {
        self.mark = Some(bytes);
        self
    }

    /// The site that published it: the provider's word for it, or its
    /// page's host.
    pub fn site(&self) -> String {
        named_or_host(&self.site, &self.page)
    }

    /// The site's mark, as a site found has one.
    pub fn letter(&self) -> Option<String> {
        crate::results::letter(&self.site())
    }

    /// The mark's tint, as a site found's, by its page's host.
    pub fn tint(&self) -> usize {
        crate::results::tint(&self.page)
    }
}

/// `name`, trimmed, or `page`'s host when it is nothing.
fn named_or_host(name: &str, page: &Url) -> String {
    if name.trim().is_empty() {
        alo_url::reading::host(page)
    } else {
        name.trim().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap_or_else(|_| Url::about_blank())
    }

    #[test]
    fn a_length_is_digits_split_by_colons_and_nothing_else() {
        for length in ["4:05", "0:59", "12:00", "1:02:03", "45", "10:00:00"] {
            assert!(is_length(length), "{length}");
        }
        for not in [
            "",
            "4:5",
            ":05",
            "4:",
            "1:2:3:4",
            "100:00:00",
            "4m05s",
            "4:05 ",
            "\u{661}:05",
            "<b>4:05</b>",
        ] {
            assert!(!is_length(not), "{not}");
        }
        let video = Video::new("A", url("https://v.example/watch")).lasting("PT4M5S");
        assert_eq!(video.length, "", "a length that is not one is none");
        assert_eq!(video.lasting("4:05").length, "4:05");
    }

    #[test]
    fn a_video_and_a_story_are_named_by_the_provider_or_by_their_host() {
        let video = Video::new("A", url("https://www.youtube.example/watch?v=1"));
        assert_eq!(video.site(), "www.youtube.example");
        assert_eq!(video.letter().as_deref(), Some("W"));
        let named = video.from_site(" YouTube ", "2 days ago");
        assert_eq!(named.site(), "YouTube");
        assert_eq!(named.age, "2 days ago");
        assert!(named.tint() < 4);
        let story = Story::new("S", url("https://news.example/a")).from_site("", "1 week ago");
        assert_eq!(story.site(), "news.example");
        assert_eq!(story.letter().as_deref(), Some("N"));
    }
}

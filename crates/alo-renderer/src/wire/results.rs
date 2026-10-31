/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's results on the wire (queue item 708, ADR 0083), sent with the
//! first screen's state ([`super::start`]).
//!
//! Read by a confined renderer as the rest of the first screen is: whether
//! searching is set up and why not; and, for each turn of the conversation
//! that is a search ([`super::start`], queue item 703), what the person said
//! and where the search is, by a closed list of tags.
//! Every site's title, name and description is text that stays text; its
//! address is read back with the URL parser and refused unless it is a
//! site's, so a renderer is never told to draw a link to anything else, and
//! more sites than [`MOST_FOUND`] are refused.
//!
//! Then where the search for images is (queue item 743), by a closed list
//! of tags: each image's title and site as text, the page it was found on
//! refused unless it is a site's, and its thumbnail, if one is sent,
//! refused larger than [`LARGEST_THUMBNAIL`] or unless its bytes begin as a
//! picture this engine reads — decoded, in this confined process, only by
//! the picture readers every `<img>` goes through; more images than
//! [`MOST_IMAGES`] are refused. Then the tab shown (744), by a tag, refused
//! unless it can be chosen ([`Results::open`]).

use super::{Reader, Unreadable, Writer, unreadable};
use crate::results::{
    Found, Images, LARGEST_THUMBNAIL, MOST_FOUND, MOST_IMAGES, Pictured, Results, SetUp, Showing,
    Tab,
};
use alo_paint::picture::Format;

impl Writer {
    /// Whether searching is set up, and why not.
    pub(super) fn search_set_up(&mut self, search: &SetUp) {
        match search {
            SetUp::Ready => self.tag(0),
            SetUp::Not(why) => {
                self.tag(1);
                self.text(why);
            }
        }
    }

    /// A search's results: what the person said, then where the search is.
    pub(super) fn results(&mut self, results: &Results) {
        self.text(&results.said);
        match &results.showing {
            Showing::Searching => self.tag(0),
            Showing::Found(found) => {
                self.tag(1);
                self.number(found.len() as u64);
                for site in found {
                    self.text(&site.title);
                    self.text(&site.url.serialised);
                    self.text(&site.description);
                    self.text(&site.name);
                }
            }
            Showing::Unanswered(why) => {
                self.tag(2);
                self.text(why);
            }
        }
        self.images(&results.images);
        self.tag(match results.tab {
            Tab::All => 0,
            Tab::Sites => 1,
            Tab::Images => 2,
        });
    }

    /// Where a search for images is, and each image found.
    fn images(&mut self, images: &Images) {
        match images {
            Images::NotAsked => self.tag(0),
            Images::Asking => self.tag(1),
            Images::Found(found) => {
                self.tag(2);
                self.number(found.len() as u64);
                for image in found {
                    self.text(&image.title);
                    self.text(&image.page.serialised);
                    self.text(&image.source);
                    match &image.picture {
                        Some(bytes) => {
                            self.bool(true);
                            self.bytes(bytes);
                        }
                        None => self.bool(false),
                    }
                }
            }
            Images::Unanswered(why) => {
                self.tag(3);
                self.text(why);
            }
        }
    }
}

impl Reader<'_> {
    /// Whether searching is set up, as [`Writer::search_set_up`] wrote it.
    pub(super) fn search_set_up(&mut self) -> Result<SetUp, Unreadable> {
        Ok(match self.tag()? {
            0 => SetUp::Ready,
            1 => SetUp::Not(self.text()?),
            other => return Err(unreadable(format!("searching set up as {other}"))),
        })
    }

    /// A search's results, as [`Writer::results`] wrote them.
    pub(super) fn results(&mut self) -> Result<Results, Unreadable> {
        let said = self.text()?;
        let showing = match self.tag()? {
            0 => Showing::Searching,
            1 => {
                let count = self.count()?;
                if count > MOST_FOUND {
                    return Err(unreadable(format!(
                        "{count} sites found, and at most {MOST_FOUND} are shown"
                    )));
                }
                let mut found = Vec::with_capacity(count);
                for _ in 0..count {
                    let title = self.text()?;
                    let written = self.text()?;
                    let url = alo_url::parse(&written)
                        .ok()
                        .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
                        .ok_or_else(|| {
                            unreadable("a site found at an address that is not a site's")
                        })?;
                    let description = self.text()?;
                    let name = self.text()?;
                    found.push(Found {
                        title,
                        url,
                        description,
                        name,
                    });
                }
                Showing::Found(found)
            }
            2 => Showing::Unanswered(self.text()?),
            other => return Err(unreadable(format!("results tagged {other}"))),
        };
        let images = self.images()?;
        let tab = match self.tag()? {
            0 => Tab::All,
            1 => Tab::Sites,
            2 => Tab::Images,
            other => return Err(unreadable(format!("results shown on a tab tagged {other}"))),
        };
        let results = Results {
            said,
            showing,
            images,
            tab,
        };
        if !results.open(tab) {
            return Err(unreadable(format!(
                "results shown on {tab:?}, which they cannot be"
            )));
        }
        Ok(results)
    }

    /// Where a search for images is, as [`Writer::images`] wrote it.
    fn images(&mut self) -> Result<Images, Unreadable> {
        Ok(match self.tag()? {
            0 => Images::NotAsked,
            1 => Images::Asking,
            2 => {
                let count = self.count()?;
                if count > MOST_IMAGES {
                    return Err(unreadable(format!(
                        "{count} images found, and at most {MOST_IMAGES} are shown"
                    )));
                }
                let mut found = Vec::with_capacity(count);
                for _ in 0..count {
                    let title = self.text()?;
                    let written = self.text()?;
                    let page = alo_url::parse(&written)
                        .ok()
                        .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
                        .ok_or_else(|| {
                            unreadable("an image found on a page that is not a site's")
                        })?;
                    let source = self.text()?;
                    let picture = if self.bool()? {
                        let bytes = self.bytes()?;
                        if bytes.len() > LARGEST_THUMBNAIL {
                            return Err(unreadable(format!(
                                "a thumbnail of {} bytes, and at most {LARGEST_THUMBNAIL} are \
                                 drawn",
                                bytes.len()
                            )));
                        }
                        if Format::of(&bytes).is_none() {
                            return Err(unreadable("a thumbnail that is not a picture"));
                        }
                        Some(bytes)
                    } else {
                        None
                    };
                    found.push(Pictured {
                        title,
                        page,
                        source,
                        picture,
                    });
                }
                Images::Found(found)
            }
            3 => Images::Unanswered(self.text()?),
            other => return Err(unreadable(format!("images tagged {other}"))),
        })
    }
}

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
//! [`MOST_IMAGES`] are refused. Then where the searches for videos (745)
//! and news (746) are, by the same tags: each video's title, length, site,
//! age and description as text — a length refused unless it is one — its
//! page refused unless it is a site's, its picture as an image's is, and
//! more than [`MOST_VIDEOS`] refused; each story's the same, with no
//! picture, more than [`MOST_STORIES`] refused. Then the tab shown (744),
//! by a tag, refused unless it can be chosen ([`Results::open`]).

use super::{Reader, Unreadable, Writer, unreadable};
use crate::results::{
    Found, Images, LARGEST_THUMBNAIL, MOST_FOUND, MOST_IMAGES, Pictured, Results, SetUp, Showing,
    Stories, Tab, Videos,
};
use crate::results_media::{MOST_STORIES, MOST_VIDEOS, Story, Video, is_length};
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
        self.videos(&results.videos);
        self.stories(&results.news);
        self.tag(match results.tab {
            Tab::All => 0,
            Tab::Sites => 1,
            Tab::Images => 2,
            Tab::Videos => 3,
            Tab::News => 4,
        });
    }

    /// A picture's bytes, if one is held.
    fn picture(&mut self, picture: Option<&Vec<u8>>) {
        match picture {
            Some(bytes) => {
                self.bool(true);
                self.bytes(bytes);
            }
            None => self.bool(false),
        }
    }

    /// Where a search for videos is, and each video found (queue item 745).
    fn videos(&mut self, videos: &Videos) {
        match videos {
            Videos::NotAsked => self.tag(0),
            Videos::Asking => self.tag(1),
            Videos::Found(found) => {
                self.tag(2);
                self.number(found.len() as u64);
                for video in found {
                    self.text(&video.title);
                    self.text(&video.page.serialised);
                    self.text(&video.length);
                    self.text(&video.site);
                    self.text(&video.age);
                    self.text(&video.description);
                    self.picture(video.picture.as_ref());
                }
            }
            Videos::Unanswered(why) => {
                self.tag(3);
                self.text(why);
            }
        }
    }

    /// Where a search for news is, and each story found (queue item 746).
    fn stories(&mut self, news: &Stories) {
        match news {
            Stories::NotAsked => self.tag(0),
            Stories::Asking => self.tag(1),
            Stories::Found(found) => {
                self.tag(2);
                self.number(found.len() as u64);
                for story in found {
                    self.text(&story.title);
                    self.text(&story.page.serialised);
                    self.text(&story.site);
                    self.text(&story.age);
                    self.text(&story.description);
                }
            }
            Stories::Unanswered(why) => {
                self.tag(3);
                self.text(why);
            }
        }
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
                    self.picture(image.picture.as_ref());
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
        let videos = self.videos()?;
        let news = self.stories()?;
        let tab = match self.tag()? {
            0 => Tab::All,
            1 => Tab::Sites,
            2 => Tab::Images,
            3 => Tab::Videos,
            4 => Tab::News,
            other => return Err(unreadable(format!("results shown on a tab tagged {other}"))),
        };
        let results = Results {
            said,
            showing,
            images,
            tab,
            videos,
            news,
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
                    let picture = self.picture()?;
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

    /// A picture's bytes, if one was sent: refused larger than
    /// [`LARGEST_THUMBNAIL`] or unless they begin as a picture this engine
    /// reads.
    fn picture(&mut self) -> Result<Option<Vec<u8>>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let bytes = self.bytes()?;
        if bytes.len() > LARGEST_THUMBNAIL {
            return Err(unreadable(format!(
                "a thumbnail of {} bytes, and at most {LARGEST_THUMBNAIL} are drawn",
                bytes.len()
            )));
        }
        if Format::of(&bytes).is_none() {
            return Err(unreadable("a thumbnail that is not a picture"));
        }
        Ok(Some(bytes))
    }

    /// A page a video or story is on, refused unless it is a site's.
    fn page_of(&mut self, what: &str) -> Result<alo_url::Url, Unreadable> {
        let written = self.text()?;
        alo_url::parse(&written)
            .ok()
            .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
            .ok_or_else(|| unreadable(format!("{what} on a page that is not a site's")))
    }

    /// Where a search for videos is, as [`Writer::videos`] wrote it.
    fn videos(&mut self) -> Result<Videos, Unreadable> {
        Ok(match self.tag()? {
            0 => Videos::NotAsked,
            1 => Videos::Asking,
            2 => {
                let count = self.count()?;
                if count > MOST_VIDEOS {
                    return Err(unreadable(format!(
                        "{count} videos found, and at most {MOST_VIDEOS} are shown"
                    )));
                }
                let mut found = Vec::with_capacity(count);
                for _ in 0..count {
                    let title = self.text()?;
                    let page = self.page_of("a video found")?;
                    let length = self.text()?;
                    if !(length.is_empty() || is_length(&length)) {
                        return Err(unreadable("a video whose length is not a length"));
                    }
                    let site = self.text()?;
                    let age = self.text()?;
                    let description = self.text()?;
                    let picture = self.picture()?;
                    found.push(Video {
                        title,
                        page,
                        length,
                        site,
                        age,
                        description,
                        picture,
                    });
                }
                Videos::Found(found)
            }
            3 => Videos::Unanswered(self.text()?),
            other => return Err(unreadable(format!("videos tagged {other}"))),
        })
    }

    /// Where a search for news is, as [`Writer::stories`] wrote it.
    fn stories(&mut self) -> Result<Stories, Unreadable> {
        Ok(match self.tag()? {
            0 => Stories::NotAsked,
            1 => Stories::Asking,
            2 => {
                let count = self.count()?;
                if count > MOST_STORIES {
                    return Err(unreadable(format!(
                        "{count} news stories found, and at most {MOST_STORIES} are shown"
                    )));
                }
                let mut found = Vec::with_capacity(count);
                for _ in 0..count {
                    let title = self.text()?;
                    let page = self.page_of("a news story found")?;
                    let site = self.text()?;
                    let age = self.text()?;
                    let description = self.text()?;
                    found.push(Story {
                        title,
                        page,
                        site,
                        age,
                        description,
                    });
                }
                Stories::Found(found)
            }
            3 => Stories::Unanswered(self.text()?),
            other => return Err(unreadable(format!("news tagged {other}"))),
        })
    }
}

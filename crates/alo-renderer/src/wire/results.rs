/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's results on the wire (queue item 708, ADR 0083), sent with the
//! first screen's state ([`super::start`]).
//!
//! Read by a confined renderer as the rest of the first screen is: whether
//! searching is set up and why not, then whether there are results, what
//! the person said, and where the search is, by a closed list of tags.
//! Every site's title, name and description is text that stays text; its
//! address is read back with the URL parser and refused unless it is a
//! site's, so a renderer is never told to draw a link to anything else, and
//! more sites than [`MOST_FOUND`] are refused.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::results::{Found, MOST_FOUND, Results, SetUp, Showing};

impl Writer {
    /// Whether searching is set up, then the results, if there are any.
    pub(super) fn results(&mut self, search: &SetUp, results: Option<&Results>) {
        match search {
            SetUp::Ready => self.tag(0),
            SetUp::Not(why) => {
                self.tag(1);
                self.text(why);
            }
        }
        let Some(results) = results else {
            self.bool(false);
            return;
        };
        self.bool(true);
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
    }
}

impl Reader<'_> {
    /// Whether searching is set up, and the results, as
    /// [`Writer::results`] wrote them.
    pub(super) fn results(&mut self) -> Result<(SetUp, Option<Results>), Unreadable> {
        let search = match self.tag()? {
            0 => SetUp::Ready,
            1 => SetUp::Not(self.text()?),
            other => return Err(unreadable(format!("searching set up as {other}"))),
        };
        if !self.bool()? {
            return Ok((search, None));
        }
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
        Ok((search, Some(Results { said, showing })))
    }
}

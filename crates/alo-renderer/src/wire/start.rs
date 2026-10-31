/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The first screen's state on the wire (ADR 0074, queue item 700).
//!
//! Read by a confined renderer as the panel's is ([`super::panel`]): every
//! count checked against what is left, the size and the scale finite
//! numbers, and every name text that stays text — the renderer puts each
//! into its document as a text node ([`crate::start_document`]).
//!
//! What a person typed travels as text too (queue item 701), and the field
//! is given it as its value, an attribute set with `alo-dom`'s operations;
//! then whether the box has the keyboard, and each thing offered for what
//! was typed by a closed list of tags. An address offered is read back with
//! the URL parser, and one that is not a site's — no `http` or `https` —
//! is refused, so a renderer is never told to draw *Go to the site* for
//! anything else.
//!
//! Then whether searching is set up, and the conversation's turns, oldest
//! first (queue item 703), each by a closed list of tags, at most
//! [`MOST_TURNS`] of them: a search's results ([`super::results`]); the page
//! shown as a card (queue item 717), its address read back and refused
//! unless it is a site's, as an offer's is, its frame refused unless its
//! bytes begin as a PNG's — decoded, in this confined process, only by the
//! picture readers every `<img>` goes through — and refused anywhere but
//! as the newest turn, since one page is open at a time; a page shown
//! earlier, its address refused unless it is a site's; and a failure, in
//! words. A search's thumbnails together weigh at most
//! [`MOST_PICTURE_BYTES`] (queue item 743). Then how far the conversation
//! is scrolled, a finite number.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::card::Card;
use crate::conversation_scroll::MOST;
use crate::results::MOST_PICTURE_BYTES;
use crate::start::{KeptSite, Start, Waiting};
use crate::turn::{MOST_TURNS, Turn};
use crate::typed::Offer;
use alo_layout::geometry::Size;
use alo_url::Url;

/// What every PNG begins with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

impl Writer {
    /// The first screen: its size and scale, its greeting, then each site
    /// and each waiting chat in order, then what is typed, whether the box
    /// has the keyboard, and what is offered.
    pub(super) fn start(&mut self, start: &Start) {
        self.float(start.viewport.width);
        self.float(start.viewport.height);
        self.float(start.scale);
        self.text(&start.greeting);
        self.number(start.sites.len() as u64);
        for site in &start.sites {
            self.text(&site.name);
            self.text(&site.address);
        }
        self.number(start.waiting.len() as u64);
        for chat in &start.waiting {
            self.text(&chat.name);
            self.text(&chat.said);
        }
        self.text(&start.typed);
        self.bool(start.focused);
        self.number(start.offers.len() as u64);
        for offer in &start.offers {
            match offer {
                Offer::Go(url) => {
                    self.tag(0);
                    self.text(&url.serialised);
                }
                Offer::Search(said) => {
                    self.tag(1);
                    self.text(said);
                }
                Offer::Ask(said) => {
                    self.tag(2);
                    self.text(said);
                }
            }
        }
        self.search_set_up(&start.search);
        self.number(start.turns.len() as u64);
        for turn in &start.turns {
            match turn {
                Turn::Searched(results) => {
                    self.tag(0);
                    self.results(results);
                }
                Turn::Page(card) => {
                    self.tag(1);
                    self.card(card);
                }
                Turn::Visited { said, url } => {
                    self.tag(2);
                    self.text(said);
                    self.text(&url.serialised);
                }
                Turn::Failed { said, why } => {
                    self.tag(3);
                    self.text(said);
                    self.text(why);
                }
            }
        }
        self.float(start.down);
    }

    /// The page shown as a card (queue item 717): what the person said, the
    /// page's address, whether there is anywhere to go back or forward to,
    /// and the last frame Chrome drew of it, a PNG's bytes, if one has
    /// arrived.
    fn card(&mut self, card: &Card) {
        self.text(&card.said);
        self.text(&card.url.serialised);
        self.bool(card.back);
        self.bool(card.forward);
        match &card.frame {
            Some(png) => {
                self.bool(true);
                self.bytes(png);
            }
            None => self.bool(false),
        }
    }
}

impl Reader<'_> {
    /// The first screen, as [`Writer::start`] wrote it. A scale that is not
    /// a factor above zero is one (ADR 0057 § 3).
    pub(super) fn start(&mut self) -> Result<Start, Unreadable> {
        let viewport = Size {
            width: self.float()?,
            height: self.float()?,
        };
        let scale = alo_value::device_scale(self.float()?);
        let greeting = self.text()?;
        let mut sites = Vec::new();
        for _ in 0..self.count()? {
            let name = self.text()?;
            sites.push(KeptSite {
                name,
                address: self.text()?,
            });
        }
        let mut waiting = Vec::new();
        for _ in 0..self.count()? {
            let name = self.text()?;
            waiting.push(Waiting {
                name,
                said: self.text()?,
            });
        }
        let typed = self.text()?;
        let focused = self.bool()?;
        let mut offers = Vec::new();
        for _ in 0..self.count()? {
            offers.push(self.offer()?);
        }
        let search = self.search_set_up()?;
        let count = self.count()?;
        if count > MOST_TURNS {
            return Err(unreadable(format!(
                "a conversation of {count} turns, and at most {MOST_TURNS} are drawn"
            )));
        }
        let mut turns = Vec::with_capacity(count);
        for place in 0..count {
            let turn = match self.tag()? {
                0 => Turn::Searched(self.results()?),
                1 if place + 1 == count => Turn::Page(self.card()?),
                1 => {
                    return Err(unreadable(
                        "a page shown live in a turn that is not the newest",
                    ));
                }
                2 => {
                    let said = self.text()?;
                    Turn::Visited {
                        said,
                        url: self
                            .site("a page shown earlier at an address that is not a site's")?,
                    }
                }
                3 => {
                    let said = self.text()?;
                    Turn::Failed {
                        said,
                        why: self.text()?,
                    }
                }
                other => return Err(unreadable(format!("a turn tagged {other}"))),
            };
            turns.push(turn);
        }
        let pictures: usize = turns
            .iter()
            .map(|turn| match turn {
                Turn::Searched(results) => results.picture_bytes(),
                _ => 0,
            })
            .sum();
        if pictures > MOST_PICTURE_BYTES {
            return Err(unreadable(format!(
                "a first screen of {pictures} bytes of pictures, and at most \
                 {MOST_PICTURE_BYTES} are drawn"
            )));
        }
        let down = self.float()?;
        if down.abs() > MOST {
            return Err(unreadable(
                "a conversation scrolled further than any reaches",
            ));
        }
        Ok(Start {
            greeting,
            sites,
            waiting,
            viewport,
            scale,
            typed,
            focused,
            offers,
            turns,
            down,
            search,
        })
    }

    /// A card, as [`Writer::card`] wrote it. An address that is not a
    /// site's is refused, as one offered is, and so is a frame that is not
    /// a PNG: a renderer is told to draw only Chrome's pictures of a site
    /// there, and only in the one format Chrome is asked for (ADR 0076 B).
    fn card(&mut self) -> Result<Card, Unreadable> {
        let said = self.text()?;
        let url = self.site("a card for an address that is not a site's")?;
        let back = self.bool()?;
        let forward = self.bool()?;
        let frame = if self.bool()? {
            let png = self.bytes()?;
            if !png.starts_with(PNG_SIGNATURE) {
                return Err(unreadable("a card's frame that is not a PNG"));
            }
            Some(png)
        } else {
            None
        };
        Ok(Card {
            said,
            url,
            back,
            forward,
            frame,
        })
    }

    /// An address, read back with the URL parser, refused as `refused` says
    /// unless it is a site's.
    fn site(&mut self, refused: &str) -> Result<Url, Unreadable> {
        let written = self.text()?;
        alo_url::parse(&written)
            .ok()
            .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
            .ok_or_else(|| unreadable(refused))
    }

    /// One thing offered under the box: a tag nobody knows is refused, and
    /// so is an address to go to that is not a site's.
    fn offer(&mut self) -> Result<Offer, Unreadable> {
        Ok(match self.tag()? {
            0 => {
                let written = self.text()?;
                let url = alo_url::parse(&written)
                    .ok()
                    .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
                    .ok_or_else(|| unreadable("an address to go to that is not a site's"))?;
                Offer::Go(url)
            }
            1 => Offer::Search(self.text()?),
            2 => Offer::Ask(self.text()?),
            other => return Err(unreadable(format!("an offer tagged {other}"))),
        })
    }
}

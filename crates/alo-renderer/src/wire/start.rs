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
//! earlier, its address refused unless it is a site's, with the runs of
//! the agent kept with it (queue item 820); and a failure, in
//! words. A search's thumbnails together weigh at most
//! [`MOST_PICTURE_BYTES`] (queue item 743). Then how far the conversation
//! is scrolled, a finite number.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::agent_asked::Asked;
use crate::agent_limits::{Asks, Limits};
use crate::card::Card;
use crate::card_menu::{CardMenu, MenuItem};
use crate::conversation_scroll::{Bar, MOST};
use crate::results::MOST_PICTURE_BYTES;
use crate::settings_bus::Bus;
use crate::settings_privacy::{Banners, Cookies, Privacy, SignInsShown};
use crate::settings_services::{
    Entry, EntryRow, LONGEST_TYPED, MOST_DOTS, ServicePress, ServicesShown, Typed,
};
use crate::start::{KeptSite, SettingsKeyed, Start, Waiting};
use crate::turn::{MOST_TURNS, Turn};
use crate::typed::Offer;
use alo_layout::geometry::Size;
use alo_url::Url;

/// The most services a chooser of *Settings* is read with.
const MOST_SERVICES: usize = 8;

/// What every PNG begins with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

impl Writer {
    /// The first screen: its size and scale, its greeting, then each site
    /// and each waiting chat in order, then what is typed, whether the box
    /// has the keyboard, and what is offered; the face its words are set in
    /// and whether it is *Settings* (queue items 853, 854).
    pub(super) fn start(&mut self, start: &Start) {
        self.float(start.viewport.width);
        self.float(start.viewport.height);
        self.float(start.scale);
        self.face(start.face);
        self.settings_keyed(start.settings, start.settings_keyed);
        // How far alo's agent may go (queue item 878).
        self.number(u64::from(start.limits.sites));
        self.number(u64::from(start.limits.actions));
        // Which of the three choices of asking (queue item 884), by its
        // place among them.
        self.tag(match start.limits.asks {
            Asks::Never => 0,
            Asks::Unnamed => 1,
            Asks::Every => 2,
        });
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
        self.editing(&start.editing);
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
        self.search_set_up(&start.asking);
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
                Turn::Visited { said, url, ran } => {
                    self.tag(2);
                    self.text(said);
                    self.text(&url.serialised);
                    // Each run of the agent kept with it (queue item 820).
                    self.runs(ran);
                }
                Turn::Failed { said, why } => {
                    self.tag(3);
                    self.text(said);
                    self.text(why);
                }
                // alo asked with no page open (queue item 821).
                Turn::Asked(asked) => {
                    self.tag(4);
                    self.text(&asked.said);
                    self.agent_work(asked.agent.as_ref());
                    self.runs(&asked.ran);
                }
            }
        }
        self.float(start.down);
        // The sign of more above or below (queue item 755).
        self.bar(start.bar);
        // *Settings*' own scroll and sign, in its panel (queue item 886).
        self.float(start.settings_down);
        self.bar(start.settings_bar);
        // *Search and answers* (queue item 905): never a key, only dots.
        self.bool(start.services.is_some());
        if let Some(services) = &start.services {
            self.services(services);
        }
        self.privacy(start.privacy, start.sign_ins);
        self.bus(start.bus);
    }

    /// *Settings* and what of it the keyboard is on (queue item 856), in
    /// one byte: not it, it, its *Close* (886), its *Font*, and on — then,
    /// on something of *Search and answers*, which.
    fn settings_keyed(&mut self, settings: bool, keyed: Option<SettingsKeyed>) {
        self.tag(match (settings, keyed) {
            (false, _) => 0,
            (true, None) => 1,
            (true, Some(SettingsKeyed::Close)) => 2,
            (true, Some(SettingsKeyed::Font)) => 3,
            // *alo at work*'s three (queue item 878).
            (true, Some(SettingsKeyed::Sites)) => 4,
            (true, Some(SettingsKeyed::Actions)) => 5,
            (true, Some(SettingsKeyed::Asks)) => 6,
            // *Search and answers* (queue item 905), what of it after.
            (true, Some(SettingsKeyed::Service(_))) => 7,
            // *Privacy*'s two (queue item 920).
            (true, Some(SettingsKeyed::Cookies)) => 8,
            (true, Some(SettingsKeyed::Banners)) => 9,
            // *Forget all sign-ins* (ADR 0111).
            (true, Some(SettingsKeyed::ForgetSignIns)) => 10,
            // *Screen readers* (ADR 0115, queue item 989).
            (true, Some(SettingsKeyed::Bus)) => 11,
        });
        if let (true, Some(SettingsKeyed::Service(on))) = (settings, keyed) {
            self.service_press(on);
        }
    }

    /// *Screen readers* (queue item 989), if it is drawn: its choice by
    /// its place.
    fn bus(&mut self, bus: Option<Bus>) {
        self.bool(bus.is_some());
        if let Some(bus) = bus {
            self.tag(match bus {
                Bus::ReadAndAct => 0,
                Bus::ReadOnly => 1,
                Bus::Off => 2,
            });
        }
    }

    /// *Privacy* (queue item 920), if it is drawn: each choice by its
    /// place; then what its *Sign-ins* says (ADR 0111).
    fn privacy(&mut self, privacy: Option<Privacy>, sign_ins: SignInsShown) {
        self.bool(privacy.is_some());
        if let Some(privacy) = privacy {
            self.tag(match privacy.cookies {
                Cookies::RefuseOthers => 0,
                Cookies::RefuseAll => 1,
                Cookies::KeepAll => 2,
            });
            self.tag(match privacy.banners {
                Banners::Answer => 0,
                Banners::Leave => 1,
            });
        }
        // What *Sign-ins* says (ADR 0111), whether or not it is drawn.
        self.tag(match sign_ins {
            SignInsShown::Kept => 0,
            SignInsShown::Forgotten => 1,
            SignInsShown::ByAnother => 2,
            SignInsShown::NotKept => 3,
            SignInsShown::Unopened => 4,
        });
    }

    /// Something of *Search and answers*: its kind, then its place or its
    /// field.
    fn service_press(&mut self, on: ServicePress) {
        match on {
            ServicePress::Search(at) => {
                self.tag(0);
                self.number(at as u64);
            }
            ServicePress::Answers(at) => {
                self.tag(1);
                self.number(at as u64);
            }
            ServicePress::Field(entry) => {
                self.tag(2);
                self.entry(entry);
            }
            ServicePress::Keep(entry) => {
                self.tag(3);
                self.entry(entry);
            }
            ServicePress::Remove(entry) => {
                self.tag(4);
                self.entry(entry);
            }
        }
    }

    /// One of the section's fields, by a tag.
    fn entry(&mut self, entry: Entry) {
        self.tag(match entry {
            Entry::Address => 0,
            Entry::SearchKey => 1,
            Entry::AiKey => 2,
        });
    }

    /// The section: each chooser's names and the one chosen, each one's
    /// rows, and what was said.
    fn services(&mut self, services: &ServicesShown) {
        for (names, on, rows) in [
            (&services.search, services.search_on, &services.search_rows),
            (
                &services.answers,
                services.answers_on,
                &services.answers_rows,
            ),
        ] {
            self.number(names.len() as u64);
            for name in names {
                self.text(name);
            }
            self.number(on as u64);
            self.number(rows.len() as u64);
            for row in rows {
                self.entry(row.entry);
                self.text(&row.typed.shown);
                self.bool(row.typed.focused);
                self.text(&row.state);
                self.bool(row.removable);
            }
        }
        self.bool(services.said.is_some());
        if let Some(said) = &services.said {
            self.text(said);
        }
    }

    /// A sign of more above or below, if there is one.
    fn bar(&mut self, bar: Option<Bar>) {
        match bar {
            Some(bar) => {
                self.bool(true);
                self.float(bar.top);
                self.float(bar.length);
                self.bool(bar.above);
                self.bool(bar.below);
                self.float(bar.seen);
            }
            None => self.bool(false),
        }
    }

    /// The page shown as a card (queue item 717): what the person said, the
    /// page's address, whether there is anywhere to go back or forward to,
    /// whether it is still loading (queue item 751), and the last frame
    /// Chrome drew of it, a PNG's bytes, if one has arrived.
    fn card(&mut self, card: &Card) {
        self.text(&card.said);
        self.text(&card.url.serialised);
        self.bool(card.back);
        self.bool(card.forward);
        self.bool(card.loading);
        match &card.frame {
            Some(png) => {
                self.bool(true);
                self.bytes(png);
            }
            None => self.bool(false),
        }
        // Its picture drawn beneath the first screen (queue item 869).
        self.bool(card.picture_at == crate::card::PictureAt::Beneath);
        // The agent at work on its page (queue item 704).
        self.agent_work(card.agent.as_ref());
        // Its shield (queue item 807).
        self.bool(card.shield.is_some());
        if let Some(shield) = card.shield {
            self.number(u64::from(shield.blocked));
            self.bool(shield.on);
        }
        // What alo did with a cookie banner on its page (queue item 920).
        self.tag(match card.consent {
            None => 0,
            Some(crate::card::Consent::Refused) => 1,
            Some(crate::card::Consent::LeftToYou) => 2,
        });
        // alo's menu over its page (queue item 798).
        self.bool(card.menu.is_some());
        if let Some(menu) = &card.menu {
            self.number(u64::from(menu.at.0.unsigned_abs()));
            self.number(u64::from(menu.at.1.unsigned_abs()));
            self.number(menu.items.len() as u64);
            for item in &menu.items {
                match item {
                    MenuItem::Copy { selected } => {
                        self.tag(0);
                        self.bool(*selected);
                    }
                    MenuItem::CopyLink(link) => {
                        self.tag(1);
                        self.text(link);
                    }
                    MenuItem::OpenInNewChat(link) => {
                        self.tag(2);
                        self.text(link);
                    }
                    MenuItem::Search(words) => {
                        self.tag(3);
                        self.text(words);
                    }
                }
            }
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
        let face = self.face()?;
        let (settings, settings_keyed) = self.settings_keyed()?;
        let limits = self.limits()?;
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
        let editing = self.editing()?;
        let mut offers = Vec::new();
        for _ in 0..self.count()? {
            offers.push(self.offer()?);
        }
        let search = self.search_set_up()?;
        let asking = self.search_set_up()?;
        let count = self.count()?;
        if count > MOST_TURNS {
            return Err(unreadable(format!(
                "a conversation of {count} turns, and at most {MOST_TURNS} are drawn"
            )));
        }
        let mut turns = Vec::with_capacity(count);
        for place in 0..count {
            turns.push(self.turn(place + 1 == count)?);
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
        let bar = self.bar()?;
        let settings_down = self.float()?;
        if settings_down.abs() > MOST {
            return Err(unreadable("Settings scrolled further than any reaches"));
        }
        let settings_bar = self.bar()?;
        let services = self.services_drawn(settings_keyed)?;
        let (privacy, sign_ins) = self.privacy_drawn(settings_keyed)?;
        let bus = self.bus_drawn(settings_keyed)?;
        Ok(Start {
            greeting,
            sites,
            waiting,
            viewport,
            scale,
            typed,
            focused,
            editing,
            offers,
            turns,
            down,
            bar,
            search,
            asking,
            face,
            settings,
            settings_keyed,
            limits,
            settings_down,
            settings_bar,
            services,
            privacy,
            sign_ins,
            bus,
        })
    }

    /// *Screen readers*, if it is drawn — refused if the keyboard is on
    /// it as `keyed` says and it is not (queue item 989).
    fn bus_drawn(&mut self, keyed: Option<SettingsKeyed>) -> Result<Option<Bus>, Unreadable> {
        let bus = if self.bool()? {
            Some(match self.tag()? {
                0 => Bus::ReadAndAct,
                1 => Bus::ReadOnly,
                2 => Bus::Off,
                other => {
                    return Err(unreadable(format!(
                        "a choice of the accessibility bus tagged {other}"
                    )));
                }
            })
        } else {
            None
        };
        if bus.is_none() && keyed == Some(SettingsKeyed::Bus) {
            return Err(unreadable(
                "the keyboard on a section of Settings not drawn",
            ));
        }
        Ok(bus)
    }

    /// *Privacy*, if it is drawn — refused if the keyboard is on it as
    /// `keyed` says and it is not (queue item 920) — and what its
    /// *Sign-ins* says (ADR 0111).
    fn privacy_drawn(
        &mut self,
        keyed: Option<SettingsKeyed>,
    ) -> Result<(Option<Privacy>, SignInsShown), Unreadable> {
        let privacy = if self.bool()? {
            let cookies = match self.tag()? {
                0 => Cookies::RefuseOthers,
                1 => Cookies::RefuseAll,
                2 => Cookies::KeepAll,
                other => return Err(unreadable(format!("a choice of cookies tagged {other}"))),
            };
            let banners = match self.tag()? {
                0 => Banners::Answer,
                1 => Banners::Leave,
                other => return Err(unreadable(format!("a choice of banners tagged {other}"))),
            };
            Some(Privacy { cookies, banners })
        } else {
            None
        };
        if privacy.is_none()
            && matches!(
                keyed,
                Some(
                    SettingsKeyed::Cookies | SettingsKeyed::Banners | SettingsKeyed::ForgetSignIns
                )
            )
        {
            return Err(unreadable(
                "the keyboard on a section of Settings not drawn",
            ));
        }
        // What *Sign-ins* says (ADR 0111).
        let sign_ins = match self.tag()? {
            0 => SignInsShown::Kept,
            1 => SignInsShown::Forgotten,
            2 => SignInsShown::ByAnother,
            3 => SignInsShown::NotKept,
            4 => SignInsShown::Unopened,
            other => return Err(unreadable(format!("what of sign-ins tagged {other}"))),
        };
        Ok((privacy, sign_ins))
    }

    /// Whether it is *Settings*, and what of it the keyboard is on, as
    /// [`Writer::start`] wrote them.
    fn settings_keyed(&mut self) -> Result<(bool, Option<SettingsKeyed>), Unreadable> {
        Ok(match self.tag()? {
            0 => (false, None),
            1 => (true, None),
            2 => (true, Some(SettingsKeyed::Close)),
            3 => (true, Some(SettingsKeyed::Font)),
            4 => (true, Some(SettingsKeyed::Sites)),
            5 => (true, Some(SettingsKeyed::Actions)),
            6 => (true, Some(SettingsKeyed::Asks)),
            7 => (true, Some(SettingsKeyed::Service(self.service_press()?))),
            8 => (true, Some(SettingsKeyed::Cookies)),
            9 => (true, Some(SettingsKeyed::Banners)),
            10 => (true, Some(SettingsKeyed::ForgetSignIns)),
            11 => (true, Some(SettingsKeyed::Bus)),
            other => return Err(unreadable(format!("a Settings tagged {other}"))),
        })
    }

    /// *Search and answers*, if it is drawn — refused if the keyboard is on
    /// it as `keyed` says and it is not.
    fn services_drawn(
        &mut self,
        keyed: Option<SettingsKeyed>,
    ) -> Result<Option<ServicesShown>, Unreadable> {
        let services = if self.bool()? {
            Some(self.services()?)
        } else {
            None
        };
        if services.is_none() && matches!(keyed, Some(SettingsKeyed::Service(_))) {
            return Err(unreadable(
                "the keyboard on a section of Settings not drawn",
            ));
        }
        Ok(services)
    }

    /// Something of *Search and answers*, as [`Writer::start`] wrote it.
    fn service_press(&mut self) -> Result<ServicePress, Unreadable> {
        Ok(match self.tag()? {
            0 => ServicePress::Search(self.service_place()?),
            1 => ServicePress::Answers(self.service_place()?),
            2 => ServicePress::Field(self.entry()?),
            3 => ServicePress::Keep(self.entry()?),
            4 => ServicePress::Remove(self.entry()?),
            other => return Err(unreadable(format!("a press of Settings tagged {other}"))),
        })
    }

    /// A service's place among those offered: never past the most offered.
    fn service_place(&mut self) -> Result<usize, Unreadable> {
        usize::try_from(self.number()?)
            .ok()
            .filter(|at| *at < MOST_SERVICES)
            .ok_or_else(|| unreadable("a service past the most offered"))
    }

    /// One of the section's fields, by its tag.
    fn entry(&mut self) -> Result<Entry, Unreadable> {
        Ok(match self.tag()? {
            0 => Entry::Address,
            1 => Entry::SearchKey,
            2 => Entry::AiKey,
            other => return Err(unreadable(format!("a field of Settings tagged {other}"))),
        })
    }

    /// The section, as [`Writer::start`] wrote it: at most
    /// [`MOST_SERVICES`] of each and two rows, the one chosen among them, a
    /// field's words no longer than any is, and a key's never more than its
    /// dots.
    fn services(&mut self) -> Result<ServicesShown, Unreadable> {
        let mut shown = ServicesShown::default();
        for answers in [false, true] {
            let count = self.count()?;
            if count == 0 || count > MOST_SERVICES {
                return Err(unreadable(format!("a chooser of {count} services")));
            }
            let mut names = Vec::with_capacity(count);
            for _ in 0..count {
                names.push(self.text()?);
            }
            let on = usize::try_from(self.number()?).unwrap_or(usize::MAX);
            if on >= count {
                return Err(unreadable("a service chosen that is not offered"));
            }
            let rows = self.count()?;
            if rows > 2 {
                return Err(unreadable(format!("a service of {rows} rows")));
            }
            let mut read = Vec::with_capacity(rows);
            for _ in 0..rows {
                let entry = self.entry()?;
                let shown = self.text()?;
                let most = if entry.masked() {
                    MOST_DOTS
                } else {
                    LONGEST_TYPED
                };
                if shown.chars().count() > most
                    || (entry.masked() && shown.chars().any(|c| c != '\u{2022}'))
                {
                    return Err(unreadable("a field holding more than it can"));
                }
                read.push(EntryRow {
                    entry,
                    typed: Typed {
                        shown,
                        focused: self.bool()?,
                    },
                    state: self.text()?,
                    removable: self.bool()?,
                });
            }
            if answers {
                (shown.answers, shown.answers_on, shown.answers_rows) = (names, on, read);
            } else {
                (shown.search, shown.search_on, shown.search_rows) = (names, on, read);
            }
        }
        shown.said = if self.bool()? {
            Some(self.text()?)
        } else {
            None
        };
        Ok(shown)
    }

    /// One turn of the conversation, as [`Writer::start`] wrote it: a live
    /// card only as the `newest`.
    fn turn(&mut self, newest: bool) -> Result<Turn, Unreadable> {
        Ok(match self.tag()? {
            0 => Turn::Searched(self.results()?),
            1 if newest => Turn::Page(self.card()?),
            1 => {
                return Err(unreadable(
                    "a page shown live in a turn that is not the newest",
                ));
            }
            2 => {
                let said = self.text()?;
                let url = self.site("a page shown earlier at an address that is not a site's")?;
                Turn::Visited {
                    said,
                    url,
                    ran: self.runs()?,
                }
            }
            3 => {
                let said = self.text()?;
                Turn::Failed {
                    said,
                    why: self.text()?,
                }
            }
            4 => {
                let said = self.text()?;
                let agent = self.agent_work()?;
                if agent.is_some() && !newest {
                    return Err(unreadable(
                        "alo at work with no page in a turn that is not the newest",
                    ));
                }
                let ran = self.runs()?;
                if ran.len() > 1 {
                    return Err(unreadable(
                        "a turn asked with no page keeping more than its one run",
                    ));
                }
                Turn::Asked(Asked { said, agent, ran })
            }
            other => return Err(unreadable(format!("a turn tagged {other}"))),
        })
    }

    /// How far alo's agent may go (queue item 878), as [`Writer::start`]
    /// wrote it: each within its bounds, or refused.
    fn limits(&mut self) -> Result<Limits, Unreadable> {
        let sites = u8::try_from(self.number()?).ok();
        let actions = u16::try_from(self.number()?).ok();
        let asks = match self.tag()? {
            0 => Asks::Never,
            1 => Asks::Unnamed,
            2 => Asks::Every,
            other => return Err(unreadable(format!("a choice of asking tagged {other}"))),
        };
        sites
            .zip(actions)
            .and_then(|(sites, actions)| Limits::checked(sites, actions, asks))
            .ok_or_else(|| unreadable("an agent's limits past their bounds"))
    }

    /// The sign of more above or below (queue item 755), if one was
    /// written: placed no further than any room.
    fn bar(&mut self) -> Result<Option<Bar>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let top = self.float()?;
        let length = self.float()?;
        if !(0.0..=MOST).contains(&top) || !(0.0..=MOST).contains(&length) {
            return Err(unreadable(
                "a sign of more above or below placed further than any room",
            ));
        }
        let above = self.bool()?;
        let below = self.bool()?;
        // How much of it is seen (queue item 793): a fraction, and nothing
        // else.
        let seen = self.float()?;
        if !(0.0..=1.0).contains(&seen) {
            return Err(unreadable("a sign of more seen as other than a fraction"));
        }
        Ok(Some(Bar {
            top,
            length,
            above,
            below,
            seen,
        }))
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
        let loading = self.bool()?;
        let frame = if self.bool()? {
            let png = self.bytes()?;
            if !png.starts_with(PNG_SIGNATURE) {
                return Err(unreadable("a card's frame that is not a PNG"));
            }
            Some(png)
        } else {
            None
        };
        let picture_at = if self.bool()? {
            crate::card::PictureAt::Beneath
        } else {
            crate::card::PictureAt::InTheCard
        };
        let agent = self.agent_work()?;
        let shield = if self.bool()? {
            let blocked = u32::try_from(self.number()?)
                .map_err(|_| unreadable("a shield's count past what one holds"))?;
            Some(crate::card::Shield {
                blocked,
                on: self.bool()?,
            })
        } else {
            None
        };
        let consent = match self.tag()? {
            0 => None,
            1 => Some(crate::card::Consent::Refused),
            2 => Some(crate::card::Consent::LeftToYou),
            other => {
                return Err(unreadable(format!("a card's cookie banner tagged {other}")));
            }
        };
        let menu = if self.bool()? {
            Some(self.card_menu()?)
        } else {
            None
        };
        Ok(Card {
            said,
            url,
            back,
            forward,
            frame,
            picture_at,
            loading,
            agent,
            // Never sent with the live card: drawn once it is shown earlier.
            ran: Vec::new(),
            shield,
            consent,
            menu,
        })
    }

    /// alo's menu over a card's page, as [`Writer::card`] wrote it: placed
    /// inside a page box no card is past, with at most four lines, each one
    /// of the four kinds.
    fn card_menu(&mut self) -> Result<CardMenu, Unreadable> {
        let mut place = || {
            u16::try_from(self.number()?)
                .map(i32::from)
                .map_err(|_| unreadable("a menu placed past any page box"))
        };
        let at = (place()?, place()?);
        let lines = self.number()?;
        if lines > 4 {
            return Err(unreadable("a menu of more than four lines"));
        }
        let mut items = Vec::new();
        for _ in 0..lines {
            items.push(match self.tag()? {
                0 => MenuItem::Copy {
                    selected: self.bool()?,
                },
                1 => MenuItem::CopyLink(self.text()?),
                2 => MenuItem::OpenInNewChat(self.text()?),
                3 => MenuItem::Search(self.text()?),
                other => return Err(unreadable(format!("a menu's line tagged {other}"))),
            });
        }
        Ok(CardMenu { at, items })
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

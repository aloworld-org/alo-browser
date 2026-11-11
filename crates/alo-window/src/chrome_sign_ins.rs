/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card's Chromium given the sign-ins kept, and its own taken back (ADR
//! 0111 § 2, queue item 935).
//!
//! Every Chromium alo drives still starts on a new, empty profile of its
//! own, removed with it (ADR 0069): nothing of a person's is ever in a
//! profile that outlives it. What carries a sign-in from one card to the
//! next, and across alo's starts, is the browser process's own jar
//! ([`SignIns`]):
//!
//! - **As the card's Chromium starts**, before it opens any page, the jar's
//!   cookies are put into it with `Storage.setCookies` on the browser's
//!   own target ([`given`]) — then read back, so what it took is what is
//!   seen.
//! - **While it runs**, every [`EVERY`], and once more as it closes, its
//!   cookies are read with `Storage.getCookies` and what changed since they
//!   were last seen — set, changed, removed by a site signing a person out
//!   — is made to the jar ([`Keeping::look`]).
//! - **Every sign-in forgotten** meanwhile (*Settings › Privacy*, ADR 0111
//!   § 5), the next look empties the Chromium's own jar with
//!   `Storage.clearCookies` instead, and takes nothing from it.
//!
//! What a Chromium keeps, and what it sends, stays its own to decide by
//! the choice it was started with (ADR 0108 § 1): a cookie put back into a
//! Chromium refusing every site's is held and never sent.
//!
//! Measured, 11 November 2026, against Chrome 157 and CEF 156.0.14, by
//! `tests/sign_ins_kept_through_chrome.rs`: the three commands answer on
//! the browser's target of both.

use crate::chrome::{Chrome, Failed};
use crate::chrome_protocol::{self, Command};
use crate::chrome_tab::Tab;
use crate::kept_cookie::{Changes, KeptCookie};
use crate::sign_ins::SignIns;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often a card's Chromium's cookies are looked at while it runs: a
/// sign-in made in it is kept within this, should alo be killed.
pub const EVERY: Duration = Duration::from_secs(2);

/// A card's Chromium, keeping its sign-ins in a jar.
#[derive(Debug)]
pub struct Keeping {
    jar: Arc<SignIns>,
    /// What its Chromium was last seen holding.
    seen: Vec<KeptCookie>,
    /// How many times every sign-in had been forgotten when it was.
    forgotten: u64,
    /// When it is next looked at.
    next: Instant,
}

/// Every cookie `chrome` holds.
fn held(chrome: &mut Chrome) -> Result<Vec<KeptCookie>, Failed> {
    chrome_protocol::cookies(&chrome.ask(None, &Command::ReadCookies)?).map_err(Failed::Malformed)
}

/// Put `jar`'s cookies into `chrome`, which has opened no page yet: from
/// now on, what it changes is made to the jar.
///
/// # Errors
///
/// What `chrome` failed with; the card opens all the same, signed in to
/// nothing, and the jar is left as it was.
pub fn given(chrome: &mut Chrome, jar: Arc<SignIns>) -> Result<Keeping, Failed> {
    let forgotten = jar.forgotten();
    let cookies = jar.cookies();
    if !cookies.is_empty() {
        chrome.ask(None, &Command::PutCookies { cookies })?;
    }
    let seen = held(chrome)?;
    Ok(Keeping {
        jar,
        seen,
        forgotten,
        next: Instant::now() + EVERY,
    })
}

impl Keeping {
    /// When it is next to be looked at.
    pub fn next(&self) -> Instant {
        self.next
    }

    /// Look at its Chromium's cookies through `tab` now, if it is time.
    ///
    /// # Errors
    ///
    /// As [`Keeping::look`].
    pub fn look_if_due(&mut self, tab: &mut Tab<'_>) -> Result<(), String> {
        if Instant::now() < self.next {
            return Ok(());
        }
        self.look(tab)
    }

    /// Look at its Chromium's cookies through `tab`, and make what changed
    /// since they were last seen to the jar — or, when every sign-in was
    /// forgotten since, empty its Chromium's jar instead.
    ///
    /// # Errors
    ///
    /// Why the cookies were not read or not kept, in words.
    pub fn look(&mut self, tab: &mut Tab<'_>) -> Result<(), String> {
        self.next = Instant::now() + EVERY;
        let forgotten = self.jar.forgotten();
        if forgotten != self.forgotten {
            tab.ask_browser(&Command::ClearCookies)
                .map_err(|why| format!("the card's cookies were not forgotten: {why}"))?;
            self.forgotten = forgotten;
            self.seen.clear();
            return Ok(());
        }
        let now = tab
            .ask_browser(&Command::ReadCookies)
            .and_then(|read| chrome_protocol::cookies(&read).map_err(Failed::Malformed))
            .map_err(|why| format!("the card's cookies were not read: {why}"))?;
        let changes = Changes::between(&self.seen, &now);
        self.seen = now;
        self.jar.change_as_of(&changes, self.forgotten)
    }
}

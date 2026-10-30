/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tabs an agent outside this browser reaches, whichever renderer draws
//! them (ADR 0058 §§ 2 and 4, queue item 630).
//!
//! [`crate::serving`] answers an agent — the address read, the grant judged,
//! the tab found, the request recorded — and none of that is a renderer's
//! business. What it asks of the tabs is only this: which are open and at
//! what address, which renderer draws each, which document each holds, and
//! a tree read or a verb carried out. So the same serving, and the same
//! grants in the same order, stand in front of this engine's tabs
//! ([`crate::engine_pages`]) and a tab in Chrome
//! ([`crate::chrome_pages`]).
//!
//! # The renderer is named
//!
//! ADR 0058 § 6: *the product says what is rendering*. Every entry of the
//! agent record that reached a tab says which renderer drew the page it
//! read or acted on ([`RenderedBy`]), so that the record of one order run
//! through this engine and through Chrome differs in exactly that.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_net::cause::{ActionId, DocumentId};
use alo_renderer::Snapshot;
use alo_url::Url;
use core::fmt;

/// Which renderer drew a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderedBy {
    /// This browser's own engine, in a renderer process of its own.
    ThisEngine,
    /// The Chrome installed on this machine, driven over a pipe (ADR 0058).
    Chrome {
        /// Its product and version, as it says itself:
        /// `Chrome/155.0.8059.86`.
        product: String,
    },
    /// The Chromium the application carries inside it, driven over a pipe
    /// as the machine's Chrome is — the only one the application ever
    /// starts (ADR 0077).
    Carried {
        /// Its product and version, as it says itself: a Chromium names
        /// itself as Chrome does, `Chrome/156.0.8078.76`.
        product: String,
    },
}

impl fmt::Display for RenderedBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThisEngine => f.write_str("this browser's engine"),
            Self::Chrome { product } => write!(f, "{product}, driven by this browser"),
            Self::Carried { product } => {
                write!(
                    f,
                    "{product}, the Chromium alo carries, driven by this browser"
                )
            }
        }
    }
}

/// What a verb a page was given came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Acted {
    /// It ran, and this is what it changed.
    Done {
        /// What it did, as the page left it.
        outcome: Outcome,
        /// What the page's script said while it ran, as the renderer said
        /// it.
        issues: Vec<String>,
    },
    /// The page refused it, with one of `verb.rs`'s refusals.
    Refused(Refusal),
}

/// Why a tab could not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unanswered {
    /// Its renderer is gone, or could not answer; in words, for a person.
    Gone(String),
    /// The verb was decided and its renderer did not carry it out, so
    /// nothing was done; in words, for a person.
    NotCarriedOut(String),
}

/// The open tabs an agent may be answered from.
pub trait Pages {
    /// How one of these tabs is told apart from the others, by this process
    /// and never by an agent (ADR 0055 § 5).
    type Tab: Copy + PartialEq + fmt::Debug;

    /// Take in everything the renderers have said since they were last
    /// asked, waiting for nothing — so that a tab is named by where it is
    /// now. A renderer whose page goes elsewhere only as it answers has
    /// nothing to take in; one that tells of a page going elsewhere on its
    /// own — a script, a refresh — whether or not it is being asked
    /// anything (Chrome) does. A renderer that cannot be heard from is found
    /// out when it is next asked something.
    fn catch_up(&mut self);

    /// Every open tab, in order, with the address it is named by: its
    /// page's, or where it was opened if nothing has loaded. A tab whose
    /// address cannot be read as one is not here, and so cannot be named.
    fn shown(&self) -> Vec<(Self::Tab, Url)>;

    /// The renderer that draws `tab`.
    fn rendered_by(&self, tab: Self::Tab) -> RenderedBy;

    /// The document `tab` holds, or [`None`] when nothing is loaded in it.
    fn document(&self, tab: Self::Tab) -> Option<DocumentId>;

    /// The page in `tab`, read as the tree an agent reads.
    ///
    /// # Errors
    ///
    /// [`Unanswered`], when it could not be read.
    fn read(&mut self, tab: Self::Tab) -> Result<Box<Snapshot>, Unanswered>;

    /// Do `verb` to what `target` names in the page in `tab`, accepted as
    /// the action it answers with — whatever the page makes of it (ADR 0012
    /// § 4).
    ///
    /// # Errors
    ///
    /// [`Unanswered`], when the tab could not be asked, or did not carry
    /// the verb out.
    fn act(
        &mut self,
        tab: Self::Tab,
        target: &Target,
        verb: &Verb,
    ) -> Result<(ActionId, Acted), Unanswered>;
}

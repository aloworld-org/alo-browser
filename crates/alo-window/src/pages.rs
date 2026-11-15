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

impl RenderedBy {
    /// Which renderer draws the page, in the plain words the frame around
    /// Chromium's window draws (queue item 991): the product's name and its
    /// major version, and where it is from. The exact wording — this type's
    /// [`fmt::Display`], the product and version as it gives them itself —
    /// is what agents, the socket's `renderer` answer and the terminal are
    /// told, and it is unchanged.
    pub fn plainly(&self) -> String {
        match self {
            Self::ThisEngine => "Page drawn by alo's own engine".to_owned(),
            Self::Chrome { product } => {
                format!(
                    "Page drawn by {}, installed on this machine",
                    named(product, "Chrome")
                )
            }
            Self::Carried { product } => {
                format!(
                    "Page drawn by {}, carried inside alo",
                    named(product, "Chromium")
                )
            }
        }
    }
}

/// `product` as `name` and its major version — `Chrome/156.0.8078.48` is
/// *Chromium 156* when `name` is Chromium — or `name` alone when the product
/// gave no version it can be read from.
fn named(product: &str, name: &str) -> String {
    let major = product
        .split_once('/')
        .and_then(|(_, version)| version.split('.').next())
        .filter(|major| !major.is_empty() && major.bytes().all(|byte| byte.is_ascii_digit()));
    match major {
        Some(major) => format!("{name} {major}"),
        None => name.to_owned(),
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
    /// Text was to be put into a password field of a page shown in the
    /// conversation, which no agent types into (queue item 704, ADR 0096):
    /// the field, and nothing was typed.
    Password(alo_box::BoxId),
    /// Text was to be put into a field of a card's details — its number,
    /// security code or expiry — or one of its lists chosen in, on a page
    /// shown in the conversation, which no agent does (queue item 826, ADR
    /// 0099): the field, and nothing was done.
    CardDetails(alo_box::BoxId),
    /// The renderer drawing the tab does not do what was asked of it —
    /// this engine's tabs neither go to an address an agent names nor
    /// save a page as a PDF (queue item 956) — and nothing was done.
    NotOffered,
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

    /// Send the page in `tab` to `address`, an `http` or `https` one an
    /// agent named and may reach, as the action it answers with, and wait
    /// for it to arrive (queue item 956). By default not done:
    /// [`Unanswered::NotOffered`].
    ///
    /// # Errors
    ///
    /// [`Unanswered`], when the tab could not be asked or did not go.
    fn go(&mut self, tab: Self::Tab, address: &Url) -> Result<ActionId, Unanswered> {
        let _ = (tab, address);
        Err(Unanswered::NotOffered)
    }

    /// The page in `tab` printed as a PDF, its bytes as the renderer gave
    /// them (queue item 956). By default not done:
    /// [`Unanswered::NotOffered`].
    ///
    /// # Errors
    ///
    /// [`Unanswered`], when the tab could not be asked or did not print.
    fn print_pdf(&mut self, tab: Self::Tab) -> Result<Vec<u8>, Unanswered> {
        let _ = tab;
        Err(Unanswered::NotOffered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_says_which_renderer_plainly_and_agents_are_told_exactly() {
        let carried = RenderedBy::Carried {
            product: "Chrome/156.0.8078.48".to_owned(),
        };
        assert_eq!(
            carried.plainly(),
            "Page drawn by Chromium 156, carried inside alo"
        );
        assert_eq!(
            carried.to_string(),
            "Chrome/156.0.8078.48, the Chromium alo carries, driven by this browser"
        );
        let chrome = RenderedBy::Chrome {
            product: "Chrome/155.0.8059.86".to_owned(),
        };
        assert_eq!(
            chrome.plainly(),
            "Page drawn by Chrome 155, installed on this machine"
        );
        assert_eq!(
            RenderedBy::ThisEngine.plainly(),
            "Page drawn by alo's own engine"
        );
    }

    #[test]
    fn a_product_with_no_version_is_named_alone() {
        for product in ["", "Chrome", "Chrome/", "Chrome/x.1", "HeadlessChrome/ 1"] {
            let carried = RenderedBy::Carried {
                product: product.to_owned(),
            };
            assert_eq!(
                carried.plainly(),
                "Page drawn by Chromium, carried inside alo",
                "{product:?}"
            );
        }
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Words chosen from the conversation's one box, searched (queue item 708,
//! ADR 0083): a thread of the browser process that asks the search
//! provider and tells the window what came back.
//!
//! Return or *Go* on words that are not an address chooses *Search for
//! "…"* ([`crate::conversation::Conversation::searching`]). The window
//! hands the words to this thread ([`Searcher::search`]), which returns at
//! once — the window's event loop calls nothing that waits (ADR 0024 § 2)
//! — and the thread asks the provider ([`crate::brave_search`]) through
//! this repository's own HTTP stack, `alo-net`, and tells the window
//! [`Searched`] as an event like any other. Never a renderer: a renderer
//! has no network (ADR 0005), and is sent only the results, as data.
//!
//! # Not set up
//!
//! With no key ([`crate::search_key`]) there is no thread: the searcher is
//! [`Searcher::set_up`] *not*, with why, which the box's offer says, and a
//! search handed to it is told unanswered with the same words.
//!
//! # Newest wins
//!
//! Each search is numbered by the window. Searches waiting when the thread
//! is free are not all asked: only the newest is, so a person who searched
//! twice quickly is not made to wait for, nor pay for, the first.
//!
//! # Images (queue item 743, ADR 0086)
//!
//! When a web search finds sites and the provider asks images, the window
//! is told the sites at once, with the images being asked ([`Came::Sites`]);
//! then the same words alone are asked of the image search, each
//! thumbnail fetched from the provider's own proxy
//! ([`crate::search_thumbnails`]), and the window is told the images
//! ([`Came::Images`]). A search that found no sites, or was not answered,
//! asks no images: a second paid request for a search that drew nothing is
//! not made. If a newer search is waiting once the sites are told, its
//! images are not asked, and the window is told so in words.
//!
//! # Connections kept
//!
//! The thread keeps one `alo-net` pool, trusting what this machine trusts,
//! for as long as the window is open, so a second search reuses the first
//! one's connection. Nothing of it is written to a disk.

use crate::brave_search::{self, Provider};
use crate::search_thumbnails;
use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::results::{Images, PROVIDER, SetUp, Showing};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the provider is waited for before a search is said not
/// answered.
pub const PATIENCE: Duration = Duration::from_secs(15);

/// What came of a search, as the window is told it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Searched {
    /// The window's number for the search.
    pub number: u64,
    /// The words searched for.
    pub words: String,
    /// What came.
    pub came: Came,
}

/// What came of a search: first its sites, then, when they are asked, its
/// images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Came {
    /// The sites found, or why not; and where the search for images starts:
    /// [`Images::Asking`] when they will follow, [`Images::NotAsked`] when
    /// they will not.
    Sites {
        /// Sites found, or why not.
        showing: Showing,
        /// Where the search for images starts.
        images: Images,
    },
    /// The images found, each holding its thumbnail if one was kept, or why
    /// not.
    Images(Images),
}

impl Searched {
    /// The sites a search came to, with no images to follow.
    pub fn sites(number: u64, words: impl Into<String>, showing: Showing) -> Self {
        Self {
            number,
            words: words.into(),
            came: Came::Sites {
                showing,
                images: Images::NotAsked,
            },
        }
    }
}

/// How the thread tells the window what came of a search: whether anybody
/// heard.
pub type TellSearched = Box<dyn Fn(Searched) -> bool + Send>;

/// What the window hands the thread.
enum Work {
    /// Search for these words, numbered so.
    Search(u64, String),
    /// The window is closing: end.
    Finish,
}

/// A thread asking the search provider for what was chosen in the
/// conversation, one search at a time.
pub struct Searcher {
    /// Whether searching can be done.
    set_up: SetUp,
    /// Where work is sent; [`None`] with no provider, or once finished.
    asked: Option<Sender<Work>>,
    thread: Option<JoinHandle<()>>,
    /// How the window is told, while there is no thread to tell it.
    tell: Option<TellSearched>,
}

impl core::fmt::Debug for Searcher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Searcher")
            .field("set_up", &self.set_up)
            .finish_non_exhaustive()
    }
}

impl Searcher {
    /// Start a thread asking `provider` and telling the window with `tell`;
    /// or, with no provider and why, none, every search told unanswered
    /// with why.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(provider: Result<Provider, String>, tell: TellSearched) -> std::io::Result<Self> {
        let provider = match provider {
            Ok(provider) => provider,
            Err(why) => {
                return Ok(Self {
                    set_up: SetUp::Not(why),
                    asked: None,
                    thread: None,
                    tell: Some(tell),
                });
            }
        };
        let (asked, asking) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-conversation-search".to_owned())
            .spawn(move || ask(&provider, &asking, &tell))?;
        Ok(Self {
            set_up: SetUp::Ready,
            asked: Some(asked),
            thread: Some(thread),
            tell: None,
        })
    }

    /// Whether searching can be done, and why not.
    pub fn set_up(&self) -> &SetUp {
        &self.set_up
    }

    /// Search for `words`, numbered `number`. Returns at once: what came of
    /// it is told to the window.
    pub fn search(&self, number: u64, words: &str) {
        match (&self.asked, &self.set_up, &self.tell) {
            (Some(asked), _, _) => {
                // A thread that has gone is a window that is closing.
                let _ = asked.send(Work::Search(number, words.to_owned()));
            }
            (None, SetUp::Not(why), Some(tell)) => {
                let _ = tell(Searched::sites(
                    number,
                    words,
                    Showing::Unanswered(not_set_up(why)),
                ));
            }
            _ => {}
        }
    }

    /// Stop the thread, after the search it is asking, and wait for it:
    /// whether it ended rather than panicked.
    pub fn finish(mut self) -> bool {
        if let Some(asked) = self.asked.take() {
            let _ = asked.send(Work::Finish);
        }
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

impl Drop for Searcher {
    /// Never finished: the thread is told to end, and is not waited for.
    fn drop(&mut self) {
        if let Some(asked) = self.asked.take() {
            let _ = asked.send(Work::Finish);
        }
    }
}

/// What a search is told when searching is not set up, and why.
pub fn not_set_up(why: &str) -> String {
    if why.trim().is_empty() {
        "Searching is not set up yet.".to_owned()
    } else {
        format!("Searching is not set up yet: {why}.")
    }
}

/// Ask `provider` for each search `asking` sends — of those waiting, only
/// the newest — telling the window what came of each, until told to finish.
fn ask(provider: &Provider, asking: &Receiver<Work>, tell: &TellSearched) {
    let mut pool: Option<Pool> = None;
    let mut identities = Identities::default();
    // Work that arrived while a search's images were being decided on.
    let mut pending: VecDeque<Work> = VecDeque::new();
    loop {
        let first = match pending.pop_front() {
            Some(work) => work,
            None => match asking.recv() {
                Ok(work) => work,
                Err(_) => return,
            },
        };
        let waiting: Vec<Work> = core::iter::once(first)
            .chain(pending.drain(..))
            .chain(asking.try_iter())
            .collect();
        if waiting.iter().any(|work| matches!(work, Work::Finish)) {
            return;
        }
        let Some(Work::Search(number, words)) = waiting.into_iter().last() else {
            continue;
        };
        if pool.is_none() {
            match Pool::from_this_machine() {
                Ok(made) => pool = Some(made.patient_for(PATIENCE)),
                Err(why) => {
                    let why = format!(
                        "{PROVIDER} could not be reached: this Mac's certificates could not be \
                         read ({why})"
                    );
                    if !tell(Searched::sites(number, words, Showing::Unanswered(why))) {
                        return;
                    }
                    continue;
                }
            }
        }
        let Some(pool) = pool.as_mut() else {
            continue;
        };
        let showing = brave_search::search(provider, pool, &mut identities, &words);
        let images_follow = provider.asks_images()
            && matches!(&showing, Showing::Found(found) if !found.is_empty());
        let sites = Searched {
            number,
            words: words.clone(),
            came: Came::Sites {
                showing,
                images: if images_follow {
                    Images::Asking
                } else {
                    Images::NotAsked
                },
            },
        };
        if !tell(sites) {
            return;
        }
        if !images_follow {
            continue;
        }
        // Newest wins: a search waiting now is asked before these images,
        // which are then not asked at all.
        pending.extend(asking.try_iter());
        let images = if pending.is_empty() {
            images(provider, pool, &mut identities, &words)
        } else {
            Images::Unanswered(format!(
                "{PROVIDER} was not asked for images of \u{201c}{words}\u{201d}: a newer search \
                 came first."
            ))
        };
        if !tell(Searched {
            number,
            words,
            came: Came::Images(images),
        }) {
            return;
        }
    }
}

/// Ask `provider` for images of `words`, and fetch each thumbnail from its
/// proxy: the images found, or why not.
fn images(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Images {
    let Some(from) = provider.thumbnails() else {
        return Images::NotAsked;
    };
    match brave_search::images(provider, pool, identities, words) {
        Ok(found) => Images::Found(search_thumbnails::fetch(pool, identities, from, found).images),
        Err(why) => Images::Unanswered(why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn with_no_provider_searching_is_not_set_up_and_says_why() {
        let (tell, told) = channel();
        let searcher = Searcher::start(
            Err("no Brave Search key in this Mac's keychain (alo-brave-search)".to_owned()),
            Box::new(move |searched| tell.send(searched).is_ok()),
        )
        .unwrap();
        assert_eq!(
            searcher.set_up(),
            &SetUp::Not("no Brave Search key in this Mac's keychain (alo-brave-search)".into())
        );
        searcher.search(3, "x");
        assert_eq!(
            told.try_recv().ok(),
            Some(Searched::sites(
                3,
                "x",
                Showing::Unanswered(
                    "Searching is not set up yet: no Brave Search key in this Mac's keychain \
                     (alo-brave-search)."
                        .to_owned()
                ),
            ))
        );
        assert!(searcher.finish());
    }
}

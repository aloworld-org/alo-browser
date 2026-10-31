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
//! # Each site's mark (queue item 747, ADR 0088)
//!
//! Once the sites are told, each site's icon is fetched from the provider's
//! image proxy ([`crate::search_marks`]) — never from the site — and the
//! window is told them ([`Came::Marks`]), before the images are asked; not
//! when a newer search is waiting. A video's and a story's mark is fetched
//! with its tab's answer, and told with it.
//!
//! # Videos and news (queue items 745, 746, ADR 0087)
//!
//! Never asked with a search. When a person opens a turn's Videos or News
//! tab, the window hands the thread that turn's words ([`Searcher::videos`],
//! [`Searcher::news`]) under a number of its own, once for the turn; the
//! thread asks the provider the words alone — and, for videos, fetches each
//! thumbnail from its proxy — and tells the window [`Came::Videos`] or
//! [`Came::News`] under that number. Unlike a search, a newer one does not
//! pass over them: a person opened the tab, and is looking at it.
//!
//! # Connections kept
//!
//! The thread keeps one `alo-net` pool, trusting what this machine trusts,
//! for as long as the window is open, so a second search reuses the first
//! one's connection. Nothing of it is written to a disk.

use crate::brave_search::{self, Provider};
use crate::search_marks;
use crate::search_thumbnails;
use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::results::{Images, PROVIDER, SetUp, Showing, Stories, Videos};
use alo_renderer::results_media::{Story, Video};
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
    /// Each site's own mark, by the site's place, if one was fetched and
    /// kept (queue item 747): told after the sites, which are drawn with
    /// their letters until it comes.
    Marks(Vec<Option<Vec<u8>>>),
    /// The videos found, asked when a person opened the Videos tab (queue
    /// item 745), each holding its thumbnail if one was kept, or why not.
    Videos(Videos),
    /// The news found, asked when a person opened the News tab (queue item
    /// 746), or why not.
    News(Stories),
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
    /// Ask for videos of these words, numbered so (queue item 745).
    Videos(u64, String),
    /// Ask for news of these words, numbered so (queue item 746).
    News(u64, String),
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

    /// Ask for videos of `words`, numbered `number`, as a person opened a
    /// turn's Videos tab (queue item 745). Returns at once: what came of it
    /// is told to the window as [`Came::Videos`].
    pub fn videos(&self, number: u64, words: &str) {
        self.further(Work::Videos(number, words.to_owned()), number, words);
    }

    /// Ask for news of `words`, numbered `number`, as a person opened a
    /// turn's News tab (queue item 746). Returns at once: what came of it is
    /// told to the window as [`Came::News`].
    pub fn news(&self, number: u64, words: &str) {
        self.further(Work::News(number, words.to_owned()), number, words);
    }

    /// Hand the thread `work`; with no thread, tell the window at once that
    /// it was not asked, and why.
    fn further(&self, work: Work, number: u64, words: &str) {
        match (&self.asked, &self.set_up, &self.tell) {
            (Some(asked), _, _) => {
                let _ = asked.send(work);
            }
            (None, SetUp::Not(why), Some(tell)) => {
                let why = not_set_up(why);
                let came = match work {
                    Work::News(..) => Came::News(Stories::Unanswered(why)),
                    _ => Came::Videos(Videos::Unanswered(why)),
                };
                let _ = tell(Searched {
                    number,
                    words: words.to_owned(),
                    came,
                });
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
        // Every tab opened is asked, in the order opened; of the searches,
        // only the newest.
        let mut newest = None;
        let mut further = Vec::new();
        for work in waiting {
            match work {
                Work::Search(number, words) => newest = Some((number, words)),
                Work::Videos(..) | Work::News(..) => further.push(work),
                Work::Finish => {}
            }
        }
        for work in further {
            if let Some(answer) = further_answer(provider, &mut pool, &mut identities, work)
                && !tell(answer)
            {
                return;
            }
        }
        let Some((number, words)) = newest else {
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
        let (showing, marks) = brave_search::search(provider, pool, &mut identities, &words);
        let found_sites = matches!(&showing, Showing::Found(found) if !found.is_empty());
        let images_follow = provider.asks_images() && found_sites;
        let images = starting(images_follow);
        let sites = Searched {
            number,
            words: words.clone(),
            came: Came::Sites { showing, images },
        };
        if !tell(sites) {
            return;
        }
        // No sites, no marks: `brave_search::search` names none then.
        if !tell_marks(
            (provider, pool, &mut identities),
            (asking, &mut pending),
            (number, &words, &marks),
            tell,
        ) {
            return;
        }
        if !images_follow {
            continue;
        }
        // Newest wins: a search waiting now is asked before these images,
        // which are then not asked at all. A tab opened meanwhile is not a
        // newer search, and waits for them.
        let images = if newer_waits(asking, &mut pending) {
            Images::Unanswered(format!(
                "{PROVIDER} was not asked for images of \u{201c}{words}\u{201d}: a newer search \
                 came first."
            ))
        } else {
            asked_images(provider, pool, &mut identities, &words)
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

/// Where a search's images start, told with its sites: asking when they
/// will follow, not asked when they will not.
fn starting(images_follow: bool) -> Images {
    if images_follow {
        Images::Asking
    } else {
        Images::NotAsked
    }
}

/// Whether a newer search — or the window closing — waits, once what has
/// arrived on `asking` is kept in `pending`: newest wins, and a tab opened
/// meanwhile is not a newer search.
fn newer_waits(asking: &Receiver<Work>, pending: &mut VecDeque<Work>) -> bool {
    pending.extend(asking.try_iter());
    pending
        .iter()
        .any(|work| matches!(work, Work::Search(..) | Work::Finish))
}

/// Each site's mark of the search `number` for `words`, fetched from
/// `provider`'s proxy through `pool` and told to the window — before its
/// images, a kilobyte each and no paid request — unless none is named, or a
/// newer search waits (work arriving meanwhile is kept in `pending`).
/// Whether the window still hears.
fn tell_marks(
    (provider, pool, identities): (&Provider, &mut Pool, &mut Identities),
    (asking, pending): (&Receiver<Work>, &mut VecDeque<Work>),
    (number, words, marks): (u64, &str, &[Option<alo_url::Url>]),
    tell: &TellSearched,
) -> bool {
    if !marks.iter().any(Option::is_some) {
        return true;
    }
    if newer_waits(asking, pending) {
        return true;
    }
    let marked = search_marks::fetch(pool, identities, provider.thumbnails(), marks);
    if !marked.marks.iter().any(Option::is_some) {
        return true;
    }
    tell(Searched {
        number,
        words: words.to_owned(),
        came: Came::Marks(marked.marks),
    })
}

/// What came of `work`, a tab opened, asked of `provider` through `pool` —
/// made from this machine's certificates if it is not yet — or [`None`]
/// for work that is not a tab's.
fn further_answer(
    provider: &Provider,
    pool: &mut Option<Pool>,
    identities: &mut Identities,
    work: Work,
) -> Option<Searched> {
    if pool.is_none() {
        *pool = Pool::from_this_machine()
            .ok()
            .map(|made| made.patient_for(PATIENCE));
    }
    let unreached =
        || format!("{PROVIDER} could not be reached: this Mac's certificates could not be read");
    let (number, words, came) = match work {
        Work::Videos(number, words) => {
            let came = Came::Videos(match pool.as_mut() {
                Some(pool) => videos(provider, pool, identities, &words),
                None => Videos::Unanswered(unreached()),
            });
            (number, words, came)
        }
        Work::News(number, words) => {
            let came = Came::News(match pool.as_mut() {
                Some(pool) => match brave_search::news(provider, pool, identities, &words) {
                    Ok((found, marks)) => {
                        let marked =
                            search_marks::fetch(pool, identities, provider.thumbnails(), &marks);
                        Stories::Found(
                            found
                                .into_iter()
                                .zip(marked.marks.into_iter().chain(core::iter::repeat(None)))
                                .map(|(story, mark)| Story { mark, ..story })
                                .collect(),
                        )
                    }
                    Err(why) => Stories::Unanswered(why),
                },
                None => Stories::Unanswered(unreached()),
            });
            (number, words, came)
        }
        Work::Search(..) | Work::Finish => return None,
    };
    Some(Searched {
        number,
        words,
        came,
    })
}

/// Ask `provider` for videos of `words`, and fetch each thumbnail from its
/// image proxy: the videos found, or why not.
fn videos(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Videos {
    match brave_search::videos(provider, pool, identities, words) {
        Ok(found) => {
            let fetched =
                search_thumbnails::fetch_videos(pool, identities, provider.thumbnails(), found);
            let marked =
                search_marks::fetch(pool, identities, provider.thumbnails(), &fetched.marks);
            Videos::Found(
                fetched
                    .videos
                    .into_iter()
                    .zip(marked.marks.into_iter().chain(core::iter::repeat(None)))
                    .map(|(video, mark)| Video { mark, ..video })
                    .collect(),
            )
        }
        Err(why) => Videos::Unanswered(why),
    }
}

/// Ask `provider` for images of `words`, and fetch each thumbnail from its
/// proxy: the images found, or why not.
fn asked_images(
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
        searcher.videos(4, "y");
        searcher.news(5, "z");
        let told: Vec<Searched> = told.try_iter().collect();
        assert_eq!(
            told,
            vec![
                Searched {
                    number: 4,
                    words: "y".to_owned(),
                    came: Came::Videos(Videos::Unanswered(
                        "Searching is not set up yet: no Brave Search key in this Mac's \
                         keychain (alo-brave-search)."
                            .to_owned()
                    )),
                },
                Searched {
                    number: 5,
                    words: "z".to_owned(),
                    came: Came::News(Stories::Unanswered(
                        "Searching is not set up yet: no Brave Search key in this Mac's \
                         keychain (alo-brave-search)."
                            .to_owned()
                    )),
                },
            ]
        );
        assert!(searcher.finish());
    }
}

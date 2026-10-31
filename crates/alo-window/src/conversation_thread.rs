/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One chat's conversation, as the browser process holds it (ADR 0085,
//! queue item 703): every turn, oldest first, and how far it is scrolled.
//!
//! # Turns are appended, never replaced
//!
//! Each search and each page opened is a turn under the last
//! (`alo-renderer`'s `turn`): a search's results fill in when they come, a
//! page opens as a live card. One page is open at a time, so a card is live
//! only as the newest turn; when anything newer comes — another page, a
//! search — or its page is closed or goes on its own, the card is **put
//! away** as the address it was last at ([`Turn::Visited`]), and a page
//! that could not be opened leaves why, in words ([`Turn::Failed`]).
//! Nothing a person asked for is lost (`docs/design/shell.md`).
//!
//! # Scrolled
//!
//! How far the conversation is scrolled is one number, [`Thread::down`]:
//! how far the newest turn's top is above the room's top
//! (`alo-renderer`'s `conversation_scroll`). A new turn puts it at `0`, so
//! the new turn's top is in view at the room's top. A person's wheel moves
//! it, kept inside how far the turns reach as the renderer last measured
//! them ([`Thread::reached`]); until a reach is known for the turns now
//! held, the wheel does nothing.

use alo_layout::Size;
use alo_renderer::card::Card;
use alo_renderer::conversation_scroll::{self, Reach};
use alo_renderer::results::{self, Images, Results, Stories, Tab, Videos};
use alo_renderer::turn::Turn;
use alo_url::Url;

/// One chat's turns, and how far they are scrolled.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Thread {
    /// Every turn, oldest first.
    turns: Vec<Turn>,
    /// How far the newest turn's top is above the room's top, in CSS
    /// pixels.
    down: f32,
    /// How far the turns reach, as the renderer last measured as many turns
    /// as are held.
    reach: Option<Reach>,
}

impl Thread {
    /// Every turn, oldest first.
    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    /// How far the conversation is scrolled.
    pub fn down(&self) -> f32 {
        self.down
    }

    /// How far the turns reach, once the renderer has measured as many as
    /// are held.
    pub fn reach(&self) -> Option<Reach> {
        self.reach
    }

    /// Append `turn`, the live card put away first, and scroll so its top
    /// is at the room's top.
    fn append(&mut self, turn: Turn) {
        let _ = self.put_away_card();
        self.turns.push(turn);
        self.down = 0.0;
        self.reach = None;
    }

    /// A search for `said` under way, as a new turn.
    pub fn search(&mut self, said: String) {
        self.append(Turn::Searched(Results::searching(said)));
    }

    /// What came of the search for `words`: its sites, shown in the newest
    /// turn still searching for them, and where the search for its images
    /// starts (queue item 743). Whether one was.
    pub fn searched(&mut self, words: &str, showing: results::Showing, images: Images) -> bool {
        let waiting = self.turns.iter_mut().rev().find_map(|turn| match turn {
            Turn::Searched(results)
                if results.said == words
                    && matches!(results.showing, results::Showing::Searching) =>
            {
                Some(results)
            }
            _ => None,
        });
        match waiting {
            Some(results) => {
                results.showing = showing;
                results.images = images;
                true
            }
            None => false,
        }
    }

    /// What came of the search for images of `words`: shown in the newest
    /// turn still asking for them. Whether one was.
    pub fn pictured(&mut self, words: &str, images: Images) -> bool {
        let asking = self.turns.iter_mut().rev().find_map(|turn| match turn {
            Turn::Searched(results)
                if results.said == words && matches!(results.images, Images::Asking) =>
            {
                Some(results)
            }
            _ => None,
        });
        match asking {
            Some(results) => {
                results.images = images;
                true
            }
            None => false,
        }
    }

    /// Each site's mark, by its place, of the search for `words` (queue
    /// item 747): put in the newest turn of those words whose sites are as
    /// many as the marks and hold none yet. Whether one was.
    pub fn marked(&mut self, words: &str, marks: &[Option<Vec<u8>>]) -> bool {
        let waiting = self.turns.iter_mut().rev().find_map(|turn| match turn {
            Turn::Searched(results) if results.said == words => match &mut results.showing {
                results::Showing::Found(found)
                    if found.len() == marks.len()
                        && found.iter().all(|site| site.mark.is_none()) =>
                {
                    Some(found)
                }
                _ => None,
            },
            _ => None,
        });
        match waiting {
            Some(found) => {
                for (site, mark) in found.iter_mut().zip(marks) {
                    site.mark.clone_from(mark);
                }
                true
            }
            None => false,
        }
    }

    /// Show the search's results in the turn at `turn` on `tab` (queue items
    /// 743, 744): whether that changed anything, which it does not when the
    /// turn is no search's, the tab cannot be chosen, or is already shown.
    pub fn show_tab(&mut self, turn: usize, tab: Tab) -> bool {
        match self.turns.get_mut(turn) {
            Some(Turn::Searched(results)) if results.open(tab) && results.tab != tab => {
                results.tab = tab;
                true
            }
            _ => false,
        }
    }

    /// The words to ask the provider for `tab`'s results of the search in
    /// the turn at `turn`, when that tab is Videos or News and has never
    /// been asked for — which it then is (queue items 745, 746, ADR 0087):
    /// once for a turn, whatever comes of it.
    pub fn ask_for(&mut self, turn: usize, tab: Tab) -> Option<String> {
        let Some(Turn::Searched(results)) = self.turns.get_mut(turn) else {
            return None;
        };
        if !results.open(tab) {
            return None;
        }
        match tab {
            Tab::Videos if matches!(results.videos, Videos::NotAsked) => {
                results.videos = Videos::Asking;
            }
            Tab::News if matches!(results.news, Stories::NotAsked) => {
                results.news = Stories::Asking;
            }
            _ => return None,
        }
        Some(results.said.clone())
    }

    /// What came of asking for the videos of the search in the turn at
    /// `turn`: shown, if it is still asking. Whether it was.
    pub fn videos_came(&mut self, turn: usize, videos: Videos) -> bool {
        match self.turns.get_mut(turn) {
            Some(Turn::Searched(results)) if matches!(results.videos, Videos::Asking) => {
                results.videos = videos;
                true
            }
            _ => false,
        }
    }

    /// What came of asking for the news of the search in the turn at
    /// `turn`: shown, if it is still asking. Whether it was.
    pub fn news_came(&mut self, turn: usize, news: Stories) -> bool {
        match self.turns.get_mut(turn) {
            Some(Turn::Searched(results)) if matches!(results.news, Stories::Asking) => {
                results.news = news;
                true
            }
            _ => false,
        }
    }

    /// Every open tab the turns draw, in the order drawn: each turn's place
    /// and the tab.
    pub fn tabs(&self) -> Vec<(usize, Tab)> {
        self.searches()
            .flat_map(|(turn, results)| results.open_tabs().into_iter().map(move |tab| (turn, tab)))
            .collect()
    }

    /// The place of every turn drawing *More sites ›*, in order.
    pub fn more_sites(&self) -> Vec<usize> {
        self.searches()
            .filter(|(_, results)| results.draws_more_sites())
            .map(|(turn, _)| turn)
            .collect()
    }

    /// The place of every turn drawing *See all images*, in order.
    pub fn see_all(&self) -> Vec<usize> {
        self.searches()
            .filter(|(_, results)| results.draws_see_all())
            .map(|(turn, _)| turn)
            .collect()
    }

    /// The page of every image the turns draw, in order.
    pub fn pictured_pages(&self) -> Vec<Url> {
        self.turns
            .iter()
            .flat_map(Turn::pictured)
            .cloned()
            .collect()
    }

    /// The title of the image found on the page at `url`, as a turn draws
    /// it: the newest such.
    pub fn said_for_image(&self, url: &Url) -> Option<String> {
        self.searches().rev().find_map(|(_, results)| {
            results
                .drawn_images()
                .iter()
                .find(|image| image.page == *url)
                .map(|image| image.title.clone())
        })
    }

    /// Each turn that is a search, with its place.
    fn searches(&self) -> impl DoubleEndedIterator<Item = (usize, &Results)> {
        self.turns
            .iter()
            .enumerate()
            .filter_map(|(place, turn)| match turn {
                Turn::Searched(results) => Some((place, results)),
                _ => None,
            })
    }

    /// The newest search's results, if a turn holds any.
    pub fn results(&self) -> Option<&Results> {
        self.turns.iter().rev().find_map(|turn| match turn {
            Turn::Searched(results) => Some(results),
            _ => None,
        })
    }

    /// The page at `url` opens as a live card answering `said`: a new turn.
    pub fn open_card(&mut self, said: String, url: Url) {
        self.append(Turn::Page(Card::new(said, url)));
    }

    /// The live card, if the newest turn is one.
    pub fn card(&self) -> Option<&Card> {
        self.turns.last().and_then(Turn::card)
    }

    /// The live card, to change, if the newest turn is one.
    pub fn card_mut(&mut self) -> Option<&mut Card> {
        match self.turns.last_mut() {
            Some(Turn::Page(card)) => Some(card),
            _ => None,
        }
    }

    /// The live card put away as the address it was last at: whether there
    /// was one.
    pub fn put_away_card(&mut self) -> bool {
        let Some(turn) = self.turns.last_mut() else {
            return false;
        };
        let Turn::Page(card) = turn else {
            return false;
        };
        *turn = Turn::Visited {
            said: std::mem::take(&mut card.said),
            url: card.url.clone(),
        };
        true
    }

    /// The live card's page could not be opened, for `why`: the turn says so
    /// in its place. Whether there was a live card.
    pub fn card_failed(&mut self, why: String) -> bool {
        let Some(turn) = self.turns.last_mut() else {
            return false;
        };
        let Turn::Page(card) = turn else {
            return false;
        };
        *turn = Turn::Failed {
            said: std::mem::take(&mut card.said),
            why,
        };
        true
    }

    /// What the person said for the link at `url`, as a turn draws it:
    /// a site's title among a search's results, or what was said for a page
    /// shown earlier — the newest such, if several hold it.
    pub fn said_for(&self, url: &Url) -> Option<String> {
        self.turns.iter().rev().find_map(|turn| match turn {
            Turn::Searched(results) => results
                .drawn_sites()
                .iter()
                .find(|found| found.url == *url)
                .map(|found| found.title.clone())
                .or_else(|| {
                    results
                        .drawn_videos()
                        .iter()
                        .find(|video| video.page == *url)
                        .map(|video| video.title.clone())
                })
                .or_else(|| {
                    results
                        .drawn_stories()
                        .iter()
                        .find(|story| story.page == *url)
                        .map(|story| story.title.clone())
                }),
            Turn::Visited { said, url: at } if at == url => Some(said.clone()),
            _ => None,
        })
    }

    /// Every link the turns draw, in the order they draw them: what a
    /// press on a title is decided against.
    pub fn links(&self) -> Vec<Url> {
        self.turns.iter().flat_map(Turn::links).cloned().collect()
    }

    /// The renderer measured `drawn` turns to reach as `reach` says, in a
    /// first screen laid out at `viewport`: kept, when as many turns are
    /// held now, and how far it is scrolled kept inside it. Whether that
    /// moved it.
    pub fn reached(&mut self, reach: Reach, drawn: usize, viewport: Size) -> bool {
        if drawn != self.turns.len() {
            // Measured before the newest turn came: not these turns' reach.
            return false;
        }
        self.reach = Some(reach.bounded());
        let kept = conversation_scroll::clamped(self.down, reach, viewport);
        let moved = kept.to_bits() != self.down.to_bits();
        self.down = kept;
        moved
    }

    /// A person's wheel turned `by` CSS pixels down over the conversation,
    /// in a first screen laid out at `viewport`: scrolled that far, inside
    /// how far it reaches. Whether it moved.
    pub fn scroll(&mut self, by: f32, viewport: Size) -> bool {
        let Some(reach) = self.reach else {
            return false;
        };
        let kept = conversation_scroll::clamped(self.down + by, reach, viewport);
        let moved = kept.to_bits() != self.down.to_bits();
        self.down = kept;
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::results::{Found, Pictured, Showing};

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap_or_else(|_| Url::about_blank())
    }

    #[test]
    fn images_fill_in_after_the_sites_and_a_tab_is_shown_only_when_it_can_be() {
        let mut thread = Thread::default();
        thread.search("ev".to_owned());
        assert!(thread.tabs() == vec![(0, Tab::All)], "searching: All alone");
        assert!(
            !thread.pictured("ev", Images::Found(Vec::new())),
            "not asking yet"
        );
        let sites: Vec<Found> = (0..5)
            .map(|n| Found::new(format!("S{n}"), url(&format!("https://s{n}.example/")), ""))
            .collect();
        assert!(thread.searched("ev", Showing::Found(sites), Images::Asking));
        assert_eq!(
            thread.tabs(),
            vec![
                (0, Tab::All),
                (0, Tab::Sites),
                (0, Tab::Images),
                (0, Tab::Videos),
                (0, Tab::News)
            ]
        );
        assert_eq!(thread.more_sites(), vec![0]);
        assert!(thread.see_all().is_empty(), "no row while asking");
        let images: Vec<Pictured> = (0..7)
            .map(|n| Pictured::new(format!("I{n}"), url(&format!("https://p{n}.example/")), ""))
            .collect();
        assert!(!thread.pictured("other words", Images::Found(images.clone())));
        assert!(thread.pictured("ev", Images::Found(images)));
        assert!(
            !thread.pictured("ev", Images::Found(Vec::new())),
            "answered"
        );
        assert_eq!(thread.see_all(), vec![0]);
        assert_eq!(thread.pictured_pages().len(), 5, "All's row");
        assert_eq!(thread.links().len(), 3, "All's three");
        assert_eq!(
            thread
                .said_for_image(&url("https://p2.example/"))
                .as_deref(),
            Some("I2")
        );
        assert!(thread.show_tab(0, Tab::Sites));
        assert!(!thread.show_tab(0, Tab::Sites), "already shown");
        assert_eq!(thread.links().len(), 5, "every site on Sites");
        assert!(thread.pictured_pages().is_empty() && thread.more_sites().is_empty());
        assert!(thread.show_tab(0, Tab::Images));
        assert_eq!(thread.pictured_pages().len(), 7, "every image on Images");
        assert!(thread.links().is_empty());
        assert!(!thread.show_tab(1, Tab::All), "no turn there");
        thread.open_card("a".to_owned(), url("https://a.example/"));
        assert!(!thread.show_tab(1, Tab::All), "a card is no search");
        assert!(thread.show_tab(0, Tab::All));
        // A search not answered opens no other tab.
        thread.search("x".to_owned());
        assert!(thread.searched("x", Showing::Unanswered("no".to_owned()), Images::NotAsked));
        assert!(!thread.show_tab(2, Tab::Sites) && !thread.show_tab(2, Tab::Images));
    }

    const VIEWPORT: Size = Size {
        width: 1040.0,
        height: 820.0,
    };

    #[test]
    fn videos_and_news_are_asked_for_once_and_shown_only_while_asked() {
        use alo_renderer::results::{Stories, Videos};
        use alo_renderer::results_media::Video;
        let mut thread = Thread::default();
        thread.search("ev".to_owned());
        assert_eq!(thread.ask_for(0, Tab::Videos), None, "no sites yet");
        let sites = vec![Found::new("S", url("https://s.example/"), "")];
        assert!(thread.searched("ev", Showing::Found(sites), Images::NotAsked));
        assert!(
            !thread.videos_came(0, Videos::Found(Vec::new())),
            "never asked"
        );
        assert_eq!(thread.ask_for(0, Tab::Videos), Some("ev".to_owned()));
        assert_eq!(thread.ask_for(0, Tab::Videos), None, "once");
        assert_eq!(thread.ask_for(0, Tab::Images), None, "not a tab asked so");
        assert_eq!(thread.ask_for(1, Tab::News), None, "no turn there");
        let video = Video::new("V", url("https://v.example/"));
        assert!(thread.videos_came(0, Videos::Found(vec![video])));
        assert!(
            !thread.videos_came(0, Videos::Found(Vec::new())),
            "answered"
        );
        assert_eq!(
            thread.ask_for(0, Tab::Videos),
            None,
            "answered, never again"
        );
        assert_eq!(thread.ask_for(0, Tab::News), Some("ev".to_owned()));
        assert!(thread.news_came(0, Stories::Unanswered("no".to_owned())));
        assert_eq!(
            thread.ask_for(0, Tab::News),
            None,
            "unanswered, never again"
        );
        assert!(thread.show_tab(0, Tab::Videos));
        assert_eq!(thread.links(), vec![url("https://v.example/")]);
        assert_eq!(
            thread.said_for(&url("https://v.example/")).as_deref(),
            Some("V")
        );
    }

    #[test]
    fn turns_are_appended_and_a_live_card_is_put_away_by_anything_newer() {
        let mut thread = Thread::default();
        thread.open_card("example.com".to_owned(), url("https://example.com/"));
        assert!(thread.card().is_some());
        thread.search("ev chargers".to_owned());
        assert_eq!(thread.card(), None);
        assert_eq!(
            thread.turns().first(),
            Some(&Turn::Visited {
                said: "example.com".to_owned(),
                url: url("https://example.com/"),
            })
        );
        assert!(thread.searched(
            "ev chargers",
            Showing::Found(vec![Found::new("Chargers", url("https://a.example/"), "")]),
            Images::NotAsked
        ));
        assert!(
            !thread.searched("ev chargers", Showing::Found(Vec::new()), Images::NotAsked),
            "answered"
        );
        assert_eq!(
            thread.said_for(&url("https://a.example/")).as_deref(),
            Some("Chargers")
        );
        assert_eq!(
            thread.said_for(&url("https://example.com/")).as_deref(),
            Some("example.com")
        );
        assert_eq!(
            thread.links(),
            vec![url("https://example.com/"), url("https://a.example/")]
        );
        thread.open_card("Chargers".to_owned(), url("https://a.example/"));
        assert!(thread.card_failed("https://a.example/ could not be opened: no".to_owned()));
        assert!(
            matches!(thread.turns().last(), Some(Turn::Failed { said, .. }) if said == "Chargers")
        );
        assert!(!thread.put_away_card(), "no live card");
        assert_eq!(thread.turns().len(), 3);
    }

    #[test]
    fn a_wheel_scrolls_inside_the_reach_and_a_new_turn_scrolls_to_its_top() {
        let mut thread = Thread::default();
        thread.search("a".to_owned());
        assert!(!thread.scroll(-100.0, VIEWPORT), "no reach yet");
        thread.search("b".to_owned());
        let reach = Reach {
            earlier: 489.0,
            newest: 457.0,
        };
        assert!(!thread.reached(reach, 1, VIEWPORT), "one turn's reach");
        assert!(!thread.scroll(-100.0, VIEWPORT), "not these turns'");
        assert!(!thread.reached(reach, 2, VIEWPORT));
        assert!(thread.scroll(-100.0, VIEWPORT));
        assert_eq!(thread.down().to_bits(), (-100.0_f32).to_bits());
        assert!(thread.scroll(-1000.0, VIEWPORT));
        assert_eq!(
            thread.down().to_bits(),
            (-489.0_f32).to_bits(),
            "the first turn's top"
        );
        assert!(!thread.scroll(-1.0, VIEWPORT), "no further");
        assert!(thread.scroll(5000.0, VIEWPORT));
        assert_eq!(
            thread.down().to_bits(),
            0.0_f32.to_bits(),
            "the newest fits"
        );
        thread.scroll(-200.0, VIEWPORT);
        thread.search("c".to_owned());
        assert_eq!(
            thread.down().to_bits(),
            0.0_f32.to_bits(),
            "the new turn at the top"
        );
        assert!(
            !thread.scroll(-10.0, VIEWPORT),
            "its reach not measured yet"
        );
        // A shorter window: the reach measured again keeps it inside.
        let _ = thread.reached(reach, 3, VIEWPORT);
        let _ = thread.scroll(-300.0, VIEWPORT);
        assert!(!thread.reached(reach, 3, Size::new(1040.0, 520.0)));
        let tall = Reach {
            earlier: 100.0,
            newest: 457.0,
        };
        assert!(thread.reached(tall, 3, VIEWPORT), "kept inside");
        assert_eq!(thread.down().to_bits(), (-100.0_f32).to_bits());
    }
}

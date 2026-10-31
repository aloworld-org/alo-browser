/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's results, shown in the conversation as the design's **All**
//! tab draws them (`docs/design/shell.md`, *What appears when a person
//! types*; ADR 0082 § 0, ADR 0083; queue item 708): what the browser process
//! tells the first screen's renderer of them, as data.
//!
//! The browser process asks the search provider — the Brave Search API —
//! on a thread of its own and reads its answer (`alo-window`'s
//! `conversation_search`); this is what it then holds: what the person
//! said, and whether the search is still under way, found sites, or was
//! not answered and why. Each site is its title, its address, its two lines
//! of description and the name the provider gives it — every one a
//! stranger's string, which enters the first screen's document as a text
//! node ([`crate::results_document`]).
//!
//! # What All shows
//!
//! The tab row, *All* chosen; the top [`ON_ALL`] sites, each its mark — the
//! first letter of its name — its name and address, its title as a link and
//! two lines of its description; then a row of [`IMAGES_ON_ALL`] images
//! with *See all images* (queue item 743), once the images have come; then
//! *More sites ›*, which opens the Sites tab (744). The short answer the
//! design puts first waits on the model (queue item 707) and is not drawn,
//! nor anything in its place.
//!
//! # The other tabs
//!
//! [`Tab`]: **Sites**, every site the search found, and **Images**, a grid
//! of every image found, each its picture, title and site (ADR 0086);
//! **Videos**, each its picture, length, title and site, and **News**, each
//! story's site, age and title (queue items 745, 746, ADR 0087), each asked
//! of the provider only when a person opens its tab, once for a turn.
//! Answer (707) is drawn disabled, and claims nothing. Which tab a search
//! shows is the browser process's, kept with its results; a press on a tab
//! is claimed and decided as a title is.
//!
//! # The pictures
//!
//! Each image's picture is a thumbnail the browser process fetched from the
//! provider's own image proxy and bounded ([`LARGEST_THUMBNAIL`]); it is
//! carried here as the bytes that arrived, and decoded only in the confined
//! renderer, by the picture readers every `<img>` goes through. Only the
//! pictures a tab draws are sent, and at most [`MOST_PICTURE_BYTES`] of them
//! in one first screen, the newest turns' first ([`Results::drawn_images`],
//! [`within_budget`]).

use crate::results_media::{Story, Video};
use crate::turn::Turn;
use alo_url::Url;

/// How many sites the All tab shows, so that the column, the box and what
/// the box offers fit the design's window, 820 tall, without scrolling —
/// which the first screen does not do.
pub const ON_ALL: usize = 3;

/// The most sites a search's results are kept with: Brave's own most for
/// one page of web results.
pub const MOST_FOUND: usize = 20;

/// The tabs of a search's results, in the design's order.
pub const TABS: [&str; 6] = ["All", "Answer", "Sites", "Images", "Videos", "News"];

/// What the search provider is called, where the person reads it.
pub const PROVIDER: &str = "Brave Search";

/// How many images the All tab's row shows: the design's five.
pub const IMAGES_ON_ALL: usize = 5;

/// The most images a search's results are kept with, and the Images tab
/// shows: five rows of the design's four.
pub const MOST_IMAGES: usize = 20;

/// The largest thumbnail kept, in bytes. Brave's thumbnails are 500 px wide
/// (ADR 0086 § 3); a larger answer is not a thumbnail, and is left out.
pub const LARGEST_THUMBNAIL: usize = 256 * 1024;

/// The largest mark kept, in bytes (queue item 747, ADR 0088): Brave's
/// proxy sends a site's icon at 32 px square, a kilobyte or two; a larger
/// answer is not an icon, and the site keeps its letter.
pub const LARGEST_MARK: usize = 16 * 1024;

/// The most marks fetched for one answer — a search's sites, a tab's videos
/// or its stories — each distinct icon once: one for each site Brave
/// returns at most.
pub const MOST_MARKS: usize = 20;

/// The most bytes of pictures one first screen carries to its renderer
/// (ADR 0086 § 4): the first screen is sent whole for every frame of a live
/// card, so what its pictures weigh is paid again each time.
pub const MOST_PICTURE_BYTES: usize = 2 * 1024 * 1024;

/// A tab of a search's results that is built (queue items 708, 743, 744,
/// 745, 746).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    /// The top sites and a row of images.
    #[default]
    All,
    /// Every site found.
    Sites,
    /// Every image found, as a grid.
    Images,
    /// Every video found, as a list.
    Videos,
    /// Every news story found, as a list.
    News,
}

impl Tab {
    /// Its place among [`TABS`].
    pub fn place(self) -> usize {
        match self {
            Tab::All => 0,
            Tab::Sites => 2,
            Tab::Images => 3,
            Tab::Videos => 4,
            Tab::News => 5,
        }
    }

    /// The tab at `place` among [`TABS`], if it is built.
    pub fn at(place: usize) -> Option<Self> {
        [Tab::All, Tab::Sites, Tab::Images, Tab::Videos, Tab::News]
            .into_iter()
            .find(|tab| tab.place() == place)
    }
}

/// Whether searching can be done, as the box's offer says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetUp {
    /// It can: searches go to [`PROVIDER`].
    Ready,
    /// It cannot, and why, in words a person reads under the offer.
    Not(String),
}

/// A search's results, as the conversation shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Results {
    /// What the person said: the line above the results.
    pub said: String,
    /// Where the search is.
    pub showing: Showing,
    /// Where the search for images is (queue item 743).
    pub images: Images,
    /// Which tab is shown (queue item 744).
    pub tab: Tab,
    /// Where the search for videos is (queue item 745): asked only once a
    /// person opens the Videos tab.
    pub videos: Videos,
    /// Where the search for news is (queue item 746): asked only once a
    /// person opens the News tab.
    pub news: Stories,
}

/// Where a further search of a search's words is — for its images, its
/// videos or its news.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked<T> {
    /// Not asked: for images, the search found no sites, or is still under
    /// way, or images are not searched where it went (ADR 0086 § 1); for
    /// videos and news, nobody has opened the tab (ADR 0087).
    NotAsked,
    /// Asked, and not yet answered.
    Asking,
    /// Answered: what was found, best first — perhaps nothing.
    Found(Vec<T>),
    /// Not answered, and why, in a sentence a person reads.
    Unanswered(String),
}

impl<T> Default for Asked<T> {
    /// Not asked.
    fn default() -> Self {
        Self::NotAsked
    }
}

impl<T> Asked<T> {
    /// What was found, best first: nothing until it is answered.
    pub fn found(&self) -> &[T] {
        match self {
            Asked::Found(found) => found,
            Asked::NotAsked | Asked::Asking | Asked::Unanswered(_) => &[],
        }
    }
}

/// Where a search for images is.
pub type Images = Asked<Pictured>;

/// Where a search for videos is (queue item 745).
pub type Videos = Asked<Video>;

/// Where a search for news is (queue item 746).
pub type Stories = Asked<Story>;

/// One image a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pictured {
    /// What the provider calls the image, as plain text.
    pub title: String,
    /// The page the image was found on: an `http` or `https` address, which
    /// a press on the image opens.
    pub page: Url,
    /// The site the provider says it is from, perhaps nothing.
    pub source: String,
    /// The thumbnail's bytes as they arrived, if one did and was kept:
    /// decoded only by the renderer that draws it.
    pub picture: Option<Vec<u8>>,
}

/// Where a search is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Showing {
    /// Asked, and not yet answered.
    Searching,
    /// Answered: the sites found, best first — perhaps none.
    Found(Vec<Found>),
    /// Not answered, and why, in a sentence a person reads.
    Unanswered(String),
}

/// One site a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The page's title, as plain text.
    pub title: String,
    /// The page's address: an `http` or `https` one.
    pub url: Url,
    /// What the provider says of the page, as plain text, perhaps nothing.
    pub description: String,
    /// The site's name, as the provider gives it, perhaps nothing.
    pub name: String,
    /// The site's own mark — its icon, fetched from the provider's image
    /// proxy and bounded ([`LARGEST_MARK`]) — as the bytes that arrived, if
    /// one did (queue item 747): decoded only by the renderer that draws
    /// it, and the site's letter drawn instead when it does not decode.
    pub mark: Option<Vec<u8>>,
}

impl Results {
    /// A search for `said`, under way.
    pub fn searching(said: impl Into<String>) -> Self {
        Self::of(said, Showing::Searching)
    }

    /// A search for `said`, where `showing` says, on All with no images.
    pub fn of(said: impl Into<String>, showing: Showing) -> Self {
        Self {
            said: said.into(),
            showing,
            images: Images::NotAsked,
            tab: Tab::All,
            videos: Videos::NotAsked,
            news: Stories::NotAsked,
        }
    }

    /// The same results, with `images`.
    #[must_use]
    pub fn with_images(mut self, images: Images) -> Self {
        self.images = images;
        self
    }

    /// The same results, on `tab`.
    #[must_use]
    pub fn on(mut self, tab: Tab) -> Self {
        self.tab = tab;
        self
    }

    /// The same results, with `videos`.
    #[must_use]
    pub fn with_videos(mut self, videos: Videos) -> Self {
        self.videos = videos;
        self
    }

    /// The same results, with `news`.
    #[must_use]
    pub fn with_news(mut self, news: Stories) -> Self {
        self.news = news;
        self
    }

    /// Whether `tab` can be chosen: All always; Sites, Videos and News
    /// once sites were found — a search that drew nothing pays for nothing
    /// more (ADR 0087); Images once images were asked for.
    pub fn open(&self, tab: Tab) -> bool {
        let found = matches!(&self.showing, Showing::Found(found) if !found.is_empty());
        match tab {
            Tab::All => true,
            Tab::Sites | Tab::Videos | Tab::News => found,
            Tab::Images => !matches!(self.images, Images::NotAsked),
        }
    }

    /// The tabs drawn open, in the order drawn: the one shown, and every
    /// other that can be chosen. What a press on a tab is decided against.
    pub fn open_tabs(&self) -> Vec<Tab> {
        (0..TABS.len())
            .filter_map(Tab::at)
            .filter(|&tab| tab == self.tab || self.open(tab))
            .collect()
    }

    /// Whether *More sites ›* is drawn: on All, under sites found.
    pub fn draws_more_sites(&self) -> bool {
        self.tab == Tab::All && self.note().is_none()
    }

    /// Whether *See all images* is drawn: over All's row, when it holds
    /// any.
    pub fn draws_see_all(&self) -> bool {
        self.tab == Tab::All && !self.drawn_images().is_empty()
    }

    /// The sites the tab shown draws, best first: All's [`ON_ALL`], every
    /// one on Sites, none on Images.
    pub fn drawn_sites(&self) -> &[Found] {
        match (self.tab, &self.showing) {
            (Tab::All, _) => self.on_all(),
            (Tab::Sites, Showing::Found(found)) => found,
            _ => &[],
        }
    }

    /// The images the tab shown draws, best first: All's row of
    /// [`IMAGES_ON_ALL`] under its sites, every one on Images, none on
    /// Sites.
    pub fn drawn_images(&self) -> &[Pictured] {
        let Images::Found(found) = &self.images else {
            return &[];
        };
        match self.tab {
            Tab::All if !self.on_all().is_empty() => found
                .get(..IMAGES_ON_ALL.min(found.len()))
                .unwrap_or_default(),
            Tab::Images => found,
            Tab::All | Tab::Sites | Tab::Videos | Tab::News => &[],
        }
    }

    /// The videos the tab shown draws, best first: every one on Videos,
    /// none elsewhere.
    pub fn drawn_videos(&self) -> &[Video] {
        match self.tab {
            Tab::Videos => self.videos.found(),
            _ => &[],
        }
    }

    /// The news stories the tab shown draws, best first: every one on
    /// News, none elsewhere.
    pub fn drawn_stories(&self) -> &[Story] {
        match self.tab {
            Tab::News => self.news.found(),
            _ => &[],
        }
    }

    /// What the Videos tab says where the videos would be, when there are
    /// none to show.
    pub fn videos_note(&self) -> Option<String> {
        asked_note(&self.videos, &self.said, "videos")
    }

    /// What the News tab says where the stories would be, when there are
    /// none to show.
    pub fn news_note(&self) -> Option<String> {
        asked_note(&self.news, &self.said, "news")
    }

    /// What the Images tab says where the images would be, when there are
    /// none to show: that it is searching, that none were found, or why it
    /// was not answered.
    pub fn images_note(&self) -> Option<String> {
        match &self.images {
            Images::NotAsked => Some(format!(
                "Images were not searched for \u{201c}{}\u{201d}.",
                self.said
            )),
            Images::Asking => Some(format!(
                "Searching {PROVIDER} for images of \u{201c}{}\u{201d}\u{2026}",
                self.said
            )),
            Images::Found(found) if found.is_empty() => Some(format!(
                "{PROVIDER} found no images for \u{201c}{}\u{201d}.",
                self.said
            )),
            Images::Found(_) => None,
            Images::Unanswered(why) => Some(why.clone()),
        }
    }

    /// The same results with no picture the tab shown does not draw: what
    /// is sent to the renderer.
    #[must_use]
    pub fn as_drawn(&self) -> Self {
        let mut sent = self.clone();
        let drawn = self.drawn_images().len();
        let all = matches!(self.tab, Tab::Images);
        if let Images::Found(found) = &mut sent.images {
            for (place, image) in found.iter_mut().enumerate() {
                if !(all || place < drawn) {
                    image.picture = None;
                }
            }
        }
        if self.tab != Tab::Videos
            && let Videos::Found(found) = &mut sent.videos
        {
            for video in found.iter_mut() {
                video.picture = None;
                video.mark = None;
            }
        }
        // A site's mark goes only where the site is drawn.
        let sites = self.drawn_sites().len();
        if let Showing::Found(found) = &mut sent.showing {
            for site in found.iter_mut().skip(sites) {
                site.mark = None;
            }
        }
        if self.tab != Tab::News
            && let Stories::Found(found) = &mut sent.news
        {
            for story in found.iter_mut() {
                story.mark = None;
            }
        }
        sent
    }

    /// The bytes of every picture these results carry: thumbnails and
    /// marks.
    pub fn picture_bytes(&self) -> usize {
        self.mark_bytes() + self.thumbnail_bytes()
    }

    /// The bytes of every site's, video's and story's mark these results
    /// carry.
    fn mark_bytes(&self) -> usize {
        let weigh = |mark: &Option<Vec<u8>>| mark.as_ref().map_or(0, Vec::len);
        let sites: usize = match &self.showing {
            Showing::Found(found) => found.iter().map(|site| weigh(&site.mark)).sum(),
            Showing::Searching | Showing::Unanswered(_) => 0,
        };
        let videos: usize = self
            .videos
            .found()
            .iter()
            .map(|video| weigh(&video.mark))
            .sum();
        let stories: usize = self
            .news
            .found()
            .iter()
            .map(|story| weigh(&story.mark))
            .sum();
        sites + videos + stories
    }

    /// The bytes of every image's and video's thumbnail these results carry.
    fn thumbnail_bytes(&self) -> usize {
        let images: usize = self
            .images
            .found()
            .iter()
            .filter_map(|image| image.picture.as_ref())
            .map(Vec::len)
            .sum();
        let videos: usize = self
            .videos
            .found()
            .iter()
            .filter_map(|video| video.picture.as_ref())
            .map(Vec::len)
            .sum();
        images + videos
    }

    /// The sites All shows, best first: at most [`ON_ALL`].
    pub fn on_all(&self) -> &[Found] {
        match &self.showing {
            Showing::Found(found) => found.get(..ON_ALL.min(found.len())).unwrap_or_default(),
            Showing::Searching | Showing::Unanswered(_) => &[],
        }
    }

    /// What is said where the sites would be, when there are none to show:
    /// that it is searching, that nothing was found, or why it was not
    /// answered.
    pub fn note(&self) -> Option<String> {
        match &self.showing {
            Showing::Searching => Some(format!(
                "Searching {PROVIDER} for \u{201c}{}\u{201d}\u{2026}",
                self.said
            )),
            Showing::Found(found) if found.is_empty() => Some(format!(
                "{PROVIDER} found no sites for \u{201c}{}\u{201d}.",
                self.said
            )),
            Showing::Found(_) => None,
            Showing::Unanswered(why) => Some(why.clone()),
        }
    }
}

impl Found {
    /// A site found at `url`, called `title`, said of as `description`.
    pub fn new(title: impl Into<String>, url: Url, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url,
            description: description.into(),
            name: String::new(),
            mark: None,
        }
    }

    /// The same site, named `name` by the provider.
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// The same site, its mark `bytes` (queue item 747).
    #[must_use]
    pub fn marked(mut self, bytes: Vec<u8>) -> Self {
        self.mark = Some(bytes);
        self
    }

    /// What the site is called in bold: its name, or its host when the
    /// provider gave none.
    pub fn site(&self) -> String {
        if self.name.trim().is_empty() {
            alo_url::reading::host(&self.url)
        } else {
            self.name.trim().to_owned()
        }
    }

    /// Its address as the design writes it: the host, then each part of the
    /// path after a `›` — `evhomeguide.org › installing-a-charger`.
    pub fn address(&self) -> String {
        let mut written = alo_url::reading::host(&self.url);
        for part in self.url.path.split('/').filter(|part| !part.is_empty()) {
            written.push_str(" \u{203a} ");
            written.push_str(part);
        }
        written
    }

    /// The site's mark: the first letter or digit of what it is called, as
    /// a capital.
    pub fn letter(&self) -> Option<String> {
        letter(&self.site())
    }

    /// Which of the mark's four colours the site is drawn in, the same for
    /// a host every time: a number below four.
    pub fn tint(&self) -> usize {
        tint(&self.url)
    }
}

impl Pictured {
    /// An image called `title`, found on the page at `page`, from `source`,
    /// with no picture yet.
    pub fn new(title: impl Into<String>, page: Url, source: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            page,
            source: source.into(),
            picture: None,
        }
    }

    /// The same image, its thumbnail `bytes`.
    #[must_use]
    pub fn holding(mut self, bytes: Vec<u8>) -> Self {
        self.picture = Some(bytes);
        self
    }

    /// The site it is from: the provider's word for it, or its page's host.
    pub fn site(&self) -> String {
        if self.source.trim().is_empty() {
            alo_url::reading::host(&self.page)
        } else {
            self.source.trim().to_owned()
        }
    }

    /// The site's mark, as a site found has one.
    pub fn letter(&self) -> Option<String> {
        letter(&self.site())
    }

    /// The mark's tint, as a site found's, by its page's host.
    pub fn tint(&self) -> usize {
        tint(&self.page)
    }
}

/// What a tab says where what it shows would be, when there is none: that
/// it was not asked, is being asked, found nothing, or why it was not
/// answered — of the search for `said`, for `what` (`videos`, `news`).
fn asked_note<T>(asked: &Asked<T>, said: &str, what: &str) -> Option<String> {
    match asked {
        Asked::NotAsked => Some(format!(
            "{PROVIDER} was not asked for {what} of \u{201c}{said}\u{201d}."
        )),
        Asked::Asking => Some(format!(
            "Searching {PROVIDER} for {what} of \u{201c}{said}\u{201d}\u{2026}"
        )),
        Asked::Found(found) if found.is_empty() => Some(format!(
            "{PROVIDER} found no {what} for \u{201c}{said}\u{201d}."
        )),
        Asked::Found(_) => None,
        Asked::Unanswered(why) => Some(why.clone()),
    }
}

/// The first letter or digit of `name`, as a capital.
pub(crate) fn letter(name: &str) -> Option<String> {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|first| first.to_uppercase().collect())
}

/// One of four tints, the same for a host every time.
pub(crate) fn tint(url: &Url) -> usize {
    let host = alo_url::reading::host(url);
    let sum = host
        .bytes()
        .fold(0usize, |sum, byte| sum.wrapping_add(usize::from(byte)));
    sum % 4
}

/// `turns` as sent to the renderer: each search's results as drawn
/// ([`Results::as_drawn`]), then pictures taken from the oldest turns' until
/// what is left weighs at most [`MOST_PICTURE_BYTES`] — the newest turn,
/// the one a person is looking at, is the last to lose its own. A turn whose
/// pictures are taken shows each image as its title, and each site as its
/// letter. Within a turn the marks are weighed first: they are a kilobyte
/// each, and a site drawn with its letter is the larger loss.
pub fn within_budget(turns: &[Turn]) -> Vec<Turn> {
    let mut sent: Vec<Turn> = turns
        .iter()
        .map(|turn| match turn {
            Turn::Searched(results) => Turn::Searched(results.as_drawn()),
            other => other.clone(),
        })
        .collect();
    let mut left = MOST_PICTURE_BYTES;
    for turn in sent.iter_mut().rev() {
        let Turn::Searched(results) = turn else {
            continue;
        };
        let mut keep = |picture: &mut Option<Vec<u8>>| {
            let weighs = picture.as_ref().map_or(0, Vec::len);
            if weighs > left {
                *picture = None;
            } else {
                left -= weighs;
            }
        };
        if let Showing::Found(found) = &mut results.showing {
            for site in found.iter_mut() {
                keep(&mut site.mark);
            }
        }
        if let Videos::Found(found) = &mut results.videos {
            for video in found.iter_mut() {
                keep(&mut video.mark);
            }
        }
        if let Stories::Found(found) = &mut results.news {
            for story in found.iter_mut() {
                keep(&mut story.mark);
            }
        }
        if let Images::Found(found) = &mut results.images {
            for image in found.iter_mut() {
                keep(&mut image.picture);
            }
        }
        if let Videos::Found(found) = &mut results.videos {
            for video in found.iter_mut() {
                keep(&mut video.picture);
            }
        }
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(address: &str) -> Found {
        Found::new("A page", alo_url::parse(address).unwrap(), "Said of it.")
    }

    #[test]
    fn a_site_is_named_by_the_provider_or_by_its_host() {
        let plain = found("https://evhomeguide.org/installing-a-charger/");
        assert_eq!(plain.site(), "evhomeguide.org");
        assert_eq!(plain.letter().as_deref(), Some("E"));
        let named = plain.clone().named("  EV Home Guide ");
        assert_eq!(named.site(), "EV Home Guide");
        assert_eq!(
            named.address(),
            "evhomeguide.org \u{203a} installing-a-charger"
        );
        assert_eq!(found("https://example.com/").address(), "example.com");
        assert!(plain.tint() < 4);
        assert_eq!(plain.tint(), found("https://evhomeguide.org/other").tint());
    }

    fn image(n: u8, bytes: usize) -> Pictured {
        Pictured::new(
            format!("{n}"),
            alo_url::parse("https://p.example/").unwrap(),
            "",
        )
        .holding(vec![n; bytes])
    }

    fn with_images(count: u8, bytes: usize) -> Results {
        Results::of("x", Showing::Found(vec![found("https://a.example/"); 4]))
            .with_images(Images::Found((0..count).map(|n| image(n, bytes)).collect()))
    }

    #[test]
    fn each_tab_draws_its_own_sites_and_images_and_opens_only_when_it_can() {
        let all = with_images(8, 10);
        assert_eq!(all.drawn_sites().len(), ON_ALL);
        assert_eq!(all.drawn_images().len(), IMAGES_ON_ALL);
        let sites = all.clone().on(Tab::Sites);
        assert_eq!(sites.drawn_sites().len(), 4);
        assert!(sites.drawn_images().is_empty());
        let images = all.clone().on(Tab::Images);
        assert!(images.drawn_sites().is_empty());
        assert_eq!(images.drawn_images().len(), 8);
        assert!(all.open(Tab::Sites) && all.open(Tab::Images));
        let searching = Results::searching("x");
        assert!(searching.open(Tab::All));
        assert!(!searching.open(Tab::Sites) && !searching.open(Tab::Images));
        let asking = Results::of("x", Showing::Found(vec![found("https://a.example/")]))
            .with_images(Images::Asking);
        assert!(asking.open(Tab::Images));
        assert!(asking.drawn_images().is_empty());
        for place in 0..TABS.len() {
            assert_eq!(
                Tab::at(place).map(Tab::place),
                Tab::at(place).map(|_| place)
            );
        }
        assert_eq!(Tab::at(1), None, "Answer is not built");
        assert!(all.open(Tab::Videos) && all.open(Tab::News));
        assert!(!searching.open(Tab::Videos) && !searching.open(Tab::News));
        let unanswered = Results::of("x", Showing::Unanswered("no".to_owned()));
        assert!(!unanswered.open(Tab::Videos) && !unanswered.open(Tab::News));
        let nothing = Results::of("x", Showing::Found(Vec::new()));
        assert!(!nothing.open(Tab::Videos), "a search that found nothing");
    }

    fn video(n: u8, bytes: usize) -> Video {
        Video::new(
            format!("{n}"),
            alo_url::parse("https://v.example/").unwrap(),
        )
        .holding(vec![n; bytes])
    }

    #[test]
    fn videos_and_stories_are_drawn_only_on_their_own_tabs_and_say_why_when_none() {
        let story = Story::new("S", alo_url::parse("https://n.example/").unwrap());
        let results = with_images(2, 10)
            .with_videos(Videos::Found((0..3).map(|n| video(n, 7)).collect()))
            .with_news(Stories::Found(vec![story]));
        assert!(results.drawn_videos().is_empty() && results.drawn_stories().is_empty());
        let videos = results.clone().on(Tab::Videos);
        assert_eq!(videos.drawn_videos().len(), 3);
        assert!(videos.drawn_images().is_empty() && videos.drawn_sites().is_empty());
        assert_eq!(videos.as_drawn().picture_bytes(), 21, "the videos alone");
        assert_eq!(results.as_drawn().picture_bytes(), 20, "All's row alone");
        let news = results.on(Tab::News);
        assert_eq!(news.drawn_stories().len(), 1);
        assert_eq!(news.as_drawn().picture_bytes(), 0);
        let mut asked = Results::of("ev", Showing::Found(vec![found("https://a.example/")]));
        assert_eq!(
            asked.videos_note().as_deref(),
            Some("Brave Search was not asked for videos of \u{201c}ev\u{201d}.")
        );
        asked.videos = Videos::Asking;
        assert_eq!(
            asked.videos_note().as_deref(),
            Some("Searching Brave Search for videos of \u{201c}ev\u{201d}\u{2026}")
        );
        asked.news = Stories::Found(Vec::new());
        assert_eq!(
            asked.news_note().as_deref(),
            Some("Brave Search found no news for \u{201c}ev\u{201d}.")
        );
        asked.news = Stories::Unanswered("Brave Search could not answer just now (502).".into());
        assert_eq!(
            asked.news_note().as_deref(),
            Some("Brave Search could not answer just now (502).")
        );
        // Videos' pictures count against the first screen's budget too.
        let heavy = || {
            Turn::Searched(
                with_images(0, 0)
                    .with_videos(Videos::Found(
                        (0..20).map(|n| video(n, 50 * 1024)).collect(),
                    ))
                    .on(Tab::Videos),
            )
        };
        let weights: Vec<usize> = within_budget(&[heavy(), heavy(), heavy()])
            .iter()
            .map(|turn| match turn {
                Turn::Searched(results) => results.picture_bytes(),
                _ => 0,
            })
            .collect();
        assert_eq!(weights, vec![0, 1_024_000, 1_024_000]);
    }

    #[test]
    fn only_the_pictures_a_tab_draws_are_sent_and_the_newest_turns_first() {
        let all = with_images(8, 10).as_drawn();
        assert_eq!(all.picture_bytes(), 50, "the row's five");
        assert_eq!(
            with_images(8, 10).on(Tab::Sites).as_drawn().picture_bytes(),
            0
        );
        assert_eq!(
            with_images(8, 10)
                .on(Tab::Images)
                .as_drawn()
                .picture_bytes(),
            80
        );
        // Three searches of twenty pictures of 50 KiB on Images: a megabyte
        // each, and two fit.
        let heavy = || Turn::Searched(with_images(20, 50 * 1024).on(Tab::Images));
        let sent = within_budget(&[heavy(), heavy(), heavy()]);
        let weights: Vec<usize> = sent
            .iter()
            .map(|turn| match turn {
                Turn::Searched(results) => results.picture_bytes(),
                _ => 0,
            })
            .collect();
        assert_eq!(weights, vec![0, 1_024_000, 1_024_000]);
        assert!(weights.iter().sum::<usize>() <= MOST_PICTURE_BYTES);
    }

    #[test]
    fn a_mark_is_sent_only_where_its_site_is_drawn_and_weighed_before_pictures() {
        let marked = |sites: usize| {
            Results::of(
                "x",
                Showing::Found(
                    (0..sites)
                        .map(|_| found("https://a.example/").marked(vec![1; 100]))
                        .collect(),
                ),
            )
            .with_videos(Videos::Found(vec![video(1, 10).marked(vec![2; 7])]))
            .with_news(Stories::Found(vec![
                Story::new("S", alo_url::parse("https://s.example/").unwrap()).marked(vec![3; 5]),
            ]))
        };
        assert_eq!(marked(5).picture_bytes(), 500 + 10 + 7 + 5);
        assert_eq!(marked(5).as_drawn().picture_bytes(), 300, "All's three");
        assert_eq!(marked(5).on(Tab::Sites).as_drawn().picture_bytes(), 500);
        assert_eq!(
            marked(5).on(Tab::Videos).as_drawn().picture_bytes(),
            10 + 7,
            "a video's picture and its mark"
        );
        assert_eq!(marked(5).on(Tab::News).as_drawn().picture_bytes(), 5);
        // Twenty marks of 16 KiB and twenty pictures of 100 KiB on one turn:
        // every mark is kept, and the pictures that fit after them.
        let heavy = Results::of(
            "x",
            Showing::Found(
                (0..20)
                    .map(|_| found("https://a.example/").marked(vec![1; LARGEST_MARK]))
                    .collect(),
            ),
        )
        .with_images(Images::Found(
            (0..20).map(|n| image(n, 100 * 1024)).collect(),
        ))
        .on(Tab::Sites);
        let sent = within_budget(&[Turn::Searched(heavy.clone()), Turn::Searched(heavy)]);
        let Some(Turn::Searched(newest)) = sent.last() else {
            panic!("the newest turn is a search");
        };
        let Showing::Found(sites) = &newest.showing else {
            panic!("its sites");
        };
        assert!(sites.iter().all(|site| site.mark.is_some()));
        assert_eq!(newest.picture_bytes(), 20 * LARGEST_MARK);
        let Some(Turn::Searched(oldest)) = sent.first() else {
            panic!("the oldest turn is a search");
        };
        assert!(
            oldest.picture_bytes() + newest.picture_bytes() <= MOST_PICTURE_BYTES,
            "marks count in the budget"
        );
    }

    #[test]
    fn the_images_tab_says_why_when_there_are_none() {
        let mut results = Results::searching("ev");
        assert_eq!(
            results.images_note().as_deref(),
            Some("Images were not searched for \u{201c}ev\u{201d}.")
        );
        results.images = Images::Found(Vec::new());
        assert_eq!(
            results.images_note().as_deref(),
            Some("Brave Search found no images for \u{201c}ev\u{201d}.")
        );
        results.images = Images::Unanswered("Brave Search could not answer just now (503).".into());
        assert_eq!(
            results.images_note().as_deref(),
            Some("Brave Search could not answer just now (503).")
        );
        results.images = Images::Found(vec![image(1, 1)]);
        assert_eq!(results.images_note(), None);
    }

    #[test]
    fn all_shows_the_top_three_and_says_why_when_there_are_none() {
        let mut results = Results::searching("rust borrow checker");
        assert!(results.on_all().is_empty());
        assert_eq!(
            results.note().as_deref(),
            Some("Searching Brave Search for \u{201c}rust borrow checker\u{201d}\u{2026}")
        );
        results.showing = Showing::Found(vec![found("https://a.example/"); 5]);
        assert_eq!(results.on_all().len(), ON_ALL);
        assert_eq!(results.note(), None);
        results.showing = Showing::Found(vec![found("https://a.example/")]);
        assert_eq!(results.on_all().len(), 1);
        results.showing = Showing::Found(Vec::new());
        assert_eq!(
            results.note().as_deref(),
            Some("Brave Search found no sites for \u{201c}rust borrow checker\u{201d}.")
        );
        results.showing =
            Showing::Unanswered("Brave Search could not answer just now (500).".into());
        assert!(results.on_all().is_empty());
        assert_eq!(
            results.note().as_deref(),
            Some("Brave Search could not answer just now (500).")
        );
    }
}

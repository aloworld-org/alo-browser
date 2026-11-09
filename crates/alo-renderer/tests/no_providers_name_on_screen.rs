/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 900, the owner's word of 9 November 2026: *"we do not need
//! to show any brave words on the ui or to the users"*. alo still asks the
//! Brave Search API; no word of its name reaches anything a person sees.
//!
//! Every search a turn can be in — searching, nothing found, sites found,
//! not answered — on every tab, with its images, videos, news and answer
//! in each of theirs, the offer to search set up and not, and *Settings*
//! open over it: drawn by a renderer, and its whole document (every text,
//! every attribute, an accessible name being one) and every node the agent
//! reads asked for the name, in any case. What the browser process says
//! instead of an answer is its own crate's test (`alo-window`'s
//! `brave_search`, `agent_found`, `conversation_box`).

use alo_layout::Size;
use alo_renderer::results::{Found, Images, Results, SetUp, Showing, Stories, Tab, Videos};
use alo_renderer::results_answer::{Answer, Cited, Written};
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::typed::Offer;
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The provider's name, as a lower-case word.
const NAME: &str = "brave";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (weight, bytes) in [
        (Weight::NORMAL, dejavu::sans::regular()),
        (Weight::BOLD, dejavu::sans::bold()),
    ] {
        if let Some(font) = Font::load("DejaVu Sans", weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for generic in ["system-ui", "sans-serif", "serif"] {
        database.map_generic(generic, "DejaVu Sans");
    }
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

fn url(address: &str) -> alo_url::Url {
    alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
}

fn sites() -> Vec<Found> {
    (0..4)
        .map(|n| {
            Found::new(
                format!("Site {n}"),
                url(&format!("https://site{n}.example/")),
                "What the page says.",
            )
        })
        .collect()
}

fn written() -> Written {
    Written {
        line: "Half a day.".to_owned(),
        sentences: vec![Cited::new("Half a day to fit.", [1])],
        points: vec![Cited::new("A permit may be needed.", [2])],
    }
}

/// Every state a turn's results are drawn in, one search apiece.
fn every_search() -> Vec<Results> {
    let said = "ev charger";
    let mut all = Vec::new();
    for showing in [
        Showing::Searching,
        Showing::Found(Vec::new()),
        Showing::Found(sites()),
        Showing::Unanswered("not answered".to_owned()),
    ] {
        all.push(Results::of(said, showing));
    }
    let found = || Results::of(said, Showing::Found(sites()));
    for images in [Images::NotAsked, Images::Asking, Images::Found(Vec::new())] {
        all.push(found().with_images(images));
    }
    for videos in [Videos::NotAsked, Videos::Asking, Videos::Found(Vec::new())] {
        all.push(found().with_videos(videos));
    }
    for news in [
        Stories::NotAsked,
        Stories::Asking,
        Stories::Found(Vec::new()),
    ] {
        all.push(found().with_news(news));
    }
    for answer in [Answer::Asking, Answer::NotFound, Answer::Written(written())] {
        all.push(found().with_answer(answer));
    }
    all
}

fn screen(results: Results, search: SetUp, settings: bool) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(1040.0, 820.0),
        typed: "ev charger".to_owned(),
        focused: true,
        offers: vec![
            Offer::Search("ev charger".to_owned()),
            Offer::Ask("ev charger".to_owned()),
        ],
        turns: vec![Turn::Searched(results)],
        search,
        settings,
        ..Start::default()
    }
}

/// What of `start`, drawn, names the provider: each place it is said.
fn naming(start: Start) -> Vec<String> {
    let mut renderer = Renderer::new(fonts());
    let mut said = Vec::new();
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    if !matches!(renderer.handle(ToRenderer::Paint), FromRenderer::Painted(_)) {
        said.push("nothing was painted".to_owned());
    }
    match renderer.document() {
        Some(document) => {
            let whole = document.serialize_children(document.root());
            if whole.to_lowercase().contains(NAME) {
                said.push(format!("the document: {whole}"));
            }
        }
        None => said.push("no document was drawn".to_owned()),
    }
    if let FromRenderer::Tree(tree) = renderer.handle(ToRenderer::ReadTree) {
        for node in tree.nodes() {
            for words in [&node.name, &node.placeholder, &node.marker]
                .into_iter()
                .flatten()
            {
                if words.to_lowercase().contains(NAME) {
                    said.push(format!("{:?}: {words}", node.role));
                }
            }
        }
    } else {
        said.push("no tree was read".to_owned());
    }
    said
}

#[test]
fn no_search_on_any_tab_names_the_provider() {
    let mut drawn = 0;
    for results in every_search() {
        for tab in [
            Tab::All,
            Tab::Answer,
            Tab::Sites,
            Tab::Images,
            Tab::Videos,
            Tab::News,
        ] {
            let said = naming(screen(results.clone().on(tab), SetUp::Ready, false));
            assert!(said.is_empty(), "{tab:?} of {results:?}: {said:?}");
            drawn += 1;
        }
    }
    assert_eq!(drawn, 16 * 6, "every search on every tab");
}

#[test]
fn the_offer_to_search_and_settings_name_no_provider() {
    for search in [
        SetUp::Ready,
        SetUp::Not(String::new()),
        SetUp::Not("no search key in this Mac's keychain".to_owned()),
    ] {
        for settings in [false, true] {
            let said = naming(screen(
                Results::searching("ev charger"),
                search.clone(),
                settings,
            ));
            assert!(said.is_empty(), "{search:?}, settings {settings}: {said:?}");
        }
    }
}

#[test]
fn what_the_tabs_say_instead_names_no_provider() {
    for results in every_search() {
        let notes = [
            results.note(),
            results.images_note(),
            results.videos_note(),
            results.news_note(),
            results.answer.note(),
        ];
        for note in notes.into_iter().flatten() {
            assert!(!note.to_lowercase().contains(NAME), "{note}");
        }
    }
    for given in 1..=4 {
        let sent = alo_renderer::results_answer::sent(given);
        assert!(!sent.to_lowercase().contains(NAME), "{sent}");
        assert!(
            sent.contains("OpenAI"),
            "who it was sent to is said: {sent}"
        );
    }
    for search in [SetUp::Ready, SetUp::Not(String::new())] {
        let under = alo_renderer::start_document::searched_with(&search);
        assert!(!under.to_lowercase().contains(NAME), "{under}");
    }
}

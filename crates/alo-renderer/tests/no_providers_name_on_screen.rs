/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 900, the owner's word of 9 November 2026: *"we do not need
//! to show any brave words on the ui or to the users"*. alo still asks the
//! Brave Search API; no word of its name reaches anything a person sees —
//! and since ADR 0106 § 5 (queue items 904, 905) no service's name does,
//! search or AI (`Brave`, `SearXNG`, `OpenAI`), **but in one place**:
//! *Settings › Search and answers*' choosers, where a service is chosen by
//! its own name. There a name is allowed inside a chooser's option (`class
//! svc…`) and as the name of the radio in it, and only its own; anywhere
//! else in that section — its heading, lines, fields, states — it is not.
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
use alo_renderer::settings_services::{Entry, EntryRow, ServicesShown, Typed, dots};
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::typed::Offer;
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// Every service's name, as a lower-case word.
const NAMES: [&str; 3] = ["brave", "openai", "searxng"];

/// The services' own names, as *Settings*' choosers say them: the one
/// place they are drawn.
const CHOSEN_BY: [&str; 3] = ["Brave Search", "SearXNG", "OpenAI"];

/// Whether `said` names any service, in any case.
fn names_one(said: &str) -> bool {
    let said = said.to_lowercase();
    NAMES.iter().any(|name| said.contains(name))
}

/// *Search and answers* as the browser process sends it, in each state a
/// row can say: every service, a field of each kind, a key kept.
fn services(on: usize, said: Option<&str>) -> ServicesShown {
    ServicesShown {
        search: vec![CHOSEN_BY[0].to_owned(), CHOSEN_BY[1].to_owned()],
        search_on: on,
        answers: vec![CHOSEN_BY[2].to_owned()],
        answers_on: 0,
        search_rows: vec![if on == 0 {
            EntryRow {
                entry: Entry::SearchKey,
                typed: Typed {
                    shown: dots(8),
                    focused: true,
                },
                state: "The key this application carries is used.".to_owned(),
                removable: false,
            }
        } else {
            EntryRow {
                entry: Entry::Address,
                typed: Typed::default(),
                state: "No address yet.".to_owned(),
                removable: false,
            }
        }],
        answers_rows: vec![EntryRow {
            entry: Entry::AiKey,
            typed: Typed::default(),
            state: "Your own key is used, kept in this Mac\u{2019}s keychain.".to_owned(),
            removable: true,
        }],
        said: said.map(str::to_owned),
    }
}

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
        services: settings.then(|| services(0, None)),
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
        Some(document) => said.extend(named_outside_a_chooser(document)),
        None => said.push("no document was drawn".to_owned()),
    }
    if let FromRenderer::Tree(tree) = renderer.handle(ToRenderer::ReadTree) {
        for node in tree.nodes() {
            for words in [&node.name, &node.placeholder, &node.marker]
                .into_iter()
                .flatten()
            {
                // A chooser's radio is named by its service, and that alone.
                let chooser = node.role.as_str() == "radio"
                    && node.name.as_deref() == Some(words.as_str())
                    && CHOSEN_BY.contains(&words.as_str());
                if names_one(words) && !chooser {
                    said.push(format!("{:?}: {words}", node.role));
                }
            }
        }
    } else {
        said.push("no tree was read".to_owned());
    }
    said
}

/// Every place in `document` a service is named outside a chooser's
/// option: a text, or an attribute's value. Inside one, only its own name.
fn named_outside_a_chooser(document: &alo_dom::Document) -> Vec<String> {
    let mut said = Vec::new();
    for node in document.descendants(document.root()) {
        let option =
            core::iter::successors(Some(node), |&each| document.parent(each)).find(|&each| {
                document
                    .attribute_by_name(each, "class")
                    .is_some_and(|class| class.split(' ').next() == Some("svc"))
            });
        let mut words: Vec<String> = Vec::new();
        if let Some(alo_dom::NodeKind::Text(text)) = document.kind(node) {
            words.push(text.clone());
        }
        if let Some(element) = document.element(node) {
            words.extend(
                element
                    .attrs
                    .iter()
                    .map(|attribute| attribute.value.clone()),
            );
        }
        for word in words.into_iter().filter(|word| names_one(word)) {
            let own = option.is_some_and(|option| {
                let named = document.text_content(option);
                CHOSEN_BY.contains(&word.as_str()) && named.trim() == word
            });
            if !own {
                said.push(format!("{word:?} outside its chooser"));
            }
        }
    }
    said
}

#[test]
fn search_and_answers_names_a_service_in_its_chooser_alone() {
    for on in [0, 1] {
        for said in [
            None,
            Some("Your key for the search service is kept in this Mac\u{2019}s keychain."),
            Some("That is not a site\u{2019}s address: it begins https:// or http://."),
        ] {
            let start = Start {
                services: Some(services(on, said)),
                ..screen(Results::searching("ev charger"), SetUp::Ready, true)
            };
            let found = naming(start.clone());
            assert!(found.is_empty(), "{on} {said:?}: {found:?}");
            // And the names are there, where a person chooses.
            let mut renderer = Renderer::new(fonts());
            let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
            let _ = renderer.handle(ToRenderer::Paint);
            let text = renderer
                .document()
                .map(|document| document.text_content(document.root()))
                .unwrap_or_default();
            for name in CHOSEN_BY {
                assert_eq!(text.matches(name).count(), 1, "{name} once: {text}");
            }
        }
    }
    // The exception is narrow: a name anywhere else in the section is
    // found.
    let mut leaked = services(0, Some("Kept in SearXNG."));
    leaked.search_rows[0].state = "Brave Search's key is used.".to_owned();
    let found = naming(Start {
        services: Some(leaked),
        ..screen(Results::searching("ev charger"), SetUp::Ready, true)
    });
    // Each found twice: in the document, and in what an agent reads.
    assert_eq!(found.len(), 4, "{found:?}");
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
            assert!(!names_one(&note), "{note}");
        }
    }
    for given in 1..=4 {
        let sent = alo_renderer::results_answer::sent(given);
        assert!(!names_one(&sent), "{sent}");
        assert!(
            sent.contains("the AI service chosen in Settings"),
            "who it was sent to is said, by where it is chosen: {sent}"
        );
    }
    for search in [SetUp::Ready, SetUp::Not(String::new())] {
        let under = alo_renderer::start_document::searched_with(&search);
        assert!(!names_one(&under), "{under}");
    }
}

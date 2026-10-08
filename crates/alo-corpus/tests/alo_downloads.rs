/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's downloads page, in numbers: the page that opened queue item 319.
//!
//! `cases/alo-downloads` pins the whole page as files. This says out loud
//! the part item 319 is closed by: the note under the two cards is three
//! paragraphs' worth of text separated by `<br><br>`, and each `<br>` ends
//! its line. Before 319 the note was five lines with every sentence run
//! into the one before it; it is eight, two of them blank.
//!
//! It also pins what the page's script does today, because that is what
//! the render shows. Until item 74 it was refused at its first regular
//! expression, and until item 325 it stopped at its second line,
//! `navigator.platform`. Since item 327 it reads the `navigator` the
//! browser was told, marks that system's card `.rec` and shows its "Your
//! device" badge — for a Mac and a Windows machine, and neither on Linux.
//! Since item 329 its `querySelectorAll` finds the two buttons, and since
//! item 331 its `forEach` walks them. Since item 335 each button's
//! `fetch(href, { method: "HEAD" })` is asked for, and its `.then` or
//! `.catch` decides the button when the answer comes: an installer that is
//! there leaves its button as it is, and one that is not — a `404`, or no
//! answer at all — has it marked *Building — available shortly*, its
//! `href` taken away and, since item 342, its `style` set: the button's
//! `background` is `#c7bfb2`, written to its `style` attribute and cascaded
//! above `.btn`'s terracotta (item 341), and its `cursor` and
//! `pointerEvents` writes are ordinary properties of the declaration, since
//! no stage of this engine acts on either (ADR 0033 § 4). The script runs
//! to its end. The corpus froze no installer, so offline both buttons are
//! marked and greyed. The corpus renders it as the Mac `alo_corpus::SYSTEM`
//! says it is.

use alo_corpus::{Case, Rendering, cases_directory, corpus_fonts};
use alo_layout::Rect;
use alo_net::cors::{Credentials, Mode};
use alo_renderer::fetch::{Answer, FetchAsk, Fetched, Kind, Readable};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};

/// Where the page is served from: alo's own deploy serves its downloads
/// directory under `/download/` on `alomails.com`.
const ADDRESS: &str = "https://alomails.com/download/";

/// The two installers the page's buttons name, as their `href`s resolve
/// against [`ADDRESS`].
const INSTALLERS: [&str; 2] = [
    "https://alomails.com/download/alomails-windows-x64-setup.exe",
    "https://alomails.com/download/alomails-mac-universal.dmg",
];

/// What a marked button says.
const MARKED: &str = "Building — available shortly";

/// The `style` attribute `mark(a)` leaves on a button it greys (item 342):
/// its `background`, serialised as written. Its `cursor` and
/// `pointerEvents` are not in it, because no stage acts on either.
const GREYED: &str = "background: #c7bfb2;";

/// The note's line height: `0.86rem` at `line-height: 1.55` from `body`.
const NOTE_LINE: f32 = 0.86 * 16.0 * 1.55;

/// The case, read and rendered.
fn downloads() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-downloads"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The note's border box, the border boxes of the `<br>`s in it in document
/// order, and the box of the text that starts "Prefer the web?".
fn the_note(rendering: &Rendering) -> Option<(Rect, Vec<Rect>, Rect)> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let boxes = &drawing.boxes;
    let note = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. })
                if document
                    .element(*node)
                    .is_some_and(|element| element.attr("class") == Some("note"))
        )
    })?;
    let inside = boxes.descendants(note);
    let breaks = inside
        .iter()
        .filter(|id| boxes.is_forced_break(**id))
        .map(|id| drawing.layout.get(*id).map(|geometry| geometry.border_box))
        .collect::<Option<Vec<Rect>>>()?;
    let prefer = inside.iter().find(|id| {
        boxes
            .get(**id)
            .and_then(alo_box::BoxNode::text)
            .is_some_and(|text| text.trim_start().starts_with("Prefer the web?"))
    })?;
    let prefer = drawing.layout.get(*prefer)?.border_box;
    Some((drawing.layout.get(note)?.border_box, breaks, prefer))
}

/// The lede is `max-width: 44ch` at `1.08rem`, and `ch` is the advance of
/// the face's `0` (queue item 320): 44 of the corpus sans-serif's at
/// 17.28 px, where half an em made it 380.16.
#[test]
fn the_lede_is_forty_four_of_the_faces_zeros() {
    let Some((_, rendering)) = downloads() else {
        panic!("the case renders");
    };
    let Some(zero) = alo_text::Font::load(
        "DejaVu Sans",
        alo_text::Weight::NORMAL,
        alo_text::Slant::Normal,
        dejavu::sans::regular().to_vec(),
    )
    .map(|font| font.metrics(1.08 * 16.0).zero_width) else {
        panic!("DejaVu Sans loads");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the page is drawn");
    };
    let boxes = &drawing.boxes;
    let Some(lede) = boxes
        .ids()
        .find(|id| {
            matches!(
                boxes.get(*id).map(|node| &node.kind),
                Some(alo_box::BoxKind::Element { node, .. })
                    if document
                        .element(*node)
                        .is_some_and(|element| element.attr("class") == Some("lede"))
            )
        })
        .and_then(|id| drawing.layout.get(id))
    else {
        panic!("the lede is laid out");
    };
    let width = lede.border_box.size.width;
    assert!(
        (width - 44.0 * zero).abs() < 0.001,
        "{width} is not 44 × {zero}"
    );
    assert!((width - 483.7388).abs() < 0.001, "{width}");
}

/// Each download button is `display: inline-flex` with its label as its
/// only child, and the label is wrapped in an anonymous block (queue item
/// 321): its line is the body's `line-height: 1.55` at 16 px, 24.8, and the
/// button that line plus twelve pixels of padding above and below, 48.8.
/// Measured bare, the label was its font's 18.625 and the button 42.625.
#[test]
fn each_buttons_label_is_a_line_as_tall_as_its_line_height() {
    let Some((_, rendering)) = downloads() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the page is drawn");
    };
    let boxes = &drawing.boxes;
    // Offline, both buttons are marked (item 335), and a marked button's
    // label is a line like any other.
    let labels: Vec<alo_box::BoxId> = boxes
        .ids()
        .filter(|id| {
            boxes
                .get(*id)
                .and_then(alo_box::BoxNode::text)
                .is_some_and(|held| held.trim() == MARKED)
        })
        .collect();
    assert_eq!(labels.len(), 2, "both buttons are marked");
    for text in labels {
        let label = MARKED;
        let wrapper = boxes.get(text).and_then(|node| node.parent);
        assert!(
            matches!(
                wrapper.and_then(|id| boxes.get(id)).map(|node| &node.kind),
                Some(alo_box::BoxKind::Anonymous { .. })
            ),
            "{label} sits in a box nobody wrote",
        );
        let button = wrapper
            .and_then(|id| boxes.get(id))
            .and_then(|node| node.parent);
        let height = |id: Option<alo_box::BoxId>| {
            id.and_then(|id| drawing.layout.get(id))
                .map(|geometry| geometry.border_box.size.height)
        };
        let line = height(wrapper).unwrap_or(f32::NAN);
        assert!(
            (line - 16.0 * 1.55).abs() < 0.001,
            "{label}'s line is {line}"
        );
        let tall = height(button).unwrap_or(f32::NAN);
        assert!(
            (tall - (16.0 * 1.55 + 24.0)).abs() < 0.001,
            "{label}'s button is {tall}"
        );
    }
}

#[test]
fn each_br_in_the_note_ends_its_line() {
    let Some((_, rendering)) = downloads() else {
        panic!("the case renders");
    };
    let Some((note, breaks, prefer)) = the_note(&rendering) else {
        panic!("the note, its breaks and its second sentence are laid out");
    };
    assert_eq!(breaks.len(), 4, "two pairs of <br>");

    // Eight lines under a 1 px border and 18 px of padding: two of the first
    // sentence, a blank one, the second sentence, a blank one, and three of
    // the third. Before item 319 it was five lines, 125.64 tall.
    let tall = 1.0 + 18.0 + 8.0 * NOTE_LINE;
    assert!(
        (note.size.height - tall).abs() < 0.01,
        "the note is {} tall, not {tall}",
        note.size.height,
    );

    // The breaks are one line apart: the first ends the first sentence's
    // second line, the second is a blank line of its own, the third ends the
    // second sentence and the fourth is the second blank line.
    for pair in breaks.windows(2) {
        let [above, below] = pair else { continue };
        assert!(
            (below.top() - above.top() - NOTE_LINE).abs() < 0.01,
            "{above:?} then {below:?}",
        );
    }
    // A break takes no room across its line. The two alone on theirs stand
    // at the note's left edge, which is the page's 40 px gutter.
    assert!(breaks.iter().all(|held| held.size.width == 0.0));
    let alone: Vec<f32> = [breaks.get(1), breaks.get(3)]
        .into_iter()
        .flatten()
        .map(|held| held.left())
        .collect();
    assert_eq!(alone, [40.0, 40.0]);

    // The second sentence starts a line of its own, at the left edge and not
    // after its leading space, one line under the blank one. It is in the
    // same font as the breaks, so it sits in its line exactly as they do.
    assert!(
        (prefer.left() - 40.0).abs() < 0.001,
        "\"Prefer the web?\" starts at {}",
        prefer.left(),
    );
    let Some(blank) = breaks.get(1) else {
        panic!("a second break");
    };
    assert!(
        (prefer.top() - blank.top() - NOTE_LINE).abs() < 0.01,
        "{prefer:?} under {blank:?}",
    );
}

/// ADR 0030 § 2's rows, and which card each marks: the Mac's, the
/// Windows machine's, or — on Linux, where neither of the script's branches
/// runs — none.
const SYSTEMS: [(&str, &str, Option<&str>); 3] = [
    (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) alo/0.0",
        "MacIntel",
        Some("mac"),
    ),
    (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) alo/0.0",
        "Win32",
        Some("win"),
    ),
    (
        "Mozilla/5.0 (X11; Linux x86_64) alo/0.0",
        "Linux x86_64",
        None,
    ),
];

/// Whether the element with `id` is marked as the visitor's, in the
/// document — its card's `class` has `rec` and its badge no `hidden` — and
/// in the box tree: the badge has a box only when it is shown.
fn marked(renderer: &Renderer, id: &str) -> Option<(bool, bool, bool)> {
    let document = renderer.document()?;
    let element = |wanted: &str| {
        document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("id") == Some(wanted))
        })
    };
    let card = element(&format!("card-{id}"))?;
    let badge = element(&format!("badge-{id}"))?;
    let rec = document
        .element(card)?
        .attr("class")?
        .split_ascii_whitespace()
        .any(|token| token == "rec");
    let shown = document.element(badge)?.attr("hidden").is_none();
    let boxes = &renderer.rendered()?.boxes;
    let drawn = boxes.ids().any(|held| {
        matches!(
            boxes.get(held).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. }) if *node == badge
        )
    });
    Some((rec, shown, drawn))
}

/// What a load said, and what it asked to fetch.
type Load = (Vec<String>, Vec<FetchAsk>);

/// The page `case` holds, loaded at [`ADDRESS`] as the system `user_agent`
/// and `platform` say: the renderer, what the load said, and what it asked
/// to fetch — [`None`] if it did not load.
fn loaded(case: &Case, user_agent: &str, platform: &str) -> Option<(Renderer, Load)> {
    let mut renderer = Renderer::new(corpus_fonts());
    let mut page = Page::new(case.html.clone(), alo_layout::Size::new(800.0, 780.0));
    user_agent.clone_into(&mut page.user_agent);
    platform.clone_into(&mut page.platform);
    page.url = alo_url::parse(ADDRESS).ok()?;
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded {
            issues, fetches, ..
        } => Some((renderer, (issues, fetches))),
        _ => None,
    }
}

/// The download button in the card `id`: its element.
fn button_node(document: &alo_dom::Document, id: &str) -> Option<alo_dom::NodeId> {
    let card = document.descendants(document.root()).find(|node| {
        document
            .element(*node)
            .is_some_and(|element| element.attr("id") == Some(&format!("card-{id}")))
    })?;
    let link = document.descendants(card).find(|node| {
        document.element(*node).is_some_and(|element| {
            element
                .attr("class")
                .is_some_and(|class| class.split_ascii_whitespace().any(|token| token == "btn"))
        })
    })?;
    Some(link)
}

/// The text of the download button in the card `id`, whether it still has
/// an `href`, and its `style` attribute.
fn button(renderer: &Renderer, id: &str) -> Option<(String, bool, Option<String>)> {
    let document = renderer.document()?;
    let link = button_node(document, id)?;
    let element = document.element(link)?;
    let has_href = element.attr("href").is_some();
    let style = element.attr("style").map(str::to_owned);
    Some((document.text_content(link), has_href, style))
}

/// The colour the download button in the card `id` is filled with, as the
/// display list holds it.
fn button_fill(renderer: &Renderer, id: &str) -> Option<String> {
    let link = button_node(renderer.document()?, id)?;
    let drawing = renderer.rendered()?;
    let boxes = &drawing.boxes;
    let held = boxes.ids().find(|held| {
        matches!(
            boxes.get(*held).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. }) if *node == link
        )
    })?;
    drawing.display.items().iter().find_map(|item| match item {
        alo_paint::DisplayItem::Fill {
            box_id,
            paint: alo_paint::Paint::Solid(colour),
            ..
        } if *box_id == held => Some(colour.to_string()),
        _ => None,
    })
}

/// Deliver `answer` to ask `number`, answering what the delivery said —
/// [`None`] if it was not answered as a delivery, or its reactions asked to
/// go somewhere or fetch again, or a policy objected, which this page's
/// never do.
fn deliver(renderer: &mut Renderer, number: u64, answer: Answer) -> Option<Vec<String>> {
    let fetched = Fetched { number, answer };
    match renderer.handle(ToRenderer::Fetched(Box::new(fetched))) {
        FromRenderer::Delivered {
            issues,
            objections,
            navigation: None,
            fetches,
        } if fetches.is_empty() && objections.is_empty() => Some(issues),
        _ => None,
    }
}

/// A `HEAD` answered by alo's own server, with `status`.
fn answered(url: &str, status: u16, status_text: &str) -> Answer {
    Answer::Response(Box::new(Readable {
        kind: Kind::Basic,
        status,
        status_text: status_text.to_owned(),
        url: Some(url.to_owned()),
        redirected: false,
        headers: Vec::new(),
        body: Vec::new(),
    }))
}

#[test]
fn the_pages_script_marks_the_card_of_the_system_it_is_told() {
    let Some(case) = Case::read(&cases_directory().join("alo-downloads")) else {
        panic!("the case is read");
    };
    for (user_agent, platform, chosen) in SYSTEMS {
        let Some((mut renderer, (issues, fetches))) = loaded(&case, user_agent, platform) else {
            panic!("{platform}: the page loads");
        };
        // It runs past both branches, every system alike, past the
        // `querySelectorAll` that finds the buttons (queue item 329) and the
        // `forEach` that walks them (queue item 331), and asks for each
        // installer with a `HEAD` (queue item 335) — and says nothing,
        // because nothing has failed yet.
        assert!(
            !issues.iter().any(|issue| issue.contains("uncaught")),
            "{platform}: {issues:?}",
        );
        let asked: Vec<(&str, &str)> = fetches
            .iter()
            .map(|ask| (ask.url.as_str(), ask.method.as_str()))
            .collect();
        assert_eq!(
            asked,
            [(INSTALLERS[0], "HEAD"), (INSTALLERS[1], "HEAD")],
            "{platform}"
        );
        for ask in &fetches {
            assert_eq!(
                (ask.mode, ask.credentials),
                (Mode::Cors, Credentials::SameOrigin)
            );
            assert!(ask.headers.is_empty() && ask.body.is_empty());
        }
        for id in ["mac", "win"] {
            let mark = chosen == Some(id);
            assert_eq!(
                marked(&renderer, id),
                Some((mark, mark, mark)),
                "{platform}: the {id} card is marked {mark}",
            );
        }
        // Offline: each answer is a network error, and its `.catch` marks
        // and greys the button, running to its end with nothing to say.
        for ask in &fetches {
            assert_eq!(
                deliver(&mut renderer, ask.number, Answer::NetworkError),
                Some(Vec::new()),
                "{platform}"
            );
        }
        for id in ["win", "mac"] {
            assert_eq!(
                button(&renderer, id),
                Some((MARKED.to_owned(), false, Some(GREYED.to_owned()))),
                "{platform}: the {id} button"
            );
        }
    }
}

/// An installer that is there leaves its button as it is; one that is not
/// has it marked by the `.then`, which reads `r.ok` — the same page, told
/// two different things by alo's server.
#[test]
fn each_buttons_then_decides_it_from_its_answer() {
    let Some(case) = Case::read(&cases_directory().join("alo-downloads")) else {
        panic!("the case is read");
    };
    let (user_agent, platform, _) = SYSTEMS[0];
    let Some((mut renderer, (_, fetches))) = loaded(&case, user_agent, platform) else {
        panic!("the page loads");
    };
    let [windows, mac] = fetches.as_slice() else {
        panic!("two asks: {fetches:?}");
    };
    let issues = deliver(
        &mut renderer,
        windows.number,
        answered(INSTALLERS[0], 200, "OK"),
    );
    assert_eq!(issues, Some(Vec::new()));
    let issues = deliver(
        &mut renderer,
        mac.number,
        answered(INSTALLERS[1], 404, "Not Found"),
    );
    assert_eq!(issues, Some(Vec::new()));
    assert_eq!(
        button(&renderer, "win"),
        Some(("Download for Windows".to_owned(), true, None))
    );
    assert_eq!(
        button(&renderer, "mac"),
        Some((MARKED.to_owned(), false, Some(GREYED.to_owned())))
    );
    // Each answer is delivered once; a second is answered by nobody.
    let again = deliver(&mut renderer, mac.number, Answer::NetworkError).unwrap_or_default();
    assert!(
        again.len() == 1 && again[0].contains("nothing on this page is waiting"),
        "{again:?}"
    );
}

/// The corpus froze no installer — they are built by CI into a directory
/// alo's deploy mounts, and are in no repository — so both fetches are
/// network errors, and `origin.txt` says which.
#[test]
fn the_corpus_answers_it_offline_and_says_so() {
    let Some((case, Rendering::Loaded(renderer, answered))) = downloads() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(case.address.as_deref(), Some(ADDRESS));
    assert!(case.responses.is_empty());
    assert_eq!(answered.delivered, 2);
    assert_eq!(answered.unfrozen, INSTALLERS);
    // The script runs to its end: nothing it does is refused.
    assert!(answered.issues.is_empty(), "{:?}", answered.issues);
    for id in ["win", "mac"] {
        assert_eq!(
            button(&renderer, id),
            Some((MARKED.to_owned(), false, Some(GREYED.to_owned())))
        );
        // Drawn `#c7bfb2`, the `style` attribute's, over `.btn`'s
        // terracotta `rgb(231 111 81)`.
        assert_eq!(
            button_fill(&renderer, id).as_deref(),
            Some("rgb(199 191 178)"),
            "the {id} button"
        );
    }
    let origin = std::fs::read_to_string(case.expectation("origin.txt")).unwrap_or_default();
    for url in INSTALLERS {
        assert!(origin.contains(url), "origin.txt does not say {url}");
    }
}

#[test]
fn the_corpus_renders_it_as_the_mac_it_says_it_is() {
    let Some((_, Rendering::Loaded(renderer, _))) = downloads() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(alo_corpus::SYSTEM.platform, "MacIntel");
    assert_eq!(marked(&renderer, "mac"), Some((true, true, true)));
    assert_eq!(marked(&renderer, "win"), Some((false, false, false)));
}

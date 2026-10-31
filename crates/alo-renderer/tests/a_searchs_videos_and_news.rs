/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 745 and 746 (ADR 0087): a search's Videos and News tabs, as
//! the owner's canvas draws them (`Videos.dc.html`'s `.vr`,
//! `News.dc.html`'s `.n`).
//!
//! What closes their engine half, each a test below: layout assertions of
//! both tabs, in numbers; reference renders, `references/videos-tab.png`
//! and `news-tab.png`; a video's picture laid out at its own proportions in
//! its tile with its length over the tile's corner; the agent reading each
//! title as a link and each length, site and age as text; every string a
//! stranger's; a press on a title claimed by its place among the titles,
//! and on a picture nothing; and the same through the real, confined
//! renderer.
//!
//! The thumbnails are made here, by hand, as flat pictures of known sizes;
//! the videos and stories are written by hand, never captured.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_searchs_videos_and_news`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::canvas::Canvas;
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Results, SetUp, Showing, Stories, Tab, Videos};
use alo_renderer::results_media::{Story, Video};
use alo_renderer::start::{self, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_value::Rgba;
use std::path::{Path, PathBuf};

/// The corpus's faces: its sans-serif for both weights, which is what
/// `system-ui` and `sans-serif` mean here, and its serif for the greeting.
fn faces() -> Vec<(&'static str, Weight, &'static [u8])> {
    vec![
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ]
}

fn generics() -> Vec<(&'static str, &'static str)> {
    vec![
        ("system-ui", "DejaVu Sans"),
        ("sans-serif", "DejaVu Sans"),
        ("serif", "DejaVu Serif"),
    ]
}

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in faces() {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for (generic, family) in generics() {
        database.map_generic(generic, family);
    }
    // The design's typefaces, as the browser process hands them to the
    // interface's renderers (ADR 0084).
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// The room the first screen has in the design's window: 1280 wide less the
/// panel's 240, and the window's 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// What a renderer holding a first screen answers to a paint and a reading.
struct Drawn {
    answer: FromRenderer,
    frame: Option<Frame>,
    tree: Snapshot,
}

fn draw(start: Start) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    Drawn {
        answer,
        frame,
        tree,
    }
}

/// Every node the agent reads in `role`, in order.
fn read<'a>(tree: &'a Snapshot, role: &str) -> Vec<&'a SnapshotNode> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == role)
        .collect()
}

fn names(nodes: &[&SnapshotNode]) -> Vec<Option<String>> {
    nodes.iter().map(|node| node.name.clone()).collect()
}

/// A rectangle as the outline writes it: layout holds whole 1/64 px, so two
/// rectangles that write alike are the same, compared without comparing
/// floats.
fn units(rect: Rect) -> String {
    format!(
        "({}, {}) {}×{}",
        rect.left(),
        rect.top(),
        rect.size.width,
        rect.size.height
    )
}

fn px(left: f32, top: f32, width: f32, height: f32) -> String {
    units(Rect::new(left, top, width, height))
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `frame` with the committed reference, or write it when updating.
/// What differs, in words, if anything does.
fn holds(name: &str, frame: Option<&Frame>) -> Result<(), String> {
    let frame = frame.ok_or_else(|| format!("{name}: nothing was painted"))?;
    let path = reference(name);
    let drawn = frame_to_png(frame).map_err(|why| format!("{name} could not be encoded: {why}"))?;
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn)
            .map_err(|why| format!("{} could not be written: {why}", path.display()));
    }
    let committed = std::fs::read(&path).map_err(|why| {
        format!(
            "{} is not there ({why}); ALO_UPDATE_REFERENCES=1 writes it",
            path.display()
        )
    })?;
    let expected = from_png(&committed).map_err(|why| format!("{name} is not a picture: {why}"))?;
    let expected = Frame::from_canvas(&expected);
    if (frame.width, frame.height) != (expected.width, expected.height) {
        return Err(format!(
            "{name}: the screen is {}×{}, and the reference {}×{}",
            frame.width, frame.height, expected.width, expected.height
        ));
    }
    let moved = frame
        .pixels
        .chunks(4)
        .zip(expected.pixels.chunks(4))
        .position(|(drawn, committed)| drawn != committed);
    match moved {
        None => Ok(()),
        Some(at) => {
            let width = usize::try_from(frame.width).unwrap_or(1).max(1);
            Err(format!(
                "{name}: pixels moved, the first at ({}, {}). If this is intended, \
                 ALO_UPDATE_REFERENCES=1 rewrites the reference",
                at % width,
                at / width
            ))
        }
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    let rgba = frame.pixels.get(at..at + 4)?;
    rgba.try_into().ok()
}

/// The border box of every element of `class`, in document order.
fn boxes_of(renderer: &Renderer, class: &str) -> Vec<String> {
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        return Vec::new();
    };
    drawing
        .boxes
        .ids()
        .filter(|&id| {
            drawing
                .boxes
                .get(id)
                .filter(|made| !matches!(made.kind, alo_box::BoxKind::Text { .. }))
                .and_then(|made| made.kind.node())
                .is_some_and(|node| document.attribute_by_name(node, "class") == Some(class))
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| units(geometry.border_box))
        .collect()
}

/// The renderer binary, confined as every renderer is, with the corpus's
/// fonts.
fn renderers() -> Renderers {
    let faces = faces()
        .into_iter()
        .filter_map(|(family, weight, bytes)| {
            Face::new(family, weight, Slant::Normal, bytes.to_vec())
        })
        .collect();
    let generics = Generics::stating(
        generics()
            .into_iter()
            .map(|(generic, family)| (generic.to_owned(), family.to_owned()))
            .collect(),
    );
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .with_fonts([faces, alo_renderer::typefaces::faces()].concat())
        .with_generics(generics)
}

/// What the person searched for.
const SAID: &str = "home ev charger installation";

fn url(address: &str) -> alo_url::Url {
    alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
}

/// A picture `width` × `height` in one colour with a white band a tenth of
/// its height across its middle, as a PNG.
fn flat(width: u32, height: u32, (red, green, blue): (u8, u8, u8)) -> Vec<u8> {
    let mut canvas = Canvas::new(width, height, Rgba::from_rgba8(red, green, blue, 255));
    let band = height / 10;
    canvas.fill_rect(
        0,
        i32::try_from((height - band) / 2).unwrap_or(0),
        width,
        band,
        Rgba::WHITE,
    );
    alo_paint::encode::to_png(&canvas).unwrap_or_default()
}

fn sites(count: usize) -> Vec<Found> {
    (0..count)
        .map(|n| {
            Found::new(
                format!("Site number {n}: a page about chargers"),
                url(&format!("https://site{n}.example/page-{n}")),
                "Two lines of what the page says, which is short.",
            )
            .named(format!("Site {n}"))
        })
        .collect()
}

/// Three videos: the first a 16:9 picture four minutes long, the second a
/// 4:3 picture over an hour long, the third no picture and no length.
fn three() -> Vec<Video> {
    vec![
        Video::new(
            "How a home charger is installed, start to finish",
            url("https://videos0.example/watch?v=a"),
        )
        .lasting("4:05")
        .from_site("Home Energy Workshop", "2 days ago")
        .said_of("A side-by-side look at charging from a socket and a charger.")
        .holding(flat(480, 270, (0x8a, 0x5a, 0x3e))),
        Video::new(
            "Wall socket or wall charger? What you actually need",
            url("https://videos1.example/v/2"),
        )
        .lasting("1:02:03")
        .from_site("Consumer energy report", "")
        .said_of("For short and long daily drives.")
        .holding(flat(480, 360, (0x3e, 0x5c, 0x8a))),
        Video::new(
            "A third video with nothing more",
            url("https://videos2.example/3"),
        ),
    ]
}

/// Three stories, each its site, age and headline, two said of.
fn stories() -> Vec<Story> {
    vec![
        Story::new(
            "More electricians now trained to fit home chargers",
            url("https://news0.example/a"),
        )
        .from_site("Consumer energy report", "2 days ago")
        .said_of("Waiting times for a survey are getting shorter in many areas."),
        Story::new(
            "What to ask before you book a charger installation",
            url("https://news1.example/b"),
        )
        .from_site("Daily Driver", "1 week ago")
        .said_of("Five questions about cables and fuse boxes to settle first."),
        Story::new(
            "New shared chargers planned for blocks of flats",
            url("https://news2.example/c"),
        ),
    ]
}

fn results(tab: Tab) -> Results {
    Results::of(SAID, Showing::Found(sites(4)))
        .with_videos(Videos::Found(three()))
        .with_news(Stories::Found(stories()))
        .on(tab)
}

fn with(results: Results) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Searched(results)],
        search: SetUp::Ready,
        ..Start::default()
    }
}

fn laid_out(start: Start) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");
    renderer
}

/// The paper, and a video tile's own colour where no picture is drawn.
const PAPER: [u8; 4] = [0xf4, 0xf1, 0xec, 0xff];
const TILE: [u8; 4] = [0x2a, 0x34, 0x46, 0xff];

#[test]
fn the_videos_tab_is_a_list_of_pictures_lengths_and_titles() {
    let renderer = laid_out(with(results(Tab::Videos)));
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("rt on"), vec![px(352.281_25, 102.0, 73.765_625, 42.0)]);
    assert!(at("res").is_empty() && at("more").is_empty() && at("imrow").is_empty());
    // 20 under the tab row, three rows 112 tall — the picture's height —
    // and 20 apart.
    assert_eq!(at("vids"), vec![px(80.0, 164.0, 880.0, 376.0)]);
    let rows = [164.0, 296.0, 428.0];
    assert_eq!(
        at("vr"),
        rows.map(|top| px(80.0, top, 880.0, 112.0)).to_vec()
    );
    assert_eq!(
        at("vpic"),
        rows.map(|top| px(80.0, top, 200.0, 112.0)).to_vec(),
        "each tile 200 × 112, a picture or not"
    );
    // A picture the tile's width at its own proportions, centred up and
    // down: 16:9 is 112.5 tall, a quarter above the tile; 4:3 is 150, 19
    // above and below.
    assert_eq!(
        at("vimg"),
        vec![
            px(80.0, 163.75, 200.0, 112.5),
            px(80.0, 277.0, 200.0, 150.0)
        ]
    );
    // Each length 8 in from the tile's bottom right corner, 19 tall: its
    // right edge at 272 and its foot at 268 (and 400).
    let lengths = at("dur");
    assert_eq!(lengths.len(), 2, "the third has no length");
    assert_eq!(
        lengths.first(),
        Some(&px(236.546_88, 249.0, 35.453_125, 19.0))
    );
    assert_eq!(lengths.get(1), Some(&px(222.4375, 381.0, 49.5625, 19.0)));
    // 16 beside the tile: the title 2 down, 24 tall; 4 under it the site
    // and age, 18; 6 under that what is said, 22.
    assert_eq!(
        at("vb"),
        rows.map(|top| px(296.0, top, 664.0, 112.0)).to_vec()
    );
    assert_eq!(
        at("nt"),
        rows.map(|top| px(296.0, top + 2.0, 664.0, 24.0)).to_vec()
    );
    assert_eq!(
        at("m"),
        rows.map(|top| px(296.0, top + 30.0, 664.0, 18.0)).to_vec()
    );
    assert_eq!(at("fav tint0"), vec![px(296.0, 194.0, 18.0, 18.0)]);
    assert_eq!(
        at("vd"),
        vec![px(296.0, 218.0, 664.0, 22.0), px(296.0, 350.0, 664.0, 22.0)]
    );
    assert_eq!(at("box"), vec![px(80.0, 740.0, 880.0, 56.0)]);
}

#[test]
fn the_videos_tab_is_drawn_as_the_reference() {
    let drawn = draw(with(results(Tab::Videos)));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let frame = drawn.frame.as_ref();
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = frame {
        // The first picture, in its colour, and its white band at the
        // tile's middle, 220.
        assert_eq!(pixel(frame, 100, 180), Some([0x8a, 0x5a, 0x3e, 0xff]));
        assert_eq!(pixel(frame, 100, 219), Some([0xff, 0xff, 0xff, 0xff]));
        // The second's band at its tile's middle, 352; the third has no
        // picture, and is the tile's colour.
        assert_eq!(pixel(frame, 100, 351), Some([0xff, 0xff, 0xff, 0xff]));
        assert_eq!(pixel(frame, 180, 480), Some(TILE));
        // The length's ground, dark over the picture, at its left edge
        // inside it; and the tile rounded 10, the paper at its corner.
        assert!(
            pixel(frame, 238, 252)
                .is_some_and(|[red, green, blue, _]| red < 0x40 && green < 0x30 && blue < 0x20),
            "{:?}",
            pixel(frame, 238, 252)
        );
        assert_eq!(pixel(frame, 80, 164), Some(PAPER));
        // Between the tile and the title, the paper.
        assert_eq!(pixel(frame, 288, 200), Some(PAPER));
    }
    assert_eq!(holds("videos-tab.png", frame), Ok(()));
}

#[test]
fn the_news_tab_is_a_list_of_sites_ages_and_headlines() {
    let renderer = laid_out(with(results(Tab::News)));
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("rt on"), vec![px(428.781_25, 102.0, 63.953_125, 42.0)]);
    // Each story 16 above and below — the first 4 above — over a line of
    // 1: 4 + 18 + 4 + 24 + 4 + 22 + 16 + 1 = 93 for the first, 105 for the
    // second, and 83 for the third, which has nothing said of it.
    assert_eq!(at("news"), vec![px(80.0, 164.0, 880.0, 281.0)]);
    assert_eq!(
        at("n"),
        vec![
            px(80.0, 164.0, 880.0, 93.0),
            px(80.0, 257.0, 880.0, 105.0),
            px(80.0, 362.0, 880.0, 83.0),
        ]
    );
    assert_eq!(
        at("src"),
        [168.0, 273.0, 378.0]
            .map(|top| px(80.0, top, 880.0, 18.0))
            .to_vec()
    );
    // The site's name 8 after its mark, and its age 8 after the name.
    assert_eq!(
        at("name").first(),
        Some(&px(106.0, 168.0, 151.015_63, 18.0))
    );
    assert_eq!(
        at("age").first(),
        Some(&px(265.015_63, 168.0, 65.9375, 18.0))
    );
    assert_eq!(at("age").len(), 2, "the third has no age");
    assert_eq!(
        at("nt"),
        [190.0, 295.0, 400.0]
            .map(|top| px(80.0, top, 880.0, 24.0))
            .to_vec()
    );
    assert_eq!(
        at("vd"),
        vec![px(80.0, 218.0, 740.0, 22.0), px(80.0, 323.0, 740.0, 22.0)]
    );
    assert!(at("vpic").is_empty(), "no photo beside a story (765)");
    let drawn = draw(with(results(Tab::News)));
    let frame = drawn.frame.as_ref();
    if let Some(frame) = frame {
        // The line under the first story, #e6e0d6, across the column.
        assert_eq!(pixel(frame, 500, 256), Some([0xe6, 0xe0, 0xd6, 0xff]));
        assert_eq!(pixel(frame, 500, 255), Some(PAPER));
    }
    assert_eq!(holds("news-tab.png", frame), Ok(()));
}

#[test]
fn while_asked_or_unanswered_each_tab_says_so_in_one_sentence() {
    let asking = Results::of(SAID, Showing::Found(sites(4)))
        .with_videos(Videos::Asking)
        .on(Tab::Videos);
    let drawn = draw(with(asking));
    let outline = drawn.tree.to_outline();
    assert!(
        outline.contains(
            "Searching Brave Search for videos of \u{201c}home ev charger installation\u{201d}\u{2026}"
        ),
        "{outline}"
    );
    let unanswered = Results::of(SAID, Showing::Found(sites(4)))
        .with_news(Stories::Unanswered(
            "Brave Search could not answer just now (503).".to_owned(),
        ))
        .on(Tab::News);
    let renderer = laid_out(with(unanswered));
    assert_eq!(
        boxes_of(&renderer, "note"),
        vec![px(80.0, 164.0, 880.0, 23.0)]
    );
    let none = Results::of(SAID, Showing::Found(sites(4)))
        .with_news(Stories::Found(Vec::new()))
        .on(Tab::News);
    let outline = draw(with(none)).tree.to_outline();
    assert!(
        outline.contains(
            "Brave Search found no news for \u{201c}home ev charger installation\u{201d}."
        ),
        "{outline}"
    );
}

#[test]
fn an_agent_reads_each_title_as_a_link_and_its_length_site_and_age_as_text() {
    let some = |name: &str| Some(name.to_owned());
    let drawn = draw(with(results(Tab::Videos)));
    let outline = drawn.tree.to_outline();
    assert_eq!(
        names(&read(&drawn.tree, "link")),
        three()
            .iter()
            .map(|video| some(&video.title))
            .collect::<Vec<_>>(),
        "{outline}"
    );
    for text in ["4:05", "1:02:03", "Home Energy Workshop \u{b7} 2 days ago"] {
        assert!(outline.contains(text), "{text}: {outline}");
    }
    assert!(
        read(&drawn.tree, "img").is_empty(),
        "a picture is decoration: {outline}"
    );
    let disabled: Vec<bool> = read(&drawn.tree, "tab")
        .iter()
        .map(|tab| tab.states.disabled)
        .collect();
    assert_eq!(
        disabled,
        vec![false, true, false, true, false, false],
        "All, Sites, Videos and News open; Answer not built, and no images asked: {outline}"
    );
    assert!(
        outline.contains("tab \"Videos\" [selected=true]"),
        "{outline}"
    );
    let news = draw(with(results(Tab::News)));
    let outline = news.tree.to_outline();
    assert_eq!(
        names(&read(&news.tree, "link")),
        stories()
            .iter()
            .map(|story| some(&story.title))
            .collect::<Vec<_>>(),
        "{outline}"
    );
    for text in [
        "Consumer energy report",
        "\u{b7} 1 week ago",
        "news2.example",
    ] {
        assert!(outline.contains(text), "{text}: {outline}");
    }
    assert!(
        outline.contains("tab \"News\" [selected=true]"),
        "{outline}"
    );
}

#[test]
fn every_title_site_and_age_stays_its_characters() {
    let hostile = "<img src=x onerror=y()>&amp;</a>";
    let video = Video::new(hostile, url("https://a.example/"))
        .from_site(hostile, hostile)
        .said_of(hostile)
        .holding(flat(50, 50, (1, 2, 3)));
    let story = Story::new(hostile, url("https://b.example/"))
        .from_site(hostile, hostile)
        .said_of(hostile);
    for tab in [Tab::Videos, Tab::News] {
        let drawn = draw(with(
            Results::of(SAID, Showing::Found(sites(1)))
                .with_videos(Videos::Found(vec![video.clone()]))
                .with_news(Stories::Found(vec![story.clone()]))
                .on(tab),
        ));
        let outline = drawn.tree.to_outline();
        assert!(outline.contains(hostile), "{tab:?}: {outline}");
        assert!(read(&drawn.tree, "img").is_empty(), "{tab:?}: {outline}");
    }
}

/// What a renderer holding `start` claims a press at each point is on.
fn claims(start: Start, at: &[(f32, f32)]) -> Vec<FromRenderer> {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    let _ = renderer.handle(ToRenderer::Paint);
    at.iter()
        .map(|&(x, y)| renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))))
        .collect()
}

#[test]
fn a_press_on_a_title_is_claimed_by_its_place_and_on_a_picture_nothing() {
    let claimed = |claim| FromRenderer::ConversationClaimed(Some(claim));
    let nothing = FromRenderer::ConversationClaimed(None);
    assert_eq!(
        claims(
            with(results(Tab::Videos)),
            &[
                (400.0, 178.0),
                (400.0, 310.0),
                (400.0, 442.0),
                (180.0, 220.0),
                (250.0, 258.0),
                (400.0, 203.0),
                (390.0, 123.0),
                (460.0, 123.0),
                (100.0, 123.0),
            ]
        ),
        vec![
            claimed(Claim::Found { at: 0 }),
            claimed(Claim::Found { at: 1 }),
            claimed(Claim::Found { at: 2 }),
            nothing.clone(),
            nothing.clone(),
            nothing.clone(),
            claimed(Claim::Tab { at: 2 }),
            claimed(Claim::Tab { at: 3 }),
            claimed(Claim::Tab { at: 0 }),
        ],
        "three titles; a picture, a length and a site claim nothing; then Videos, News and \
         All among the open tabs All, Sites, Videos, News"
    );
    assert_eq!(
        claims(
            with(results(Tab::News)),
            &[
                (200.0, 202.0),
                (200.0, 307.0),
                (200.0, 412.0),
                (200.0, 177.0)
            ]
        ),
        vec![
            claimed(Claim::Found { at: 0 }),
            claimed(Claim::Found { at: 1 }),
            claimed(Claim::Found { at: 2 }),
            nothing,
        ]
    );
}

#[test]
fn the_videos_and_news_are_drawn_the_same_by_a_confined_renderer() {
    let mut renderers = renderers();
    let site = start::site();
    for tab in [Tab::Videos, Tab::News] {
        let screen = with(results(tab));
        let loaded = renderers.ask(&site, &ToRenderer::Start(Box::new(screen.clone())));
        assert!(
            matches!(&loaded, Ok(FromRenderer::Loaded { .. })),
            "{loaded:?}"
        );
        let here = draw(screen);
        let painted = renderers.ask(&site, &ToRenderer::Paint);
        assert_eq!(
            painted.as_ref().ok(),
            here.frame.map(FromRenderer::Painted).as_ref(),
            "the confined renderer drew other videos or news on {tab:?}"
        );
    }
    renderers.stop_everything();
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 747 (ADR 0088): each site's own mark — its icon, fetched by
//! the browser process from Brave's image proxy — drawn beside its name in
//! the letter's place, on All, Videos and News; and the letter kept where
//! there is no icon, or the icon does not read.
//!
//! What closes its engine half, each a test below: a layout assertion in
//! numbers — the icon in the design's 18 square where the letter was, the
//! row's name where it was; reference renders,
//! `references/all-tab-with-marks.png`, `videos-tab-with-marks.png` and
//! `news-tab-with-marks.png`; an icon that does not decode, or decodes
//! larger than a mark may be, drawn as the letter; the agent reading no
//! mark at all; and the same through the real, confined renderer.
//!
//! The icons are made here, by hand, as small pictures of known colours;
//! the sites, videos and stories are written by hand, never captured.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_sites_own_mark`
//! rewrites the references. A rewritten reference is a diff somebody reads.
use alo_layout::{Rect, Size};
use alo_paint::canvas::Canvas;
use alo_paint::from_png;
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

/// An icon `side` square, its top half `top` and its foot `foot`, as a
/// PNG: which way up it is drawn can be read off its pixels.
fn icon(side: u32, top: (u8, u8, u8), foot: (u8, u8, u8)) -> Vec<u8> {
    let mut canvas = Canvas::new(side, side, Rgba::from_rgba8(foot.0, foot.1, foot.2, 255));
    canvas.fill_rect(
        0,
        0,
        side,
        side / 2,
        Rgba::from_rgba8(top.0, top.1, top.2, 255),
    );
    alo_paint::encode::to_png(&canvas).unwrap_or_default()
}

/// Brave's red and blue, the icon every test site has.
fn red_over_blue() -> Vec<u8> {
    icon(32, (0xc4, 0x30, 0x2b), (0x2b, 0x4e, 0xc4))
}

/// Bytes that begin as a PNG and do not decode.
fn undecodable() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&[0x5a; 64]);
    bytes
}

/// Three sites: the first with its icon, the second with one that does not
/// decode, the third with none; and a fourth, with an icon too large to be
/// a mark, which the Sites tab draws.
fn sites() -> Vec<Found> {
    vec![
        Found::new(
            "Installing a home charger, step by step",
            url("https://evhomeguide.example/installing"),
            "What a survey covers, which cable and fuse a charger needs.",
        )
        .named("EV Home Guide")
        .marked(red_over_blue()),
        Found::new(
            "Smart chargers compared",
            url("https://chargers.example/compared"),
            "Seven wall chargers, side by side.",
        )
        .named("Chargers Compared")
        .marked(undecodable()),
        Found::new(
            "Grants for home charging",
            url("https://grants.example/"),
            "Who can claim, and how much.",
        ),
        Found::new(
            "A fourth site",
            url("https://fourth.example/"),
            "Its icon is a poster.",
        )
        .named("Fourth")
        .marked(icon(300, (0, 0, 0), (0, 0, 0))),
    ]
}

/// Two videos on one site, both with its icon, and one with none.
fn videos() -> Vec<Video> {
    let youtube = icon(32, (0xff, 0x00, 0x00), (0xff, 0xff, 0xff));
    vec![
        Video::new(
            "How a home charger is installed",
            url("https://videos.example/watch?v=a"),
        )
        .lasting("4:05")
        .from_site("YouTube", "2 days ago")
        .said_of("Start to finish.")
        .marked(youtube.clone()),
        Video::new("A second video", url("https://videos.example/watch?v=b"))
            .from_site("YouTube", "")
            .marked(youtube),
        Video::new("A third, with no icon", url("https://other.example/3")),
    ]
}

/// Two stories, the first with its site's icon.
fn stories() -> Vec<Story> {
    vec![
        Story::new(
            "More electricians trained to fit home chargers",
            url("https://news.example/a"),
        )
        .from_site("Daily Driver", "2 days ago")
        .said_of("Waiting times are getting shorter.")
        .marked(icon(32, (0x1f, 0x7a, 0x4d), (0xf0, 0xc0, 0x20))),
        Story::new("A second story", url("https://news2.example/b")).from_site("Weekly", ""),
    ]
}

fn results(tab: Tab) -> Results {
    Results::of(SAID, Showing::Found(sites()))
        .with_videos(Videos::Found(videos()))
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

/// The paper.
const PAPER: [u8; 4] = [0xf4, 0xf1, 0xec, 0xff];

#[test]
fn an_icon_takes_the_letters_place_and_a_site_with_none_that_reads_keeps_its_letter() {
    let renderer = laid_out(with(results(Tab::All)));
    let at = |class: &str| boxes_of(&renderer, class);
    // The first site's icon in the design's 18 square, where its letter
    // was, its name 8 beside it as the letters' are.
    assert_eq!(at("fav icon"), vec![px(80.0, 168.0, 18.0, 18.0)]);
    // The second's does not decode and the third has none: their letters,
    // each on its tint.
    let letters: Vec<String> = (0..4)
        .flat_map(|tint| at(&format!("fav tint{tint}")))
        .collect();
    assert_eq!(
        letters,
        vec![px(80.0, 264.0, 18.0, 18.0), px(80.0, 360.0, 18.0, 18.0)]
    );
    assert_eq!(
        at("src"),
        [168.0, 264.0, 360.0]
            .map(|top| px(80.0, top, 880.0, 18.0))
            .to_vec(),
        "each site's line as tall as it was"
    );
    let names: Vec<String> = at("name");
    assert_eq!(names.len(), 3);
    assert!(
        names.iter().all(|name| name.starts_with("(106, ")),
        "every name starts where a letter's did: {names:?}"
    );
    // On Sites, the fourth's icon decodes to 300 square, larger than a mark
    // may be: its letter.
    let renderer = laid_out(with(results(Tab::Sites)));
    assert_eq!(
        boxes_of(&renderer, "fav icon"),
        vec![px(80.0, 168.0, 18.0, 18.0)]
    );
    assert_eq!(
        boxes_of(&renderer, "fav tint2"),
        vec![px(80.0, 456.0, 18.0, 18.0)],
        "the poster-sized icon is the fourth site's letter"
    );
}

#[test]
fn a_video_and_a_story_are_marked_by_their_sites_icon() {
    let renderer = laid_out(with(results(Tab::Videos)));
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(
        at("fav icon"),
        vec![px(296.0, 194.0, 18.0, 18.0), px(296.0, 326.0, 18.0, 18.0)],
        "both videos on the site with an icon"
    );
    assert_eq!(at("fav tint0"), vec![px(296.0, 458.0, 18.0, 18.0)]);
    let sites = at("mn");
    assert!(
        sites.iter().all(|site| site.starts_with("(320, ")),
        "each site's name 6 beside its mark: {sites:?}"
    );
    let renderer = laid_out(with(results(Tab::News)));
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("fav icon"), vec![px(80.0, 168.0, 18.0, 18.0)]);
    assert_eq!(at("fav tint1"), vec![px(80.0, 273.0, 18.0, 18.0)]);
}

/// An icon drawn: its 18 square's corner, its top half's colour and its
/// foot's.
type Icon = ((u32, u32), [u8; 4], [u8; 4]);

/// What `tab` draws, held against the reference `name`, each icon checked
/// by its pixels at `icons` — its top half's colour 3 down, its foot's 3 up
/// from its foot — and the paper at each icon's rounded corner.
fn drawn_as(tab: Tab, name: &str, icons: &[Icon]) {
    let drawn = draw(with(results(tab)));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let frame = drawn.frame.as_ref();
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = frame {
        for &((left, top), upper, lower) in icons {
            assert_eq!(pixel(frame, left + 9, top + 3), Some(upper), "{name}");
            assert_eq!(pixel(frame, left + 9, top + 14), Some(lower), "{name}");
            assert_eq!(
                pixel(frame, left, top),
                Some(PAPER),
                "{name}: the icon is rounded as the letter's tile is"
            );
        }
    }
    assert_eq!(holds(name, frame), Ok(()));
}

#[test]
fn the_all_tab_with_marks_is_drawn_as_the_reference() {
    let red = [0xc4, 0x30, 0x2b, 0xff];
    let blue = [0x2b, 0x4e, 0xc4, 0xff];
    drawn_as(
        Tab::All,
        "all-tab-with-marks.png",
        &[((80, 168), red, blue)],
    );
}

#[test]
fn the_videos_tab_with_marks_is_drawn_as_the_reference() {
    let red = [0xff, 0x00, 0x00, 0xff];
    let white = [0xff, 0xff, 0xff, 0xff];
    drawn_as(
        Tab::Videos,
        "videos-tab-with-marks.png",
        &[((296, 194), red, white), ((296, 326), red, white)],
    );
}

#[test]
fn the_news_tab_with_marks_is_drawn_as_the_reference() {
    drawn_as(
        Tab::News,
        "news-tab-with-marks.png",
        &[(
            (80, 168),
            [0x1f, 0x7a, 0x4d, 0xff],
            [0xf0, 0xc0, 0x20, 0xff],
        )],
    );
}

#[test]
fn an_agent_reads_no_mark_and_every_site_by_its_name() {
    for tab in [Tab::All, Tab::Videos, Tab::News] {
        let drawn = draw(with(results(tab)));
        let outline = drawn.tree.to_outline();
        assert!(read(&drawn.tree, "img").is_empty(), "{tab:?}: {outline}");
        assert!(!outline.contains("alo:mark"), "{tab:?}: {outline}");
    }
    let drawn = draw(with(results(Tab::All)));
    let links = names(&read(&drawn.tree, "link"));
    assert!(
        links.contains(&Some("Installing a home charger, step by step".to_owned())),
        "{links:?}"
    );
}

#[test]
fn the_marks_are_drawn_the_same_by_a_confined_renderer() {
    let mut renderers = renderers();
    let site = start::site();
    for tab in [Tab::All, Tab::Videos, Tab::News] {
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
            "the confined renderer drew other marks on {tab:?}"
        );
    }
    renderers.stop_everything();
}

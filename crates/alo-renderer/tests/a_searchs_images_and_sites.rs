/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 743 and 744 (ADR 0086): a search's images — All's row of
//! five with *See all images*, the Images tab's grid — and the Sites tab,
//! as the owner's canvas draws them (`Links.dc.html`'s `.imgs`,
//! `Images.dc.html`'s `.grid`, and the Sites board's list).
//!
//! What closes their engine half, each a test below: layout assertions of
//! the row, the grid and the Sites tab, in numbers; reference renders of
//! each, `references/all-tab-with-images.png`, `images-tab.png`,
//! `sites-tab.png`; a picture laid out at its own proportions in its tile,
//! a thumbnail that does not decode drawn as its tile's colour; the agent
//! reading the row and the grid as links named by their titles; every
//! title a stranger's string; a press on each image, open tab, *More sites
//! ›* and *See all images* claimed by its place, and on a disabled tab
//! nothing; and the same through the real, confined renderer.
//!
//! The thumbnails are made here, by hand, as flat pictures of known sizes;
//! the images and sites are written by hand, never captured.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_searchs_images_and_sites`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::canvas::Canvas;
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Images, Pictured, Results, SetUp, Showing, Tab};
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
/// its height across its middle, as a PNG: a thumbnail made by hand, whose
/// band shows where its middle was drawn.
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

/// Bytes that begin as a PNG and are not one.
fn undecodable() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&[0x5a; 64]);
    bytes
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

/// Eight images: the first a 4:3 picture, the second 2:1, the third
/// square, the fourth no picture, the fifth bytes that do not decode, then
/// three more 4:3 pictures.
fn eight() -> Vec<Pictured> {
    let colours = [
        (0x2f, 0x6b, 0x5e),
        (0x3e, 0x5c, 0x8a),
        (0x8a, 0x5a, 0x3e),
        (0, 0, 0),
        (0, 0, 0),
        (0x5a, 0x6b, 0x8a),
        (0x6b, 0x65, 0x5c),
        (0x3d, 0x41, 0x48),
    ];
    colours
        .iter()
        .enumerate()
        .map(|(n, &colour)| {
            let image = Pictured::new(
                format!("Image {n} of a wall charger"),
                url(&format!("https://pictures{n}.example/charger")),
                format!("pictures{n}.example"),
            );
            match n {
                0 | 5 | 6 | 7 => image.holding(flat(500, 375, colour)),
                1 => image.holding(flat(500, 250, colour)),
                2 => image.holding(flat(400, 400, colour)),
                4 => image.holding(undecodable()),
                _ => image,
            }
        })
        .collect()
}

fn results(tab: Tab) -> Results {
    Results::of(SAID, Showing::Found(sites(6)))
        .with_images(Images::Found(eight()))
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

/// The tile's own colour, where no picture is drawn.
const TILE: [u8; 4] = [0xe9, 0xe3, 0xd8, 0xff];

fn laid_out(start: Start) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");
    renderer
}

#[test]
fn the_all_tabs_row_of_images_is_laid_out_as_the_design_draws_it() {
    let renderer = laid_out(with(results(Tab::All)));
    let at = |class: &str| boxes_of(&renderer, class);
    // Three sites, each its line, title and one line of description, 74
    // tall and 22 apart; 20 under them the row; 20 under it More sites.
    assert_eq!(at("res"), vec![px(80.0, 164.0, 880.0, 270.0)]);
    assert_eq!(at("found").len(), 3);
    assert_eq!(at("imrow"), vec![px(80.0, 454.0, 880.0, 110.0)]);
    // The heading, 18 tall: IMAGES at its left, See all images at its
    // right; 8 under it the tiles.
    assert_eq!(at("imh"), vec![px(80.0, 454.0, 880.0, 18.0)]);
    assert!(
        at("seeall")
            .first()
            .is_some_and(|all| all.ends_with("454) 85.546875×18")),
        "{:?}",
        at("seeall")
    );
    assert_eq!(at("imgs"), vec![px(80.0, 480.0, 880.0, 84.0)]);
    // Five tiles 168 × 84, 10 apart: 5 × 168 + 4 × 10 = 880. The fourth has
    // no picture, so holds its title.
    assert_eq!(
        at("im held"),
        vec![
            px(80.0, 480.0, 168.0, 84.0),
            px(258.0, 480.0, 168.0, 84.0),
            px(436.0, 480.0, 168.0, 84.0),
            px(792.0, 480.0, 168.0, 84.0),
        ]
    );
    assert_eq!(at("im"), vec![px(614.0, 480.0, 168.0, 84.0)]);
    assert_eq!(at("imt").len(), 1);
    // Each picture the tile's width at its own proportions, centred up and
    // down: 4:3 is 126 tall, 21 above the tile and 21 below; 2:1 is the
    // tile; a square 168, 42 either side. The bytes that do not decode are
    // no picture, and lay out as nothing.
    assert_eq!(
        at("thumb"),
        vec![
            px(80.0, 459.0, 168.0, 126.0),
            px(258.0, 480.0, 168.0, 84.0),
            px(436.0, 438.0, 168.0, 168.0),
            px(792.0, 522.0, 168.0, 0.0),
        ]
    );
    assert_eq!(at("more"), vec![px(80.0, 584.0, 75.6875, 20.0)]);
    assert_eq!(at("box"), vec![px(80.0, 740.0, 880.0, 56.0)]);
    // The tabs: All shown, Sites, Images, Videos and News open (queue
    // items 744–746), Answer disabled.
    assert_eq!(at("rt on"), vec![px(80.0, 102.0, 45.546_875, 42.0)]);
    assert_eq!(at("rt").len(), 4);
    assert_eq!(at("rt off").len(), 1);
}

#[test]
fn the_all_tab_with_images_is_drawn_as_the_reference() {
    let drawn = draw(with(results(Tab::All)));
    // The bytes that begin as a PNG and are not one are said, by the
    // name of ours they were drawn by, and nothing else is.
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. }
            if issues.len() == 1
                && issues.iter().all(|issue| issue.starts_with("\"alo:thumbnail-0-4\" is not a picture"))),
        "{:?}",
        drawn.answer
    );
    let frame = drawn.frame.as_ref();
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = frame {
        // The first tile's picture, in its colour, and its white band in
        // the tile's middle: the picture's middle is the tile's.
        assert_eq!(pixel(frame, 90, 490), Some([0x2f, 0x6b, 0x5e, 0xff]));
        assert_eq!(pixel(frame, 164, 522), Some([0xff, 0xff, 0xff, 0xff]));
        // The square's band too; the fifth tile, whose bytes do not
        // decode, is its own colour; and the paper between two tiles.
        assert_eq!(pixel(frame, 520, 522), Some([0xff, 0xff, 0xff, 0xff]));
        assert_eq!(pixel(frame, 876, 500), Some(TILE));
        assert_eq!(pixel(frame, 253, 522), Some([0xf4, 0xf1, 0xec, 0xff]));
        // The tile's corner, rounded 10: the paper at its very corner.
        assert_eq!(pixel(frame, 80, 480), Some([0xf4, 0xf1, 0xec, 0xff]));
    }
    assert_eq!(holds("all-tab-with-images.png", frame), Ok(()));
}

#[test]
fn the_sites_tab_shows_every_site_and_no_row_of_images() {
    let renderer = laid_out(with(results(Tab::Sites)));
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("rt on"), vec![px(209.843_75, 102.0, 60.9375, 42.0)]);
    assert_eq!(at("res"), vec![px(80.0, 164.0, 880.0, 558.0)]);
    assert_eq!(
        at("found"),
        [168.0, 264.0, 360.0, 456.0, 552.0, 648.0]
            .map(|top| px(80.0, top, 880.0, 74.0))
            .to_vec(),
        "every site, 74 tall and 22 apart"
    );
    assert!(at("imrow").is_empty() && at("more").is_empty());
    // The sixth runs past the room, which ends 24 above the box, and is
    // clipped there: the box is unmoved.
    assert_eq!(at("box"), vec![px(80.0, 740.0, 880.0, 56.0)]);
    let drawn = draw(with(results(Tab::Sites)));
    assert_eq!(holds("sites-tab.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn the_images_tab_is_a_grid_of_four_with_each_title_and_site() {
    let renderer = laid_out(with(results(Tab::Images)));
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("rt on"), vec![px(273.390_63, 102.0, 75.9375, 42.0)]);
    assert!(at("res").is_empty() && at("more").is_empty());
    // Four columns of 208, 16 apart: 4 × 208 + 3 × 16 = 880; rows 20
    // apart, each a tile of 150, 8, a title of 18, 2 and a site of 16.
    assert_eq!(at("grid"), vec![px(80.0, 164.0, 880.0, 408.0)]);
    let columns = [80.0, 304.0, 528.0, 752.0];
    let cells: Vec<String> = [164.0, 378.0]
        .iter()
        .flat_map(|&top| columns.iter().map(move |&left| px(left, top, 208.0, 194.0)))
        .collect();
    assert_eq!(at("pic"), cells);
    assert_eq!(at("img").first(), Some(&px(80.0, 164.0, 208.0, 150.0)));
    assert_eq!(at("ct").first(), Some(&px(80.0, 322.0, 208.0, 18.0)));
    assert_eq!(at("cs").first(), Some(&px(80.0, 342.0, 208.0, 16.0)));
    assert_eq!(
        at("fav small tint0").first(),
        Some(&px(752.0, 343.0, 14.0, 14.0))
    );
    // 4:3 at 208 is 156, 3 above and below; 2:1 is 104, 23 of the tile
    // either side; a square 208.
    assert_eq!(
        at("thumb").get(..3),
        Some(
            [
                px(80.0, 161.0, 208.0, 156.0),
                px(304.0, 187.0, 208.0, 104.0),
                px(528.0, 135.0, 208.0, 208.0),
            ]
            .as_slice()
        )
    );
    let drawn = draw(with(results(Tab::Images)));
    let frame = drawn.frame.as_ref();
    if let Some(frame) = frame {
        // Above the 2:1 picture, the tile's colour; in it, the picture's.
        assert_eq!(pixel(frame, 400, 175), Some(TILE));
        assert_eq!(pixel(frame, 400, 200), Some([0x3e, 0x5c, 0x8a, 0xff]));
        // The image with no picture and the one that does not decode are
        // their tiles' colour.
        assert_eq!(pixel(frame, 850, 240), Some(TILE));
        assert_eq!(pixel(frame, 180, 450), Some(TILE));
    }
    assert_eq!(holds("images-tab.png", frame), Ok(()));
}

#[test]
fn with_no_images_yet_all_draws_no_row_and_the_images_tab_says_so() {
    let asking = Results::of(SAID, Showing::Found(sites(4))).with_images(Images::Asking);
    let renderer = laid_out(with(asking.clone()));
    assert!(boxes_of(&renderer, "imrow").is_empty());
    assert_eq!(
        boxes_of(&renderer, "more"),
        vec![px(80.0, 454.0, 75.6875, 20.0)],
        "More sites 20 under the sites, where it was before images"
    );
    let drawn = draw(with(asking.on(Tab::Images)));
    let outline = drawn.tree.to_outline();
    assert!(
        outline.contains("Searching Brave Search for images of \u{201c}home ev charger installation\u{201d}\u{2026}"),
        "{outline}"
    );
    let unanswered = Results::of(SAID, Showing::Found(sites(4)))
        .with_images(Images::Unanswered(
            "Brave Search could not answer just now (503).".to_owned(),
        ))
        .on(Tab::Images);
    let renderer = laid_out(with(unanswered));
    assert_eq!(
        boxes_of(&renderer, "note"),
        vec![px(80.0, 164.0, 880.0, 23.0)]
    );
}

#[test]
fn an_agent_reads_the_images_as_links_named_by_their_titles() {
    let some = |name: &str| Some(name.to_owned());
    let drawn = draw(with(results(Tab::All)));
    let outline = drawn.tree.to_outline();
    assert!(
        names(&read(&drawn.tree, "region")).contains(&some("Images")),
        "{outline}"
    );
    let links = names(&read(&drawn.tree, "link"));
    assert_eq!(
        links.get(3..),
        Some(
            (0..5)
                .map(|n| some(&format!("Image {n} of a wall charger")))
                .collect::<Vec<_>>()
                .as_slice()
        ),
        "three titles, then the five images: {outline}"
    );
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
        vec![false, true, false, false, false, false],
        "{outline}"
    );
    let buttons = read(&drawn.tree, "button");
    for name in ["See all images", "More sites \u{203a}"] {
        assert!(
            buttons
                .iter()
                .any(|button| button.name.as_deref() == Some(name) && !button.states.disabled),
            "{name}: {outline}"
        );
    }
    let grid = draw(with(results(Tab::Images)));
    let outline = grid.tree.to_outline();
    let links = names(&read(&grid.tree, "link"));
    assert_eq!(links.len(), 8, "{outline}");
    assert!(
        links
            .first()
            .and_then(Option::as_deref)
            .is_some_and(|name| name.contains("Image 0 of a wall charger")
                && name.contains("pictures0.example")),
        "{outline}"
    );
    assert!(
        outline.contains("tab \"Images\" [selected=true]"),
        "{outline}"
    );
}

#[test]
fn every_title_and_site_stays_its_characters() {
    let hostile = "<img src=x onerror=y()>&amp;</a>";
    let images = vec![
        Pictured::new(hostile, url("https://a.example/"), hostile).holding(flat(50, 50, (1, 2, 3))),
        Pictured::new(hostile, url("https://b.example/"), hostile),
    ];
    for tab in [Tab::All, Tab::Images] {
        let drawn = draw(with(
            Results::of(SAID, Showing::Found(sites(1)))
                .with_images(Images::Found(images.clone()))
                .on(tab),
        ));
        let outline = drawn.tree.to_outline();
        assert!(outline.contains(hostile), "{tab:?}: {outline}");
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
fn a_press_on_an_image_a_tab_more_sites_or_see_all_is_claimed_by_its_place() {
    let claimed = |claim| FromRenderer::ConversationClaimed(Some(claim));
    let nothing = FromRenderer::ConversationClaimed(None);
    assert_eq!(
        claims(
            with(results(Tab::All)),
            &[
                (164.0, 522.0),
                (341.0, 500.0),
                (698.0, 522.0),
                (876.0, 522.0),
                (100.0, 123.0),
                (167.0, 123.0),
                (240.0, 123.0),
                (311.0, 123.0),
                (390.0, 123.0),
                (120.0, 594.0),
                (917.0, 463.0),
                (100.0, 463.0),
                (253.0, 522.0),
            ]
        ),
        vec![
            claimed(Claim::Image { at: 0 }),
            claimed(Claim::Image { at: 1 }),
            claimed(Claim::Image { at: 3 }),
            claimed(Claim::Image { at: 4 }),
            claimed(Claim::Tab { at: 0 }),
            nothing.clone(),
            claimed(Claim::Tab { at: 1 }),
            claimed(Claim::Tab { at: 2 }),
            claimed(Claim::Tab { at: 3 }),
            claimed(Claim::MoreSites { at: 0 }),
            claimed(Claim::SeeAllImages { at: 0 }),
            nothing.clone(),
            nothing.clone(),
        ],
        "the images, All, Answer (disabled), Sites, Images, Videos (open since 745), More \
         sites, See all images, the row's heading, between two tiles"
    );
    // On the grid, the tile and the caption are the image's.
    assert_eq!(
        claims(
            with(results(Tab::Images)),
            &[
                (184.0, 240.0),
                (400.0, 331.0),
                (850.0, 560.0),
                (296.0, 240.0)
            ]
        ),
        vec![
            claimed(Claim::Image { at: 0 }),
            claimed(Claim::Image { at: 1 }),
            claimed(Claim::Image { at: 7 }),
            nothing,
        ]
    );
}

#[test]
fn the_images_are_drawn_the_same_by_a_confined_renderer() {
    let mut renderers = renderers();
    let site = start::site();
    for tab in [Tab::All, Tab::Images] {
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
            "the confined renderer drew other images on {tab:?}"
        );
    }
    renderers.stop_everything();
}

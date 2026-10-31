/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0088: characters the interface's two faces lack — emoji, and rare
//! letters and signs — in a search's results.
//!
//! A title and a description holding emoji, a star and a Cyrillic letter,
//! as Brave's do. What closes it, each a test below: the face that drew
//! each, by name, read off the drawing — an emoji in **Noto Color Emoji**,
//! drawn as a picture in its own colours, never the text's ink; the star
//! and the letter in the corpus's `DejaVu Sans`, as the machine's fonts
//! would draw them; and the rest in **Instrument Sans**; a layout
//! assertion in numbers — an emoji as wide as the face says and the line
//! no taller for it; a reference render, `references/emoji-in-results.png`;
//! and the same drawn by the real, confined renderer.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test emoji_in_a_searchs_results`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::DisplayItem;
use alo_paint::from_png;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::start::{self, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
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

/// The title: a grinning face, a star and a car — the car with a
/// selector asking for its picture.
const TITLE: &str =
    "Home chargers \u{1f600} rated \u{2605}\u{2605}\u{2605} for every \u{1f697}\u{fe0f}";

/// The description: a thumbs-up in a skin tone, a Cyrillic word, a heart
/// asked for as a picture and a woman at a laptop, joined.
const SAID_OF: &str = "Works with every car \u{1f44d}\u{1f3fd} \u{2014} \u{416}\u{434}\u{443} \u{2764}\u{fe0f} \u{1f469}\u{200d}\u{1f4bb} installs it";

fn sites() -> Vec<Found> {
    vec![
        Found::new(TITLE, url("https://chargers.example/rated"), SAID_OF).named("Chargers"),
        Found::new("\u{1f50c}\u{1f50c}", url("https://plugs.example/"), "").named("Plugs"),
    ]
}

fn with(found: Vec<Found>) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Searched(Results::of(SAID, Showing::Found(found)))],
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

/// Every run of text the drawing puts down, with the family of the face
/// that drew it; and how many pictures each run in a face of pictures put
/// down after it.
fn faces_drawn(renderer: &Renderer) -> Vec<(String, String, usize)> {
    let Some(drawing) = renderer.rendered() else {
        return Vec::new();
    };
    let mut runs: Vec<(String, String, usize)> = Vec::new();
    for item in drawing.display.items() {
        match item {
            DisplayItem::Text { text, font, .. } => {
                runs.push((text.clone(), font.family().to_owned(), 0));
            }
            DisplayItem::Picture { .. } => {
                if let Some((_, _, pictures)) = runs.last_mut() {
                    *pictures += 1;
                }
            }
            _ => {}
        }
    }
    runs
}

fn run(text: &str, family: &str, pictures: usize) -> (String, String, usize) {
    (text.to_owned(), family.to_owned(), pictures)
}

#[test]
fn each_character_is_drawn_by_the_face_that_has_it_and_an_emoji_by_noto() {
    let mut renderer = laid_out(with(sites()));
    let _ = renderer.handle(ToRenderer::Paint);
    let drawn = faces_drawn(&renderer);
    let at = |text: &str| drawn.iter().position(|(each, _, _)| each == text);
    let from = at("Home chargers ").unwrap_or(drawn.len());
    let to = at("More sites \u{203a}").unwrap_or(drawn.len());
    let emoji = "Noto Color Emoji";
    let sans = "Instrument Sans";
    assert_eq!(
        drawn.get(from..to).unwrap_or_default(),
        [
            run("Home chargers ", sans, 0),
            run("\u{1f600}", emoji, 1),
            run(" rated ", sans, 0),
            run("\u{2605}\u{2605}\u{2605}", "DejaVu Sans", 0),
            run(" for every ", sans, 0),
            run("\u{1f697}\u{fe0f}", emoji, 1),
            run("Works with every car ", sans, 0),
            run("\u{1f44d}\u{1f3fd}", emoji, 1),
            run(" \u{2014} ", sans, 0),
            run("\u{416}\u{434}\u{443}", "DejaVu Sans", 0),
            run(" ", sans, 0),
            run("\u{2764}\u{fe0f}", emoji, 1),
            run(" ", sans, 0),
            run("\u{1f469}\u{200d}\u{1f4bb}", emoji, 1),
            run(" installs it", sans, 0),
            run("P", sans, 0),
            run("Plugs", sans, 0),
            run("plugs.example", sans, 0),
            run("\u{1f50c}\u{1f50c}", emoji, 2),
        ],
        "a skin tone, a selector and a joined pair are each one picture"
    );
}

#[test]
fn an_emoji_is_as_wide_as_its_face_says_and_its_line_no_taller() {
    let renderer = laid_out(with(sites()));
    let at = |class: &str| boxes_of(&renderer, class);
    // Two plugs at 18 px: Noto's advance is 2550 of 2048 units each,
    // 22.412… px, 44.828125 in layout's sixty-fourths.
    assert_eq!(at("title").get(1), Some(&px(80.0, 287.0, 44.828_125, 22.0)));
    // Each title's line 25 tall and each description's 23, as with no emoji
    // in them: the emoji face's taller ascent does not grow a line whose
    // height the stylesheet sets.
    assert_eq!(
        at("t"),
        vec![px(80.0, 190.0, 880.0, 25.0), px(80.0, 286.0, 880.0, 25.0)]
    );
    assert_eq!(at("d"), vec![px(80.0, 219.0, 740.0, 23.0)]);
    assert_eq!(
        at("found"),
        vec![px(80.0, 168.0, 880.0, 74.0), px(80.0, 264.0, 880.0, 51.0)]
    );
    assert_eq!(at("title").first(), Some(&px(80.0, 191.0, 358.9375, 22.0)));
}

#[test]
fn the_results_with_emoji_are_drawn_as_the_reference() {
    let drawn = draw(with(sites()));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let frame = drawn.frame.as_ref();
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = frame {
        // The grinning face's yellow inside its picture, 212.8 to 235.3
        // across and 191.3 to 212.5 down: drawn in its own colours, never
        // in the title's ink.
        let face = pixel(frame, 224, 196);
        assert!(
            face.is_some_and(|[red, green, blue, _]| red > 0xd0 && green > 0x90 && blue < 0x60),
            "{face:?}"
        );
    }
    assert_eq!(holds("emoji-in-results.png", frame), Ok(()));
}

#[test]
fn the_emoji_are_drawn_the_same_by_a_confined_renderer() {
    let mut renderers = renderers();
    let site = start::site();
    let screen = with(sites());
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
        "the confined renderer, handed the emoji face as bytes, drew other emoji"
    );
    renderers.stop_everything();
}

#[test]
fn an_agent_reads_each_emoji_as_the_characters_it_is() {
    let drawn = draw(with(sites()));
    let links = names(&read(&drawn.tree, "link"));
    assert!(links.contains(&Some(TITLE.to_owned())), "{links:?}");
    assert!(
        links.contains(&Some("\u{1f50c}\u{1f50c}".to_owned())),
        "{links:?}"
    );
    assert!(
        read(&drawn.tree, "img").is_empty(),
        "an emoji drawn as a picture is still text: {}",
        drawn.tree.to_outline()
    );
}

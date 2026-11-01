/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 767 (ADR 0088 § 5): a Chinese, Japanese or Korean title in
//! the interface is drawn in a CJK face read out of a font collection, not
//! in a last resort's box.
//!
//! Never this Mac's own faces: two faces made for these tests with
//! `fontTools` 4.61's `FontBuilder` and under this repository's licence,
//! in `tests/fonts/` — `cjk-test.ttf` (896 bytes, *alo CJK Test*: `東`,
//! `京`, `天`, `気`, each an em square with a hole of its own, and a space)
//! and `last-resort-test.ttf` (708 bytes, *alo Last Resort Test*: one box,
//! every character of U+0020–9FFF and the hangul mapped to it by `cmap`
//! format 13, as Apple's `LastResort` maps them). The collection is made
//! here: `DejaVu Sans` and the CJK face, in that order, as one `ttcf` file
//! whose second face is read out as the browser process reads a machine's
//! ([`alo_renderer::font_collection::face_of`]).
//!
//! The renderer is handed, in this order, the corpus's faces, the last
//! resort, then the face read out — as an interface renderer is handed the
//! machine's two dozen (where `LastResort` is) and the CJK faces after
//! them. Asserted: the face is read out whole and states its own family;
//! the title's Han characters are drawn in it, not the last resort; the
//! title's box in numbers; its pixels, the CJK face's hole where the last
//! resort's would be ink; a reference render,
//! `references/cjk-in-the-interface.png`; and, with no CJK face, the same
//! title drawn in the last resort — which is what this Mac drew before.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test cjk_in_the_interface`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{DisplayItem, from_png};
use alo_renderer::face::Face;
use alo_renderer::font_collection::face_of;
use alo_renderer::frame::frame_to_png;
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// The CJK test face, as committed.
const CJK: &[u8] = include_bytes!("fonts/cjk-test.ttf");

/// The last-resort test face, as committed.
const LAST_RESORT: &[u8] = include_bytes!("fonts/last-resort-test.ttf");

/// The title: two Han characters, a space, two more.
const TITLE: &str = "\u{6771}\u{4eac} \u{5929}\u{6c17}";

/// The big-endian `u32` at `at` of `bytes`.
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// `fonts` as one collection: a `ttcf` header naming each face's place,
/// then each font with every table's offset moved by where the font now
/// starts.
fn collection(fonts: &[&[u8]]) -> Option<Vec<u8>> {
    let count = u32::try_from(fonts.len()).ok()?;
    let mut file = b"ttcf\x00\x01\x00\x00".to_vec();
    file.extend_from_slice(&count.to_be_bytes());
    let mut at = 12 + 4 * fonts.len();
    let mut places = Vec::new();
    for font in fonts {
        places.push(at);
        at += font.len().div_ceil(4) * 4;
    }
    for place in &places {
        file.extend_from_slice(&u32::try_from(*place).ok()?.to_be_bytes());
    }
    for (font, place) in fonts.iter().zip(&places) {
        let mut moved = font.to_vec();
        let tables = usize::from(u16::from_be_bytes(font.get(4..6)?.try_into().ok()?));
        for table in 0..tables {
            let offset = 12 + 16 * table + 8;
            let was = u32_at(font, offset)?;
            let now = was.checked_add(u32::try_from(*place).ok()?)?;
            moved
                .get_mut(offset..offset + 4)?
                .copy_from_slice(&now.to_be_bytes());
        }
        moved.resize(font.len().div_ceil(4) * 4, 0);
        file.extend_from_slice(&moved);
    }
    Some(file)
}

/// The CJK face, read out of the collection it was put second in.
fn read_out() -> Option<Face> {
    let file = collection(&[dejavu::sans::regular(), CJK])?;
    let bytes = face_of(&mut Cursor::new(file), 1).ok()?;
    Face::described(bytes)
}

/// The corpus's faces, the last resort, then — when `cjk` — the face read
/// out, in the order an interface renderer is handed them.
fn fonts(cjk: bool) -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
        ("alo Last Resort Test", Weight::NORMAL, LAST_RESORT),
    ] {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    if cjk
        && let Some(face) = read_out()
        && let Some(font) = Font::load(&face.family, face.weight(), face.slant, face.bytes)
    {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Serif");
    database
}

/// The first screen: one search, its one site titled in Han characters.
fn screen() -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(1040.0, 820.0),
        turns: vec![Turn::Searched(Results::of(
            "tokyo weather",
            Showing::Found(vec![
                Found::new(
                    TITLE,
                    alo_url::parse("https://tenki.example/tokyo")
                        .unwrap_or_else(|_| alo_url::Url::about_blank()),
                    "",
                )
                .named("Tenki"),
            ]),
        ))],
        search: SetUp::Ready,
        ..Start::default()
    }
}

/// What was drawn, and each run of text with the family it was drawn in.
struct Drawn {
    renderer: Renderer,
    frame: Option<Frame>,
    runs: Vec<(String, String)>,
}

fn drawn(cjk: bool) -> Drawn {
    let mut renderer = Renderer::new(fonts(cjk));
    let answer = renderer.handle(ToRenderer::Start(Box::new(screen())));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let runs = renderer
        .rendered()
        .map(|drawing| {
            drawing
                .display
                .items()
                .iter()
                .filter_map(|item| match item {
                    DisplayItem::Text { text, font, .. } => {
                        Some((text.clone(), font.family().to_owned()))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    Drawn {
        renderer,
        frame,
        runs,
    }
}

/// The family each run holding a Han character of the title was drawn in.
fn han_families(runs: &[(String, String)]) -> Vec<(String, String)> {
    runs.iter()
        .filter(|(text, _)| text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)))
        .cloned()
        .collect()
}

/// The border box of the element of `class`, in CSS pixels.
fn box_of(renderer: &Renderer, class: &str) -> Option<Rect> {
    let (drawing, document) = (renderer.rendered()?, renderer.document()?);
    drawing.boxes.ids().find_map(|id| {
        let made = drawing.boxes.get(id)?;
        if matches!(made.kind, alo_box::BoxKind::Text { .. }) {
            return None;
        }
        let node = made.kind.node()?;
        (document.attribute_by_name(node, "class") == Some(class))
            .then(|| drawing.layout.get(id).map(|at| at.border_box))
            .flatten()
    })
}

fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

fn reference() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/references/cjk-in-the-interface.png")
}

fn holds(frame: Option<&Frame>) -> Result<(), String> {
    let frame = frame.ok_or("nothing was painted")?;
    let drawn = frame_to_png(frame).map_err(|why| format!("not encoded: {why}"))?;
    let path = reference();
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn).map_err(|why| why.to_string());
    }
    let committed = std::fs::read(&path).map_err(|why| format!("{}: {why}", path.display()))?;
    let expected = Frame::from_canvas(&from_png(&committed).map_err(|why| why.to_string())?);
    if (frame.width, frame.height) != (expected.width, expected.height) {
        return Err("the reference is another size".to_owned());
    }
    match frame
        .pixels
        .chunks(4)
        .zip(expected.pixels.chunks(4))
        .position(|(drawn, committed)| drawn != committed)
    {
        None => Ok(()),
        Some(at) => Err(format!(
            "pixels moved, the first at {at}; ALO_UPDATE_REFERENCES=1 rewrites the reference"
        )),
    }
}

#[test]
fn a_face_is_read_out_of_a_collection_whole() {
    let face = read_out();
    assert_eq!(
        face.as_ref().map(|face| face.family.as_str()),
        Some("alo CJK Test"),
        "the second face, by its own name"
    );
    // Its tables are its own font's, byte for byte: the same size, padded.
    assert_eq!(
        face.map(|face| face.bytes.len()),
        Some(CJK.len().div_ceil(4) * 4)
    );
    let first = collection(&[dejavu::sans::regular(), CJK])
        .and_then(|file| face_of(&mut Cursor::new(file), 0).ok())
        .and_then(Face::described);
    assert_eq!(
        first.map(|face| face.family),
        Some("DejaVu Sans".to_owned())
    );
    // The last resort is known by its character map, whatever it is called.
    let last = Font::load("x", Weight::NORMAL, Slant::Normal, LAST_RESORT.to_vec());
    assert_eq!(last.map(|font| font.is_last_resort()), Some(true));
    let cjk = Font::load("x", Weight::NORMAL, Slant::Normal, CJK.to_vec());
    assert_eq!(cjk.map(|font| font.is_last_resort()), Some(false));
}

#[test]
fn a_cjk_title_is_drawn_in_the_cjk_face_not_the_last_resort() {
    let with = drawn(true);
    assert_eq!(
        han_families(&with.runs),
        vec![
            ("\u{6771}\u{4eac}".to_owned(), "alo CJK Test".to_owned()),
            ("\u{5929}\u{6c17}".to_owned(), "alo CJK Test".to_owned()),
        ],
        "every run: {:?}",
        with.runs
    );
    // The title, 18 px and 600: four glyphs an em wide, 18 each, and
    // DejaVu Sans Bold's space between, 713 units of 2,048 (6.2666) — 78.2666,
    // on the layout unit 78.28125 — at (80, 192), 21 tall.
    let title = box_of(&with.renderer, "title");
    assert!(
        title.is_some_and(|title| {
            title.left().to_bits() == 80.0_f32.to_bits()
                && title.top().to_bits() == 192.0_f32.to_bits()
                && title.size.width.to_bits() == 78.281_25_f32.to_bits()
                && title.size.height.to_bits() == 21.0_f32.to_bits()
        }),
        "{title:?}"
    );
    let title = title.unwrap_or_default();
    let frame = with.frame.as_ref();
    if let Some(frame) = frame {
        // `東` is a square with a hole from 0.3 to 0.7 across; a fifth of
        // an em in is ink — where the last resort's box would be its hole.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a point on a 1040 × 820 screen"
        )]
        let at = |x: f32, y: f32| (x.round() as u32, y.round() as u32);
        let (x, y) = at(title.left() + 0.2 * 18.0, title.top() + 12.5);
        assert_ne!(pixel(frame, x, y), Some([0xf4, 0xf1, 0xec, 0xff]), "ink");
        let (x, y) = at(title.left() + 0.5 * 18.0, title.top() + 12.5);
        assert_eq!(
            pixel(frame, x, y),
            Some([0xf4, 0xf1, 0xec, 0xff]),
            "its hole"
        );
    }
    assert_eq!(holds(frame), Ok(()));
}

#[test]
fn with_no_cjk_face_the_title_is_the_last_resorts_box() {
    let without = drawn(false);
    assert_eq!(
        han_families(&without.runs),
        vec![
            (
                "\u{6771}\u{4eac}".to_owned(),
                "alo Last Resort Test".to_owned()
            ),
            (
                "\u{5929}\u{6c17}".to_owned(),
                "alo Last Resort Test".to_owned()
            ),
        ],
        "every run: {:?}",
        without.runs
    );
}

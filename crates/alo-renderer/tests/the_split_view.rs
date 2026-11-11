/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 819 (`docs/design/shell.md`, *The agent at work*): while
//! alo's agent works on the card, the first screen is the split view — the
//! page large on the left, the agent's column 384 wide on the right with
//! the box at its foot — drawn by the first screen's renderer from the
//! same state as the conversation.
//!
//! Each a test below: layout assertions in numbers at the design's window
//! beside the rail (a first screen of 1216 × 820), a wide one (1616 ×
//! 1050) and the least (1138, and 1137, where it is the conversation
//! again), each page box held equal to `split_place`'s, which the browser
//! process aims a person's press by and tells Chrome; the newest of twenty
//! steps kept in the column's room; presses on *Stop*, *Allow* and the box
//! claimed as each; the agent reading the column; and a reference render,
//! `references/split-view-asking.png`, the page a frame Chrome would send
//! at the split's page box, drawn here as a test pattern of known pixels.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_split_view`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::agent_document::options_named;
use alo_renderer::agent_document::{ALLOW, AT_WORK, STOP};
use alo_renderer::agent_work::{AtWork, Dropdown, Now, Step, WORKING};
use alo_renderer::card::Card;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::split_document::BESIDE;
use alo_renderer::split_place::{self, Stands};
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces for the generics, and the design's typefaces as the
/// browser process hands them to the interface's renderers (ADR 0084).
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ] {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for (generic, family) in [
        ("system-ui", "DejaVu Sans"),
        ("sans-serif", "DejaVu Sans"),
        ("serif", "DejaVu Serif"),
    ] {
        database.map_generic(generic, family);
    }
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// A frame Chrome would stream of a page laid out at `page` CSS pixels at
/// one: white, a band of the page's own blue 60 tall across its top, and a
/// grey line 12 tall every 28 under it, 40 in from either side — pixels
/// known in advance, so the reference shows where the page box is.
fn page_frame(page: (u32, u32)) -> Result<Vec<u8>, String> {
    let (width, height) = page;
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let lined = y >= 84 && (y - 84) % 28 < 12 && x >= 40 && x + 40 < width;
            let colour = if y < 60 {
                [0x36, 0x66, 0xcc, 0xff]
            } else if lined {
                [0xd8, 0xd8, 0xd8, 0xff]
            } else {
                [0xff, 0xff, 0xff, 0xff]
            };
            pixels.extend_from_slice(&colour);
        }
    }
    frame_to_png(&Frame {
        width,
        height,
        pixels,
    })
    .map_err(|why| format!("the page's frame could not be encoded: {why}"))
}

fn step(line: &str, why: &str) -> Step {
    Step {
        line: line.to_owned(),
        why: why.to_owned(),
    }
}

/// It acting: two steps, the second under way with why it chose.
fn acting() -> AtWork {
    AtWork {
        plan: None,
        asked: "Search Wikipedia for Kampala and open the article".to_owned(),
        steps: vec![
            step("Typing \u{201c}Kampala\u{201d} into Search Wikipedia", ""),
            step(
                "Pressing Search",
                "the search box shows no suggestion for it yet",
            ),
        ],
        now: Now::Acting,
        choosing: None,
    }
}

/// It asking before pressing *Send*.
fn asking() -> AtWork {
    AtWork {
        plan: None,
        asked: "Send my name, Ada, through the form".to_owned(),
        steps: vec![
            step("Typing \u{201c}Ada\u{201d} into Name", ""),
            step("Pressing Send", ""),
        ],
        now: Now::Asking {
            what: "Press \u{201c}Send\u{201d} on 127.0.0.1:8765? It sends the form on this page."
                .to_owned(),
            paying: None,
        },
        choosing: None,
    }
}

/// It acting after twenty steps, each with why.
fn twenty() -> AtWork {
    AtWork {
        plan: None,
        asked: "Read every section".to_owned(),
        steps: (1..=20)
            .map(|n| step(&format!("Opening section {n}"), "it is the next one down"))
            .collect(),
        now: Now::Acting,
        choosing: None,
    }
}

/// The first screen `viewport` big, a page shown earlier and then the live
/// card, its frame at the split's page box, the agent at work on it as
/// `work`.
fn with(work: AtWork, viewport: Size) -> Result<Start, String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .map_err(|_| "the card's address".to_owned())?;
    let earlier = alo_url::parse("https://en.wikipedia.org/")
        .map_err(|_| "the earlier address".to_owned())?;
    let mut card = Card::new("127.0.0.1:8765/card.html", url)
        .holding(page_frame(split_place::page_size(viewport))?)
        .loaded();
    card.agent = Some(work);
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport,
        turns: vec![
            Turn::Visited {
                said: "wikipedia".to_owned(),
                url: earlier,
                ran: Vec::new(),
            },
            Turn::Page(card),
        ],
        ..Start::default()
    })
}

/// A rectangle as the outline writes it.
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

/// The border box of every element of `class`, in document order.
fn boxes_of(renderer: &Renderer, class: &str) -> Vec<String> {
    rects_of(renderer, class).into_iter().map(units).collect()
}

/// The border box of every element of `class`, in document order.
fn rects_of(renderer: &Renderer, class: &str) -> Vec<Rect> {
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
        .map(|geometry| geometry.border_box)
        .collect()
}

fn laid_out(start: Start) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    renderer
}

/// What a renderer holding `start` claims a press at `x`, `y` is on.
fn pressed(start: Start, x: f32, y: f32) -> FromRenderer {
    let mut renderer = laid_out(start);
    let _ = renderer.handle(ToRenderer::Paint);
    renderer.handle(ToRenderer::ConversationPress(Point::new(x, y)))
}

/// What a renderer holding `start` paints, and the tree an agent reads.
fn drawn(start: Start) -> (Option<Frame>, Snapshot) {
    let mut renderer = laid_out(start);
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    (frame, tree)
}

/// Compare `frame` with the committed reference, or write it when updating.
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

/// The design's window, 1280 × 820 at one, beside the rail's 64.
const NORMAL: Size = Size {
    width: 1216.0,
    height: 820.0,
};

#[test]
fn in_the_designs_window_the_page_is_large_and_the_column_384_on_the_right() -> Result<(), String> {
    let renderer = laid_out(with(acting(), NORMAL)?);
    let at = |class: &str| boxes_of(&renderer, class);
    // The page's side the first screen less the column; the card 16 in from
    // its top, left and foot and from the column, outlined while the agent
    // works; its bar 46, then the page box the rest: 798 × 740 — what the
    // browser process aims a press by and tells Chrome.
    assert_eq!(at("beside-page"), vec![px(0.0, 0.0, 832.0, 820.0)]);
    assert_eq!(at("card acting"), vec![px(16.0, 16.0, 800.0, 788.0)]);
    assert_eq!(at("bar"), vec![px(17.0, 17.0, 798.0, 46.0)]);
    assert_eq!(at("page"), vec![units(split_place::page_box(NORMAL))]);
    assert_eq!(at("page"), vec![px(17.0, 63.0, 798.0, 740.0)]);
    assert_eq!(at("ring"), vec![px(17.0, 63.0, 798.0, 740.0)]);
    // The column at the right edge, the whole height, as split_place has
    // it; 1 of border and 16 inside: 351 across, from 849.
    assert_eq!(at("beside"), vec![units(split_place::column(NORMAL))]);
    assert_eq!(at("beside"), vec![px(832.0, 0.0, 384.0, 820.0)]);
    assert_eq!(at("agent"), vec![px(849.0, 16.0, 351.0, 156.0)]);
    // The working line its two lines of 18 and 8 above and below, alo's
    // mark 12 in and in its middle and the words 8 after it (queue item
    // 974), *Stop* in its middle at its end; what was asked 10 under it;
    // the steps.
    assert_eq!(at("working"), vec![px(849.0, 16.0, 351.0, 52.0)]);
    assert_eq!(at("acting-mark"), vec![px(861.0, 34.0, 16.0, 16.0)]);
    assert_eq!(at("said"), vec![px(885.0, 24.0, 243.640_63, 36.0)]);
    assert_eq!(
        at("abtn stop"),
        vec![px(1_140.640_6, 28.0, 55.359_375, 28.0)]
    );
    assert_eq!(at("asked"), vec![px(849.0, 78.0, 351.0, 20.0)]);
    assert_eq!(at("steps"), vec![px(849.0, 108.0, 351.0, 64.0)]);
    assert_eq!(at("step"), vec![px(849.0, 108.0, 351.0, 20.0)]);
    assert_eq!(at("step now"), vec![px(849.0, 134.0, 351.0, 38.0)]);
    // The box at the column's foot, 24 above the screen's: 740, as in the
    // conversation.
    assert_eq!(at("box"), vec![px(849.0, 740.0, 351.0, 56.0)]);
    // No conversation is drawn while it works.
    assert!(at("thread").is_empty() && at("visit").is_empty());
    Ok(())
}

#[test]
fn asking_the_question_stands_in_the_column_under_the_steps() -> Result<(), String> {
    let renderer = laid_out(with(asking(), NORMAL)?);
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("page"), vec![px(17.0, 63.0, 798.0, 740.0)]);
    assert_eq!(at("steps"), vec![px(849.0, 108.0, 351.0, 46.0)]);
    assert!(at("step now").is_empty());
    // 10 under the steps; its words on two lines of 20 in the 321 inside.
    assert_eq!(at("asks"), vec![px(849.0, 164.0, 351.0, 104.0)]);
    assert_eq!(at("question"), vec![px(864.0, 177.0, 321.0, 40.0)]);
    assert_eq!(at("abtn allow"), vec![px(864.0, 227.0, 59.390_625, 28.0)]);
    assert_eq!(
        at("abtn notnow"),
        vec![px(931.390_6, 227.0, 77.062_5, 28.0)]
    );
    Ok(())
}

#[test]
fn in_a_wide_window_the_page_takes_the_room_and_the_column_stays_384() -> Result<(), String> {
    // 1680 × 1050 beside the rail.
    let wide = Size::new(1616.0, 1050.0);
    let renderer = laid_out(with(acting(), wide)?);
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("card acting"), vec![px(16.0, 16.0, 1200.0, 1018.0)]);
    assert_eq!(at("page"), vec![px(17.0, 63.0, 1198.0, 970.0)]);
    assert_eq!(at("page"), vec![units(split_place::page_box(wide))]);
    assert_eq!(at("beside"), vec![px(1232.0, 0.0, 384.0, 1050.0)]);
    assert_eq!(at("agent"), vec![px(1249.0, 16.0, 351.0, 156.0)]);
    assert_eq!(at("box"), vec![px(1249.0, 970.0, 351.0, 56.0)]);
    Ok(())
}

#[test]
fn at_the_least_width_it_splits_and_one_narrower_it_is_the_conversation() -> Result<(), String> {
    // 1202 beside the rail: the page box 720.
    let least = Size::new(split_place::LEAST_ROOM, 820.0);
    let renderer = laid_out(with(acting(), least)?);
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("page"), vec![px(17.0, 63.0, 720.0, 740.0)]);
    assert_eq!(at("page"), vec![units(split_place::page_box(least))]);
    assert_eq!(at("beside"), vec![px(754.0, 0.0, 384.0, 820.0)]);
    assert_eq!(at("box"), vec![px(771.0, 740.0, 351.0, 56.0)]);
    // One narrower: the conversation, the card in the column 880 wide and
    // the agent's column under its page, as queue item 704 drew it — the
    // page box card_place's.
    let narrower = Size::new(split_place::LEAST_ROOM - 1.0, 820.0);
    let start = with(acting(), narrower)?;
    assert_eq!(start.stands(), Stands::InTheConversation);
    let renderer = laid_out(start);
    let at = |class: &str| boxes_of(&renderer, class);
    assert!(at("beside").is_empty());
    assert_eq!(at("card acting"), vec![px(128.5, 98.0, 880.0, 739.0)]);
    assert_eq!(at("page"), vec![px(129.5, 145.0, 878.0, 520.0)]);
    assert_eq!(
        at("page"),
        vec![units(alo_renderer::card_place::page_box(narrower, 0.0))]
    );
    assert_eq!(at("agent"), vec![px(129.5, 665.0, 878.0, 171.0)]);
    assert_eq!(at("thread"), vec![px(96.5, 40.0, 944.0, 676.0)]);
    Ok(())
}

/// The box's hint as laid out, and the field it is shown in: the hint's
/// text box and the field's content box.
fn hint_and_field(renderer: &Renderer) -> Option<(Rect, Rect)> {
    let drawing = renderer.rendered()?;
    let boxes = &drawing.boxes;
    let hint = boxes.ids().find(|&id| boxes.is_placeholder(id))?;
    let field = rects_of(renderer, "ask").into_iter().next()?;
    let geometry = drawing
        .boxes
        .ids()
        .filter(|&id| {
            boxes
                .get(id)
                .is_some_and(|made| !matches!(made.kind, alo_box::BoxKind::Text { .. }))
        })
        .filter_map(|id| drawing.layout.get(id))
        .find(|geometry| geometry.border_box == field)?;
    Some((drawing.layout.get(hint)?.border_box, geometry.content_box()))
}

#[test]
fn the_boxs_hint_in_the_column_is_its_own_and_whole() -> Result<(), String> {
    // Queue item 827: the first screen's hint, *Ask anything, search, or
    // type a website…*, is wider than the column's field and was cut at
    // *webs*; the column says it without *anything*, whole in the field.
    let renderer = laid_out(with(acting(), NORMAL)?);
    let (hint, field) =
        hint_and_field(&renderer).ok_or("the column's box shows no hint, or no field")?;
    // The field the box's 351 less its border, padding, *Go* and the gap
    // between them: 1 + 22 + 263 + 10 + 44 + 10 + 1.
    assert_eq!(units(field), px(872.0, 748.0, 263.0, 40.0));
    // The hint's line is Manrope's at 15, 22 tall, centred in the field
    // (queue item 975; Inter's was 20).
    assert_eq!(units(hint), px(872.0, 757.0, 227.390_63, 22.0));
    assert!(hint.right() <= field.right(), "{}", units(hint));
    let (_, tree) = drawn(with(acting(), NORMAL)?);
    let outline = tree.to_outline();
    assert!(
        outline.contains(alo_renderer::split_document::COLUMN_HINT),
        "the hint read: {outline}"
    );
    assert!(
        outline.contains("Ask anything, search, or type a website"),
        "the field's own name kept: {outline}"
    );
    // In the conversation, one narrower than the split, the box is wider
    // and its hint the first screen's, whole too.
    let narrower = Size::new(split_place::LEAST_ROOM - 1.0, 820.0);
    let renderer = laid_out(with(acting(), narrower)?);
    let (hint, field) =
        hint_and_field(&renderer).ok_or("the conversation's box shows no hint, or no field")?;
    assert_eq!(units(hint), px(151.5, 757.0, 296.359_38, 22.0));
    assert!(hint.right() <= field.right(), "{}", units(hint));
    Ok(())
}

#[test]
fn twenty_steps_keep_the_newest_seen_and_the_box_clear_of_them() -> Result<(), String> {
    let renderer = laid_out(with(twenty(), NORMAL)?);
    let steps = rects_of(&renderer, "steps");
    let older = rects_of(&renderer, "step");
    let newest = rects_of(&renderer, "step now");
    let the_box = rects_of(&renderer, "box");
    let (Some(steps), Some(oldest), Some(newest), Some(the_box)) = (
        steps.first(),
        older.first(),
        newest.first(),
        the_box.first(),
    ) else {
        return Err("the steps, a step, the newest or the box not laid out".to_owned());
    };
    // The steps' room from 108 to 16 above the box's 740.
    assert_eq!(units(*steps), px(849.0, 108.0, 351.0, 616.0));
    assert_eq!(units(*the_box), px(849.0, 740.0, 351.0, 56.0));
    // The newest at its foot, 38 tall (its line and why), whole.
    assert_eq!(units(*newest), px(849.0, 686.0, 351.0, 38.0));
    // The oldest clipped above it, out of sight.
    assert!(oldest.top() < steps.top(), "{}", units(*oldest));
    assert_eq!(older.len(), 19);
    Ok(())
}

#[test]
fn stop_allow_not_now_and_the_box_are_claimed_in_the_column() -> Result<(), String> {
    for (x, y, claim) in [
        (1_168.0, 42.0, Some(Claim::Stop)),
        (894.0, 241.0, Some(Claim::Allow)),
        (970.0, 241.0, Some(Claim::NotNow)),
        (1_000.0, 768.0, Some(Claim::TheBox)),
        // The steps and the question's words claim nothing.
        (1_000.0, 118.0, None),
        (1_000.0, 190.0, None),
    ] {
        assert_eq!(
            pressed(with(asking(), NORMAL)?, x, y),
            FromRenderer::ConversationClaimed(claim),
            "{claim:?} at {x}, {y}"
        );
    }
    Ok(())
}

#[test]
fn an_agent_reads_the_page_and_the_column_beside_it() -> Result<(), String> {
    let (_, tree) = drawn(with(asking(), NORMAL)?);
    let outline = tree.to_outline();
    let beside = tree
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some(BESIDE))
        .ok_or_else(|| format!("nothing named {BESIDE}: {outline}"))?;
    assert_eq!(beside.role.as_str(), "complementary", "{outline}");
    let inside = Snapshot {
        root: Some(beside.clone()),
    }
    .to_outline();
    for said in [AT_WORK, WORKING, STOP, ALLOW, "Pressing Send"] {
        assert!(inside.contains(said), "{said} not read: {inside}");
    }
    // The card's page is read by its address, as in the conversation.
    assert!(outline.contains("127.0.0.1:8765"), "{outline}");
    Ok(())
}

#[test]
fn the_split_view_is_drawn_as_the_reference() -> Result<(), String> {
    let (asked, _) = drawn(with(asking(), NORMAL)?);
    let frame = asked.as_ref().ok_or("nothing was painted")?;
    assert_eq!((frame.width, frame.height), (1216, 820));
    // The outline, 2 of the acting colour over the page's left edge.
    for x in [17, 18] {
        assert_eq!(pixel(frame, x, 400), Some([0x0f, 0x6b, 0x72, 0xff]), "{x}");
    }
    // The page's own blue band, inside the outline, and its white.
    assert_eq!(pixel(frame, 400, 90), Some([0x36, 0x66, 0xcc, 0xff]));
    assert_eq!(pixel(frame, 30, 790), Some([0xff, 0xff, 0xff, 0xff]));
    // The column's rule in the line's colour, and the column white.
    assert_eq!(pixel(frame, 832, 400), Some([0xdd, 0xd6, 0xca, 0xff]));
    assert_eq!(pixel(frame, 1000, 500), Some([0xff, 0xff, 0xff, 0xff]));
    // The working line's tint, clear of its words, and *Allow* in the ink.
    assert_eq!(pixel(frame, 852, 20), Some([0xe2, 0xed, 0xee, 0xff]));
    assert_eq!(pixel(frame, 866, 241), Some([0x10, 0x2a, 0x43, 0xff]));
    holds("split-view-asking.png", asked.as_ref())
}

/// Queue item 810: it choosing *Dates* in the closed list *Fruit* — the
/// list's options in its own column under the step, the one chosen ticked.
fn choosing() -> AtWork {
    let mut work = acting();
    work.steps.push(step("Choosing Dates in Fruit", ""));
    work.choosing = Some(Dropdown {
        list: "Fruit".to_owned(),
        options: ["Apples", "Apricots", "Bananas", "Cherries", "Dates"]
            .map(str::to_owned)
            .to_vec(),
        chosen: 4,
    });
    work
}

#[test]
fn a_closed_lists_options_stand_in_the_column_under_the_step_choosing() -> Result<(), String> {
    let renderer = laid_out(with(choosing(), NORMAL)?);
    let at = |class: &str| boxes_of(&renderer, class);
    // The step under way 6 under the two before it; the options 4 under
    // its line: 1 of border, 4 inside, a line of 20 for each of the five.
    assert_eq!(at("step now"), vec![px(849.0, 178.0, 351.0, 134.0)]);
    assert_eq!(at("dropdown"), vec![px(849.0, 202.0, 351.0, 110.0)]);
    let lines = at("opt");
    assert_eq!(lines.len(), 4);
    assert_eq!(lines.first(), Some(&px(850.0, 207.0, 349.0, 20.0)));
    assert_eq!(at("opt chosen"), vec![px(850.0, 287.0, 349.0, 20.0)]);
    let (frame, tree) = drawn(with(choosing(), NORMAL)?);
    let listed: Vec<&alo_renderer::SnapshotNode> = tree
        .nodes()
        .into_iter()
        .filter(|node| node.name.as_deref() == Some(&options_named("Fruit")))
        .collect();
    assert_eq!(listed.len(), 1, "{}", tree.to_outline());
    holds("agent-choosing-in-a-list.png", frame.as_ref())
}

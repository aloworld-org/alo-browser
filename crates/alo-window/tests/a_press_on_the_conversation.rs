/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 710: a person's press on the chats panel or the first screen
//! in the conversation's window, claimed by place in the renderer that drew
//! it and decided by the browser process (ADR 0074, ADR 0024 § 5).
//!
//! What closes it, each a test below: a press on *New chat* adds a chat on
//! screen, and a press on the other chat is decided as that chat and makes
//! it current — the panel drawn again and compared with a committed
//! reference, and the current row's box asserted in numbers; a press on the
//! box is claimed as the box and gives it the keyboard (item 701); a lying claim is
//! refused by name; and the same presses through the two **confined**
//! renderers the window's thread holds, from the window's own aim of a
//! point in device pixels.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window --test
//! a_press_on_the_conversation` rewrites the reference. A rewritten
//! reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::conversation_decide::{Decided, Listed, Refusal, decide};
use alo_renderer::frame::frame_to_png;
use alo_renderer::host::Renderers;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::beside::renderer_beside;
use alo_window::conversation::{Conversation, Pressed, Showing};
use alo_window::conversation_aim::aim;
use alo_window::conversation_drawing::{Asked, ConversationDrawing, Drawn, Part};
use alo_window::conversation_place::{
    ConversationPlacement, Parts, panel_viewport, start_viewport,
};
use alo_window::scale::Scale;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// The design's window at one, in device pixels.
const AT_ONE: (u32, u32) = (1280, 820);

/// The corpus's faces, and what `system-ui`, `sans-serif` and `serif` mean.
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
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Serif");
    database
}

/// What the window asks to be drawn of `conversation` at one.
fn asked(conversation: &Conversation) -> Asked {
    Asked {
        panel: conversation.panel(panel_viewport(AT_ONE, Scale::ONE), Scale::ONE),
        start: conversation.start(start_viewport(AT_ONE, Scale::ONE), Scale::ONE),
        showing: conversation.showing(),
    }
}

/// The two parts held in renderers of this process, as the window's thread
/// holds them in confined ones.
struct Here {
    panel: Renderer,
    start: Renderer,
    /// What each showed when it was last built.
    painted: Showing,
}

impl Here {
    fn of(conversation: &Conversation) -> Self {
        let mut here = Self {
            panel: Renderer::new(fonts()),
            start: Renderer::new(fonts()),
            painted: Showing::default(),
        };
        here.draw(conversation);
        here
    }

    /// Build both parts as `conversation` has them now.
    fn draw(&mut self, conversation: &Conversation) {
        let Asked {
            panel,
            start,
            showing,
        } = asked(conversation);
        for (renderer, work) in [
            (&mut self.panel, ToRenderer::Panel(Box::new(panel))),
            (&mut self.start, ToRenderer::Start(Box::new(start))),
        ] {
            let built = renderer.handle(work);
            assert!(matches!(built, FromRenderer::Loaded { .. }), "{built:?}");
        }
        self.painted = showing;
    }

    /// A press on `part` at (`x`, `y`) in its CSS pixels, decided against
    /// what was painted and what `conversation` holds.
    fn press(&mut self, conversation: &Conversation, part: Part, x: f32, y: f32) -> Pressed {
        let renderer = match part {
            Part::Panel => &mut self.panel,
            Part::Start => &mut self.start,
        };
        let answer = renderer.handle(ToRenderer::ConversationPress(Point::new(x, y)));
        decide(answer, part, &self.painted, &conversation.showing())
    }

    fn paint_panel(&mut self) -> Result<Frame, String> {
        match self.panel.handle(ToRenderer::Paint) {
            FromRenderer::Painted(frame) => Ok(frame),
            other => Err(format!("not painted: {other:?}")),
        }
    }

    fn panel_tree(&mut self) -> Snapshot {
        match self.panel.handle(ToRenderer::ReadTree) {
            FromRenderer::Tree(tree) => *tree,
            _ => Snapshot::default(),
        }
    }
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

/// The chats the panel's tree reads, each by its name and box, and which
/// is current.
fn chats(tree: &Snapshot) -> Vec<(Option<String>, String, bool)> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == "link")
        .map(|node| {
            (
                node.name.clone(),
                units(node.rect),
                node.states.current.is_some(),
            )
        })
        .collect()
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `frame` with the committed reference, or write it when updating.
/// What differs, in words, if anything does.
fn holds(name: &str, frame: &Frame) -> Result<(), String> {
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
            "{name}: the panel is {}×{}, and the reference {}×{}",
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

/// New chat, then the chat that was on screen before it, pressed: the
/// conversation after both, and the panel drawn again from it.
fn new_chat_then_the_first() -> Result<(Conversation, Here), String> {
    let mut conversation = Conversation::default();
    let first = conversation
        .on_screen()
        .ok_or("a new conversation has a chat on screen")?;
    let mut here = Here::of(&conversation);
    // New chat, 220 × 40 at (10, 56) (`alo-renderer`'s
    // `tests/the_chats_panel.rs`).
    let pressed = here.press(&conversation, Part::Panel, 60.0, 75.0);
    assert_eq!(pressed, Pressed::NewChat);
    assert!(conversation.act(pressed), "the panel is drawn again");
    here.draw(&conversation);
    let made = conversation.on_screen();
    assert_ne!(made, Some(first), "the new chat is on screen");
    // The first chat is now the second row, 38 tall at (10, 174).
    let pressed = here.press(&conversation, Part::Panel, 120.0, 190.0);
    assert_eq!(pressed, Pressed::Chat(first), "said by which chat it was");
    assert!(conversation.act(pressed));
    assert_eq!(conversation.on_screen(), Some(first), "and it is current");
    here.draw(&conversation);
    Ok((conversation, here))
}

#[test]
fn a_press_on_a_chat_makes_it_current_and_the_panel_is_drawn_again() {
    let (conversation, mut here) = new_chat_then_the_first().unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        chats(&here.panel_tree()),
        vec![
            (
                Some("New chat".to_owned()),
                px(10.0, 56.0, 220.0, 40.0),
                false
            ),
            (
                Some("New chat".to_owned()),
                px(10.0, 134.0, 220.0, 38.0),
                false
            ),
            (
                Some("New chat".to_owned()),
                px(10.0, 174.0, 220.0, 38.0),
                true
            ),
            (
                Some("Settings".to_owned()),
                px(10.0, 770.0, 220.0, 38.0),
                false
            ),
        ],
        "New chat, the chat it made, the first chat — current — and Settings"
    );
    let frame = here.paint_panel().unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((frame.width, frame.height), (240, 820));
    // The current row is white, the other the panel's warm grey.
    let pixel = |x: u32, y: u32| {
        let at = usize::try_from((y * frame.width + x) * 4).unwrap_or(usize::MAX);
        frame.pixels.get(at..at + 4).map(<[u8]>::to_vec)
    };
    assert_eq!(pixel(14, 193), Some(vec![0xff, 0xff, 0xff, 0xff]));
    assert_eq!(pixel(14, 153), Some(vec![0xee, 0xea, 0xe2, 0xff]));
    assert_eq!(
        holds("the-panel-after-a-press.png", &frame),
        Ok(()),
        "the panel after New chat and then the first chat"
    );
    assert_eq!(
        conversation
            .panel(Size::new(240.0, 820.0), Scale::ONE)
            .chats
            .len(),
        2
    );
}

#[test]
fn a_press_on_the_box_is_claimed_as_the_box_and_gives_it_the_keyboard() {
    let mut conversation = Conversation::default();
    let mut here = Here::of(&conversation);
    let before = asked(&conversation);
    // The box 720 × 64 at (160, 254) in the first screen; Go 44 square at
    // (825, 264). Go with nothing typed chooses nothing, and Add a site
    // changes nothing yet.
    for (x, y, decided) in [
        (846.0, 285.0, Pressed::Go),
        (215.0, 428.0, Pressed::AddASite),
    ] {
        let pressed = here.press(&conversation, Part::Start, x, y);
        assert_eq!(pressed, decided, "({x}, {y})");
        assert!(!conversation.act(pressed));
        assert_eq!(conversation.last_pressed(), Some(&decided));
    }
    let pressed = here.press(&conversation, Part::Panel, 120.0, 529.0);
    assert_eq!(pressed, Pressed::Nothing, "the room above Settings at 820");
    let pressed = here.press(&conversation, Part::Panel, 120.0, 789.0);
    assert_eq!(pressed, Pressed::Settings);
    assert!(!conversation.act(pressed));
    assert_eq!(asked(&conversation), before, "nothing drawn changed");
    assert_eq!(conversation.chosen(), None);
    // The box takes the keyboard, and the first screen is drawn again
    // showing so; the panel is not.
    let pressed = here.press(&conversation, Part::Start, 400.0, 285.0);
    assert_eq!(pressed, Pressed::TheBox);
    assert!(conversation.act(pressed), "the first screen is drawn again");
    let after = asked(&conversation);
    assert!(after.start.focused);
    assert_eq!(after.panel, before.panel);
}

#[test]
fn a_lying_claim_is_refused_by_name_and_changes_nothing() {
    let mut conversation = Conversation::default();
    let painted = conversation.showing();
    let before = asked(&conversation);
    for (claim, part, refusal) in [
        // A chat the browser process never showed or holds.
        (
            Claim::Chat { at: 1 },
            Part::Panel,
            Refusal::NotShown {
                listed: Listed::Chats,
                at: 1,
                shown: 1,
            },
        ),
        // A site, where none is kept.
        (
            Claim::Site { at: 0 },
            Part::Start,
            Refusal::NotShown {
                listed: Listed::Sites,
                at: 0,
                shown: 0,
            },
        ),
        // The box, claimed by the panel's renderer.
        (
            Claim::TheBox,
            Part::Panel,
            Refusal::NotDrawnThere {
                part: Part::Panel,
                claim: Claim::TheBox,
            },
        ),
    ] {
        let pressed = decide(
            FromRenderer::ConversationClaimed(Some(claim)),
            part,
            &painted,
            &conversation.showing(),
        );
        assert_eq!(pressed, Decided::Refused(refusal.clone()), "{claim:?}");
        assert!(!conversation.act(pressed));
    }
    assert_eq!(asked(&conversation), before);
    assert_eq!(
        Refusal::NotShown {
            listed: Listed::Chats,
            at: 1,
            shown: 1
        }
        .to_string(),
        "a renderer claimed the chat at place 1, and 1 were shown, and nothing was done"
    );
}

/// How long the confined renderers may take to answer.
const AT_MOST: Duration = Duration::from_secs(60);

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// The next thing the window is told that is not a frame of the other
/// part: a part painted, a line said, or a press decided.
fn next(told: &Receiver<Drawn>) -> Result<Drawn, String> {
    told.recv_timeout(AT_MOST)
        .map_err(|why| format!("nothing was told: {why}"))
}

/// Wait until both parts have painted, failing on anything said.
fn both_painted(told: &Receiver<Drawn>) -> Result<(), String> {
    let (mut panel, mut start) = (false, false);
    while !(panel && start) {
        match next(told)? {
            Drawn::Painted(Part::Panel, _) => panel = true,
            Drawn::Painted(Part::Start, _) => start = true,
            other => return Err(format!("not a frame: {other:?}")),
        }
    }
    Ok(())
}

/// Where the window's own aim sends a press at `at`, in device pixels, in
/// the design's window at one with both parts painted.
fn aimed(at: (f64, f64)) -> Result<(Part, Point), String> {
    let placed = ConversationPlacement::of(
        AT_ONE,
        Scale::ONE,
        Parts {
            panel: Some((240, 820)),
            start: Some((1040, 820)),
            ..Parts::default()
        },
    );
    aim(&placed, Scale::ONE, at).ok_or_else(|| format!("{at:?} is over nothing"))
}

/// A press at `at`, in the window's device pixels, aimed as the window aims
/// it and handed to `drawing`: what it was decided to be.
fn press(
    drawing: &mut ConversationDrawing,
    told: &Receiver<Drawn>,
    at: (f64, f64),
) -> Result<Pressed, String> {
    let (part, on) = aimed(at)?;
    drawing.press(part, on);
    match next(told)? {
        Drawn::Pressed(said, pressed) if said == part => Ok(pressed),
        other => Err(format!("not a decision on {part}: {other:?}")),
    }
}

/// The window's road, through confined renderers: a point aimed, pressed,
/// decided by the thread, acted on, and the panel drawn again.
fn pressed_through_confined_renderers() -> Result<Frame, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, told) = channel();
    let mut drawing =
        ConversationDrawing::start(renderers, Box::new(move |drawn| tell.send(drawn).is_ok()))
            .map_err(|why| format!("no thread: {why}"))?;
    let mut conversation = Conversation::default();
    let first = conversation.on_screen().ok_or("no chat on screen")?;
    drawing.draw(asked(&conversation));
    both_painted(&told)?;
    // The box, at (640, 285) of the window: 400 into the first screen.
    assert_eq!(press(&mut drawing, &told, (640.0, 285.0))?, Pressed::TheBox);
    // New chat.
    let pressed = press(&mut drawing, &told, (60.0, 75.0))?;
    assert_eq!(pressed, Pressed::NewChat);
    assert!(conversation.act(pressed));
    drawing.draw(asked(&conversation));
    match next(&told)? {
        Drawn::Painted(Part::Panel, _) => {}
        other => return Err(format!("the panel was not drawn again: {other:?}")),
    }
    // The first chat, now the second row.
    let pressed = press(&mut drawing, &told, (120.0, 190.0))?;
    assert_eq!(pressed, Pressed::Chat(first));
    assert!(conversation.act(pressed));
    drawing.draw(asked(&conversation));
    let frame = match next(&told)? {
        Drawn::Painted(Part::Panel, frame) => frame,
        other => return Err(format!("the panel was not drawn again: {other:?}")),
    };
    // A press between two rows claims nothing.
    assert_eq!(
        press(&mut drawing, &told, (120.0, 172.0))?,
        Pressed::Nothing
    );
    if !drawing.finish() {
        return Err("the drawing thread did not finish".to_owned());
    }
    Ok(frame)
}

#[test]
fn the_same_presses_through_two_confined_renderers() {
    let frame = pressed_through_confined_renderers().unwrap_or_else(|why| panic!("{why}"));
    let (_, mut here) = new_chat_then_the_first().unwrap_or_else(|why| panic!("{why}"));
    let painted_here = here.paint_panel().unwrap_or_else(|why| panic!("{why}"));
    assert!(
        frame == painted_here,
        "the confined renderer painted another panel after the presses"
    );
}

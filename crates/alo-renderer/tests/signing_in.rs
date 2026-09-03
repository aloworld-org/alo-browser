//! An agent filling in alo's own sign-in screen.
//!
//! Not a unit test of anything: it is the thing this project exists for, done
//! end to end through the boundary. An agent reads the page, finds the fields
//! by the names a person would read out, types into them, ticks a box, and
//! reads the page back to check.
//!
//! It is `alo-workplace`'s sign-in markup, kept small enough to read here. The
//! full screen with its real stylesheet is
//! `crates/alo-corpus/cases/alo-sign-in/`.

use alo_agent::{Target, Verb};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const SIGN_IN: &str = "<!DOCTYPE html><html><body><main>\
<h1>Sign in</h1>\
<form aria-label='Sign in'>\
<label for=email>Email</label><input id=email type=email>\
<label for=password>Password</label><input id=password type=password>\
<input type=checkbox id=remember><label for=remember>Remember me</label>\
<button type=submit>Sign in</button>\
</form></main></body></html>";

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 14px }
main { padding: 16px } h1 { font-size: 20px; margin: 0 }
input { display: block }";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

/// Read the tree, or an empty one when the renderer answered something else.
///
/// Empty rather than a panic in a helper, so that the assertion which asked
/// the question is the thing that fails and says what it wanted.
fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

fn expect_acted(answer: &FromRenderer) {
    assert!(
        matches!(answer, FromRenderer::Acted(_)),
        "the verb should have run: {answer:?}",
    );
}

#[test]
fn an_agent_can_fill_in_the_sign_in_screen_and_read_back_what_it_did() {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(
        Page::new(SIGN_IN, Size::new(360.0, 320.0)).with_sheet(SHEET),
    )));

    // Read it first, by name, exactly as ADR 0002 says an agent should.
    let before = read(&mut renderer).to_outline();
    assert!(before.contains("textbox \"Email\""), "{before}");
    assert!(before.contains("checkbox \"Remember me\""), "{before}");
    assert!(before.contains("button \"Sign in\""), "{before}");

    expect_acted(&renderer.handle(ToRenderer::Act {
        target: Target::Named("Email".to_owned()),
        verb: Verb::PutText("someone@alo.build".to_owned()),
    }));
    expect_acted(&renderer.handle(ToRenderer::Act {
        target: Target::Named("Password".to_owned()),
        verb: Verb::PutText("correct horse".to_owned()),
    }));
    expect_acted(&renderer.handle(ToRenderer::Act {
        target: Target::Named("Remember me".to_owned()),
        verb: Verb::Activate,
    }));

    let after = read(&mut renderer).to_outline();
    assert!(
        after.contains("someone@alo.build"),
        "the email is in the field:\n{after}",
    );
    assert!(
        after.contains("checkbox \"Remember me\" [checked=true]"),
        "the box is ticked:\n{after}",
    );
    assert!(
        !after.contains("correct horse"),
        "and the password is never read back:\n{after}",
    );
}

#[test]
fn the_screen_is_drawn_with_what_was_typed_in_it() {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(
        Page::new(SIGN_IN, Size::new(360.0, 320.0)).with_sheet(SHEET),
    )));
    renderer.handle(ToRenderer::Act {
        target: Target::Named("Email".to_owned()),
        verb: Verb::PutText("someone@alo.build".to_owned()),
    });

    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => assert_eq!((frame.width, frame.height), (360, 320)),
        other => panic!("expected a picture, got {other:?}"),
    }
    let drawn = renderer.rendered().expect("a render").display.to_outline();
    assert!(
        drawn.contains("someone@alo.build"),
        "what was typed is on the screen, not only in the tree:\n{drawn}",
    );
}

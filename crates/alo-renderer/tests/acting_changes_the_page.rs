//! A verb changes the page, and the page an agent reads next is the changed
//! one.
//!
//! Until queue item 42 a verb decided and reported and nothing happened. These
//! are the two halves of what "typed verbs" has to mean: what changed, and —
//! the harder one — that **the ids an agent is holding still name the same
//! things afterwards**. ADR 0003 promises that, and a page that re-parsed
//! itself on every keystroke would break it silently.

use alo_agent::{Target, Verb};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 14px }";

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

fn renderer(html: &str) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(
        Page::new(html, Size::new(300.0, 200.0)).with_sheet(SHEET),
    )));
    renderer
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

fn act(renderer: &mut Renderer, target: Target, verb: Verb) -> FromRenderer {
    renderer.handle(ToRenderer::Act { target, verb })
}

const FORM: &str = "<!DOCTYPE html><html><body><form aria-label='Sign in'>\
<input id=email aria-label='Email'>\
<input type=checkbox id=remember aria-label='Remember me'>\
<div role=switch aria-checked='false' aria-label='Notify me'></div>\
<input type=radio name=plan id=free aria-label='Free' checked>\
<input type=radio name=plan id=paid aria-label='Paid'>\
</form></body></html>";

#[test]
fn text_put_into_a_field_is_there_when_the_page_is_read_again() {
    let mut renderer = renderer(FORM);
    act(
        &mut renderer,
        Target::Named("Email".to_owned()),
        Verb::PutText("someone@example.com".to_owned()),
    );
    let outline = read(&mut renderer).to_outline();
    assert!(
        outline.contains("someone@example.com"),
        "the field holds what was put into it:\n{outline}",
    );
}

#[test]
fn a_field_shows_what_it_holds() {
    // Not only the tree: the text is in the page, so it is laid out and drawn.
    let mut renderer = renderer(FORM);
    act(
        &mut renderer,
        Target::Named("Email".to_owned()),
        Verb::PutText("hello".to_owned()),
    );
    let inside = renderer.rendered().expect("a render");
    assert!(
        inside.display.to_outline().contains("\"hello\""),
        "a field draws its value:\n{}",
        inside.display.to_outline(),
    );
}

#[test]
fn a_password_shows_that_it_holds_something_and_never_what() {
    let mut renderer = renderer(
        "<!DOCTYPE html><html><body><input type=password id=p aria-label='Password'>\
</body></html>",
    );
    act(
        &mut renderer,
        Target::Named("Password".to_owned()),
        Verb::PutText("hunter2".to_owned()),
    );
    let drawn = renderer.rendered().expect("a render").display.to_outline();
    assert!(!drawn.contains("hunter2"), "{drawn}");
    assert!(drawn.contains("•••••••"), "one dot a character:\n{drawn}");
}

#[test]
fn a_checkbox_toggles_and_says_so_both_times() {
    let mut renderer = renderer(FORM);
    assert!(read(&mut renderer).to_outline().contains("checked=false"));

    act(
        &mut renderer,
        Target::Named("Remember me".to_owned()),
        Verb::Activate,
    );
    assert!(
        read(&mut renderer).to_outline().contains("checked=true"),
        "{}",
        read(&mut renderer).to_outline(),
    );

    act(
        &mut renderer,
        Target::Named("Remember me".to_owned()),
        Verb::Activate,
    );
    assert!(read(&mut renderer).to_outline().contains("checked=false"));
}

#[test]
fn a_switch_the_author_declared_with_aria_is_changed_with_aria() {
    let mut renderer = renderer(FORM);
    act(
        &mut renderer,
        Target::Named("Notify me".to_owned()),
        Verb::Activate,
    );
    let outline = read(&mut renderer).to_outline();
    assert!(
        outline.contains("switch \"Notify me\" [checked=true]"),
        "{outline}",
    );
}

#[test]
fn choosing_a_radio_unchooses_the_rest_of_its_group() {
    // A radio does not toggle. Choosing one has to un-choose the others, or
    // the page ends up in a state a person could not have put it in.
    let mut renderer = renderer(FORM);
    let before = read(&mut renderer).to_outline();
    assert!(before.contains("radio \"Free\" [checked=true]"), "{before}");

    act(
        &mut renderer,
        Target::Named("Paid".to_owned()),
        Verb::Activate,
    );
    let after = read(&mut renderer).to_outline();
    assert!(after.contains("radio \"Paid\" [checked=true]"), "{after}");
    assert!(after.contains("radio \"Free\" [checked=false]"), "{after}");
}

#[test]
fn the_ids_an_agent_is_holding_still_name_the_same_things_afterwards() {
    // The one that would fail silently. A page that re-parsed itself would
    // mint new ids on every keystroke, every snapshot anybody held would be
    // stale, and nothing would say so — the verb would simply act on the wrong
    // thing or on nothing. ADR 0003 is what makes this assertion possible and
    // this test is what makes it true.
    let mut renderer = renderer(FORM);
    let before = read(&mut renderer);
    let email = before
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some("Email"))
        .expect("the field")
        .id;

    act(
        &mut renderer,
        Target::Named("Remember me".to_owned()),
        Verb::Activate,
    );

    // The id read *before* the change still names the field afterwards.
    let answer = act(
        &mut renderer,
        Target::Node(email),
        Verb::PutText("still me".to_owned()),
    );
    match answer {
        FromRenderer::Acted(outcome) => assert_eq!(outcome.node(), email),
        other => panic!("a held id went stale: {other:?}"),
    }
    assert!(read(&mut renderer).to_outline().contains("still me"));
}

#[test]
fn activating_a_button_changes_nothing_because_there_is_nothing_to_change() {
    // Correct rather than missing: what a button does is run a script, and a
    // page without one does nothing when it is pressed. The verb still says
    // what it pressed, which is what a caller asked.
    let mut renderer = renderer("<!DOCTYPE html><html><body><button>Save</button></body></html>");
    let before = read(&mut renderer).to_outline();
    match act(
        &mut renderer,
        Target::Named("Save".to_owned()),
        Verb::Activate,
    ) {
        FromRenderer::Acted(outcome) => assert!(outcome.to_string().contains("Save")),
        other => panic!("expected the verb to run, got {other:?}"),
    }
    assert_eq!(read(&mut renderer).to_outline(), before);
}

#[test]
fn a_verb_that_is_refused_changes_nothing() {
    let mut renderer = renderer(FORM);
    let before = read(&mut renderer).to_outline();
    let answer = act(
        &mut renderer,
        Target::Named("Nothing called this".to_owned()),
        Verb::Activate,
    );
    assert!(matches!(answer, FromRenderer::Refused(_)), "{answer:?}");
    assert_eq!(read(&mut renderer).to_outline(), before);
}

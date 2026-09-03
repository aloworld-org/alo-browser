//! An agent reading alo's Settings screen and acting on it by name.
//!
//! The last clause of stage 1's exit gate, and the reason this repository
//! exists rather than a faster fork of somebody else's engine (`CLAUDE.md`,
//! ★ the agent surface). Everything it uses was built and tested before —
//! the tree, the verbs, the screen. **What this adds is a real screen**, which
//! is where a role declared wrongly, a name that reads twice, or a control
//! nothing can find actually shows up. None of those survive a page somebody
//! wrote to test with; all of them survive a specification.
//!
//! The screen is `crates/alo-corpus/cases/alo-settings/`, read from disk, so
//! this test and the committed reference render are looking at the same thing.

use alo_agent::{Target, Verb};
use alo_box::{KnownRole, Role};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::PathBuf;

fn case() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/alo-settings")
}

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

/// alo's Settings screen, loaded into a renderer.
fn settings() -> Renderer {
    // Empty rather than a panic in a helper: the assertion below is what says
    // the screen was read, and it says it with the file that was missing.
    let html = std::fs::read_to_string(case().join("page.html")).unwrap_or_default();
    let css = std::fs::read_to_string(case().join("style.css")).unwrap_or_default();
    assert!(
        !html.is_empty() && !css.is_empty(),
        "the case is at {}",
        case().display()
    );
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(html, Size::new(900.0, 600.0)).with_sheet(css),
    )));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues } if issues.is_empty()),
        "alo's own screen asks for nothing this engine refuses: {answer:?}",
    );
    renderer
}

fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

fn named<'a>(snapshot: &'a Snapshot, name: &str) -> Option<&'a SnapshotNode> {
    snapshot
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))
}

#[test]
fn an_agent_reads_settings_as_what_it_is() {
    let mut renderer = settings();
    let snapshot = read(&mut renderer);
    let outline = snapshot.to_outline();

    // What the screen *is*, in the words a person would use for it.
    assert!(outline.contains("dialog \"Mail settings\""), "{outline}");
    assert!(
        outline.contains("navigation \"Mail settings\""),
        "{outline}"
    );
    for row in [
        "General",
        "Filters & rules",
        "Sharing",
        "Notifications",
        "App passwords",
    ] {
        let node = named(&snapshot, row).unwrap_or_else(|| panic!("no row called {row}"));
        assert_eq!(
            node.role,
            Role::Known(KnownRole::Button),
            "{row} is a thing that can be operated",
        );
    }
    assert!(
        outline.contains("heading \"Your signature\" [level=3]"),
        "{outline}"
    );
    assert!(
        outline.contains("heading \"Out of office\" [level=3]"),
        "{outline}"
    );
}

#[test]
fn the_tree_says_which_section_is_open() {
    // ADR 0002's own example is "invoice list, twelve rows, row three
    // selected". This is that sentence for this screen, and an agent that
    // could not read it would have to guess from a colour.
    let mut renderer = settings();
    let snapshot = read(&mut renderer);

    let open = named(&snapshot, "General").expect("the open section");
    assert!(open.states.current.is_some(), "General is the current one");
    for other in [
        "Filters & rules",
        "Sharing",
        "Notifications",
        "App passwords",
    ] {
        let node = named(&snapshot, other).expect("a row");
        assert!(node.states.current.is_none(), "{other} is not");
    }
}

#[test]
fn an_agent_activates_a_row_by_name_and_never_by_position() {
    // The exit gate's own words. The row is named, not pointed at: no
    // coordinate is involved anywhere in this, and `Target` has no way to
    // express one (ADR 0002).
    let mut renderer = settings();
    let snapshot = read(&mut renderer);
    let row = named(&snapshot, "Filters & rules").expect("the row");

    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Filters & rules".to_owned()),
        verb: Verb::Activate,
    });
    match answer {
        FromRenderer::Acted(outcome) => {
            assert_eq!(outcome.node(), row.id, "the row it named, not another");
            assert!(outcome.to_string().contains("Filters & rules"));
        }
        other => panic!("the row should be operable: {other:?}"),
    }
}

#[test]
fn what_a_row_does_next_needs_a_script_and_this_says_so() {
    // Honest about the edge of stage 1. Pressing a nav row runs the page's
    // own code, and there is none — so the verb finds the row, reports what
    // it pressed, and the screen does not change. A browser that pretended
    // otherwise would be lying to the thing driving it.
    let mut renderer = settings();
    let before = read(&mut renderer).to_outline();
    renderer.handle(ToRenderer::Act {
        target: Target::Named("Sharing".to_owned()),
        verb: Verb::Activate,
    });
    assert_eq!(read(&mut renderer).to_outline(), before);
}

#[test]
fn an_agent_operates_the_controls_that_do_not_need_one() {
    // The other half: everything on this screen that is a document state
    // rather than a script's, driven by name and read back.
    let mut renderer = settings();

    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Send automatic replies".to_owned()),
        verb: Verb::Activate,
    });
    assert!(matches!(answer, FromRenderer::Acted(_)), "{answer:?}");

    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("First day away".to_owned()),
        verb: Verb::PutText("3 June".to_owned()),
    });
    assert!(matches!(answer, FromRenderer::Acted(_)), "{answer:?}");

    let outline = read(&mut renderer).to_outline();
    assert!(
        outline.contains("checkbox \"Send automatic replies\" [checked=true]"),
        "the box is ticked:\n{outline}",
    );
    assert!(
        outline.contains("3 June"),
        "and the date is typed:\n{outline}"
    );
}

#[test]
fn asking_for_something_the_screen_does_not_have_is_refused_by_name() {
    let mut renderer = settings();
    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Delete everything".to_owned()),
        verb: Verb::Activate,
    });
    assert!(
        matches!(answer, FromRenderer::Refused(_)),
        "acting on the wrong row is worse than acting on none: {answer:?}",
    );
}

#[test]
fn every_row_is_somewhere_and_the_agent_knows_where_without_looking() {
    // ADR 0002: the tree knows where everything is, so an agent never needs a
    // screenshot to find out. The positions are the layout's own — nothing
    // here measured a picture.
    let mut renderer = settings();
    let snapshot = read(&mut renderer);
    let mut previous = 0.0_f32;
    for row in [
        "General",
        "Filters & rules",
        "Sharing",
        "Notifications",
        "App passwords",
    ] {
        let node = named(&snapshot, row).expect("a row");
        assert!(
            node.rect.size.width > 0.0 && node.rect.size.height > 0.0,
            "{row}"
        );
        assert!(
            node.rect.top() > previous,
            "the rows are in the order a person meets them: {row}",
        );
        assert!(!node.offscreen, "{row} is on the screen");
        previous = node.rect.top();
    }
}

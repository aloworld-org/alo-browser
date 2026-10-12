/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent reading and driving pages nobody wrote for us, by name: queue
//! item 131.
//!
//! ADR 0002 held on alo's own screens because we wrote them. This is where
//! it holds or does not on somebody else's markup, so every page here is a
//! frozen web case — httpbin's copy of the HTML specification's own pizza
//! form, and the first web page — read from disk exactly as it was served.
//! Nothing here names a coordinate: every verb is aimed at what a person
//! would read out, and every check reads the tree back rather than looking
//! at a picture.
//!
//! The form found three things a page of ours never had. Its fields sit
//! *inside* their labels, and the text a field held was folded into the
//! label's words and left out of the tree, so an agent could put a name in
//! and never read it back. Text put into its `<textarea>` was told it had
//! gone in and went nowhere anything drew or read. And text put into its
//! `time` field was accepted and drawn by nothing. The first two are fixed;
//! the third is refused by name until a time field is built.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_box::{KnownRole, Role};
use alo_corpus::{Case, cases_directory, corpus_fonts};
use alo_layout::{Rect, Size};
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, SnapshotNode, ToRenderer};

/// A frozen web case, loaded into a renderer of its own at the address it
/// was taken from — the first line of its `origin.txt` — so its relative
/// links resolve as they did when it was served.
fn loaded(name: &str) -> Renderer {
    let case = Case::read(&cases_directory().join(name));
    let origin = std::fs::read_to_string(cases_directory().join(name).join("origin.txt"))
        .unwrap_or_default();
    let address = origin
        .lines()
        .next()
        .and_then(|line| alo_url::parse(line.trim()).ok());
    assert!(
        case.is_some() && address.is_some(),
        "{name} is a case with an origin"
    );
    let mut renderer = Renderer::new(corpus_fonts());
    let (Some(case), Some(address)) = (case, address) else {
        return renderer;
    };
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(case.html.clone(), Size::new(case.size.0, case.size.1)).at(address),
    )));
    assert!(
        matches!(answer, FromRenderer::Loaded { .. }),
        "{name} loads: {answer:?}"
    );
    renderer
}

/// The tree as the renderer sends it, or an empty one when it answered
/// something else, so the assertion that asked is what fails.
fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

/// The one node called `name` with the role `role`, if there is one.
fn the<'a>(snapshot: &'a Snapshot, role: KnownRole, name: &str) -> Option<&'a SnapshotNode> {
    snapshot
        .nodes()
        .into_iter()
        .find(|node| node.role == Role::Known(role) && node.name.as_deref() == Some(name))
}

/// Ask for `verb` on whatever is called `name`.
fn act(renderer: &mut Renderer, name: &str, verb: Verb) -> FromRenderer {
    renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb,
    })
}

/// What the verb came to, when it was carried out.
fn outcome(answer: FromRenderer) -> Option<Outcome> {
    match answer {
        FromRenderer::Acted { outcome, .. } => Some(outcome),
        _ => None,
    }
}

/// Why the verb was refused, when it was.
fn refusal(answer: FromRenderer) -> Option<Refusal> {
    match answer {
        FromRenderer::Refused(refusal) => Some(refusal),
        _ => None,
    }
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

/// Whether `rect` is at `(x, y)` and `width × height`, to a thousandth.
fn at(rect: Rect, x: f32, y: f32, width: f32, height: f32) -> bool {
    near(rect.origin.x, x)
        && near(rect.origin.y, y)
        && near(rect.size.width, width)
        && near(rect.size.height, height)
}

/// The pizza form, filled in by name as a person would read it out.
fn filled() -> Renderer {
    let mut form = loaded("web-a-form");
    for (field, text) in [
        ("Customer name:", "Ada Lovelace"),
        ("Telephone:", "+44 20 7946 0000"),
        ("E-mail address:", "ada@example.com"),
        ("Delivery instructions:", "Ring twice"),
    ] {
        let done = outcome(act(&mut form, field, Verb::PutText(text.to_owned())));
        assert!(
            matches!(&done, Some(Outcome::TextPut { text: put, .. }) if put == text),
            "{field} takes {text:?}: {done:?}"
        );
    }
    for choice in ["Large", "Bacon", "Mushroom"] {
        let done = outcome(act(&mut form, choice, Verb::Activate));
        assert!(
            matches!(&done, Some(Outcome::Activated { name: Some(said), .. }) if said == choice),
            "{choice} is operated: {done:?}"
        );
    }
    form
}

#[test]
fn the_form_reads_as_what_it_is_in_the_words_on_it() {
    let mut form = loaded("web-a-form");
    let tree = read(&mut form);
    for field in [
        "Customer name:",
        "Telephone:",
        "E-mail address:",
        "Delivery instructions:",
    ] {
        assert!(
            the(&tree, KnownRole::TextBox, field).is_some(),
            "{field} is a textbox named by the label around it:\n{}",
            tree.to_outline()
        );
    }
    for size in ["Small", "Medium", "Large"] {
        assert!(the(&tree, KnownRole::Radio, size).is_some(), "{size}");
    }
    for topping in ["Bacon", "Extra Cheese", "Onion", "Mushroom"] {
        assert!(
            the(&tree, KnownRole::CheckBox, topping).is_some(),
            "{topping}"
        );
    }
    assert!(the(&tree, KnownRole::Group, "Pizza Size").is_some());
    assert!(the(&tree, KnownRole::Group, "Pizza Toppings").is_some());
    assert!(the(&tree, KnownRole::Button, "Submit order").is_some());
}

#[test]
fn what_an_agent_puts_in_a_field_inside_its_label_is_read_back() {
    let mut form = filled();
    let tree = read(&mut form);
    for (field, text) in [
        ("Customer name:", "Ada Lovelace"),
        ("Telephone:", "+44 20 7946 0000"),
        ("E-mail address:", "ada@example.com"),
        ("Delivery instructions:", "Ring twice"),
    ] {
        let Some(node) = the(&tree, KnownRole::TextBox, field) else {
            panic!("{field} is in the tree");
        };
        let held: Vec<Option<&str>> = node
            .children
            .iter()
            .map(|child| child.name.as_deref())
            .collect();
        assert_eq!(
            held,
            [Some(text)],
            "{field} holds what was put in it, once, and the label's words are not read again:\n{}",
            tree.to_outline()
        );
    }
}

#[test]
fn the_text_put_in_each_field_sits_inside_it_in_numbers() {
    // The fields' own boxes are the frozen case's (`agent.txt`): the name
    // field at (146.65625, 16), the textarea at (183.67188, 537.1875), on
    // the grid of 1/64 px since item 466 (ADR 0054). The textarea rose
    // 0.75 when each one-line field above it became its font's line, 19,
    // rather than 1.2 em, 19.2, on the grid (item 467), and fell 8 when
    // the four fields above it took Chrome's 2 px border (item 505). Then
    // every control took Chrome's own 13.33 px font rather than the page's
    // 16 (item 506): each one-line field is 21 tall, not 25, so the
    // textarea rose 16 to 537.1875, and what is typed is set in 13.33 px,
    // a 15 px line, in `DejaVu Sans` for the field and `DejaVu Sans Mono`
    // for the textarea, which is `monospace` as Chrome's is. The text each
    // holds sits its border and padding in from that corner — 4 pixels
    // across and 3 down in the one-line field, 3 and 3 in the textarea,
    // whose border is still Chrome's one pixel. And 13.333333 px is set at
    // 13.33 (item 514), so "Ada Lovelace" is 5,669.83 units wide, which a
    // box holds in 5,670, Chrome 154's width for it; at 13.333333 it was
    // 5,671.25 and a unit wider. "Ring twice" in `DejaVu Sans Mono` is
    // 5,136.22, held in Chrome's 5,137, where it was 5,137.5.
    let mut form = filled();
    let tree = read(&mut form);
    let text_of = |field: &str| {
        the(&tree, KnownRole::TextBox, field)
            .and_then(|node| node.children.first())
            .map(|text| text.rect)
    };
    let (Some(name), Some(instructions)) =
        (text_of("Customer name:"), text_of("Delivery instructions:"))
    else {
        panic!("both fields hold text:\n{}", tree.to_outline());
    };
    assert!(
        at(name, 150.656_25, 19.0, 88.591_125, 15.0),
        "Ada Lovelace: {name:?}"
    );
    assert!(
        at(instructions, 186.671_88, 540.1875, 80.253_37, 15.0),
        "Ring twice: {instructions:?}"
    );

    // And it is drawn there, in the field's colour, on its baseline.
    let drawn = form
        .rendered()
        .map(|drawing| drawing.display.to_outline())
        .unwrap_or_default();
    assert!(
        drawn.contains("\"Ada Lovelace\" rgb(0 0 0) 13.333333px at (150.65625, 31)"),
        "{drawn}"
    );
    assert!(
        drawn.contains("\"Ring twice\" rgb(0 0 0) 13.333333px at (186.67188, 552.1875)"),
        "{drawn}"
    );
}

#[test]
fn the_filled_form_is_drawn_as_its_reference_says() {
    // A reference render of the form after the agent filled it, beside the
    // case's own `render.png` of it empty: a change that moves a pixel of
    // what an agent put in says so. Rewritten, like every reference, with
    // `ALO_UPDATE_REFERENCES` set.
    let form = filled();
    let Some(drawing) = form.rendered() else {
        panic!("the form is drawn");
    };
    let path = cases_directory().join("web-a-form").join("filled.png");
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        let written = to_png(&drawing.canvas)
            .ok()
            .and_then(|bytes| std::fs::write(&path, bytes).ok());
        assert!(written.is_some(), "{} is written", path.display());
        return;
    }
    let reference: Option<Canvas> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| from_png(&bytes).ok());
    let Some(reference) = reference else {
        panic!("{} is the reference", path.display());
    };
    assert_eq!(
        (reference.width(), reference.height()),
        (drawing.canvas.width(), drawing.canvas.height())
    );
    let differing = reference
        .pixels()
        .iter()
        .zip(drawing.canvas.pixels())
        .filter(|(want, got)| want.to_rgba8() != got.to_rgba8())
        .count();
    assert_eq!(differing, 0, "pixels differ from {}", path.display());
}

#[test]
fn choosing_a_size_unchooses_the_other_and_toppings_are_each_their_own() {
    let mut form = filled();
    let tree = read(&mut form);
    let outline = tree.to_outline();
    for line in [
        "radio \"Small\" [checked=false]",
        "radio \"Medium\" [checked=false]",
        "radio \"Large\" [checked=true]",
        "checkbox \"Bacon\" [checked=true]",
        "checkbox \"Extra Cheese\" [checked=false]",
        "checkbox \"Onion\" [checked=false]",
        "checkbox \"Mushroom\" [checked=true]",
    ] {
        assert!(outline.contains(line), "{line}:\n{outline}");
    }

    // One size at a time: they share `name=size`.
    let done = outcome(act(&mut form, "Small", Verb::Activate));
    assert!(done.is_some(), "Small is operated");
    let outline = read(&mut form).to_outline();
    assert!(
        outline.contains("radio \"Small\" [checked=true]"),
        "{outline}"
    );
    assert!(
        outline.contains("radio \"Large\" [checked=false]"),
        "{outline}"
    );
}

#[test]
fn the_same_refusals_hold_on_somebody_elses_page() {
    let mut form = loaded("web-a-form");

    // A legend names its group and is also words on the page: two things
    // answer to it, and choosing one would be a guess. Narrowed by role, it
    // is one thing — and a group is not a thing to press.
    let ambiguous = refusal(act(&mut form, "Pizza Size", Verb::Activate));
    assert!(
        matches!(&ambiguous, Some(Refusal::Ambiguous { candidates, .. }) if candidates.len() == 2),
        "{ambiguous:?}"
    );
    let group = refusal(form.handle(ToRenderer::Act {
        target: Target::NamedOfRole {
            role: Role::Known(KnownRole::Group),
            name: "Pizza Size".to_owned(),
        },
        verb: Verb::Activate,
    }));
    assert!(
        matches!(group, Some(Refusal::NotOperable { .. })),
        "{group:?}"
    );

    let not_a_field = refusal(act(&mut form, "Large", Verb::PutText("x".to_owned())));
    assert!(
        matches!(not_a_field, Some(Refusal::NotAField { .. })),
        "{not_a_field:?}"
    );
    let not_operable = refusal(act(&mut form, "Telephone:", Verb::Activate));
    assert!(
        matches!(not_operable, Some(Refusal::NotOperable { .. })),
        "{not_operable:?}"
    );
    let not_found = refusal(act(&mut form, "Pay now", Verb::Activate));
    assert!(
        matches!(not_found, Some(Refusal::NotFound { .. })),
        "{not_found:?}"
    );

    // The time field draws nothing it holds yet, so it is not told it took
    // a time — and the page is as it was.
    let before = read(&mut form);
    let unbuilt = refusal(act(
        &mut form,
        "Preferred delivery time:",
        Verb::PutText("12:15".to_owned()),
    ));
    assert!(
        matches!(&unbuilt, Some(Refusal::Unbuilt { kind, .. }) if kind == "time"),
        "{unbuilt:?}"
    );
    assert_eq!(read(&mut form), before, "nothing changed");
}

#[test]
fn the_first_web_page_is_followed_by_the_words_of_its_links() {
    let mut page = loaded("web-first-website");
    let done = outcome(act(&mut page, "hypermedia", Verb::Activate));
    assert!(
        matches!(&done, Some(Outcome::Followed { to, .. }) if to == "WhatIs.html"),
        "the link, by its words, with where it goes as written: {done:?}"
    );

    // Where it was asked to go is the address the page was served from,
    // resolved, and the browser process's to decide.
    let answer = act(&mut page, "What's out there?", Verb::Activate);
    let FromRenderer::Acted {
        navigation: Some(asked),
        ..
    } = answer
    else {
        panic!("following a link asks to go somewhere: {answer:?}");
    };
    assert_eq!(
        asked.url,
        "http://info.cern.ch/hypertext/DataSources/Top.html"
    );

    // A heading is read, not pressed; and of the page's twenty-five links,
    // "the link" is no particular one.
    let heading = refusal(act(&mut page, "World Wide Web", Verb::Activate));
    assert!(
        matches!(heading, Some(Refusal::NotOperable { .. })),
        "{heading:?}"
    );
    let any_link = refusal(page.handle(ToRenderer::Act {
        target: Target::OfRole(Role::Known(KnownRole::Link)),
        verb: Verb::Activate,
    }));
    assert!(
        matches!(&any_link, Some(Refusal::Ambiguous { candidates, .. }) if candidates.len() == 25),
        "{any_link:?}"
    );
}

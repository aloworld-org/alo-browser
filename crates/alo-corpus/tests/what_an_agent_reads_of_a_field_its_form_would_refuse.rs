/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the agent tree says of a field its form would refuse, before and
//! after the press, on two frozen pages: queue item 476, ADR 0051 § 6.
//!
//! Every reading here is Chrome 154's accessibility tree on the same frozen
//! bytes, served at the same address, read over `DevTools` (iteration 301):
//!
//! - `alo-sites-booking`'s required date field is `invalid` as the page
//!   loads, still `invalid` after *See free times* is pressed and refused,
//!   and not `invalid` once a day is in it;
//! - `alo-sites-contact-form`'s three required text fields are **not**
//!   `invalid` while empty, before the press and after it; its email field
//!   holding `nope` is.
//!
//! Both pages are loaded whole, script and all.

use alo_agent::{Outcome, Target, Verb};
use alo_box::{Invalid, KnownRole, Role, States};
use alo_corpus::{Case, cases_directory, corpus_fonts};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};

/// The frozen case `name`, loaded at its `address.txt`.
fn loaded(name: &str) -> Option<Renderer> {
    let directory = cases_directory().join(name);
    let case = Case::read(&directory)?;
    let address = std::fs::read_to_string(directory.join("address.txt")).ok()?;
    let address = alo_url::parse(address.lines().next()?.trim()).ok()?;
    let mut renderer = Renderer::new(corpus_fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(case.html, Size::new(case.size.0, case.size.1)).at(address),
    )));
    matches!(answer, FromRenderer::Loaded { .. }).then_some(renderer)
}

/// The tree as the renderer sends it, or an empty one when it answered
/// something else, so the assertion that asked is what fails.
fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

/// What is true of the one box called `name` with the role `role`.
fn states(renderer: &mut Renderer, role: KnownRole, name: &str) -> Option<States> {
    read(renderer)
        .nodes()
        .into_iter()
        .find(|node| node.role == Role::Known(role) && node.name.as_deref() == Some(name))
        .map(|node| node.states)
}

/// Ask for `verb` on whatever is called `name`, and what it came to.
fn act(renderer: &mut Renderer, name: &str, verb: Verb) -> Option<Outcome> {
    match renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb,
    }) {
        FromRenderer::Acted { outcome, .. } => Some(outcome),
        _ => None,
    }
}

#[test]
fn the_booking_pages_empty_day_is_invalid_before_and_after_the_press() {
    let mut page = loaded("alo-sites-booking").expect("the frozen page loads");
    let day = |page: &mut Renderer| states(page, KnownRole::Date, "Choose a day");

    // As it loads, before anybody pressed anything.
    let before = day(&mut page).expect("the date field is in the tree");
    assert_eq!(before.invalid, Some(Invalid::Yes));
    assert!(before.required);
    assert_eq!(before.to_string(), "required invalid");

    // The press is refused, and the field reads as it did.
    let pressed = act(&mut page, "See free times", Verb::Activate);
    assert!(
        matches!(pressed, Some(Outcome::NotSubmitted { .. })),
        "{pressed:?}"
    );
    assert_eq!(
        day(&mut page).and_then(|states| states.invalid),
        Some(Invalid::Yes)
    );

    // A day in it, and it is not invalid any more.
    let put = act(
        &mut page,
        "Choose a day",
        Verb::PutText("2026-10-20".to_owned()),
    );
    assert!(put.is_some(), "the day was put in");
    let after = day(&mut page).expect("the date field is still in the tree");
    assert_eq!(after.invalid, None);
    assert_eq!(after.to_string(), "required");
}

#[test]
fn the_contact_forms_empty_fields_are_not_invalid_and_a_bad_address_is() {
    let mut page = loaded("alo-sites-contact-form").expect("the frozen page loads");
    let invalid = |page: &mut Renderer, name: &str| {
        states(page, KnownRole::TextBox, name).map(|states| states.invalid)
    };
    for name in ["Name", "Email", "Message"] {
        assert_eq!(invalid(&mut page, name), Some(None), "{name} as it loads");
    }

    let put = act(&mut page, "Email", Verb::PutText("nope".to_owned()));
    assert!(put.is_some(), "the address was put in");
    assert_eq!(invalid(&mut page, "Email"), Some(Some(Invalid::Yes)));

    // Chrome refuses the press on the empty name, and still does not read
    // the name as invalid.
    let pressed = act(&mut page, "Send", Verb::Activate);
    assert!(
        matches!(pressed, Some(Outcome::NotSubmitted { .. })),
        "{pressed:?}"
    );
    assert_eq!(invalid(&mut page, "Name"), Some(None));
    assert_eq!(invalid(&mut page, "Message"), Some(None));
    assert_eq!(invalid(&mut page, "Email"), Some(Some(Invalid::Yes)));
}

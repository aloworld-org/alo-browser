/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Agenda's calendar list, in numbers: the page that opened queue item
//! 440.
//!
//! `cases/alo-agenda-calendars` pins the whole page as files. This says out
//! loud what item 440 is closed by. Each calendar row is a `.calItem` whose
//! toggle (`.calToggle`) takes the room its buttons and badge leave, by
//! `flex: 1`, and whose name (`.calName`) takes the room the toggle's box
//! leaves, by `flex: 1` again. Until item 440 the `flex` shorthand was read
//! by nothing, so each toggle was as wide as its name and the row's share
//! and delete buttons stood against it rather than at the row's end. Chrome
//! 154 gives every number below for the same frozen page, given the
//! corpus's font, to within its 1/64 px layout unit.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_value::Rgba;

/// The case, rendered.
fn agenda() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("alo-agenda-calendars"))?;
    Rendering::of(&case).ok()
}

/// The border boxes of every element that `wanted` picks, in document order.
fn all(rendering: &Rendering, wanted: impl Fn(&alo_dom::Element) -> bool) -> Vec<Rect> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| document.element(*id).is_some_and(&wanted))
        .filter_map(|node| {
            let id = boxes.ids().find(|id| {
                matches!(
                    boxes.get(*id).map(|box_node| &box_node.kind),
                    Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
                )
            })?;
            Some(drawing.layout.get(id)?.border_box)
        })
        .collect()
}

/// The border boxes of every element whose `class` names `class`.
fn of_class(rendering: &Rendering, class: &str) -> Vec<Rect> {
    all(rendering, |element| {
        element
            .attr("class")
            .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
    })
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

/// Where a box ends on the right.
fn right(rect: &Rect) -> f32 {
    rect.origin.x + rect.size.width
}

#[test]
fn it_records_only_the_resets_pseudo_elements() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    assert_eq!(issues.len(), 2, "{issues:#?}");
    assert!(
        issues
            .iter()
            .all(|issue| issue.contains("::before") || issue.contains("::after")),
        "{issues:#?}"
    );
}

#[test]
fn each_toggle_takes_the_room_its_rows_buttons_leave() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let toggles = of_class(&rendering, "calToggle");
    let names = of_class(&rendering, "calName");
    let buttons = of_class(&rendering, "calDel");
    let (
        [personal, work, _, _],
        [personal_name, work_name, _, _],
        [personal_share, work_share, work_delete],
    ) = (toggles.as_slice(), names.as_slice(), buttons.as_slice())
    else {
        panic!("four toggles, four names, three buttons: {toggles:?} {names:?} {buttons:?}");
    };
    // The row is the sidebar's 207, less 4 px of padding each side; the one
    // 13 px button is at its end, 8 px after the toggle. Chrome: 178 and 206.
    assert!(near(personal.origin.x, 20.0), "{personal:?}");
    assert!(near(personal.size.width, 178.0), "{personal:?}");
    assert!(near(personal_share.origin.x, 206.0), "{personal_share:?}");
    assert!(near(right(personal_share), 219.0), "{personal_share:?}");
    // Two buttons: 157, then the share at 185 and the delete at 206.
    assert!(near(work.size.width, 157.0), "{work:?}");
    assert!(near(work_share.origin.x, 185.0), "{work_share:?}");
    assert!(near(work_delete.origin.x, 206.0), "{work_delete:?}");
    // Inside each toggle the name takes what the 16 px box and 8 px gap
    // leave: 154 and 133.
    assert!(near(personal_name.origin.x, 44.0), "{personal_name:?}");
    assert!(near(personal_name.size.width, 154.0), "{personal_name:?}");
    assert!(near(work_name.size.width, 133.0), "{work_name:?}");
}

#[test]
fn a_shared_calendars_badge_stands_at_its_rows_end() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let toggles = of_class(&rendering, "calToggle");
    let names = of_class(&rendering, "calName");
    let badges = of_class(&rendering, "calShared");
    let ([_, _, holidays, mira], [_, _, _, mira_name], [can_edit, can_view]) =
        (toggles.as_slice(), names.as_slice(), badges.as_slice())
    else {
        panic!("four toggles, four names, two badges: {toggles:?} {names:?} {badges:?}");
    };
    // `flex-shrink: 0` keeps each badge whole at the row's end; the toggle
    // takes the rest, less the 8 px gap.
    for (toggle, badge) in [(holidays, can_edit), (mira, can_view)] {
        assert!(near(right(badge), 219.0), "{badge:?}");
        assert!(
            near(right(toggle) + 8.0, badge.origin.x),
            "{toggle:?} {badge:?}"
        );
    }
    // Mira's name is longer than the room it has, and does not widen the
    // toggle: `flex: 1` makes its basis 0% and `min-width: 0` lets the
    // toggle shrink below what it holds.
    assert!(
        near(mira_name.size.width, mira.size.width - 24.0),
        "{mira_name:?}"
    );
}

#[test]
fn a_shown_calendar_is_filled_with_its_colour_and_a_hidden_one_is_not() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let checks = of_class(&rendering, "calCheck");
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    let [personal, work, holidays, _] = checks.as_slice() else {
        panic!("four boxes: {checks:?}");
    };
    let pixel = |x: f32, y: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a pixel on the canvas"
        )]
        let (x, y) = (x as u32, y as u32);
        drawing
            .canvas
            .at(x, y)
            .map_or_else(|| panic!("({x}, {y}) is on the canvas"), Rgba::to_rgba8)
    };
    // Each box's colour comes from its own `style="--cal: …"`. Just inside
    // the top-left of the box, clear of the tick.
    assert_eq!(
        pixel(personal.origin.x + 3.0, personal.origin.y + 3.0),
        (231, 111, 81, 255)
    );
    assert_eq!(
        pixel(work.origin.x + 3.0, work.origin.y + 3.0),
        (75, 131, 196, 255)
    );
    // The hidden calendar's box is its green ring and the sidebar's
    // `--bg-sunken` inside it.
    assert_eq!(
        pixel(holidays.origin.x + 0.5, holidays.origin.y + 8.0),
        (46, 139, 87, 255)
    );
    assert_eq!(
        pixel(holidays.origin.x + 8.0, holidays.origin.y + 8.0),
        (240, 236, 230, 255)
    );
}

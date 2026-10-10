/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' contact form, in numbers: the page that opened queue item 397.
//!
//! `cases/alo-sites-contact-form` pins the whole page as files. This says
//! out loud what item 397 is closed by. The section is a grid of two
//! columns, `.8fr 1.2fr`, with `gap: clamp(2rem, 6vw, 5rem)`, and until item
//! 397 layout split the `gap` shorthand at every space: `clamp(2rem,`,
//! `6vw,` and `5rem)` each failed to read, were dropped without a record,
//! and the gap was 0. The columns were 304 and 456, touching.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn contact_form() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-contact-form"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border boxes of every element named `tag`, in document order.
fn all(rendering: &Rendering, tag: &str) -> Vec<Rect> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| {
            document
                .element(*id)
                .is_some_and(|element| &*element.name.local == tag)
        })
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

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = contact_form() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(
        case.address.as_deref(),
        Some("https://nordwind.alosites.com/")
    );
    assert_eq!(answered.sheets, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
    assert!(answered.said.is_empty(), "{:?}", answered.said);
    assert!(answered.issues.is_empty(), "{:?}", answered.issues);
}

#[test]
fn what_it_records_is_the_sheets_list_and_nothing_of_its_gap() {
    let Some((_, rendering)) = contact_form() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    // The other alo Sites cases' list exactly — pseudo-elements, `@keyframes`
    // and `prefers-reduced-motion`. The `gap` is read, so it is not refused.
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")),
        "{issues:#?}"
    );
}

#[test]
fn its_two_columns_stand_a_clamped_gap_apart() {
    let Some((_, rendering)) = contact_form() else {
        panic!("the case renders");
    };
    let (sections, headings, forms) = (
        all(&rendering, "section"),
        all(&rendering, "h2"),
        all(&rendering, "form"),
    );
    let ([section], [heading], [form]) =
        (sections.as_slice(), headings.as_slice(), forms.as_slice())
    else {
        panic!("a section, a heading and a form: {sections:?} {headings:?} {forms:?}");
    };
    // `main > section` is 70rem at most and the page is 800, so the section
    // is 800 with 1.25rem of padding each side: 760 of content at x 20.
    assert!(near(section.size.width, 800.0), "{section:?}");
    // The gap is `clamp(2rem, 6vw, 5rem)`: 6vw of 800 is 48, between 32 and
    // 80. The 712 left is shared .8 to 1.2: 284.8 and 427.2.
    assert!(near(heading.origin.x, 20.0), "{heading:?}");
    assert!(near(heading.size.width, 284.8), "{heading:?}");
    assert!(near(form.origin.x, 20.0 + 284.8 + 48.0), "{form:?}");
    assert!(near(form.size.width, 427.2), "{form:?}");
    assert!(
        near(
            form.origin.x - (heading.origin.x + heading.size.width),
            48.0
        ),
        "{heading:?} {form:?}"
    );
}

#[test]
fn its_rows_are_the_same_gap_apart() {
    let Some((_, rendering)) = contact_form() else {
        panic!("the case renders");
    };
    let (headings, paragraphs, forms) = (
        all(&rendering, "h2"),
        all(&rendering, "p"),
        all(&rendering, "form"),
    );
    let ([heading], [intro, ..], [form]) =
        (headings.as_slice(), paragraphs.as_slice(), forms.as_slice())
    else {
        panic!("a heading, paragraphs and a form: {headings:?} {paragraphs:?} {forms:?}");
    };
    // One value is both gaps. The heading is two lines of 28 × 1.2 = 67.2,
    // with 0.5em (14) of margin under it, so its row is 81.2; the next row,
    // the introduction, starts 48 below that, at 48 + 81.2 + 48.
    assert!(near(heading.origin.y, 48.0), "{heading:?}");
    assert!(near(heading.size.height, 67.2), "{heading:?}");
    assert!(near(intro.origin.y, 177.2), "{intro:?}");
    // The form is placed in rows one to three and starts at the top of the
    // first, `align-items: start`.
    assert!(near(form.origin.y, 48.0), "{form:?}");
}

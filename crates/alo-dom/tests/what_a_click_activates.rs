/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 256, ADR 0018 § 6: what a click does to the element it
//! activates — before its listeners, when one cancels it, and after.

use alo_dom::activation::{Activation, Follows, activation_target, after, before, cancelled};
use alo_dom::{Document, NodeId, parse_document};

/// The element whose `id` attribute is `id`.
fn by_id(document: &Document, id: &str) -> Option<NodeId> {
    document.descendants(document.root()).find(|node| {
        document
            .element(*node)
            .is_some_and(|element| element.attr("id") == Some(id))
    })
}

fn checked(document: &Document, id: &str) -> bool {
    by_id(document, id)
        .and_then(|node| document.element(node))
        .is_some_and(|element| element.attr("checked").is_some())
}

#[test]
fn a_checkbox_turns_over_before_the_listeners_and_back_if_one_cancels() {
    let mut document = parse_document(r"<input type=checkbox id=box>");
    let node = by_id(&document, "box").expect("an element with that id");

    let done = before(&mut document, node);
    assert_eq!(done, Activation::Checkbox { node, was: false });
    assert!(checked(&document, "box"), "ticked before any listener runs");

    cancelled(&mut document, &done);
    assert!(!checked(&document, "box"), "and unticked when one cancels");

    let done = before(&mut document, node);
    assert!(checked(&document, "box"));
    assert_eq!(after(&document, &done), Follows::InputAndChange(node));
    assert!(checked(&document, "box"), "nobody cancelled: it stays");

    // And the other way, from ticked.
    let done = before(&mut document, node);
    assert_eq!(done, Activation::Checkbox { node, was: true });
    assert!(!checked(&document, "box"));
    cancelled(&mut document, &done);
    assert!(checked(&document, "box"));
}

#[test]
fn the_type_is_matched_without_regard_to_case() {
    let mut document = parse_document(r"<input type=CheckBox id=box>");
    let node = by_id(&document, "box").expect("an element with that id");
    assert!(matches!(
        before(&mut document, node),
        Activation::Checkbox { .. }
    ));
}

#[test]
fn a_radio_is_chosen_its_group_cleared_and_a_cancel_puts_the_old_choice_back() {
    let mut document = parse_document(
        r"<input type=radio name=size id=s checked>
           <input type=radio name=size id=m>
           <input type=radio name=other id=o checked>",
    );
    let m = by_id(&document, "m").expect("an element with that id");
    let s = by_id(&document, "s").expect("an element with that id");

    let done = before(&mut document, m);
    assert_eq!(
        done,
        Activation::Radio {
            node: m,
            was: false,
            unchecked: vec![s],
        }
    );
    assert!(checked(&document, "m") && !checked(&document, "s"));
    assert!(checked(&document, "o"), "another group is not touched");

    cancelled(&mut document, &done);
    assert!(checked(&document, "s") && !checked(&document, "m"));
    assert!(checked(&document, "o"));

    let done = before(&mut document, m);
    assert_eq!(after(&document, &done), Follows::InputAndChange(m));
}

#[test]
fn a_cancelled_radio_whose_old_choice_left_the_group_is_unchecked() {
    let mut document = parse_document(
        r"<input type=radio name=size id=s checked><input type=radio name=size id=m>",
    );
    let m = by_id(&document, "m").expect("an element with that id");
    let s = by_id(&document, "s").expect("an element with that id");
    let done = before(&mut document, m);
    // A listener renames the old choice out of the group.
    document.set_attribute(s, "name", "elsewhere");
    cancelled(&mut document, &done);
    assert!(!checked(&document, "m"), "no previous choice to go back to");
    assert!(
        !checked(&document, "s"),
        "and the one that left is not checked"
    );
}

#[test]
fn a_radio_without_a_name_is_a_group_of_one() {
    let mut document = parse_document(r"<input type=radio id=a checked><input type=radio id=b>");
    let b = by_id(&document, "b").expect("an element with that id");
    let done = before(&mut document, b);
    assert!(
        checked(&document, "a"),
        "no name, no group: nothing cleared"
    );
    assert!(checked(&document, "b"));
    cancelled(&mut document, &done);
    assert!(!checked(&document, "b"));
}

#[test]
fn a_checked_radio_clicked_again_stays_checked_even_when_cancelled() {
    let mut document = parse_document(r"<input type=radio name=n id=a checked>");
    let a = by_id(&document, "a").expect("an element with that id");
    let count = document.change_count();
    let done = before(&mut document, a);
    assert_eq!(document.change_count(), count, "nothing changed");
    cancelled(&mut document, &done);
    assert!(checked(&document, "a"));
}

#[test]
fn a_link_is_followed_after_and_changes_nothing_before() {
    let mut document = parse_document(r#"<p><a href="/inbox" id=l><span id=s>Inbox</span></a>"#);
    let span = by_id(&document, "s").expect("an element with that id");
    let link = by_id(&document, "l").expect("an element with that id");
    let count = document.change_count();

    let done = before(&mut document, span);
    assert_eq!(done, Activation::Link { node: link }, "a click bubbles");
    assert_eq!(document.change_count(), count);
    assert_eq!(
        after(&document, &done),
        Follows::Link {
            node: link,
            href: "/inbox".to_owned(),
        }
    );
}

#[test]
fn a_link_whose_href_a_listener_took_goes_nowhere() {
    let mut document = parse_document(r#"<a href="/inbox" id=l>Inbox</a>"#);
    let link = by_id(&document, "l").expect("an element with that id");
    let done = before(&mut document, link);
    document.remove_attribute(link, "href");
    assert_eq!(after(&document, &done), Follows::Nothing);
}

#[test]
fn an_anchor_without_an_href_is_no_link_and_the_search_goes_on_up() {
    let mut document = parse_document(r#"<a href="/x" id=out><a id=in>no</a></a>"#);
    // The parser closes the first `a` before the second, so the second is
    // not inside it: there is nothing to activate.
    let inner = by_id(&document, "in").expect("an element with that id");
    assert_eq!(activation_target(&document, inner), None);
    assert_eq!(before(&mut document, inner), Activation::None);
}

#[test]
fn a_button_inside_a_link_is_what_a_click_inside_it_activates() {
    let mut document = parse_document(
        r#"<a href="/x"><button type=button id=b><span id=s>Go</span></button></a>"#,
    );
    let span = by_id(&document, "s").expect("an element with that id");
    let button = by_id(&document, "b").expect("an element with that id");
    let done = before(&mut document, span);
    assert_eq!(done, Activation::Button { node: button });
    assert_eq!(
        after(&document, &done),
        Follows::Nothing,
        "forms are item 82"
    );
}

#[test]
fn a_box_a_listener_took_out_of_the_page_changes_and_nobody_is_told() {
    let mut document = parse_document(r"<div><input type=checkbox id=box></div>");
    let node = by_id(&document, "box").expect("an element with that id");
    let done = before(&mut document, node);
    assert!(document.remove(node));
    assert_eq!(after(&document, &done), Follows::Nothing);
    assert!(
        document
            .element(node)
            .is_some_and(|element| element.attr("checked").is_some()),
        "it was still ticked"
    );
}

#[test]
fn a_plain_element_has_nothing_to_activate() {
    let mut document = parse_document(r"<div id=d>text</div><input type=text id=t>");
    for id in ["d", "t"] {
        let node = by_id(&document, id).expect("an element with that id");
        let count = document.change_count();
        let done = before(&mut document, node);
        assert_eq!(done, Activation::None);
        cancelled(&mut document, &done);
        assert_eq!(after(&document, &done), Follows::Nothing);
        assert_eq!(document.change_count(), count);
    }
}

#[test]
fn an_id_from_nowhere_activates_nothing() {
    let mut document = parse_document(r"<input type=checkbox>");
    let mut scratch = Document::new();
    let mut stranger = scratch.root();
    for _ in 0..50 {
        stranger = scratch.create_text_node("");
    }
    assert_eq!(before(&mut document, stranger), Activation::None);
}

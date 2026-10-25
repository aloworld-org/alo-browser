/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<select>` is chosen from, and takes no text (ADR 0064 § 4, queue item
//! 648). `PutText` on one was answered `TextPut` and wrote a `value`
//! attribute nothing reads, so the form sent what it sent before while the
//! agent was told its text went in. It is refused by name now, and the
//! document is left as it was.

use alo_agent::{AgentTree, Outcome, Refusal, Target, Verb, apply, perform};
use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::submission::{Entry, Value, entries};
use alo_dom::{Document, NodeId, parse_document};
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{FontDatabase, TextMeasurer};

/// Ask `verb` of whatever is called `name`, and carry it into the document
/// when it was not refused.
fn act(document: &mut Document, name: &str, verb: &Verb) -> Result<Outcome, Refusal> {
    let (outcome, boxes) = {
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
        let styles = resolve(document, &sheets, &MediaContext::default());
        let boxes = build_boxes(document, &styles);
        let database = FontDatabase::new();
        let measurer = TextMeasurer::new(&database);
        let layout = compute(&boxes, &styles, Size::new(400.0, 200.0), &measurer);
        let tree = AgentTree::new(document, &boxes, &layout);
        let outcome = perform(&tree, &Target::Named(name.to_owned()), verb)?;
        (outcome, boxes)
    };
    apply(document, &boxes, &outcome);
    Ok(outcome)
}

/// The first `<form>`.
fn form(document: &Document) -> Option<NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html("form"))
    })
}

/// What the form would send, as `name=value` pairs.
fn sent(document: &Document) -> Vec<String> {
    let Some(form) = form(document) else {
        return vec!["no form".to_owned()];
    };
    match entries(document, form, None) {
        Ok(entries) => entries
            .iter()
            .map(|Entry { name, value }| match value {
                Value::Text(text) => format!("{name}={text}"),
                Value::NoFile => format!("{name}=(no file)"),
            })
            .collect(),
        Err(why) => vec![format!("unsubmittable: {why:?}")],
    }
}

const FORM: &str = "<form>\
    <label>Size <select name=size><option>Small<option selected>Medium<option>Large</select></label>\
    <label>Toppings <select name=toppings multiple><option selected>Bacon<option>Onion</select></label>\
    <label>Crust <select name=crust disabled><option>Thin</select></label>\
    </form>";

#[test]
fn text_on_a_list_is_refused_by_name_and_the_form_sends_what_it_sent() {
    let mut document = parse_document(FORM);
    let before = document.change_count();
    let was = sent(&document);
    assert_eq!(was, ["size=Medium", "toppings=Bacon"]);
    for name in ["Size", "Toppings", "Crust"] {
        let refused = act(&mut document, name, &Verb::PutText("Large".to_owned()));
        assert!(
            matches!(refused, Err(Refusal::Chooses { .. })),
            "{name}: {refused:?}"
        );
    }
    assert_eq!(document.change_count(), before, "nothing was written");
    assert_eq!(sent(&document), was);
}

#[test]
fn a_combobox_an_author_types_into_still_takes_text() {
    let mut document =
        parse_document("<form><label>City <input name=city role=combobox></label></form>");
    let put = act(&mut document, "City", &Verb::PutText("Kampala".to_owned()));
    assert!(
        matches!(&put, Ok(Outcome::TextPut { text, .. }) if text == "Kampala"),
        "{put:?}"
    );
    assert_eq!(sent(&document), ["city=Kampala"]);
}

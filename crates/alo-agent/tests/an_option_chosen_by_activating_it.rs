/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An option is chosen by activating it, on a page that never ran script
//! (ADR 0064 §§ 1, 2, 3 and 5; queue item 659). An option is
//! `display: none` and generated no box, so nothing in the tree could be
//! activated to choose one. A list's options are nodes of the agent's tree
//! under it now, with no rectangle; activating one chooses it — or toggles
//! it in a list of several — and the answer says whether it is chosen,
//! read from the page afterwards. What the form sends is the proof.

use alo_agent::{AgentTree, Change, Outcome, Refusal, Target, Verb, apply, chosen_after, perform};
use alo_box::{BoxId, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::submission::{Entry, Value, entries};
use alo_dom::{Document, parse_document};
use alo_layout::{LayoutTree, Rect, Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{FontDatabase, TextMeasurer};

/// Lay `document` out and hand its tree to `read`.
fn laid_out<T>(document: &Document, read: impl FnOnce(&AgentTree<'_>, &LayoutTree) -> T) -> T {
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
    let styles = resolve(document, &sheets, &MediaContext::default());
    let boxes = build_boxes(document, &styles);
    let database = FontDatabase::new();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(400.0, 200.0), &measurer);
    let tree = AgentTree::new(document, &boxes, &layout);
    read(&tree, &layout)
}

/// Ask `verb` of `target`, carry it into the document when it was not
/// refused, and answer as the page holds it afterwards.
fn act(
    document: &mut Document,
    target: &Target,
    verb: &Verb,
) -> (Result<Outcome, Refusal>, Vec<Change>) {
    let (outcome, boxes) = {
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
        let styles = resolve(document, &sheets, &MediaContext::default());
        let boxes = build_boxes(document, &styles);
        let database = FontDatabase::new();
        let measurer = TextMeasurer::new(&database);
        let layout = compute(&boxes, &styles, Size::new(400.0, 200.0), &measurer);
        let tree = AgentTree::new(document, &boxes, &layout);
        (perform(&tree, target, verb), boxes)
    };
    match outcome {
        Ok(outcome) => {
            let changes = apply(document, &boxes, &outcome);
            (Ok(chosen_after(document, &boxes, outcome)), changes)
        }
        Err(refusal) => (Err(refusal), Vec::new()),
    }
}

fn choose(document: &mut Document, name: &str) -> Result<Outcome, Refusal> {
    act(document, &Target::Named(name.to_owned()), &Verb::Activate).0
}

/// What the first form would send, as `name=value` pairs.
fn sent(document: &Document) -> Vec<String> {
    let Some(form) = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html("form"))
    }) else {
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
    </form>";

#[test]
fn a_lists_options_are_read_under_it_and_are_not_drawn() {
    let document = parse_document(FORM);
    let outline = laid_out(&document, |tree, _| tree.to_outline());
    let lines: Vec<&str> = outline
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("option") || line.starts_with("combobox"))
        .collect();
    assert_eq!(
        lines,
        [
            "combobox \"Size\" at (8, 17) 139.32813×4",
            "option \"Small\" [selected=false] not drawn",
            "option \"Medium\" [selected=true] not drawn",
            "option \"Large\" [selected=false] not drawn",
            "option \"Bacon\" [selected=true] not drawn",
            "option \"Onion\" [selected=false] not drawn",
        ],
        "{outline}",
    );
}

/// The layout assertion: a closed list's options have no box — nothing
/// laid out, no rectangle — and holding them moves nothing, the list laid
/// out to the same numbers as one with no options at all.
#[test]
fn a_closed_lists_options_have_no_box_and_move_nothing() {
    let rects = |html: &str| {
        let document = parse_document(html);
        laid_out(&document, |tree, layout| {
            let nodes = tree.nodes();
            let list = nodes
                .iter()
                .find(|node| node.role().to_string() == "combobox")
                .map(|node| (node.id(), node.rect()));
            let options: Vec<(BoxId, usize, Option<Rect>)> = nodes
                .iter()
                .filter(|node| node.role().to_string() == "option")
                .map(|node| (node.id(), node.rects().len(), layout.border_box(node.id())))
                .collect();
            (list, options)
        })
    };
    let (held, options) =
        rects("<p>a<select><option>One<optgroup label=G><option>Two</optgroup></select>b</p>");
    let (empty, none) = rects("<p>a<select></select>b</p>");
    assert!(none.is_empty());
    assert_eq!(options.len(), 2);
    for (option, pieces, laid_out) in options {
        assert_eq!((pieces, laid_out), (0, None), "{option} has no box");
    }
    let (Some((_, held)), Some((_, empty))) = (held, empty) else {
        panic!("both pages hold a list");
    };
    assert_eq!(held, empty);
    // The numbers an empty list is laid out to, measured with no fonts:
    // the options add none of their own.
    assert_eq!(
        (held.left(), held.top(), held.size.width, held.size.height),
        (8.0, 25.0, 139.328_13, 4.0),
    );
}

#[test]
fn an_option_activated_in_a_list_of_one_choice_is_the_one_its_form_sends() {
    let mut document = parse_document(FORM);
    assert_eq!(sent(&document), ["size=Medium", "toppings=Bacon"]);
    let large = choose(&mut document, "Large");
    let Ok(Outcome::Chosen {
        node,
        list,
        chosen: true,
    }) = large
    else {
        panic!("{large:?}");
    };
    assert_eq!(
        large.as_ref().map(ToString::to_string),
        Ok(format!("chose {node} in {list}"))
    );
    assert_eq!(sent(&document), ["size=Large", "toppings=Bacon"]);
    // And it reads as chosen, the one it was not.
    let states = laid_out(&document, |tree, _| {
        ["Medium", "Large"].map(|name| {
            tree.named(name)
                .first()
                .and_then(|node| node.states().selected)
        })
    });
    assert_eq!(states, [Some(false), Some(true)]);
    // Choosing what it holds is still chosen, and sends the same.
    assert!(matches!(
        choose(&mut document, "Large"),
        Ok(Outcome::Chosen { chosen: true, .. })
    ));
    assert_eq!(sent(&document), ["size=Large", "toppings=Bacon"]);
}

#[test]
fn an_option_activated_in_a_list_of_several_is_added_and_taken_away() {
    let mut document = parse_document(FORM);
    let (onion, changes) = act(
        &mut document,
        &Target::Named("Onion".to_owned()),
        &Verb::Activate,
    );
    assert!(
        matches!(onion, Ok(Outcome::Chosen { chosen: true, .. })),
        "{onion:?}"
    );
    assert!(
        matches!(changes.as_slice(), [Change::Set { attribute, .. }] if attribute == "selected"),
        "{changes:?}"
    );
    assert_eq!(
        sent(&document),
        ["size=Medium", "toppings=Bacon", "toppings=Onion"]
    );
    let bacon = choose(&mut document, "Bacon");
    let Ok(Outcome::Chosen {
        node,
        list,
        chosen: false,
    }) = bacon
    else {
        panic!("{bacon:?}");
    };
    assert_eq!(
        bacon.as_ref().map(ToString::to_string),
        Ok(format!("activated {node}, and it is not chosen in {list}"))
    );
    assert_eq!(sent(&document), ["size=Medium", "toppings=Onion"]);
}

#[test]
fn a_disabled_option_its_groups_or_its_lists_is_refused_and_nothing_changes() {
    let mut document = parse_document(
        "<form><select name=a><option>Open<option disabled>Shut</select>\
         <select name=b><optgroup label=Gone disabled><option>Grouped</optgroup>\
         <option>Free</select>\
         <select name=c disabled><option>Listed</select></form>",
    );
    let was = sent(&document);
    let before = document.change_count();
    for name in ["Shut", "Grouped", "Listed"] {
        let refused = choose(&mut document, name);
        assert!(
            matches!(refused, Err(Refusal::Disabled { .. })),
            "{name}: {refused:?}"
        );
    }
    assert_eq!(document.change_count(), before);
    assert_eq!(sent(&document), was);
}

#[test]
fn two_options_called_the_same_are_asked_for_by_id() {
    let mut document = parse_document(
        "<form><select name=delivery><option>No<option>Yes</select>\
         <select name=gift><option>No<option>Yes</select></form>",
    );
    let ambiguous = choose(&mut document, "Yes");
    let Err(Refusal::Ambiguous { candidates, .. }) = ambiguous else {
        panic!("{ambiguous:?}");
    };
    let Some(second) = candidates.get(1) else {
        panic!("two candidates: {candidates:?}");
    };
    let (chosen, _) = act(&mut document, &Target::Node(*second), &Verb::Activate);
    assert!(
        matches!(chosen, Ok(Outcome::Chosen { chosen: true, .. })),
        "{chosen:?}"
    );
    assert_eq!(sent(&document), ["delivery=No", "gift=Yes"]);
}

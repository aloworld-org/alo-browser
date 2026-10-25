/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 659: an option chosen by activating it, on a page that runs
//! script and on one that never did (ADR 0064 §§ 2 and 3).
//!
//! On a page that runs script, `Activate` on a list's option is one task
//! run before the agent is answered: the option chosen — or toggled in a
//! list of several — then a trusted `input` and a `change` at the list,
//! only if that changed what it holds. The answer says whether the option
//! is chosen **after** the task, read from the page, so a `change` listener
//! that puts the old choice back is answered as not chosen. On a page that
//! never ran script `apply` chooses it and nobody hears anything.
//!
//! Every page whose answer a collection could change is acted on twice —
//! once ordinarily and once with the collector at every allocation — and the
//! two must say the same.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_dom::submission::{Entry, Value, entries};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("monospace", "DejaVu Sans");
    database
}

/// `body` loaded, its scripts run, with the collector at every allocation
/// from then on if `stress`.
fn loaded(body: &str, stress: bool) -> Result<Renderer, String> {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        format!("<!doctype html><body>{body}</body>"),
        Size::new(400.0, 300.0),
    ))));
    if !matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()) {
        return Err(format!("the page loads and says nothing: {answer:?}"));
    }
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    Ok(renderer)
}

/// Activate `target`, answering what it did and said, or the refusal, or
/// what the renderer answered instead.
fn activate(
    renderer: &mut Renderer,
    target: Target,
) -> Result<Result<(Outcome, Vec<String>), Refusal>, String> {
    match renderer.handle(ToRenderer::Act {
        target,
        verb: Verb::Activate,
    }) {
        FromRenderer::Acted {
            outcome, issues, ..
        } => Ok(Ok((outcome, issues))),
        FromRenderer::Refused(refusal) => Ok(Err(refusal)),
        other => Err(format!("{other:?}")),
    }
}

fn choose(renderer: &mut Renderer, name: &str) -> Result<(Outcome, Vec<String>), String> {
    match activate(renderer, Target::Named(name.to_owned()))? {
        Ok(done) => Ok(done),
        Err(refusal) => Err(format!("{name} was refused: {refusal}")),
    }
}

/// The `which`th element named `local`, in tree order.
fn nth(document: &Document, local: &str, which: usize) -> Option<NodeId> {
    document
        .descendants(document.root())
        .filter(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.name.is_html(local))
        })
        .nth(which)
}

/// The text of the page's first `<p>`, where its script writes what it
/// heard.
fn heard(renderer: &Renderer) -> String {
    renderer
        .document()
        .and_then(|document| Some(document.text_content(nth(document, "p", 0)?)))
        .unwrap_or_default()
}

/// What the page's first form would send, as `name=value` pairs.
fn sent(renderer: &Renderer) -> Vec<String> {
    let Some(document) = renderer.document() else {
        return vec!["no page".to_owned()];
    };
    let Some(form) = nth(document, "form", 0) else {
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

/// A closed list and a list of several, and a listener on the body writing
/// each `input` and `change` it hears into the `<p>`: its type, the list's
/// name and whether the browser made it.
const ORDER: &str = "<p>-</p><form>\
    <select name=size aria-label=Size><option>Small<option selected>Medium<option>Large\
    <option disabled>Huge</select>\
    <select name=toppings aria-label=Toppings multiple><option selected>Bacon<option>Onion</select>\
    </form><script>\
    const out = document.body.firstChild;\
    function say(event) {\
      out.textContent = out.textContent + ' ' + event.type + ':' +\
        event.target.getAttribute('name') + ':' + event.isTrusted;\
    }\
    document.body.addEventListener('input', say);\
    document.body.addEventListener('change', say);\
    </script>";

#[test]
fn an_option_chosen_is_the_one_its_form_sends_and_the_page_hears_input_then_change() {
    for stress in [false, true] {
        let mut renderer = loaded(ORDER, stress).expect("the page loads");
        assert_eq!(sent(&renderer), ["size=Medium", "toppings=Bacon"]);
        let (outcome, issues) = choose(&mut renderer, "Large").expect("an option");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(
            matches!(outcome, Outcome::Chosen { chosen: true, .. }),
            "{outcome:?}"
        );
        assert_eq!(heard(&renderer), "- input:size:true change:size:true");
        assert_eq!(sent(&renderer), ["size=Large", "toppings=Bacon"]);

        // What the list already holds changes nothing, and is not heard.
        let (again, _) = choose(&mut renderer, "Large").expect("an option");
        assert!(
            matches!(again, Outcome::Chosen { chosen: true, .. }),
            "{again:?}"
        );
        assert_eq!(heard(&renderer), "- input:size:true change:size:true");
    }
}

#[test]
fn an_option_of_a_list_of_several_is_added_and_taken_away_and_each_is_heard() {
    for stress in [false, true] {
        let mut renderer = loaded(ORDER, stress).expect("the page loads");
        let (onion, _) = choose(&mut renderer, "Onion").expect("an option");
        assert!(
            matches!(onion, Outcome::Chosen { chosen: true, .. }),
            "{onion:?}"
        );
        assert_eq!(
            sent(&renderer),
            ["size=Medium", "toppings=Bacon", "toppings=Onion"]
        );
        let (bacon, _) = choose(&mut renderer, "Bacon").expect("an option");
        assert!(
            matches!(bacon, Outcome::Chosen { chosen: false, .. }),
            "{bacon:?}"
        );
        assert_eq!(sent(&renderer), ["size=Medium", "toppings=Onion"]);
        assert_eq!(
            heard(&renderer),
            "- input:toppings:true change:toppings:true input:toppings:true \
             change:toppings:true"
        );
    }
}

#[test]
fn a_change_listener_that_puts_the_choice_back_is_answered_as_not_chosen() {
    let page = "<p>-</p><form><select name=locked aria-label=Locked multiple>\
                <option>Only</select></form><script>\
                const list = document.body.firstChild.nextSibling.firstChild;\
                list.addEventListener('change', () => {\
                  list.firstChild.removeAttribute('selected');\
                });\
                </script>";
    for stress in [false, true] {
        let mut renderer = loaded(page, stress).expect("the page loads");
        let (outcome, _) = choose(&mut renderer, "Only").expect("an option");
        let Outcome::Chosen {
            node,
            list,
            chosen: false,
        } = outcome
        else {
            panic!("the page undid it: {outcome:?}");
        };
        assert_eq!(
            outcome.to_string(),
            format!("activated {node}, and it is not chosen in {list}")
        );
        assert_eq!(sent(&renderer), Vec::<String>::new());
    }
}

#[test]
fn a_disabled_option_is_refused_and_the_page_hears_nothing() {
    let mut renderer = loaded(ORDER, false).expect("the page loads");
    let refused = activate(&mut renderer, Target::Named("Huge".to_owned())).expect("an answer");
    assert!(
        matches!(refused, Err(Refusal::Disabled { .. })),
        "{refused:?}"
    );
    assert_eq!(heard(&renderer), "-");
    assert_eq!(sent(&renderer), ["size=Medium", "toppings=Bacon"]);
}

#[test]
fn on_a_page_that_never_ran_script_the_option_is_chosen_all_the_same() {
    let page = "<p>-</p><form><select name=size aria-label=Size><option>Small\
                <option>Large</select></form>";
    let mut renderer = loaded(page, false).expect("the page loads");
    assert!(renderer.event_loop().is_none(), "no script, no heap");
    assert_eq!(sent(&renderer), ["size=Small"]);
    let (outcome, issues) = choose(&mut renderer, "Large").expect("an option");
    assert!(issues.is_empty(), "{issues:?}");
    assert!(
        matches!(outcome, Outcome::Chosen { chosen: true, .. }),
        "{outcome:?}"
    );
    assert_eq!(sent(&renderer), ["size=Large"]);
    // The tree it is read from next says so.
    let FromRenderer::Tree(snapshot) = renderer.handle(ToRenderer::ReadTree) else {
        panic!("a tree");
    };
    let outline = snapshot.to_outline();
    assert!(
        outline.contains("option \"Large\" [selected=true] not drawn"),
        "{outline}"
    );
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An option chosen through a list Chrome draws: queue item 660, ADR 0064
//! § 6.
//!
//! Against the machine's Chrome, on a page of this test's own: an option of
//! a list with a `size` chosen by `Activate` is the one the form sends, and
//! the page hears one `input` and one `change`; two options of a `multiple`
//! list are both sent after two verbs and one is taken away by a third,
//! the others left as they were; a page's `change` listener that puts the
//! old choice back is answered as not chosen; and an option of a closed
//! list is not pressed at all, the page hearing nothing.

use alo_agent::verb::{Target, Verb};
use alo_agent::{Outcome, Refusal};
use alo_box::BoxId;
use alo_dom::submission::Method;
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_tab::Tab;

/// A page of this test's own, served at `https://example.test/lists`: a
/// form of three lists Chrome draws and one it does not, a list whose
/// `change` listener puts its old choice back, a list a page builds of its
/// own, and a line that writes down what each list hears and whether it was
/// a person's (`isTrusted`).
const LISTS: &str = r#"<!doctype html>
<html lang=en><title>Lists</title>
<p id=heard>Heard:</p>
<form action="/send">
<select aria-label="Size" name=size size=3><option>Small</option><option selected>Medium</option><option>Large</option></select>
<select aria-label="Toppings" name=toppings multiple><option selected>Bacon</option><optgroup label="Greens"><option>Onion</option><option>Leek</option><option disabled>Kale</option></optgroup></select>
<select aria-label="Crust" name=crust><option>Thin</option><option>Deep</option></select>
<select aria-label="Kept" name=kept size=2><option selected>Mine</option><option>Theirs</option></select>
<button>Send</button>
</form>
<div role=listbox aria-label="Own"><div role=option tabindex=0 aria-selected=false id=own>Picked</div></div>
<script>
const heard = document.getElementById("heard");
for (const type of ["input", "change"]) {
  document.addEventListener(type, (event) => {
    heard.textContent += " " + type + ":" + event.target.name + (event.isTrusted ? "" : "(by script)");
  }, true);
}
const kept = document.querySelector("[name=kept]");
kept.addEventListener("change", () => { kept.options[0].selected = true; });
const own = document.getElementById("own");
own.addEventListener("click", () => own.setAttribute("aria-selected", "true"));
</script>
"#;

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
}

/// The node an agent names `name` by, on the page as it is now.
fn id_of(tab: &mut Tab<'_>, name: &str) -> Result<BoxId, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))
        .map(|node| node.id)
        .ok_or_else(|| format!("no {name} in {}", read.to_outline()))
}

/// What the page has written down that it heard.
fn heard(tab: &mut Tab<'_>) -> Result<String, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with("Heard:"))
        .ok_or_else(|| format!("nothing heard in {}", read.to_outline()))
}

/// `option`, chosen or not afterwards in `list`, as an answer says.
fn chosen(tab: &mut Tab<'_>, option: &str, list: &str, now: bool) -> Result<Outcome, String> {
    Ok(Outcome::Chosen {
        node: id_of(tab, option)?,
        list: id_of(tab, list)?,
        chosen: now,
    })
}

/// Activate the option `option`, which is then chosen in `list` or not.
fn activate(tab: &mut Tab<'_>, option: &str, list: &str, now: bool) -> Result<(), String> {
    let wanted = chosen(tab, option, list, now)?;
    match tab.act(&named(option), &Verb::Activate) {
        Ok(outcome) if outcome == wanted => Ok(()),
        other => Err(format!("{option}: {other:?}, not {wanted:?}")),
    }
}

/// Where the form asks to go when its button is pressed.
fn sent(tab: &mut Tab<'_>) -> Result<String, String> {
    match tab.act(&named("Send"), &Verb::Activate) {
        Ok(Outcome::Submitted {
            method: Method::Get,
            to,
            ..
        }) => Ok(to),
        other => Err(format!("the form was not sent: {other:?}")),
    }
}

fn open(chrome: &mut Chrome) -> Result<Tab<'_>, String> {
    Tab::open_frozen(
        chrome,
        "https://example.test/lists",
        LISTS.as_bytes().to_vec(),
        (600, 700),
    )
    .map_err(|why| why.to_string())
}

#[test]
fn an_option_of_a_list_with_a_size_is_the_one_the_form_sends() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = open(&mut chrome).unwrap();
    activate(&mut tab, "Large", "Size", true).unwrap();
    assert_eq!(heard(&mut tab).unwrap(), "Heard: input:size change:size");
    // Choosing what the list holds already is still chosen, and the page
    // hears nothing more, as from a person.
    activate(&mut tab, "Large", "Size", true).unwrap();
    assert_eq!(heard(&mut tab).unwrap(), "Heard: input:size change:size");
    assert_eq!(
        sent(&mut tab).unwrap(),
        "https://example.test/send?size=Large&toppings=Bacon&crust=Thin&kept=Mine"
    );
}

#[test]
fn options_of_a_multiple_list_are_toggled_one_verb_at_a_time() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = open(&mut chrome).unwrap();
    // A disabled option is refused, by the rule that refuses it on this
    // engine's page, and is not pressed.
    let kale = id_of(&mut tab, "Kale").unwrap();
    assert_eq!(
        tab.act(&named("Kale"), &Verb::Activate),
        Err(Unacted::Refused(Refusal::Disabled { node: kale }))
    );
    for option in ["Onion", "Leek"] {
        activate(&mut tab, option, "Toppings", true).unwrap();
    }
    assert_eq!(
        heard(&mut tab).unwrap(),
        "Heard: input:toppings change:toppings input:toppings change:toppings"
    );
    let all = "https://example.test/send?size=Medium&toppings=Bacon&toppings=Onion&toppings=Leek&crust=Thin&kept=Mine";
    assert_eq!(sent(&mut tab).unwrap(), all);
    // Sending the form leaves the page, so the third verb is on the page
    // anew: two toggles on, and one that takes away what the page held,
    // leaving the others as they were.
    drop(tab);
    let mut tab = open(&mut chrome).unwrap();
    for option in ["Onion", "Leek"] {
        activate(&mut tab, option, "Toppings", true).unwrap();
    }
    activate(&mut tab, "Bacon", "Toppings", false).unwrap();
    assert_eq!(
        sent(&mut tab).unwrap(),
        "https://example.test/send?size=Medium&toppings=Onion&toppings=Leek&crust=Thin&kept=Mine"
    );
}

#[test]
fn an_option_of_a_closed_list_is_not_pressed_and_the_page_hears_nothing() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = open(&mut chrome).unwrap();
    let (deep, crust) = (
        id_of(&mut tab, "Deep").unwrap(),
        id_of(&mut tab, "Crust").unwrap(),
    );
    let refused = tab.act(&named("Deep"), &Verb::Activate);
    assert_eq!(
        refused,
        Err(Unacted::Closed {
            node: deep,
            list: crust
        })
    );
    assert_eq!(
        refused.unwrap_err().to_string(),
        format!(
            "{deep} is an option of the closed list {crust}, which this browser chooses in only through a shown Chrome on macOS, so nothing was pressed"
        )
    );
    assert_eq!(heard(&mut tab).unwrap(), "Heard:");
    assert_eq!(
        sent(&mut tab).unwrap(),
        "https://example.test/send?size=Medium&toppings=Bacon&crust=Thin&kept=Mine"
    );
}

#[test]
fn a_choice_the_page_puts_back_is_answered_as_not_chosen() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = open(&mut chrome).unwrap();
    activate(&mut tab, "Theirs", "Kept", false).unwrap();
    assert_eq!(heard(&mut tab).unwrap(), "Heard: input:kept change:kept");
    // A list a page builds of its own is not a `<select>`: its option is
    // pressed, as any control is, and what that means is the page's.
    let own = id_of(&mut tab, "Picked").unwrap();
    assert_eq!(
        tab.act(&named("Picked"), &Verb::Activate),
        Ok(Outcome::Activated {
            node: own,
            name: Some("Picked".to_owned()),
        })
    );
    let read = tab.read().unwrap();
    let picked = read
        .nodes()
        .into_iter()
        .find(|node| node.id == own)
        .map(|node| node.states.selected);
    assert_eq!(picked, Some(Some(true)), "{}", read.to_outline());
}

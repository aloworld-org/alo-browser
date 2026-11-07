/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 810: an option of a closed list on a card's page, chosen by
//! an agent without its menu.
//!
//! On a card a closed list's menu is never opened for an agent — it would
//! be a window nobody sees taking the person's keys. Against the machine's
//! Chrome, on a page of this test's own, the tab shown as a card's:
//!
//! - *Dates*, the one option beginning with `d`, is chosen by that one key —
//!   the page hears one `input` and one `change`, by a person's key
//!   (`isTrusted`), and the list holds it;
//! - *Apples*, which shares its `a` with *Apricots*, is not chosen and the
//!   page hears nothing: no one key reaches it alone;
//! - *Dates*, held already, is answered as chosen, the page hearing nothing
//!   more.
//!
//! The same page as no card's, headless: refused as ADR 0064 § 6 refuses a
//! closed list there, with nothing sent.

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_box::BoxId;
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_tab::Tab;

/// A page of this test's own, served at `https://example.test/fruit`: one
/// closed list holding *Bananas*, and a line writing down what it hears.
const FRUIT: &str = r#"<!doctype html>
<html lang=en><title>Fruit</title>
<p id=heard>Heard:</p>
<select aria-label="Fruit" name=fruit><option>Apples</option><option>Apricots</option><option selected>Bananas</option><option>Cherries</option><option>Dates</option></select>
<script>
const heard = document.getElementById("heard");
for (const type of ["input", "change"]) {
  document.addEventListener(type, (event) => {
    heard.textContent += " " + type + ":" + event.target.value + (event.isTrusted ? "" : "(by script)");
  }, true);
}
</script>
"#;

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

/// The fruit page in `chrome`, headless.
fn open(chrome: &mut Chrome) -> Result<Tab<'_>, String> {
    Tab::open_frozen(
        chrome,
        "https://example.test/fruit",
        FRUIT.as_bytes().to_vec(),
        (600, 700),
    )
    .map_err(|why| why.to_string())
}

#[test]
fn on_a_card_one_key_chooses_an_option_alone_in_its_letter_and_nothing_else() -> Result<(), String>
{
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    let mut tab = open(&mut chrome)?;
    tab.show_as_a_card();
    let (dates, apples) = (id_of(&mut tab, "Dates")?, id_of(&mut tab, "Apples")?);
    let activate =
        |tab: &mut Tab<'_>, name: &str| tab.act(&Target::Named(name.to_owned()), &Verb::Activate);
    let fruit = match activate(&mut tab, "Dates") {
        Ok(Outcome::Chosen {
            node,
            list,
            chosen: true,
        }) if node == dates => list,
        other => return Err(format!("Dates not chosen: {other:?}")),
    };
    assert_eq!(heard(&mut tab)?, "Heard: input:Dates change:Dates");
    let refused = activate(&mut tab, "Apples");
    assert_eq!(
        refused,
        Err(Unacted::NoOneKey {
            node: apples,
            list: fruit,
        })
    );
    assert!(refused.err().is_some_and(|why| {
        why.to_string()
            .ends_with("The person chooses it by pressing the list")
    }),);
    assert_eq!(
        heard(&mut tab)?,
        "Heard: input:Dates change:Dates",
        "nothing more heard"
    );
    assert_eq!(
        activate(&mut tab, "Dates"),
        Ok(Outcome::Chosen {
            node: dates,
            list: fruit,
            chosen: true,
        }),
        "held already"
    );
    assert_eq!(heard(&mut tab)?, "Heard: input:Dates change:Dates");
    Ok(())
}

#[test]
fn the_same_list_on_no_card_headless_is_refused_as_before() -> Result<(), String> {
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    let mut tab = open(&mut chrome)?;
    let dates = id_of(&mut tab, "Dates")?;
    assert!(matches!(
        tab.act(&Target::Named("Dates".to_owned()), &Verb::Activate),
        Err(Unacted::Closed { node, .. }) if node == dates
    ));
    assert_eq!(heard(&mut tab)?, "Heard:");
    Ok(())
}

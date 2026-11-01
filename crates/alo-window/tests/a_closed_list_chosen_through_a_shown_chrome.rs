/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An option of a closed list, chosen in its menu through a shown Chrome:
//! queue item 658, ADR 0071.
//!
//! Against the machine's Chrome, shown, on a page of this test's own: an
//! option several down, past a disabled option, a `hidden` one and a
//! disabled group, is chosen by `Activate` and the page hears one `input`
//! and one `change` for it; then one up, and one past an option whose
//! `display` is `none`; an option the list holds already is chosen with no
//! key, the page hearing nothing; an option the menu has no item for is not
//! chosen, and nothing is posted. A list holding no option is walked from
//! nowhere. A menu closed by another application brought in front before
//! Return is answered as not chosen, the page hearing no `change`; and so
//! is one the pointer is moved over, which is then closed, and one walked
//! while the pointer moves anywhere else (queue item 809).
//!
//! The pointer moves a menu's highlight, wherever it is moved, so each test
//! first moves it to the top of Chrome's window, where no menu of the page
//! opens, waits until the window server has it there and it has been still
//! for as long as the walk asks (`pointer`, queue item 809), and puts it
//! back where it was however the test ends.

#![cfg(target_os = "macos")]

mod pointer;

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_box::BoxId;
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_choose::Unwalked;
use alo_window::chrome_menu;
use alo_window::chrome_tab::Tab;
use alo_window::window_server::{self, Listed};
use std::time::{Duration, Instant};

/// A closed list of the options ADR 0071 measured, *Grapes*, whose
/// `display` a style sheet makes `none`, and *Ice* after the rest, so that
/// a count that took *Grapes* for an item does not stop at the end; and a
/// line that writes down what the list hears, with its value then and
/// whether it was a person's (`isTrusted`).
const FRUIT: &str = r#"<!doctype html>
<html lang=en><title>Basket</title>
<style>.gone { display: none }</style>
<p id=heard>Heard:</p>
<select aria-label="Fruit">
<option>Apples</option><option disabled>Bananas</option><option hidden>Cherries</option>
<optgroup label="Late"><option>Dates</option><option>Elderberries</option></optgroup>
<optgroup label="Off" disabled><option>Figs</option></optgroup>
<option class=gone>Grapes</option><option>Honeydew</option><option>Ice</option>
</select>
<script>
const list = document.querySelector("select");
const heard = document.getElementById("heard");
for (const type of ["input", "change"]) {
  list.addEventListener(type, (event) => {
    heard.textContent += " " + type + ":" + list.value + (event.isTrusted ? "" : "(by script)");
  });
}
</script>
"#;

/// A closed list long enough that walking its menu from the first option
/// to the last takes a few seconds of keys, each posted after a reading of
/// the window server.
fn long_list() -> String {
    let mut options = "<option>Apples</option>".to_owned();
    for number in 1..=40 {
        options += "<option>Number ";
        options += &number.to_string();
        options += "</option>";
    }
    FRUIT.replace("<option>Apples</option>", &options)
}

/// The longest the menu is waited for in the test's own thread.
const AT_MOST: Duration = Duration::from_secs(10);

/// The page at `address` in the shown Chrome `chrome`, and where the pointer
/// was before it was moved to the top of Chrome's window.
fn open<'c>(
    chrome: &'c mut Chrome,
    address: &str,
    page: &str,
) -> Result<(Tab<'c>, (f64, f64)), String> {
    let owner = chrome.process();
    let tab = Tab::open_frozen(chrome, address, page.as_bytes().to_vec(), (600, 500))
        .map_err(|why| why.to_string())?;
    let window = window_server::on_the_screen(owner)
        .map_err(|why| why.to_string())?
        .into_iter()
        .find_map(|window| window.bounds.filter(|_| window.layer == 0))
        .ok_or("Chrome's window is not on the screen")?;
    let was = pointer::at()?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a window's bounds are a few thousand points"
    )]
    pointer::move_to((
        (window.left + window.width - 40) as f64,
        (window.top + 8) as f64,
    ))?;
    Ok((tab, was))
}

/// The menu of the process `owner` that `before` did not hold, once the
/// window server shows it.
fn menu(owner: u32, before: &[Listed]) -> Result<Listed, String> {
    let deadline = Instant::now() + AT_MOST;
    loop {
        let now = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
        let number = chrome_menu::menu_of(before, &now);
        if let Some(menu) = now.into_iter().find(|window| Some(window.number) == number) {
            return Ok(menu);
        }
        if Instant::now() >= deadline {
            return Err("the menu was never shown".to_owned());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
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

/// Activate the option `option` of *Fruit*, and require it chosen.
fn choose(tab: &mut Tab<'_>, option: &str) -> Result<(), String> {
    let wanted = Outcome::Chosen {
        node: id_of(tab, option)?,
        list: id_of(tab, "Fruit")?,
        chosen: true,
    };
    match tab.act(&Target::Named(option.to_owned()), &Verb::Activate) {
        Ok(outcome) if outcome == wanted => Ok(()),
        other => Err(format!("{option}: {other:?}, not {wanted:?}")),
    }
}

#[test]
fn options_of_a_closed_list_are_chosen_in_its_menu_one_change_each() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let (mut tab, was) = open(&mut chrome, "https://example.test/fruit", FRUIT)?;
    let _put_back = pointer::PutBack(was);
    // Two down from Apples, past Bananas, disabled, and Cherries, hidden.
    choose(&mut tab, "Elderberries")?;
    assert_eq!(
        heard(&mut tab)?,
        "Heard: input:Elderberries change:Elderberries"
    );
    // One up.
    choose(&mut tab, "Dates")?;
    // Two down, past Figs, in a disabled group, and Grapes, not displayed.
    choose(&mut tab, "Honeydew")?;
    assert_eq!(
        heard(&mut tab)?,
        "Heard: input:Elderberries change:Elderberries input:Dates change:Dates input:Honeydew change:Honeydew"
    );
    // Held already: no key, and the page hears nothing more.
    choose(&mut tab, "Honeydew")?;
    // Not displayed: the menu has no item for it, and no key is posted.
    let (grapes, fruit) = (id_of(&mut tab, "Grapes")?, id_of(&mut tab, "Fruit")?);
    let refused = tab.act(&Target::Named("Grapes".to_owned()), &Verb::Activate);
    assert_eq!(
        refused,
        Err(Unacted::Unwalked {
            node: grapes,
            list: fruit,
            why: Unwalked::NoItem,
        })
    );
    assert_eq!(
        heard(&mut tab)?,
        "Heard: input:Elderberries change:Elderberries input:Dates change:Dates input:Honeydew change:Honeydew"
    );
    Ok(())
}

#[test]
fn a_closed_list_holding_nothing_is_walked_from_nowhere() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let nothing = FRUIT.replace("const heard", "list.selectedIndex = -1;\nconst heard");
    let (mut tab, was) = open(&mut chrome, "https://example.test/fruit", &nothing)?;
    let _put_back = pointer::PutBack(was);
    // Highlighting nothing, Down goes to Apples first: two to Dates.
    choose(&mut tab, "Dates")?;
    assert_eq!(heard(&mut tab)?, "Heard: input:Dates change:Dates");
    Ok(())
}

#[test]
fn a_menu_closed_by_another_application_before_return_chooses_nothing() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let owner = chrome.process();
    let (mut tab, was) = open(&mut chrome, "https://example.test/fruit", &long_list())?;
    let _put_back = pointer::PutBack(was);
    let (honeydew, fruit) = (id_of(&mut tab, "Honeydew")?, id_of(&mut tab, "Fruit")?);
    let before = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    // Once the menu is shown, Finder is brought in front, as a person
    // switching applications mid-verb does.
    let switching = std::thread::spawn(move || -> Result<(), String> {
        menu(owner, &before)?;
        let ran = std::process::Command::new("osascript")
            .args(["-e", "tell application \"Finder\" to activate"])
            .output()
            .map_err(|why| why.to_string())?;
        ran.status
            .success()
            .then_some(())
            .ok_or_else(|| String::from_utf8_lossy(&ran.stderr).into_owned())
    });
    let acted = tab.act(&Target::Named("Honeydew".to_owned()), &Verb::Activate);
    switching
        .join()
        .map_err(|_| "the switching thread panicked".to_owned())??;
    match acted {
        Err(Unacted::Unwalked {
            node,
            list,
            why: Unwalked::Gone { posted },
        }) if node == honeydew && list == fruit && posted < 44 => {}
        other => {
            return Err(format!(
                "not refused as gone: {other:?}; {}",
                heard(&mut tab)?
            ));
        }
    }
    assert_eq!(heard(&mut tab)?, "Heard:", "no change, and no input");
    Ok(())
}

#[test]
fn a_menu_the_pointer_is_moved_over_chooses_nothing_and_is_closed() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let owner = chrome.process();
    let (mut tab, was) = open(&mut chrome, "https://example.test/fruit", &long_list())?;
    let _put_back = pointer::PutBack(was);
    let (honeydew, fruit) = (id_of(&mut tab, "Honeydew")?, id_of(&mut tab, "Fruit")?);
    let before = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    let witness = before.clone();
    // Once the menu is shown, the pointer is moved to its middle, as a
    // person reaching for it mid-verb does: its highlight goes to the item
    // under the pointer.
    let reaching = std::thread::spawn(move || -> Result<(), String> {
        let shown = menu(owner, &witness)?;
        let bounds = shown.bounds.ok_or("the menu at a fraction of a point")?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a window's bounds are a few thousand points"
        )]
        pointer::move_to((
            (bounds.left + bounds.width / 2) as f64,
            (bounds.top + bounds.height / 2) as f64,
        ))
    });
    let acted = tab.act(&Target::Named("Honeydew".to_owned()), &Verb::Activate);
    reaching
        .join()
        .map_err(|_| "the reaching thread panicked".to_owned())??;
    match acted {
        Err(Unacted::Unwalked {
            node,
            list,
            why: Unwalked::UnderPointer { posted },
        }) if node == honeydew && list == fruit && posted < 44 => {}
        other => {
            return Err(format!(
                "not refused under the pointer: {other:?}; {}",
                heard(&mut tab)?
            ));
        }
    }
    assert_eq!(heard(&mut tab)?, "Heard:", "no change, and no input");
    let now = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    assert_eq!(chrome_menu::menu_of(&before, &now), None, "closed: {now:?}");
    Ok(())
}

#[test]
fn a_menu_walked_while_the_pointer_moves_elsewhere_chooses_nothing_and_is_closed()
-> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let owner = chrome.process();
    let (mut tab, was) = open(&mut chrome, "https://example.test/fruit", &long_list())?;
    let _put_back = pointer::PutBack(was);
    let (honeydew, fruit) = (id_of(&mut tab, "Honeydew")?, id_of(&mut tab, "Fruit")?);
    let before = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    let window = before
        .iter()
        .find_map(|window| window.bounds.filter(|_| window.layer == 0))
        .ok_or("Chrome's window is not on the screen")?;
    let witness = before.clone();
    // Once the menu is shown, the pointer is moved along the top of
    // Chrome's window, a hundred points from where it rested — nowhere
    // near the menu — as a person's hand on the mouse does mid-verb: the
    // menu's highlight moves with it (queue item 809).
    let elsewhere = std::thread::spawn(move || -> Result<(), String> {
        menu(owner, &witness)?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a window's bounds are a few thousand points"
        )]
        pointer::move_to((
            (window.left + window.width - 140) as f64,
            (window.top + 8) as f64,
        ))
    });
    let acted = tab.act(&Target::Named("Honeydew".to_owned()), &Verb::Activate);
    elsewhere
        .join()
        .map_err(|_| "the moving thread panicked".to_owned())??;
    match acted {
        Err(Unacted::Unwalked {
            node,
            list,
            why: Unwalked::PointerMoving { posted },
        }) if node == honeydew && list == fruit && posted < 44 => {}
        other => {
            return Err(format!(
                "not refused as moving: {other:?}; {}",
                heard(&mut tab)?
            ));
        }
    }
    assert_eq!(heard(&mut tab)?, "Heard:", "no change, and no input");
    let now = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    assert_eq!(chrome_menu::menu_of(&before, &now), None, "closed: {now:?}");
    Ok(())
}

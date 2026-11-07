/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 802: an input method's composition on a card's page, and
//! where its candidates stand.
//!
//! Against the machine's Chrome, headless, on a page of this test's own —
//! the input a person's, sent as the conversation's window sends it:
//!
//! 1. **where the person is typing**, read as a composition begins: in a
//!    field holding *Kampa*, at the right edge of its words, on its line,
//!    as tall as it — in numbers, against where Chrome itself drew the
//!    field; in an empty field, at the start of its editing box, its
//!    placeholder never counted;
//! 2. **a composition** — `きょう` composed, then `今日` chosen — reaching
//!    the field as Chrome's own input methods reach it: the page hears
//!    `compositionstart`, `compositionupdate` and `input` with the composed
//!    words in the field, then with what was chosen, then
//!    `compositionend`: the field holds *Kampa今日*.
//!
//! Through alo's CEF host the same commands are sent: not tested here, the
//! road being the DevTools pipe's in both.

use alo_window::card_input::{CardInput, Went};
use alo_window::chrome::Chrome;
use alo_window::chrome_card_caret::{self, Caret};
use alo_window::chrome_card_input;
use alo_window::chrome_tab::Tab;

/// A page of this test's own: a field holding *Kampa*, at a known place,
/// a field holding nothing with a placeholder, and a line writing down the
/// field's composition events and value.
const PAGE: &str = r#"<!doctype html>
<html lang=en><title>Typing</title>
<style>body{margin:0}
input{position:absolute;left:40px;width:300px;padding:2px;border:1px solid #888;
  font:20px/24px sans-serif;height:24px}</style>
<input id=held style="top:40px" value="Kampa" aria-label="Held">
<input id=bare style="top:120px" placeholder="Search Wikipedia" aria-label="Bare">
<p id=heard style="position:absolute;top:200px">Heard:</p>
<script>
const heard = document.getElementById("heard");
for (const type of ["compositionstart", "compositionupdate", "compositionend"]) {
  document.addEventListener(type, (event) => {
    heard.textContent += " " + type + (event.data ? ":" + event.data : "");
  }, true);
}
document.addEventListener("input", (event) => {
  heard.textContent += " value:" + event.target.value;
}, true);
</script>
"#;

/// What the page has written down that it heard.
fn heard(tab: &mut Tab<'_>) -> Result<String, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with("Heard:"))
        .ok_or_else(|| format!("nothing heard in {}", read.to_outline()))
}

/// A person's press on the page at `at`, giving the field there the
/// keyboard — at its words' end, the press being to their right.
fn press(tab: &mut Tab<'_>, (x, y): (f64, f64)) {
    for went in [Went::Pressed { clicks: 1 }, Went::Released { clicks: 1 }] {
        chrome_card_input::forward(tab, &CardInput::Pointer { went, x, y });
    }
}

#[test]
fn the_caret_is_read_where_the_person_types_and_a_composition_reaches_the_field()
-> Result<(), String> {
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/typing",
        PAGE.as_bytes().to_vec(),
        (600, 400),
    )
    .map_err(|why| why.to_string())?;

    // 1. Where the person types: the field's border 1 and padding 2 in
    // from 40; after words, as tall as Chrome draws their run (23 at 20 px
    // in its line of 24); in an empty field, its editing box's 24.
    press(&mut tab, (300.0, 55.0));
    let held = chrome_card_caret::caret(&mut tab).ok_or("no caret in Held")?;
    eprintln!("the caret after Kampa: {held:?}");
    assert_eq!((held.y, held.height), (43, 23), "on its line: {held:?}");
    assert!(
        held.x > 43 + 40 && held.x < 43 + 100,
        "after Kampa's 20 px words: {held:?}"
    );
    press(&mut tab, (300.0, 135.0));
    assert_eq!(
        chrome_card_caret::caret(&mut tab),
        Some(Caret {
            x: 43,
            y: 123,
            height: 24
        }),
        "at an empty field's start, its placeholder not counted"
    );

    // 2. A composition, then what it came to.
    press(&mut tab, (300.0, 55.0));
    for input in [
        CardInput::Compose {
            text: "きょう".to_owned(),
            selection: (3, 3),
        },
        CardInput::Commit("今日".to_owned()),
    ] {
        chrome_card_input::forward(&mut tab, &input);
    }
    // As Chrome's own input methods are heard: the composed words in the
    // field while composed, then what was chosen, then the end.
    assert_eq!(
        heard(&mut tab)?,
        "Heard: compositionstart compositionupdate:きょう value:Kampaきょう \
         compositionupdate:今日 value:Kampa今日 compositionend:今日"
    );
    Ok(())
}

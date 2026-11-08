/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 882: a button whose middle is its own icon is pressed.
//!
//! Measuring Figma (item 879), `activate` was refused three times — *the
//! middle of box#73 is under Chrome's node 76* — on buttons whose middle is
//! an icon drawn as an SVG. Confirmed on this page, frozen, with Chrome
//! 157.0.8103.38 (the session scratchpad's `m882/probe-157.txt`): what
//! `DOM.getNodeForLocation` names at such a button's middle is the SVG's
//! `path` or `rect`, which is inside the button's DOM, and which Chrome's
//! tree of what the page is leaves out — an `<svg>`'s shapes are not in it,
//! and an `aria-hidden` one not at all — so the check that a click lands on
//! the node or something inside it, read from that tree alone, said it was
//! covered. It now reads what the tree leaves out from the node's DOM.
//!
//! Through the machine's Chrome and through the Chromium alo carries (its
//! CEF host, ADR 0079 § 8; skipped, saying why, with no `CEF_PATH`): each
//! button pressed on its own icon is pressed — the page hears a trusted
//! click on it — and a button with something of the page's over its icon
//! is still not pressed, and the page hears nothing of it.

#[cfg(target_os = "macos")]
mod carried;

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_tab::Tab;

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 500);

/// Four buttons whose middle is an icon, as Figma's were: one named by its
/// label holding an SVG, one holding an `aria-hidden` SVG and its words
/// for a screen reader only, an element given the button role holding one, and one named
/// by its label whose icon an overlay of the page's lies over.
const ICONS: &str = r#"<!doctype html>
<html lang=en><title>Icons</title>
<style>
button, [role=button] { display: block; margin: 10px; width: 40px; height: 40px; padding: 0; }
svg { display: block; margin: 12px; }
.sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
#over { position: absolute; left: 10px; top: 230px; width: 40px; height: 40px; }
#lid { position: absolute; left: 30px; top: 250px; width: 20px; height: 20px; background: white; }
</style>
<p id=heard>Heard:</p>
<button id=close aria-label="Close"><svg width=16 height=16 viewBox="0 0 16 16"><path d="M0 0L16 16M16 0L0 16" stroke="black" stroke-width=4 /></svg></button>
<button id=words><svg aria-hidden=true width=16 height=16 viewBox="0 0 16 16"><rect width=16 height=16 /></svg><span class=sr>Dismiss</span></button>
<div id=menu role=button tabindex=0 aria-label="Menu"><svg width=16 height=16 viewBox="0 0 16 16"><rect width=16 height=16 /></svg></div>
<button id=over aria-label="Covered"><svg width=16 height=16 viewBox="0 0 16 16"><rect width=16 height=16 /></svg></button>
<div id=lid></div>
<script>
const heard = document.getElementById("heard");
for (const id of ["close", "words", "menu", "over"]) {
  document.getElementById(id).addEventListener("click", (event) => {
    heard.textContent += " " + id + (event.isTrusted ? "" : "(by script)");
  });
}
</script>
"#;

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
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

/// The page opened through `chromium`, each button pressed on its icon.
fn pressed_on_their_icons_through(chromium: &Chromium) -> Result<(), String> {
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/icons",
        ICONS.as_bytes().to_vec(),
        SIZE,
    )
    .map_err(|why| format!("{why:?}"))?;
    for (name, id) in [("Close", "close"), ("Dismiss", "words"), ("Menu", "menu")] {
        match tab.act(&named(name), &Verb::Activate) {
            Ok(Outcome::Activated { .. }) => {}
            other => return Err(format!("{name}: {other:?}")),
        }
        let said = heard(&mut tab)?;
        if !said.ends_with(&format!(" {id}")) {
            return Err(format!("{name} pressed, and the page heard {said:?}"));
        }
    }
    // The page's own lid over the last one's icon: not pressed, and the
    // page hears nothing more.
    match tab.act(&named("Covered"), &Verb::Activate) {
        Err(Unacted::Covered { by: Some(_), .. }) => {}
        other => return Err(format!("Covered: {other:?}")),
    }
    let said = heard(&mut tab)?;
    if said != "Heard: close words menu" {
        return Err(format!("the page heard {said:?}"));
    }
    Ok(())
}

#[test]
fn a_button_pressed_on_its_own_icon_through_chrome_is_pressed() {
    pressed_on_their_icons_through(&Chromium::Machines).unwrap();
}

/// The same through the Chromium alo carries, its CEF host.
#[cfg(target_os = "macos")]
#[test]
fn through_alos_cef_host_a_button_pressed_on_its_own_icon_is_pressed() {
    let Some(host) = carried::host("882").unwrap() else {
        return;
    };
    pressed_on_their_icons_through(&host.chromium()).unwrap();
}

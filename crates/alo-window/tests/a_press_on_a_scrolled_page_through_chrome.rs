/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 922: a press on a page scrolled away from its top is pressed.
//!
//! Rehearsing the demonstration (10 November 2026, unlocked screen),
//! `activate` on Toolshop's *Add to cart*, plainly in the middle of the
//! card once it was scrolled there, was refused: *Chrome finds nothing at
//! the middle of box#907*. Measured with Chrome 157.0.8103.50 on that page
//! (the session scratchpad's `p922/`): `DOM.getContentQuads` says where a
//! node is in the **window** — the button at 462…565 × 318…380 of a window
//! 700 tall — and `DOM.getNodeForLocation` takes its point in the
//! **page**, the window's point plus how far the page is scrolled. Asked at
//! the button's middle as the window has it, 513, 349, with the page
//! scrolled 547 down, Chrome said *No node found at given location*; asked
//! at 513, 896 it named the button. A page at its top, as every page of the
//! earlier tests was, has the two the same, which is why only a scrolled
//! one was refused — and a point the window and the page disagree on can
//! as well land on something else, as it did on Toolshop's first page.
//!
//! So asserted here, through the machine's Chrome and through the Chromium
//! alo carries (its CEF host, ADR 0079 § 8; skipped, saying why, with no
//! `CEF_PATH`), on a frozen page shaped as Toolshop's are and scrolled by
//! itself as it loads, down and across:
//!
//! 1. **A link wrapping a product's card** — a picture, a heading, a price —
//!    far down and across the page, which the page takes as a router does
//!    (its own `click`, the default prevented, the address pushed): pressed,
//!    answered `Activated`, and **heard** by the page as a trusted click.
//! 2. **A button holding an icon element of its own** and words, as
//!    Angular's Font Awesome draws one, further down: pressed, `Activated`,
//!    and heard — the page's cart count goes from 0 to 1.
//! 3. **That button under the page's own overlay**, fixed over the window
//!    once the cart has something in it: not pressed, `Covered` by a node
//!    Chrome names, and the page hears nothing more.

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

/// A shop's page as Toolshop's is built: a header with the cart's count, a
/// product's card as a link far down and across, an *Add to cart* button
/// with its icon further down, and an overlay the page shows over the
/// window once something is in the cart. It scrolls itself as it loads.
const SHOP: &str = r#"<!doctype html>
<html lang=en><title>Shop</title>
<style>
body { margin: 0; width: 2000px; height: 3200px; position: relative; font-size: 16px; }
header { position: absolute; left: 0; top: 0; }
.card { position: absolute; left: 1100px; top: 1300px; width: 240px; display: block; text-decoration: none; color: black; }
.card img { display: block; width: 240px; height: 160px; }
#btn-add-to-cart { position: absolute; left: 1300px; top: 2300px; width: 140px; height: 40px; }
fa-icon { display: inline-block; width: 16px; height: 16px; }
#toast { position: fixed; left: 0; top: 0; width: 100%; height: 100%; background: rgba(0, 0, 0, 0.3); }
</style>
<header><p id=heard>Heard:</p><p id=cart>Cart 0</p></header>
<a class=card href="/product/01" data-test="product-01">
  <div class=card-img-wrapper><img alt="Combination Pliers" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='240' height='160'/%3E"></div>
  <div class=card-body><h5 class=card-title>Combination Pliers</h5></div>
  <div class=card-footer><span>$14.15</span></div>
</a>
<button id=btn-add-to-cart class="btn-success btn" data-test="add-to-cart"><fa-icon class="ng-fa-icon"><svg aria-hidden=true focusable=false role=img viewBox="0 0 16 16"><path d="M0 0H16V16H0Z" /></svg></fa-icon> Add to cart</button>
<script>
const heard = document.getElementById("heard");
const said = (what, event) => {
  heard.textContent += " " + what + (event.isTrusted ? "" : "(by script)");
};
document.querySelector(".card").addEventListener("click", (event) => {
  event.preventDefault();
  said("product", event);
  history.pushState(null, "", "/shop#product-01");
});
let count = 0;
document.getElementById("btn-add-to-cart").addEventListener("click", (event) => {
  said("cart", event);
  count += 1;
  document.getElementById("cart").textContent = "Cart " + count;
  const toast = document.createElement("div");
  toast.id = "toast";
  document.body.append(toast);
});
addEventListener("load", () => scrollTo(300, 200));
</script>
"#;

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
}

/// The first name on the page that begins with `start`.
fn name_starting(tab: &mut Tab<'_>, start: &str) -> Result<String, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with(start))
        .ok_or_else(|| format!("no {start:?} in {}", read.to_outline()))
}

/// The page opened through `chromium`, scrolled by itself, and each of its
/// presses made and heard.
fn pressed_on_a_scrolled_page_through(chromium: &Chromium) -> Result<(), String> {
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/shop",
        SHOP.as_bytes().to_vec(),
        SIZE,
    )
    .map_err(|why| format!("{why:?}"))?;
    // The product's card, by the name Chrome's tree gives the link: all it
    // holds, as Toolshop's was named.
    let product = name_starting(&mut tab, "Combination Pliers")?;
    match tab.act(&named(&product), &Verb::Activate) {
        Ok(Outcome::Activated { .. }) => {}
        other => return Err(format!("{product}: {other:?}")),
    }
    let said = name_starting(&mut tab, "Heard:")?;
    if said != "Heard: product" {
        return Err(format!("the product pressed, and the page heard {said:?}"));
    }
    match tab.act(&named("Add to cart"), &Verb::Activate) {
        Ok(Outcome::Activated { .. }) => {}
        other => return Err(format!("Add to cart: {other:?}")),
    }
    let said = name_starting(&mut tab, "Heard:")?;
    let cart = name_starting(&mut tab, "Cart")?;
    if said != "Heard: product cart" || cart != "Cart 1" {
        return Err(format!(
            "Add to cart pressed, and the page heard {said:?}, its cart {cart:?}"
        ));
    }
    // The page's overlay over the window now: not pressed, and the page
    // hears nothing more.
    match tab.act(&named("Add to cart"), &Verb::Activate) {
        Err(Unacted::Covered { by: Some(_), .. }) => {}
        other => return Err(format!("Add to cart under the overlay: {other:?}")),
    }
    let said = name_starting(&mut tab, "Heard:")?;
    let cart = name_starting(&mut tab, "Cart")?;
    if said != "Heard: product cart" || cart != "Cart 1" {
        return Err(format!(
            "the page heard {said:?} under its overlay, its cart {cart:?}"
        ));
    }
    Ok(())
}

#[test]
fn a_press_on_a_scrolled_page_through_chrome_is_pressed_and_heard() {
    pressed_on_a_scrolled_page_through(&Chromium::Machines).unwrap();
}

/// The same through the Chromium alo carries, its CEF host.
#[cfg(target_os = "macos")]
#[test]
fn through_alos_cef_host_a_press_on_a_scrolled_page_is_pressed_and_heard() {
    let Some(host) = carried::host("922").unwrap() else {
        return;
    };
    pressed_on_a_scrolled_page_through(&host.chromium()).unwrap();
}

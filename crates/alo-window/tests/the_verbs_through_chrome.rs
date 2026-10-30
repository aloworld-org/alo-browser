/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The verbs, through Chrome: queue item 621, ADR 0058 § 4.
//!
//! The order item 131's agent places on the frozen `web-a-form` — three
//! fields filled by the words on them, a size chosen, the order sent by its
//! button — is placed through the machine's Chrome and through this
//! browser's own engine, with the very same verbs, and comes to the same
//! outcomes. A page of this test's own then shows the rest: every refusal
//! `alo_agent::verb` makes is reached on Chrome's page by its own name, and
//! the page hears none of them; what the page does hear is a person's click
//! and a person's typing, trusted, never script; a click that would land on
//! something else is not made; and a form is known to have been sent even
//! when the page was busy with its own work as it went.
//!
//! **Through the Chromium alo carries too** (queue item 726, ADR 0079 § 8):
//! the order, a person's click and typing, and the tabs a page opens are
//! done again through alo's own CEF host, started as the application starts
//! it. With no CEF distribution named by `CEF_PATH`, those tests say why on
//! their error output and return.

#[cfg(target_os = "macos")]
mod carried;

use alo_agent::verb::{ScrollBy, Target, Verb};
use alo_agent::{Outcome, Refusal};
use alo_box::{KnownRole, Role};
use alo_corpus::{Case, cases_directory, corpus_fonts};
use alo_dom::submission::Method;
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_tab::Tab;

/// Where the form was served from: the first line of its `origin.txt`.
const FORM: &str = "https://httpbin.org/forms/post";

/// The size the case is laid out at, its `size.txt`.
const SIZE: (u32, u32) = (600, 700);

fn named_of(role: KnownRole, name: &str) -> Target {
    Target::NamedOfRole {
        role: Role::Known(role),
        name: name.to_owned(),
    }
}

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
}

fn put(text: &str) -> Verb {
    Verb::PutText(text.to_owned())
}

/// The order, verb by verb, as the agent of `an_agent_outside_the_browser.rs`
/// sends it down the socket.
fn the_order() -> Vec<(Target, Verb)> {
    vec![
        (named("Customer name:"), put("Ada Lovelace")),
        (named("Telephone:"), put("+44 20 7946 0000")),
        (named("E-mail address:"), put("ada@example.com")),
        (named_of(KnownRole::Radio, "Large"), Verb::Activate),
        (named_of(KnownRole::Button, "Submit order"), Verb::Activate),
    ]
}

/// The verbs on the form that `an_agent_outside_the_browser.rs` is refused.
fn the_refused() -> Vec<(Target, Verb)> {
    vec![
        (named("Nothing here"), Verb::Activate),
        (named("Small"), put("x")),
        (named("Preferred delivery time:"), put("12:00")),
    ]
}

/// What a verb came to, without the node it came to it on: the engine's ids
/// are its boxes and Chrome's are its DOM nodes, and neither is the other's.
fn came_to(result: Result<Outcome, Refusal>) -> String {
    match result {
        Ok(Outcome::Activated { name, .. }) => format!("activated {name:?}"),
        Ok(Outcome::Followed { to, .. }) => format!("followed to {to}"),
        Ok(Outcome::TextPut { text, .. }) => format!("put {text}"),
        Ok(Outcome::TextCanceled { text, .. }) => format!("cancelled {text}"),
        Ok(Outcome::Submitted { method, to, .. }) => format!("{} {to}", method.name()),
        Ok(other) => format!("{other}"),
        Err(refusal) => kind(&refusal),
    }
}

/// A refusal's kind, by its name.
fn kind(refusal: &Refusal) -> String {
    format!("{refusal:?}")
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The frozen form's case and bytes.
fn the_form() -> Result<(Case, Vec<u8>), String> {
    let directory = cases_directory().join("web-a-form");
    let origin = std::fs::read_to_string(directory.join("origin.txt"))
        .map_err(|why| format!("no origin: {why}"))?;
    if origin.lines().next() != Some(FORM) {
        return Err(format!(
            "the form was taken from {:?}",
            origin.lines().next()
        ));
    }
    let case = Case::read(&directory).ok_or("web-a-form is not a case")?;
    let page = std::fs::read(directory.join("page.html")).map_err(|why| why.to_string())?;
    Ok((case, page))
}

/// `verbs` done on the form through this browser's own engine, as item 131
/// does them.
fn through_ours(verbs: &[(Target, Verb)]) -> Result<Vec<String>, String> {
    let (case, _) = the_form()?;
    let address = alo_url::parse(FORM).map_err(|why| format!("{why:?}"))?;
    let mut renderer = Renderer::new(corpus_fonts());
    let page = Page::new(case.html.clone(), Size::new(case.size.0, case.size.1)).at(address);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the form did not load: {other:?}")),
    }
    verbs
        .iter()
        .map(|(target, verb)| {
            match renderer.handle(ToRenderer::Act {
                target: target.clone(),
                verb: verb.clone(),
            }) {
                FromRenderer::Acted { outcome, .. } => Ok(came_to(Ok(outcome))),
                FromRenderer::Refused(refusal) => Ok(came_to(Err(refusal))),
                other => Err(format!("{target}: {other:?}")),
            }
        })
        .collect()
}

/// `verb` done through Chrome, with what it came to written as
/// [`came_to`] writes it — or why Chrome did not do it, which is a failure.
fn through_chrome(tab: &mut Tab<'_>, target: &Target, verb: &Verb) -> Result<String, String> {
    match tab.act(target, verb) {
        Ok(outcome) => Ok(came_to(Ok(outcome))),
        Err(Unacted::Refused(refusal)) => Ok(came_to(Err(refusal))),
        Err(why) => Err(format!("{target}: {why}")),
    }
}

/// The node of `snapshot` that reads as `line`.
fn node<'s>(snapshot: &'s Snapshot, line: &str) -> Result<&'s SnapshotNode, String> {
    snapshot
        .nodes()
        .into_iter()
        .find(|node| node.to_string() == line)
        .ok_or_else(|| format!("no {line} in {}", snapshot.to_outline()))
}

/// The words read inside `node`.
fn words_in(node: &SnapshotNode) -> Vec<String> {
    node.children
        .iter()
        .filter(|child| child.role == Role::Known(KnownRole::Text))
        .filter_map(|child| child.name.clone())
        .collect()
}

#[test]
fn the_order_placed_through_chrome_comes_to_what_it_comes_to_through_ours() {
    the_order_through(&Chromium::Machines).unwrap();
}

/// The same order placed through the Chromium alo carries, its CEF host
/// (queue item 726, ADR 0079 § 8): the same verbs, the same outcomes.
#[cfg(target_os = "macos")]
#[test]
fn the_order_placed_through_alos_cef_host_comes_to_what_it_comes_to_through_ours() {
    let Some(host) = carried::host("726").unwrap() else {
        return;
    };
    the_order_through(&host.chromium()).unwrap();
}

/// The order placed on the frozen form through `chromium`, seen to come
/// to what it comes to through this browser's own engine.
fn the_order_through(chromium: &Chromium) -> Result<(), String> {
    let ours = through_ours(&the_order())?;
    assert_eq!(
        ours,
        [
            "put Ada Lovelace",
            "put +44 20 7946 0000",
            "put ada@example.com",
            "activated Some(\"Large\")",
            "POST https://httpbin.org/post",
        ]
    );

    let (_, page) = the_form()?;
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    let mut tab =
        Tab::open_frozen(&mut chrome, FORM, page, SIZE).map_err(|why| format!("{why:?}"))?;
    let order = the_order();
    let (sending, filling) = order.split_last().ok_or("an empty order")?;
    let mut chromes = Vec::new();
    for (target, verb) in filling {
        chromes.push(through_chrome(&mut tab, target, verb)?);
    }

    // Read again before it is sent: what was put in is in the fields, as
    // the words they hold, and the size is chosen.
    let filled = tab.read().map_err(|why| format!("{why:?}"))?;
    for (field, text) in [
        (r#"textbox "Customer name:""#, "Ada Lovelace"),
        (r#"textbox "Telephone:""#, "+44 20 7946 0000"),
        (r#"textbox "E-mail address:""#, "ada@example.com"),
    ] {
        assert_eq!(words_in(node(&filled, field)?), [text], "{field}");
    }
    node(&filled, r#"radio "Large" [checked=true]"#)?;
    node(&filled, r#"radio "Small" [checked=false]"#)?;

    let (target, verb) = sending;
    chromes.push(through_chrome(&mut tab, target, verb)?);
    assert_eq!(chromes, ours, "the same verbs, the same outcomes");
    // Nothing reached a server: the form's own address was answered with
    // its bytes, and the icon, if Chrome asked for one, was refused.
    assert!(
        tab.refused().iter().all(|url| [
            "https://httpbin.org/favicon.ico",
            "https://httpbin.org/post"
        ]
        .contains(&url.as_str())),
        "{:?}",
        tab.refused()
    );
    Ok(())
}

#[test]
fn the_form_refuses_through_chrome_what_it_refuses_through_ours() {
    let ours = through_ours(&the_refused()).unwrap();
    assert_eq!(ours, ["NotFound", "NotAField", "Unbuilt"]);

    let (_, page) = the_form().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let mut tab = Tab::open_frozen(&mut chrome, FORM, page, SIZE).unwrap();
    let chromes: Vec<String> = the_refused()
        .iter()
        .map(|(target, verb)| through_chrome(&mut tab, target, verb).unwrap())
        .collect();
    assert_eq!(chromes, ours);
}

/// A page of this test's own, served at `https://example.test/verbs`: a
/// thing for each refusal, a button under a cover, fields, a link, and a
/// line that writes down every click and every key the page hears — and
/// whether each was a person's (`isTrusted`) or a script's.
const VERBS: &str = r#"<!doctype html>
<html lang=en><title>Verbs</title>
<button style="position: absolute; left: 10px; top: 10px">Under</button>
<div style="position: fixed; left: 0; top: 0; width: 300px; height: 50px"></div>
<main style="margin-top: 60px">
<p id=heard>Heard:</p>
<button>Twice</button> <button>Twice</button>
<p>Words</p>
<button disabled>Off</button>
<input aria-label="Fixed" readonly>
<input aria-label="Day" type=date>
<input aria-label="When" type=time>
<input aria-label="Short" maxlength=3>
<select aria-label="Size"><option>Small</option></select>
<select aria-label="Sizes" multiple><option>Small</option><option>Large</option></select>
<input aria-label="Name">
<input aria-label="Kept" id=kept>
<a href="/next">Next</a>
</main>
<script>
const heard = document.getElementById("heard");
for (const type of ["click", "keydown", "beforeinput", "input", "change"]) {
  document.addEventListener(type, (event) => {
    heard.textContent += " " + type + (event.isTrusted ? "" : "(by script)");
  }, true);
}
document.getElementById("kept").addEventListener("beforeinput", (event) => event.preventDefault());
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

#[test]
fn every_refusal_is_reached_on_chromes_page_and_the_page_hears_none_of_them() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/verbs",
        VERBS.as_bytes().to_vec(),
        SIZE,
    )
    .unwrap();
    let refused: Vec<String> = [
        (named("Nothing here"), Verb::Activate),
        (named("Twice"), Verb::Activate),
        (named("Words"), Verb::Activate),
        (named("Off"), Verb::Activate),
        (named("Words"), put("x")),
        (named("Fixed"), put("x")),
        (named("Day"), put("12/10/2026")),
        (named("When"), put("12:00")),
        (named("Size"), put("Large")),
        (named("Sizes"), put("Large")),
        (named("Short"), put("four")),
        (named("Words"), Verb::Scroll(ScrollBy::ToEnd)),
    ]
    .iter()
    .map(|(target, verb)| match tab.act(target, verb) {
        Err(Unacted::Refused(refusal)) => kind(&refusal),
        other => format!("not refused: {other:?}"),
    })
    .collect();
    assert_eq!(
        refused,
        [
            "NotFound",
            "Ambiguous",
            "NotOperable",
            "Disabled",
            "NotAField",
            "ReadOnly",
            "NotADate",
            "Unbuilt",
            "Chooses",
            "Chooses",
            "TooLong",
            "DoesNotScroll",
        ]
    );
    // Refused here, before Chrome was asked to do anything: the page heard
    // not one click or key.
    assert_eq!(heard(&mut tab).unwrap(), "Heard:");

    // The ambiguous refusal names both, by the ids an agent can name them
    // with, and either one by its id is a click the page hears.
    let Err(Unacted::Refused(Refusal::Ambiguous { candidates, .. })) =
        tab.act(&named("Twice"), &Verb::Activate)
    else {
        panic!("Twice is two buttons");
    };
    assert_eq!(candidates.len(), 2);
    let second = candidates.last().copied().unwrap();
    assert_eq!(
        tab.act(&Target::Node(second), &Verb::Activate),
        Ok(Outcome::Activated {
            node: second,
            name: Some("Twice".to_owned()),
        })
    );
    assert_eq!(heard(&mut tab).unwrap(), "Heard: click");
}

#[test]
fn the_page_hears_a_persons_click_and_a_persons_typing() {
    a_persons_click_and_typing_through(&Chromium::Machines).unwrap();
}

/// The same page through the Chromium alo carries, its CEF host (queue
/// item 726, ADR 0079 § 8): a person's click and typing, never script.
#[cfg(target_os = "macos")]
#[test]
fn through_alos_cef_host_the_page_hears_a_persons_click_and_typing() {
    let Some(host) = carried::host("726").unwrap() else {
        return;
    };
    a_persons_click_and_typing_through(&host.chromium()).unwrap();
}

/// The verbs' page opened through `chromium`, and what it hears of a
/// person's click and typing seen to be a person's.
fn a_persons_click_and_typing_through(chromium: &Chromium) -> Result<(), String> {
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/verbs",
        VERBS.as_bytes().to_vec(),
        SIZE,
    )
    .map_err(|why| format!("{why:?}"))?;

    // A button a person could not press, because something lies over it,
    // is not pressed.
    match tab.act(&named("Under"), &Verb::Activate) {
        Err(Unacted::Covered { by: Some(_), .. }) => {}
        other => return Err(format!("the cover was pressed through: {other:?}")),
    }
    assert_eq!(heard(&mut tab)?, "Heard:");

    // Typing, as a person types: the key that selects what is there, then
    // the text, which the page is asked about first.
    assert_eq!(
        through_chrome(&mut tab, &named("Name"), &put("Ada"))?,
        "put Ada"
    );
    assert_eq!(heard(&mut tab)?, "Heard: keydown beforeinput input");
    let read = tab.read().map_err(|why| format!("{why:?}"))?;
    assert_eq!(words_in(node(&read, r#"textbox "Name""#)?), ["Ada"]);

    // A page that will not take the text keeps what it had, and says so;
    // the field before it hears its `change` as the keyboard leaves it.
    assert_eq!(
        through_chrome(&mut tab, &named("Kept"), &put("x"))?,
        "cancelled x"
    );
    assert_eq!(
        heard(&mut tab)?,
        "Heard: keydown beforeinput input change keydown beforeinput"
    );
    let read = tab.read().map_err(|why| format!("{why:?}"))?;
    assert!(words_in(node(&read, r#"textbox "Kept""#)?).is_empty());

    // The empty text empties a field.
    assert_eq!(through_chrome(&mut tab, &named("Name"), &put(""))?, "put ");
    let read = tab.read().map_err(|why| format!("{why:?}"))?;
    assert!(words_in(node(&read, r#"textbox "Name""#)?).is_empty());
    assert!(!heard(&mut tab)?.contains("(by script)"));

    // A link is followed, to where its author wrote; the frozen page's tab
    // then refuses to go there, as it refuses every address but its own.
    assert_eq!(
        through_chrome(&mut tab, &named("Next"), &Verb::Activate)?,
        "followed to /next"
    );
    Ok(())
}

/// A form whose button, as it is pressed, queues half a second of the page's
/// own work ahead of the form being sent.
const BUSY: &str = r#"<!doctype html>
<html lang=en><title>Busy</title>
<form method=post action="/post"><input aria-label="Name" name=name value=Ada><button>Send</button></form>
<script>
document.querySelector("button").addEventListener("click", () => {
  for (let i = 0; i < 10; i++) window.postMessage("busy", "*");
});
window.addEventListener("message", () => {
  const end = Date.now() + 50;
  while (Date.now() < end) {}
});
</script>
"#;

#[test]
fn a_form_is_known_to_be_sent_though_the_page_is_busy_as_it_goes() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/busy",
        BUSY.as_bytes().to_vec(),
        SIZE,
    )
    .unwrap();
    let sent = tab.act(&named("Send"), &Verb::Activate).unwrap();
    assert!(
        matches!(
            &sent,
            Outcome::Submitted { method: Method::Post, to, .. } if to == "https://example.test/post"
        ),
        "{sent:?}"
    );
}

/// A page with two ways of opening a tab: a link to another window, and a
/// button whose script opens one.
const OPENS: &str = r#"<!doctype html>
<html lang=en><title>Opens</title>
<p><a href="https://example.invalid/away" target=_blank>Away</a>
<p><button onclick="window.open('https://example.invalid/opened')">Open</button>
"#;

#[test]
fn a_tab_the_frozen_page_opens_is_closed_before_it_loads_anything() {
    a_tab_the_page_opens_closed_through(&Chromium::Machines).unwrap();
}

/// The same tabs opened through the Chromium alo carries, its CEF host
/// (queue item 726, ADR 0079 § 8), which holds a new tab in Chrome's own
/// style, where CEF does not stall it (CEF issue #4308).
#[cfg(target_os = "macos")]
#[test]
fn through_alos_cef_host_a_tab_the_frozen_page_opens_is_closed_before_it_loads() {
    let Some(host) = carried::host("726").unwrap() else {
        return;
    };
    a_tab_the_page_opens_closed_through(&host.chromium()).unwrap();
}

/// A link and a script each opening a tab from a frozen page in
/// `chromium`, and each tab seen closed before it loaded anything.
fn a_tab_the_page_opens_closed_through(chromium: &Chromium) -> Result<(), String> {
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    {
        let mut tab = Tab::open_frozen(
            &mut chrome,
            "https://example.test/opens",
            OPENS.as_bytes().to_vec(),
            SIZE,
        )
        .map_err(|why| format!("{why:?}"))?;
        // A link that names another window was activated, not followed, as
        // the engine answers it.
        assert_eq!(
            through_chrome(&mut tab, &named("Away"), &Verb::Activate)?,
            "activated Some(\"Away\")"
        );
        assert_eq!(
            through_chrome(&mut tab, &named("Open"), &Verb::Activate)?,
            "activated Some(\"Open\")"
        );
        // Read once more, so that Chrome has told of every tab by now.
        tab.read().map_err(|why| format!("{why:?}"))?;
        assert_eq!(tab.closed(), 2);
        assert!(
            !tab.refused()
                .iter()
                .any(|url| url.contains("example.invalid")),
            "never asked for, so never refused: {:?}",
            tab.refused()
        );
    }
    // And the tab after it is not held: it opens and is read.
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/opens",
        OPENS.as_bytes().to_vec(),
        SIZE,
    )
    .map_err(|why| format!("{why:?}"))?;
    assert!(
        tab.read()
            .map_err(|why| format!("{why:?}"))?
            .nodes()
            .iter()
            .any(|node| node.to_string() == r#"link "Away""#)
    );
    assert_eq!(tab.closed(), 0);
    Ok(())
}

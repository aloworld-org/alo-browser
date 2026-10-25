/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A date put in through Chrome: queue item 632, ADR 0042 § 4, ADR 0058 § 4.
//!
//! The same form — an empty date field, one holding a date, and one emptied
//! — is filled with the same verbs through this browser's own engine and
//! through the machine's Chrome, and sent; both send the very same dates,
//! which is the date each field holds read back by the form itself. The
//! engine says what a form sends beside where it goes, and Chrome in the
//! address it goes to, so each is read where its renderer says it. A page
//! of the test's own then shows the rest: a date Chrome's field would not
//! hold as written is not typed at all, a page that cancels the keys is
//! answered as having cancelled them, and what a page hears is a person's
//! keys, never script.

use alo_agent::Outcome;
use alo_agent::Refusal;
use alo_agent::verb::{Target, Verb};
use alo_dom::submission::Value;
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_date::Untypable;
use alo_window::chrome_tab::Tab;

/// The size the pages are laid out at.
const SIZE: (u32, u32) = (600, 400);

/// Where the pages say they are.
const AT: &str = "https://example.test/dates";

/// A form of three date fields: one empty, one holding a leap day, and one
/// holding a date that is to be taken away.
const FORM: &str = r#"<!doctype html>
<html lang=en><title>Dates</title>
<form action="/sent">
<label>Day <input name=day type=date></label>
<label>Moved <input name=moved type=date value=2024-02-29></label>
<label>Gone <input name=gone type=date value=2020-01-01></label>
<button>Send</button>
</form>
"#;

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
}

fn put(text: &str) -> Verb {
    Verb::PutText(text.to_owned())
}

/// The form filled and sent, verb by verb.
fn the_dates() -> Vec<(Target, Verb)> {
    vec![
        (named("Day"), put("2026-10-12")),
        (named("Moved"), put("0005-01-02")),
        (named("Gone"), put("")),
        (named("Send"), Verb::Activate),
    ]
}

/// What a verb came to, without the node it came to it on: the engine's ids
/// are its boxes and Chrome's are its DOM nodes.
fn came_to(outcome: &Outcome) -> String {
    match outcome {
        Outcome::TextPut { text, .. } => format!("put {text:?}"),
        Outcome::TextCanceled { text, .. } => format!("cancelled {text:?}"),
        Outcome::Submitted { method, to, .. } => format!("{} {to}", method.name()),
        other => format!("{other}"),
    }
}

/// `FORM` filled and sent through this browser's own engine: what each verb
/// came to, and the entries the form sent, as `name=value`.
fn through_ours() -> Result<(Vec<String>, Vec<String>), String> {
    let address = alo_url::parse(AT).map_err(|why| format!("{why:?}"))?;
    let mut renderer = Renderer::new(alo_corpus::corpus_fonts());
    let page = Page::new(FORM.to_owned(), Size::new(600.0, 400.0)).at(address);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the form did not load: {other:?}")),
    }
    let mut came = Vec::new();
    let mut entries = Vec::new();
    for (target, verb) in the_dates() {
        match renderer.handle(ToRenderer::Act { target, verb }) {
            FromRenderer::Acted {
                outcome,
                navigation,
                ..
            } => {
                came.push(came_to(&outcome));
                let sent = navigation.and_then(|asked| asked.submission);
                for entry in sent.map(|sent| sent.entries).unwrap_or_default() {
                    let Value::Text(value) = entry.value else {
                        return Err(format!("{} is not text", entry.name));
                    };
                    entries.push(format!("{}={value}", entry.name));
                }
            }
            other => return Err(format!("not acted: {other:?}")),
        }
    }
    Ok((came, entries))
}

fn opened<'c>(chrome: &'c mut Chrome, page: &str) -> Result<Tab<'c>, String> {
    Tab::open_frozen(chrome, AT, page.as_bytes().to_vec(), SIZE).map_err(|why| why.to_string())
}

#[test]
fn a_date_put_in_through_chrome_is_sent_as_the_engine_sends_it() {
    let (ours, entries) = through_ours().unwrap();
    let put = [r#"put "2026-10-12""#, r#"put "0005-01-02""#, r#"put """#];
    assert_eq!(ours[..3], put);
    assert_eq!(ours[3], "GET https://example.test/sent");
    let sent = ["day=2026-10-12", "moved=0005-01-02", "gone="];
    assert_eq!(entries, sent);

    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, FORM).unwrap();
    let theirs: Vec<String> = the_dates()
        .iter()
        .map(|(target, verb)| match tab.act(target, verb) {
            Ok(outcome) => came_to(&outcome),
            Err(why) => format!("not done: {why}"),
        })
        .collect();
    assert_eq!(theirs[..3], put, "the same verbs, the same outcomes");
    // What the form sent is what each field held, read by Chrome's own
    // form: the dates, in their own form, whatever order the machine's
    // region lays the segments out in — the engine's entries exactly.
    let query = theirs[3].strip_prefix("GET https://example.test/sent?");
    let theirs: Vec<&str> = query
        .map(|query| query.split('&').collect())
        .unwrap_or_default();
    assert_eq!(theirs, sent, "{query:?}");
}

/// A page of the test's own: a field it lets alone, a field whose keys it
/// cancels, and a line that writes down every key and event it hears —
/// whether each was a person's (`isTrusted`) or a script's — and what the
/// field holds at each `change`.
const HEARD: &str = r#"<!doctype html>
<html lang=en><title>Heard</title>
<p id=heard>Heard:</p>
<label>Far <input type=date></label>
<label>Kept <input type=date value=2024-02-29 id=kept></label>
<script>
const heard = document.getElementById("heard");
for (const type of ["keydown", "beforeinput", "input", "change"]) {
  document.addEventListener(type, (event) => {
    heard.textContent += " " + type + (event.isTrusted ? "" : "(by script)")
      + (type === "change" ? "=" + event.target.value : "");
  }, true);
}
document.getElementById("kept").addEventListener("keydown", (event) => event.preventDefault());
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
fn a_date_chrome_would_not_hold_is_not_typed_and_a_cancelled_one_is_said() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, HEARD).unwrap();
    // Valid date strings Chrome's field would not hold as written: no key
    // is sent for either.
    let untypable = |tab: &mut Tab<'_>, text: &str| match tab.act(&named("Far"), &put(text)) {
        Err(Unacted::Untypable { why, .. }) => Ok(why),
        other => Err(format!("{text}: {other:?}")),
    };
    assert_eq!(
        untypable(&mut tab, "275760-09-14"),
        Ok(Untypable::PastTheLastDay {
            text: "275760-09-14".to_owned()
        })
    );
    assert_eq!(
        untypable(&mut tab, "02026-01-02"),
        Ok(Untypable::LeadingZeros {
            text: "02026-01-02".to_owned()
        })
    );
    // A text that is not a date is the agent's to correct, refused before
    // anything is asked of Chrome.
    assert!(matches!(
        tab.act(&named("Far"), &put("12/10/2026")),
        Err(Unacted::Refused(Refusal::NotADate { .. }))
    ));
    assert_eq!(heard(&mut tab).unwrap(), "Heard:");

    // A page that cancels every key leaves the field as it was, and says so.
    let kept = tab.act(&named("Kept"), &put("2026-10-12")).unwrap();
    assert!(
        matches!(&kept, Outcome::TextCanceled { text, .. } if text == "2026-10-12"),
        "{kept:?}"
    );
    let cancelled = heard(&mut tab).unwrap();
    assert!(!cancelled.contains("change"), "{cancelled}");

    // Put in, a person's keys: each a key the page hears, then an `input`
    // and a `change` as the field's value changes — the last of them the
    // date — and nothing a script made.
    let far = tab.act(&named("Far"), &put("2026-10-12")).unwrap();
    assert!(
        matches!(&far, Outcome::TextPut { text, .. } if text == "2026-10-12"),
        "{far:?}"
    );
    let all = heard(&mut tab).unwrap();
    let put_in = all.strip_prefix(&cancelled).unwrap();
    assert!(!put_in.contains("(by script)"), "{put_in}");
    assert_eq!(put_in.matches("keydown").count(), 8, "{put_in}");
    assert!(put_in.ends_with(" change=2026-10-12"), "{put_in}");

    // And taken away again: the field is empty.
    let gone = tab.act(&named("Far"), &put("")).unwrap();
    assert!(matches!(&gone, Outcome::TextPut { text, .. } if text.is_empty()));
    // Chrome empties the field at the first segment emptied, and says so
    // once; the others' keys change nothing more.
    let all = heard(&mut tab).unwrap();
    let cleared = all.rsplit_once(" change").map(|(_, last)| last);
    assert!(
        cleared.is_some_and(|last| last == "=" || last.starts_with("= ")),
        "{all}"
    );

    // The tree reads the date field as a date, as the engine's does.
    let read = tab.read().unwrap();
    let field = read
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some("Far"))
        .map(|node| node.role.to_string());
    assert_eq!(field.as_deref(), Some("date"));
}

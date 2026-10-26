/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page's script said while a verb ran, through Chrome: queue item
//! 639, `docs/contracts/agent-socket.md`.
//!
//! The same page, pressed and typed into, says the same throws through the
//! machine's Chrome as through this engine, in the engine's words, each
//! placed where its renderer places it — Chrome by the page's address and
//! the line in the page, the engine by `script 1` and the line in the
//! script. A console is Chrome's alone: there the call is said, where the
//! engine says the console is not defined. Nothing said before a verb or
//! between two is heard, and something too long for one of Chrome's
//! messages is said to be so while the tab goes on answering.

use alo_agent::verb::{Target, Verb};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_window::chrome::Chrome;
use alo_window::chrome_pages::{ChromePages, TheTab};
use alo_window::chrome_tab::Tab;
use alo_window::chrome_wire::MOST_MESSAGE;
use alo_window::pages::{Acted, Pages};

/// The size the pages are laid out at.
const SIZE: (u32, u32) = (600, 400);

/// Where the pages say they are.
const AT: &str = "https://example.test/said";

/// A button whose click throws, and a field whose `input` throws.
const THROWS: &str = "<!doctype html><html lang=en><title>Throws</title><body>\
<button>Go</button><label>Name <input name=who></label><script>
const go = document.body.firstChild;
go.addEventListener('click', () => { throw new Error('the click threw'); });
go.nextSibling.lastChild.addEventListener('input', () => { throw new TypeError('the field threw'); });
</script>";

/// A button whose click writes to the console.
const LOGS: &str = "<!doctype html><html lang=en><title>Logs</title><body>\
<button>Log</button><script>
document.body.firstChild.addEventListener('click', () => { console.log('clicked', 4); console.warn('careful'); });
</script>";

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
}

/// `verb` done to `name` on `page` through this browser's own engine, and
/// what the page's script said.
fn through_ours(page: &str, name: &str, verb: Verb) -> Result<Vec<String>, String> {
    let address = alo_url::parse(AT).map_err(|why| format!("{why:?}"))?;
    let mut renderer = Renderer::new(alo_corpus::corpus_fonts());
    let page = Page::new(page.to_owned(), Size::new(600.0, 400.0)).at(address);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the page did not load: {other:?}")),
    }
    match renderer.handle(ToRenderer::Act {
        target: named(name),
        verb,
    }) {
        FromRenderer::Acted { issues, .. } => Ok(issues),
        other => Err(format!("{name}: {other:?}")),
    }
}

fn opened<'c>(chrome: &'c mut Chrome, page: &str) -> Result<Tab<'c>, String> {
    Tab::open_frozen(chrome, AT, page.as_bytes().to_vec(), SIZE).map_err(|why| why.to_string())
}

/// `verb` done to `name` in `tab`, and what the page's script said.
fn heard(tab: &mut Tab<'_>, name: &str, verb: &Verb) -> Result<Vec<String>, String> {
    tab.act_and_hear(&named(name), verb)
        .map(|(_, issues)| issues)
        .map_err(|why| format!("{name}: {why}"))
}

/// Each line split into what was said and where, at its ` (at `.
fn split(lines: &[String]) -> Vec<(&str, &str)> {
    lines
        .iter()
        .map(|line| line.split_once(" (at ").unwrap_or((line, "")))
        .collect()
}

#[test]
fn a_throw_through_chrome_is_said_as_the_engine_says_it_placed_by_the_page() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, THROWS).unwrap();
    for (name, verb, said) in [
        (
            "Go",
            Verb::Activate,
            "the click: uncaught: Error: the click threw",
        ),
        (
            "Name",
            Verb::PutText("Ada".to_owned()),
            "the text: uncaught: TypeError: the field threw",
        ),
    ] {
        let ours = through_ours(THROWS, name, verb.clone()).unwrap();
        let chromes = heard(&mut tab, name, &verb).unwrap();
        let (ours, chromes) = (split(&ours), split(&chromes));
        assert_eq!(ours.len(), 1, "{ours:?}");
        assert_eq!(chromes.len(), 1, "{chromes:?}");
        assert_eq!(ours.first().map(|line| line.0), Some(said));
        assert_eq!(chromes.first().map(|line| line.0), Some(said));
        // The engine places it in the script; Chrome in the page.
        assert!(
            ours.first()
                .is_some_and(|line| line.1.starts_with("script 1, line ")),
            "{ours:?}"
        );
        assert!(
            chromes
                .first()
                .is_some_and(|line| line.1.starts_with(&format!("{AT}, line "))),
            "{chromes:?}"
        );
    }
}

#[test]
fn a_console_through_chrome_is_said_where_the_engine_has_none() {
    let ours = through_ours(LOGS, "Log", Verb::Activate).unwrap();
    let ours = split(&ours);
    assert_eq!(
        ours.iter().map(|line| line.0).collect::<Vec<_>>(),
        ["the click: uncaught: ReferenceError: 'console' is not defined"]
    );

    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, LOGS).unwrap();
    let chromes = heard(&mut tab, "Log", &Verb::Activate).unwrap();
    assert_eq!(
        chromes,
        [
            format!("the click: console.log: clicked 4 (at {AT}, line 2, column 68)"),
            format!("the click: console.warn: careful (at {AT}, line 2, column 95)"),
        ]
    );
}

#[test]
fn only_what_was_said_while_a_verb_ran_is_heard() {
    let page = "<!doctype html><html lang=en><title>When</title><body>\
<button>Later</button><button>Now</button><script>
console.log('loaded');
const later = document.body.firstChild;
later.addEventListener('click', () => setTimeout(() => console.log('after the click'), 2000));
later.nextSibling.addEventListener('click', () => console.log('now'));
</script>";
    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, page).unwrap();
    assert_eq!(
        heard(&mut tab, "Later", &Verb::Activate).unwrap(),
        Vec::<String>::new(),
        "what the page said as it loaded is not the verb's"
    );
    // The page says "after the click" two seconds after it, and this waits
    // four, so it is said between the two verbs. Both are instruments, not
    // the claim: the claim is that what is said after a verb has been
    // answered is not that verb's. They were 100 ms and 400 ms until
    // 2026-10-26, when the full gate counted "after the click" as the click's
    // own — a click there took longer than 100 ms to be answered, so the
    // page spoke while it was still running and was heard, correctly by the
    // rule and wrongly by the test. The test measured the machine. Two
    // seconds is far longer than any verb takes here.
    std::thread::sleep(std::time::Duration::from_secs(4));
    assert_eq!(
        split(&heard(&mut tab, "Now", &Verb::Activate).unwrap())
            .into_iter()
            .map(|line| line.0)
            .collect::<Vec<_>>(),
        ["the click: console.log: now"],
        "what it said between the two verbs is neither's"
    );
    // A verb refused before it was carried out hears nothing, and leaves
    // nothing to be heard by the next.
    assert!(
        tab.act_and_hear(&named("Nowhere"), &Verb::Activate)
            .is_err()
    );
    assert_eq!(
        split(&heard(&mut tab, "Now", &Verb::Activate).unwrap()).len(),
        1
    );
}

#[test]
fn something_too_long_to_read_is_said_to_be_and_the_tab_goes_on() {
    // Twenty million characters, which Chrome sends whole: a message past
    // the longest this browser reads.
    let page = "<!doctype html><html lang=en><title>Length</title><body>\
<button>Long</button><button>Short</button><script>
const long = document.body.firstChild;
long.addEventListener('click', () => { console.log('x'.repeat(20000000)); throw new Error('after'); });
long.nextSibling.addEventListener('click', () => console.log('short'));
</script>";
    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, page).unwrap();
    let said = heard(&mut tab, "Long", &Verb::Activate).unwrap();
    assert_eq!(
        split(&said),
        [
            (
                format!(
                    "the click: the page's script said something longer than one message \
                     from Chrome may be ({MOST_MESSAGE} bytes), and it was not read"
                )
                .as_str(),
                ""
            ),
            (
                "the click: uncaught: Error: after",
                &*format!("{AT}, line 3, column 81)")
            ),
        ]
    );
    assert_eq!(
        split(&heard(&mut tab, "Short", &Verb::Activate).unwrap())
            .into_iter()
            .map(|line| line.0)
            .collect::<Vec<_>>(),
        ["the click: console.log: short"]
    );
}

#[test]
fn an_agent_is_answered_with_what_was_heard() {
    let mut chrome = Chrome::start().unwrap();
    let product = chrome.version().unwrap().product;
    let tab = opened(&mut chrome, THROWS).unwrap();
    let mut pages = ChromePages::of(tab, product);
    let Ok((_, Acted::Done { issues, .. })) = pages.act(TheTab, &named("Go"), &Verb::Activate)
    else {
        panic!("not done");
    };
    assert_eq!(
        split(&issues)
            .into_iter()
            .map(|line| line.0)
            .collect::<Vec<_>>(),
        ["the click: uncaught: Error: the click threw"]
    );
}

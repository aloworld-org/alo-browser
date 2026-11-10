/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 920, ADR 0108 §§ 2–4: a card's page's cookie banner is
//! answered with its most private choice, found by name in the tree Chrome
//! gives and pressed by the agent's own verb — and never a consent. Frozen
//! banners of four common consent platforms, taken from their own sites on
//! 10 November 2026 (`tests/banners/SOURCES.txt`), through the machine's
//! Chrome and the Chromium alo carries (its CEF host), headless:
//!
//! - three offering a refusal — *Accept only necessary*, *Deny*, *Reject
//!   Optional* — each refused, and nothing else pressed;
//! - one offering only *AGREE* and *MORE OPTIONS*, left to the person,
//!   nothing pressed;
//! - a page with no banner, whose own *Accept* and *Decline* answer an
//!   invitation: untouched;
//! - *Leave them to me*: nothing looked at, nothing pressed.
//!
//! Each page holds a few lines of the test's own, which say which control
//! was pressed and take the banner away, as a platform's script would.
//! Nothing here touches the network.

#[cfg(target_os = "macos")]
mod carried;

use alo_renderer::settings_privacy::Banners;
use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use alo_window::consent_answer::{Answered, Answering};
use alo_window::consent_banner::{self, Choice};
use std::time::{Duration, Instant};

/// The size every page is laid out at: the card's page box.
const SIZE: (u32, u32) = (878, 520);

/// How long a page's banner is waited for to be answered or left.
const AT_MOST: Duration = Duration::from_secs(20);

/// Each frozen page, what alo is to do with its banner, and what the page
/// then says was pressed — `None`, nothing.
const PAGES: &[(&str, &str, Expected, Option<&str>)] = &[
    (
        "onetrust",
        include_str!("banners/onetrust.html"),
        Expected::Refused("Accept only necessary"),
        Some("Pressed onetrust-reject-all-handler"),
    ),
    (
        "cookiebot",
        include_str!("banners/cookiebot.html"),
        Expected::Refused("Deny"),
        Some("Pressed CybotCookiebotDialogBodyButtonDecline"),
    ),
    (
        "trustarc",
        include_str!("banners/trustarc.html"),
        Expected::Refused("Reject Optional"),
        Some("Pressed truste-consent-required"),
    ),
    (
        "quantcast",
        include_str!("banners/quantcast.html"),
        Expected::Left("AGREE"),
        None,
    ),
    (
        "no-banner",
        include_str!("banners/no-banner.html"),
        Expected::Nothing,
        None,
    ),
];

/// What alo is to do with a page's banner.
#[derive(Debug, Clone, Copy)]
enum Expected {
    /// Press its refusal of this name.
    Refused(&'static str),
    /// Leave it, it offering this consent alone.
    Left(&'static str),
    /// Find none.
    Nothing,
}

/// What the page's own status line says was pressed, if anything.
fn pressed(tab: &mut Tab<'_>) -> Result<Option<String>, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    Ok(read
        .nodes()
        .into_iter()
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with("Pressed "))
        .map(|name| name.trim().to_owned()))
}

/// `page`, frozen as `name`, opened in `chrome` and answered under
/// `banners` until it is answered, left or looked at enough.
fn answered(
    chrome: &mut Chrome,
    name: &str,
    page: &str,
    banners: Banners,
) -> Result<(Option<Answered>, Option<String>, String), String> {
    let address = format!("https://{name}.example.test/story");
    let mut tab = Tab::open_frozen(chrome, &address, page.as_bytes().to_vec(), SIZE)
        .map_err(|why| format!("{name}: {why:?}"))?;
    let outline = tab.read().map_err(|why| why.to_string())?.to_outline();
    let mut answering = Answering::default();
    let until = Instant::now() + AT_MOST;
    let mut answer = None;
    while Instant::now() < until {
        answer = answering.pass(&mut tab, banners);
        if answer.is_some() || answering.is_done() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(200));
    Ok((answer, pressed(&mut tab)?, outline))
}

/// Every frozen page through `chromium`.
fn every_page(chromium: &Chromium) -> Result<(), String> {
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    for (name, page, expected, said) in PAGES {
        let (answer, pressed, outline) = answered(&mut chrome, name, page, Banners::Answer)?;
        let fits = match (expected, &answer) {
            (Expected::Refused(refusal), Some(Answered::Refused { name })) => name == refusal,
            (Expected::Left(consent), Some(Answered::LeftToYou { offered })) => offered == consent,
            (Expected::Nothing, None) => true,
            _ => false,
        };
        if !fits {
            return Err(format!(
                "{name}: expected {expected:?}, came to {answer:?}\n{outline}"
            ));
        }
        if pressed.as_deref() != *said {
            return Err(format!(
                "{name}: the page says {pressed:?}, not {said:?}\n{outline}"
            ));
        }
        // What was pressed, if anything, was never a consent.
        if let Some(Answered::Refused { name: refusal }) = &answer
            && consent_banner::choice(refusal) != Choice::Refusal
        {
            return Err(format!("{name}: pressed {refusal:?}, which is no refusal"));
        }
    }
    // *Leave them to me*: a banner with a refusal is not touched.
    let (name, page, _, _) = PAGES.first().ok_or("no page")?;
    let (answer, pressed, _) = answered(&mut chrome, name, page, Banners::Leave)?;
    if answer.is_some() || pressed.is_some() {
        return Err(format!("left to the person, yet {answer:?}, {pressed:?}"));
    }
    Ok(())
}

#[test]
fn the_machines_chrome_answers_each_banner_with_its_refusal_and_never_a_consent() {
    let result = every_page(&Chromium::Machines);
    assert!(result.is_ok(), "{}", result.err().unwrap_or_default());
}

#[cfg(target_os = "macos")]
#[test]
fn alos_own_chromium_answers_each_banner_with_its_refusal_and_never_a_consent() {
    let host = match carried::host("920") {
        Ok(Some(host)) => host,
        Ok(None) => return,
        Err(why) => panic!("{why}"),
    };
    let result = every_page(&host.chromium());
    assert!(result.is_ok(), "{}", result.err().unwrap_or_default());
}

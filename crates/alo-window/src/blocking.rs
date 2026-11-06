/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Trackers and ads, refused by the lists alo carries (ADR 0094, queue item
//! 736): the one file that names the rented matcher, `adblock` — Brave's,
//! the engine of its Shields.
//!
//! The lists are `EasyList` and `EasyPrivacy`, as published, pinned in the
//! repository beside this crate (`lists/`, with their licence and where
//! they came from) and compiled into the program: nothing is fetched to
//! decide a request, by a page's load or by anything else. They are read
//! once per process, on the first request asked of them, into one matcher
//! shared by every page ([`Blocker::carried`]); each rule keeps its line so
//! a refusal names it.
//!
//! [`crate::chrome_tab`] asks [`Blocker::judge`] of every request a page
//! from the network makes in Chrome, at the place ADR 0060 holds and
//! decides each one, after the browser process's own rules have let it go
//! ([`crate::chrome_gate`]). What it names is refused before it leaves —
//! `Fetch.failRequest`, as every refusal there is — and written in the
//! record with the list, line and rule that refused it. The tab's own
//! document is never judged here: a person, or an agent, chose to go there.
//!
//! # Hiding what is left of an advert
//!
//! Queue item 806, ADR 0094's amendment of 6 November 2026: the lists'
//! element-hiding rules are read too, and [`Blocker::hiding`] answers, for
//! a document's address, the one style sheet that hides what they name in
//! it — every generic rule (`##.ad-banner`), less those the lists except
//! anywhere or on that site, and the site's own rules; only the site's own
//! where the lists say `$generichide` for it. Each selector is a rule of its
//! own, so a selector Chromium cannot read loses that rule alone. What puts
//! the sheet into the document is [`crate::chrome_cosmetic`]'s; no script.
//! Procedural rules and scriptlets are never read into a sheet.

use adblock::Engine;
use adblock::lists::{FilterSet, ParseOptions, RuleTypes};
use adblock::request::Request;
use core::fmt;
use std::sync::OnceLock;

/// A list of rules, by the name a refusal says it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct List {
    /// What the list calls itself.
    pub name: &'static str,
    /// Its rules, in Adblock Plus's syntax, as published.
    pub text: &'static str,
}

/// `EasyList`: adverts.
pub const EASYLIST: List = List {
    name: "EasyList",
    text: include_str!("../lists/easylist.txt"),
};

/// `EasyPrivacy`: trackers.
pub const EASYPRIVACY: List = List {
    name: "EasyPrivacy",
    text: include_str!("../lists/easyprivacy.txt"),
};

/// The lists alo carries, in the order they are read.
pub const CARRIED: [List; 2] = [EASYLIST, EASYPRIVACY];

/// The lists' licence and attribution, by the name the application carries
/// it under beside its other licences (`Contents/Resources/licences`).
pub const LICENCES: [(&str, &str); 1] = [(
    "EasyList-and-EasyPrivacy.txt",
    include_str!("../lists/LICENCE.txt"),
)];

/// A request the lists name: which list, which line of it, and the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocked {
    /// The list holding the rule, or [`None`] when the matcher fused rules
    /// and cannot say which.
    pub list: Option<&'static str>,
    /// The rule's line in its list, counted from one, when it can say.
    pub line: Option<u32>,
    /// The rule as the list writes it, when it can say.
    pub rule: Option<String>,
}

impl fmt::Display for Blocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a tracker or an advert by ")?;
        match (self.list, self.line) {
            (Some(list), Some(line)) => write!(f, "{list}, line {line}")?,
            (Some(list), None) => write!(f, "{list}")?,
            _ => write!(f, "EasyList or EasyPrivacy")?,
        }
        match &self.rule {
            Some(rule) => write!(f, ": {rule}"),
            None => Ok(()),
        }
    }
}

/// Where a request was asked from, and what for: what the lists are asked
/// with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked<'a> {
    /// The address asked for, as Chrome spells it.
    pub url: &'a str,
    /// The address of the page the tab shows: the side a rule's
    /// `third-party` and `domain=` are read against.
    pub page: &'a str,
    /// What kind of thing it is, in Chrome's word (`Script`, `Image`,
    /// `XHR`, `Document`, …).
    pub kind: &'a str,
    /// Whether it is for a frame inside the page rather than the tab's own.
    pub in_a_frame: bool,
    /// Its method, as Chrome spells it.
    pub method: &'a str,
}

/// The rules of some lists, read into Brave's matcher.
pub struct Blocker {
    engine: Engine,
    /// The lists' names, in the order their rules were read: the matcher
    /// names a rule's list by this place.
    names: Vec<&'static str>,
    /// Every generic hiding rule's selector, in the lists' order, less
    /// those a generic exception (`#@#`) excuses everywhere.
    generic: Vec<&'static str>,
    /// The same, to ask whether a selector is one.
    generic_set: std::collections::HashSet<&'static str>,
}

impl Blocker {
    /// The lists alo carries, read once for the whole process, on the first
    /// ask.
    pub fn carried() -> &'static Blocker {
        static CARRIED_ONCE: OnceLock<Blocker> = OnceLock::new();
        CARRIED_ONCE.get_or_init(|| Blocker::of(&CARRIED))
    }

    /// `lists`' rules, read: each network rule keeps its text and line,
    /// and the hiding rules are read for [`Blocker::hiding`].
    pub fn of(lists: &[List]) -> Self {
        // Each rule's text kept, so a refusal names it (ADR 0094 § 4).
        let mut set = FilterSet::new(true);
        for list in lists {
            set.add_filter_list(
                list.text.to_owned(),
                ParseOptions {
                    rule_types: RuleTypes::All,
                    ..ParseOptions::default()
                },
            );
        }
        let generic = generic_hiding(lists);
        Self {
            engine: Engine::new_with_filter_set(set),
            names: lists.iter().map(|list| list.name).collect(),
            generic_set: generic.iter().copied().collect(),
            generic,
        }
    }

    /// The style sheet hiding what the lists' element-hiding rules name in
    /// a document at `address` (queue item 806), or [`None`] when they
    /// name nothing there: each selector a rule `{display:none!important}`
    /// of its own, generic ones first in the lists' order, then the site's
    /// own, sorted.
    pub fn hiding(&self, address: &str) -> Option<String> {
        let resources = self.engine.url_cosmetic_resources(address);
        let mut own: Vec<&str> = resources
            .hide_selectors
            .iter()
            .map(String::as_str)
            .filter(|selector| !self.generic_set.contains(selector))
            .collect();
        own.sort_unstable();
        let generic = self
            .generic
            .iter()
            .copied()
            .filter(|_| !resources.generichide)
            .filter(|selector| !resources.exceptions.contains(*selector));
        let sheet: String = generic
            .chain(own)
            .filter(|selector| is_a_selector(selector))
            .flat_map(|selector| [selector, "{display:none!important}\n"])
            .collect();
        (!sheet.is_empty()).then_some(sheet)
    }

    /// Whether the lists name `asked`, and by which rule: [`None`] for a
    /// request they let be — every request of the tab's own document, and
    /// every one the matcher cannot read as an address.
    pub fn judge(&self, asked: &Asked<'_>) -> Option<Blocked> {
        let kind = kind_of(asked.kind, asked.in_a_frame)?;
        let request = Request::new(asked.url, asked.page, kind, asked.method).ok()?;
        let result = self.engine.check_network_request(&request);
        if !result.should_block() {
            return None;
        }
        let matched = result.filter.unwrap_or_default();
        let place = matched.source_location;
        Some(Blocked {
            list: place.as_ref().and_then(|place| {
                usize::try_from(place.source_index)
                    .ok()
                    .and_then(|index| self.names.get(index).copied())
            }),
            line: place.and_then(|place| place.line_number.checked_add(1)),
            rule: matched.raw_line,
        })
    }
}

/// Every generic hiding rule of `lists` — `##` and a selector, naming no
/// site — less every one a generic exception (`#@#` and the same selector)
/// excuses, in the lists' order.
fn generic_hiding(lists: &[List]) -> Vec<&'static str> {
    let lines = || lists.iter().flat_map(|list| list.text.lines());
    let excused: std::collections::HashSet<&str> = lines()
        .filter_map(|line| line.strip_prefix("#@#"))
        .collect();
    let mut seen = std::collections::HashSet::new();
    lines()
        .filter_map(|line| line.strip_prefix("##"))
        .filter(|selector| !excused.contains(selector) && seen.insert(*selector))
        .collect()
}

/// Whether `selector` can stand before a declaration block as a rule of
/// its own: never one that could close the block or open another.
fn is_a_selector(selector: &str) -> bool {
    !selector.is_empty() && !selector.contains(['{', '}']) && !selector.contains("/*")
}

/// What the matcher calls a request of Chrome's `kind` (`resourceType`),
/// for a frame inside the page or the tab's own — or [`None`] for the tab's
/// own document, which the lists never judge.
fn kind_of(kind: &str, in_a_frame: bool) -> Option<&'static str> {
    Some(match kind {
        "Document" if in_a_frame => "sub_frame",
        "Document" => return None,
        "Stylesheet" => "stylesheet",
        "Image" => "image",
        "Media" => "media",
        "Font" => "font",
        "Script" => "script",
        // A `fetch` is matched as Adblock Plus's lists mean
        // `xmlhttprequest`, as uBlock Origin and Brave match it.
        "XHR" | "Fetch" => "xhr",
        "Ping" => "ping",
        "CSPViolationReport" => "csp_report",
        "WebSocket" => "websocket",
        _ => "other",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "https://news.example/today";

    fn asked<'a>(url: &'a str, kind: &'a str) -> Asked<'a> {
        Asked {
            url,
            page: PAGE,
            kind,
            in_a_frame: false,
            method: "GET",
        }
    }

    fn small() -> Blocker {
        Blocker::of(&[
            List {
                name: "First",
                text: "! a comment\n||tracker.example^\n/banner.$image\n",
            },
            List {
                name: "Second",
                text: "! two lines of comment\n!\n||ads.example^$third-party\n\
                       @@||ads.example/allowed^\n||frames.example^$subdocument\n",
            },
        ])
    }

    #[test]
    fn a_request_a_list_names_is_blocked_by_its_list_line_and_rule() {
        let blocker = small();
        assert_eq!(
            blocker.judge(&asked("https://tracker.example/t.js", "Script")),
            Some(Blocked {
                list: Some("First"),
                line: Some(2),
                rule: Some("||tracker.example^".to_owned()),
            })
        );
        let banner = blocker
            .judge(&asked("https://cdn.example/banner.png", "Image"))
            .unwrap();
        assert_eq!(
            banner.to_string(),
            "a tracker or an advert by First, line 3: /banner.$image"
        );
        assert_eq!(
            blocker.judge(&asked("https://ads.example/a.js", "Script")),
            Some(Blocked {
                list: Some("Second"),
                line: Some(3),
                rule: Some("||ads.example^$third-party".to_owned()),
            })
        );
    }

    #[test]
    fn what_the_lists_do_not_name_or_excuse_is_let_be() {
        let blocker = small();
        // Named by no rule.
        assert_eq!(
            blocker.judge(&asked("https://news.example/story.js", "Script")),
            None
        );
        // A rule for pictures, asked of a script.
        assert_eq!(
            blocker.judge(&asked("https://cdn.example/banner.js", "Script")),
            None
        );
        // An exception.
        assert_eq!(
            blocker.judge(&asked("https://ads.example/allowed/x.js", "Script")),
            None
        );
        // `third-party`, asked by the site itself.
        let own = Asked {
            page: "https://ads.example/",
            ..asked("https://ads.example/a.js", "Script")
        };
        assert_eq!(blocker.judge(&own), None);
        // Not an address.
        assert_eq!(blocker.judge(&asked("not an address", "Script")), None);
    }

    #[test]
    fn the_tabs_own_document_is_never_judged_and_a_frames_is() {
        let blocker = small();
        assert_eq!(
            blocker.judge(&asked("https://tracker.example/", "Document")),
            None,
            "the page the tab goes to was chosen by a person or an agent"
        );
        let framed = Asked {
            in_a_frame: true,
            ..asked("https://frames.example/ad.html", "Document")
        };
        assert_eq!(
            blocker.judge(&framed).and_then(|blocked| blocked.line),
            Some(5)
        );
        assert_eq!(
            blocker.judge(&asked("https://frames.example/ad.js", "Script")),
            None,
            "`subdocument` is a frame's document only"
        );
    }

    #[test]
    fn chromes_kinds_are_the_matchers() {
        for (chrome, matcher) in [
            ("Stylesheet", "stylesheet"),
            ("Image", "image"),
            ("Media", "media"),
            ("Font", "font"),
            ("Script", "script"),
            ("XHR", "xhr"),
            ("Fetch", "xhr"),
            ("Ping", "ping"),
            ("CSPViolationReport", "csp_report"),
            ("WebSocket", "websocket"),
            ("EventSource", "other"),
            ("Prefetch", "other"),
            ("Other", "other"),
        ] {
            assert_eq!(kind_of(chrome, false), Some(matcher), "{chrome}");
        }
        assert_eq!(kind_of("Document", true), Some("sub_frame"));
        assert_eq!(kind_of("Document", false), None);
    }

    #[test]
    fn the_carried_lists_block_known_trackers_and_adverts_and_not_a_page() {
        let blocker = Blocker::carried();
        let script = |url: &'static str| asked(url, "Script");
        let analytics = blocker
            .judge(&script("https://www.google-analytics.com/analytics.js"))
            .unwrap();
        assert_eq!(analytics.list, Some("EasyPrivacy"), "{analytics}");
        let ads = blocker
            .judge(&script(
                "https://securepubads.g.doubleclick.net/tag/js/gpt.js",
            ))
            .unwrap();
        assert_eq!(ads.list, Some("EasyList"), "{ads}");
        // The line it names holds the rule it names.
        for blocked in [&analytics, &ads] {
            let list = CARRIED
                .iter()
                .find(|list| Some(list.name) == blocked.list)
                .unwrap();
            let line = blocked.line.and_then(|line| usize::try_from(line).ok());
            assert_eq!(
                line.and_then(|line| list.text.lines().nth(line - 1)),
                blocked.rule.as_deref(),
                "{blocked}"
            );
        }
        assert_eq!(blocker.judge(&script("https://news.example/app.js")), None);
        assert_eq!(
            blocker.judge(&asked(
                "https://upload.wikimedia.org/wikipedia/commons/cat.jpg",
                "Image"
            )),
            None
        );
    }

    fn hiding() -> Blocker {
        Blocker::of(&[List {
            name: "Hiding",
            text: "! hiding rules\n##.ad-banner\n###top-ad\n##.fine\n#@#.fine\n\
                   news.example##.local-ad\nnews.example#@#.ad-banner\n\
                   ##div{color:red}\n##.ad-banner\n@@||quiet.example^$generichide\n\
                   quiet.example##.its-own\n",
        }])
    }

    #[test]
    fn a_sheet_hides_every_generic_rule_less_the_excused_and_the_sites_own() {
        let blocker = hiding();
        assert_eq!(
            blocker.hiding("https://elsewhere.example/page").as_deref(),
            Some(".ad-banner{display:none!important}\n#top-ad{display:none!important}\n"),
            "an exception everywhere is no rule, a selector closing the block is \
             never one, and a rule twice is once"
        );
        assert_eq!(
            blocker.hiding("https://www.news.example/today").as_deref(),
            Some("#top-ad{display:none!important}\n.local-ad{display:none!important}\n"),
            "excused on the site, and the site's own after the generic"
        );
        assert_eq!(
            blocker.hiding("https://quiet.example/").as_deref(),
            Some(".its-own{display:none!important}\n"),
            "`$generichide`: the site's own alone"
        );
        assert_eq!(small().hiding("https://elsewhere.example/"), None);
        assert_eq!(
            blocker.hiding("not an address"),
            blocker.hiding("https://elsewhere.example/"),
            "no site's own: the generic alone"
        );
    }

    #[test]
    fn the_carried_lists_hide_an_ad_slot_and_name_no_page_s_own_words() {
        let sheet = Blocker::carried()
            .hiding("https://www.theguardian.com/international")
            .unwrap_or_default();
        let rules: Vec<&str> = sheet.lines().collect();
        assert!(rules.len() > 13_000, "{}", rules.len());
        assert!(
            rules
                .iter()
                .all(|rule| rule.ends_with("{display:none!important}")
                    && rule.matches('{').count() == 1),
            "every rule is one selector and the one declaration"
        );
        // Measured 6 November 2026 on the Guardian's, the BBC's and CNN's
        // front pages: what the generic rules hid were these slots.
        for named in [".ad-slot-container", ".dotcom-ad", ".ad-slot"] {
            let rule = format!("{named}{{display:none!important}}");
            assert!(rules.contains(&rule.as_str()), "{named}");
        }
    }

    #[test]
    fn the_carried_lists_are_the_pinned_ones_with_their_licence() {
        assert!(
            EASYLIST
                .text
                .starts_with("[Adblock Plus 2.0]\n! Version: 202611011105\n")
        );
        assert!(
            EASYPRIVACY
                .text
                .starts_with("[Adblock Plus 1.1]\n! Version: 202611011057\n")
        );
        let [(name, licence)] = LICENCES;
        assert_eq!(name, "EasyList-and-EasyPrivacy.txt");
        for named in [
            "202611011105",
            "202611011057",
            "The EasyList authors (https://easylist.to/)",
            "https://creativecommons.org/licenses/by-sa/3.0/legalcode",
        ] {
            assert!(licence.contains(named), "{named}");
        }
    }
}

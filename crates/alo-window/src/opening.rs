/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person asked `alo` to show when they started it.
//!
//! Until the address bar exists (item 119) the only way to say where to go is
//! the command line: a page's file, and the style sheets to give it, the way
//! the corpus keeps a frozen page beside its sheet. Nothing named is a page
//! from the network. With nothing named, the window opens one empty tab,
//! `about:blank` (ADR 0024 § 6).
//!
//! The files are read **here, in the browser process**, which is the process
//! allowed to open a file (ADR 0005); a renderer is handed the text and never
//! learns a path.
//!
//! It is also where a person keeps agents outside the browser to the sites
//! they name (`--agent-site`, queue item 497): a person's deliberate act,
//! which no agent can make (ADR 0026), and ours alone — it only ever narrows
//! what the socket already allows ([`crate::agent_sites`]).
//!
//! And it is where a person lays the window out at a scale its screen is
//! not (`--scale`, queue item 599), as Chrome's `--force-device-scale-factor`
//! does: the one way to see a scale none of this machine's screens has.

use crate::agent_sites::AgentSites;
use crate::conductor::Fonts;
use crate::message::Order;
use crate::scale::Scale;
use alo_layout::Size;
use alo_renderer::Page;
use alo_url::Url;
use std::path::{Path, PathBuf};

/// How to use `alo`, as it is said when it was used wrongly.
pub const USAGE: &str = "usage: alo [--frozen-fonts] [--scale FACTOR] [--agent-site ADDRESS ...] \
     [PAGE.html [SHEET.css ...]]";

/// What was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// Whether renderers are sent the fonts a page asks for.
    pub fonts: Fonts,
    /// The page's file, if one was named.
    pub page: Option<PathBuf>,
    /// The style sheets to give it, in the order they were named.
    pub sheets: Vec<PathBuf>,
    /// The sites agents outside the browser may reach.
    pub agents: AgentSites,
    /// The scale the window is laid out at whatever its screen's, if one
    /// was named.
    pub scale: Option<Scale>,
}

impl Opening {
    /// Read the command line's arguments, the program's own name left out.
    ///
    /// `--frozen-fonts` starts renderers with the corpus's compiled-in fonts
    /// and sends them no others, so that a frozen page is drawn exactly as its
    /// committed render was — which is what lets a capture of the window be
    /// compared with a reference at all.
    ///
    /// `--agent-site ADDRESS`, as often as there are sites, keeps every agent
    /// outside the browser to the sites of those addresses. Without it, they
    /// are kept to nothing narrower than the socket.
    ///
    /// `--scale FACTOR` lays the window out at that scale on any screen: one
    /// this engine has measured ([`Scale::named`]) — a whole factor up to
    /// eight, or 1.5 — so that a scale nobody measured is refused rather
    /// than quietly shown at another.
    ///
    /// # Errors
    ///
    /// [`USAGE`] with what was wrong, for an option that is not one, an
    /// `--agent-site` with no address or one that has no site, a `--scale`
    /// with no factor or one that is not measured, or a sheet with no page.
    pub fn from_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut fonts = Fonts::AsAsked;
        let mut files = Vec::new();
        let mut sites = Vec::new();
        let mut scale = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            if argument == "--frozen-fonts" {
                fonts = Fonts::AsStarted;
            } else if argument == "--agent-site" {
                let Some(address) = arguments.next() else {
                    return Err(format!("--agent-site names no address\n{USAGE}"));
                };
                sites.push(address);
            } else if argument == "--scale" {
                let Some(factor) = arguments.next() else {
                    return Err(format!("--scale names no factor\n{USAGE}"));
                };
                scale = Some(named_scale(&factor)?);
            } else if argument.starts_with("--") {
                return Err(format!("{argument:?} is not an option\n{USAGE}"));
            } else {
                files.push(PathBuf::from(argument));
            }
        }
        let agents = AgentSites::named(sites.iter().map(String::as_str))
            .map_err(|why| format!("--agent-site: {why}\n{USAGE}"))?;
        let mut files = files.into_iter();
        Ok(Self {
            fonts,
            page: files.next(),
            sheets: files.collect(),
            agents,
            scale,
        })
    }

    /// The order that opens the first tab, its files read.
    ///
    /// The page is laid out at [`Size::ZERO`] here and at the window's size by
    /// the conductor, which knows it.
    ///
    /// # Errors
    ///
    /// Which file could not be read or named as a URL, and why.
    pub fn first_tab(&self) -> Result<Order, String> {
        let Some(path) = &self.page else {
            return Ok(Order::Open {
                url: Url::about_blank(),
                page: None,
            });
        };
        let url = file_url(path)?;
        let mut page = Page::new(read(path)?, Size::ZERO).at(url.clone());
        for sheet in &self.sheets {
            page = page.with_sheet(read(sheet)?);
        }
        Ok(Order::Open {
            url,
            page: Some(Box::new(page)),
        })
    }
}

/// The measured scale `factor` names, or why it names none.
fn named_scale(factor: &str) -> Result<Scale, String> {
    factor
        .parse::<f64>()
        .ok()
        .and_then(Scale::named)
        .ok_or_else(|| {
            format!(
                "--scale {factor:?} is not a scale this browser has measured: \
                 a whole factor from 1 to 8, or 1.5\n{USAGE}"
            )
        })
}

/// A file's text.
fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|why| format!("{} could not be read: {why}", path.display()))
}

/// The `file:` URL of a path, made absolute first.
fn file_url(path: &Path) -> Result<Url, String> {
    let absolute = std::path::absolute(path)
        .map_err(|why| format!("{} has no absolute path: {why}", path.display()))?;
    let text = absolute
        .to_str()
        .ok_or_else(|| format!("{} is not a path a URL can name", absolute.display()))?;
    alo_url::parse(&format!("file://{text}")).map_err(|why| {
        format!(
            "{} is not a path a URL can name: {why:?}",
            absolute.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn nothing_named_is_one_empty_tab() {
        let opening = Opening::from_arguments(arguments(&[])).expect("no arguments is fine");
        assert_eq!(opening.page, None);
        assert_eq!(opening.fonts, Fonts::AsAsked);
        let Ok(Order::Open { url, page }) = opening.first_tab() else {
            panic!("no tab");
        };
        assert_eq!(url, Url::about_blank());
        assert!(page.is_none());
    }

    #[test]
    fn a_page_and_its_sheets_are_named_in_order() {
        let opening =
            Opening::from_arguments(arguments(&["--frozen-fonts", "a.html", "b.css", "c.css"]))
                .expect("these are arguments");
        assert_eq!(opening.fonts, Fonts::AsStarted);
        assert_eq!(opening.page, Some(PathBuf::from("a.html")));
        assert_eq!(
            opening.sheets,
            vec![PathBuf::from("b.css"), PathBuf::from("c.css")]
        );
    }

    #[test]
    fn agents_are_kept_to_the_sites_a_person_names() {
        let opening = Opening::from_arguments(arguments(&[
            "--agent-site",
            "https://example.com",
            "page.html",
            "--agent-site",
            "https://shop.example.org/",
        ]))
        .expect("these are arguments");
        assert_eq!(opening.page, Some(PathBuf::from("page.html")));
        assert_eq!(
            opening.agents.to_string(),
            "only https://example.com, https://example.org"
        );
        let nothing = Opening::from_arguments(arguments(&[])).expect("no arguments is fine");
        assert_eq!(nothing.agents, AgentSites::Every);
    }

    #[test]
    fn a_site_that_is_not_one_is_refused_before_the_window_opens() {
        let refused = |words: &[&str]| Opening::from_arguments(arguments(words));
        assert!(refused(&["--agent-site"]).is_err_and(|why| why.contains("names no address")));
        assert!(
            refused(&["--agent-site", "nowhere"])
                .is_err_and(|why| why.contains("\"nowhere\" is not an address"))
        );
        assert!(
            refused(&["--agent-site", "data:text/html,hi"])
                .is_err_and(|why| why.contains("opaque"))
        );
    }

    #[test]
    fn a_window_is_laid_out_at_a_scale_a_person_names() {
        let opening = Opening::from_arguments(arguments(&["--scale", "1.5", "page.html"]))
            .expect("these are arguments");
        assert_eq!(opening.scale, Some(Scale::ONE_AND_A_HALF));
        assert_eq!(opening.page, Some(PathBuf::from("page.html")));
        let two = Opening::from_arguments(arguments(&["--scale", "2"])).expect("an argument");
        assert_eq!(two.scale, Some(Scale::TWO));
        let none = Opening::from_arguments(arguments(&[])).expect("no arguments is fine");
        assert_eq!(none.scale, None);
    }

    #[test]
    fn a_scale_nobody_measured_is_refused_before_the_window_opens() {
        let refused = |words: &[&str]| Opening::from_arguments(arguments(words));
        assert!(refused(&["--scale"]).is_err_and(|why| why.contains("names no factor")));
        for factor in ["1.25", "1.75", "0", "-1", "9", "NaN", "inf", "one", ""] {
            assert!(
                refused(&["--scale", factor])
                    .is_err_and(|why| why.contains("not a scale this browser has measured")),
                "{factor:?}"
            );
        }
    }

    #[test]
    fn an_option_that_is_not_one_is_refused_by_name() {
        let refused = Opening::from_arguments(arguments(&["--fonts"]));
        assert!(refused.is_err_and(|why| why.contains("\"--fonts\"") && why.contains(USAGE)));
    }

    #[test]
    fn a_frozen_page_is_read_with_its_sheet_and_named_by_its_file() {
        let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/alo-sign-in");
        let opening = Opening {
            fonts: Fonts::AsStarted,
            page: Some(case.join("page.html")),
            sheets: vec![case.join("style.css")],
            agents: AgentSites::Every,
            scale: None,
        };
        let Ok(Order::Open {
            url,
            page: Some(page),
        }) = opening.first_tab()
        else {
            panic!("the frozen page was not read");
        };
        assert_eq!(url.scheme, "file");
        assert!(url.path.ends_with("/alo-sign-in/page.html"), "{}", url.path);
        assert_eq!(page.url, url);
        assert!(page.html.contains("alo's sign-in screen"));
        assert_eq!(page.sheets.len(), 1);
    }

    #[test]
    fn a_file_that_is_not_there_is_said_by_its_name() {
        let opening = Opening {
            fonts: Fonts::AsAsked,
            page: Some(PathBuf::from("/nowhere/at/all.html")),
            sheets: Vec::new(),
            agents: AgentSites::Every,
            scale: None,
        };
        assert!(
            opening
                .first_tab()
                .is_err_and(|why| why.contains("/nowhere/at/all.html"))
        );
    }
}

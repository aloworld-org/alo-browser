/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own small menu over a card's page (queue item 798,
//! `docs/design/shell.md`, approved by the owner 2026-11-06): what it
//! offers, where it stands, and the elements it is drawn as.
//!
//! A press of the secondary button on a card's page — or the main one with
//! Ctrl, which a Mac takes as it — is the page's first: it hears its
//! `contextmenu`, and may take it for itself, showing a menu of its own in
//! the page. When it does not, Chromium would show its context menu in the
//! parked window, where nobody sees it; the Chromium alo carries tells alo
//! instead what the press was over, and alo draws this menu where the
//! press was:
//!
//! - **Copy** — what is selected on the page, as ⌘C copies it; greyed when
//!   nothing is;
//! - **Copy link** and **Open link in new chat** — over a link only;
//! - **Search for “…”** — only when words are selected, the words cut at
//!   [`SEARCHED_SHOWN`] characters in the menu and searched whole.
//!
//! It is a white box rounded [`ROUNDED`], under a line-colour border and a
//! soft shadow, [`WIDE`] wide, [`ITEM`] a line with [`INSIDE`] above the
//! first and below the last ([`CardMenu::tall`]) — its top left corner at
//! the press, moved left so it stays inside the page box, and above the
//! press when there is no room under it ([`CardMenu::placed`]). To an
//! agent it is a `menu` of named `menuitem`s; nothing in it takes a point.

use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// How wide the menu is, its border included, in CSS pixels.
pub const WIDE: f32 = 220.0;

/// How tall one of its lines is.
pub const ITEM: f32 = 30.0;

/// The room above its first line and below its last, inside its border.
pub const INSIDE: f32 = 4.0;

/// Its border, each side.
pub const BORDER: f32 = 1.0;

/// How rounded its corners are.
pub const ROUNDED: f32 = 10.0;

/// The most characters of the words selected the menu shows in *Search
/// for “…”*: past them it shows `…`. Sixteen, so the line fits the menu's
/// 196 inside its padding at 13 px — found by hand: at 24 the line was
/// cut before its closing quote.
pub const SEARCHED_SHOWN: usize = 16;

/// What the menu is named, to an agent.
pub const NAMED: &str = "Page menu";

/// What a line of the menu does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    /// Copy what is selected on the page — greyed, doing nothing, when
    /// nothing is.
    Copy {
        /// Whether something is selected.
        selected: bool,
    },
    /// Copy the link's address.
    CopyLink(String),
    /// Open the link as a new chat's first turn.
    OpenInNewChat(String),
    /// Search for the words selected, as the conversation's next turn.
    Search(String),
}

impl MenuItem {
    /// The words it is drawn with, and named by.
    pub fn label(&self) -> String {
        match self {
            Self::Copy { .. } => "Copy".to_owned(),
            Self::CopyLink(_) => "Copy link".to_owned(),
            Self::OpenInNewChat(_) => "Open link in new chat".to_owned(),
            Self::Search(words) => {
                let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut shown: String = words.chars().take(SEARCHED_SHOWN).collect();
                if words.chars().count() > SEARCHED_SHOWN {
                    shown = format!("{}…", shown.trim_end());
                }
                format!("Search for “{shown}”")
            }
        }
    }

    /// Whether pressing it does anything: all but *Copy* with nothing
    /// selected.
    pub fn pressable(&self) -> bool {
        !matches!(self, Self::Copy { selected: false })
    }
}

/// alo's menu over a card's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardMenu {
    /// Its top left corner in the page's CSS pixels, placed.
    pub at: (i32, i32),
    /// Its lines, in order.
    pub items: Vec<MenuItem>,
}

impl CardMenu {
    /// What the menu offers for a press over `link`, if it was over one,
    /// with `selection` selected on the page, if anything is: *Copy*, then
    /// the link's two, then *Search*.
    pub fn offering(link: Option<&str>, selection: Option<&str>) -> Vec<MenuItem> {
        let selection = selection.map(str::trim).filter(|words| !words.is_empty());
        let mut items = vec![MenuItem::Copy {
            selected: selection.is_some(),
        }];
        if let Some(link) = link {
            items.push(MenuItem::CopyLink(link.to_owned()));
            items.push(MenuItem::OpenInNewChat(link.to_owned()));
        }
        if let Some(words) = selection {
            items.push(MenuItem::Search(words.to_owned()));
        }
        items
    }

    /// How tall a menu of `lines` lines is, its border included.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a menu has at most four lines, exact in an f32"
    )]
    pub fn tall(lines: usize) -> f32 {
        lines as f32 * ITEM + 2.0 * (INSIDE + BORDER)
    }

    /// The menu of `items` for a press at `press` in a page box of `page`
    /// CSS pixels: its top left corner at the press, moved left so its
    /// right edge is inside the page box, and drawn above the press when
    /// there is no room under it — never past the page box's top or left.
    pub fn placed(press: (i32, i32), page: (u32, u32), items: Vec<MenuItem>) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the menu's size in whole CSS pixels, far inside an i32"
        )]
        let (wide, tall) = (WIDE as i32, Self::tall(items.len()) as i32);
        let width = i32::try_from(page.0).unwrap_or(i32::MAX);
        let height = i32::try_from(page.1).unwrap_or(i32::MAX);
        let left = press.0.min(width.saturating_sub(wide)).max(0);
        let top = if press.1.saturating_add(tall) <= height {
            press.1
        } else {
            press.1.saturating_sub(tall)
        }
        .max(0);
        Self {
            at: (left, top),
            items,
        }
    }

    /// Whether `point`, in the page's CSS pixels, is over the menu.
    pub fn holds(&self, point: (f64, f64)) -> bool {
        let (left, top) = (f64::from(self.at.0), f64::from(self.at.1));
        point.0 >= left
            && point.0 < left + f64::from(WIDE)
            && point.1 >= top
            && point.1 < top + f64::from(Self::tall(self.items.len()))
    }
}

/// The menu's elements, for the card's page box to hold
/// ([`crate::card_document`]): a `menu` of `menuitem` buttons, each of the
/// class `mitem` — `mitem off`, disabled, for one that does nothing — at
/// its place.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no menu can cause.
pub fn make(document: &mut Document, menu: &CardMenu) -> Result<NodeId, Unbuilt> {
    let made = element(document, "div")?;
    attribute(document, made, "class", "cmenu")?;
    attribute(document, made, "role", "menu")?;
    attribute(document, made, "aria-label", NAMED)?;
    attribute(
        document,
        made,
        "style",
        &format!("left: {}px; top: {}px", menu.at.0, menu.at.1),
    )?;
    for item in &menu.items {
        let line = element(document, "button")?;
        attribute(document, line, "type", "button")?;
        attribute(document, line, "role", "menuitem")?;
        if item.pressable() {
            attribute(document, line, "class", "mitem")?;
        } else {
            attribute(document, line, "class", "mitem off")?;
            attribute(document, line, "disabled", "")?;
        }
        // Words a stranger's page selected, entering as a text node's data.
        let words = document.create_text_node(&item.label());
        append(document, line, words)?;
        append(document, made, line)?;
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_offers_copy_then_the_links_then_search() {
        assert_eq!(
            CardMenu::offering(None, None),
            vec![MenuItem::Copy { selected: false }]
        );
        assert_eq!(
            CardMenu::offering(Some("https://a.example/"), Some("  Kampala ")),
            vec![
                MenuItem::Copy { selected: true },
                MenuItem::CopyLink("https://a.example/".to_owned()),
                MenuItem::OpenInNewChat("https://a.example/".to_owned()),
                MenuItem::Search("Kampala".to_owned()),
            ]
        );
        assert_eq!(
            CardMenu::offering(None, Some("   ")),
            vec![MenuItem::Copy { selected: false }],
            "only white space is nothing selected"
        );
    }

    #[test]
    fn search_shows_the_words_cut_and_copy_with_nothing_selected_does_nothing() {
        assert_eq!(
            MenuItem::Search("Kampala".to_owned()).label(),
            "Search for “Kampala”"
        );
        assert_eq!(
            MenuItem::Search("the hills of Kampala\nand  Entebbe by the lake".to_owned()).label(),
            "Search for “the hills of Kam…”"
        );
        assert!(!MenuItem::Copy { selected: false }.pressable());
        assert!(MenuItem::Copy { selected: true }.pressable());
        assert!(MenuItem::Search("a".to_owned()).pressable());
    }

    #[test]
    fn it_stands_at_the_press_inside_the_page_box() {
        let four = CardMenu::offering(Some("https://a.example/"), Some("a"));
        assert_eq!(CardMenu::tall(4).to_bits(), 130.0_f32.to_bits());
        assert_eq!(CardMenu::tall(1).to_bits(), 40.0_f32.to_bits());
        // At the press.
        assert_eq!(
            CardMenu::placed((60, 52), (878, 520), four.clone()).at,
            (60, 52)
        );
        // Too near the right edge: moved left; too near the foot: above.
        assert_eq!(
            CardMenu::placed((800, 450), (878, 520), four.clone()).at,
            (658, 320)
        );
        // A page box too small for it: at its top left.
        assert_eq!(CardMenu::placed((10, 50), (100, 100), four).at, (0, 0));
    }

    #[test]
    fn a_point_is_over_it_inside_its_box_alone() {
        let menu = CardMenu::placed((100, 100), (878, 520), CardMenu::offering(None, None));
        assert!(menu.holds((100.0, 100.0)));
        assert!(menu.holds((319.5, 139.5)));
        assert!(!menu.holds((320.0, 120.0)));
        assert!(!menu.holds((150.0, 140.0)));
        assert!(!menu.holds((99.9, 120.0)));
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ An option chosen through Chrome (ADR 0064 § 6, queue item 660).
//!
//! `alo-agent` decides that `Activate` on an option of a `<select>` chooses
//! it in its list ([`Outcome::Chosen`]); this carries that out on a page in
//! Chrome as a person does, and reads back what came of it.
//!
//! - **A list Chrome draws** — one with a `size`, or `multiple` — is chosen
//!   in by a click at the option's middle, aimed and checked as every press
//!   is ([`Tab::aim`]). In a list of several the click holds the platform's
//!   toggle, ⌘ on macOS: measured against Chrome 155, a plain click there
//!   chooses that option **alone** and drops every other choice, and a
//!   ⌘-click toggles it, as `alo_dom::choice` does on this engine's page.
//!   The page hears what it hears from a person: one `input` and one
//!   `change` at the list when the click changed what it holds.
//! - **A closed list** draws no option, so there is no box to press; and on
//!   a focused closed list every key that reaches an option — type-ahead's
//!   letters — passes through options nobody chose, each heard as `input`
//!   and `change`. So **headless**, and off macOS, nothing is sent to
//!   Chrome at all, and the answer is [`Unacted::Closed`].
//! - **A closed list in a shown Chrome on macOS** is chosen in its menu, as
//!   a person chooses from the keyboard (ADR 0071, item 658): the click
//!   every press makes, at the list's middle, opens the menu; arrows walk
//!   its highlight, which chooses nothing; Return chooses. Each key is
//!   posted to Chrome's process only while the window server shows the
//!   menu and the pointer is not over it ([`crate::chrome_menu`]), and the
//!   verb answers [`Unacted::Unwalked`] the moment either fails, closing
//!   any menu left open with Escape. The page hears one `input` and one
//!   `change`, for the option chosen.
//!
//!   The arrows are counted over the menu's items ([`walk`]), as measured
//!   against Chrome 156 on macOS 26: the menu has an item for every option
//!   but one whose own `display` computed to `none` — `hidden` is that —
//!   which Chrome's tree does not say, so each option's computed style is
//!   read over Chrome's CSS domain; and the highlight reaches every item
//!   but a disabled one's, an option in a disabled group being disabled in
//!   Chrome's tree. An option in a
//!   group whose `display` is `none` has an item, and so does one whose
//!   `visibility` is `hidden`. The highlight starts at the option the list
//!   holds; when it holds none, or one the highlight cannot reach, it
//!   starts nowhere, and Down goes to the first item and Up to the last.
//!   It stops at either end rather than going round.
//! - **Never script**, and never `DOM.setAttributeValue`: setting the
//!   attribute sends the page no `input` and no `change`, so its listeners
//!   and checks would not run.
//!
//! Whether the option is chosen afterwards is **read from Chrome's tree**
//! once the click has been answered, not assumed from what was asked: a
//! page's `change` listener may put the old choice back, and the answer then
//! says the option is not chosen.

use crate::chrome::Failed;
use crate::chrome_act::{Unacted, dom_of};
use crate::chrome_menu::{self, Key, Posted, Unposted};
use crate::chrome_protocol::{self, AxNode, Command};
use crate::chrome_tab::Tab;
use crate::window_server::{self, Unlisted};
use alo_agent::Outcome;
use alo_box::BoxId;
use core::fmt;
use std::collections::HashMap;

/// An option to choose, in its list, as the reading of the page says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Choosing {
    /// The option.
    pub(crate) option: BoxId,
    /// Its list.
    pub(crate) list: BoxId,
    /// Whether the list drops down, and so draws no option.
    pub(crate) closed: bool,
    /// Whether the list takes several choices, in which the click toggles.
    pub(crate) toggles: bool,
}

impl Tab<'_> {
    /// Choose `choosing.option` in its list, with `nodes` the tree read
    /// before the click, and say whether it is chosen afterwards.
    pub(crate) fn choose(
        &mut self,
        nodes: &[AxNode],
        choosing: Choosing,
    ) -> Result<Outcome, Unacted> {
        let Choosing {
            option,
            list,
            closed,
            toggles,
        } = choosing;
        if closed {
            if !self.is_shown() || !cfg!(target_os = "macos") {
                return Err(Unacted::Closed { node: option, list });
            }
            self.choose_in_menu(nodes, option, list)?;
        } else {
            let (x, y) = self.aim(nodes, option)?;
            self.click(x, y, toggles)?;
        }
        let read = chrome_protocol::ax_nodes(&self.ask(&Command::ReadNode {
            node: dom_of(option),
        })?)?;
        Ok(Outcome::Chosen {
            node: option,
            list,
            chosen: is_selected(&read, dom_of(option)),
        })
    }

    /// Choose `option` of the closed list `list`, read in `nodes`, in the
    /// list's menu (ADR 0071 § 2) — or post nothing when the list holds it
    /// already.
    fn choose_in_menu(
        &mut self,
        nodes: &[AxNode],
        option: BoxId,
        list: BoxId,
    ) -> Result<(), Unacted> {
        let unwalked = |why| Unacted::Unwalked {
            node: option,
            list,
            why,
        };
        let options = self.in_menu(nodes, dom_of(list))?;
        let (key, count) = match walk(&options, dom_of(option)) {
            Some(Walk::Held) => return Ok(()),
            Some(Walk::Arrows { key, count }) => (key, count),
            None => return Err(unwalked(Unwalked::NoItem)),
        };
        let owner = self.chrome_process();
        let before =
            window_server::on_the_screen(owner).map_err(|why| unwalked(Unwalked::Unlisted(why)))?;
        let (x, y) = self.aim(nodes, list)?;
        self.click(x, y, false)?;
        let menu = chrome_menu::shown(owner, &before)
            .map_err(|why| unwalked(Unwalked::Unlisted(why)))?
            .ok_or_else(|| unwalked(Unwalked::NeverShown))?;
        let keys = core::iter::repeat_n(key, count).chain([Key::Return]);
        for (posted, key) in keys.enumerate() {
            let why = match chrome_menu::post_while_shown(owner, key, menu) {
                Ok(Posted::Posted) => continue,
                Ok(Posted::Gone) => Unwalked::Gone { posted },
                Ok(Posted::UnderPointer) => Unwalked::UnderPointer { posted },
                Err(why) => Unwalked::Unposted(why),
            };
            // A key that raced the menu's going may have opened it again,
            // a menu under the pointer is still open, and whether the last
            // key went is not always known: any menu of the list's still
            // shown is closed, choosing nothing.
            if chrome_menu::close_any(owner, &before) == Ok(Posted::Posted) {
                // Answered once it has gone, or once it was waited for as
                // long as a menu is: the answer is the same either way.
                drop(chrome_menu::closed(owner, &before));
            }
            return Err(unwalked(why));
        }
        // Return chose; what the page holds is read once the menu has gone.
        if chrome_menu::gone(owner, menu).map_err(|why| unwalked(Unwalked::Unlisted(why)))? {
            Ok(())
        } else {
            // Still open: closed choosing nothing, if Escape reaches it.
            drop(chrome_menu::close_any(owner, &before));
            Err(unwalked(Unwalked::NeverClosed))
        }
    }

    /// The options of the closed list `list`, by its DOM node, as its menu
    /// has them, in tree order: from Chrome's tree, `nodes`, which they are,
    /// which is disabled and which is chosen, and from each one's computed
    /// style whether its `display` is `none`.
    fn in_menu(&mut self, nodes: &[AxNode], list: u64) -> Result<Vec<InMenu>, Unacted> {
        let options = options_of(nodes, list);
        let doms: Vec<u64> = options.iter().map(|option| option.dom).collect();
        self.ask(&Command::ReadDocument)?;
        let ids = chrome_protocol::pushed(
            &self.ask(&Command::IdsOf {
                nodes: doms.clone(),
            })?,
            doms.len(),
        )
        .map_err(Failed::Malformed)?;
        self.ask(&Command::ReadStyles)?;
        let displayed = self.displayed(&ids);
        // Off again whatever was read: Chrome otherwise tells of every
        // style sheet the page adds from now on.
        let off = self.ask(&Command::StopReadingStyles);
        let displayed = displayed?;
        off?;
        Ok(options
            .into_iter()
            .zip(displayed)
            .map(|(option, displayed)| InMenu {
                item: option.item && displayed,
                ..option
            })
            .collect())
    }

    /// Whether each node of `ids`, by the DOM domain's id, has a `display`
    /// other than `none`.
    fn displayed(&mut self, ids: &[u64]) -> Result<Vec<bool>, Failed> {
        ids.iter()
            .map(|id| {
                let style = self.ask(&Command::ComputedStyle { node: *id })?;
                let display =
                    chrome_protocol::computed(&style, "display").map_err(Failed::Malformed)?;
                Ok(display != "none")
            })
            .collect()
    }
}

/// Why an option of a closed list was not chosen in its menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unwalked {
    /// The menu has no item for the option — its `display` is `none` — so
    /// no key can reach it, and none was posted.
    NoItem,
    /// The window server never showed the menu after the click, so no key
    /// was posted.
    NeverShown,
    /// The menu witnessed after the click was not shown before a key,
    /// after `posted` keys, so no further key was posted — and, Return
    /// never posted, nothing chosen in it.
    Gone {
        /// How many keys were posted before it went.
        posted: usize,
    },
    /// The pointer was over the menu before a key, after `posted` keys —
    /// so the highlight was wherever it put it — and no further key was
    /// posted, and nothing chosen in it.
    UnderPointer {
        /// How many keys were posted before.
        posted: usize,
    },
    /// The window server still showed the menu after Return.
    NeverClosed,
    /// The window server could not be read.
    Unlisted(Unlisted),
    /// Whether a key was posted is not known.
    Unposted(Unposted),
}

impl fmt::Display for Unwalked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoItem => f.write_str("its menu has no item for it, so no key was posted"),
            Self::NeverShown => {
                f.write_str("its menu was never shown after the click, so no key was posted")
            }
            Self::Gone { posted } => write!(
                f,
                "its menu was gone after {posted} of the keys, so no more were posted and nothing was chosen"
            ),
            Self::UnderPointer { posted } => write!(
                f,
                "the pointer was over its menu after {posted} of the keys, so no more were posted and nothing was chosen"
            ),
            Self::NeverClosed => f.write_str("its menu was still shown after Return"),
            Self::Unlisted(why) => why.fmt(f),
            Self::Unposted(why) => why.fmt(f),
        }
    }
}

/// An option of a closed list, as its menu has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InMenu {
    /// The option, by Chrome's `backendNodeId`.
    dom: u64,
    /// Whether the menu has an item for it that its highlight can reach.
    item: bool,
    /// Whether the list holds it.
    chosen: bool,
}

/// The keys that walk a closed list's menu to an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    /// The list holds it already: no key.
    Held,
    /// `count` of `key`, then Return.
    Arrows {
        /// Down or Up.
        key: Key,
        /// How many.
        count: usize,
    },
}

/// The keys that walk the menu of a list of `options` to the option `asked`,
/// by Chrome's `backendNodeId` — or [`None`] when no key can reach it.
fn walk(options: &[InMenu], asked: u64) -> Option<Walk> {
    if options
        .iter()
        .any(|option| option.dom == asked && option.chosen)
    {
        return Some(Walk::Held);
    }
    let items: Vec<&InMenu> = options.iter().filter(|option| option.item).collect();
    let to = items.iter().position(|option| option.dom == asked)?;
    let arrows = |key, count| Some(Walk::Arrows { key, count });
    match items.iter().position(|option| option.chosen) {
        Some(from) if from < to => arrows(Key::Down, to - from),
        Some(from) => arrows(Key::Up, from - to),
        // Highlighting nothing, Down goes to the first item and Up to the
        // last: the nearer way round.
        None => {
            let down = to.saturating_add(1);
            let up = items.len() - to;
            if down <= up {
                arrows(Key::Down, down)
            } else {
                arrows(Key::Up, up)
            }
        }
    }
}

/// The options under the DOM node `list` in Chrome's tree, `nodes`, in tree
/// order: each with whether it is chosen, and whether it is not disabled —
/// its menu's item, if it has one, reachable.
fn options_of(nodes: &[AxNode], list: u64) -> Vec<InMenu> {
    let by_id: HashMap<&str, &AxNode> = nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let said = |node: &AxNode, name: &str| {
        node.properties
            .iter()
            .any(|(property, value)| property == name && value == "true")
    };
    let mut options = Vec::new();
    let mut waiting: Vec<&AxNode> = nodes
        .iter()
        .filter(|node| node.node == Some(list))
        .collect();
    // A tree read already holds no loop; the bound keeps this from
    // following one regardless.
    for _ in 0..nodes.len() {
        let Some(node) = waiting.pop() else {
            break;
        };
        if node.role == "option"
            && let Some(dom) = node.node
        {
            options.push(InMenu {
                dom,
                item: !said(node, "disabled"),
                chosen: said(node, "selected"),
            });
        }
        waiting.extend(
            node.children
                .iter()
                .rev()
                .filter_map(|child| by_id.get(child.as_str()).copied()),
        );
    }
    options
}

/// Whether Chrome's tree, `read`, says the DOM node `dom` is selected: an
/// option nobody's tree says is chosen is not.
fn is_selected(read: &[AxNode], dom: u64) -> bool {
    read.iter()
        .find(|node| node.node == Some(dom))
        .is_some_and(|node| {
            node.properties
                .iter()
                .any(|(name, value)| name == "selected" && value == "true")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(dom: u64, properties: &[(&str, &str)]) -> AxNode {
        AxNode {
            id: dom.to_string(),
            node: Some(dom),
            role: "option".to_owned(),
            properties: properties
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            ..AxNode::default()
        }
    }

    /// The list measured for ADR 0071 and item 658: *Apples* chosen,
    /// *Bananas* disabled, *Cherries* `hidden`, *Dates* and *Elderberries*
    /// in a group, *Figs* in a disabled group, *Grapes* `display: none`,
    /// *Honeydew*.
    fn fruit(chosen: Option<u64>) -> Vec<InMenu> {
        [
            (1, true),
            (2, false),
            (3, false),
            (4, true),
            (5, true),
            (6, false),
            (7, false),
            (8, true),
        ]
        .map(|(dom, item)| InMenu {
            dom,
            item,
            chosen: chosen == Some(dom),
        })
        .to_vec()
    }

    fn arrows(key: Key, count: usize) -> Walk {
        Walk::Arrows { key, count }
    }

    #[test]
    fn the_arrows_are_counted_over_the_items_the_highlight_reaches() {
        // Measured: from Apples, one Down chose Dates, two Elderberries,
        // three Honeydew.
        let from_apples = fruit(Some(1));
        assert_eq!(walk(&from_apples, 4), Some(arrows(Key::Down, 1)));
        assert_eq!(walk(&from_apples, 5), Some(arrows(Key::Down, 2)));
        assert_eq!(walk(&from_apples, 8), Some(arrows(Key::Down, 3)));
        // Measured: from Elderberries one Up chose Dates; from Honeydew,
        // Elderberries.
        assert_eq!(walk(&fruit(Some(5)), 4), Some(arrows(Key::Up, 1)));
        assert_eq!(walk(&fruit(Some(8)), 5), Some(arrows(Key::Up, 1)));
        assert_eq!(walk(&fruit(Some(8)), 1), Some(arrows(Key::Up, 3)));
    }

    #[test]
    fn an_option_the_list_holds_takes_no_key() {
        assert_eq!(walk(&fruit(Some(4)), 4), Some(Walk::Held));
        assert_eq!(
            walk(&fruit(Some(7)), 7),
            Some(Walk::Held),
            "held, though the menu has no item for it"
        );
    }

    #[test]
    fn an_option_with_no_item_is_reached_by_no_key() {
        for asked in [2, 3, 6, 7, 9] {
            assert_eq!(walk(&fruit(Some(1)), asked), None, "{asked}");
        }
    }

    #[test]
    fn highlighting_nothing_down_goes_to_the_first_item_and_up_to_the_last() {
        // Measured: with none chosen, or a disabled, hidden or undisplayed
        // option chosen, one Down chose Apples and one Up Honeydew.
        for chosen in [None, Some(2), Some(3), Some(6), Some(7)] {
            let options = fruit(chosen);
            assert_eq!(walk(&options, 1), Some(arrows(Key::Down, 1)), "{chosen:?}");
            assert_eq!(walk(&options, 4), Some(arrows(Key::Down, 2)), "{chosen:?}");
            assert_eq!(walk(&options, 5), Some(arrows(Key::Up, 2)), "{chosen:?}");
            assert_eq!(walk(&options, 8), Some(arrows(Key::Up, 1)), "{chosen:?}");
        }
    }

    fn ax(
        id: &str,
        role: &str,
        dom: u64,
        properties: &[(&str, &str)],
        children: &[&str],
    ) -> AxNode {
        AxNode {
            id: id.to_owned(),
            node: Some(dom),
            role: role.to_owned(),
            properties: properties
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            children: children.iter().map(|child| (*child).to_owned()).collect(),
            ..AxNode::default()
        }
    }

    #[test]
    fn a_lists_options_are_read_in_tree_order_through_its_groups() {
        // As Chrome 156 reads a closed list: a combobox, its popup, and the
        // options and groups under that.
        let nodes = [
            ax("a", "combobox", 2, &[], &["b"]),
            ax("b", "MenuListPopup", 11, &[], &["c", "d", "e", "g"]),
            ax("c", "option", 15, &[("selected", "true")], &[]),
            ax("d", "option", 18, &[("disabled", "true")], &[]),
            ax("e", "group", 24, &[], &["f", "x"]),
            ax("f", "option", 27, &[("selected", "false")], &[]),
            ax("g", "option", 42, &[("selected", "false")], &[]),
            ax("y", "option", 99, &[], &[]),
        ];
        assert_eq!(
            options_of(&nodes, 2),
            [
                InMenu {
                    dom: 15,
                    item: true,
                    chosen: true
                },
                InMenu {
                    dom: 18,
                    item: false,
                    chosen: false
                },
                InMenu {
                    dom: 27,
                    item: true,
                    chosen: false
                },
                InMenu {
                    dom: 42,
                    item: true,
                    chosen: false
                },
            ],
            "a child Chrome did not send is passed over, and an option outside the list is not its"
        );
        assert_eq!(options_of(&nodes, 7), [], "a list not read");
    }

    #[test]
    fn why_an_option_was_not_chosen_in_its_menu_is_said() {
        assert_eq!(
            Unwalked::Gone { posted: 2 }.to_string(),
            "its menu was gone after 2 of the keys, so no more were posted and nothing was chosen"
        );
        assert_eq!(
            Unwalked::NoItem.to_string(),
            "its menu has no item for it, so no key was posted"
        );
    }

    #[test]
    fn an_option_is_chosen_only_when_chromes_tree_says_it_is_selected() {
        let read = [
            option(4, &[("selected", "false")]),
            option(5, &[("focusable", "true"), ("selected", "true")]),
            option(6, &[]),
        ];
        assert!(!is_selected(&read, 4));
        assert!(is_selected(&read, 5));
        assert!(!is_selected(&read, 6), "saying nothing is not chosen");
        assert!(!is_selected(&read, 7), "a node not read");
    }
}

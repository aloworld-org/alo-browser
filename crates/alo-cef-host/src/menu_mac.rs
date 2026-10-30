/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The host's menu bar (queue item 732).
//!
//! Chrome builds its menus in its application controller, which CEF does
//! not make, so a shown host's menu bar held only an empty *alo* menu
//! (measured, queue item 727): ⌘Q and ⌘W did nothing, and there was no
//! Edit menu for ⌘C, ⌘V or ⌘A to go through. This is the least a person
//! needs from it, and nothing more:
//!
//! - **alo**: *Quit alo* (⌘Q), which reaches the host's `-terminate:`
//!   (`application_mac.rs`) — the host's pages close and the host ends,
//!   and alo hears its pipe close as when Chrome is quit.
//! - **Edit**: *Cut* (⌘X), *Copy* (⌘C), *Paste* (⌘V) and *Select All*
//!   (⌘A), each sent to the focused page.
//! - **Window**: *Close Window* (⌘W), as the window's close button.
//!
//! The application menu's own title is never shown: macOS shows the
//! bundle's name, `alo`, there. No `unsafe` here: the one selector each
//! item sends is chosen in `application_mac.rs`.

use crate::application_mac::{Action, item};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApp, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

/// Put the host's menu bar in place; nothing when not on the main thread,
/// where there is no menu bar to put it in.
pub fn install() {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let bar = NSMenu::new(main);
    bar.addItem(&holding(
        main,
        "alo",
        &[item(main, "Quit alo", Action::Quit, "q")],
    ));
    bar.addItem(&holding(
        main,
        "Edit",
        &[
            item(main, "Cut", Action::Cut, "x"),
            item(main, "Copy", Action::Copy, "c"),
            item(main, "Paste", Action::Paste, "v"),
            item(main, "Select All", Action::SelectAll, "a"),
        ],
    ));
    bar.addItem(&holding(
        main,
        "Window",
        &[item(main, "Close Window", Action::CloseWindow, "w")],
    ));
    NSApp(main).setMainMenu(Some(&bar));
}

/// An item of the menu bar titled `title`, opening a menu of `items`.
fn holding(
    main: MainThreadMarker,
    title: &str,
    items: &[Retained<NSMenuItem>],
) -> Retained<NSMenuItem> {
    let title = NSString::from_str(title);
    let menu = NSMenu::initWithTitle(NSMenu::alloc(main), &title);
    for item in items {
        menu.addItem(item);
    }
    let opening = NSMenuItem::new(main);
    opening.setTitle(&title);
    opening.setSubmenu(Some(&menu));
    opening
}

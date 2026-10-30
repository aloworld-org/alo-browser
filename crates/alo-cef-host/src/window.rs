/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The host's pages: the one alo asked for with `--app=`, shown in a
//! window of the host's own (queue item 727, ADR 0079 § 3.2), and every
//! page closed when the host ends (queue item 731).
//!
//! Chrome opens `--app=URL` in a window with no tab strip and no address
//! bar, only a title bar over the page, and alo shows a page that way
//! (ADR 0061). That is Chrome's startup code, which CEF does not run — a
//! host told `--app=` opened nothing, not even a target (measured, 725) —
//! so the host opens the window itself, once CEF's context is up
//! ([`Opener`]): a top-level `CefWindow` of CEF's views holding one
//! `CefBrowserView` on the page, both **Chrome style** (ADR 0060's held
//! requests work only there, CEF issue #4308) — and **no toolbar**: CEF
//! leaves a Chrome-style toolbar for the host to place in the window, the
//! host places none, and asks for none ([`ChromeToolbarType::NONE`]; asked
//! for a normal one, the window was measured to look the same). What a
//! person sees (measured on this Mac, CEF 156.0.14): a window with rounded
//! corners, a title bar holding the system's close and minimise buttons
//! (zoom greyed) and the page's title ([`Titled`]) beside them,
//! left-aligned, and the page under it — no tab strip, no address bar,
//! no menu button. Without `Framed::with_standard_window_buttons` it has
//! no buttons at all.
//!
//! Everything else is alo's, over the pipe, exactly as for Chrome's own
//! `--app` window: the window's page is the one target Chromium lists, so
//! `chrome_tab.rs` attaches to it; `Browser.getWindowForTarget` and
//! `Browser.setWindowBounds` size and place it (`chrome_window.rs`). The
//! window is shown before its page is put in it, so the page is laid out
//! at the window's size from the start.
//!
//! # Closed
//!
//! A person closing the window — its close button, or ⌘W
//! (`menu_mac.rs`) — asks its page first, as Chrome does
//! (`Framed::can_close`); the page closes, alo hears it go (Chromium
//! detaches its session), and the window goes with it
//! (`Viewed::on_browser_destroyed`). A page closed from inside —
//! `Target.closeTarget`, the page's own `window.close()` — goes the same
//! way. The process goes on with no window until it is ended.
//!
//! # Ended in order
//!
//! Every page the host has open is counted ([`Counted`]): its own, and
//! every one Chromium opens of its own (`Target.createTarget`, a popup),
//! which CEF gives the same client ([`Opener`]'s `default_client`).
//! `Browser.close`, Quit in the menu bar and a logout reach the
//! application's `-terminate:`, which the host's own application object
//! leaves to [`close_all`] (`application_mac.rs`, ADR 0080): every page is
//! closed, and once the last is, the message loop is left and `host.rs`
//! shuts CEF down — status 0, and Chromium's own clean-up run, where
//! `AppKit`'s `-terminate:` ended the process at once (725). ⌘Q never
//! reaches the menu bar: the page's window takes it as Chrome's Exit
//! command, which is answered the same way ([`Commanded`]).
//!
//! Before ADR 0080 the host had no application object of its own, and
//! Chromium asks the application object `-isHandlingSendEvent`
//! (`CefAppProtocol`): a page closed from inside, a context menu and a
//! closed list's menu each crashed a shown host (`SIGTRAP`, measured,
//! 727). They no longer do (731).
//!
//! **What still crashes it, measured:** a page closed while its context
//! menu is still open — by alo (`Target.closeTarget` then `Browser.close`),
//! or by a person's close button — ends the host with `SIGSEGV` inside
//! CEF's framework, reading memory already freed (`0xcdcd…`), once the
//! page is gone. A context menu dismissed first (Escape) leaves the host
//! running and ending in order. Queue item 734.

//! # The `cef` crate's own `unsafe`, in its macros
//!
//! CEF calls back into the host through reference-counted C structures,
//! which the `cef` crate builds from a Rust type with its `wrap_*` macros.
//! The code those macros expand to — the reference count, the structure's
//! base — holds `unsafe` blocks, and it is **the crate's code, written in
//! the crate**, expanded here: the compiler's `unsafe_code` lint does not
//! report code from another crate's macro, and `main.rs` denies it in all
//! that is written in this one but `application_mac.rs` (ADR 0080).
//! Nothing in this file is `unsafe`. Clippy
//! does read that code, and finds the crate's own way of reaching the
//! structure's base — a reference transmuted to another — which is the
//! crate's to change, not ours: that one lint is expected here, in this
//! file only, and nowhere it would find code of ours.

#![expect(
    clippy::transmute_ptr_to_ptr,
    reason = "the `cef` crate's `wrap_*` macros transmute their own base reference; the \
              code is the crate's, expanded here, and no code of ours transmutes"
)]

use cef::{
    App, Browser, BrowserProcessHandler, BrowserSettings, BrowserView, BrowserViewDelegate,
    CefString, ChromeToolbarType, Client, CommandHandler, DisplayHandler, ImplApp, ImplBrowser,
    ImplBrowserHost, ImplBrowserProcessHandler, ImplBrowserView, ImplBrowserViewDelegate,
    ImplClient, ImplCommandHandler, ImplDisplayHandler, ImplLifeSpanHandler, ImplPanel,
    ImplPanelDelegate, ImplView, ImplViewDelegate, ImplWindow, ImplWindowDelegate, LifeSpanHandler,
    RuntimeStyle, Size, View, Window, WindowDelegate, WindowOpenDisposition, WrapApp,
    WrapBrowserProcessHandler, WrapBrowserViewDelegate, WrapClient, WrapCommandHandler,
    WrapDisplayHandler, WrapLifeSpanHandler, WrapWindowDelegate, browser_view_create,
    browser_view_get_for_browser, window_create_top_level, wrap_app, wrap_browser_process_handler,
    wrap_browser_view_delegate, wrap_client, wrap_command_handler, wrap_display_handler,
    wrap_life_span_handler, wrap_window_delegate,
};
use std::cell::{Cell, RefCell};
// The reference count the macros' code keeps, by its trait.
use cef::rc::Rc as _;

/// The size the window opens at, in the screen's points, before alo sizes
/// it for the page: Chrome's own `--app` window opens at a size of its
/// choosing too, and alo never relies on it.
const OPENED: Size = Size {
    width: 800,
    height: 600,
};

/// The host's application: the browser process's handler, opening `page`
/// once CEF is ready, when there is one.
pub fn app(page: Option<String>) -> App {
    Host::new(page)
}

wrap_app! {
    struct Host {
        page: Option<String>,
    }

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(Opener::new(self.page.clone()))
        }
    }
}

wrap_browser_process_handler! {
    struct Opener {
        page: Option<String>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            if let Some(page) = &self.page {
                open(page);
            }
        }

        fn default_client(&self) -> Option<Client> {
            // Every page Chromium opens of its own — `Target.createTarget`,
            // a popup — is counted as the host's own are, so that
            // [`close_all`] closes it too.
            Some(Page::new())
        }
    }
}

/// Open `page` in a window of its own, Chrome style, with no toolbar —
/// or say why not, and open nothing.
fn open(page: &str) {
    let mut client = Page::new();
    let mut viewed = Viewed::new();
    let Some(view) = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(page)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut viewed),
    ) else {
        eprintln!("alo's CEF host: CEF made no view for the page");
        return;
    };
    let mut framed = Framed::new(view);
    if window_create_top_level(Some(&mut framed)).is_none() {
        eprintln!("alo's CEF host: CEF made no window for the page");
    }
}

wrap_client! {
    struct Page {}

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(Titled::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(Counted::new())
        }

        fn command_handler(&self) -> Option<CommandHandler> {
            Some(Commanded::new())
        }
    }
}

wrap_command_handler! {
    struct Commanded {}

    impl CommandHandler {
        fn on_chrome_command(
            &self,
            _browser: Option<&mut Browser>,
            command_id: ::std::os::raw::c_int,
            _disposition: WindowOpenDisposition,
        ) -> ::std::os::raw::c_int {
            // ⌘Q is Chrome's own shortcut for its Exit command, taken by
            // the page's window before the menu bar sees it; Chrome's Exit
            // left the host neither ended nor able to end (measured), so
            // the host ends as its Quit item ends it. Every other command
            // is Chrome's.
            if command_id == cef::sys::IDC_EXIT {
                close_all();
                1
            } else {
                0
            }
        }
    }
}

thread_local! {
    /// Every page open in the host, on CEF's UI thread, which on macOS is
    /// the process's main thread.
    static OPEN: RefCell<Vec<Browser>> = const { RefCell::new(Vec::new()) };

    /// Whether the host is ending: once every page is closed, the message
    /// loop is left.
    static ENDING: Cell<bool> = const { Cell::new(false) };
}

wrap_life_span_handler! {
    struct Counted {}

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                OPEN.with_borrow_mut(|open| open.push(browser.clone()));
            }
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            let identifier = browser.identifier();
            let left = OPEN.with_borrow_mut(|open| {
                open.retain(|other| other.identifier() != identifier);
                open.len()
            });
            if left == 0 && ENDING.get() {
                cef::quit_message_loop();
            }
        }
    }
}

/// End the host in order — what `-terminate:` does (`application_mac.rs`),
/// so what ⌘Q and `Browser.close` reach: every page open is closed without
/// asking it (as Chrome's own `Browser.close` closes pages, ignoring their
/// `beforeunload`), and once the last is closed the message loop is left,
/// so `host.rs` shuts CEF down and the process ends with status 0.
pub fn close_all() {
    ENDING.set(true);
    let open = OPEN.with_borrow(Clone::clone);
    if open.is_empty() {
        cef::quit_message_loop();
        return;
    }
    for browser in open {
        if let Some(host) = browser.host() {
            host.close_browser(1);
        }
    }
}

wrap_display_handler! {
    struct Titled {}

    impl DisplayHandler {
        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(window) = browser_view_get_for_browser(browser).and_then(|view| view.window()) {
                window.set_title(title);
            }
        }
    }
}

wrap_browser_view_delegate! {
    struct Viewed {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }

        fn chrome_toolbar_type(&self, _browser_view: Option<&mut BrowserView>) -> ChromeToolbarType {
            ChromeToolbarType::NONE
        }

        fn on_browser_destroyed(
            &self,
            browser_view: Option<&mut BrowserView>,
            _browser: Option<&mut Browser>,
        ) {
            // Its page closed — by alo over the pipe, or by the page — and
            // the window that showed it goes with it.
            if let Some(window) = browser_view
                .and_then(|view| view.window())
                .filter(|window| window.is_closed() == 0)
            {
                window.close();
            }
        }
    }
}

wrap_window_delegate! {
    struct Framed {
        view: BrowserView,
    }

    impl ViewDelegate {}

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else {
                return;
            };
            window.center_window(Some(&OPENED));
            window.show();
            window.add_child_view(Some(&mut View::from(&self.view)));
            self.view.request_focus();
        }

        fn can_close(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            // Its page is asked first, as Chrome asks one, and the window
            // closes once the page has.
            match self.view.browser().and_then(|browser| browser.host()) {
                Some(host) => host.try_close_browser(),
                None => 1,
            }
        }

        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }
    }
}

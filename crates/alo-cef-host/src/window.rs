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
//! # A card's window: unseen and parked
//!
//! Started for a page alo shows as a card in its conversation
//! ([`crate::switches::card`], ADR 0076 B4 as amended, queue item 771), the
//! window is made **unseen** — fully transparent, letting a person's
//! pointer through (`unseen_mac.rs`) — before it is first shown, so it is
//! never drawn. alo fits it to the card where it was shown, then parks it
//! far left of every display, and the window server moves it back until a
//! strip of it is on one; the page goes on painting and streaming, and
//! takes the input alo sends it. The window is made seen once, and for
//! good, when alo puts it where a person is to see it after it was parked —
//! its centre on a display ([`crate::parked::placed`], [`Stage`]), which is
//! *Expand* — so a person who then drags it partly off a display keeps
//! seeing it. Where it was shown and fitted does not count: the window
//! server shows a window wholly on a display, wherever it was asked to be
//! (measured: a window put at −20000 before it was shown was shown wholly
//! on the leftmost display).
//!
//! A card's page cannot move its window while it is unseen: a page's own
//! `moveTo` or `resizeTo` — which CEF hands the page's display handler
//! (`OnContentsBoundsChange`) before carrying it out — is refused until the
//! card is seen ([`Stage::moved_by_its_page`], queue item 778), so no move
//! but alo's is ever taken for *Expand*. And the host is an accessory
//! application — no Dock icon, no place in ⌘-Tab, no tile in Stage
//! Manager's strip — until the card is seen, then an ordinary one
//! ([`Stage::listed`], `host.rs`, queue items 781 and 739).
//!
//! # Never sent to another program
//!
//! Every page's client cancels a navigation to a scheme Chromium does not
//! load itself — a `mailto:` link — before Chromium decides it
//! ([`crate::handed`], queue item 777, ADR 0060's fifth amendment): the page
//! stays as it was, where CEF's own answer committed Chrome's error page
//! over it.
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
//! # Never closed under a context menu
//!
//! A page closed while its context menu was still shown — by alo
//! (`Browser.close`, or `Target.closeTarget` then `Browser.close`), or by a
//! person's close button — ended the host with `SIGSEGV` inside CEF's
//! framework, reading memory already freed (`0xcdcd…`), once the page was
//! gone (measured, queue item 734). Every page's client counts the menus
//! CEF shows ([`Menus`]); a close asked while one is shown — [`close_all`],
//! or a window's `can_close` — is owed instead, the menu dismissed as a
//! person's Escape dismisses it (`dismissed_mac.rs`), and the close carried
//! out in a task of its own once CEF says the menu is gone, or after
//! [`GIVE_UP_AFTER`] milliseconds whatever it says (`after_menus.rs`).

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

use crate::after_menus::{Closing, GIVE_UP_AFTER, Now, Owed};
use crate::parked::Stage;
use cef::{
    App, Browser, BrowserProcessHandler, BrowserSettings, BrowserView, BrowserViewDelegate,
    CefString, ChromeToolbarType, Client, CommandHandler, ContextMenuHandler, ContextMenuParams,
    Display, DisplayHandler, Frame, ImplApp, ImplBrowser, ImplBrowserHost,
    ImplBrowserProcessHandler, ImplBrowserView, ImplBrowserViewDelegate, ImplClient,
    ImplCommandHandler, ImplContextMenuHandler, ImplDisplay, ImplDisplayHandler,
    ImplLifeSpanHandler, ImplPanel, ImplPanelDelegate, ImplRequest, ImplRequestHandler, ImplTask,
    ImplView, ImplViewDelegate, ImplWindow, ImplWindowDelegate, LifeSpanHandler, MenuModel, Rect,
    Request, RequestHandler, RuntimeStyle, Size, Task, ThreadId, View, Window, WindowDelegate,
    WindowOpenDisposition, WrapApp, WrapBrowserProcessHandler, WrapBrowserViewDelegate, WrapClient,
    WrapCommandHandler, WrapContextMenuHandler, WrapDisplayHandler, WrapLifeSpanHandler,
    WrapRequestHandler, WrapTask, WrapWindowDelegate, browser_view_create,
    browser_view_get_for_browser, display_get_alls, display_get_count, post_delayed_task,
    post_task, window_create_top_level, wrap_app, wrap_browser_process_handler,
    wrap_browser_view_delegate, wrap_client, wrap_command_handler, wrap_context_menu_handler,
    wrap_display_handler, wrap_life_span_handler, wrap_request_handler, wrap_task,
    wrap_window_delegate,
};
use std::cell::{Cell, RefCell};
use std::os::raw::c_int;
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
/// once CEF is ready, when there is one — unseen and parked when it is a
/// `card`'s.
pub fn app(page: Option<String>, card: bool) -> App {
    Host::new(page, card)
}

wrap_app! {
    struct Host {
        page: Option<String>,
        card: bool,
    }

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(Opener::new(self.page.clone(), self.card))
        }
    }
}

wrap_browser_process_handler! {
    struct Opener {
        page: Option<String>,
        card: bool,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            if let Some(page) = &self.page {
                open(page, self.card);
            }
        }

        fn default_client(&self) -> Option<Client> {
            // Every page Chromium opens of its own — `Target.createTarget`,
            // a popup — is counted as the host's own are, so that
            // [`close_all`] closes it too. None is a card's window.
            Some(Page::new(std::rc::Rc::new(Cell::new(None))))
        }
    }
}

/// Open `page` in a window of its own, Chrome style, with no toolbar —
/// unseen and parked when it is a `card`'s — or say why not, and open
/// nothing.
fn open(page: &str, card: bool) {
    // Where a card's window is in its life, shared by the window, which
    // follows it, and the page's client, which refuses the page's own moves
    // while it is unseen.
    let stage = std::rc::Rc::new(Cell::new(card.then_some(Stage::Opened)));
    let mut client = Page::new(std::rc::Rc::clone(&stage));
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
    let mut framed = Framed::new(view, stage);
    if window_create_top_level(Some(&mut framed)).is_none() {
        eprintln!("alo's CEF host: CEF made no window for the page");
    }
}

wrap_client! {
    struct Page {
        // Where its window is, when it is a card's (`Framed`'s own).
        stage: std::rc::Rc<Cell<Option<Stage>>>,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(Titled::new(std::rc::Rc::clone(&self.stage)))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(Counted::new())
        }

        fn command_handler(&self) -> Option<CommandHandler> {
            Some(Commanded::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(Kept::new())
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(Menus::new())
        }
    }
}

wrap_context_menu_handler! {
    struct Menus {}

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            _model: Option<&mut MenuModel>,
        ) {
            // Shown as Chromium shows it; counted, so no page closes under
            // it (`after_menus.rs`, queue item 734).
            CLOSING.with_borrow_mut(Closing::shown);
        }

        fn on_context_menu_dismissed(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>) {
            let owed = CLOSING.with_borrow_mut(Closing::dismissed);
            // Carried out in a task of its own, once the menu's own has
            // returned, never inside it.
            if !owed.is_empty() {
                let _ = post_task(ThreadId::UI, Some(&mut Carried::new(owed)));
            }
        }
    }
}

wrap_task! {
    struct Carried {
        owed: Owed,
    }

    impl Task {
        fn execute(&self) {
            carry_out(&self.owed);
        }
    }
}

wrap_task! {
    struct GivenUp {}

    impl Task {
        fn execute(&self) {
            let owed = CLOSING.with_borrow_mut(Closing::given_up);
            carry_out(&owed);
        }
    }
}

/// A close owed while a menu was shown, carried out: every page closed, or
/// each window a person asked to close closed as its close button would.
fn carry_out(owed: &Owed) {
    if owed.all {
        close_all();
    }
    for identifier in &owed.windows {
        let browser = OPEN.with_borrow(|open| {
            open.iter()
                .find(|browser| browser.identifier() == *identifier)
                .cloned()
        });
        if let Some(window) = browser
            .and_then(|mut browser| browser_view_get_for_browser(Some(&mut browser)))
            .and_then(|view| view.window())
            .filter(|window| window.is_closed() == 0)
        {
            window.close();
        }
    }
}

/// `now` said a close is owed while a menu is shown: on the first owed,
/// the menu is dismissed, and the close waits for it at most
/// [`GIVE_UP_AFTER`].
fn wait_for_the_menu(now: Now) {
    if now == (Now::Owed { first: true }) {
        let _ = crate::dismissed_mac::escape();
        let _ = post_delayed_task(ThreadId::UI, Some(&mut GivenUp::new()), GIVE_UP_AFTER);
    }
}

wrap_request_handler! {
    struct Kept {}

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // A scheme Chromium would hand to another program — a link to
            // `mailto:` — is never asked for: cancelled here, before
            // Chromium decides it, the frame stays at the page it was at,
            // where CEF's own answer committed its error page over it
            // (`handed.rs`, queue item 777). Cancelled, CEF tells
            // `ERR_ABORTED`, which commits nothing.
            let address = request.map(|request| CefString::from(&request.url()).to_string());
            c_int::from(address.is_some_and(|address| crate::handed::cancelled(&address)))
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

    /// What waits for a context menu to go before it closes (queue item
    /// 734).
    static CLOSING: RefCell<Closing> = RefCell::new(Closing::default());
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
    // Never under a context menu shown: it is dismissed first, and this is
    // done again once it has gone (queue item 734).
    let now = CLOSING.with_borrow_mut(Closing::close_all);
    if now != Now::Close {
        wait_for_the_menu(now);
        return;
    }
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
    struct Titled {
        // Where its window is, when it is a card's (`Framed`'s own).
        stage: std::rc::Rc<Cell<Option<Stage>>>,
    }

    impl DisplayHandler {
        fn on_contents_bounds_change(
            &self,
            _browser: Option<&mut Browser>,
            _new_bounds: Option<&Rect>,
        ) -> ::std::os::raw::c_int {
            // A page's own `moveTo`, `moveBy`, `resizeTo` or `resizeBy`
            // (CEF's word for each). A card's window is alo's alone to move
            // until it is seen: said handled, nothing is done, and the
            // window stays where alo parked it (queue item 778). Otherwise
            // CEF's own handling moves it, as Chrome moves an `--app`
            // window its page asks to.
            let refused = self
                .stage
                .get()
                .is_some_and(|stage| !stage.moved_by_its_page());
            c_int::from(refused)
        }

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

/// Where every display is, as CEF gives them: in the screen's points, the
/// main display's top left corner at the origin, as a window's bounds are.
fn displays() -> Vec<crate::parked::Area> {
    // CEF fills as many as the list holds, so it is made as long as CEF
    // says there are.
    let mut all: Vec<Option<Display>> = std::iter::repeat_with(|| None)
        .take(display_get_count())
        .collect();
    display_get_alls(Some(&mut all));
    all.iter()
        .flatten()
        .map(|display| area(&display.bounds()))
        .collect()
}

/// A rectangle of CEF's as the host's own.
fn area(rect: &Rect) -> crate::parked::Area {
    crate::parked::Area {
        left: rect.x,
        top: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

wrap_window_delegate! {
    struct Framed {
        view: BrowserView,
        // Where a card's window is in its life — unseen until it is seen
        // (the module's words) — or nothing for a window that is no card's.
        stage: std::rc::Rc<Cell<Option<Stage>>>,
    }

    impl ViewDelegate {}

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else {
                return;
            };
            if self.stage.get().is_some() {
                // Unseen before it is ever shown, so it is never drawn.
                crate::unseen_mac::unseen(window.window_handle(), true);
            }
            window.center_window(Some(&OPENED));
            window.show();
            window.add_child_view(Some(&mut View::from(&self.view)));
            self.view.request_focus();
        }

        fn can_close(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            let Some(browser) = self.view.browser() else {
                return 1;
            };
            // Never under a context menu shown: it is dismissed first, and
            // the window closed again once it has gone (queue item 734).
            let now = CLOSING.with_borrow_mut(|closing| closing.close_window(browser.identifier()));
            if now != Now::Close {
                wait_for_the_menu(now);
                return 0;
            }
            // Its page is asked first, as Chrome asks one, and the window
            // closes once the page has.
            browser.host().map_or(1, |host| host.try_close_browser())
        }

        fn on_window_bounds_changed(&self, window: Option<&mut Window>, new_bounds: Option<&Rect>) {
            let (Some(window), Some(bounds)) = (window, new_bounds) else {
                return;
            };
            let Some(stage) = self.stage.get() else {
                return;
            };
            let after = stage.after(crate::parked::placed(area(bounds), &displays()));
            self.stage.set(Some(after));
            if stage != Stage::Seen && after == Stage::Seen {
                crate::unseen_mac::unseen(window.window_handle(), false);
                crate::unseen_mac::listed(after.listed());
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

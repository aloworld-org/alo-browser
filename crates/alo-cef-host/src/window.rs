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
//! Nor does a card's page show a window of Chromium's own beside it while
//! it is unseen (queue item 808): its browser is made with no status
//! bubble ([`settings`]), and a permission prompt is dismissed unshown
//! until the card is seen ([`Stage::prompts`]).
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
//! # With no window at all: the spike of queue item 959
//!
//! Told `--alo-windowless=WIDTHxHEIGHT@SCALE` beside `--app=`
//! ([`crate::switches::windowless`]), which alo never gives, the host opens
//! the page as a **windowless** browser ([`open_windowless`], ADR 0113,
//! proposed): no window is made or placed, CEF asks the page's render
//! handler ([`Drawn`]) its view's size and scale, and says each paint, which
//! the spike says on the error output without reading a pixel
//! (`windowless.rs`). The page's client is a parked card's for its life
//! ([`Stage::Parked`]): what it tells is told, prompts dismissed, its own
//! moves refused. CEF makes such a browser Alloy style, where every other
//! page here is Chrome style — so a popup it opens is **never made**:
//! `OnBeforePopup` cancels it and tells alo a person's ([`Counted`],
//! `windowless.rs`, queue item 978), where made it stalled ADR 0060's hold
//! or escaped it, and kept the host from ending (measured, 959).
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
use crate::told::{self, Told};
use crate::windowless::Viewport;
use cef::{
    App, Browser, BrowserProcessHandler, BrowserSettings, BrowserView, BrowserViewDelegate,
    CefString, ChromeToolbarType, Client, CommandHandler, ContextMenuHandler, ContextMenuParams,
    CursorInfo, CursorType, DictionaryValue, Display, DisplayHandler, DragData, DragOperationsMask,
    Frame, ImplApp, ImplBrowser, ImplBrowserHost, ImplBrowserProcessHandler, ImplBrowserView,
    ImplBrowserViewDelegate, ImplClient, ImplCommandHandler, ImplContextMenuHandler,
    ImplContextMenuParams, ImplDisplay, ImplDisplayHandler, ImplLifeSpanHandler, ImplPanel,
    ImplPanelDelegate, ImplPermissionHandler, ImplPermissionPromptCallback, ImplRenderHandler,
    ImplRequest, ImplRequestHandler, ImplRunContextMenuCallback, ImplTask, ImplView,
    ImplViewDelegate, ImplWindow, ImplWindowDelegate, LifeSpanHandler, MenuModel, PaintElementType,
    PermissionHandler, PermissionPromptCallback, PermissionRequestResult, PopupFeatures, Rect,
    RenderHandler, Request, RequestHandler, RunContextMenuCallback, RuntimeStyle, ScreenInfo, Size,
    State, Task, ThreadId, View, Window, WindowDelegate, WindowInfo, WindowOpenDisposition,
    WrapApp, WrapBrowserProcessHandler, WrapBrowserViewDelegate, WrapClient, WrapCommandHandler,
    WrapContextMenuHandler, WrapDisplayHandler, WrapLifeSpanHandler, WrapPermissionHandler,
    WrapRenderHandler, WrapRequestHandler, WrapTask, WrapWindowDelegate,
    browser_host_create_browser, browser_view_create, browser_view_get_for_browser,
    display_get_alls, display_get_count, post_delayed_task, post_task, window_create_top_level,
    wrap_app, wrap_browser_process_handler, wrap_browser_view_delegate, wrap_client,
    wrap_command_handler, wrap_context_menu_handler, wrap_display_handler, wrap_life_span_handler,
    wrap_permission_handler, wrap_render_handler, wrap_request_handler, wrap_task,
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
/// `card`'s, or with no window at all at the viewport it is opened
/// `windowless` at (the spike, [`crate::windowless`]).
pub fn app(page: Option<String>, card: bool, windowless: Option<Viewport>) -> App {
    Host::new(page, card, windowless)
}

wrap_app! {
    struct Host {
        page: Option<String>,
        card: bool,
        windowless: Option<Viewport>,
    }

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(Opener::new(self.page.clone(), self.card, self.windowless))
        }
    }
}

wrap_browser_process_handler! {
    struct Opener {
        page: Option<String>,
        card: bool,
        windowless: Option<Viewport>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            match (&self.page, self.windowless) {
                (Some(page), Some(viewport)) => open_windowless(page, viewport),
                (Some(page), None) => open(page, self.card),
                (None, _) => {}
            }
        }

        fn default_client(&self) -> Option<Client> {
            // Every page Chromium opens of its own — `Target.createTarget`,
            // a popup — is counted as the host's own are, so that
            // [`close_all`] closes it too. None is a card's window.
            Some(Page::new(std::rc::Rc::new(Cell::new(None)), None))
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
    let mut client = Page::new(std::rc::Rc::clone(&stage), None);
    let mut viewed = Viewed::new();
    let Some(view) = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(page)),
        Some(&settings(card)),
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

/// Open `page` with no window at all, drawn at `viewport` through CEF's
/// windowless rendering (the spike, [`crate::windowless`]), or say why not.
/// The page is a card's that is never seen: what it tells is told to alo,
/// its permission prompts dismissed and its own moves refused, as a parked
/// card's are ([`Stage::Parked`], for its life — there is no window to
/// place: *Expand* is alo's, the view told a new size, queue item 979).
/// CEF makes it Alloy style whatever is asked (`cef_types_runtime.h`), and
/// gives it no parent view: none is needed to paint. What alo tells of its
/// view is heard from now on ([`listen_to_alo`]).
fn open_windowless(page: &str, viewport: Viewport) {
    VIEW.set(Some(viewport));
    listen_to_alo();
    let stage = std::rc::Rc::new(Cell::new(Some(Stage::Parked)));
    let mut client = Page::new(stage, Some(viewport));
    let info = WindowInfo::default().set_as_windowless(std::ptr::null_mut());
    let settings = BrowserSettings {
        windowless_frame_rate: crate::windowless::FRAME_RATE,
        background_color: crate::windowless::GROUND,
        ..settings(true)
    };
    if browser_host_create_browser(
        Some(&info),
        Some(&mut client),
        Some(&CefString::from(page)),
        Some(&settings),
        None,
        None,
    ) != 1
    {
        eprintln!("alo's CEF host: CEF made no windowless browser for the page");
    }
}

/// Hear what alo tells of the windowless page's view on a thread of its
/// own (`heard.rs`), each line alo may tell carried out on CEF's UI thread
/// ([`Applied`]) — or nothing, when descriptor 5 cannot be read: it is a
/// pipe, alo telling nothing, or it is not open at all.
fn listen_to_alo() {
    let Ok(from) = std::fs::File::open(told::DESCRIPTOR) else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("heard from alo".to_owned())
        .spawn(move || {
            crate::heard::listen(from, |heard| {
                // Said nowhere when it fails: CEF is shutting down.
                let _ = post_task(ThreadId::UI, Some(&mut Applied::new(heard)));
            });
        });
    if let Err(why) = spawned {
        eprintln!("alo's CEF host: windowless: nothing heard from alo: {why}");
    }
}

wrap_task! {
    struct Applied {
        heard: crate::heard::Heard,
    }

    impl Task {
        fn execute(&self) {
            apply(self.heard);
        }
    }
}

/// Carry out what alo told of the windowless page's view, on CEF's UI
/// thread: a new size resized (`WasResized`, CEF then asks [`Drawn`] the
/// view again), a new scale told as the screen's (`NotifyScreenInfoChanged`)
/// first; hidden or shown (`WasHidden`); the focus given or taken
/// (`SetFocus`). Nothing before the page is made, or after it is closed.
fn apply(heard: crate::heard::Heard) {
    let Some(host) = DRAWN_PAGE.with_borrow(|page| page.as_ref().and_then(ImplBrowser::host))
    else {
        return;
    };
    match heard {
        crate::heard::Heard::View(after) => {
            let Some(before) = VIEW.replace(Some(after)) else {
                return;
            };
            let (rescaled, resized) = crate::heard::changes(before, after);
            if rescaled {
                host.notify_screen_info_changed();
            }
            if resized {
                host.was_resized();
            }
        }
        crate::heard::Heard::Hidden(hidden) => host.was_hidden(c_int::from(hidden)),
        crate::heard::Heard::Focused(focused) => host.set_focus(c_int::from(focused)),
    }
}

wrap_render_handler! {
    struct Drawn {
        // The view it was opened at, until alo tells another ([`VIEW`]).
        viewport: Viewport,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            // The page's view, in CSS pixels, at the origin: nothing of it is
            // anywhere on a screen.
            let viewport = VIEW.get().unwrap_or(self.viewport);
            if let Some(rect) = rect {
                *rect = Rect {
                    x: 0,
                    y: 0,
                    width: viewport.width,
                    height: viewport.height,
                };
            }
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            // The scale the page is drawn at is the viewport's, never a
            // display's: there is no display under it.
            let Some(screen_info) = screen_info else {
                return 0;
            };
            let viewport = VIEW.get().unwrap_or(self.viewport);
            let whole = Rect {
                x: 0,
                y: 0,
                width: viewport.width,
                height: viewport.height,
            };
            screen_info.device_scale_factor = viewport.scale;
            screen_info.depth = 24;
            screen_info.depth_per_component = 8;
            screen_info.rect = whole.clone();
            screen_info.available_rect = whole;
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            _buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            // The pixels are behind the pointer, which the spike never reads
            // (`windowless.rs`): the paint is said, for its cadence and size.
            let what = if type_ == PaintElementType::POPUP {
                crate::windowless::Painted::Popup
            } else {
                crate::windowless::Painted::View
            };
            let dirty = dirty_rects.map_or(0, <[Rect]>::len);
            eprintln!(
                "{}",
                crate::windowless::painted(crate::windowless::now(), what, width, height, dirty)
            );
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            let what = if show != 0 { "popup shown" } else { "popup hidden" };
            eprintln!("{}", crate::windowless::said(crate::windowless::now(), what));
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            if let Some(rect) = rect {
                let what = format!(
                    "popup at {} {} {}x{}",
                    rect.x, rect.y, rect.width, rect.height
                );
                eprintln!("{}", crate::windowless::said(crate::windowless::now(), &what));
            }
        }

        fn start_dragging(
            &self,
            _browser: Option<&mut Browser>,
            _drag_data: Option<&mut DragData>,
            _allowed_ops: DragOperationsMask,
            _x: ::std::os::raw::c_int,
            _y: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // Said, and refused: CEF then ends the drag as cancelled. A
            // windowless page has nowhere of its own to drag to.
            eprintln!("{}", crate::windowless::said(crate::windowless::now(), "drag refused"));
            0
        }
    }
}

/// The settings a page's browser is made with: for a `card`'s, **no
/// status bubble**. Measured through this host (queue item 808): the pointer
/// over a link of a parked card's page showed Chromium's status bubble — the
/// link's address — as a window of its own, opaque, at the parked window's
/// bottom left corner (1669, 565, 292 × 22, then 511 wide), its first 41
/// points on the display the window was parked by. The parked window's
/// transparency is its own and not its bubble's, and Chromium fades the
/// bubble in and out by its own transparency each time it shows it, so the
/// bubble is not made for a card at all — for the card's life, since a
/// browser's settings are made once: an expanded card shows no address
/// under a link either. Every other page keeps Chromium's own.
fn settings(card: bool) -> BrowserSettings {
    BrowserSettings {
        chrome_status_bubble: if card {
            State::DISABLED
        } else {
            State::DEFAULT
        },
        ..BrowserSettings::default()
    }
}

wrap_client! {
    struct Page {
        // Where its window is, when it is a card's (`Framed`'s own).
        stage: std::rc::Rc<Cell<Option<Stage>>>,
        // The viewport it is drawn at when it has no window (the spike).
        windowless: Option<Viewport>,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            self.windowless.map(Drawn::new)
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(Titled::new(std::rc::Rc::clone(&self.stage)))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(Counted::new(self.windowless.is_some()))
        }

        fn command_handler(&self) -> Option<CommandHandler> {
            Some(Commanded::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(Kept::new(std::rc::Rc::clone(&self.stage)))
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(Menus::new(std::rc::Rc::clone(&self.stage)))
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(Asked::new(std::rc::Rc::clone(&self.stage)))
        }
    }
}

wrap_permission_handler! {
    struct Asked {
        // Where its window is, when it is a card's (`Framed`'s own).
        stage: std::rc::Rc<Cell<Option<Stage>>>,
    }

    impl PermissionHandler {
        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            _prompt_id: u64,
            _requesting_origin: Option<&CefString>,
            _requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            // A page's permission prompt is a window of Chromium's own
            // beside the page's, opaque (measured: 320 × 266 at the parked
            // window's top left corner, queue item 808). While the card is
            // unseen it is never shown: dismissed, as a person closing it
            // unanswered does, which grants nothing and keeps nothing.
            // Otherwise Chromium shows it, as Chrome does.
            let unseen = self.stage.get().is_some_and(|stage| !stage.prompts());
            match callback {
                Some(callback) if unseen => {
                    callback.cont(PermissionRequestResult::DISMISS);
                    1
                }
                _ => 0,
            }
        }
    }
}

wrap_context_menu_handler! {
    struct Menus {
        // Where its window is, when it is a card's (`Framed`'s own).
        stage: std::rc::Rc<Cell<Option<Stage>>>,
    }

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

        fn run_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            _model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> ::std::os::raw::c_int {
            // A card's page, unseen: no menu of Chromium's is shown — it
            // would be a window beside the parked one, and the host would
            // crash under it if the card closed (734) — and alo is told
            // where the press was and what it was over, and draws its own
            // menu over the card (queue item 798). The menu is cancelled at
            // once, so CEF says it is dismissed. Seen, Chromium's own.
            let unseen = self.stage.get().is_some_and(Stage::tells);
            let (Some(params), Some(callback), true) = (params, callback, unseen) else {
                return 0;
            };
            told::tell(&Told::Menu {
                x: params.xcoord(),
                y: params.ycoord(),
                link: CefString::from(&params.link_url()).to_string(),
                selection: CefString::from(&params.selection_text()).to_string(),
                editable: params.is_editable() != 0,
            });
            callback.cancel();
            1
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
    struct Kept {
        // Where its window is, when it is a card's (`Framed`'s own).
        stage: std::rc::Rc<Cell<Option<Stage>>>,
    }

    impl RequestHandler {
        fn on_open_urlfrom_tab(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            target_disposition: WindowOpenDisposition,
            user_gesture: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // A link a person pressed with ⌘, or with the middle button, on
            // an unseen card's page: Chromium would open it in a tab or a
            // window of its own, which no card shows. It is cancelled here
            // and told to alo, which opens it as the conversation's next
            // turn (queue item 798). Only a person's, and only one asked to
            // go somewhere new; seen, Chromium's own.
            let unseen = self.stage.get().is_some_and(Stage::tells);
            let elsewhere = elsewhere(target_disposition);
            match target_url {
                Some(address) if unseen && elsewhere && user_gesture != 0 => {
                    told::tell(&Told::Open(address.to_string()));
                    1
                }
                _ => 0,
            }
        }

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

    /// The windowless page's view as alo last told it (queue item 979), or
    /// as it was opened at: what [`Drawn`] answers CEF.
    static VIEW: Cell<Option<Viewport>> = const { Cell::new(None) };

    /// The windowless page, while it is open: the one what alo tells of a
    /// view is carried out on ([`apply`]).
    static DRAWN_PAGE: RefCell<Option<Browser>> = const { RefCell::new(None) };
}

wrap_life_span_handler! {
    struct Counted {
        // Whether its page has no window (`open_windowless`), so that a
        // popup it opens is never made (queue item 978).
        windowless: bool,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            // A windowless page's popup — `window.open`, a link to another
            // window — is cancelled before Chromium makes it, and a
            // person's told to alo, which opens it as the conversation's
            // next turn (`windowless.rs`, queue item 978): made in Alloy
            // style, it stalled under ADR 0060's hold or left unheld, and
            // its host never ended (measured, 959). A page in a window
            // makes its own, which alo holds and closes over the pipe.
            if !self.windowless {
                return 0;
            }
            let address = target_url.map(CefString::to_string);
            if let Some(told) = crate::windowless::popup(address, user_gesture != 0) {
                told::tell(&told);
            }
            1
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                OPEN.with_borrow_mut(|open| open.push(browser.clone()));
                if self.windowless {
                    DRAWN_PAGE.set(Some(browser.clone()));
                }
            }
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            let identifier = browser.identifier();
            if self.windowless {
                DRAWN_PAGE.set(None);
            }
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
        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: *mut u8,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            // The pointer's shape the page asks for, on an unseen card's
            // page: told to alo, whose window the pointer is over (queue
            // item 799), and kept from Chromium, whose window is not.
            // Seen, Chromium's own.
            if self.stage.get().is_some_and(Stage::tells) {
                told::tell(&Told::Cursor(shape(type_)));
                1
            } else {
                0
            }
        }

        fn on_status_message(&self, browser: Option<&mut Browser>, value: Option<&CefString>) {
            // The address of a link under the pointer, or nothing: shown in
            // the host's own strip once the card is seen (queue item 811).
            if self.stage.get().is_some_and(Stage::strips) {
                let said = value.map(ToString::to_string);
                show_strip(browser, crate::strip::says(said.as_deref()));
            }
        }

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

/// The pointer's shape `kind` asks for, by its name in CSS, as alo is told
/// it (queue item 799): every shape CEF names that CSS names too; a page's
/// own picture (`CT_CUSTOM`) and a drag's shapes are told as the arrow,
/// which is what alo draws for them.
fn shape(kind: CursorType) -> &'static str {
    let named: [(CursorType, &'static str); 43] = [
        (CursorType::CROSS, "crosshair"),
        (CursorType::HAND, "pointer"),
        (CursorType::IBEAM, "text"),
        (CursorType::WAIT, "wait"),
        (CursorType::HELP, "help"),
        (CursorType::EASTRESIZE, "e-resize"),
        (CursorType::NORTHRESIZE, "n-resize"),
        (CursorType::NORTHEASTRESIZE, "ne-resize"),
        (CursorType::NORTHWESTRESIZE, "nw-resize"),
        (CursorType::SOUTHRESIZE, "s-resize"),
        (CursorType::SOUTHEASTRESIZE, "se-resize"),
        (CursorType::SOUTHWESTRESIZE, "sw-resize"),
        (CursorType::WESTRESIZE, "w-resize"),
        (CursorType::NORTHSOUTHRESIZE, "ns-resize"),
        (CursorType::EASTWESTRESIZE, "ew-resize"),
        (CursorType::NORTHEASTSOUTHWESTRESIZE, "nesw-resize"),
        (CursorType::NORTHWESTSOUTHEASTRESIZE, "nwse-resize"),
        (CursorType::COLUMNRESIZE, "col-resize"),
        (CursorType::ROWRESIZE, "row-resize"),
        (CursorType::MIDDLEPANNING, "all-scroll"),
        (CursorType::EASTPANNING, "e-resize"),
        (CursorType::NORTHPANNING, "n-resize"),
        (CursorType::NORTHEASTPANNING, "ne-resize"),
        (CursorType::NORTHWESTPANNING, "nw-resize"),
        (CursorType::SOUTHPANNING, "s-resize"),
        (CursorType::SOUTHEASTPANNING, "se-resize"),
        (CursorType::SOUTHWESTPANNING, "sw-resize"),
        (CursorType::WESTPANNING, "w-resize"),
        (CursorType::MOVE, "move"),
        (CursorType::VERTICALTEXT, "vertical-text"),
        (CursorType::CELL, "cell"),
        (CursorType::CONTEXTMENU, "context-menu"),
        (CursorType::ALIAS, "alias"),
        (CursorType::PROGRESS, "progress"),
        (CursorType::NODROP, "no-drop"),
        (CursorType::COPY, "copy"),
        (CursorType::NONE, "none"),
        (CursorType::NOTALLOWED, "not-allowed"),
        (CursorType::ZOOMIN, "zoom-in"),
        (CursorType::ZOOMOUT, "zoom-out"),
        (CursorType::GRAB, "grab"),
        (CursorType::GRABBING, "grabbing"),
        (CursorType::MIDDLE_PANNING_VERTICAL, "ns-resize"),
    ];
    named
        .iter()
        .find(|(each, _)| *each == kind)
        .map_or("default", |(_, name)| *name)
}

/// Whether `disposition` opens a page somewhere other than where it was
/// asked from — a tab or a window of its own — which a card never shows.
fn elsewhere(disposition: WindowOpenDisposition) -> bool {
    [
        WindowOpenDisposition::NEW_FOREGROUND_TAB,
        WindowOpenDisposition::NEW_BACKGROUND_TAB,
        WindowOpenDisposition::NEW_WINDOW,
        WindowOpenDisposition::NEW_POPUP,
        WindowOpenDisposition::SINGLETON_TAB,
        WindowOpenDisposition::SWITCH_TO_TAB,
    ]
    .contains(&disposition)
}

/// Show `says` in the strip of the window holding `browser`, or hide it
/// with nothing to say (`strip.rs`, `strip_mac.rs`).
fn show_strip(browser: Option<&mut Browser>, says: Option<&str>) {
    if let Some(window) = browser_view_get_for_browser(browser).and_then(|view| view.window()) {
        crate::strip_mac::show(window.window_handle(), says);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cards_browser_has_no_status_bubble_and_every_other_has_chromiums() {
        assert_eq!(settings(true).chrome_status_bubble, State::DISABLED);
        assert_eq!(settings(false).chrome_status_bubble, State::DEFAULT);
        // Nothing else is changed from CEF's own.
        assert_eq!(
            settings(true).chrome_zoom_bubble,
            BrowserSettings::default().chrome_zoom_bubble
        );
    }

    #[test]
    fn the_pointers_shape_is_told_by_its_name_in_css() {
        assert_eq!(shape(CursorType::HAND), "pointer");
        assert_eq!(shape(CursorType::IBEAM), "text");
        assert_eq!(shape(CursorType::POINTER), "default");
        assert_eq!(shape(CursorType::NOTALLOWED), "not-allowed");
        assert_eq!(shape(CursorType::CUSTOM), "default", "a page's own picture");
        assert_eq!(shape(CursorType::DND_COPY), "default");
    }

    #[test]
    fn only_a_link_asked_to_open_elsewhere_is_told() {
        assert!(elsewhere(WindowOpenDisposition::NEW_BACKGROUND_TAB));
        assert!(elsewhere(WindowOpenDisposition::NEW_FOREGROUND_TAB));
        assert!(elsewhere(WindowOpenDisposition::NEW_WINDOW));
        assert!(!elsewhere(WindowOpenDisposition::CURRENT_TAB));
        assert!(!elsewhere(WindowOpenDisposition::SAVE_TO_DISK));
    }
}

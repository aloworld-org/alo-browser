/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The window and its event loop: the one file that may name `winit` (ADR
//! 0024 § 1).
//!
//! # What the event loop does, and what it never does
//!
//! It turns what the window server says into [`Order`]s for the conductor,
//! takes in the conductor's [`News`], and draws the window from what it has
//! been sent ([`crate::showing`], [`crate::compose`]). It **never calls a
//! renderer** and never waits on the conductor (ADR 0024 § 2): every exchange
//! with a renderer is on the conductor's thread, and what it learnt arrives
//! here through `winit`'s own proxy, as an event like any other.
//!
//! # A person's pointer
//!
//! Items 535 and 542, ADR 0024 § 5. The window keeps where the cursor last
//! was and decides from its own composition what it is over
//! ([`crate::aim`]); [`crate::cursor`] decides what the page is sent of it
//! — its moves over the page and its leaving, a press of the **main
//! button** over the page and the moves and release that follow — each in
//! the page's CSS pixels as [`Order::Pointer`]. A press over the strip is
//! sent as [`Order::StripPressed`], in the strip's CSS pixels, and chooses
//! the tab its renderer claims (item 539); over a line the window is saying
//! something in it is nobody's.
//!
//! Not sent yet: the other buttons (544), a press with a modifier held
//! (545) — ⌘ held over a link opens a tab in Chrome, which no tab here does
//! — and how many presses came in quick succession (547). A move is sent
//! whatever modifier is held, and reports none (545). The window shows the
//! tab strip across its top and the selected tab below it (item 528).
//!
//! # The keyboard
//!
//! Read here, before anything else could see it, and only for the browser's
//! own keys (item 529, ADR 0024 § 5): each press is made a
//! [`crate::keys::Press`] and [`crate::keys::order`] says whether it is one,
//! and which. No key goes anywhere else, because a page has no keyboard yet
//! (item 258).
//!
//! # The title
//!
//! The window is called "alo", and never a page's title: a title is a
//! stranger's string, and the window server would shape it in this, the
//! privileged process (ADR 0024 § 4). A tab's title is the tab strip's, which
//! is rendered in a sandboxed renderer of its own.
//!
//! # The frame around Chrome's window
//!
//! ADR 0062, queue item 642. [`frame`] runs a second kind of window, for
//! `alo --chrome`: a strip of ours that **floats** — above ordinary
//! windows, so that Stage Manager does not put it in its strip when
//! Chrome's comes in front — has no title bar and is never resized by a
//! person, and stands where it is told ([`Follow`]), by the thread that
//! holds Chrome and reads where Chrome's window is. Told to stand aside it
//! is hidden; told Chrome's window is gone it closes. Closing it — or ⌘W
//! in it, the browser's key for closing the tab, which is the only one —
//! is sent to that thread, which closes Chrome's window and then tells
//! the frame to close. It is made without taking the screen from Chrome:
//! the program is not brought in front as it starts, and the window is
//! made inactive, so a person's keys go on reaching Chrome's page.
//!
//! It draws which renderer draws the page, and what agents' verbs were
//! answered with (queue item 679, [`crate::compose::compose_frame`]): told
//! each line by the same thread ([`ToFrame::Agent`]), it keeps them as this
//! engine's window does ([`AgentLines`]) and is drawn again.
//!
//! And it draws the page's tab (queue item 681). Told what Chrome calls the
//! page ([`ToFrame::Named`]), it hands a strip of that one tab to a tab
//! strip's renderer of its own, on a thread of its own
//! ([`crate::frame_strip`]), and draws what that renderer painted when it
//! is told it ([`ToFrame::Strip`]) — never the title itself, which is a
//! stranger's string this process does not shape (ADR 0024 § 4). The
//! window's own title stays "alo".
//!
//! # The conversation's window
//!
//! ADR 0074, queue item 709. [`conversation`] runs a third kind of window,
//! for `alo --conversation`: the browser's own interface as the owner's
//! design draws it, the chats panel down the left and the first screen
//! beside it, at the design's size. It holds no tabs and no conductor. Each
//! part is a document of ours drawn by a confined renderer of its own, on
//! a thread that [`crate::conversation_drawing`] holds; the window sends
//! that thread what it wants drawn at its size and scale
//! ([`crate::conversation::Conversation`]) and draws what it is told was
//! painted ([`crate::conversation_compose`]), never waiting on either
//! renderer. No key is the browser's in it, since it has no tab to open or
//! close: a key going down edits the one box on the first screen instead,
//! when a press gave it the keyboard ([`crate::typing`],
//! [`crate::conversation_box`], queue item 701). What is typed is held by
//! the browser process and sent to the first screen's renderer as data;
//! when Return or *Go* chooses, what was chosen is said in the first
//! screen's line — where it was decided to go, what is searched for, or
//! that searching or asking is not set up yet. An address chosen is opened in the machine's Chrome
//! (queue item 713, ADR 0075) by a thread [`crate::conversation_chrome`]
//! holds, never waited on, and shown as a card in the first screen drawn
//! from the frames Chrome streams (queue item 717, ADR 0076 B), streamed
//! only while this window can be seen; *Expand* puts Chrome's own window
//! over the first screen, read from where this window is
//! (`Window::outer_position`), and *Close* closes the page. What became of
//! it — opening, opened, why not, closed — is said in the first screen's
//! line as the thread tells it. Closing the window closes that Chrome,
//! waits for it and removes its profile. Words chosen are searched by a
//! third thread, [`crate::conversation_search`], with the key read from
//! this Mac's keychain (queue item 708, ADR 0083), and drawn as the All tab.
//! The text a key writes is `KeyEvent::text`, a field read like the others.
//!
//! What the window does with each of these is not here: this file reads
//! the event and hands it, as this browser's own value, to
//! [`ConversationWindow`] (`crate::conversation_window`), which a test
//! drives through the same functions (queue item 742). This file then does
//! what that asks of the window — draw again, say a refused press on the
//! terminal, and take the front back from a card's Chromium that took it
//! as it showed its parked window (`Window::focus_window`).
//!
//! A press of the main button with no modifier held is aimed from the same
//! composition ([`crate::conversation_aim`]) and handed to that thread, in
//! the part's CSS pixels, to be claimed by the renderer that drew the part
//! and decided by the browser process (queue item 710); what was decided is
//! acted on ([`crate::conversation::Conversation::act`]) and the parts drawn
//! again when it changed what they show. A refusal, or a renderer that
//! could not answer, is said in that part's line. Its release, and the
//! cursor's moves, are nobody's: neither part runs script.
//!
//! **Over the card's page** (queue item 718, ADR 0024 § 5) a point is the
//! page's, not a part's: the window decides it from its own composition and
//! the page box it computes ([`crate::card_aim`]) while the card shows its
//! picture, and sends the press — then the moves while it is held, and the
//! release wherever it comes — to the page in the page's CSS pixels, as a
//! person's input through the thread holding its Chrome
//! ([`crate::card_input`], [`ChromeOpener::person`]). The press gives the
//! card's page the keyboard and takes it from the box; a press anywhere
//! else takes it back ([`Conversation::press_the_page`]). While the page
//! has it, every key goes to the page as [`crate::card_keys`] says, and
//! none to the box. A wheel or a trackpad turned over the card's page
//! scrolls the page; anywhere else it is nobody's, since neither part
//! scrolls. Nothing an agent sends becomes any of these (ADR 0002).
//!
//! # No `unsafe`
//!
//! Every `winit` function called here — `EventLoop::with_user_event`,
//! `EventLoopBuilder::build`, `EventLoop::create_proxy`, `EventLoop::run_app`,
//! `EventLoopProxy::send_event`, `ActiveEventLoop::create_window`,
//! `ActiveEventLoop::exit`, `Window::inner_size`, `Window::scale_factor`,
//! `Window::request_redraw`, `Window::outer_position`, `Window::has_focus`,
//! `Window::focus_window` (queue item 742), `Modifiers::state`,
//! `ModifiersState`'s `super_key`, `control_key`, `shift_key` and
//! `alt_key`, and
//! `KeyEventExtModifierSupplement::key_without_modifiers` — is a safe
//! function in `winit` 0.30.13's source, which the commits adding them
//! checked (ADR 0024 § 1's stop rule). The pointer adds no call: it reads
//! the fields of `WindowEvent::CursorMoved`, `CursorLeft`, `MouseInput` and
//! `MouseWheel`,
//! and `ModifiersState::is_empty`, a safe `bitflags` function. The frame
//! adds `WindowAttributes::with_decorations`, `with_window_level`,
//! `with_visible`, `with_resizable` and `with_active`,
//! `Window::set_outer_position`, `request_inner_size` and `set_visible`, and
//! on macOS `EventLoopBuilderExtMacOS::with_activate_ignoring_other_apps`,
//! each a safe function in 0.30.13's source as checked by item 642 (ADR
//! 0062's consequences), and the conversation's window adds
//! `WindowAttributes::with_min_inner_size` (queue item 775), a safe setter
//! of a field in the same source; `with_parent_window`, the one `unsafe fn` beside
//! them, is not called. Quitting the program adds no call either: the
//! frame's shutdown is run in `ApplicationHandler::exiting`, a method of
//! ours that `winit`'s own application delegate calls from
//! `applicationWillTerminate:` (item 688).

use crate::agent_lines::AgentLines;
use crate::ai_service::AiService;
use crate::aim::{Aimed, aim, on_page};
use crate::beside::Chromium;
use crate::card_aim::Turned;
use crate::chat_key::ChatKey;
use crate::chat_store::ChatStore;
use crate::chrome_follow::Follow;
use crate::chrome_protocol::Bounds;
use crate::chrome_protocol::Named;
use crate::closing::Closing;
use crate::compose::{FrameScene, compose, compose_frame};
use crate::conversation_agents::Reaching;
use crate::conversation_chrome::ChromeOpener;
use crate::conversation_composed::Composed;
use crate::conversation_drawing::ConversationDrawing;
use crate::conversation_fold::LEAST_SIZE;
use crate::conversation_glide::FRAME;
use crate::conversation_search::Searcher;
use crate::conversation_window::{ConversationWindow, Heard, Measured};
use crate::cursor::{Cursor, Pressed};
use crate::frame_strip::{FrameStrip, strip_of};
use crate::frame_told::ToFrame;
use crate::inbox::Orders;
use crate::keys::{self, Arrow, Held, Key, Modifier, Press};
use crate::message::{News, Order};
use crate::notice::Lettering;
use crate::place::{Placement, STRIP_HEIGHT, screen};
use crate::present::Presenter;
use crate::present_chosen::{How, Screen};
use crate::scale::Scale;
use crate::search_service::SearchService;
use crate::showing::Showing;
use alo_renderer::host::Renderers;
use alo_renderer::{Frame, Visibility};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{self as winit_keys, ModifiersState, NamedKey};
#[cfg(target_os = "macos")]
use winit::platform::macos::EventLoopBuilderExtMacOS;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{CursorIcon, Window, WindowAttributes, WindowId, WindowLevel};

/// How big a new window is, in the window server's logical points.
const FIRST_SIZE: (f64, f64) = (1000.0, 700.0);

/// How big the conversation's window opens, in logical points: the
/// design's window, 1280 by 820 (`docs/design/shell.md`).
const CONVERSATION_SIZE: (f64, f64) = (1280.0, 820.0);

/// Run the window until it is closed and every tab under it is.
///
/// The window is laid out at `forced` when a person named a scale, and at
/// its screen's otherwise ([`Scale::of_screen`]) — as Chrome's
/// `--force-device-scale-factor` lays a page out at a scale the screen is
/// not, which is how a scale this machine's screens do not have is seen.
///
/// `start` is handed the way to tell the window something — a function that
/// answers whether the window was still there to hear it — and returns where
/// to send orders. It is a function rather than a value because the way to
/// tell the window only exists once its event loop does.
///
/// # Errors
///
/// What `winit` said, in words, when there could be no event loop or no
/// window.
pub fn run(
    forced: Option<Scale>,
    start: impl FnOnce(Box<dyn Fn(News) -> bool + Send>) -> Orders,
) -> Result<(), String> {
    let lettering =
        Lettering::compiled_in().ok_or_else(|| "the window's own font is not a font".to_owned())?;
    let event_loop = EventLoop::<News>::with_user_event()
        .build()
        .map_err(|why| format!("no event loop: {why}"))?;
    let proxy = event_loop.create_proxy();
    let orders = start(Box::new(move |news| proxy.send_event(news).is_ok()));
    let mut app = App {
        orders,
        forced,
        lettering,
        showing: Showing::default(),
        window: None,
        presenter: None,
        modifiers: ModifiersState::empty(),
        cursor: None,
        pointer: Cursor::default(),
        failed: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|why| format!("the event loop stopped: {why}"))?;
    match app.failed {
        Some(why) => Err(why),
        None => Ok(()),
    }
}

/// The event loop's state.
struct App {
    orders: Orders,
    /// The scale a person named, if they named one.
    forced: Option<Scale>,
    lettering: Lettering,
    showing: Showing,
    window: Option<Rc<Window>>,
    presenter: Option<Presenter<Rc<Window>>>,
    /// The modifier keys held, as the window server last said.
    modifiers: ModifiersState,
    /// Where the cursor last was in the window, in device pixels, while it
    /// is over the window.
    cursor: Option<(f64, f64)>,
    /// What the page has been sent of the cursor ([`crate::cursor`]).
    pointer: Cursor,
    /// Why the window had to stop, if it did.
    failed: Option<String>,
}

impl App {
    /// The window's size in device pixels and its scale, if there is a
    /// window.
    fn measured(&self) -> Option<((u32, u32), Scale)> {
        self.window.as_ref().map(|window| {
            let size = window.inner_size();
            let scale = self
                .forced
                .unwrap_or_else(|| Scale::of_screen(window.scale_factor()));
            ((size.width, size.height), scale)
        })
    }

    /// Tell the conductor the size a page in this window is laid out at,
    /// and the scale of the screen it is on (ADR 0057 § 3).
    fn send_size(&self) {
        if let Some((window, scale)) = self.measured() {
            // A conductor that has gone is a window closing; there is nobody
            // left to lay anything out for.
            let _ = self.orders.send(Order::Resize(screen(window, scale)));
        }
    }

    /// Draw the window from what it has been sent.
    fn draw(&mut self) {
        let Some((window, scale)) = self.measured() else {
            return;
        };
        let canvas = compose(&self.showing.scene(window, scale), &self.lettering);
        if let Some(presenter) = self.presenter.as_mut()
            && let Err(why) = presenter.show(&canvas)
        {
            self.failed = Some(why);
        }
    }

    /// Where things are in the window now, and its scale, if there is a
    /// window.
    fn placed(&self) -> Option<((u32, u32), Placement, Scale)> {
        let (window, scale) = self.measured()?;
        let scene = self.showing.scene(window, scale);
        Some((
            window,
            Placement::of(window, scale, scene.contents()),
            scale,
        ))
    }

    /// What the cursor is over, and where it is in the page's CSS pixels,
    /// if there is a window — over nothing, at the window's corner, when the
    /// cursor has not been seen.
    fn aimed(&self) -> Option<(Aimed, alo_paint::Point)> {
        let (window, placed, scale) = self.placed()?;
        let at = self.cursor.unwrap_or_default();
        let over = self
            .cursor
            .map_or(Aimed::Nothing, |at| aim(window, &placed, scale, at));
        Some((over, on_page(&placed, scale, at)))
    }

    /// The cursor moved to `at`, in device pixels: what the page is sent of
    /// it, if anything.
    fn cursor_moved(&mut self, at: (f64, f64)) {
        self.cursor = Some(at);
        let Some((over, on_page)) = self.aimed() else {
            return;
        };
        let sent = self.pointer.moved(over, on_page);
        self.send_pointer(sent);
    }

    /// The cursor left the window: the page is told, unless a press it
    /// heard is held, whose release still reaches it from where the cursor
    /// was last seen.
    fn cursor_left(&mut self) {
        let Some((_, on_page)) = self.aimed() else {
            return;
        };
        let sent = self.pointer.left(on_page);
        if !self.pointer.is_pressing() {
            self.cursor = None;
        }
        self.send_pointer(sent);
    }

    /// The main button went `state`: a press over the page or the strip is
    /// sent, and so is the release of a press the page heard.
    fn main_button(&mut self, state: ElementState) {
        let Some((over, on_page)) = self.aimed() else {
            return;
        };
        match state {
            ElementState::Pressed => match self.pointer.pressed(over, self.modifiers.is_empty()) {
                Some(Pressed::Page(pointer)) => self.send_pointer(Some(pointer)),
                // A conductor that has gone is a window closing.
                Some(Pressed::Strip(at)) => {
                    let _ = self.orders.send(Order::StripPressed(at));
                }
                None => {}
            },
            ElementState::Released => {
                let sent = self.pointer.released(on_page);
                self.send_pointer(sent);
            }
        }
    }

    /// Send the conductor what the page is sent of the cursor, if anything.
    fn send_pointer(&self, sent: Option<alo_renderer::pointer::Pointer>) {
        if let Some(pointer) = sent {
            // A conductor that has gone is a window closing.
            let _ = self.orders.send(Order::Pointer(pointer));
        }
    }

    /// Stop showing the window and close every tab, and leave the event loop
    /// when the conductor says they are closed.
    fn close(&mut self, event_loop: &ActiveEventLoop) {
        self.presenter = None;
        self.window = None;
        if self.orders.send(Order::CloseEverything).is_err() {
            // Nobody to wait for.
            event_loop.exit();
        }
    }
}

impl ApplicationHandler<News> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("alo")
            .with_inner_size(LogicalSize::new(FIRST_SIZE.0, FIRST_SIZE.1));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(why) => {
                self.failed = Some(format!("no window: {why}"));
                event_loop.exit();
                return;
            }
        };
        match Presenter::over(Rc::clone(&window)) {
            Ok(presenter) => self.presenter = Some(presenter),
            Err(why) => {
                self.failed = Some(why);
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.send_size();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, news: News) {
        if news == News::Closed {
            // Every tab is closed and every renderer stopped: the window has
            // nothing left to be.
            event_loop.exit();
            return;
        }
        if self.showing.hear(news)
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(PhysicalSize { .. }) | WindowEvent::ScaleFactorChanged { .. } => {
                self.send_size();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.failed.is_some() {
                    self.close(event_loop);
                }
            }
            // Covered or minimised, or seen again (ADR 0039 § 1): `winit`
            // says both as occlusion on macOS.
            WindowEvent::Occluded(occluded) => {
                let to = if occluded {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                };
                // A conductor that has gone is a window closing; there is no
                // page left to tell.
                let _ = self.orders.send(Order::Visibility(to));
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } => self.cursor_moved((x, y)),
            WindowEvent::CursorLeft { .. } => self.cursor_left(),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self.main_button(state),
            // `is_synthetic` is a key already down as the window gained focus:
            // pressed somewhere else, so not a press here.
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if let Some(order) =
                    pressed(&event, self.modifiers).and_then(|press| keys::order(&press))
                {
                    // A conductor that has gone is a window closing.
                    let _ = self.orders.send(order);
                }
            }
            WindowEvent::CloseRequested => self.close(event_loop),
            _ => {}
        }
    }
}

/// Run the frame of ours around Chrome's window until it is told Chrome's
/// window is gone (ADR 0062, queue item 642): told where to stand, what
/// agents did and what the page is called on `told`, saying that
/// `renderer` draws the page, sending `orders` to the thread holding
/// Chrome, and having the page's tab drawn by a renderer of `strips`' —
/// and running `closing` as the event loop ends, however it ends.
///
/// `closing` is run in the handler's `exiting`, because quitting the program
/// ends the process there and never returns from the event loop (queue item
/// 688, [`crate::closing`]); a frame refused before its event loop began
/// runs it as it returns.
///
/// The frame is told on a channel rather than through the event loop,
/// because the thread holding Chrome starts first. Measured: a program
/// whose event loop existed but was not yet running while Chrome started
/// took fifteen seconds to have Chrome's window placed, and the window
/// server's listing as long to read, against a moment with no event loop
/// at all — so Chrome is started and placed before there is one, and what
/// it said meanwhile waits on the channel.
///
/// # Errors
///
/// What `winit` said, in words, when there could be no event loop or no
/// window.
pub fn frame(
    told: Receiver<ToFrame>,
    orders: Orders,
    renderer: String,
    strips: Renderers,
    closing: Closing<'_>,
) -> Result<(), String> {
    let Some(lettering) = Lettering::compiled_in() else {
        let _ = orders.send(Order::CloseEverything);
        return Err("the frame's own font is not a font".to_owned());
    };
    let mut building = EventLoop::<ToFrame>::with_user_event();
    // Chrome's window was just brought in front, and a program that took
    // the screen as it started would send it to Stage Manager's strip.
    #[cfg(target_os = "macos")]
    building.with_activate_ignoring_other_apps(false);
    let event_loop = building
        .build()
        .map_err(|why| format!("no event loop: {why}"))?;
    let proxy = event_loop.create_proxy();
    let painted = event_loop.create_proxy();
    let strip = match FrameStrip::start(
        strips,
        Box::new(move |news| painted.send_event(news).is_ok()),
    ) {
        Ok(strip) => strip,
        Err(why) => {
            let _ = orders.send(Order::CloseEverything);
            return Err(format!("no thread to draw the page's tab: {why}"));
        }
    };
    let forwarding = std::thread::Builder::new()
        .name("alo-frame".to_owned())
        .spawn(move || {
            // Ends when the thread holding Chrome does, or the event loop.
            for news in told {
                if proxy.send_event(news).is_err() {
                    return;
                }
            }
        });
    if let Err(why) = forwarding {
        let _ = orders.send(Order::CloseEverything);
        return Err(format!("no thread to tell the frame where to stand: {why}"));
    }
    let mut framing = Framing {
        orders,
        closing,
        told: Follow::Aside,
        renderer,
        lines: AgentLines::default(),
        named: None,
        strip: Some(strip),
        tab: None,
        tab_said: None,
        lettering,
        window: None,
        presenter: None,
        modifiers: ModifiersState::empty(),
        failed: None,
    };
    let ran = event_loop
        .run_app(&mut framing)
        .map_err(|why| format!("the event loop stopped: {why}"));
    // Outside the event loop, so a renderer still painting is waited for
    // here and never there; its renderer is stopped with it.
    let finished = framing.strip.take().is_none_or(FrameStrip::finish);
    ran?;
    match framing.failed {
        Some(why) => Err(why),
        None if !finished => Err("the thread drawing the page's tab did not finish".to_owned()),
        None => Ok(()),
    }
}

/// The frame's event loop's state.
struct Framing<'a> {
    orders: Orders,
    /// What the program does as it ends, run as the event loop does.
    closing: Closing<'a>,
    /// Where the frame was last told to stand.
    told: Follow,
    /// Which renderer draws the page, as the frame says it.
    renderer: String,
    /// What agents' verbs were answered with, as long as they are kept.
    lines: AgentLines,
    /// What Chrome last called the page, and where it is.
    named: Option<Named>,
    /// The thread drawing the page's tab; [`None`] once finished.
    strip: Option<FrameStrip>,
    /// The page's tab, as its renderer last painted it.
    tab: Option<Frame>,
    /// Why the strip's renderer painted no tab, since it last painted one.
    tab_said: Option<String>,
    /// The face the frame's words are drawn in.
    lettering: Lettering,
    window: Option<Rc<Window>>,
    presenter: Option<Presenter<Rc<Window>>>,
    /// The modifier keys held, as the window server last said.
    modifiers: ModifiersState,
    /// Why the frame had to stop, if it did.
    failed: Option<String>,
}

impl Framing<'_> {
    /// Put the window where it was last told, or hide it.
    fn stand(&self) {
        let Some(window) = &self.window else {
            return;
        };
        match self.told {
            Follow::At(at) => {
                let Some((position, size)) = standing(at) else {
                    // Somewhere no window of ours can be put is somewhere
                    // a person cannot see it beside Chrome's.
                    window.set_visible(false);
                    return;
                };
                window.set_outer_position(position);
                // `winit` answers the size it took at once, or `None` when
                // the window server will say it in a resize; either way
                // the frame is drawn at what its window is.
                let _ = window.request_inner_size(size);
                window.set_visible(true);
                window.request_redraw();
            }
            Follow::Aside => window.set_visible(false),
            Follow::Closed => {}
        }
    }

    /// Have the page's tab drawn for what Chrome last called the page, at
    /// the frame's scale — which its strip's thread does only when that is
    /// a different strip from the last.
    fn draw_tab(&mut self) {
        let (Some(window), Some(named), Some(strip)) =
            (&self.window, &self.named, self.strip.as_mut())
        else {
            return;
        };
        let scale = Scale::of_screen(window.scale_factor());
        strip.draw(strip_of(named, scale.factor()));
    }

    /// Draw the frame at its window's size.
    fn draw(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let canvas = compose_frame(
            &FrameScene {
                window: (size.width, size.height),
                scale: Scale::of_screen(window.scale_factor()),
                strip: self.tab.as_ref(),
                strip_sentence: self.tab_said.as_deref(),
                renderer: &self.renderer,
                agent_lines: self.lines.lines(),
            },
            &self.lettering,
        );
        if let Some(presenter) = self.presenter.as_mut()
            && let Err(why) = presenter.show(&canvas)
        {
            self.failed = Some(why);
        }
    }

    /// Have the frame drawn again.
    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Send the thread holding Chrome `order`, and leave the event loop
    /// at once if nobody is there to close Chrome's window and say so.
    fn send(&self, order: Order, event_loop: &ActiveEventLoop) {
        if self.orders.send(order).is_err() {
            event_loop.exit();
        }
    }
}

/// Where a window of `winit`'s goes to stand at `at`, in the screen's
/// points — or [`None`] for bounds past what a window can be put at.
fn standing(at: Bounds) -> Option<(LogicalPosition<f64>, LogicalSize<f64>)> {
    let side = |points: i64| i32::try_from(points).ok().map(f64::from);
    Some((
        LogicalPosition::new(side(at.left)?, side(at.top)?),
        LogicalSize::new(side(at.width)?, side(at.height)?),
    ))
}

impl ApplicationHandler<ToFrame> for Framing<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("alo")
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_resizable(false)
            .with_active(false)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(FIRST_SIZE.0, f64::from(STRIP_HEIGHT)));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(why) => {
                self.failed = Some(format!("no frame: {why}"));
                let _ = self.orders.send(Order::CloseEverything);
                event_loop.exit();
                return;
            }
        };
        match Presenter::over(Rc::clone(&window)) {
            Ok(presenter) => self.presenter = Some(presenter),
            Err(why) => {
                self.failed = Some(why);
                let _ = self.orders.send(Order::CloseEverything);
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.stand();
        // A name told before there was a window is drawn now there is.
        self.draw_tab();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, news: ToFrame) {
        let follow = match news {
            ToFrame::Stand(follow) => follow,
            ToFrame::Agent(line) => {
                self.lines.heard(line);
                self.redraw();
                return;
            }
            ToFrame::Named(named) => {
                self.named = Some(named);
                self.draw_tab();
                return;
            }
            ToFrame::Strip(tab) => {
                self.tab = Some(tab);
                self.tab_said = None;
                self.redraw();
                return;
            }
            ToFrame::StripSaid(said) => {
                self.tab_said = Some(said);
                self.redraw();
                return;
            }
            // `alo --chrome`'s page is never a card (queue item 717).
            ToFrame::Cast(_)
            | ToFrame::Uncast(_)
            | ToFrame::Moved(_)
            | ToFrame::Loading(_)
            | ToFrame::Travel(_)
            | ToFrame::Blocked { .. }
            | ToFrame::Told(_)
            | ToFrame::ListPressed
            | ToFrame::Caret(_)
            | ToFrame::Consent(_) => {
                return;
            }
        };
        self.told = follow;
        if follow == Follow::Closed {
            // Chrome's window is gone, or this browser closed it: the
            // frame has nothing left to stand beside.
            self.presenter = None;
            self.window = None;
            event_loop.exit();
            return;
        }
        self.stand();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(PhysicalSize { .. }) | WindowEvent::ScaleFactorChanged { .. } => {
                // A frame moved to a screen of another scale has its tab
                // painted again at it.
                self.draw_tab();
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.failed.is_some() {
                    self.send(Order::CloseEverything, event_loop);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if let Some(order) =
                    pressed(&event, self.modifiers).and_then(|press| keys::order(&press))
                {
                    self.send(order, event_loop);
                }
            }
            WindowEvent::CloseRequested => self.send(Order::CloseEverything, event_loop),
            _ => {}
        }
    }

    /// The event loop is ending — returning, or the program quit, when
    /// `AppKit` ends the process once this returns (item 688).
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.closing.run();
    }
}

/// Run the conversation's window until it is closed (ADR 0074, queue item
/// 709): the chats panel and the first screen, each drawn by a renderer of
/// `renderers`' at a site of its own, laid out at `forced` when a person
/// named a scale and at its screen's otherwise. An address chosen in it
/// opens in the Chromium `chromium` names (queue items 713 and 719), and
/// words chosen are searched with `search`, or said not set up and why
/// (queue item 708), and answered with `answer`; *Settings › Search and
/// answers* shows the application's `defaults` under the person's choices
/// and keeps a person's own keys with `keys` (ADR 0106). Its chats are kept in the directory `chats` names,
/// read from it as the window opens (queue item 757, ADR 0092), or, with
/// none or one that cannot be opened, not kept, and said so; its cards'
/// sign-ins in the jar `sign_ins` (ADR 0111); the chats' key in `keychain`
/// — or, under a data directory with no secret store (queue item 957),
/// none, and the chats not kept. The card live
/// is reached by agents as `reaching` says (queue item 716), and `closing`
/// is run as the window's event loop ends — quit included, after which
/// `AppKit` ends the process (item 688) — once every card is closed.
///
/// # Errors
///
/// What `winit` said, in words, when there could be no event loop or no
/// window; or that the thread drawing the two parts could not be started,
/// or did not finish.
pub fn conversation(
    forced: Option<Scale>,
    renderers: Renderers,
    chromium: Chromium,
    (search, answer, defaults, keys): (
        Result<SearchService, String>,
        Result<AiService, String>,
        crate::services::Named,
        crate::service_keys::Keys,
    ),
    (chats, sign_ins, keychain): (
        Result<PathBuf, String>,
        std::sync::Arc<crate::sign_ins::SignIns>,
        Option<crate::keychain::Keychain>,
    ),
    (reaching, closing, agent): (
        Reaching,
        Closing<'_>,
        Result<crate::conversation_agent::Agent, String>,
    ),
) -> Result<(), String> {
    let lettering =
        Lettering::compiled_in().ok_or_else(|| "the window's own font is not a font".to_owned())?;
    let event_loop = EventLoop::<Heard>::with_user_event()
        .build()
        .map_err(|why| format!("no event loop: {why}"))?;
    let proxy = event_loop.create_proxy();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |drawn| proxy.send_event(Heard::Drawn(drawn)).is_ok()),
    )
    .map_err(|why| format!("no thread to draw the chats panel and the first screen: {why}"))?;
    hear_new_tabs(&reaching, event_loop.create_proxy());
    let proxy = event_loop.create_proxy();
    // Every card's Chromium given the sign-ins kept, and keeping its own
    // in the same jar (ADR 0111).
    let chrome = ChromeOpener::start_kept(
        chromium,
        reaching,
        Some(sign_ins.clone()),
        Box::new(move |went| proxy.send_event(Heard::Chrome(went)).is_ok()),
    )
    .map_err(|why| format!("no thread to open a page in Chrome: {why}"))?;
    // A search's answer, asked on a thread of its own once its sites have
    // come (queue item 707), told to the window as a search's news is.
    let proxy = event_loop.create_proxy();
    let answerer = crate::conversation_answer::Answerer::start(
        answer,
        Box::new(move |searched| proxy.send_event(Heard::Searched(searched)).is_ok()),
    )
    .map_err(|why| format!("no thread to answer with: {why}"))?;
    let proxy = event_loop.create_proxy();
    let searcher = Searcher::answering(
        search,
        Some(answerer),
        Box::new(move |searched| proxy.send_event(Heard::Searched(searched)).is_ok()),
    )
    .map_err(|why| format!("no thread to search with: {why}"))?;
    // The agent at work on a card's page (queue item 704, ADR 0096), each
    // run telling the window as the other threads do.
    let proxy = event_loop.create_proxy();
    let tell_agent: crate::conversation_agent::TellAgent =
        std::sync::Arc::new(move |said| proxy.send_event(Heard::Agent(said)).is_ok());
    // What a person chose in Settings, kept beside the chats whether or not
    // the chats themselves can be opened (queue item 854): a file that does
    // not read is Manrope, and said.
    let settings = chats.as_deref().ok().map(crate::settings_file::file_in);
    let read = settings.as_deref().map_or(
        crate::settings_file::Read {
            face: alo_renderer::interface_face::InterfaceFace::Manrope,
            limits: alo_renderer::agent_limits::Limits::default(),
            services: crate::services::Named::default(),
            privacy: alo_renderer::settings_privacy::Privacy::default(),
            said: None,
        },
        crate::settings_file::read,
    );
    let state = ConversationWindow::new(forced, drawing, chrome)
        .with_search(searcher)
        .with_agent(agent, tell_agent)
        .with_settings(read, settings)
        .with_services(defaults, keys)
        .with_sign_ins(sign_ins);
    // Where the chats are kept across launches (queue item 757, ADR 0092),
    // sealed under the key in `keychain` (queue item 790, ADR 0098), read
    // before the window opens; with nowhere, or no key, said, and none
    // kept.
    let state = match chats.and_then(|directory| {
        let key = chat_key(keychain)?;
        ChatStore::open(&directory, key.key).map(|opened| (opened, key.made))
    }) {
        Ok(((store, opened), made)) => {
            // The sites blocking is switched off for, kept beside the chats
            // (queue item 807), are this process's from here.
            let file = crate::blocking_off::file_in(store.directory());
            let off = crate::blocking_off::process();
            let read = crate::blocking_off::read(&file);
            for site in read.as_deref().unwrap_or_default() {
                off.switch(site, true);
            }
            let state = state
                .with_chats(store, opened)
                .key_made(made)
                .with_blocking_off(off.clone(), Some(file));
            match read {
                Ok(_) => state,
                Err(why) => state.saying(format!(
                    "the sites blocking is switched off for are not read, so none is: {why}"
                )),
            }
        }
        Err(why) => state
            .keeping_nothing(&why)
            .with_blocking_off(crate::blocking_off::process().clone(), None),
    };
    let mut conversing = Conversing {
        state,
        lettering,
        window: None,
        presenter: None,
        composed: Composed::new(),
        modifiers: ModifiersState::empty(),
        failed: None,
        closing,
    };
    let ran = event_loop
        .run_app(&mut conversing)
        .map_err(|why| format!("the event loop stopped: {why}"));
    // A loop refused before it ran never reached `exiting`.
    conversing.state.finish();
    conversing.closing.run();
    ran?;
    match (conversing.failed, conversing.state.unfinished()) {
        (Some(why), _) => Err(why),
        (None, Some(unfinished)) => Err(unfinished.to_owned()),
        (None, None) => Ok(()),
    }
}

/// The conversation's event loop's state: the window, and what it holds
/// short of the window server ([`ConversationWindow`]), which every event
/// is handed to as this browser's own value.
struct Conversing<'c> {
    /// Everything but the window itself.
    state: ConversationWindow,
    /// The face the window's own lines are drawn in.
    lettering: Lettering,
    window: Option<Arc<Window>>,
    /// How the window is put on screen: from its pixels kept on the GPU,
    /// given only what was composed again (queue item 873, ADR 0105), or
    /// copied whole by `softbuffer` where that cannot be had.
    presenter: Option<Screen<Arc<Window>>>,
    /// The window's pixels as last composed, composed again only where what
    /// they are made from changed (queue item 872).
    composed: Composed,
    /// The modifier keys held.
    modifiers: ModifiersState,
    /// Why the window had to stop, if it did.
    failed: Option<String>,
    /// Run once every card is closed, as the event loop ends: the agent
    /// socket stopped and removed (queue item 716).
    closing: Closing<'c>,
}

impl Conversing<'_> {
    /// Tell the state where the window is now.
    fn measure(&mut self) {
        if let Some(window) = &self.window {
            let size = window.inner_size();
            self.state.measured_as(Measured {
                size: (size.width, size.height),
                scale_factor: window.scale_factor(),
                outer: window
                    .outer_position()
                    .ok()
                    .map(|PhysicalPosition { x, y }| (x, y)),
            });
        }
    }

    /// Do what the state asked of the window since it was last asked: say
    /// its lines on the terminal, take the front back, draw again.
    fn follow(&mut self) {
        for said in self.state.take_said() {
            eprintln!("alo: {said}");
        }
        let front = self.state.take_front();
        let redraw = self.state.take_redraw();
        let cursor = self.state.take_cursor();
        let ime = self.state.take_ime();
        if let Some(window) = &self.window {
            // Called as the open card's page is, where the window server
            // draws the title itself (queue item 958, `window_title`).
            if let Some(title) = self.state.take_title() {
                window.set_title(crate::window_title::shown(&title));
            }
            // Input methods on the card's page (queue item 802): allowed
            // while it has the keyboard, the candidates under its caret.
            if let Some(allowed) = ime.allowed {
                window.set_ime_allowed(allowed);
            }
            if let Some((left, top, width, height)) = ime.caret {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(left, top),
                    PhysicalSize::new(width, height),
                );
            }
            if front {
                window.focus_window();
            }
            // The shape the card's page asked for over it (queue item 799).
            if let Some(shape) = cursor {
                window.set_cursor_visible(shape != "none");
                window.set_cursor(shape.parse::<CursorIcon>().unwrap_or_default());
            }
            if redraw {
                window.request_redraw();
            }
        }
    }

    /// Draw the window from what it has been told.
    fn draw(&mut self) {
        let Some(scene) = self.state.scene() else {
            return;
        };
        let size = scene.window;
        let (words, recomposed) = self.composed.compose(&scene, &self.lettering);
        if let Some(presenter) = self.presenter.as_mut()
            && let Err(why) = presenter.show(size, words, &recomposed)
        {
            self.failed = Some(why);
        }
    }
}

impl ApplicationHandler<Heard> for Conversing<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // Never narrower than the tab row beside the rail, nor shorter than
        // the first screen's sites row (queue item 775).
        let attributes = WindowAttributes::default()
            .with_title(crate::window_title::ALONE)
            .with_inner_size(LogicalSize::new(CONVERSATION_SIZE.0, CONVERSATION_SIZE.1))
            .with_min_inner_size(LogicalSize::new(LEAST_SIZE.0, LEAST_SIZE.1));
        let attributes = named(attributes);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(why) => {
                self.failed = Some(format!("no window: {why}"));
                event_loop.exit();
                return;
            }
        };
        match Screen::over(Arc::clone(&window), How::asked()) {
            Ok((presenter, missed)) => {
                if let Some(why) = missed {
                    eprintln!("alo: the window's pixels are copied whole each frame: {why}");
                }
                self.presenter = Some(presenter);
            }
            Err(why) => {
                self.failed = Some(why);
                event_loop.exit();
                return;
            }
        }
        window.request_redraw();
        self.state.focused(window.has_focus());
        self.window = Some(window);
        self.measure();
        self.state.ask_drawn();
    }

    fn user_event(&mut self, _: &ActiveEventLoop, heard: Heard) {
        self.measure();
        self.state.heard(heard);
        self.follow();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        self.measure();
        match event {
            WindowEvent::Resized(PhysicalSize { .. }) | WindowEvent::ScaleFactorChanged { .. } => {
                if matches!(event, WindowEvent::ScaleFactorChanged { .. }) {
                    // A new scale is frames of another size.
                    self.state.seen(true);
                }
                self.state.ask_drawn();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            // Covered or minimised, the card is off the screen, and its
            // page is not streamed (ADR 0076). Seen again, it is presented
            // again: a Metal layer covered gives no drawable, and keeps the
            // frame written while it was (ADR 0105).
            WindowEvent::Occluded(hidden) => {
                self.state.seen(!hidden);
                if !hidden && let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Focused(focused) => self.state.focused(focused),
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.failed.is_some() {
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => {
                self.presenter = None;
                self.window = None;
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                self.state.held(held(self.modifiers));
            }
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } => self.state.moved((x, y)),
            WindowEvent::CursorLeft { .. } => self.state.cursor_left(),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.state.main_button_pressed(),
            // The secondary and the middle buttons over a card's page
            // (queue item 798).
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: button @ (MouseButton::Right | MouseButton::Middle),
                ..
            } => self
                .state
                .other_button_pressed(button == MouseButton::Middle),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.state.main_button_released(),
            WindowEvent::MouseWheel { delta, .. } => self.state.wheeled(match delta {
                MouseScrollDelta::LineDelta(across, down) => Turned::Lines { across, down },
                MouseScrollDelta::PixelDelta(PhysicalPosition { x, y }) => {
                    Turned::Pixels { across: x, down: y }
                }
            }),
            // An input method's composition, and what it came to, on the
            // card's page (queue item 802).
            WindowEvent::Ime(Ime::Preedit(text, selection)) => {
                self.state.composing(&text, selection);
            }
            WindowEvent::Ime(Ime::Commit(text)) => self.state.committed(&text),
            // A key already down as the window gained focus was pressed
            // somewhere else.
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if let Some(press) = pressed(&event, self.modifiers) {
                    self.state.key(&press, event.text.as_deref());
                }
            }
            _ => {}
        }
        self.follow();
    }

    /// Nothing more to hear for now: while a wheel's lines are glided
    /// (queue item 756), move the glide on and wake again a frame later;
    /// while a chat deleted can be brought back (queue item 789), wake when
    /// it no longer can; otherwise wait for the next event.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        self.state.tick(now);
        let wake = self.state.wake_at();
        if self.state.gliding() {
            self.state.glide(now);
            self.follow();
            let next = now + FRAME;
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                wake.map_or(next, |wake| wake.min(next)),
            ));
        } else {
            self.follow();
            event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
        }
    }

    /// The event loop is ending — returning, or the program quit, when
    /// `AppKit` ends the process once this returns (item 688): the drawing
    /// thread is stopped here, and its renderers with it, and the Chrome
    /// showing a page is closed, waited for and its profile removed.
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.state.finish();
        self.closing.run();
    }
}

/// The modifier keys `modifiers` holds, as the browser names them.
fn held(modifiers: ModifiersState) -> Held {
    [
        (modifiers.super_key(), Modifier::Command),
        (modifiers.control_key(), Modifier::Control),
        (modifiers.shift_key(), Modifier::Shift),
        (modifiers.alt_key(), Modifier::Option),
    ]
    .into_iter()
    .filter(|(down, _)| *down)
    .fold(Held::NOTHING, |held, (_, modifier)| held.with(modifier))
}

/// Which function key `named` is, by its number, if it is one of the twenty
/// a keyboard of a Mac's has at most.
fn function(named: NamedKey) -> Option<u8> {
    [
        NamedKey::F1,
        NamedKey::F2,
        NamedKey::F3,
        NamedKey::F4,
        NamedKey::F5,
        NamedKey::F6,
        NamedKey::F7,
        NamedKey::F8,
        NamedKey::F9,
        NamedKey::F10,
        NamedKey::F11,
        NamedKey::F12,
        NamedKey::F13,
        NamedKey::F14,
        NamedKey::F15,
        NamedKey::F16,
        NamedKey::F17,
        NamedKey::F18,
        NamedKey::F19,
        NamedKey::F20,
    ]
    .iter()
    .position(|each| *each == named)
    .and_then(|at| u8::try_from(at + 1).ok())
}

/// A key going down, as the browser reads one; [`None`] for a key coming up.
fn pressed(event: &KeyEvent, modifiers: ModifiersState) -> Option<Press> {
    if event.state != ElementState::Pressed {
        return None;
    }
    let key = match event.key_without_modifiers() {
        winit_keys::Key::Character(written) => Key::Character(written.to_string()),
        winit_keys::Key::Named(NamedKey::Tab) => Key::Tab,
        winit_keys::Key::Named(NamedKey::Enter) => Key::Return,
        winit_keys::Key::Named(NamedKey::Backspace) => Key::Backspace,
        winit_keys::Key::Named(NamedKey::Escape) => Key::Escape,
        winit_keys::Key::Named(NamedKey::Space) => Key::Space,
        winit_keys::Key::Named(NamedKey::PageUp) => Key::PageUp,
        winit_keys::Key::Named(NamedKey::PageDown) => Key::PageDown,
        winit_keys::Key::Named(NamedKey::Home) => Key::Home,
        winit_keys::Key::Named(NamedKey::End) => Key::End,
        winit_keys::Key::Named(NamedKey::ArrowLeft) => Key::Arrow(Arrow::Left),
        winit_keys::Key::Named(NamedKey::ArrowRight) => Key::Arrow(Arrow::Right),
        winit_keys::Key::Named(NamedKey::ArrowUp) => Key::Arrow(Arrow::Up),
        winit_keys::Key::Named(NamedKey::ArrowDown) => Key::Arrow(Arrow::Down),
        winit_keys::Key::Named(NamedKey::Delete) => Key::Delete,
        winit_keys::Key::Named(named) => function(named).map_or(Key::Other, Key::Function),
        _ => Key::Other,
    };
    Some(Press {
        key,
        held: held(modifiers),
        repeat: event.repeat,
    })
}

/// The key the chats are sealed under, in `keychain` — or, with none, why
/// none: a data directory has no secret store yet (queue item 957).
fn chat_key(keychain: Option<crate::keychain::Keychain>) -> Result<ChatKey, String> {
    let keychain = keychain.ok_or(crate::data_dir::NO_SECRET_STORE)?;
    ChatKey::in_keychain(&keychain, &crate::keychain::this_person())
}

/// A new tab an agent outside asks for over the socket, judged by the
/// relay, is the window's to open (queue item 956): handed to it by
/// `proxy`.
fn hear_new_tabs(reaching: &Reaching, proxy: winit::event_loop::EventLoopProxy<Heard>) {
    reaching.reached.answered_by(Box::new(move |asked| {
        proxy.send_event(Heard::NewTab(asked)).is_ok()
    }));
}

/// `attributes`, naming the application to the window server where `winit`
/// lets it be named (queue item 958, ADR 0112 § 1): on Linux, Wayland's
/// `xdg_toplevel` `app_id` — the desktop entry's name,
/// [`crate::window_title::APP_ID`]. On macOS the system names the
/// application by its bundle's identifier, which is the same name, and
/// there is nothing to set.
#[cfg(target_os = "linux")]
fn named(attributes: WindowAttributes) -> WindowAttributes {
    use winit::platform::wayland::WindowAttributesExtWayland;
    attributes.with_name(crate::window_title::APP_ID, crate::window_title::APP_ID)
}

/// `attributes` as they are: see the Linux build's.
#[cfg(not(target_os = "linux"))]
fn named(attributes: WindowAttributes) -> WindowAttributes {
    attributes
}

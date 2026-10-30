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
//! screen's line — where it was decided to go, which opening in Chrome from
//! here is queue item 713, or that searching or asking is not set up yet.
//! The text a key writes is `KeyEvent::text`, a field read like the others.
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
//! # No `unsafe`
//!
//! Every `winit` function called here — `EventLoop::with_user_event`,
//! `EventLoopBuilder::build`, `EventLoop::create_proxy`, `EventLoop::run_app`,
//! `EventLoopProxy::send_event`, `ActiveEventLoop::create_window`,
//! `ActiveEventLoop::exit`, `Window::inner_size`, `Window::scale_factor`,
//! `Window::request_redraw`, `Modifiers::state`, `ModifiersState`'s
//! `super_key`, `control_key`, `shift_key` and `alt_key`, and
//! `KeyEventExtModifierSupplement::key_without_modifiers` — is a safe
//! function in `winit` 0.30.13's source, which the commits adding them
//! checked (ADR 0024 § 1's stop rule). The pointer adds no call: it reads
//! the fields of `WindowEvent::CursorMoved`, `CursorLeft` and `MouseInput`,
//! and `ModifiersState::is_empty`, a safe `bitflags` function. The frame
//! adds `WindowAttributes::with_decorations`, `with_window_level`,
//! `with_visible`, `with_resizable` and `with_active`,
//! `Window::set_outer_position`, `request_inner_size` and `set_visible`, and
//! on macOS `EventLoopBuilderExtMacOS::with_activate_ignoring_other_apps`,
//! each a safe function in 0.30.13's source as checked by item 642 (ADR
//! 0062's consequences); `with_parent_window`, the one `unsafe fn` beside
//! them, is not called. Quitting the program adds no call either: the
//! frame's shutdown is run in `ApplicationHandler::exiting`, a method of
//! ours that `winit`'s own application delegate calls from
//! `applicationWillTerminate:` (item 688).

use crate::agent_lines::AgentLines;
use crate::aim::{Aimed, aim, on_page};
use crate::chrome_follow::Follow;
use crate::chrome_protocol::Bounds;
use crate::chrome_protocol::Named;
use crate::closing::Closing;
use crate::compose::{FrameScene, compose, compose_frame};
use crate::conversation::{Conversation, Pressed as Decided};
use crate::conversation_aim;
use crate::conversation_box::{self, Typed};
use crate::conversation_compose::{ConversationScene, compose_conversation};
use crate::conversation_drawing::{Asked, ConversationDrawing, Drawn, Part};
use crate::conversation_place::{ConversationPlacement, panel_viewport, start_viewport};
use crate::cursor::{Cursor, Pressed};
use crate::frame_strip::{FrameStrip, strip_of};
use crate::frame_told::ToFrame;
use crate::inbox::Orders;
use crate::keys::{self, Held, Key, Modifier, Press};
use crate::message::{News, Order};
use crate::notice::Lettering;
use crate::place::{Placement, STRIP_HEIGHT, screen};
use crate::present::Presenter;
use crate::scale::Scale;
use crate::showing::Showing;
use crate::typing;
use alo_renderer::host::Renderers;
use alo_renderer::{Frame, Visibility};
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{self as winit_keys, ModifiersState, NamedKey};
#[cfg(target_os = "macos")]
use winit::platform::macos::EventLoopBuilderExtMacOS;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

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
/// named a scale and at its screen's otherwise.
///
/// # Errors
///
/// What `winit` said, in words, when there could be no event loop or no
/// window; or that the thread drawing the two parts could not be started,
/// or did not finish.
pub fn conversation(forced: Option<Scale>, renderers: Renderers) -> Result<(), String> {
    let lettering =
        Lettering::compiled_in().ok_or_else(|| "the window's own font is not a font".to_owned())?;
    let event_loop = EventLoop::<Drawn>::with_user_event()
        .build()
        .map_err(|why| format!("no event loop: {why}"))?;
    let proxy = event_loop.create_proxy();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |drawn| proxy.send_event(drawn).is_ok()),
    )
    .map_err(|why| format!("no thread to draw the chats panel and the first screen: {why}"))?;
    let mut conversing = Conversing {
        forced,
        conversation: Conversation::default(),
        drawing: Some(drawing),
        finished: true,
        panel: None,
        panel_said: None,
        start: None,
        start_said: None,
        lettering,
        window: None,
        presenter: None,
        cursor: None,
        modifiers: ModifiersState::empty(),
        failed: None,
    };
    let ran = event_loop
        .run_app(&mut conversing)
        .map_err(|why| format!("the event loop stopped: {why}"));
    // A loop refused before it ran never reached `exiting`.
    if let Some(drawing) = conversing.drawing.take() {
        conversing.finished = drawing.finish();
    }
    ran?;
    match conversing.failed {
        Some(why) => Err(why),
        None if !conversing.finished => {
            Err("the thread drawing the chats panel and the first screen did not finish".to_owned())
        }
        None => Ok(()),
    }
}

/// The conversation's event loop's state.
struct Conversing {
    /// The scale a person named, if they named one.
    forced: Option<Scale>,
    /// What the browser process holds of the interface.
    conversation: Conversation,
    /// The thread drawing the two parts; [`None`] once finished.
    drawing: Option<ConversationDrawing>,
    /// Whether that thread finished rather than panicked, once waited for.
    finished: bool,
    /// The panel, as its renderer last painted it.
    panel: Option<Frame>,
    /// Why the panel's renderer painted nothing, since it last painted.
    panel_said: Option<String>,
    /// The first screen, as its renderer last painted it.
    start: Option<Frame>,
    /// Why the first screen's renderer painted nothing, since it last
    /// painted.
    start_said: Option<String>,
    /// The face the window's own lines are drawn in.
    lettering: Lettering,
    window: Option<Rc<Window>>,
    presenter: Option<Presenter<Rc<Window>>>,
    /// Where the cursor last was, in device pixels, while it is over the
    /// window.
    cursor: Option<(f64, f64)>,
    /// The modifier keys held.
    modifiers: ModifiersState,
    /// Why the window had to stop, if it did.
    failed: Option<String>,
}

impl Conversing {
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

    /// Have both parts drawn at the window's size and scale — which the
    /// drawing thread does only for a part that differs from the last.
    fn ask_drawn(&mut self) {
        let Some((window, scale)) = self.measured() else {
            return;
        };
        let Some(drawing) = self.drawing.as_mut() else {
            return;
        };
        drawing.draw(Asked {
            panel: self
                .conversation
                .panel(panel_viewport(window, scale), scale),
            start: self
                .conversation
                .start(start_viewport(window, scale), scale),
            showing: self.conversation.showing(),
        });
    }

    /// What the window is drawn from now, at its size and scale.
    fn scene(&self) -> Option<ConversationScene<'_>> {
        let (window, scale) = self.measured()?;
        Some(ConversationScene {
            window,
            scale,
            panel: self.panel.as_ref(),
            panel_sentence: self.panel_said.as_deref(),
            start: self.start.as_ref(),
            start_sentence: self.start_said.as_deref(),
        })
    }

    /// The main button went down with nothing held: a press over a part is
    /// handed to the thread that holds its renderer, to be claimed and
    /// decided.
    fn main_button_pressed(&mut self) {
        if !self.modifiers.is_empty() {
            return;
        }
        let Some(at) = self.cursor else {
            return;
        };
        let Some(scene) = self.scene() else {
            return;
        };
        let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
        let Some((part, on)) = conversation_aim::aim(&placed, scene.scale, at) else {
            return;
        };
        if let Some(drawing) = self.drawing.as_mut() {
            drawing.press(part, on);
        }
    }

    /// What a press on `part` was decided to be: acted on, and the parts
    /// drawn again if it changed what they show; or, refused or unanswered,
    /// said in the part's line.
    fn pressed(&mut self, part: Part, pressed: Decided) {
        let said = match &pressed {
            Decided::Refused(refusal) => Some(refusal.to_string()),
            Decided::Failed(failure) => Some(format!("{part} could not answer a press: {failure}")),
            _ => None,
        };
        if let Some(said) = said {
            match part {
                Part::Panel => self.panel_said = Some(said),
                Part::Start => self.start_said = Some(said),
            }
        }
        let go = pressed == Decided::Go;
        if self.conversation.act(pressed) {
            self.ask_drawn();
        }
        if go && let Some(chosen) = self.conversation.chosen() {
            self.start_said = Some(conversation_box::said(chosen));
        }
    }

    /// A key went down: an edit of the one box, if it makes one, drawn
    /// again when it changed what the box holds, and what it chose said.
    fn key(&mut self, event: &KeyEvent) {
        let Some(press) = pressed(event, self.modifiers) else {
            return;
        };
        let Some(edit) = typing::edit(&press, event.text.as_deref()) else {
            return;
        };
        match self.conversation.edit(edit) {
            Typed::Unchanged => {}
            Typed::Changed => self.ask_drawn(),
            Typed::Chose(chosen) => {
                self.start_said = Some(conversation_box::said(&chosen));
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    /// Draw the window from what it has been told.
    fn draw(&mut self) {
        let Some(scene) = self.scene() else {
            return;
        };
        let canvas = compose_conversation(&scene, &self.lettering);
        if let Some(presenter) = self.presenter.as_mut()
            && let Err(why) = presenter.show(&canvas)
        {
            self.failed = Some(why);
        }
    }
}

impl ApplicationHandler<Drawn> for Conversing {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("alo")
            .with_inner_size(LogicalSize::new(CONVERSATION_SIZE.0, CONVERSATION_SIZE.1));
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
        window.request_redraw();
        self.window = Some(window);
        self.ask_drawn();
    }

    fn user_event(&mut self, _: &ActiveEventLoop, drawn: Drawn) {
        match drawn {
            Drawn::Painted(Part::Panel, frame) => {
                self.panel = Some(frame);
                self.panel_said = None;
            }
            Drawn::Painted(Part::Start, frame) => {
                self.start = Some(frame);
                self.start_said = None;
            }
            Drawn::Said(Part::Panel, said) => self.panel_said = Some(said),
            Drawn::Said(Part::Start, said) => self.start_said = Some(said),
            Drawn::Pressed(part, pressed) => self.pressed(part, pressed),
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(PhysicalSize { .. }) | WindowEvent::ScaleFactorChanged { .. } => {
                self.ask_drawn();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
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
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } => self.cursor = Some((x, y)),
            WindowEvent::CursorLeft { .. } => self.cursor = None,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.main_button_pressed(),
            // A key already down as the window gained focus was pressed
            // somewhere else.
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.key(&event),
            _ => {}
        }
    }

    /// The event loop is ending — returning, or the program quit, when
    /// `AppKit` ends the process once this returns (item 688): the drawing
    /// thread is stopped here, and its renderers with it.
    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(drawing) = self.drawing.take() {
            self.finished = drawing.finish();
        }
    }
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
        _ => Key::Other,
    };
    let held = [
        (modifiers.super_key(), Modifier::Command),
        (modifiers.control_key(), Modifier::Control),
        (modifiers.shift_key(), Modifier::Shift),
        (modifiers.alt_key(), Modifier::Option),
    ]
    .into_iter()
    .filter(|(down, _)| *down)
    .fold(Held::NOTHING, |held, (_, modifier)| held.with(modifier));
    Some(Press {
        key,
        held,
        repeat: event.repeat,
    })
}

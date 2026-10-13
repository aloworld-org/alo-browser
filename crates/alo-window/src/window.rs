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
//! Item 535, ADR 0024 § 5. The window keeps where the cursor last was and
//! decides from its own composition what a press of the **main button** is
//! over ([`crate::aim`]): over the page, it is sent to the conductor in the
//! page's CSS pixels as [`Order::Pointer`], and its release is sent too,
//! wherever the cursor is by then. Over the strip it is item 539's, and
//! over a line the window is saying something in it is nobody's.
//!
//! Not sent yet: the cursor moving with no button down (item 542), the
//! other buttons (544), a press with a modifier held (545) — ⌘ held over a
//! link opens a tab in Chrome, which no tab here does — and how many
//! presses came in quick succession (547). The window shows the tab strip
//! across its top and the selected tab below it (item 528).
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
//! and `ModifiersState::is_empty`, a safe `bitflags` function.

use crate::aim::{Aimed, aim, on_page};
use crate::compose::compose;
use crate::inbox::Orders;
use crate::keys::{self, Held, Key, Modifier, Press};
use crate::message::{News, Order};
use crate::notice::Lettering;
use crate::place::{Placement, replication, viewport};
use crate::present::Presenter;
use crate::showing::Showing;
use alo_renderer::Visibility;
use alo_renderer::pointer::{Pointer, Went};
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{self as winit_keys, ModifiersState, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowAttributes, WindowId};

/// How big a new window is, in the window server's logical points.
const FIRST_SIZE: (f64, f64) = (1000.0, 700.0);

/// Run the window until it is closed and every tab under it is.
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
pub fn run(start: impl FnOnce(Box<dyn Fn(News) -> bool + Send>) -> Orders) -> Result<(), String> {
    let lettering =
        Lettering::compiled_in().ok_or_else(|| "the window's own font is not a font".to_owned())?;
    let event_loop = EventLoop::<News>::with_user_event()
        .build()
        .map_err(|why| format!("no event loop: {why}"))?;
    let proxy = event_loop.create_proxy();
    let orders = start(Box::new(move |news| proxy.send_event(news).is_ok()));
    let mut app = App {
        orders,
        lettering,
        showing: Showing::default(),
        window: None,
        presenter: None,
        modifiers: ModifiersState::empty(),
        cursor: None,
        pressing: false,
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
    lettering: Lettering,
    showing: Showing,
    window: Option<Rc<Window>>,
    presenter: Option<Presenter<Rc<Window>>>,
    /// The modifier keys held, as the window server last said.
    modifiers: ModifiersState,
    /// Where the cursor last was in the window, in device pixels, while it
    /// is over the window.
    cursor: Option<(f64, f64)>,
    /// Whether a press of the main button was sent to the page, and so its
    /// release is owed.
    pressing: bool,
    /// Why the window had to stop, if it did.
    failed: Option<String>,
}

impl App {
    /// The window's size in device pixels and how much a CSS pixel is
    /// replicated, if there is a window.
    fn measured(&self) -> Option<((u32, u32), u32)> {
        self.window.as_ref().map(|window| {
            let size = window.inner_size();
            (
                (size.width, size.height),
                replication(window.scale_factor()),
            )
        })
    }

    /// Tell the conductor the size a page in this window is laid out at.
    fn send_size(&self) {
        if let Some((window, replication)) = self.measured() {
            // A conductor that has gone is a window closing; there is nobody
            // left to lay anything out for.
            let _ = self
                .orders
                .send(Order::Resize(viewport(window, replication)));
        }
    }

    /// Draw the window from what it has been sent.
    fn draw(&mut self) {
        let Some((window, replication)) = self.measured() else {
            return;
        };
        let canvas = compose(&self.showing.scene(window, replication), &self.lettering);
        if let Some(presenter) = self.presenter.as_mut()
            && let Err(why) = presenter.show(&canvas)
        {
            self.failed = Some(why);
        }
    }

    /// Where things are in the window now, and how much a CSS pixel is
    /// replicated, if there is a window.
    fn placed(&self) -> Option<((u32, u32), Placement, u32)> {
        let (window, replication) = self.measured()?;
        let scene = self.showing.scene(window, replication);
        Some((
            window,
            Placement::of(window, replication, scene.contents()),
            replication,
        ))
    }

    /// The main button went `state`: a press over the page is sent, and so
    /// is the release of a press that was.
    fn main_button(&mut self, state: ElementState) {
        let Some((window, placed, replication)) = self.placed() else {
            return;
        };
        let pointer = match state {
            ElementState::Pressed => {
                let over = self.cursor.map(|at| aim(window, &placed, replication, at));
                match over {
                    Some(Aimed::Page(at)) if self.modifiers.is_empty() => Pointer {
                        went: Went::Down,
                        at,
                    },
                    _ => return,
                }
            }
            ElementState::Released => {
                if !self.pressing {
                    return;
                }
                let at = self.cursor.unwrap_or_default();
                Pointer {
                    went: Went::Up,
                    at: on_page(&placed, replication, at),
                }
            }
        };
        self.pressing = pointer.went == Went::Down;
        // A conductor that has gone is a window closing.
        let _ = self.orders.send(Order::Pointer(pointer));
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
            } => self.cursor = Some((x, y)),
            // A release outside the window still reaches it, from where the
            // cursor was last seen.
            WindowEvent::CursorLeft { .. } if !self.pressing => self.cursor = None,
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

/// A key going down, as the browser reads one; [`None`] for a key coming up.
fn pressed(event: &KeyEvent, modifiers: ModifiersState) -> Option<Press> {
    if event.state != ElementState::Pressed {
        return None;
    }
    let key = match event.key_without_modifiers() {
        winit_keys::Key::Character(written) => Key::Character(written.to_string()),
        winit_keys::Key::Named(NamedKey::Tab) => Key::Tab,
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

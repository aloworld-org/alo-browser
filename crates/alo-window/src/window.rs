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
//! # What it does not do yet
//!
//! A person's pointer reaches no page: that is item 298, which decides which
//! frame a point falls in and sends it to that frame's renderer. The window
//! shows the tab strip across its top and the selected tab below it (item
//! 528).
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
//! checked (ADR 0024 § 1's stop rule).

use crate::compose::compose;
use crate::inbox::Orders;
use crate::keys::{self, Held, Key, Modifier, Press};
use crate::message::{News, Order};
use crate::notice::Lettering;
use crate::place::{replication, viewport};
use crate::present::Presenter;
use crate::showing::Showing;
use alo_renderer::Visibility;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, WindowEvent};
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

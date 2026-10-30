/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo`: the browser a person starts (ADR 0024 § 1).
//!
//! `alo [--frozen-fonts] [--scale FACTOR] [--agent-site ADDRESS ...]
//! [PAGE.html [SHEET.css ...]]` opens a window with one tab showing the
//! page, or an empty tab if none is named, laid out at the scale named or at
//! its screen's. Closing the window closes every tab and stops every renderer.
//!
//! An agent outside it reaches it over a socket under the person's runtime
//! directory (ADR 0055), whose path is said on the terminal as the window
//! opens. A browser that cannot place one still opens, and says why there is
//! none. `--agent-site` keeps every agent to the sites named (queue item 497),
//! and the terminal says so, in this browser's own words: it is a refusal of
//! ours, not a grant of any system's.
//!
//! `alo --chrome [--agent-site ADDRESS ...] (ADDRESS | PAGE.html)` has the
//! Chrome installed on the machine draw the page instead (ADR 0058, queue
//! item 641), behind the same socket, the same grants and the same record.
//! Chrome shows it in a window of its own, placed on the screen by this
//! process (item 627, ADR 0061), and the terminal says where it went. A
//! frame of ours floats directly above it and follows it as a person moves
//! and resizes it, standing aside while it is not on the screen (item 642,
//! ADR 0062). The frame shows the page's tab, drawn by a tab strip's
//! renderer of this engine's from what Chrome calls the page (item 681),
//! and says which Chrome draws the page, and what each agent's verb was
//! answered with or why it was refused (item 679).
//! Closing either closes both, and so does this browser's input ending —
//! Ctrl-D at a terminal.
//!
//! `alo --conversation [--frozen-fonts] [--scale FACTOR]` opens the
//! browser's own interface as a conversation instead (ADR 0074, queue item
//! 709): the chats panel down the left and the first screen beside it, each
//! drawn by a confined renderer of this engine's, in a window of the
//! design's size. It holds one new chat and greets a person; it opens no
//! page and no socket, and a press on it is not answered yet (queue item
//! 710). Closing the window stops both renderers.
//!
//! Inside the macOS application, `alo browser.app` (queue item 477, ADRs
//! 0067 and 0068), nothing named opens the application's own page in the
//! machine's Chrome as `--chrome` would — or, on a Mac with no Chrome, in
//! this engine, on a page that says so ([`alo_window::bundle`]). There its
//! input is not watched: macOS gives an application `/dev/null`, which ends
//! as it is read, and it is closed by its windows or by Quit instead.
//!
//! Either way the terminal says which renderer draws the page: this engine,
//! or Chrome by the version it gives itself (ADR 0058 § 6). And while it
//! runs, a person asks it which renderer draws a tab with `alo-ask` and the
//! socket's `renderer` ask, answered by this process and never by the page
//! (queue item 622).

use alo_net::Pool;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::beside::{chrome_on_this_machine, renderer_beside};
use alo_window::bundle::Bundle;
use alo_window::chrome_profile::Swept;
use alo_window::chrome_serving::ChromeServing;
use alo_window::closing::Closing;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::inbox::Orders;
use alo_window::message::Order;
use alo_window::opening::{CHROME_AT, CHROME_SIZE, Opening, Renderer};
use alo_window::pages::RenderedBy;
use std::process::ExitCode;

fn main() -> ExitCode {
    let asked = match Opening::from_arguments(std::env::args().skip(1)) {
        Ok(opening) => opening,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::from(2);
        }
    };
    let bundle = std::env::current_exe()
        .ok()
        .and_then(|program| Bundle::around(&program));
    let opening = match &bundle {
        Some(bundle) => in_the_application(bundle, &asked),
        None => asked,
    };
    if opening.conversation {
        return in_the_conversation(&opening);
    }
    match opening.renderer {
        Renderer::ThisEngine => in_this_engine(&opening),
        Renderer::Chrome => in_chrome(&opening, bundle.is_none()),
    }
}

/// What the application opens when it was asked for `asked`, saying on the
/// terminal when the machine has no Chrome and this engine draws its page.
fn in_the_application(bundle: &Bundle, asked: &Opening) -> Opening {
    let chrome = chrome_on_this_machine();
    let opening = bundle.opening(asked.clone(), chrome.is_ok());
    if let Err(none) = chrome
        && opening != *asked
    {
        eprintln!("alo: {none}, so this browser's own engine draws the application's page");
    }
    opening
}

/// Open a window whose page this engine draws, and run until it closes.
fn in_this_engine(opening: &Opening) -> ExitCode {
    // Read before the window opens, so a file that is not there is said on
    // the terminal rather than as an empty window.
    let first = match opening.first_tab() {
        Ok(order) => order,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::from(2);
        }
    };
    let renderers = match renderers(opening.fonts) {
        Ok(renderers) => renderers,
        Err(failed) => return failed,
    };
    // A session's pool: what it caches and records is in memory and goes
    // with the process (ADR 0011, ADR 0012 § 6).
    let network = match Pool::from_this_machine() {
        Ok(pool) => Network::over(pool),
        Err(why) => {
            eprintln!("alo: this machine's certificates could not be read: {why}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("alo: pages are drawn by {}", RenderedBy::ThisEngine);

    let mut conductor = None;
    let mut refused = None;
    let mut listening = None;
    let ran = alo_window::window::run(opening.scale, |tell| {
        let sites = opening.agents.clone();
        match Conductor::start_keeping_agents_to(
            sites,
            Tabs::over(renderers),
            opening.fonts,
            network,
            tell,
        ) {
            Ok(started) => {
                let orders = started.orders();
                // The first tab, before the window has a size: the conductor
                // holds it until the window says one.
                let _ = orders.send(first);
                listening = listen(&orders, &opening.agents);
                conductor = Some(started);
                orders
            }
            Err(why) => {
                refused = Some(format!("the conductor could not be started: {why}"));
                // A sender nobody receives on: the window opens, says nothing
                // reaches it, and closes when asked.
                alo_window::inbox::inbox().0
            }
        }
    });
    stop_listening(listening);
    let finished = conductor.is_none_or(Conductor::finish);
    match (ran, refused) {
        (Err(why), _) | (Ok(()), Some(why)) => {
            eprintln!("alo: {why}");
            ExitCode::FAILURE
        }
        (Ok(()), None) if !finished => {
            eprintln!("alo: the conductor did not finish");
            ExitCode::FAILURE
        }
        (Ok(()), None) => ExitCode::SUCCESS,
    }
}

/// Open the conversation's window — the chats panel and the first screen,
/// each drawn by a renderer of this engine's — and run until it closes.
fn in_the_conversation(opening: &Opening) -> ExitCode {
    let renderers = match renderers(opening.fonts) {
        Ok(renderers) => renderers,
        Err(failed) => return failed,
    };
    eprintln!(
        "alo: the browser's own interface is drawn by {}",
        RenderedBy::ThisEngine
    );
    match alo_window::window::conversation(opening.scale, renderers) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("alo: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Open the page in the machine's Chrome with a frame of ours around its
/// window, answer agents about it, and run until either window is closed
/// or — when `watching` its input — that input ends.
fn in_chrome(opening: &Opening, watching: bool) -> ExitCode {
    // Read before Chrome is started, so a file that is not there is said
    // without starting it.
    let page = match opening.chrome_page() {
        Ok(page) => page,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::from(2);
        }
    };
    // The page's tab in the frame is drawn by a tab strip's renderer of
    // this engine's, never in this process (item 681): one that is not
    // there is said before Chrome is started.
    let strips = match renderers(opening.fonts) {
        Ok(renderers) => renderers,
        Err(failed) => return failed,
    };
    let (tell, told) = std::sync::mpsc::channel();
    let started = ChromeServing::start_shown(
        page,
        CHROME_SIZE,
        CHROME_AT,
        opening.agents.clone(),
        Box::new(move |news| tell.send(news).is_ok()),
    );
    let serving = match started {
        Ok(serving) => serving,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::FAILURE;
        }
    };
    say_where_chrome_is(&serving);
    let orders = serving.orders();
    let listening = listen(&orders, &opening.agents);
    if watching {
        close_when_input_ends(orders.clone());
        eprintln!(
            "alo: this browser runs until Chrome's window or the frame above it is closed, \
             or its input ends, Ctrl-D at a terminal"
        );
    } else {
        eprintln!(
            "alo: this browser runs until Chrome's window or the frame above it is closed, \
             or it is quit"
        );
    }
    let rendered_by = serving.rendered_by().to_string();
    // Whether the thread holding Chrome finished, once it has been waited for.
    let finished = std::cell::Cell::new(false);
    let frame_orders = orders.clone();
    // Run as the frame's event loop ends, which is also as the program is
    // quit: `AppKit` then ends the process without returning here (item
    // 688), so nothing that has to happen may be left for after `frame`.
    let closing = Closing::of(|| {
        stop_listening(listening);
        // The frame may have gone before Chrome's window did — a failure,
        // or the program quit — and Chrome is closed with it, waited for,
        // and its profile removed.
        let _ = orders.send(Order::CloseEverything);
        drop(orders);
        finished.set(serving.records().is_some());
    });
    let ran = alo_window::window::frame(told, frame_orders, rendered_by, strips, closing);
    match ran {
        Err(why) => {
            eprintln!("alo: {why}");
            ExitCode::FAILURE
        }
        Ok(()) if !finished.get() => {
            eprintln!("alo: Chrome's thread did not finish");
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}

/// The renderers this program starts — the renderer built beside it, with
/// the fonts asked for — or, said on the terminal, why there can be none.
fn renderers(fonts: Fonts) -> Result<Renderers, ExitCode> {
    let renderer = match std::env::current_exe() {
        Ok(program) => renderer_beside(&program),
        Err(why) => {
            eprintln!("alo: where this program is cannot be found: {why}");
            return Err(ExitCode::FAILURE);
        }
    };
    if !renderer.is_file() {
        eprintln!(
            "alo: there is no renderer at {}; build it with `cargo build -p alo-renderer`",
            renderer.display()
        );
        return Err(ExitCode::FAILURE);
    }
    let renderers = Renderers::running(renderer, &[]);
    Ok(match fonts {
        Fonts::AsAsked => renderers.with_machine(alo_renderer::fonts::from_this_machine()),
        Fonts::AsStarted => renderers.with_machine(alo_window::fonts::frozen()),
    })
}

/// Say on the terminal which Chrome draws the page, and where its window
/// went.
fn say_where_chrome_is(serving: &ChromeServing) {
    eprintln!("alo: pages are drawn by {}", serving.rendered_by());
    say_what_was_swept(serving.swept());
    if let Some(placed) = serving.placed() {
        let at = placed.window.bounds;
        eprintln!(
            "alo: Chrome shows the page in a window of its own at {},{}, {}×{} on the screen, \
             the page laid out at {}×{} in it",
            at.left, at.top, at.width, at.height, placed.page.0, placed.page.1
        );
        if let Some(unlike) = placed.unlike(CHROME_SIZE) {
            eprintln!("alo: {unlike}");
        }
    }
}

/// Say what starting Chrome removed of the profiles killed browsers left,
/// once and only when there was any (ADR 0069 § 3): somebody's data gone
/// from a disk is not removed silently.
fn say_what_was_swept(swept: &Swept) {
    let profiles = |count: usize| match count {
        1 => "1 Chrome profile a killed browser".to_owned(),
        count => format!("{count} Chrome profiles killed browsers"),
    };
    if !swept.removed.is_empty() {
        eprintln!("alo: removed {} left", profiles(swept.removed.len()));
    }
    if let Some((first, why)) = swept.unremoved.first() {
        eprintln!(
            "alo: {} left could not be removed, and the next start tries again — {}: {why}",
            profiles(swept.unremoved.len()),
            first.display()
        );
    }
}

/// Close Chrome's window and the frame once this program's input ends, on
/// a thread of its own: Ctrl-D at a terminal, or the end of what a program
/// that started this one sends it.
fn close_when_input_ends(orders: Orders) {
    let watched = std::thread::Builder::new()
        .name("alo-input".to_owned())
        .spawn(move || {
            let read = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
            if let Err(why) = read {
                eprintln!("alo: this browser's input could not be read, and it closes: {why}");
            }
            // Chrome's thread gone is the browser already closing.
            let _ = orders.send(Order::CloseEverything);
        });
    if let Err(why) = watched {
        eprintln!("alo: this browser's input is not watched, and Ctrl-D will not close it: {why}");
    }
}

/// Open the socket an agent speaks to, saying where it is or why there is
/// none.
#[cfg(unix)]
fn listen(
    orders: &alo_window::inbox::Orders,
    sites: &alo_window::agent_sites::AgentSites,
) -> Option<alo_window::agents::Listening> {
    let placed = alo_window::socket_place::prepared()
        .map_err(|why| why.to_string())
        .and_then(|path| {
            alo_window::agents::Listening::at(&path, orders.clone())
                .map_err(|why| format!("{} could not be opened: {why}", path.display()))
        });
    match placed {
        Ok(listening) => {
            eprintln!(
                "alo: an agent reaches this browser at {}",
                listening.path().display()
            );
            if *sites != alo_window::agent_sites::AgentSites::Every {
                eprintln!(
                    "alo: this browser lets agents reach {sites}: a refusal of its own, \
                     which narrows what reaches it and grants nothing"
                );
            }
            Some(listening)
        }
        Err(why) => {
            eprintln!("alo: no agent can reach this browser: {why}");
            None
        }
    }
}

/// Stop listening, removing the socket.
#[cfg(unix)]
fn stop_listening(listening: Option<alo_window::agents::Listening>) {
    if let Some(listening) = listening {
        listening.stop();
    }
}

/// No socket: on Windows how an agent reaches this browser is undecided
/// (ADR 0055 § 1).
#[cfg(not(unix))]
fn listen(_: &alo_window::inbox::Orders, _: &alo_window::agent_sites::AgentSites) -> Option<()> {
    eprintln!("alo: no agent can reach this browser: on Windows how one would is undecided");
    None
}

/// Nothing to stop.
#[cfg(not(unix))]
fn stop_listening(_: Option<()>) {}

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
//! process (item 627, ADR 0061), and the terminal says where it went. No
//! window of ours goes around it yet (item 642), so this browser runs until
//! its input ends — Ctrl-D at a terminal — and then closes Chrome.
//!
//! Either way the terminal says which renderer draws the page: this engine,
//! or Chrome by the version it gives itself (ADR 0058 § 6).

use alo_net::Pool;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::beside::renderer_beside;
use alo_window::chrome_serving::ChromeServing;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::opening::{CHROME_AT, CHROME_SIZE, Opening, Renderer};
use alo_window::pages::RenderedBy;
use std::process::ExitCode;

fn main() -> ExitCode {
    let opening = match Opening::from_arguments(std::env::args().skip(1)) {
        Ok(opening) => opening,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::from(2);
        }
    };
    match opening.renderer {
        Renderer::ThisEngine => in_this_engine(&opening),
        Renderer::Chrome => in_chrome(&opening),
    }
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
    let renderer = match std::env::current_exe() {
        Ok(program) => renderer_beside(&program),
        Err(why) => {
            eprintln!("alo: where this program is cannot be found: {why}");
            return ExitCode::FAILURE;
        }
    };
    if !renderer.is_file() {
        eprintln!(
            "alo: there is no renderer at {}; build it with `cargo build -p alo-renderer`",
            renderer.display()
        );
        return ExitCode::FAILURE;
    }
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
    let renderers = Renderers::running(renderer, &[]);
    let renderers = match opening.fonts {
        Fonts::AsAsked => renderers.with_machine(alo_renderer::fonts::from_this_machine()),
        Fonts::AsStarted => renderers.with_machine(alo_window::fonts::frozen()),
    };

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

/// Open the page in the machine's Chrome, answer agents about it, and run
/// until this program's input ends.
fn in_chrome(opening: &Opening) -> ExitCode {
    // Read before Chrome is started, so a file that is not there is said
    // without starting it.
    let page = match opening.chrome_page() {
        Ok(page) => page,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::from(2);
        }
    };
    let serving =
        match ChromeServing::start_shown(page, CHROME_SIZE, CHROME_AT, opening.agents.clone()) {
            Ok(serving) => serving,
            Err(why) => {
                eprintln!("alo: {why}");
                return ExitCode::FAILURE;
            }
        };
    eprintln!("alo: pages are drawn by {}", serving.rendered_by());
    if let Some(placed) = serving.placed() {
        let at = placed.window.bounds;
        eprintln!(
            "alo: Chrome shows the page in a window of its own at {},{}, {}×{} on the screen, \
             the page laid out at {}×{} in it",
            at.left, at.top, at.width, at.height, placed.page.0, placed.page.1
        );
    }
    let listening = listen(&serving.orders(), &opening.agents);
    eprintln!("alo: this browser runs until its input ends, Ctrl-D at a terminal");
    let waited = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
    stop_listening(listening);
    let finished = serving.records().is_some();
    match waited {
        Err(why) => {
            eprintln!("alo: this browser's input could not be read, and it has closed: {why}");
            ExitCode::FAILURE
        }
        Ok(_) if !finished => {
            eprintln!("alo: Chrome's thread did not finish");
            ExitCode::FAILURE
        }
        Ok(_) => ExitCode::SUCCESS,
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

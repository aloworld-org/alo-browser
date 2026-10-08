/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo`: the browser a person starts (ADR 0024 § 1).
//!
//! `alo [--frozen-fonts] [PAGE.html [SHEET.css ...]]` opens a window with one
//! tab showing the page, or an empty tab if none is named. Closing the window
//! closes every tab and stops every renderer.

use alo_net::Pool;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::opening::Opening;
use std::process::ExitCode;

fn main() -> ExitCode {
    let opening = match Opening::from_arguments(std::env::args().skip(1)) {
        Ok(opening) => opening,
        Err(why) => {
            eprintln!("alo: {why}");
            return ExitCode::from(2);
        }
    };
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
    let renderers = Renderers::running(renderer, &[]);
    let renderers = match opening.fonts {
        Fonts::AsAsked => renderers.with_machine(alo_renderer::fonts::from_this_machine()),
        Fonts::AsStarted => renderers.with_machine(alo_window::fonts::frozen()),
    };

    let mut conductor = None;
    let mut refused = None;
    let ran = alo_window::window::run(|tell| {
        match Conductor::start(Tabs::over(renderers), opening.fonts, network, tell) {
            Ok(started) => {
                let orders = started.orders();
                // The first tab, before the window has a size: the conductor
                // holds it until the window says one.
                let _ = orders.send(first);
                conductor = Some(started);
                orders
            }
            Err(why) => {
                refused = Some(format!("the conductor could not be started: {why}"));
                // A sender nobody receives on: the window opens, says nothing
                // reaches it, and closes when asked.
                std::sync::mpsc::channel().0
            }
        }
    });
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

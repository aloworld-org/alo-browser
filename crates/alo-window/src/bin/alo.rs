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
//! `alo --conversation [--frozen-fonts] [--scale FACTOR] [--chats DIR]
//! [--sign-ins DIR] [--agent-site ADDRESS ...]` opens the browser's own interface as a
//! conversation instead (ADR 0074, queue item 709): the chats panel down
//! the left and the first screen beside it, each drawn by a confined
//! renderer of this engine's, in a window of the design's size. It holds
//! one new chat and greets a person; it opens no page. A press on a chat or *New chat* is answered (queue
//! item 710), and a press on the box lets a person type into it, with *Go
//! to the site* offered for an address and Return deciding to go there
//! (item 701). Going there opens the address in the machine's Chrome — or,
//! inside the application, the Chromium it carries (item 719) — in
//! Chrome's own window put over the first screen and with no frame of ours
//! beside it, its every request held to the same rules and recorded as
//! `--chrome`'s (item 713, ADR 0075); a second address replaces the first,
//! and the first screen's line says what became of each. **An agent
//! reaches the page it shows** (queue item 716, ADR 0075's amendment) over
//! the same socket `--chrome` opens, said on the terminal, for as long as
//! the conversation runs: it follows the newest card, and with none open
//! there is no tab to name; `--agent-site` keeps agents to its sites.
//! Closing the window stops both renderers and closes that Chrome, its
//! profile removed, and the socket.
//!
//! `alo --conversation ADDRESS` starts it at an `http` or `https` address,
//! opened in a new chat once the window is on the screen — unless an alo is
//! already running on the same profile (queue item 976, ADR 0114): the
//! same `--data DIR`, or neither naming one. Then the address is handed to
//! that alo over its socket, which opens it in a new chat and comes
//! forward, and this one says what it answered and ends, having opened
//! nothing. With another `--data DIR` nobody answers at that profile's
//! socket, and it is an instance of its own; so is a hand run naming
//! `--chats` or `--sign-ins`, whose places are its own.
//!
//! `--sign-ins DIR` is a developer's hand run's alone: its key is kept in a
//! keychain file under a password written in alo's source, so inside the
//! application it is refused before anything is made, naming `--data DIR`
//! instead (queue item 973, ADR 0111 § 6, [`alo_window::bundle_refused`]).
//!
//! Inside the macOS application, `alo browser.app` (queue items 477 and
//! 719, ADRs 0067, 0068 and 0077), nothing named opens the application's own
//! page as `--chrome` would, but in **the Chromium the application carries**
//! at `Contents/Frameworks/alo Chromium.app`, alo's own CEF host (ADR 0079,
//! queue item 720) — and every Chromium it starts is
//! that one, never the machine's Chrome, even when one is installed
//! ([`alo_window::bundle_chromium`]). With none there, or one that cannot be
//! started, it draws its page with this engine on a page that says so, and
//! says why on the terminal. There its input is not watched: macOS gives an
//! application `/dev/null`, which ends as it is read, and it is closed by
//! its windows or by Quit instead.
//!
//! Either way the terminal says which renderer draws the page: this engine,
//! or Chrome by the version it gives itself (ADR 0058 § 6), named as the
//! application's own Chromium when it is. And while it
//! runs, a person asks it which renderer draws a tab with `alo-ask` and the
//! socket's `renderer` ask, answered by this process and never by the page
//! (queue item 622).

use alo_net::Pool;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::bundle::Bundle;
use alo_window::bundle_chromium::{decide, undrawn};
use alo_window::chrome_profile::Swept;
use alo_window::chrome_serving::{ChromeServing, Unserved};
use alo_window::closing::Closing;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::data_dir::DataDir;
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
    // What only a developer's hand run names is refused inside the
    // application before anything is made (queue item 973, ADR 0111 § 6).
    if let Some(why) = alo_window::bundle_refused::refused(bundle.as_ref(), &asked) {
        eprintln!("alo: {why}");
        return ExitCode::from(2);
    }
    let decided = decide(bundle.as_ref(), asked.clone());
    if let Some(said) = &decided.said {
        eprintln!("alo: {said}");
    }
    let opening = decided.opening;
    // A data directory a person named is made or refused before anything
    // starts (queue item 957): an alo that cannot keep its own never takes
    // another's places.
    if let Some(named) = opening.data.as_deref() {
        match DataDir::at(named) {
            Ok(data) => eprintln!(
                "alo: everything this alo keeps is kept in {}",
                data.root().display()
            ),
            Err(why) => {
                eprintln!("alo: --data: {why}");
                return ExitCode::from(2);
            }
        }
    }
    if opening.conversation {
        if let Some(handed) = handed_over(&opening) {
            return handed;
        }
        return in_the_conversation(&opening, decided.chromium, bundle.as_ref());
    }
    match opening.renderer {
        Renderer::ThisEngine => in_this_engine(&opening),
        Renderer::Chrome => match in_chrome(&opening, bundle.is_none(), decided.chromium) {
            Ok(ran) => ran,
            // The application's own Chromium could not draw its page: this
            // engine does, never the machine's Chrome (ADR 0077).
            Err(why) => {
                if let Some((instead, said)) = undrawn(bundle.as_ref(), &asked, &why) {
                    eprintln!("alo: {said}");
                    in_this_engine(&instead)
                } else {
                    eprintln!("alo: {why}");
                    ExitCode::FAILURE
                }
            }
        },
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
                listening = listen(&orders, &opening.agents, socket_of(opening));
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
/// each drawn by a renderer of this engine's — and run until it closes, an
/// address chosen in it opened in the Chromium `chromium` names, searching
/// with the key the application `bundle` carries, or this Mac's keychain's.
fn in_the_conversation(opening: &Opening, chromium: Chromium, bundle: Option<&Bundle>) -> ExitCode {
    // Everything this alo keeps in one directory a person named (queue
    // item 957), which `main` made or refused to start without.
    let data = (opening.data.as_deref()).and_then(|named| DataDir::at(named).ok());
    let renderers = match interface_renderers(opening.fonts) {
        Ok(renderers) => renderers,
        Err(failed) => return failed,
    };
    eprintln!(
        "alo: the browser's own interface is drawn by {}",
        RenderedBy::ThisEngine
    );
    // Where its chats (queue item 757, ADR 0092), keys and cards' sign-ins
    // (ADR 0111) are kept: under the data directory a person named (queue
    // item 957), or the directories a hand run named, or alo's own and the
    // person's keychain; said, so a hand run knows which it wrote.
    let keeping = alo_window::data_dir::keeping(
        data.as_ref(),
        opening.chats.clone(),
        opening.sign_ins.as_deref(),
    );
    let chats = keeping.chats.clone();
    // Which services it asks (ADR 0106 § 3): the person's lines in the
    // settings beside the chats, over the application's own defaults, over
    // Brave and OpenAI — each asked with the person's own key, the
    // application's, or an older alo's keychain item (§ 4), read here by
    // the browser process alone and never said; with none, a service says
    // why it is not set up where it would answer.
    let named =
        chats
            .as_deref()
            .ok()
            .map_or_else(alo_window::services::Named::default, |directory| {
                alo_window::settings_file::read(&alo_window::settings_file::file_in(directory))
                    .services
            });
    let defaults = bundle.map_or_else(alo_window::services::Named::default, |bundle| {
        alo_window::services_made::defaults_in(bundle.resources()).unwrap_or_default()
    });
    let (services, unread) = alo_window::services_made::chosen(&named, bundle);
    if let Some(unread) = unread {
        eprintln!("alo: {unread}");
    }
    eprintln!(
        "alo: searches with {} and answers with {}",
        services.search.name(),
        services.answers.name()
    );
    // A person's own keys only where there is a keychain to keep them in
    // (queue item 957): under a data directory, the application's alone.
    let resources = bundle.map(|bundle| bundle.resources().to_path_buf());
    let keys = match keeping.keychain.clone() {
        Some(keychain) => alo_window::service_keys::Keys::in_keychain(
            keychain,
            &alo_window::keychain::this_person(),
            resources,
        ),
        None => alo_window::service_keys::Keys::carried_only(resources),
    };
    let search = alo_window::services_made::search(&services, &keys);
    let answer = alo_window::services_made::ai(&services, &keys);
    if let Ok(directory) = &chats {
        eprintln!("alo: chats are kept in {}", directory.display());
    }
    let sign_ins = keeping.sign_ins.clone();
    eprintln!(
        "{}",
        alo_window::conversation_sign_ins::told(&sign_ins.kept_as())
    );
    // The socket an agent reaches the card live by (queue item 716), kept
    // to the sites named, stopped and removed once every card is closed —
    // as the window's event loop ends, quit included (item 688).
    // alo's own agent (queue item 704, ADR 0096), and assistive technology
    // on the card's page (988), reach it by the same relay's orders.
    let mut assisted = None;
    let (reaching, listening, agent) = match alo_window::conversation_agents::relay(opening.scope())
    {
        Ok((orders, reaching)) => {
            let listening = listen(&orders, &opening.agents, keeping.socket.clone());
            assisted = Some(orders.clone());
            // It begins with no page open through the box's own search
            // (queue item 821), and opens only sites agents are kept
            // to.
            let agent = answer.clone().map(|provider| {
                alo_window::conversation_agent::Agent::new(provider, orders)
                    .searching_with(search.clone().ok())
                    .kept_to(opening.agents.clone())
            });
            (reaching, listening, agent)
        }
        Err(why) => {
            eprintln!("alo: no agent can reach this browser: no thread to reach a card by: {why}");
            (
                alo_window::conversation_agents::Reaching::default(),
                None,
                Err(format!("no thread to reach a card by ({why})")),
            )
        }
    };
    let closing = Closing::of(|| stop_listening(listening));
    match alo_window::window::conversation(
        opening.scale,
        renderers,
        chromium,
        (search, answer, defaults, keys),
        (chats, sign_ins, keeping.keychain.clone()),
        (reaching, closing, agent, assisted),
        opening.address.clone(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("alo: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Open the page in the Chromium `chromium` names — the machine's Chrome,
/// or inside the application its own — with a frame of ours around its
/// window, answer agents about it, and run until either window is closed
/// or — when `watching` its input — that input ends.
///
/// # Errors
///
/// Why that Chromium did not start, say which it is, or open the page,
/// for the caller to say — or, inside the application, to draw its page
/// with this engine instead. Anything else is said here and ends in the
/// exit code returned.
fn in_chrome(opening: &Opening, watching: bool, chromium: Chromium) -> Result<ExitCode, Unserved> {
    // Read before Chrome is started, so a file that is not there is said
    // without starting it.
    let page = match opening.chrome_page() {
        Ok(page) => page,
        Err(why) => {
            eprintln!("alo: {why}");
            return Ok(ExitCode::from(2));
        }
    };
    // The page's tab in the frame is drawn by a tab strip's renderer of
    // this engine's, never in this process (item 681): one that is not
    // there is said before Chrome is started.
    let strips = match renderers(opening.fonts) {
        Ok(renderers) => renderers,
        Err(failed) => return Ok(failed),
    };
    let (tell, told) = std::sync::mpsc::channel();
    let started = ChromeServing::start_shown(
        chromium,
        page,
        CHROME_SIZE,
        CHROME_AT,
        opening.scope(),
        Box::new(move |news| tell.send(news).is_ok()),
    );
    let serving = started?;
    say_where_chrome_is(&serving);
    let orders = serving.orders();
    let listening = listen(&orders, &opening.agents, socket_of(opening));
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
    Ok(match ran {
        Err(why) => {
            eprintln!("alo: {why}");
            ExitCode::FAILURE
        }
        Ok(()) if !finished.get() => {
            eprintln!("alo: Chrome's thread did not finish");
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    })
}

/// When the conversation was started at an address and an alo is already
/// running on the same profile (queue item 976, ADR 0114): the address
/// handed to it, what it answered said, and how this alo ends. [`None`]
/// when this alo is to be the instance: no address, a hand run's own
/// places, or nobody answering at the profile's socket — a socket a
/// stopped alo left included, which this alo's own then clears.
#[cfg(unix)]
fn handed_over(opening: &Opening) -> Option<ExitCode> {
    use alo_window::launch_handover::{
        ANSWERED_WITHIN, Handover, hand_over, socket_of_the_profile,
    };
    let address = opening.address.as_ref()?;
    if opening.chats.is_some() || opening.sign_ins.is_some() {
        return None;
    }
    let data = opening
        .data
        .as_deref()
        .and_then(|named| DataDir::at(named).ok());
    let socket = socket_of_the_profile(data.as_ref())?;
    let address = address.to_string();
    match hand_over(&socket, &address, ANSWERED_WITHIN) {
        Handover::NobodyThere => None,
        Handover::Answered { opened, said } => {
            eprintln!(
                "alo: an alo is already running on this profile, at {}: {address} was handed \
                 to it, to open in a new chat and come forward",
                socket.display()
            );
            eprintln!("alo: it answered: {said}");
            // What only an instance uses is the running one's own, never
            // this launch's: said, rather than quietly dropped.
            let own = opening.fonts != Fonts::AsAsked
                || opening.scale.is_some()
                || opening.agents != alo_window::agent_sites::AgentSites::Every
                || opening.saves != alo_window::agent_saves::AgentSaves::none();
            if own {
                eprintln!(
                    "alo: the running alo keeps its own fonts, scale, agents' sites and \
                     agents' saves; this launch's were not handed to it"
                );
            }
            Some(if opened {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Handover::Unanswered(why) => {
            eprintln!(
                "alo: {address} was handed to the alo already running on this profile, at {}, \
                 which has not said whether it opened: {why}",
                socket.display()
            );
            Some(ExitCode::FAILURE)
        }
    }
}

/// No socket, so no alo to hand an address to: on Windows how one would be
/// reached is undecided (ADR 0055 § 1).
#[cfg(not(unix))]
fn handed_over(_: &Opening) -> Option<ExitCode> {
    None
}

/// Where the socket goes when `opening` named a data directory (queue item
/// 957): in it — which `main` made, or refused to start without.
fn socket_of(opening: &Opening) -> Option<std::path::PathBuf> {
    let named = opening.data.as_deref()?;
    DataDir::at(named).ok().map(|data| data.socket())
}

/// The renderers the conversation's own interface is drawn by, as
/// [`renderers`] starts them — given the machine's Chinese, Japanese and
/// Korean faces, read out of their collections, for the interface's
/// renderers alone (queue item 767); under `--frozen-fonts`, none, so the
/// window is the corpus.
fn interface_renderers(fonts: Fonts) -> Result<Renderers, ExitCode> {
    let renderers = renderers(fonts)?;
    Ok(match fonts {
        Fonts::AsAsked => {
            let faces = alo_window::fonts::with_cjk(
                renderers.fonts(),
                alo_renderer::cjk_faces::of_this_machine(),
            );
            renderers.with_fonts(faces)
        }
        Fonts::AsStarted => renderers,
    })
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
    at: Option<std::path::PathBuf>,
) -> Option<alo_window::agents::Listening> {
    // In the data directory a person named (queue item 957), or the
    // person's runtime directory.
    let placed = at
        .map_or_else(alo_window::socket_place::prepared, |at| {
            alo_window::socket_place::prepare(&at)
        })
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
fn listen(
    _: &alo_window::inbox::Orders,
    _: &alo_window::agent_sites::AgentSites,
    _: Option<std::path::PathBuf>,
) -> Option<()> {
    eprintln!("alo: no agent can reach this browser: on Windows how one would is undecided");
    None
}

/// Nothing to stop.
#[cfg(not(unix))]
fn stop_listening(_: Option<()>) {}

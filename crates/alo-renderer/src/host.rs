/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The browser side: one renderer process per site.
//!
//! # What this file is for, in one sentence
//!
//! So that a page which finds a way to take over the process it is rendered in
//! has taken over **a process that holds one site's pages and nothing else** —
//! no network, no disk, no profile, and no other site's anything.
//!
//! # What a dead renderer looks like
//!
//! ADR 0005: *"The tab keeps the last frame it painted and says what happened.
//! Every other tab is untouched, because no other tab was in that process.
//! Reloading is the user's to ask for; a browser that silently restarts a
//! renderer hides a bug that somebody needs to see."*
//!
//! So [`Renderers::ask`] returns a [`Gone`] when a renderer has died, and does
//! **not** start another one to retry. The site's entry is dropped, so the next
//! *deliberate* load gets a fresh process — but nothing here decides on its own
//! that a page should be reloaded.
//!
//! # And what one that has stopped answering without dying looks like
//!
//! The same thing, which is the point. A renderer that is alive and silent used
//! to be nothing at all — an exchange had no bound, so the browser process
//! waited in a read for ever and every other tab waited with it. It is a
//! [`Gone`] now, after [`crate::answers::LONGEST_SILENCE`], and the process is
//! stopped on the way: a tab hears the same sentence in the same shape as one
//! whose renderer crashed, because from a person's side those are the same
//! event.

use crate::answers::{Answers, LONGEST_SILENCE};
use crate::face::Face;
use crate::generic::Generics;
use crate::message::{FromRenderer, ToRenderer};
use crate::pipe::{self, Arrived};
use crate::sandbox;
use crate::site::Site;
use crate::wire;
use std::collections::{HashMap, HashSet};
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::time::Duration;

/// How many renderer processes may exist at once.
///
/// ADR 0005: *"N processes cost N processes. Memory goes up and it is the price
/// of the first reason in this document."* A bound, because the price has to
/// have a ceiling — a browser with three hundred tabs open on three hundred
/// sites cannot be three hundred processes.
pub const MOST_RENDERERS: usize = 16;

/// Why a renderer could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gone {
    /// Which site's renderer.
    pub site: String,
    /// What happened, in words somebody can be shown.
    pub why: String,
}

impl core::fmt::Display for Gone {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "the renderer for {} is gone: {}", self.site, self.why)
    }
}

impl std::error::Error for Gone {}

/// One renderer process, and the two ends of its pipe.
#[derive(Debug)]
struct Held {
    child: Child,
    to: BufWriter<ChildStdin>,
    /// What it has said, read on a thread of its own so that waiting for it can
    /// be given a bound — see [`crate::answers`] for why that needs a thread.
    answers: Answers,
}

/// The renderers a browser process is holding.
#[derive(Debug)]
pub struct Renderers {
    program: PathBuf,
    arguments: Vec<String>,
    /// The fonts handed to every renderer as it starts.
    ///
    /// Held here because a renderer cannot open a font file (ADR 0010), so the
    /// browser process reads them once and gives each new renderer a copy —
    /// rather than each renderer going looking, which is the thing it cannot
    /// do.
    faces: Vec<Face>,
    /// What the generic families mean on this machine, handed over with them.
    ///
    /// Held here for the same reason and it is the more important half: every
    /// page ever loaded asks for `system-ui, sans-serif` through the user-agent
    /// sheet, and a renderer that was never told what those are answers them by
    /// falling off the end of its fallback chain.
    generics: Generics,
    held: HashMap<Site, Held>,
    /// Which site was used least recently, oldest first — so the bound above
    /// evicts something rather than refusing to open a tab.
    order: Vec<Site>,
    started: usize,
    /// How long a renderer may say nothing before it is given up on.
    ///
    /// A field rather than the constant used in place, so that a test can name
    /// a bound of its own: one that waits [`LONGEST_SILENCE`] to find out that
    /// a wedged renderer is killed is a test nobody runs.
    patience: Duration,
}

impl Renderers {
    /// Renderers started by running this program with these arguments.
    ///
    /// The program is a parameter rather than `current_exe()` so that a test
    /// can point at a real binary, and so that a build which puts the renderer
    /// somewhere else does not need this file changed.
    pub fn running(program: impl Into<PathBuf>, arguments: &[&str]) -> Self {
        Self {
            program: program.into(),
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            faces: Vec::new(),
            generics: Generics::new(),
            held: HashMap::new(),
            order: Vec::new(),
            started: 0,
            patience: LONGEST_SILENCE,
        }
    }

    /// The same, giving a renderer this long to answer before it is stopped.
    ///
    /// [`LONGEST_SILENCE`] otherwise, and that constant says why it is the
    /// number it is. This exists for a test, and for whoever one day puts the
    /// question to a person instead.
    #[must_use]
    pub fn waiting_at_most(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    /// The same, giving every renderer these fonts.
    #[must_use]
    pub fn with_fonts(mut self, faces: Vec<Face>) -> Self {
        self.faces = faces;
        self
    }

    /// The same, with what this machine's generic families mean.
    #[must_use]
    pub fn with_generics(mut self, generics: Generics) -> Self {
        self.generics = generics;
        self
    }

    /// Everything one machine had to say: its fonts and its generics, together.
    ///
    /// The pair rather than two calls, because the generics name families that
    /// have to be among the faces — [`crate::fonts::from_this_machine`] decides
    /// them together for that reason, and splitting them here would be the one
    /// place they could be given to a renderer apart.
    #[must_use]
    pub fn with_machine(self, machine: crate::fonts::Machine) -> Self {
        self.with_fonts(machine.faces)
            .with_generics(machine.generics)
    }

    /// The fonts every renderer is given.
    pub fn fonts(&self) -> &[Face] {
        &self.faces
    }

    /// What every renderer is told the generic families mean.
    pub fn generics(&self) -> &Generics {
        &self.generics
    }

    /// How many are running.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// How many have been started since this browser process began.
    ///
    /// For a test, and for anybody asking whether renderers are being reused
    /// or quietly restarted.
    pub fn started(&self) -> usize {
        self.started
    }

    /// Whether a renderer is running for this site.
    pub fn holds(&self, site: &Site) -> bool {
        self.held.contains_key(site)
    }

    /// The process id of a site's renderer, for a test that wants to kill it.
    pub fn process_of(&self, site: &Site) -> Option<u32> {
        self.held.get(site).map(|held| held.child.id())
    }

    /// Send work to a site's renderer, starting one if there is none.
    ///
    /// # Errors
    ///
    /// [`Gone`] when the renderer could not be started, died, or **said nothing
    /// for [`Renderers::waiting_at_most`]** — the last of which stops it, since
    /// an answer that arrived after we gave up waiting would be handed back as
    /// the answer to somebody else's question. A renderer that answered with a
    /// refusal or a failure is **not** an error here — that is the renderer
    /// doing its job, and it comes back as a [`FromRenderer`].
    pub fn ask(&mut self, site: &Site, work: &ToRenderer) -> Result<FromRenderer, Gone> {
        if !self.held.contains_key(site) {
            self.start(site)?;
        }
        self.remember_as_newest(site);
        let sent = wire::write_to_renderer(work);
        let patience = self.patience;

        let answer = {
            let Some(held) = self.held.get_mut(site) else {
                return Err(self.lost(site, "it could not be started"));
            };
            match pipe::write(&mut held.to, &sent) {
                Ok(()) => held.answers.within(patience).map_err(|why| why.to_string()),
                Err(why) => Err(why.to_string()),
            }
        };

        match answer {
            Ok(Arrived::Message(bytes)) => match wire::read_from_renderer(&bytes) {
                Ok(message) => Ok(message),
                // A renderer that sends something unreadable is a renderer that
                // is not itself any more. Nothing it says can be believed, so
                // it is dropped rather than asked again.
                Err(why) => Err(self.lost(site, &format!("it said something unreadable: {why}"))),
            },
            Ok(Arrived::Ended) => Err(self.lost(site, "it exited")),
            Err(why) => Err(self.lost(site, &why)),
        }
    }

    /// Answer a renderer that said which families it wanted and did not have.
    ///
    /// [`FromRenderer::Loaded`] carries that list. A renderer may not open a
    /// font file (ADR 0010), so this side goes and looks, sends over every face
    /// it found, and **returns the families that are genuinely not on this
    /// machine** — which is the answer the caller shows, because a family
    /// nobody has is a substitution somebody should be told about rather than a
    /// silence.
    ///
    /// The page has already been laid out in the wrong font by the time this
    /// runs. Loading it again is the caller's to decide and deliberately not
    /// done here: a browser that reloaded on its own would render every page
    /// asking for a missing font twice, and ADR 0005's rule that nothing
    /// restarts a renderer by itself is the same rule.
    ///
    /// # Errors
    ///
    /// [`Gone`] when the renderer died while being sent a font. Nothing found
    /// is not an error — it is this machine's honest answer.
    pub fn supply(&mut self, site: &Site, families: &[String]) -> Result<Vec<String>, Gone> {
        let mut absent = Vec::new();
        // Bounded again here, having been bounded in the renderer that built
        // the list: a renderer is the process that parsed a hostile page, so a
        // limit it applied to itself is not one this side may rely on.
        for family in families.iter().take(crate::families::MOST_WANTED) {
            let faces = crate::fonts::named(family);
            if faces.is_empty() {
                absent.push(family.clone());
                continue;
            }
            for face in faces {
                // Kept, so a renderer started for this site later begins with
                // the font rather than asking for it again.
                if !self.faces.iter().any(|held| held == &face) {
                    self.faces.push(face.clone());
                }
                match self.ask(site, &ToRenderer::UseFont(Box::new(face)))? {
                    // A font this machine has and this renderer will not take
                    // is not on the machine as far as the page is concerned,
                    // and saying otherwise would report a family as supplied
                    // while the text stayed in the wrong one.
                    FromRenderer::UsingFont { .. } => {}
                    _ => absent.push(family.clone()),
                }
            }
        }
        absent.dedup();
        Ok(absent)
    }

    /// Stop a site's renderer.
    pub fn stop(&mut self, site: &Site) {
        if let Some(mut held) = self.held.remove(site) {
            let _ = held.child.kill();
            let _ = held.child.wait();
        }
        self.order.retain(|held| held != site);
    }

    /// Stop every renderer nothing wants any more, and say which those were.
    ///
    /// The **reaping** half of the lifecycle, and the one this file did not
    /// have: a renderer whose last tab has closed used to run until the ceiling
    /// happened to evict it, which is what happens when reaping has *not*
    /// happened rather than a way of doing it. A person who closes a tab has
    /// stopped wanting the process behind it, and sixteen of somebody else's
    /// pages held open behind a bound is the memory ADR 0005 says the split
    /// costs, spent on nothing.
    ///
    /// # Why the caller says what it wants rather than what to stop
    ///
    /// Because a caller that named a process to end would be deciding the
    /// lifecycle from outside this file, on the strength of happening to hold
    /// the last reference to it. What a caller actually knows is which sites it
    /// still has open — [`crate::tab::Tabs`] knows that and nothing else about
    /// processes — and turning that into an ending is this file's to do. It is
    /// also the safer direction: a site left out of `wanted` by mistake costs a
    /// process that starts again, and one left in by mistake would be a
    /// renderer nothing can ever reach.
    ///
    /// Sites in `wanted` with no renderer are not started; this only ever ends
    /// things. Stopping is [`Renderers::stop`]'s kill and wait, so a reaped
    /// renderer is gone by the time this returns rather than left as a zombie
    /// for whoever waits next.
    pub fn reap(&mut self, wanted: &HashSet<Site>) -> Vec<Site> {
        let unwanted: Vec<Site> = self
            .held
            .keys()
            .filter(|site| !wanted.contains(*site))
            .cloned()
            .collect();
        for site in &unwanted {
            self.stop(site);
        }
        unwanted
    }

    /// Stop all of them, which is what closing the browser means.
    pub fn stop_everything(&mut self) {
        let sites: Vec<Site> = self.held.keys().cloned().collect();
        for site in sites {
            self.stop(&site);
        }
    }

    fn start(&mut self, site: &Site) -> Result<(), Gone> {
        while self.held.len() >= MOST_RENDERERS {
            let Some(oldest) = self.order.first().cloned() else {
                break;
            };
            self.stop(&oldest);
        }
        // ADR 0010: confined before it reads a byte of any page, and no
        // rendering at all if it cannot be. A renderer that ran unconfined
        // would have removed a protection the person believes they have, at
        // the moment it found out it could not provide it.
        let mut command =
            sandbox::confined(&self.program, &self.arguments).map_err(|why| Gone {
                site: site.to_string(),
                why: why.why,
            })?;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Left alone deliberately: a renderer's own diagnostics go to the
            // terminal, where a person can see them, rather than into a pipe
            // nobody reads and which would fill and block the process.
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|why| Gone {
                site: site.to_string(),
                why: format!("it could not be started: {why}"),
            })?;
        let (Some(to), Some(from)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(Gone {
                site: site.to_string(),
                why: "it started without a pipe".to_owned(),
            });
        };
        self.held.insert(
            site.clone(),
            Held {
                child,
                to: BufWriter::new(to),
                answers: Answers::read_from(from),
            },
        );
        self.order.push(site.clone());
        self.started += 1;

        // Hand over the fonts before any page. A renderer that received a page
        // first would lay it out with nothing to draw text in, and the result
        // would be a rendering difference nobody could explain from outside.
        //
        // Then what the generics mean, and in that order: a generic names a
        // family, and a renderer asked which of them it can answer before it
        // holds any face would truthfully say none of them.
        let mut opening: Vec<ToRenderer> = self
            .faces
            .clone()
            .into_iter()
            .map(|face| ToRenderer::UseFont(Box::new(face)))
            .collect();
        if !self.generics.is_empty() {
            opening.push(ToRenderer::UseGenerics(self.generics.clone()));
        }
        let patience = self.patience;
        for work in opening {
            let sent = wire::write_to_renderer(&work);
            let Some(held) = self.held.get_mut(site) else {
                break;
            };
            if pipe::write(&mut held.to, &sent).is_err() {
                break;
            }
            // Read the answer, so the two ends stay in step — the next thing
            // written would otherwise be read as the answer to this. Bounded
            // like every other wait for a renderer: a process that goes silent
            // while being handed a font is as capable of hanging this one as
            // any other, and the first page is what finds out.
            if held.answers.within(patience).is_err() {
                break;
            }
        }
        Ok(())
    }

    /// Drop a site's renderer and say why.
    ///
    /// Dropped rather than restarted: a browser that silently starts another
    /// one hides a bug somebody needs to see, and would turn a page that
    /// crashes its renderer every time into an invisible loop.
    fn lost(&mut self, site: &Site, why: &str) -> Gone {
        self.stop(site);
        Gone {
            site: site.to_string(),
            why: why.to_owned(),
        }
    }

    fn remember_as_newest(&mut self, site: &Site) {
        self.order.retain(|held| held != site);
        self.order.push(site.clone());
    }
}

impl Drop for Renderers {
    /// A browser process that exits leaves no renderers behind.
    fn drop(&mut self) {
        self.stop_everything();
    }
}

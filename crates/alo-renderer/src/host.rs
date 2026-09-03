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

use crate::face::Face;
use crate::message::{FromRenderer, ToRenderer};
use crate::pipe::{self, Arrived};
use crate::sandbox;
use crate::site::Site;
use crate::wire;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

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
    from: BufReader<ChildStdout>,
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
    held: HashMap<Site, Held>,
    /// Which site was used least recently, oldest first — so the bound above
    /// evicts something rather than refusing to open a tab.
    order: Vec<Site>,
    started: usize,
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
            held: HashMap::new(),
            order: Vec::new(),
            started: 0,
        }
    }

    /// The same, giving every renderer these fonts.
    #[must_use]
    pub fn with_fonts(mut self, faces: Vec<Face>) -> Self {
        self.faces = faces;
        self
    }

    /// The fonts every renderer is given.
    pub fn fonts(&self) -> &[Face] {
        &self.faces
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
    /// [`Gone`] when the renderer could not be started, or died. A renderer
    /// that answered with a refusal or a failure is **not** an error here —
    /// that is the renderer doing its job, and it comes back as a
    /// [`FromRenderer`].
    pub fn ask(&mut self, site: &Site, work: &ToRenderer) -> Result<FromRenderer, Gone> {
        if !self.held.contains_key(site) {
            self.start(site)?;
        }
        self.remember_as_newest(site);
        let sent = wire::write_to_renderer(work);

        let answer = {
            let Some(held) = self.held.get_mut(site) else {
                return Err(self.lost(site, "it could not be started"));
            };
            match pipe::write(&mut held.to, &sent) {
                Ok(()) => pipe::read(&mut held.from).map_err(|why| why.why),
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

    /// Stop a site's renderer.
    pub fn stop(&mut self, site: &Site) {
        if let Some(mut held) = self.held.remove(site) {
            let _ = held.child.kill();
            let _ = held.child.wait();
        }
        self.order.retain(|held| held != site);
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
                from: BufReader::new(from),
            },
        );
        self.order.push(site.clone());
        self.started += 1;

        // Hand over the fonts before any page. A renderer that received a page
        // first would lay it out with nothing to draw text in, and the result
        // would be a rendering difference nobody could explain from outside.
        for face in self.faces.clone() {
            let sent = wire::write_to_renderer(&ToRenderer::UseFont(Box::new(face)));
            let Some(held) = self.held.get_mut(site) else {
                break;
            };
            if pipe::write(&mut held.to, &sent).is_err() {
                break;
            }
            // Read the answer, so the two ends stay in step — the next thing
            // written would otherwise be read as the answer to this.
            if pipe::read(&mut held.from).is_err() {
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

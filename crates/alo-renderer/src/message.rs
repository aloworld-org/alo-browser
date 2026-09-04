/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The protocol: what crosses the boundary, and in which direction.
//!
//! ADR 0005: *"the browser process sends work; a renderer returns results"*. So
//! there are exactly two types here, one for each direction, and no third one
//! for a renderer calling back — because there is no calling back.
//!
//! # Coarse on purpose
//!
//! Serialisation costs something on every crossing, so a request asks for *a
//! layout*, *a frame*, *the tree* — never for a box at a time. ADR 0005 says
//! it plainly: a chatty protocol is the way this design becomes slow, and it is
//! easier to keep coarse than to make coarse.
//!
//! # Owned, all of it
//!
//! Every type here is `Clone` and `Send + 'static` and holds no borrow. That
//! is the property that makes a transport possible later without changing a
//! single caller. The transport itself is queue item 29's, and inventing a wire
//! format before there is a process to send it to would be inventing.

use crate::face::Face;
use crate::frame::Frame;
use crate::generic::Generics;
use crate::page::Page;
use crate::snapshot::Snapshot;
use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_layout::Size;
use core::fmt;

/// Work for a renderer.
#[derive(Debug, Clone, PartialEq)]
pub enum ToRenderer {
    /// Here is a font, as bytes.
    ///
    /// A confined renderer cannot open a font file (ADR 0010), so the browser
    /// process reads them and hands them over. Sent once per renderer rather
    /// than with each page, because ADR 0005 asks for a coarse protocol and a
    /// font resent with every load would be megabytes a page.
    UseFont(Box<Face>),
    /// This is what the generic families mean here.
    ///
    /// `sans-serif` is a fact about the **machine**, and a machine is the thing
    /// a confined renderer may not look at (ADR 0010) — so it arrives the same
    /// way a font does. Sent once, after the fonts, because a generic names a
    /// family and a family is only real once its faces are here.
    UseGenerics(Generics),
    /// Render this page.
    Load(Box<Page>),
    /// The window is a different size now.
    Resize(Size),
    /// Draw what is loaded.
    Paint,
    /// Read the page as an agent reads it.
    ReadTree,
    /// Do something to the page, naming what to do it to.
    Act {
        /// What to act on, described rather than pointed at (ADR 0002).
        target: Target,
        /// What to do.
        verb: Verb,
    },
}

/// What a renderer answers with.
#[derive(Debug, Clone, PartialEq)]
pub enum FromRenderer {
    /// A font was taken, and this is what it turned out to be called.
    ///
    /// The family comes back rather than being assumed, because a font file
    /// may not be the family the browser process guessed from its name — and a
    /// renderer drawing with something other than what was asked for is a
    /// rendering difference nobody could explain from the outside.
    UsingFont {
        /// The family it was filed under.
        family: String,
    },
    /// The generics were taken, and these are the ones that now mean something.
    ///
    /// Not an echo of what was sent: a generic mapped to a family this renderer
    /// was never given resolves to nothing, and reporting it as understood would
    /// be the browser process believing every page on the machine has a
    /// `sans-serif` while text was still coming out in whatever was to hand.
    /// So a generic is here only when a face answers it.
    UsingGenerics {
        /// The generic names that now resolve to a face, in the order they were
        /// sent.
        answering: Vec<String>,
    },
    /// A page was rendered, with everything the engine refused along the way.
    ///
    /// The issues come back rather than being logged, because a page that
    /// looks wrong is nearly always a page that was told something the engine
    /// could not do, and the browser process is where a person can be shown.
    Loaded {
        /// What was refused, in the order it was met.
        issues: Vec<String>,
        /// The font families this page asked for by name and this renderer does
        /// not have, in the order it first asked.
        ///
        /// A renderer may not open a font file (ADR 0010), so this is the only
        /// way it can ask for one — and the browser process is the only side
        /// that can answer, because it is the side allowed to look. Sending it
        /// with the load rather than as a message of its own keeps the protocol
        /// coarse: which families a page wants is known exactly when the page
        /// has been laid out, and that is this answer.
        ///
        /// **It is a request, not an instruction.** The names come from a page
        /// a stranger wrote, so the browser process treats each as a string to
        /// look up among the fonts it already knows about — never as a path.
        wanted: Vec<String>,
    },
    /// A picture.
    Painted(Frame),
    /// The tree, as it was at that instant.
    Tree(Box<Snapshot>),
    /// A verb ran, and this is what it did.
    Acted(Outcome),
    /// A verb was refused. **Not a failure**: ADR 0002 makes refusing a
    /// result, because acting on the wrong row is worse than acting on none.
    Refused(Refusal),
    /// The request could not be answered at all.
    Failed(Failure),
}

/// Why a request could not be answered.
///
/// Distinct from [`FromRenderer::Refused`], which is the engine doing its job.
/// This is the engine unable to do it, and a renderer that answers with one is
/// still usable afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Nothing is loaded, so there is nothing to answer about.
    NothingLoaded,
    /// Bytes that were offered as a font and are not one.
    ///
    /// Refused rather than kept: a font that does not parse would fail at the
    /// moment text is shaped, which is a long way from the moment somebody
    /// could have been told.
    NotAFont {
        /// What it was offered as.
        family: String,
    },
    /// A size no picture can be made at.
    Unpaintable {
        /// What was asked for, in words.
        why: String,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::NothingLoaded => f.write_str("nothing is loaded"),
            Failure::NotAFont { family } => {
                write!(f, "the bytes offered as {family:?} are not a font")
            }
            Failure::Unpaintable { why } => write!(f, "nothing could be painted: {why}"),
        }
    }
}

impl fmt::Display for ToRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToRenderer::UseFont(face) => write!(
                f,
                "use {} bytes as the font {:?}",
                face.bytes.len(),
                face.family
            ),
            ToRenderer::UseGenerics(generics) => {
                let said: Vec<String> = generics
                    .pairs()
                    .iter()
                    .map(|(generic, family)| format!("{generic} is {family:?}"))
                    .collect();
                if said.is_empty() {
                    f.write_str("no generic family means anything here")
                } else {
                    write!(f, "{}", said.join(", "))
                }
            }
            ToRenderer::Load(page) => write!(
                f,
                "load {} bytes of markup at {}×{}",
                page.html.len(),
                page.viewport.width,
                page.viewport.height,
            ),
            ToRenderer::Resize(size) => write!(f, "resize to {}×{}", size.width, size.height),
            ToRenderer::Paint => f.write_str("paint"),
            ToRenderer::ReadTree => f.write_str("read the tree"),
            ToRenderer::Act { target, verb } => write!(f, "{verb:?} {target}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything that crosses the boundary must be able to.
    ///
    /// Not a formality: the whole of ADR 0005's "the shape is the expensive
    /// part" comes down to this. A message holding a borrow, or a handle, or
    /// anything tied to this process compiles perfectly well today and cannot
    /// be sent tomorrow — and by then everything is written against it.
    fn could_be_sent<T: Send + Clone + 'static>() {}

    #[test]
    fn every_message_could_be_sent_to_another_process() {
        could_be_sent::<ToRenderer>();
        could_be_sent::<FromRenderer>();
        could_be_sent::<Failure>();
        could_be_sent::<Page>();
        could_be_sent::<Frame>();
        could_be_sent::<Snapshot>();
    }

    /// ADR 0012 § 7: **no page, ever, and not the agent either.** The record of
    /// what this browser asked for is kept for the person and is *about* the
    /// agent, and one readable by script is a cross-site history oracle handed
    /// out by the browser.
    ///
    /// Kept by the shape rather than by a check. A renderer holds no
    /// [`alo_net::Pool`] and therefore no [`alo_net::Activity`], and nothing in
    /// either direction of this boundary carries a line of one — which
    /// [`crate::wire`] then cannot encode, because it can only encode what these
    /// enums hold.
    ///
    /// This test **is** the enforcement rather than a description of it: a
    /// variant added to either enum makes one of these matches non-exhaustive,
    /// and the diff that fixes it is the diff where somebody has to say what the
    /// new message carries. `alo-agent` needs no clause of its own — it does not
    /// depend on `alo-net` at all, so no [`Outcome`] can name a record.
    #[test]
    fn nothing_crossing_the_boundary_carries_the_record_of_what_was_asked_for() {
        let sent = |work: &ToRenderer| match work {
            ToRenderer::UseFont(_)
            | ToRenderer::UseGenerics(_)
            | ToRenderer::Load(_)
            | ToRenderer::Resize(_)
            | ToRenderer::Paint
            | ToRenderer::ReadTree
            | ToRenderer::Act { .. } => "a page, a font, or a thing to do to one",
        };
        let answered = |answer: &FromRenderer| match answer {
            FromRenderer::UsingFont { .. }
            | FromRenderer::UsingGenerics { .. }
            | FromRenderer::Loaded { .. }
            | FromRenderer::Painted(_)
            | FromRenderer::Tree(_)
            | FromRenderer::Acted(_)
            | FromRenderer::Refused(_)
            | FromRenderer::Failed(_) => "what became of it, and nothing about any other request",
        };

        assert!(!sent(&ToRenderer::Paint).is_empty());
        assert!(!answered(&FromRenderer::Failed(Failure::NothingLoaded)).is_empty());
    }

    #[test]
    fn a_request_says_what_it_is_asking_for() {
        let page = Page::new("<p>hi</p>", Size::new(800.0, 600.0));
        assert_eq!(
            ToRenderer::Load(Box::new(page)).to_string(),
            "load 9 bytes of markup at 800×600",
        );
        assert_eq!(ToRenderer::Paint.to_string(), "paint");
        assert_eq!(ToRenderer::ReadTree.to_string(), "read the tree");
        assert_eq!(
            ToRenderer::Resize(Size::new(320.0, 480.0)).to_string(),
            "resize to 320×480",
        );
    }

    #[test]
    fn a_failure_says_what_could_not_be_done() {
        assert_eq!(Failure::NothingLoaded.to_string(), "nothing is loaded");
        assert_eq!(
            Failure::Unpaintable {
                why: "no size".to_owned(),
            }
            .to_string(),
            "nothing could be painted: no size",
        );
    }
}

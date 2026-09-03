/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The renderer: one page, and the answers to questions about it.
//!
//! ADR 0005's boundary, as a type. [`Renderer::handle`] takes work and returns
//! a result, and that is the whole of its surface — no callback, no handle to
//! call back through, nowhere to wait. A renderer in another process would
//! have exactly this shape, which is the point of building it before there is
//! one.
//!
//! # Nothing ambient
//!
//! Everything a renderer needs arrives in a message or in [`Renderer::new`].
//! Fonts are the interesting case: a sandboxed renderer cannot open a font
//! file, so in the split they are handed to it by the browser process. They
//! are a constructor argument here for that reason rather than for tidiness.

use crate::face::Face;
use crate::frame::Frame;
use crate::generic::Generics;
use crate::message::{Failure, FromRenderer, ToRenderer};
use crate::page::Page;
use crate::pipeline::{Rendered, render, render_document};
use crate::snapshot::Snapshot;
use alo_agent::{AgentTree, apply, perform};
use alo_agent::{Target, Verb};
use alo_layout::Size;
use alo_text::Font;
use alo_text::FontDatabase;

/// Everything that touches a page.
pub struct Renderer {
    fonts: FontDatabase,
    /// The page as it was sent, kept so that a resize can render it again.
    page: Option<Page>,
    /// What the last render produced.
    rendered: Option<Rendered>,
}

impl Renderer {
    /// A renderer that draws with these fonts and holds no page yet.
    pub fn new(fonts: FontDatabase) -> Self {
        Self {
            fonts,
            page: None,
            rendered: None,
        }
    }

    /// Take a font the browser process handed over.
    ///
    /// A confined renderer cannot go and find one (ADR 0010), so this is the
    /// only way it gets any. Bytes that do not parse are **refused here**
    /// rather than kept: a font that fails at the moment text is shaped fails a
    /// long way from the moment somebody could have been told.
    fn use_font(&mut self, face: &Face) -> FromRenderer {
        match Font::load(&face.family, face.weight(), face.slant, face.bytes.clone()) {
            Some(font) => {
                let family = font.family().to_owned();
                self.fonts.add(font);
                FromRenderer::UsingFont { family }
            }
            None => FromRenderer::Failed(Failure::NotAFont {
                family: face.family.clone(),
            }),
        }
    }

    /// Take what the browser process says the generic families mean.
    ///
    /// A renderer cannot work this out for itself — `sans-serif` is a fact about
    /// the machine, and the machine is what ADR 0010 confines it away from. What
    /// it *can* do is say which of them it can now answer, and that is what
    /// comes back: a generic whose family is not among the faces this renderer
    /// holds resolves to nothing, and reporting it as understood would tell the
    /// browser process every page here has a `sans-serif` while text kept coming
    /// out in whatever was to hand.
    fn use_generics(&mut self, generics: &Generics) -> FromRenderer {
        for (generic, family) in generics.pairs() {
            self.fonts.map_generic(generic, family);
        }
        let answering = generics
            .named()
            .into_iter()
            .filter(|generic| self.fonts.holds(generic))
            .map(ToOwned::to_owned)
            .collect();
        FromRenderer::UsingGenerics { answering }
    }

    /// Do one piece of work, and answer.
    ///
    /// The only way in. Every request is answered — with a result, with a
    /// refusal, or with a [`Failure`] that leaves the renderer usable.
    pub fn handle(&mut self, work: ToRenderer) -> FromRenderer {
        match work {
            ToRenderer::UseFont(face) => self.use_font(&face),
            ToRenderer::UseGenerics(generics) => self.use_generics(&generics),
            ToRenderer::Load(page) => self.load(*page),
            ToRenderer::Resize(viewport) => match self.page.clone() {
                Some(page) => self.load(Page { viewport, ..page }),
                None => FromRenderer::Failed(Failure::NothingLoaded),
            },
            ToRenderer::Paint => self.paint(),
            ToRenderer::ReadTree => self.read_tree(),
            ToRenderer::Act { target, verb } => self.act(&target, &verb),
        }
    }

    /// What the last render produced, for a test that asserts on the engine's
    /// insides.
    ///
    /// Not part of the boundary and never sent anywhere: a display list is not
    /// something a browser process asks for. The corpus reaches in because it
    /// is a test of the engine rather than of the browser, and ADR 0005 says
    /// tests stay single-process.
    pub fn rendered(&self) -> Option<&Rendered> {
        self.rendered.as_ref()
    }

    /// Decide what a verb does, carry it into the document, and render again.
    ///
    /// Three steps, in that order, and they cannot be fewer. The **decision**
    /// is made against the tree the agent read; the **change** is made to the
    /// document, which the tree was borrowing and so could not touch; and the
    /// page is **rendered again**, because a document that changed and a
    /// layout that did not are two structures that disagree.
    fn act(&mut self, target: &Target, verb: &Verb) -> FromRenderer {
        let Some(rendered) = &self.rendered else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let tree = AgentTree::new(&rendered.document, &rendered.boxes, &rendered.layout);
        let outcome = match perform(&tree, target, verb) {
            Ok(outcome) => outcome,
            Err(refusal) => return FromRenderer::Refused(refusal),
        };

        let Some(mut rendered) = self.rendered.take() else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let changed = apply(&mut rendered.document, &rendered.boxes, &outcome)
            .iter()
            .any(|change| *change != alo_agent::Change::Nothing);
        if changed {
            // The whole page again, from the **same document**. Correct before
            // fast: working out what a changed attribute could possibly have
            // affected is a cache, and a wrong cache is a wrong pixel nobody
            // can find. Re-parsing would be worse than slow — it would mint new
            // node ids and break every snapshot anybody was holding.
            let sheets = self
                .page
                .as_ref()
                .map(|page| page.sheets.join("\n"))
                .unwrap_or_default();
            let viewport = self
                .page
                .as_ref()
                .map_or(rendered.layout.viewport(), |page| page.viewport);
            self.rendered = Some(render_document(
                rendered.document,
                &sheets,
                viewport,
                &self.fonts,
            ));
        } else {
            self.rendered = Some(rendered);
        }
        FromRenderer::Acted(outcome)
    }

    fn load(&mut self, page: Page) -> FromRenderer {
        let sheets = page.sheets.join("\n");
        let rendered = render(&page.html, &sheets, page.viewport, &self.fonts);
        let issues = rendered.issues();
        // A renderer may not go and find a font (ADR 0010), so saying which
        // families it wanted and did not have is the whole of what it can do
        // about one — and the browser process, which may look, is exactly who
        // is listening.
        let wanted = rendered.wanted.families.clone();
        self.page = Some(page);
        self.rendered = Some(rendered);
        FromRenderer::Loaded { issues, wanted }
    }

    fn paint(&self) -> FromRenderer {
        let Some(rendered) = &self.rendered else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        if rendered.canvas.is_empty() {
            return FromRenderer::Failed(Failure::Unpaintable {
                why: "the window has no size".to_owned(),
            });
        }
        FromRenderer::Painted(Frame::from_canvas(&rendered.canvas))
    }

    fn read_tree(&self) -> FromRenderer {
        let Some(rendered) = &self.rendered else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let tree = AgentTree::new(&rendered.document, &rendered.boxes, &rendered.layout);
        FromRenderer::Tree(Box::new(Snapshot::of(&tree)))
    }

    /// How big the page it holds is, if it holds one.
    pub fn viewport(&self) -> Option<Size> {
        self.page.as_ref().map(|page| page.viewport)
    }
}

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

use crate::frame::Frame;
use crate::message::{Failure, FromRenderer, ToRenderer};
use crate::page::Page;
use crate::pipeline::{Rendered, render};
use crate::snapshot::Snapshot;
use alo_agent::{AgentTree, perform};
use alo_layout::Size;
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

    /// Do one piece of work, and answer.
    ///
    /// The only way in. Every request is answered — with a result, with a
    /// refusal, or with a [`Failure`] that leaves the renderer usable.
    pub fn handle(&mut self, work: ToRenderer) -> FromRenderer {
        match work {
            ToRenderer::Load(page) => self.load(*page),
            ToRenderer::Resize(viewport) => match self.page.clone() {
                Some(page) => self.load(Page { viewport, ..page }),
                None => FromRenderer::Failed(Failure::NothingLoaded),
            },
            ToRenderer::Paint => self.paint(),
            ToRenderer::ReadTree => self.read_tree(),
            ToRenderer::Act { target, verb } => {
                let Some(rendered) = &self.rendered else {
                    return FromRenderer::Failed(Failure::NothingLoaded);
                };
                let tree = AgentTree::new(&rendered.document, &rendered.boxes, &rendered.layout);
                match perform(&tree, &target, &verb) {
                    Ok(outcome) => FromRenderer::Acted(outcome),
                    Err(refusal) => FromRenderer::Refused(refusal),
                }
            }
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

    fn load(&mut self, page: Page) -> FromRenderer {
        let sheets = page.sheets.join("\n");
        let rendered = render(&page.html, &sheets, page.viewport, &self.fonts);
        let issues = rendered.issues();
        self.page = Some(page);
        self.rendered = Some(rendered);
        FromRenderer::Loaded { issues }
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

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Rendering a case the way its page would be rendered.
//!
//! **A case whose page carries no script is markup, rendered**: the pipeline
//! in one call, as every case was until a script could change a document.
//!
//! **A case whose page carries script is a page, loaded** — through a
//! [`Renderer`], which runs the page's scripts before it draws and draws
//! from the document they left (ADR 0017 §§ 2 and 6). Rendering such a page
//! as markup would commit a picture of a page nobody sees, and the reference
//! render of what a script did is what queue item 250 was closed by
//! (`cases/a-script-grows-a-list`).
//!
//! A loaded page is told it runs on [`crate::SYSTEM`], whatever machine
//! renders it, so a script that reads `navigator` draws the same reference
//! everywhere.
//!
//! A loaded page is served from the case's address, and what its script
//! fetches is answered from the responses the case froze, through the
//! browser process's own decision and filter ([`crate::answering`]); a
//! fetch it froze nothing for is a network error.
//!
//! A loaded page's linked style sheets are asked for by its renderer and
//! answered from the files its `linked.txt` froze, by URL, through the
//! browser process's own decision and check ([`crate::sheets`], ADR 0035
//! § 6). A loaded case cannot link a picture yet: a renderer asks for none
//! until queue item 350, and a case freezing one would be rendered without
//! it and committed that way, so the frozen file nobody asked for is refused
//! by name instead.

use alo_dom::Document;
use alo_dom::scripts::{Carried, carried};
use alo_layout::Size;
use alo_renderer::{Drawing, FromRenderer, Page, Rendered, Renderer, ToRenderer};

use crate::answering::{self, Answered, Asks, Beside};
use crate::case::Case;

/// A case, rendered.
pub enum Rendering {
    /// Markup with no script, through the pipeline.
    Markup(Box<Rendered>),
    /// A page with script, loaded by a renderer, and what answering its
    /// fetches and sheets came to, with what the load itself said.
    Loaded(Box<Renderer>, Answered),
}

impl Rendering {
    /// Render `case` the way its page would be.
    ///
    /// # Errors
    ///
    /// What is wrong with a case that cannot be rendered that way: a page
    /// with script that links what a renderer does not ask for, an answer
    /// the corpus cannot give, or a load the renderer refused.
    pub fn of(case: &Case) -> Result<Self, String> {
        let size = Size::new(case.size.0, case.size.1);
        let runs_script = carried(&alo_dom::parse_document(&case.html))
            .iter()
            .any(|found| matches!(found, Carried::Script(_)));
        if !runs_script {
            return Ok(Self::Markup(Box::new(alo_renderer::render_with_resources(
                &case.html,
                &case.css,
                size,
                &crate::corpus_fonts(),
                &case.linked,
                &case.resources,
            ))));
        }
        let mut renderer = Renderer::new(crate::corpus_fonts());
        let mut page = Page::new(case.html.clone(), size).with_sheet(case.css.clone());
        page.user_agent = crate::SYSTEM.user_agent();
        crate::SYSTEM.platform.clone_into(&mut page.platform);
        if let Some(address) = &case.address {
            page.url = alo_url::parse(address)
                .map_err(|why| format!("its address.txt is not an address: {why}"))?;
        }
        let address = page.url.clone();
        match renderer.handle(ToRenderer::Load(Box::new(page))) {
            FromRenderer::Loaded {
                issues,
                fetches,
                sheets,
                ..
            } => {
                let beside = Beside {
                    responses: &case.responses,
                    files: &case.frozen,
                };
                let mut answered =
                    answering::answer(&mut renderer, &address, beside, Asks { fetches, sheets })?;
                answered.loaded = issues;
                Ok(Self::Loaded(Box::new(renderer), answered))
            }
            other => Err(format!("the renderer did not load it: {other:?}")),
        }
    }

    /// The document it was drawn from.
    pub fn document(&self) -> Option<&Document> {
        match self {
            Self::Markup(rendered) => Some(&rendered.document),
            Self::Loaded(renderer, _) => renderer.document(),
        }
    }

    /// What was drawn.
    pub fn drawing(&self) -> Option<&Drawing> {
        match self {
            Self::Markup(rendered) => Some(&rendered.drawing),
            Self::Loaded(renderer, _) => renderer.rendered(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(html: &str) -> Case {
        Case {
            directory: std::path::PathBuf::new(),
            name: "a case".to_owned(),
            html: html.to_owned(),
            css: String::new(),
            size: (40.0, 20.0),
            linked: Vec::new(),
            resources: Vec::new(),
            frozen: Vec::new(),
            address: None,
            responses: Vec::new(),
        }
    }

    fn frozen(name: &str, file: &str, bytes: &[u8]) -> crate::case::Frozen {
        crate::case::Frozen {
            name: name.to_owned(),
            file: file.to_owned(),
            bytes: bytes.to_vec(),
        }
    }

    /// The computed `color` of the first `<p>`.
    fn colour(rendering: &Rendering) -> Option<String> {
        let document = rendering.document()?;
        let drawing = rendering.drawing()?;
        let paragraph = document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name.is_html("p"))
        })?;
        drawing
            .styles
            .get(paragraph)
            .and_then(|style| style.get("color"))
            .map(ToOwned::to_owned)
    }

    #[test]
    fn a_case_whose_page_runs_script_is_loaded_and_its_script_has_run() {
        let rendering = Rendering::of(&case(
            "<p>a</p><script>document.documentElement.lastChild.firstChild.remove()</script>",
        ));
        let Ok(rendering @ Rendering::Loaded(..)) = rendering else {
            panic!("not loaded by a renderer");
        };
        let paragraphs = rendering.document().map(|document| {
            document
                .descendants(document.root())
                .filter(|id| document.element(*id).is_some_and(|e| e.name.is_html("p")))
                .count()
        });
        assert_eq!(paragraphs, Some(0), "the script did not run");
    }

    #[test]
    fn a_case_without_script_is_markup_rendered() {
        assert!(matches!(
            Rendering::of(&case("<p>a</p>")),
            Ok(Rendering::Markup(_))
        ));
    }

    #[test]
    fn a_loaded_cases_linked_sheet_is_asked_for_answered_and_drawn() {
        let mut linking = case("<link rel=stylesheet href=/a.css><p>a</p><script>1</script>");
        linking.address = Some("https://example.com/page".to_owned());
        linking
            .frozen
            .push(frozen("/a.css", "a.css", b"p { color: rgb(1 2 3) }"));
        let Ok(rendering @ Rendering::Loaded(..)) = Rendering::of(&linking) else {
            panic!("not loaded by a renderer");
        };
        assert_eq!(colour(&rendering).as_deref(), Some("rgb(1 2 3)"));
        let Rendering::Loaded(_, answered) = rendering else {
            panic!("not loaded by a renderer");
        };
        assert_eq!((answered.delivered, answered.sheets), (1, 1));
        assert!(answered.unfrozen.is_empty() && answered.said.is_empty());
    }

    #[test]
    fn a_loaded_cases_unfrozen_sheet_is_drawn_without_and_said() {
        let mut linking = case("<link rel=stylesheet href=/a.css><p>a</p><script>1</script>");
        linking.address = Some("https://example.com/page".to_owned());
        let Ok(Rendering::Loaded(_, answered)) = Rendering::of(&linking) else {
            panic!("not loaded by a renderer");
        };
        assert_eq!(answered.unfrozen, ["https://example.com/a.css"]);
        assert!(
            answered
                .issues
                .iter()
                .any(|issue| issue.contains("did not arrive")),
            "{:?}",
            answered.issues
        );
    }

    #[test]
    fn a_loaded_case_that_freezes_a_picture_is_refused_by_name() {
        // A loaded page asks for no picture until queue item 350, so it
        // would be drawn without one and committed that way.
        let mut picturing = case("<img src=/a.png><script>1</script>");
        picturing.address = Some("https://example.com/page".to_owned());
        picturing.frozen.push(frozen("/a.png", "a.png", b"\x89PNG"));
        let Err(why) = Rendering::of(&picturing) else {
            panic!("rendered without what it linked");
        };
        assert!(
            why.contains("never asked for") && why.contains("350"),
            "{why}"
        );
    }

    #[test]
    fn what_a_load_said_is_kept_with_what_its_answers_said() {
        // Nothing is fetched or linked, so there is no answer: the throw is
        // the load's.
        let Ok(Rendering::Loaded(_, answered)) =
            Rendering::of(&case("<script>undefined.x</script>"))
        else {
            panic!("not loaded by a renderer");
        };
        assert_eq!(answered.delivered, 0);
        assert!(
            answered
                .loaded
                .iter()
                .any(|issue| issue.contains("TypeError")),
            "{:?}",
            answered.loaded
        );
        assert!(answered.issues.is_empty(), "{:?}", answered.issues);
    }
}

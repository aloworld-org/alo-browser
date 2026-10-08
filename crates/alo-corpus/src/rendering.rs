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
//! A loaded case cannot link a sheet or a picture yet: a renderer is handed
//! its sheets as text and no pictures at all (`Page`), and a case that asked
//! for both would be rendered without what it linked and committed that
//! way. Refused by name instead.

use alo_dom::Document;
use alo_dom::scripts::{Carried, carried};
use alo_layout::Size;
use alo_renderer::{Drawing, FromRenderer, Page, Rendered, Renderer, ToRenderer};

use crate::answering::{self, Answered};
use crate::case::Case;

/// A case, rendered.
pub enum Rendering {
    /// Markup with no script, through the pipeline.
    Markup(Box<Rendered>),
    /// A page with script, loaded by a renderer, and what answering its
    /// fetches came to.
    Loaded(Box<Renderer>, Answered),
}

impl Rendering {
    /// Render `case` the way its page would be.
    ///
    /// # Errors
    ///
    /// What is wrong with a case that cannot be rendered that way: a page
    /// with script that links what a renderer cannot be handed, or a load the
    /// renderer refused.
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
        if !case.linked.is_empty() || !case.resources.is_empty() {
            return Err(
                "its page runs script, so it is loaded by a renderer, which is handed no linked \
                 sheet or picture"
                    .to_owned(),
            );
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
            FromRenderer::Loaded { fetches, .. } => {
                let answered =
                    answering::answer(&mut renderer, &address, &case.responses, fetches)?;
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
            address: None,
            responses: Vec::new(),
        }
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
    fn a_case_whose_page_runs_script_and_links_a_sheet_is_refused_by_name() {
        let mut linking = case("<link rel=stylesheet href=a.css><script>1</script>");
        linking.linked.push(("a.css".to_owned(), "p {}".to_owned()));
        let Err(why) = Rendering::of(&linking) else {
            panic!("rendered without what it linked");
        };
        assert!(why.contains("no linked sheet or picture"), "{why}");
    }
}

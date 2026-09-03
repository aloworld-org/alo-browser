/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `sans-serif` means a font on the machine this is running on, or it means
//! nothing and says so.
//!
//! Queue item 193. `FontDatabase::map_generic` had existed since stage 1 and
//! only tests called it: the browser process handed over faces and never said
//! which of them was this machine's `sans-serif`. So the user-agent sheet's own
//! `font-family: system-ui, sans-serif` — which reaches *every* page — named two
//! families nobody had, and was answered by falling off the end of the fallback
//! chain into whatever face sorted first.
//!
//! The three things checked here are the three the item closes on: a renderer is
//! told what the generics mean as part of being given fonts; a page asking for
//! one on a machine that has it is not reported as substituted; and a page
//! asking on a machine that has none still is.

use alo_css::media::ColorScheme;
use alo_layout::geometry::Size;
use alo_renderer::face::Face;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::renderer::Renderer;
use alo_renderer::site::Site;
use alo_renderer::snapshot::SnapshotNode;
use alo_text::{FontDatabase, Slant, Weight};

const RENDERER: &str = env!("CARGO_BIN_EXE_alo-render");

/// The one word every case below is laid out from, so the numbers in the layout
/// assertion are comparable across them.
const TEXT: &str = "generic";

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

fn a_face() -> Face {
    // Built directly rather than through `Face::new`: a helper outside a test
    // may not panic, and there is no sensible fallback for "the font this crate
    // is tested with is missing".
    Face {
        family: "DejaVu Sans".to_owned(),
        weight: Weight::NORMAL.value(),
        slant: Slant::Normal,
        bytes: dejavu::sans::regular().to_vec(),
    }
}

/// A machine that has one font and knows what its `sans-serif` is.
fn a_machine_with_a_sans_serif() -> Generics {
    Generics::stating(vec![("sans-serif".to_owned(), "DejaVu Sans".to_owned())])
}

fn asking_for(family: &str) -> ToRenderer {
    ToRenderer::Load(Box::new(Page {
        html: format!("<p>{TEXT}</p>"),
        sheets: vec![format!(
            "html, body, p {{ margin: 0; padding: 0 }} \
             p {{ font-family: {family}; font-size: 16px }}"
        )],
        viewport: Size {
            width: 300.0,
            height: 60.0,
        },
        scheme: ColorScheme::Light,
    }))
}

// --- Across the boundary -----------------------------------------------------

/// The item's first clause: told as part of being given fonts.
///
/// Through the real process and the real pipe, because that is where the telling
/// happens — `Renderers::start` sends the faces and then the mapping, and a test
/// that called `Renderer::handle` directly would check everything except whether
/// anybody sends it.
#[test]
fn a_renderer_is_told_what_the_generics_mean_when_it_is_given_its_fonts() {
    let mut renderers = Renderers::running(RENDERER, &[])
        .with_fonts(vec![a_face()])
        .with_generics(a_machine_with_a_sans_serif());
    let site = Site::of(&url("https://example.com/"));

    // The very first thing asked of it is a page written in a generic, and it
    // can already answer — so the mapping arrived before any page did.
    let answer = renderers.ask(&site, &asking_for("sans-serif"));
    let Ok(FromRenderer::Loaded { issues, wanted }) = answer else {
        panic!("the page did not load: {answer:?}");
    };
    assert!(
        !wanted.iter().any(|family| family == "sans-serif"),
        "a renderer that was told what sans-serif means asked for it: {wanted:?}",
    );
    assert!(
        !issues.iter().any(|issue| issue.contains("sans-serif")),
        "and nothing was substituted, so nothing should say it was: {issues:?}",
    );
}

/// The item's third clause, and the one that keeps the second honest: a machine
/// that genuinely has no `sans-serif` still says so.
#[test]
fn a_page_asking_on_a_machine_with_no_sans_serif_is_still_told() {
    let mut renderers = Renderers::running(RENDERER, &[]).with_fonts(vec![a_face()]);
    let site = Site::of(&url("https://example.com/"));

    let answer = renderers.ask(&site, &asking_for("sans-serif"));
    let Ok(FromRenderer::Loaded { issues, wanted }) = answer else {
        panic!("the page did not load: {answer:?}");
    };
    assert!(
        wanted.iter().any(|family| family == "sans-serif"),
        "nobody said what sans-serif means here, and nobody asked: {wanted:?}",
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("sans-serif") && issue.contains("DejaVu Sans")),
        "the page was drawn in something else and was not told: {issues:?}",
    );
}

// --- What a renderer will and will not claim ---------------------------------

/// A generic mapped to a family this renderer was never given resolves to
/// nothing. Answering "understood" would tell the browser process that every
/// page here has a `monospace` while text kept coming out in whatever was to
/// hand — so the answer names only the generics a face actually answers.
#[test]
fn a_renderer_says_which_generics_it_can_actually_answer() {
    let mut renderer = Renderer::new(FontDatabase::new());
    renderer.handle(ToRenderer::UseFont(Box::new(a_face())));

    let answer = renderer.handle(ToRenderer::UseGenerics(Generics::stating(vec![
        ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
        ("monospace".to_owned(), "A Font Nobody Sent".to_owned()),
    ])));
    let FromRenderer::UsingGenerics { answering } = answer else {
        panic!("the mapping was not taken: {answer:?}");
    };
    assert_eq!(answering, vec!["sans-serif".to_owned()]);
}

/// A renderer with no fonts at all is a real state — it is the state every
/// renderer starts in — and it claims nothing.
#[test]
fn a_renderer_with_no_fonts_answers_no_generic() {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = renderer.handle(ToRenderer::UseGenerics(a_machine_with_a_sans_serif()));
    assert_eq!(
        answer,
        FromRenderer::UsingGenerics { answering: vec![] },
        "a generic was claimed by a renderer holding nothing to draw it with",
    );
}

/// A generic that means several families in order is the ordinary case on a
/// well-stocked machine, and the order is the machine's preference.
#[test]
fn a_generic_that_means_several_families_keeps_them_in_order() {
    let mut renderer = Renderer::new(FontDatabase::new());
    renderer.handle(ToRenderer::UseFont(Box::new(a_face())));
    let answer = renderer.handle(ToRenderer::UseGenerics(Generics::stating(vec![
        ("sans-serif".to_owned(), "A Font Nobody Sent".to_owned()),
        ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
    ])));
    assert_eq!(
        answer,
        FromRenderer::UsingGenerics {
            answering: vec!["sans-serif".to_owned()],
        },
        "a generic whose first family is missing is still answered by its second",
    );
}

// --- The layout, in numbers --------------------------------------------------

/// A generic is not a decoration: it decides what the text is **measured** in,
/// and so where every line in the page breaks.
///
/// The numbers are the assertion. Text asking for `sans-serif` on a machine that
/// has one lays out to exactly the box it lays out to when the family is named
/// outright — and a renderer nobody told lays it out to a different one, because
/// it fell off the end of the chain into a font with different metrics. Two
/// renders that differ by nothing but a mapping nobody sent are what this item
/// was opened for.
#[test]
fn text_in_a_generic_is_measured_in_the_family_the_generic_means() {
    let named = width_of_the_text(&["DejaVu Sans"], Generics::new(), "'DejaVu Sans'");
    let generic = width_of_the_text(
        &["DejaVu Sans"],
        a_machine_with_a_sans_serif(),
        "sans-serif",
    );
    let unmapped = width_of_the_text(&["DejaVu Serif"], Generics::new(), "sans-serif");

    assert!(named > 0.0, "the named family measured nothing: {named}");
    assert!(
        (generic - named).abs() < 0.01,
        "text in `sans-serif` measured {generic} and the family it means measured {named}",
    );
    assert!(
        (unmapped - named).abs() > 0.5,
        "a renderer nobody told which family is sans-serif measured the text at \
         {unmapped}, which is what one that was told measured — so this test \
         proves nothing",
    );
}

/// The width of the one text box on the page, laid out by a renderer holding
/// these families and told these generics.
///
/// Through the agent tree rather than through the display list, because the
/// snapshot is what actually crosses the boundary — a display list is not a
/// thing a browser process asks for.
fn width_of_the_text(families: &[&str], generics: Generics, asked_for: &str) -> f32 {
    let mut renderer = Renderer::new(FontDatabase::new());
    for family in families {
        renderer.handle(ToRenderer::UseFont(Box::new(Face {
            family: (*family).to_owned(),
            weight: Weight::NORMAL.value(),
            slant: Slant::Normal,
            bytes: match *family {
                "DejaVu Serif" => dejavu::serif::regular().to_vec(),
                _ => dejavu::sans::regular().to_vec(),
            },
        })));
    }
    if !generics.is_empty() {
        renderer.handle(ToRenderer::UseGenerics(generics));
    }
    renderer.handle(asking_for(asked_for));
    let FromRenderer::Tree(snapshot) = renderer.handle(ToRenderer::ReadTree) else {
        return 0.0;
    };
    snapshot
        .root
        .as_ref()
        .and_then(|root| widest_named(root, TEXT))
        .unwrap_or_default()
}

/// The widest box whose name is this text — the paragraph, whatever depth the
/// tree put it at.
fn widest_named(node: &SnapshotNode, text: &str) -> Option<f32> {
    let mine = node
        .name
        .as_deref()
        .filter(|name| name.trim() == text)
        .map(|_| node.rect.size.width);
    node.children
        .iter()
        .filter_map(|child| widest_named(child, text))
        .chain(mine)
        .fold(None, |widest: Option<f32>, width| {
            Some(widest.map_or(width, |held| held.max(width)))
        })
}

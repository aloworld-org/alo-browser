/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A renderer draws with a font it was given and never with one it went
//! looking for.
//!
//! ADR 0010 confines a renderer, and the consequence people underestimate is
//! that it cannot open a font file. There were two ways out and the ADR chose
//! the harder one: the browser process reads them and hands over the bytes,
//! rather than the sandbox policy permitting a font directory. This is that
//! choice, checked — including the part that would have made the easy way
//! tempting, which is that a renderer with no fonts really cannot get any.

use alo_css::media::ColorScheme;
use alo_layout::geometry::Size;
use alo_renderer::face::Face;
use alo_renderer::fonts;
use alo_renderer::host::Renderers;
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::sandbox;
use alo_renderer::site::Site;
use alo_text::{Slant, Weight};

const RENDERER: &str = env!("CARGO_BIN_EXE_alo-render");

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
    // Built directly rather than through `Face::new`, because a helper outside
    // a test may not panic and there is no sensible fallback for "the embedded
    // font is missing" — the fields are the whole of a face.
    Face {
        family: "DejaVu Sans".to_owned(),
        weight: Weight::NORMAL.value(),
        slant: Slant::Normal,
        bytes: dejavu::sans::regular().to_vec(),
    }
}

fn a_page() -> ToRenderer {
    ToRenderer::Load(Box::new(Page {
        html: "<p>text needs a font</p>".to_owned(),
        sheets: vec!["p { font-family: 'DejaVu Sans'; font-size: 16px }".to_owned()],
        viewport: Size {
            width: 200.0,
            height: 60.0,
        },
        scheme: ColorScheme::Light,
    }))
}

// --- Handed over, not gone looking for ---------------------------------------

/// The whole item, in one exchange: bytes go across, and the renderer says what
/// it actually found — rather than echoing back the name the browser process
/// guessed from a filename.
#[test]
fn a_renderer_takes_a_font_it_is_handed_and_says_what_it_is() {
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));

    let answer = renderers.ask(&site, &ToRenderer::UseFont(Box::new(a_face())));
    let Ok(FromRenderer::UsingFont { family }) = answer else {
        panic!("the font was not taken: {answer:?}");
    };
    assert!(
        !family.is_empty(),
        "a renderer should say which family it filed the bytes under"
    );
}

/// A font that fails at the moment text is shaped fails a long way from the
/// moment somebody could have been told.
#[test]
fn bytes_that_are_not_a_font_are_refused_when_they_arrive() {
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));
    let rubbish = Face::new(
        "Not A Font",
        Weight::NORMAL,
        Slant::Normal,
        b"this is not a font, it is a sentence".to_vec(),
    )
    .unwrap_or(Face {
        family: "Not A Font".to_owned(),
        weight: Weight::NORMAL.value(),
        slant: Slant::Normal,
        bytes: b"this is not a font, it is a sentence".to_vec(),
    });

    let answer = renderers.ask(&site, &ToRenderer::UseFont(Box::new(rubbish)));
    assert!(
        matches!(answer, Ok(FromRenderer::Failed(Failure::NotAFont { .. }))),
        "rubbish was accepted as a font: {answer:?}"
    );
}

/// The renderer that a browser process was given fonts for gets them **before**
/// its first page — a renderer handed a page first would lay it out with
/// nothing to draw text in, and the result would be a rendering difference
/// nobody could explain from outside.
#[test]
fn a_renderer_has_its_fonts_before_it_is_given_a_page() {
    let mut renderers = Renderers::running(RENDERER, &[]).with_fonts(vec![a_face()]);
    let site = Site::of(&url("https://example.com/"));

    // The very first thing asked of it is a page, and it can already draw.
    assert!(matches!(
        renderers.ask(&site, &a_page()),
        Ok(FromRenderer::Loaded { .. })
    ));
    let painted = renderers.ask(&site, &ToRenderer::Paint);
    let Ok(FromRenderer::Painted(frame)) = painted else {
        panic!("nothing was painted: {painted:?}");
    };
    assert_eq!(frame.pixels.len(), 200 * 60 * 4);
    assert!(
        frame.pixels.iter().any(|byte| *byte != frame.pixels[0]),
        "the frame is one flat colour, so nothing was drawn"
    );
}

/// The part that would have made permitting a font directory tempting: a
/// confined renderer that was given nothing really has nothing, and cannot go
/// and find any.
#[test]
fn a_renderer_given_no_fonts_has_none_and_cannot_fetch_any() {
    if !sandbox::is_available() {
        return;
    }
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));
    assert!(renderers.fonts().is_empty());

    // It still renders — a page with no text to shape is a page — and the
    // point is that it did not acquire a font on the way.
    assert!(matches!(
        renderers.ask(&site, &a_page()),
        Ok(FromRenderer::Loaded { .. })
    ));

    // And the confinement that makes that true is the same one item 167
    // watches: the renderer cannot read a file.
    let arguments = vec!["--check-confinement".to_owned()];
    let confined =
        sandbox::confined(std::path::Path::new(RENDERER), &arguments).and_then(|mut command| {
            command
                .output()
                .map_err(|why| alo_renderer::sandbox::Unconfined {
                    why: why.to_string(),
                })
        });
    assert!(
        confined.is_ok_and(|done| done.status.success()),
        "a renderer with no fonts was not actually confined, so it could have found one"
    );
}

// --- What the browser process finds ------------------------------------------

/// The other side of the decision: somebody still has to open the files, and it
/// is the process that is allowed to.
#[test]
fn the_browser_process_finds_fonts_on_this_machine() {
    let found = fonts::from_this_machine();
    assert!(
        !found.is_empty(),
        "no fonts were found on a machine that certainly has some"
    );
    assert!(
        found.len() <= alo_renderer::face::MOST_FONTS,
        "more fonts than one renderer is given"
    );
    assert!(
        found.iter().all(|face| !face.bytes.is_empty()),
        "a font was found with no bytes in it"
    );
    // Sorted, so two runs on the same machine hand a renderer the same fonts in
    // the same order — which is what makes a rendering difference between runs
    // mean something.
    let mut sorted = found.clone();
    sorted.sort_by(|one, two| one.family.cmp(&two.family));
    assert_eq!(
        found.iter().map(|face| &face.family).collect::<Vec<_>>(),
        sorted.iter().map(|face| &face.family).collect::<Vec<_>>()
    );
}

/// A machine's own fonts, through the boundary, drawing a page. If the family
/// naming or the byte carrying were wrong anywhere, this is where it would
/// show.
#[test]
fn a_page_is_drawn_with_a_font_that_came_from_this_machine() {
    let found = fonts::from_this_machine();
    if found.is_empty() {
        return;
    }
    let family = found
        .first()
        .map(|face| face.family.clone())
        .unwrap_or_default();
    let mut renderers = Renderers::running(RENDERER, &[]).with_fonts(found);
    let site = Site::of(&url("https://example.com/"));

    let loaded = renderers.ask(
        &site,
        &ToRenderer::Load(Box::new(Page {
            html: "<p>drawn with this machine's own font</p>".to_owned(),
            sheets: vec![format!("p {{ font-family: '{family}'; font-size: 16px }}")],
            viewport: Size {
                width: 300.0,
                height: 60.0,
            },
            scheme: ColorScheme::Light,
        })),
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert!(matches!(
        renderers.ask(&site, &ToRenderer::Paint),
        Ok(FromRenderer::Painted(_))
    ));
}

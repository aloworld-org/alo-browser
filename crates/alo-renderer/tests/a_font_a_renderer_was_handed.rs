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
    let machine = fonts::from_this_machine();
    let found = &machine.faces;
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
    // Ordered, so two runs on the same machine hand a renderer the same fonts
    // in the same order — which is what makes a rendering difference between
    // runs mean something. The families a generic means come first, because the
    // cut down to what one renderer is given must not be what decides whether
    // this machine has a `sans-serif`; the rest follow by name.
    let names_a_generic = |family: &str| {
        machine
            .generics
            .pairs()
            .iter()
            .any(|(_, named)| named.eq_ignore_ascii_case(family))
    };
    let mut sorted = found.clone();
    sorted.sort_by(|one, two| {
        (!names_a_generic(&one.family), one.family.clone())
            .cmp(&(!names_a_generic(&two.family), two.family.clone()))
    });
    assert_eq!(
        found.iter().map(|face| &face.family).collect::<Vec<_>>(),
        sorted.iter().map(|face| &face.family).collect::<Vec<_>>()
    );
}

/// What this machine's generics mean, on this machine.
///
/// Queue item 193. Which families they are is not something a test can assert —
/// it differs by machine, which is the whole reason the mapping exists — so what
/// is asserted is the property that has to hold **everywhere**: a generic names
/// only families the renderer was actually handed. A mapping naming anything
/// else is a generic that resolves to nothing, reported to a person as a family
/// this machine does not have, while the browser process believes it sent one.
#[test]
fn a_generic_family_names_a_font_this_renderer_was_given() {
    let machine = fonts::from_this_machine();
    if machine.faces.is_empty() {
        return;
    }
    for (generic, family) in machine.generics.pairs() {
        assert!(
            machine
                .faces
                .iter()
                .any(|face| face.family.eq_ignore_ascii_case(family)),
            "{generic} was said to mean {family:?}, which was not handed over",
        );
        assert!(
            alo_renderer::generic::is_a_candidate(family),
            "{family:?} was chosen for {generic} and is in no candidate list",
        );
    }
    // And the other direction, so this test is not vacuous on the machine it is
    // run on: a family a generic would like to mean, which was handed over and
    // which no generic means, is a mapping that was dropped somewhere between
    // being decided and being sent.
    for face in &machine.faces {
        if alo_renderer::generic::is_a_candidate(&face.family) {
            assert!(
                !machine.generics.is_empty(),
                "{:?} is a family a generic wants and no generic means anything",
                face.family,
            );
        }
    }
}

/// A face is named by the font rather than by the file it came out of.
///
/// Queue item 192's second half. `from_file` took the family from the filename
/// until this test, on the argument that a database is a guess about what to
/// look at — but a guess about *what a font is called* is what decides whether
/// a page is drawn as its author wrote it, and the bytes are already in hand by
/// the time the question is asked.
#[test]
fn a_font_is_named_by_itself_and_never_by_its_filename() {
    let place = std::env::temp_dir().join(format!("alo-named-{}", std::process::id()));
    std::fs::create_dir_all(&place).expect("a temporary directory");
    let path = place.join("Definitely-Not-The-Family-Name.ttf");
    std::fs::write(&path, dejavu::sans::regular()).expect("a font written to it");

    let face = fonts::from_file(&path).expect("a real font read back");
    assert_eq!(
        face.family, "DejaVu Sans",
        "the face was named after the file rather than after the font",
    );

    // A file that is not a font is not a face, rather than a face called
    // whatever the file was called.
    let rubbish = place.join("Helvetica.ttf");
    std::fs::write(&rubbish, b"this is not a font").expect("some bytes written");
    assert!(
        fonts::from_file(&rubbish).is_none(),
        "a file with a font's name on it was taken for a font",
    );

    std::fs::remove_dir_all(&place).ok();
}

/// A face is weighed by the font rather than by the file it came out of, and
/// the text it sets is measured accordingly.
///
/// Queue item 194, and the half item 192 left: the family stopped being a guess
/// there, and `bold` and `italic` in a filename were still what decided which
/// face of that family this was. Both files here are named wrongly on purpose,
/// and the numbers at the end are what makes the assertion more than a label —
/// which face a page is given decides how wide its text is and so where every
/// line of it breaks.
#[test]
fn a_face_is_weighed_by_the_font_and_never_by_its_filename() {
    let place = std::env::temp_dir().join(format!("alo-weighed-{}", std::process::id()));
    std::fs::create_dir_all(&place).expect("a temporary directory");

    // Swapped, so that a rule reading the filename gets both of them wrong and
    // a rule reading the font gets both of them right.
    let heavy_file = place.join("Text-Regular.ttf");
    let light_file = place.join("Text-Bold.ttf");
    // And one with no word for what it is in its name at all, which is the
    // ordinary case rather than the awkward one: `Oblique` is what half the
    // world calls a face that leans.
    let leaning_file = place.join("Text-Two.ttf");
    std::fs::write(&heavy_file, dejavu::sans::bold()).expect("a bold font written");
    std::fs::write(&light_file, dejavu::sans::regular()).expect("a regular font written");
    std::fs::write(&leaning_file, dejavu::sans::oblique()).expect("an oblique font written");

    let heavy = fonts::from_file(&heavy_file).expect("the bold font read back");
    let light = fonts::from_file(&light_file).expect("the regular font read back");
    let leaning = fonts::from_file(&leaning_file).expect("the oblique font read back");
    assert_eq!(
        heavy.weight(),
        Weight::BOLD,
        "a bold font in a file called Regular was filed by its file",
    );
    assert_eq!(
        light.weight(),
        Weight::NORMAL,
        "a regular font in a file called Bold was filed by its file",
    );
    assert_eq!(
        leaning.slant,
        Slant::Italic,
        "a face that leans is upright unless its file says the word italic",
    );

    // The numbers. A database of the two faces, asked for the same word at the
    // same size in each weight: what comes back has to be the width of the file
    // the font said was bold, and it has to differ from the other.
    let mut database = alo_text::FontDatabase::new();
    for face in [&heavy, &light] {
        let font =
            alo_text::Font::load(&face.family, face.weight(), face.slant, face.bytes.clone())
                .expect("a face this engine can load");
        database.add(font);
    }
    let asking = |weight| alo_text::FontRequest {
        families: vec!["DejaVu Sans".to_owned()],
        weight,
        slant: Slant::Normal,
    };
    let bold = alo_text::measure_unwrapped("Invoices", &database, &asking(Weight::BOLD), 16.0);
    let normal = alo_text::measure_unwrapped("Invoices", &database, &asking(Weight::NORMAL), 16.0);
    assert!(
        bold.width() > normal.width(),
        "the two files were filed under one weight: {} against {}",
        bold.width(),
        normal.width(),
    );

    // And it is the bold *font* that was chosen rather than merely a different
    // one: the same text through a database holding only those bytes measures
    // the same, to the pixel.
    let mut only_bold = alo_text::FontDatabase::new();
    only_bold.add(
        alo_text::Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::bold().to_vec(),
        )
        .expect("the bold font this test was written with"),
    );
    let known = alo_text::measure_unwrapped("Invoices", &only_bold, &asking(Weight::NORMAL), 16.0);
    assert!(
        (bold.width() - known.width()).abs() < 0.01,
        "a request for bold was answered with something else: {} against {}",
        bold.width(),
        known.width(),
    );

    std::fs::remove_dir_all(&place).ok();
}

/// A machine's own fonts, through the boundary, drawing a page. If the family
/// naming or the byte carrying were wrong anywhere, this is where it would
/// show.
#[test]
fn a_page_is_drawn_with_a_font_that_came_from_this_machine() {
    let machine = fonts::from_this_machine();
    if machine.faces.is_empty() {
        return;
    }
    let family = machine
        .faces
        .first()
        .map(|face| face.family.clone())
        .unwrap_or_default();
    let mut renderers = Renderers::running(RENDERER, &[]).with_machine(machine);
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

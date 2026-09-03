/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Rendering a case, all the way through.
//!
//! Every stage of the engine in one call: parse, cascade, box, lay out, draw.
//! It lives here rather than in each test because three test files had grown
//! their own copy of it, and three copies of the pipeline is three places for
//! the pipeline to be assembled differently.
//!
//! It is **not** the boundary. ADR 0005's boundary is [`crate::renderer`], and
//! what crosses it is a frame and a snapshot — never a display list, which is
//! not a thing a browser process asks for. This is what happens *inside* a
//! renderer, and it produces every intermediate tree because the corpus
//! asserts on them.
//!
//! It is not an embedding surface either: no incremental update, no window,
//! and fonts arrive whole. `docs/features.md` promises alo OS a surface to
//! render into, and that is queue item 40's.

use alo_box::{BoxId, BoxTree};
use alo_css::{ColorScheme, MediaContext, parse_stylesheet};
use alo_dom::Document;
use alo_layout::{LayoutTree, Size};
use alo_paint::{Canvas, DisplayList, PaintContext};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET};
use alo_text::{FontDatabase, TextMeasurer};
use alo_value::Rgba;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Everything one render produced.
pub struct Rendered {
    /// The document.
    pub document: Document,
    /// The style of every element.
    pub styles: StyleTree,
    /// The boxes.
    pub boxes: BoxTree,
    /// Where every box ended up.
    pub layout: LayoutTree,
    /// What was drawn, in order.
    pub display: DisplayList,
    /// The picture.
    pub canvas: Canvas,
    /// Everything the style sheets themselves asked for that was refused.
    ///
    /// Kept separately because the sheets are consumed by the cascade: a
    /// selector this engine cannot evaluate is refused when the sheet is
    /// parsed, long before any element is styled.
    pub sheet_issues: Vec<String>,
    /// The families this page asked for and did not get, and what it was drawn
    /// in instead.
    ///
    /// Kept separately from the trees for the same reason as `sheet_issues`:
    /// it is not a property of any one of them. It needs the boxes, the styles
    /// **and** the fonts, and the fonts are the one thing no tree here holds.
    pub wanted: crate::families::Wanted,
}

impl Rendered {
    /// Everything that would surprise somebody about this render, as one list.
    ///
    /// Gathered from all four stages, because a case that renders oddly is
    /// nearly always a case that was told something it could not do — and
    /// finding that out should not mean asking four objects separately.
    ///
    /// Mostly refusals, and not only: a style sheet that never arrived and a
    /// font family nobody had are both pages rendering *differently* rather
    /// than pages being refused anything. What the list has in common is that
    /// every line in it explains something a person can see.
    pub fn issues(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        out.extend(self.document.issues().iter().map(ToString::to_string));
        out.extend(self.sheet_issues.iter().cloned());
        out.extend(self.styles.issues().iter().map(ToString::to_string));
        out.extend(self.boxes.issues().iter().map(ToString::to_string));
        out.extend(self.layout.issues().iter().map(ToString::to_string));
        // Last, because a substituted font is the only thing in this list that
        // is not something the engine *refused* — the page rendered, in the
        // wrong typeface, and that reads better after the refusals than among
        // them.
        out.extend(self.wanted.substitutions.iter().cloned());
        out
    }
}

/// Render markup and a style sheet at a size.
pub fn render(html: &str, css: &str, size: Size, fonts: &FontDatabase) -> Rendered {
    render_with(html, css, size, fonts, &[])
}

/// The same, with the style sheets a page linked to already fetched.
///
/// `linked` maps an `href` exactly as the page wrote it to the CSS behind it.
/// A page that links to something not in the list is **not** an error — it is a
/// sheet that has not arrived, which is a real state a page can be in — but it
/// is recorded as an issue, because a page styled by a sheet that never came is
/// a page that looks wrong for a reason nobody can see.
pub fn render_with(
    html: &str,
    css: &str,
    size: Size,
    fonts: &FontDatabase,
    linked: &[(String, String)],
) -> Rendered {
    render_document_with(alo_dom::parse_document(html), css, size, fonts, linked, &[])
}

/// The same, with the pictures a page asks for already fetched.
///
/// `resources` maps a `src` exactly as the page wrote it to the bytes behind
/// it. A page whose picture is not in the list renders a box of the size its
/// style asked for and records the fact — which is what a browser shows for a
/// broken image.
pub fn render_with_resources(
    html: &str,
    css: &str,
    size: Size,
    fonts: &FontDatabase,
    linked: &[(String, String)],
    resources: &[(String, Vec<u8>)],
) -> Rendered {
    render_document_with(
        alo_dom::parse_document(html),
        css,
        size,
        fonts,
        linked,
        resources,
    )
}

/// Render a document that already exists.
///
/// **The document is moved in and comes back out**, which is the whole point:
/// a page that was changed — a field typed into, a box checked — is rendered
/// again from the same nodes, so every id an agent read a moment ago still
/// names what it named (ADR 0003). Re-parsing would mint new ones and quietly
/// break every snapshot anybody was holding.
pub fn render_document(
    document: Document,
    css: &str,
    size: Size,
    fonts: &FontDatabase,
) -> Rendered {
    render_document_with(document, css, size, fonts, &[], &[])
}

/// The same, with the style sheets a page linked to already fetched.
///
/// # Errors
///
/// None: a linked sheet that is not in `linked` is recorded as an issue rather
/// than refused, because a page whose style has not arrived is still a page.
pub fn render_document_with(
    document: Document,
    css: &str,
    size: Size,
    fonts: &FontDatabase,
    linked: &[(String, String)],
    resources: &[(String, Vec<u8>)],
) -> Rendered {
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    // A page's own `<style>` elements, then whatever the caller supplied. In
    // that order because a later sheet overrides an earlier one, and a caller
    // handing a sheet in is saying something *about* the page — a corpus case's
    // expectations, or a user sheet — which has to be able to win.
    //
    // This was missing until the first page taken off the web arrived carrying
    // its whole style sheet inside itself, which is what pages do and which the
    // corpus never showed because the corpus was ours.
    let mut missing = Vec::new();
    let mut parsed: Vec<_> = alo_dom::sheets::asked_for(&document)
        .into_iter()
        .map(|sheet| match sheet {
            alo_dom::sheets::Sheet::Written(text) => parse_stylesheet(&text),
            alo_dom::sheets::Sheet::Linked { href } => {
                if let Some((_, text)) = linked.iter().find(|(at, _)| *at == href) {
                    parse_stylesheet(text)
                } else {
                    // Not an error: a sheet that has not arrived is a real
                    // state a page can be in. Recorded, because a page styled
                    // by a sheet that never came looks wrong for a reason
                    // nobody can see from the page itself.
                    missing.push(format!("no style sheet was loaded for {href:?}"));
                    parse_stylesheet("")
                }
            }
        })
        .collect();
    parsed.push(parse_stylesheet(css));
    let mut sheets = vec![SourcedSheet::new(Origin::UserAgent, &agent)];
    sheets.extend(
        parsed
            .iter()
            .map(|sheet| SourcedSheet::new(Origin::Author, sheet)),
    );
    // Both dimensions, because `vh` is as real as `vw` and the window is the
    // one thing here that actually knows them.
    let device = MediaContext::sized(size.width, size.height, ColorScheme::Light);
    let mut sheet_issues: Vec<String> = agent
        .issues()
        .iter()
        .chain(parsed.iter().flat_map(alo_css::Stylesheet::issues))
        .map(ToString::to_string)
        .collect();
    sheet_issues.extend(missing);
    let styles = alo_style::resolve(&document, &sheets, &device);
    let mut boxes = alo_box::build(&document, &styles);

    // Pictures, before layout, because a picture's own size is what an `<img>`
    // with no width lays out at — so the size has to be known before anything
    // is measured.
    let (pictures, picture_issues) = pictures_for(&document, &mut boxes, resources);
    sheet_issues.extend(picture_issues);

    let measurer = TextMeasurer::new(fonts);
    let layout = alo_layout::compute(&boxes, &styles, size, &measurer);
    let display = alo_paint::build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts,
            pictures: &pictures,
        },
    );

    // White, because a page with no background of its own is a white page and
    // a transparent picture is harder to look at in a diff.
    let mut canvas = Canvas::new(whole(size.width), whole(size.height), Rgba::WHITE);
    alo_paint::render(&display, &mut canvas);

    // After the boxes exist, because it is the text that was actually built
    // that decides which families a page really asked for.
    let wanted = crate::families::wanted(&boxes, &styles, fonts);

    Rendered {
        document,
        styles,
        boxes,
        layout,
        display,
        canvas,
        sheet_issues,
        wanted,
    }
}

/// Decode every picture a page asks for, and tell the boxes how big they are.
///
/// # Why this is here rather than in `alo-box` or `alo-paint`
///
/// It needs three things that live in three places: the document, to find an
/// `<img>` and its `src`; the frozen bytes, which the caller has; and the
/// decoder, which is `alo_paint::encode`. This is the only place that has all
/// three, and putting it in any of them would mean that crate learning about
/// the other two.
///
/// A picture that could not be decoded is **recorded and skipped**. The box
/// keeps whatever size its style asked for, which is what a browser shows for a
/// broken image — an empty box of the right shape rather than a collapsed page.
fn pictures_for(
    document: &Document,
    boxes: &mut alo_box::BoxTree,
    resources: &[(String, Vec<u8>)],
) -> (BTreeMap<BoxId, Arc<Canvas>>, Vec<String>) {
    let mut pictures = BTreeMap::new();
    let mut issues = Vec::new();
    // Every box rather than a walk: this is looking for a kind of box rather
    // than following the tree's shape.
    let ids: Vec<BoxId> = boxes.ids().collect();
    for id in ids {
        let Some(node) = boxes.get(id) else {
            continue;
        };
        let alo_box::BoxKind::Element { node: element, .. } = node.kind else {
            continue;
        };
        let Some(element) = document.element(element) else {
            continue;
        };
        if !element.name.local.eq_ignore_ascii_case("img") {
            continue;
        }
        let Some(src) = element
            .attrs
            .iter()
            .find(|attribute| attribute.name.local.eq_ignore_ascii_case("src"))
            .map(|attribute| attribute.value.trim().to_owned())
            .filter(|src| !src.is_empty())
        else {
            issues.push("an <img> with no src".to_owned());
            continue;
        };
        let Some((_, bytes)) = resources.iter().find(|(at, _)| *at == src) else {
            issues.push(format!("no picture was loaded for {src:?}"));
            continue;
        };
        // By what the bytes are rather than what the `src` ends in: a name on a
        // page proves nothing about what a server sent.
        match alo_paint::picture::read(bytes) {
            Ok(canvas) => {
                let (width, height) = (canvas.width(), canvas.height());
                boxes.set_natural_size(
                    id,
                    (
                        f32::from(u16::try_from(width).unwrap_or(u16::MAX)),
                        f32::from(u16::try_from(height).unwrap_or(u16::MAX)),
                    ),
                );
                pictures.insert(id, Arc::new(canvas));
            }
            Err(why) => issues.push(format!("{src:?} is not a picture this engine reads: {why}")),
        }
    }
    (pictures, issues)
}

/// A size in whole pixels, without a float-to-integer cast.
fn whole(value: f32) -> u32 {
    let clamped = value.round().clamp(0.0, 4096.0);
    let mut whole = 0u32;
    while f32::from(u16::try_from(whole).unwrap_or(u16::MAX)) + 1.0 <= clamped {
        whole += 1;
    }
    whole
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_fonts() -> FontDatabase {
        FontDatabase::new()
    }

    /// A database that can answer what the user-agent sheet asks for.
    ///
    /// The sheet sets `font-family: system-ui, sans-serif` on every document,
    /// so a test rendering with no fonts at all is a test of a page drawn in
    /// nothing — which is a real state and is now reported, and is not what
    /// most of these tests are about.
    fn one_font() -> FontDatabase {
        let mut fonts = FontDatabase::new();
        if let Some(font) = alo_text::Font::load(
            "DejaVu Sans",
            alo_text::Weight::NORMAL,
            alo_text::Slant::Normal,
            dejavu::sans::regular().to_vec(),
        ) {
            fonts.add(font);
        }
        fonts.map_generic("system-ui", "DejaVu Sans");
        fonts
    }

    #[test]
    fn a_size_becomes_whole_pixels() {
        assert_eq!(whole(0.0), 0);
        assert_eq!(whole(1.4), 1);
        assert_eq!(whole(1.6), 2);
        assert_eq!(whole(-5.0), 0);
        assert_eq!(whole(1.0e9), 4096);
    }

    #[test]
    fn a_page_renders_all_the_way_through() {
        let rendered = render(
            "<!DOCTYPE html><html><body><p>hello</p></body></html>",
            "p { background: #ff0000 }",
            Size::new(20.0, 10.0),
            &no_fonts(),
        );
        assert!(rendered.boxes.root().is_some());
        assert!(!rendered.layout.is_empty());
        assert!(!rendered.display.is_empty());
        assert_eq!(
            (rendered.canvas.width(), rendered.canvas.height()),
            (20, 10)
        );
    }

    #[test]
    fn an_ordinary_page_asks_for_nothing_the_engine_refuses() {
        let rendered = render(
            "<!DOCTYPE html><html><body><div><p>text</p></div></body></html>",
            "div { padding: 4px } p { margin: 0; color: #000000 }",
            Size::new(40.0, 20.0),
            &one_font(),
        );
        assert!(rendered.issues().is_empty(), "{:?}", rendered.issues());
    }

    #[test]
    fn a_page_drawn_in_a_font_it_never_asked_for_says_so() {
        // The user-agent sheet asks every document for `system-ui, sans-serif`.
        // A renderer holding neither draws the text in something else, and the
        // whole of queue item 170 is that this is said out loud rather than
        // being a stable, diffable render nobody can explain.
        let rendered = render(
            "<!DOCTYPE html><html><body><p>text</p></body></html>",
            "",
            Size::new(40.0, 20.0),
            &no_fonts(),
        );
        assert_eq!(
            rendered.wanted.families,
            vec!["system-ui".to_owned(), "sans-serif".to_owned()],
        );
        assert!(
            rendered
                .issues()
                .iter()
                .any(|issue| issue.contains("system-ui")),
            "{:?}",
            rendered.issues(),
        );
    }

    #[test]
    fn everything_refused_is_gathered_from_every_stage() {
        // Two boxes rather than one, because a box whose `display` fell back
        // to `inline` joins the line around it and layout never reads its
        // width — so one box could not raise both refusals.
        let rendered = render(
            "<!DOCTYPE html><html><body><div id=a>text</div><div id=b>text</div></body></html>",
            "div:has(b) { color: red } #a { display: table } #b { width: 50dvh }",
            Size::new(40.0, 20.0),
            &no_fonts(),
        );
        let issues = rendered.issues();
        assert!(
            issues.iter().any(|issue| issue.contains(":has")),
            "the sheet's own refusal: {issues:?}",
        );
        assert!(
            issues.iter().any(|issue| issue.contains("display: table")),
            "the box tree's: {issues:?}",
        );
        assert!(
            issues.iter().any(|issue| issue.contains("50dvh")),
            "and layout's: {issues:?}",
        );
    }
}

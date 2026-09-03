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

use alo_box::BoxTree;
use alo_css::{ColorScheme, MediaContext, parse_stylesheet};
use alo_dom::Document;
use alo_layout::{LayoutTree, Size};
use alo_paint::{Canvas, DisplayList, PaintContext};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET};
use alo_text::{FontDatabase, TextMeasurer};
use alo_value::Rgba;

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
}

impl Rendered {
    /// Everything the engine refused along the way, as one list.
    ///
    /// Gathered from all four stages, because a case that renders oddly is
    /// nearly always a case that was told something it could not do — and
    /// finding that out should not mean asking four objects separately.
    pub fn issues(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        out.extend(self.document.issues().iter().map(ToString::to_string));
        out.extend(self.sheet_issues.iter().cloned());
        out.extend(self.styles.issues().iter().map(ToString::to_string));
        out.extend(self.boxes.issues().iter().map(ToString::to_string));
        out.extend(self.layout.issues().iter().map(ToString::to_string));
        out
    }
}

/// Render markup and a style sheet at a size.
pub fn render(html: &str, css: &str, size: Size, fonts: &FontDatabase) -> Rendered {
    render_document(alo_dom::parse_document(html), css, size, fonts)
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
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let device = MediaContext::new(size.width, ColorScheme::Light);
    let sheet_issues: Vec<String> = agent
        .issues()
        .iter()
        .chain(author.issues())
        .map(ToString::to_string)
        .collect();
    let styles = alo_style::resolve(&document, &sheets, &device);
    let boxes = alo_box::build(&document, &styles);

    let measurer = TextMeasurer::new(fonts);
    let layout = alo_layout::compute(&boxes, &styles, size, &measurer);
    let display = alo_paint::build::build(&boxes, &layout, &styles, PaintContext { fonts });

    // White, because a page with no background of its own is a white page and
    // a transparent picture is harder to look at in a diff.
    let mut canvas = Canvas::new(whole(size.width), whole(size.height), Rgba::WHITE);
    alo_paint::render(&display, &mut canvas);

    Rendered {
        document,
        styles,
        boxes,
        layout,
        display,
        canvas,
        sheet_issues,
    }
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
            &no_fonts(),
        );
        assert!(rendered.issues().is_empty(), "{:?}", rendered.issues());
    }

    #[test]
    fn everything_refused_is_gathered_from_every_stage() {
        // Two boxes rather than one, because a box whose `display` fell back
        // to `inline` joins the line around it and layout never reads its
        // width — so one box could not raise both refusals.
        let rendered = render(
            "<!DOCTYPE html><html><body><div id=a>text</div><div id=b>text</div></body></html>",
            "div:has(b) { color: red } #a { display: table } #b { width: 50vw }",
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
            issues.iter().any(|issue| issue.contains("50vw")),
            "and layout's: {issues:?}",
        );
    }
}

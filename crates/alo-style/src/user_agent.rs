//! The engine's own style sheet: what an element looks like before anybody
//! says otherwise.
//!
//! Every browser has one, and most of a browser's is archaeology — the shapes
//! of `<xmp>`, `<marquee>` and forty years of forms. This one is not. It
//! covers the elements a modern interface is written with and nothing else,
//! which is law 1 applied to the one file where it is easiest to forget: a
//! default for an element we refuse to lay out is a default nobody will ever
//! read, and it is one more thing to keep right.
//!
//! **No tables.** `docs/features.md` puts CSS-table layout in stage 3, so
//! `<table>` has no defaults here rather than defaults that do not work.
//!
//! **No `::before` or `::after`.** Stage 1 produces no pseudo-elements, so
//! there is nothing to give content to — including list markers, which is why
//! `list-item` appears without one.

/// The engine's style sheet, as CSS text.
///
/// It is text rather than a built structure so that it goes through exactly
/// the same parser, cascade and refusals as anything an author writes. A
/// user-agent sheet that took a private path would be a second implementation
/// of the cascade, and the second one is the one that is wrong.
pub const USER_AGENT_STYLE_SHEET: &str = r"
/* What is in the document but never drawn. */
head, meta, link, title, script, style, base, template, source, track, param {
  display: none;
}

/* The document, and the blocks a page is built from. */
html, body, div, p, section, article, aside, header, footer, main, nav,
figure, figcaption, blockquote, address, hgroup, search, dialog, details,
summary, fieldset, legend, form, hr, pre {
  display: block;
}

h1, h2, h3, h4, h5, h6 { display: block; font-weight: bold }

ul, ol, menu { display: block; list-style-type: disc }
ol { list-style-type: decimal }
li { display: list-item }
dl { display: block }
dt { display: block }
dd { display: block }

/* Things that sit in a line of text. */
span, a, em, strong, b, i, u, s, small, mark, code, kbd, samp, var, sub, sup,
abbr, cite, q, time, data, bdi, bdo, ruby, del, ins, br, wbr, picture, slot {
  display: inline;
}

img, svg, video, audio, canvas, iframe, object, embed, math, progress, meter {
  display: inline-block;
}

button, input, select, textarea, output, label, optgroup, option, datalist,
  legend, fieldset, textarea {
  display: inline-block;
}
label { display: inline }
datalist, optgroup, option { display: none }

/* Text defaults. A person reading this is reading in a direction. */
html { color: black; direction: ltr; font-family: system-ui, sans-serif }
b, strong, th { font-weight: bold }
i, em, cite, var, address, dfn { font-style: italic }
code, kbd, samp, pre { font-family: monospace }
pre { white-space: pre }
a { text-decoration: underline }
small { font-size: smaller }
mark { background: yellow; color: black }

/* An element the author hid is hidden, whatever else the sheet says. */
[hidden] { display: none }
";

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::parse_stylesheet;

    #[test]
    fn the_engines_own_sheet_parses_with_nothing_refused() {
        let sheet = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        assert!(
            sheet.issues().is_empty(),
            "the engine's own sheet must not contain anything the engine refuses: {:?}",
            sheet
                .issues()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        );
        assert!(!sheet.rules().is_empty());
    }

    #[test]
    fn it_says_nothing_about_the_legacy_this_engine_refuses() {
        // `docs/features.md` puts CSS-table layout in stage 3. A default for a
        // thing we do not lay out is a default nobody reads.
        for legacy in ["table", "marquee", "xmp", "frameset"] {
            assert!(
                !USER_AGENT_STYLE_SHEET.contains(legacy),
                "{legacy} should not have defaults here",
            );
        }
    }
}

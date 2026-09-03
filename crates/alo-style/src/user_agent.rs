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
pub const USER_AGENT_STYLE_SHEET: &str = r#"
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

/* What a page looks like before anybody styles it.
 *
 * Until the first page we had not written arrived (queue item 68), this sheet
 * said what elements *are* and nothing about what they look like — no margins,
 * no heading sizes, no list indent. Every case in the corpus set its own
 * spacing, because every case was ours, so a heading sat directly against the
 * paragraph under it and nothing looked wrong.
 *
 * These are the HTML specification's own rendering defaults. They are not
 * decoration: a page that says nothing about spacing is relying on them, which
 * is most pages, and getting them wrong is a page that is subtly wrong
 * everywhere rather than obviously wrong somewhere.
 */
body { margin: 8px }

h1 { font-size: 2em;    margin: 0.67em 0 }
h2 { font-size: 1.5em;  margin: 0.83em 0 }
h3 { font-size: 1.17em; margin: 1em 0 }
h4 { font-size: 1em;    margin: 1.33em 0 }
h5 { font-size: 0.83em; margin: 1.67em 0 }
h6 { font-size: 0.67em; margin: 2.33em 0 }

p { margin: 1em 0 }
pre { margin: 1em 0 }
hr { margin: 0.5em 0 }

/* Forty pixels, and it is forty rather than an em on purpose: it is what every
 * browser uses, it does not shrink with the text, and a list that indented by
 * its own font size would step raggedly when a nested list had a different one.
 */
blockquote, figure { margin: 1em 40px }
ul, ol, menu { margin: 1em 0; padding-left: 40px }
dl { margin: 1em 0 }
dd { margin-left: 40px }

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

/* The controls, which sit in a line and have a size of their own.
 *
 * `fieldset`, `legend` and a second `textarea` used to be in this list as well
 * as in the block list above — and a duplicate in one sheet is the later rule
 * winning, so a `<fieldset>` laid out **inline**. Nothing noticed until a page
 * with a form arrived, because no alo screen uses one: the corpus had every
 * control and not one fieldset.
 */
button, input, select, textarea, output, optgroup, option, datalist {
  display: inline-block;
}
label { display: inline }
datalist, optgroup, option { display: none }

/* Form controls have a size of their own.
 *
 * A bare `<input>` is not empty — it is a box a person types into, and it is
 * about twenty characters wide before anybody says otherwise. Without this
 * every control on a page lays out at nothing by nothing, which is invisible
 * on screen and, worse, gives an agent a target it cannot point at. `ch` is
 * the width of the font's zero, which is what "twenty characters" means.
 */
input, select {
  width: 20ch;
  /* No height. A field with nothing typed into it is still one line tall,
     and that comes from the box the control holds its text in
     (`alo_box::Purpose::Control`) rather than from a rule here. A rule would
     have been a *fixed* height, so a field with something in it would be too
     short for it — which is exactly what happened. */
  padding: 1px 2px;
  border: 1px solid #767676;
}
textarea {
  width: 20ch;
  height: 3em;
  padding: 2px;
  border: 1px solid #767676;
}
/* What is written on a button sits in the middle of it, across and down —
   and that is *not* here, because it cannot be. A rule that centred a
   button's label would also centre the children of a button an author had
   made a flex container, and an author cannot override a rule they cannot
   see. Browsers hold a control's content in an internal box instead, and so
   does this engine: `alo_box::Purpose::Control`. */
button {
  padding: 1px 6px;
  border: 1px solid #767676;
  background: #efefef;
  text-align: center;
}
input[type="checkbox"], input[type="radio"] {
  width: 13px;
  height: 13px;
  padding: 0;
}
/* A radio is round and a checkbox is square, and that is not decoration.
 *
 * The agent tree told them apart from the first commit — `radio "Small"` and
 * `checkbox "Bacon"` — while a person looking at the page could not, because
 * both drew as a bordered square. Found by the first page with a form: the alo
 * screens have checkboxes and no radios, so nothing had ever put the two side
 * by side.
 *
 * A radio group and a checkbox group mean different things: one answer or
 * several. Somebody who cannot see which they are looking at is being asked a
 * question without being told its shape. */
input[type="radio"] {
  border-radius: 50%;
}
input[type="button"], input[type="submit"], input[type="reset"] {
  width: auto;
  padding: 1px 6px;
  background: #efefef;
}

/* Text defaults. A person reading this is reading in a direction. */
html { color: black; direction: ltr; font-family: system-ui, sans-serif }
b, strong, th { font-weight: bold }
i, em, cite, var, address, dfn { font-style: italic }
code, kbd, samp, pre { font-family: monospace }
pre { white-space: pre }
/* A link looks like a link.
 *
 * Found by the first web page (queue item 172), which is almost nothing but
 * links and which rendered as an undifferentiated wall of black text. The
 * sheet had `text-decoration: underline` and no colour at all, so on a page
 * with no style of its own there was no way to see what could be followed.
 *
 * `:any-link` rather than `a`, because an `<a>` without an `href` is not a
 * link — it is an anchor, and 1991 pages are full of them.
 *
 * **There is no visited colour, and that is deliberate.** `:visited` never
 * matches in this engine (see `alo_css::PseudoClass::Visited`): whether a link
 * has been visited is history, and a style that depends on it is readable from
 * the page. So there are no purple links here, and that is a privacy decision
 * with a visible cost rather than an oversight — which is why it is written
 * where somebody would otherwise add the rule.
 */
a:any-link { color: #0000ee; text-decoration: underline }
small { font-size: smaller }
mark { background: yellow; color: black }

/* An element the author hid is hidden, whatever else the sheet says. */
[hidden] { display: none }
"#;

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

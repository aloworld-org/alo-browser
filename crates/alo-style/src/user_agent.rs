/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

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
//! **No `::before` or `::after`.** The pseudo-elements this engine produces
//! are a field's `::placeholder` (ADR 0043) and a list item's `::marker`
//! (ADR 0044), so there is nothing to give content to. A marker's text comes
//! from `list-style-type` and the item's number, not from `content`.

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
/* A list inside a list is part of it, not a paragraph after it, so it has no
 * space above or below: HTML's rendering section says so, and every browser
 * does it. Found by alo Sites' rich text (queue item 400), whose nested point
 * stood a line's height away from the point it belongs to. */
:is(dl, menu, ol, ul) :is(dl, menu, ol, ul) { margin-block: 0 }

ul, ol, menu { display: block; list-style-type: disc }
ol { list-style-type: decimal }
/* A bulleted list inside a list is drawn with hollow circles, and one inside
 * that with squares: HTML's rendering section, whose inner lists are `menu`
 * and `ul` only, so a numbered list keeps its numbers however deep it is
 * (ADR 0044 § 2; obsolete `dir` left out). */
:is(menu, ol, ul) :is(menu, ul) { list-style-type: circle }
:is(menu, ol, ul) :is(menu, ol, ul) :is(menu, ul) { list-style-type: square }
li { display: list-item }
/* A marker is not put in its item's case: an `uppercase` list keeps its `a.`
 * (CSS Lists 3's user-agent sheet, and Chrome 154's, measured; ADR 0049
 * § 6). Written when `text-transform` was first read, in the change that
 * read it. */
::marker { text-transform: none }
dl { display: block }
dt { display: block }
dd { display: block }

/* Things that sit in a line of text. */
span, a, em, strong, b, i, u, s, small, mark, code, kbd, samp, var, sub, sup,
abbr, cite, q, time, data, bdi, bdo, ruby, del, ins, br, wbr, picture, slot {
  display: inline;
}

/* An `img` is `inline`, as every engine computes it: a picture is atomic
 * whatever its `display` says, and a broken one's `alt` is then a run of text
 * that wraps with its line and takes no `width` (ADR 0046 § 2). */
svg, video, audio, canvas, iframe, object, embed, math, progress, meter {
  display: inline-block;
}

/* An <svg>'s drawing stops at its viewport, as SVG's own sheet says: a shape
 * that runs past the edge is cut there unless an author asks otherwise
 * (ADR 0022 § 2). */
svg { overflow: hidden }

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
/* A checkbox and a radio are 13 by 13 *with* their border, and stand a few
 * pixels off the text beside them: Chrome 154's computed values, measured
 * (queue item 443). The margins are not symmetrical, and not the same for
 * the two: a radio is a pixel further from the text before it and sits on
 * the baseline with nothing below it. Without them alo Settings' checkbox
 * stood 4 pixels left of Chrome's and its label 7.
 *
 * And no border. Chrome computes none for either, and draws the control
 * itself instead: a rounded square or a circle, white inside a grey edge,
 * filled with the accent colour when it is checked (queue item 454). That
 * picture is `alo_paint::toggle`'s, for the reason a tick is not a property.
 * Which shape a control is — one answer or several — is drawn there, by the
 * element's type, and not by a `border-radius` here that an author could
 * take away without meaning to. */
input[type="checkbox"], input[type="radio"] {
  box-sizing: border-box;
  width: 13px;
  height: 13px;
  padding: 0;
  border: none;
}
input[type="checkbox"] { margin: 3px 3px 3px 4px }
input[type="radio"] { margin: 3px 3px 0 5px }
input[type="button"], input[type="submit"], input[type="reset"] {
  width: auto;
  padding: 1px 6px;
  background: #efefef;
}

/* A group of controls looks like a group.
 *
 * A `<fieldset>` had no border at all until now, which made the one thing it
 * is *for* invisible: three radio buttons under "Pizza Size" looked exactly
 * like three radio buttons. The border and the padding here are the HTML
 * specification's own, and the legend is what makes them interesting — it sits
 * **in** the border rather than above it, which is `alo_layout::legend`.
 *
 * A `groove`, as every other browser draws it: two tones of one colour, cut
 * into the page, with the legend's part of the line left out —
 * `alo_paint::banded`.
 *
 * The legend's width is `fit-content(100%)` rather than the bare `fit-content`
 * the specification writes, which is the same thing for a block in flow — as
 * wide as its words, and never wider than the fieldset. It matters more than
 * it looks: a legend stretched to the full width would leave a gap in the
 * border with no border either side of it, which is to say no border at all.
 */
fieldset {
  margin-left: 2px;
  margin-right: 2px;
  padding: 0.35em 0.75em 0.625em;
  border: 2px groove #c0c0c0;
}
legend {
  padding-left: 2px;
  padding-right: 2px;
  width: fit-content(100%);
}

/* A control nobody can operate says so.
 *
 * "You cannot change this" and "this is off" are different things to be told,
 * and a disabled unchecked checkbox drawn exactly like a live one tells
 * somebody the second when the first is true. The state a control is *in* is
 * drawn by the control itself — see `alo_paint::control` — because a tick is
 * not a property; whether it is **live** is ordinary colour, so it is here,
 * where a page can override it.
 */
button:disabled, input:disabled, select:disabled, textarea:disabled,
fieldset:disabled {
  border-color: #c0c0c0;
  color: #6d6d6d;
}
button:disabled, select:disabled, textarea:disabled,
input[type="button"]:disabled, input[type="submit"]:disabled,
input[type="reset"]:disabled {
  background-color: #f5f5f5;
}

/* A field's hint is grey, and a page that says otherwise wins.
 *
 * Chromium's colour, a fixed #757575: 4.6:1 against a white field. Firefox
 * mixes the field's own `currentColor` at 54%, which suits a dark field
 * better and needs `color-mix()`, which this engine does not read (item
 * 378); it is not taken by halves (ADR 0043 § 3). The hint inherits the rest
 * — its font, its line — from its field, so it stands where the field's text
 * will. */
::placeholder { color: #757575 }

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
/* An edit looks like one. `<s>` and `<del>` are struck through and `<u>` and
 * `<ins>` underlined, as HTML's rendering section says: a struck-out task in a
 * checklist drawn as plain text reads as a task still to do. Found by alo
 * Sites' rich text (queue item 400), whose finished task is `<s>Publish</s>`.
 * `<strike>` says the same and is left out, because it is obsolete markup. */
s, del { text-decoration: line-through }
u, ins { text-decoration: underline }
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

    /// What the engine's sheet alone gives the element whose `id` is
    /// `wanted`, for the property `name`.
    fn computed(html: &str, wanted: &str, name: &str) -> Option<String> {
        let document = alo_dom::parse_document(html);
        let sheet = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let sheets = [crate::SourcedSheet::new(crate::Origin::UserAgent, &sheet)];
        let styles = crate::resolve(&document, &sheets, &alo_css::MediaContext::default());
        let id = document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))?;
        Some(styles.get(id)?.get(name)?.to_owned())
    }

    #[test]
    fn a_list_in_a_list_has_no_space_above_or_below_it() {
        let html = "<ul id=outer><li>a<ul id=inner><li>b</ul></ul>\
                    <ol><li><dl id=terms><dt>c</dl></ol>\
                    <dl id=alone><dt>d</dl><menu><li><ol id=numbered><li>e</ol></menu>";
        for (id, margin) in [
            ("outer", "1em"),
            ("inner", "0"),
            ("terms", "0"),
            ("alone", "1em"),
            ("numbered", "0"),
        ] {
            for side in ["margin-top", "margin-bottom"] {
                assert_eq!(
                    computed(html, id, side).as_deref(),
                    Some(margin),
                    "{id} {side}"
                );
            }
        }
        // Only the block axis: a nested list is still indented.
        assert_eq!(
            computed(html, "inner", "padding-left").as_deref(),
            Some("40px")
        );
    }

    #[test]
    fn a_bulleted_list_in_a_list_is_circled_and_one_deeper_squared() {
        let html = "<ul id=one><li><ul id=two><li><ul id=three><li><menu id=four></menu>\
                    </ul></ul></ul><ol id=n1><li><ol id=n2><li><ul id=b3></ul></ol></ol>";
        for (id, kind) in [
            ("one", "disc"),
            ("two", "circle"),
            ("three", "square"),
            ("four", "square"),
            ("n1", "decimal"),
            ("n2", "decimal"),
            ("b3", "square"),
        ] {
            assert_eq!(
                computed(html, id, "list-style-type").as_deref(),
                Some(kind),
                "{id}"
            );
        }
    }

    #[test]
    fn a_checkbox_and_a_radio_have_chromes_margins_size_and_no_border() {
        let html = "<input id=check type=checkbox><input id=radio type=radio><input id=text>";
        for (id, top, right, bottom, left) in [
            ("check", "3px", "3px", "3px", "4px"),
            ("radio", "3px", "3px", "0", "5px"),
        ] {
            for (side, value) in [
                ("margin-top", top),
                ("margin-right", right),
                ("margin-bottom", bottom),
                ("margin-left", left),
            ] {
                assert_eq!(
                    computed(html, id, side).as_deref(),
                    Some(value),
                    "{id} {side}"
                );
            }
            assert_eq!(
                computed(html, id, "box-sizing").as_deref(),
                Some("border-box"),
                "{id}"
            );
            // Chrome computes no border and no rounding: the control draws
            // its own edge (queue item 454).
            assert_eq!(
                computed(html, id, "border").as_deref(),
                Some("none"),
                "{id}"
            );
            assert_eq!(computed(html, id, "border-radius"), None, "{id}");
        }
        // A field keeps no margin of its own.
        assert_eq!(computed(html, "text", "margin-left"), None);
    }

    #[test]
    fn an_edit_is_struck_through_or_underlined() {
        let html = "<p><s id=s>a</s><del id=del>b</del><u id=u>c</u><ins id=ins>d</ins>\
                    <span id=span>e</span>";
        for (id, line) in [
            ("s", Some("line-through")),
            ("del", Some("line-through")),
            ("u", Some("underline")),
            ("ins", Some("underline")),
            ("span", None),
        ] {
            assert_eq!(
                computed(html, id, "text-decoration").as_deref(),
                line,
                "{id}"
            );
        }
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

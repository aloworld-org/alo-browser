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

/* A control is set in a font of its own, not the page's: Chrome 154's
 * computed values, measured (queue item 506). Every `<input>`, `<button>`,
 * `<select>` and `<textarea>` is 13.3333px, weight 400, upright, with a
 * `normal` line, whatever the paragraph around it says — so a field in
 * bold 20 px text with a line of 3 is none of those. The family is Arial,
 * and `monospace` for a `<textarea>` and the five fields that write a date
 * or a time. A machine without Arial gets its own `sans-serif`, which is
 * what the corpus's `DejaVu Sans` answers. A page that wants its own font
 * on a control says so, and alo's screens do: `font: inherit`, which is
 * every one of these longhands (`alo_css::font`). */
button, input, select, textarea {
  font-family: Arial, sans-serif;
  font-size: 13.333333px;
  font-weight: normal;
  font-style: normal;
  line-height: normal;
}
textarea, input[type="date"], input[type="time"], input[type="datetime-local"],
input[type="month"], input[type="week"] {
  font-family: monospace;
}

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
}
/* A field that writes a date or a time has a pixel before it and nothing
 * above, below or after: Chrome 154's computed `0px 0px 0px 1px`, measured
 * (queue item 507). What it holds is held in segments a pixel inside its
 * line above and below (`alo_box::Purpose::Control`'s `in_segments`), so
 * with a text field's padding it would stand 2 px taller than Chrome's. */
input[type="date"], input[type="time"], input[type="datetime-local"],
input[type="month"], input[type="week"] {
  padding: 0 0 0 1px;
}
/* A field's border and a button's are two pixels, `inset` and `outset`:
 * Chrome 154's computed values, measured (queue item 505). Chrome does not
 * draw them while the page leaves them alone. It draws its own picture of
 * the control in the room they take, a one-pixel rounded edge over the
 * fill, and that picture is `alo_paint::face`'s, for the reason a tick is
 * not a property. A page that writes any border or background of its own
 * gets these drawn as written instead, two tones and all. A `<select>`
 * and a `<textarea>` keep their one solid pixel, which is Chrome's computed
 * border too. A `<textarea>` draws a field's picture in it (queue item
 * 510); a `<select>` draws its pixel, as yet (queue item 521). */
input { border: 2px inset #767676 }
select { border: 1px solid #767676 }
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
  border: 2px outset black;
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
  border: 2px outset black;
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
 *
 * The colours are Chrome 154's computed ones, measured for every kind of
 * control (queue item 509). A field's text is a solid dark grey and a
 * button's is near-black at three tenths; both are tinted and edged in the
 * same translucent greys. A list is greyed and then faded as a whole. A
 * checkbox and a radio take only the grey, which their pictures do not use
 * (`alo_paint::toggle` draws them disabled on its own). A `<fieldset>`
 * looks the same disabled as live: it is what it holds that is disabled. A
 * button or a field that draws its own picture (`alo_paint::face`) shows
 * only the text colour from here, and the rest once a page restyles it.
 */
input:disabled, textarea:disabled { color: rgb(84, 84, 84) }
button:disabled, input[type="button"]:disabled, input[type="submit"]:disabled,
input[type="reset"]:disabled, input[type="color"]:disabled {
  color: rgba(16, 16, 16, 0.3);
}
input[type="range"]:disabled { color: rgb(197, 197, 197) }
button:disabled, select:disabled, textarea:disabled,
input:disabled:not([type="checkbox"]):not([type="radio"]) {
  border-color: rgba(118, 118, 118, 0.3);
}
button:disabled, textarea:disabled,
input:disabled:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="file"]):not([type="image"]):not([type="hidden"]) {
  background-color: rgba(239, 239, 239, 0.3);
}
select:disabled, option:disabled, optgroup:disabled { color: rgb(128, 128, 128) }
select:disabled { opacity: 0.7 }

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
    fn a_field_and_a_button_have_chromes_two_pixel_borders() {
        // Chrome 154's computed values, measured (queue item 505): what it
        // draws in their place is `alo_paint::face`'s.
        let html = "<input id=text><input id=date type=date><button id=button>b</button>\
                    <input id=submit type=submit><input id=reset type=reset>\
                    <input id=push type=button><select id=list></select>";
        for (id, border) in [
            ("text", "2px inset #767676"),
            ("date", "2px inset #767676"),
            ("button", "2px outset black"),
            ("submit", "2px outset black"),
            ("reset", "2px outset black"),
            ("push", "2px outset black"),
            ("list", "1px solid #767676"),
        ] {
            assert_eq!(
                computed(html, id, "border").as_deref(),
                Some(border),
                "{id}"
            );
        }
    }

    #[test]
    fn a_disabled_control_has_chromes_computed_colours() {
        // Chrome 154's computed values, measured for each kind of control
        // (queue item 509). `None` is a property the sheet leaves alone.
        let html = "<button id=button disabled>b</button><input id=text disabled>\
                    <input id=submit type=submit disabled><input id=push type=button disabled>\
                    <input id=reset type=reset disabled><input id=colour type=color disabled>\
                    <input id=date type=date disabled><input id=check type=checkbox disabled>\
                    <input id=radio type=radio disabled><input id=range type=range disabled>\
                    <input id=file type=file disabled><input id=hidden type=hidden disabled>\
                    <textarea id=area disabled></textarea>\
                    <select id=list disabled><option id=choice disabled>o</select>\
                    <fieldset id=group disabled><input id=inside></fieldset><input id=live>";
        let edge = Some("rgba(118, 118, 118, 0.3)");
        let tint = Some("rgba(239, 239, 239, 0.3)");
        let faint = Some("rgba(16, 16, 16, 0.3)");
        let grey = Some("rgb(84, 84, 84)");
        for (id, color, background, border) in [
            ("button", faint, tint, edge),
            ("text", grey, tint, edge),
            ("submit", faint, tint, edge),
            ("push", faint, tint, edge),
            ("reset", faint, tint, edge),
            ("colour", faint, tint, edge),
            ("date", grey, tint, edge),
            ("check", grey, None, None),
            ("radio", grey, None, None),
            ("range", Some("rgb(197, 197, 197)"), None, edge),
            ("file", grey, None, edge),
            ("hidden", grey, None, edge),
            ("area", grey, tint, edge),
            ("list", Some("rgb(128, 128, 128)"), None, edge),
            ("choice", Some("rgb(128, 128, 128)"), None, None),
            ("inside", grey, tint, edge),
        ] {
            assert_eq!(computed(html, id, "color").as_deref(), color, "{id}");
            assert_eq!(
                computed(html, id, "background-color").as_deref(),
                background,
                "{id}"
            );
            for side in ["top", "right", "bottom", "left"] {
                let name = format!("border-{side}-color");
                let side = computed(html, id, &name);
                if border.is_some() {
                    assert_eq!(side.as_deref(), border, "{id}");
                } else {
                    assert_ne!(side.as_deref(), edge, "{id}");
                }
            }
        }
        // A list is faded as a whole, and nothing else is.
        assert_eq!(computed(html, "list", "opacity").as_deref(), Some("0.7"));
        assert_eq!(computed(html, "button", "opacity"), None);
        // A disabled fieldset looks as a live one does, and so does a live
        // field: their colours are the sheet's ordinary ones, the page's
        // text colour inherited.
        assert_eq!(computed(html, "group", "color").as_deref(), Some("black"));
        assert_eq!(computed(html, "group", "border-top-color"), None);
        assert_eq!(
            computed(html, "group", "border").as_deref(),
            Some("2px groove #c0c0c0")
        );
        assert_eq!(computed(html, "live", "color").as_deref(), Some("black"));
        assert_eq!(computed(html, "live", "border-top-color"), None);
        assert_eq!(computed(html, "live", "background-color"), None);
    }

    #[test]
    fn a_field_that_writes_a_date_or_a_time_has_a_pixel_before_it_alone() {
        // Chrome 154's computed values, measured (queue item 507).
        let html = "<input id=text><input id=date type=date><input id=time type=time>\
                    <input id=local type=datetime-local><input id=month type=month>\
                    <input id=week type=week><input id=number type=number>";
        for (id, padding) in [
            ("text", "1px 2px"),
            ("number", "1px 2px"),
            ("date", "0 0 0 1px"),
            ("time", "0 0 0 1px"),
            ("local", "0 0 0 1px"),
            ("month", "0 0 0 1px"),
            ("week", "0 0 0 1px"),
        ] {
            assert_eq!(
                computed(html, id, "padding").as_deref(),
                Some(padding),
                "{id}"
            );
        }
    }

    /// What the engine's sheet and then `author`'s give the element whose
    /// `id` is `wanted`, for the property `name`.
    fn computed_with(html: &str, author: &str, wanted: &str, name: &str) -> Option<String> {
        let document = alo_dom::parse_document(html);
        let engine = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let page = parse_stylesheet(author);
        let sheets = [
            crate::SourcedSheet::new(crate::Origin::UserAgent, &engine),
            crate::SourcedSheet::new(crate::Origin::Author, &page),
        ];
        let styles = crate::resolve(&document, &sheets, &alo_css::MediaContext::default());
        let id = document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))?;
        Some(styles.get(id)?.get(name)?.to_owned())
    }

    #[test]
    fn a_control_is_set_in_chromes_font_and_not_the_pages() {
        // Chrome 154's computed values, measured (queue item 506), inside a
        // paragraph whose own font says otherwise in every longhand.
        let html = "<p id=p><input id=text><button id=button>b</button>\
                    <select id=list></select><input id=box type=checkbox>\
                    <textarea id=area></textarea><input id=date type=date>\
                    <input id=time type=time><input id=local type=datetime-local>\
                    <input id=month type=month><input id=week type=week>\
                    <input id=number type=number><span id=span>s</span></p>";
        let page = "p { font-family: serif; font-size: 20px; font-weight: bold; \
                    font-style: italic; line-height: 3 }";
        for id in [
            "text", "button", "list", "box", "area", "date", "time", "local", "month", "week",
            "number",
        ] {
            for (name, value) in [
                ("font-size", "13.333333px"),
                ("font-weight", "normal"),
                ("font-style", "normal"),
                ("line-height", "normal"),
            ] {
                assert_eq!(
                    computed_with(html, page, id, name).as_deref(),
                    Some(value),
                    "{id} {name}"
                );
            }
        }
        for (id, family) in [
            ("text", "Arial, sans-serif"),
            ("button", "Arial, sans-serif"),
            ("list", "Arial, sans-serif"),
            ("number", "Arial, sans-serif"),
            ("area", "monospace"),
            ("date", "monospace"),
            ("time", "monospace"),
            ("local", "monospace"),
            ("month", "monospace"),
            ("week", "monospace"),
        ] {
            assert_eq!(
                computed_with(html, page, id, "font-family").as_deref(),
                Some(family),
                "{id}"
            );
        }
        // Text that is not a control keeps the paragraph's.
        assert_eq!(
            computed_with(html, page, "span", "font-size").as_deref(),
            Some("20px")
        );
    }

    #[test]
    fn a_page_takes_a_controls_font_back_with_font_inherit() {
        // alo's screens write `font: inherit` on their controls. Chrome then
        // computes the paragraph's whole font, its line included.
        let html = "<p><button id=button>b</button><input id=text></p>";
        let page = "p { font-family: serif; font-size: 20px; font-weight: bold; \
                    line-height: 3 } button { font: inherit } input { font-size: inherit }";
        for (name, value) in [
            ("font-size", "20px"),
            ("font-family", "serif"),
            ("font-weight", "bold"),
            ("line-height", "3"),
        ] {
            assert_eq!(
                computed_with(html, page, "button", name).as_deref(),
                Some(value),
                "button {name}"
            );
        }
        // One longhand takes back only itself.
        assert_eq!(
            computed_with(html, page, "text", "font-size").as_deref(),
            Some("20px")
        );
        assert_eq!(
            computed_with(html, page, "text", "line-height").as_deref(),
            Some("normal")
        );
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

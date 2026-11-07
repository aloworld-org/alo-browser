/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A field of alo's own drawn with its caret, its selection and an input
//! method's composition, its words scrolled to keep the caret in view
//! (queue items 861 and 848, `docs/design/shell.md`, *The one box as a
//! text field*).
//!
//! # Why the words are drawn twice, and read once
//!
//! The field is an `<input>`, and stays one: it is what an agent reads —
//! a `textbox` named as the design names it, holding its value, with the
//! selection as a range of characters (ADR 0002). But this engine draws an
//! `<input>`'s value as one run it cannot scroll, select or put a caret
//! in. So the stylesheet draws the field's own words in no colour, and the
//! template holds beside the field a **line**, `aria-hidden`, which this
//! file fills with the words drawn: in pieces where the selection or the
//! composition begins and ends — the selection on its tint, the composition
//! underlined — with the caret over them, and the whole line moved left by
//! how far it is scrolled, inside the field's own room, which clips it.
//! The line is built for painting and nothing reads it; the field it stands
//! over is read, and the two hold the same words.
//!
//! # What is measured
//!
//! Where the caret may stand is where each of the words' clusters ends, as
//! the very measurer the layout sets the line with measures them
//! ([`alo_layout::MeasureText::cluster_ends`]): a letter with its marks is
//! one cluster, so the caret never stands inside one. The caret is placed
//! by those numbers, on whole device pixels; the words are scrolled by them
//! ([`crate::field::kept_in_view`]). Both are written as the line's and the
//! caret's own style, numbers in CSS pixels, as the conversation's scroll
//! is (`conversation_document`).

use crate::field::{Drawn, Editing, FieldLaidOut, kept_in_view};
use crate::pipeline::Drawing;
use crate::start_document::{Unbuilt, append, attribute, by_id, element};
use alo_dom::Document;
use alo_layout::{MeasureText, TextStyle};

/// The class of a run of the words selected.
pub const SELECTED: &str = "selected";

/// The class of a run of the words an input method is composing.
pub const COMPOSED: &str = "composed";

/// The class of the caret.
pub const CARET: &str = "caret";

/// The class of the block the words' runs are laid out in, one line, the
/// caret beside it: a box placed by its numbers cannot stand in a line
/// this engine lays out (it is placed against a block, not a line's box).
pub const WORDS: &str = "field-words";

/// One of alo's fields, as its template names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The id of the `<input>` that is read.
    pub input: &'static str,
    /// The id of the line drawn over it.
    pub line: &'static str,
}

/// The one box on the first screen (`start.html`), its words 16 px.
pub const THE_BOX: Field = Field {
    input: crate::start_document::FIELD,
    line: "ask-line",
};

/// *Search chats* in the panel (`panel.html`), its words 13.5 px.
pub const SEARCH_CHATS: Field = Field {
    input: "find",
    line: "find-line",
};

/// The size of the one box's words, in CSS pixels (`.ask`, `.line`).
pub const BOX_SIZE: f32 = 16.0;

/// The size of *Search chats*' words, in CSS pixels (`.find-field`).
pub const SEARCH_SIZE: f32 = 13.5;

/// How a field's words are set: at `size` CSS pixels and weight 400 in
/// `family`, then `system-ui`, then `sans-serif`, as the stylesheets ask,
/// kept as they are typed, on a screen of `scale`.
pub fn style(family: &str, size: f32, scale: f32) -> TextStyle {
    TextStyle {
        families: vec![
            family.to_owned(),
            "system-ui".to_owned(),
            "sans-serif".to_owned(),
        ],
        size,
        weight: 400,
        white_space: alo_box::WhiteSpace::Pre,
        scale,
        ..TextStyle::default()
    }
}

/// Where `field` was laid out in `drawing` of `document`, its line laid as
/// `laid` says: its `<input>`'s box, which is the room its words have.
pub fn laid_out(
    drawing: &Drawing,
    document: &Document,
    field: Field,
    laid: &Laid,
) -> Option<FieldLaidOut> {
    let node = by_id(document, field.input).ok()?;
    let boxes = &drawing.boxes;
    let geometry = boxes
        .ids()
        .filter(|&made| {
            // The field's own box, never the text box it holds its value in.
            boxes
                .get(made)
                .filter(|held| !matches!(held.kind, alo_box::BoxKind::Text { .. }))
                .and_then(|held| held.kind.node())
                == Some(node)
        })
        .find_map(|made| drawing.layout.get(made))?;
    let content = geometry.border_box;
    Some(FieldLaidOut {
        left: content.left() + geometry.border.left + geometry.padding.left,
        top: content.top(),
        height: content.size.height,
        room: (content.size.width
            - geometry.border.left
            - geometry.border.right
            - geometry.padding.left
            - geometry.padding.right)
            .max(0.0),
        scrolled: laid.scrolled,
        stops: laid.stops.clone(),
    })
}

/// What laying a field's line out came to: each place the caret may stand
/// and how far the words were scrolled — what the renderer answers the
/// browser process with ([`crate::field::FieldLaidOut`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Laid {
    /// Each place the caret may stand: a character and how far from the
    /// words' start, unscrolled; the first the start, the last the end.
    pub stops: Vec<(usize, f32)>,
    /// How far the words are scrolled, in CSS pixels.
    pub scrolled: f32,
}

/// Where each cluster of `text` ends, in characters, with how far from its
/// start: `(0, 0)` first, and the text's end and whole width last.
pub fn stops(text: &str, measurer: &dyn MeasureText, style: &TextStyle) -> Vec<(usize, f32)> {
    let mut stops = vec![(0, 0.0)];
    let mut counted = 0;
    let mut bytes = 0;
    let mut chars = text.char_indices().peekable();
    for (end, x) in measurer.cluster_ends(text, style) {
        while let Some((at, _)) = chars.peek() {
            if *at >= end {
                break;
            }
            let _ = chars.next();
            counted += 1;
        }
        if end > bytes {
            bytes = end;
            stops.push((counted, x));
        }
    }
    let count = text.chars().count();
    if stops.last().is_some_and(|(at, _)| *at != count) {
        // A measurer with no fonts measures nothing: the end at nought.
        stops.push((count, 0.0));
    }
    stops
}

/// How far from the words' start the stop at or before `at` is.
fn x_at(stops: &[(usize, f32)], at: usize) -> f32 {
    stops
        .iter()
        .take_while(|(stop, _)| *stop <= at)
        .last()
        .map_or(0.0, |(_, x)| *x)
}

/// `value` on whole device pixels at `scale`.
fn on_pixels(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

/// Fill `field`'s line in `document` with `typed` as `editing` says it
/// stands — the caret and the selection only while the field has the
/// keyboard (`focused`) — measured with `measurer` in `style`, on a screen
/// of `scale`: where the caret may stand, and how far the words were
/// scrolled.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the field or its line, or `alo-dom`
/// refused an operation, which no state can cause.
pub fn put(
    document: &mut Document,
    field: Field,
    (typed, editing, focused): (&str, &Editing, bool),
    (measurer, style): (&dyn MeasureText, &TextStyle),
    scale: f32,
) -> Result<Laid, Unbuilt> {
    let input = by_id(document, field.input)?;
    let line = by_id(document, field.line)?;
    let drawn = if focused {
        editing.drawn(typed)
    } else {
        Drawn {
            text: typed.to_owned(),
            caret: 0,
            selected: None,
            composed: None,
        }
    };
    if drawn.composed.is_some() {
        // What is composed is in the field's value as it is drawn, as a
        // browser's field holds it while a person composes.
        attribute(document, input, "value", &drawn.text)?;
    }
    if focused {
        // The selection, or the caret, as the field's state: what an agent
        // reads (ADR 0002), in characters of the value.
        let (start, end) = drawn.selected.unwrap_or((drawn.caret, drawn.caret));
        let _ = alo_dom::text_selection::set(document, input, start, end);
    }
    let stops = stops(&drawn.text, measurer, style);
    let width = stops.last().map_or(0.0, |(_, x)| *x);
    let caret = on_pixels(x_at(&stops, drawn.caret), scale);
    let scrolled = match (focused, editing.room) {
        (false, _) => 0.0,
        (true, Some(room)) => kept_in_view(editing.scrolled, caret, width, room, scale),
        // Not laid out yet: as it was, never past the words.
        (true, None) => on_pixels(editing.scrolled.clamp(0.0, width), scale),
    };
    if scrolled > 0.0 {
        attribute(document, line, "style", &format!("left: {}px", -scrolled))?;
    }
    let words_block = element(document, "div")?;
    attribute(document, words_block, "class", WORDS)?;
    append(document, line, words_block)?;
    let mut cuts = vec![0];
    for (start, end) in [drawn.selected, drawn.composed].into_iter().flatten() {
        cuts.extend([start, end]);
    }
    let count = drawn.text.chars().count();
    cuts.push(count);
    cuts.sort_unstable();
    cuts.dedup();
    for pair in cuts.windows(2) {
        let [from, to] = pair else { continue };
        let words: String = drawn.text.chars().skip(*from).take(to - from).collect();
        let run = element(document, "span")?;
        let inside = |range: Option<(usize, usize)>| {
            range.is_some_and(|(start, end)| start <= *from && *to <= end)
        };
        if inside(drawn.selected) {
            attribute(document, run, "class", SELECTED)?;
        } else if inside(drawn.composed) {
            attribute(document, run, "class", COMPOSED)?;
        }
        let text = document.create_text_node(&words);
        append(document, run, text)?;
        append(document, words_block, run)?;
    }
    if focused && editing.caret_shown && drawn.selected.is_none() {
        let mark = element(document, "span")?;
        attribute(document, mark, "class", CARET)?;
        attribute(document, mark, "style", &format!("left: {caret}px"))?;
        append(document, line, mark)?;
    }
    Ok(Laid { stops, scrolled })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Composing;
    use alo_layout::BlockFont;

    const FIELD: Field = Field {
        input: "f",
        line: "l",
    };

    fn template() -> Document {
        alo_dom::parse_document(
            "<!doctype html><div><input id=f><div id=l aria-hidden=true></div></div>",
        )
    }

    /// Every run of the line — the words' runs, then the caret — with its
    /// class, if any, and its words.
    fn runs(document: &Document) -> Vec<(Option<String>, String)> {
        let Ok(line) = by_id(document, "l") else {
            return Vec::new();
        };
        document
            .children(line)
            .flat_map(|child| {
                if document.attribute_by_name(child, "class") == Some(WORDS) {
                    document.children(child).collect::<Vec<_>>()
                } else {
                    vec![child]
                }
            })
            .map(|run| {
                (
                    document.attribute_by_name(run, "class").map(str::to_owned),
                    document.text_content(run),
                )
            })
            .collect()
    }

    fn style_of(document: &Document, class: &str) -> Option<String> {
        document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some(class))
            .and_then(|node| document.attribute_by_name(node, "style"))
            .map(str::to_owned)
    }

    fn lay(typed: &str, editing: &Editing, focused: bool) -> (Document, Laid) {
        let mut document = template();
        if let Ok(input) = by_id(&document, "f") {
            let _ = document.set_attribute(input, "value", typed);
        }
        // A block font: every character eight pixels.
        let laid = put(
            &mut document,
            FIELD,
            (typed, editing, focused),
            (&BlockFont, &TextStyle::default()),
            1.0,
        );
        (document, laid.unwrap_or_default())
    }

    #[test]
    fn the_caret_stands_after_the_characters_before_it() {
        let (document, laid) = lay("kampala", &Editing::caret_at(3), true);
        assert_eq!(
            runs(&document),
            vec![
                (None, "kampala".to_owned()),
                (Some(CARET.to_owned()), String::new())
            ]
        );
        assert_eq!(style_of(&document, CARET).as_deref(), Some("left: 24px"));
        assert_eq!(laid.stops.len(), 8);
        assert_eq!(laid.stops.last().map(|(at, _)| *at), Some(7));
    }

    #[test]
    fn a_selection_is_its_own_run_and_hides_the_caret() {
        let (document, _) = lay("kampala", &Editing::caret_at(1).selecting_from(4), true);
        assert_eq!(
            runs(&document),
            vec![
                (None, "k".to_owned()),
                (Some(SELECTED.to_owned()), "amp".to_owned()),
                (None, "ala".to_owned()),
            ]
        );
        assert_eq!(style_of(&document, CARET), None);
        let input = by_id(&document, "f").ok();
        assert_eq!(
            input.and_then(|input| alo_dom::text_selection::of(&document, input)),
            Some((1, 4))
        );
    }

    #[test]
    fn a_composition_is_underlined_in_the_value_with_the_caret_in_it() {
        let editing = Editing {
            composing: Some(Composing {
                text: "きょう".to_owned(),
                caret: 2,
            }),
            ..Editing::caret_at(2)
        };
        let (document, _) = lay("ab", &editing, true);
        assert_eq!(
            runs(&document).into_iter().take(3).collect::<Vec<_>>(),
            vec![
                (None, "ab".to_owned()),
                (Some(COMPOSED.to_owned()), "きょう".to_owned()),
                (Some(CARET.to_owned()), String::new()),
            ]
        );
        let input = by_id(&document, "f").ok();
        assert_eq!(
            input.and_then(|input| document.attribute_by_name(input, "value")),
            Some("abきょう")
        );
        assert_eq!(style_of(&document, CARET).as_deref(), Some("left: 32px"));
    }

    #[test]
    fn long_words_scroll_to_keep_the_caret_in_view_and_unfocused_show_their_start() {
        let typed = "x".repeat(40);
        let editing = Editing {
            room: Some(300.0),
            ..Editing::caret_at(40)
        };
        let (document, laid) = lay(&typed, &editing, true);
        // 320 wide, the caret at 320 and one more, in 300: 21.
        assert_eq!(laid.scrolled.to_bits(), 21.0_f32.to_bits());
        let line = by_id(&document, "l").ok();
        assert_eq!(
            line.and_then(|line| document.attribute_by_name(line, "style")),
            Some("left: -21px")
        );
        let (document, laid) = lay(&typed, &editing, false);
        assert_eq!(laid.scrolled.to_bits(), 0.0_f32.to_bits());
        assert_eq!(style_of(&document, CARET), None);
        let input = by_id(&document, "f").ok();
        assert_eq!(
            input.and_then(|input| alo_dom::text_selection::of(&document, input)),
            None,
            "no selection told while it lacks the keyboard"
        );
    }

    #[test]
    fn nothing_typed_is_only_the_caret() {
        let (document, laid) = lay("", &Editing::caret_at(0), true);
        assert_eq!(
            runs(&document),
            vec![(Some(CARET.to_owned()), String::new())]
        );
        assert_eq!(laid.stops, vec![(0, 0.0)]);
        let (document, _) = lay("", &Editing::default(), false);
        assert!(runs(&document).is_empty());
    }
}

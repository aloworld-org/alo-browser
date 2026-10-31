/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The fonts a renderer is started with.
//!
//! A renderer cannot open a font file (ADR 0010), so the browser process
//! hands every one its fonts as it starts. Ordinarily they are this machine's
//! (`alo-renderer`'s `fonts::from_this_machine`). Under `--frozen-fonts` they
//! are the corpus's: the same five `DejaVu` faces, compiled in, that every
//! committed render was drawn with, and the same meaning for the generic
//! families — so a frozen page in the window is the page in the corpus, pixel
//! for pixel, and a capture of the window can be compared with a reference
//! rather than eyeballed.
//!
//! The interface's renderers — the chats panel's and the first screen's —
//! are handed the design's own typefaces as well ([`for_the_interface`],
//! queue item 706, ADR 0084), and only they: a page's renderer never is.

use alo_renderer::face::Face;
use alo_renderer::fonts::Machine;
use alo_renderer::generic::Generics;
use alo_text::{Slant, Weight};

/// The corpus's fonts, as a renderer is handed them.
///
/// The faces `alo-corpus`'s `corpus_fonts` loads, in its order, and the
/// generics it maps. **Both, or the window is not the corpus**: without the
/// generics a page's `system-ui` falls to whichever face is to hand, weight
/// unasked, and alo's headings stop being bold.
pub fn frozen() -> Machine {
    let faces = [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
        (
            "DejaVu Sans Mono",
            Weight::NORMAL,
            dejavu::sans_mono::regular(),
        ),
        ("DejaVu Sans Mono", Weight::BOLD, dejavu::sans_mono::bold()),
    ]
    .into_iter()
    .filter_map(|(family, weight, bytes)| Face::new(family, weight, Slant::Normal, bytes.to_vec()))
    .collect();
    let generics = Generics::stating(
        [
            ("sans-serif", "DejaVu Sans"),
            ("system-ui", "DejaVu Sans"),
            ("serif", "DejaVu Serif"),
            ("monospace", "DejaVu Sans Mono"),
        ]
        .into_iter()
        .map(|(generic, family)| (generic.to_owned(), family.to_owned()))
        .collect(),
    );
    Machine { faces, generics }
}

/// The faces an interface renderer is handed: `faces`, then the design's
/// typefaces (`alo-renderer`'s `typefaces`) that are not among them already.
///
/// Called where the conversation's renderers start
/// ([`crate::conversation_drawing::ConversationDrawing::start`]), and nowhere
/// a page's renderers start: the typefaces are the interface's, so no page is
/// drawn in them and none can find from its fonts that it is in this
/// browser.
pub fn for_the_interface(faces: &[Face]) -> Vec<Face> {
    let mut handed = faces.to_vec();
    for face in alo_renderer::typefaces::faces() {
        if !handed.contains(&face) {
            handed.push(face);
        }
    }
    handed
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::typefaces::{INSTRUMENT_SANS, NEWSREADER};

    fn families(faces: &[Face]) -> Vec<&str> {
        faces.iter().map(|face| face.family.as_str()).collect()
    }

    #[test]
    fn the_interface_is_handed_the_frozen_faces_then_the_designs_two() {
        let handed = for_the_interface(&frozen().faces);
        assert_eq!(
            families(&handed),
            vec![
                "DejaVu Sans",
                "DejaVu Sans",
                "DejaVu Serif",
                "DejaVu Sans Mono",
                "DejaVu Sans Mono",
                INSTRUMENT_SANS,
                NEWSREADER
            ]
        );
        // Handed twice over is handed once.
        assert_eq!(for_the_interface(&handed), handed);
    }

    #[test]
    fn a_pages_fonts_never_hold_the_designs_typefaces() {
        let page = families(&frozen().faces).join(", ");
        assert!(!page.contains(INSTRUMENT_SANS), "{page}");
        assert!(!page.contains(NEWSREADER), "{page}");
    }

    #[test]
    fn the_frozen_faces_are_the_corpus_five() {
        let machine = frozen();
        let named: Vec<(&str, u16)> = machine
            .faces
            .iter()
            .map(|face| (face.family.as_str(), face.weight))
            .collect();
        assert_eq!(
            named,
            vec![
                ("DejaVu Sans", 400),
                ("DejaVu Sans", 700),
                ("DejaVu Serif", 400),
                ("DejaVu Sans Mono", 400),
                ("DejaVu Sans Mono", 700)
            ]
        );
    }

    #[test]
    fn the_frozen_generics_are_the_corpus_four() {
        let machine = frozen();
        assert_eq!(
            machine.generics.families_of("sans-serif"),
            vec!["DejaVu Sans"]
        );
        assert_eq!(
            machine.generics.families_of("system-ui"),
            vec!["DejaVu Sans"]
        );
        assert_eq!(machine.generics.families_of("serif"), vec!["DejaVu Serif"]);
        assert_eq!(
            machine.generics.families_of("monospace"),
            vec!["DejaVu Sans Mono"]
        );
    }
}

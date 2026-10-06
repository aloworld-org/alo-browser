/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The fonts a renderer is started with.
//!
//! A renderer cannot open a font file (ADR 0010), so the browser process
//! hands every one its fonts as it starts. Ordinarily they are this machine's
//! (`alo-renderer`'s `fonts::from_this_machine`). Under `--frozen-fonts` they
//! are the corpus's: the same three `DejaVu` faces, compiled in, that every
//! committed render was drawn with, and the same meaning for the generic
//! families — so a frozen page in the window is the page in the corpus, pixel
//! for pixel, and a capture of the window can be compared with a reference
//! rather than eyeballed.

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
    ]
    .into_iter()
    .filter_map(|(family, weight, bytes)| Face::new(family, weight, Slant::Normal, bytes.to_vec()))
    .collect();
    let generics = Generics::stating(
        [
            ("sans-serif", "DejaVu Sans"),
            ("system-ui", "DejaVu Sans"),
            ("serif", "DejaVu Serif"),
        ]
        .into_iter()
        .map(|(generic, family)| (generic.to_owned(), family.to_owned()))
        .collect(),
    );
    Machine { faces, generics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frozen_faces_are_the_corpus_three() {
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
                ("DejaVu Serif", 400)
            ]
        );
    }

    #[test]
    fn the_frozen_generics_are_the_corpus_three() {
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
    }
}

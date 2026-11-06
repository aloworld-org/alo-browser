/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 829: which face draws a character — a letter, an emoji, one
//! asked for as either by a variation selector — chosen as Chrome 156
//! chooses it.
//!
//! **Measured** in Chrome 156.0.8078.76 (`--version`), headless, its source
//! not read: seventeen characters and sequences at 100 px, each in thirty
//! `font-family` lists of four faces made for it with `fontTools` 4.61 under
//! this repository's licence, and `DejaVu Sans`, each face told apart by
//! its advance (`getBoundingClientRect().width`, 510 widths, 476 of them here):
//!
//! - `tests/fonts/choice-plain.ttf` — outlines, 600 units wide;
//! - `choice-cbdt.ttf` — `CBDT` pictures, 700;
//! - `choice-colr.ttf` — `COLR` version 0 layers, 800;
//! - `choice-sbix.ttf` — `sbix` pictures, 900;
//!
//! each mapping `B`, `#`, `1`, `©`, U+263A ☺, U+2764 ❤, U+2B50 ⭐, U+E000,
//! U+1F600 😀 and U+1F642 🙂 (no selector). A width none of them has was
//! the system's: Apple Color Emoji's (100 px, `e` below) or a face of
//! letters (`t`). Here the system is two stand-ins after the listed faces:
//! `choice-system-emoji.ttf` (the `CBDT` face renamed) and
//! `choice-system-text.ttf` (the plain face less 🙂 and 😀, which no face
//! of letters on this Mac has — Chrome went to its emoji face for 🙂 asked
//! as a letter there).
//!
//! Left out: the lists naming `sans-serif`, which Chrome on macOS widens
//! into CoreText's own cascade of faces before the next family — a
//! machine's list, not a rule this engine can take.

use alo_text::{Font, FontDatabase, FontRequest, Slant, Weight, split};

/// The characters and sequences measured, in the order of each row below.
const SEQUENCES: [&str; 17] = [
    "B",
    "\u{1f642}",
    "\u{1f642}\u{fe0e}",
    "\u{1f642}\u{fe0f}",
    "\u{263a}",
    "\u{263a}\u{fe0e}",
    "\u{263a}\u{fe0f}",
    "\u{2764}",
    "\u{2764}\u{fe0f}",
    "\u{a9}",
    "\u{a9}\u{fe0f}",
    "\u{e000}",
    "#",
    "#\u{fe0f}",
    "\u{1f600}",
    "\u{2b50}",
    "\u{2b50}\u{fe0e}",
];

/// Chrome's answer for each list, a letter per sequence: `P`, `C`, `L`,
/// `S` and `D` the listed faces, `e` the system's emoji face and `t` its
/// face of letters. `X` names a family nobody has.
const MEASURED: [(&str, &str); 28] = [
    ("P", "PPPePPePePePPePPP"),
    ("C", "CCCCCtCCCCCCCCCCt"),
    ("L", "LLLLLtLLLLLLLLLLt"),
    ("S", "SSSSSSSSSSSSSSSSS"),
    ("D", "DeeeDDeDeDetDeDet"),
    ("X", "teeettetetetteeet"),
    ("C,P", "CCPCCPCCCCCCCCCCP"),
    ("P,C", "PPPCPPCPCPCPPCPPP"),
    ("L,C", "LLLLLtLLLLLLLLLLt"),
    ("C,L", "CCCCCtCCCCCCCCCCt"),
    ("P,L", "PPPLPPLPLPLPPLPPP"),
    ("L,P", "LLPLLPLLLLLLLLLLP"),
    ("S,P", "SSPSSPSSSSSSSSSSP"),
    ("P,S", "PPPSPPSPSPSPPSPPP"),
    ("S,C", "SSSSSSSSSSSSSSSSS"),
    ("C,S", "CCCCCtCCCCCCCCCCt"),
    ("L,S", "LLLLLtLLLLLLLLLLt"),
    ("S,L", "SSSSSSSSSSSSSSSSS"),
    ("D,C", "DCCCDDCDCDCCDCDCt"),
    ("C,D", "CCCCCDCCCCCCCCCCt"),
    ("D,L", "DLLLDDLDLDLLDLDLt"),
    ("L,D", "LLLLLDLLLLLLLLLLt"),
    ("D,S", "DSSSDDSDSDSSDSDSt"),
    ("X,C", "CCCCCtCCCCCCCCCCt"),
    ("X,P", "PPPePPePePePPePPP"),
    ("P,X", "PPPePPePePePPePPP"),
    ("D,P", "DPPeDDeDeDePDeDPP"),
    ("P,D", "PPPePPePePePPePPP"),
];

/// A face's family, by its letter in [`MEASURED`].
fn family(letter: char) -> &'static str {
    match letter {
        'P' => "alo Plain Probe",
        'C' => "alo Cbdt Probe",
        'L' => "alo Colr Probe",
        'S' => "alo Sbix Probe",
        'D' => "DejaVu Sans",
        'e' => "alo System Emoji",
        't' => "alo System Text",
        _ => "alo Nothing",
    }
}

fn bytes(letter: char) -> &'static [u8] {
    match letter {
        'P' => include_bytes!("fonts/choice-plain.ttf"),
        'C' => include_bytes!("fonts/choice-cbdt.ttf"),
        'L' => include_bytes!("fonts/choice-colr.ttf"),
        'S' => include_bytes!("fonts/choice-sbix.ttf"),
        'D' => dejavu::sans::regular(),
        'e' => include_bytes!("fonts/choice-system-emoji.ttf"),
        't' => include_bytes!("fonts/choice-system-text.ttf"),
        _ => &[],
    }
}

/// The faces a list names, then the system's two.
fn database(list: &str) -> FontDatabase {
    let mut database = FontDatabase::new();
    for letter in list
        .split(',')
        .filter_map(|name| name.chars().next())
        .chain(['t', 'e'])
    {
        if let Some(font) = Font::load(
            family(letter),
            Weight::NORMAL,
            Slant::Normal,
            bytes(letter).to_vec(),
        ) {
            database.add(font);
        }
    }
    database
}

#[test]
fn every_measured_choice_is_chromes() {
    let mut compared = 0;
    for (list, answers) in MEASURED {
        let fonts = database(list);
        let families: Vec<String> = list
            .split(',')
            .filter_map(|name| name.chars().next())
            .map(|letter| family(letter).to_owned())
            .collect();
        let request = FontRequest {
            families,
            ..FontRequest::default()
        };
        for (sequence, answer) in SEQUENCES.iter().zip(answers.chars()) {
            let runs = split(sequence, &fonts, &request);
            let chosen: Vec<String> = runs
                .iter()
                .map(|run| {
                    run.font
                        .as_ref()
                        .map_or_else(String::new, |font| font.family().to_owned())
                })
                .collect();
            assert_eq!(
                chosen,
                [family(answer)],
                "{sequence:?} in {list}: Chrome took {answer}"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 28 * 17);
}

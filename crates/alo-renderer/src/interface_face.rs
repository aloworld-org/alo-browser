/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which face the interface's words are set in (queue items 853 and 854,
//! `docs/design/shell.md`, *Settings*; approved by the owner 2026-11-07).
//!
//! Every word of alo's own interface is set in one face, **Inter** unless
//! a person chose another in *Settings*: Geist, DM Sans, Plus Jakarta Sans
//! or Instrument Sans. The mark *alo* and the greeting stay in Newsreader
//! whatever is chosen, and a page in a card keeps its own fonts — none of
//! these is ever handed to a page's renderer (ADR 0084 § 3).
//!
//! # How a document is set in it
//!
//! The interface's stylesheets ask for `var(--face), system-ui,
//! sans-serif` wherever they set words, and say `--face` is Inter on the
//! root. The document builders put the face chosen on the root element as
//! its own style ([`InterfaceFace::declaration`]), so the whole document
//! follows it at once — no stylesheet is changed and no font handed again,
//! since the interface's renderers hold all five from the start. Which face
//! is chosen is the browser process's to say, as one of a closed list: the
//! declaration is built from this file's names, never from a stranger's
//! string.

use alo_dom::Document;

/// A face the interface's words may be set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InterfaceFace {
    /// Inter: the default.
    #[default]
    Inter,
    /// Geist.
    Geist,
    /// DM Sans.
    DmSans,
    /// Plus Jakarta Sans.
    PlusJakartaSans,
    /// Instrument Sans: the interface's face until 7 November 2026.
    InstrumentSans,
}

impl InterfaceFace {
    /// Every face, in the order *Settings* offers them: the default first.
    pub const ALL: [Self; 5] = [
        Self::Inter,
        Self::Geist,
        Self::DmSans,
        Self::PlusJakartaSans,
        Self::InstrumentSans,
    ];

    /// What a person calls it: what *Settings* shows, what an agent reads
    /// each choice as, and what the settings file writes.
    pub fn name(self) -> &'static str {
        match self {
            Self::Inter => "Inter",
            Self::Geist => "Geist",
            Self::DmSans => "DM Sans",
            Self::PlusJakartaSans => "Plus Jakarta Sans",
            Self::InstrumentSans => "Instrument Sans",
        }
    }

    /// The family its font file says it is, which a stylesheet asks for
    /// ([`crate::typefaces`]): Inter's calls itself *Inter Variable*.
    pub fn family(self) -> &'static str {
        match self {
            Self::Inter => crate::typefaces::INTER,
            Self::Geist => crate::typefaces::GEIST,
            Self::DmSans => crate::typefaces::DM_SANS,
            Self::PlusJakartaSans => crate::typefaces::PLUS_JAKARTA_SANS,
            Self::InstrumentSans => crate::typefaces::INSTRUMENT_SANS,
        }
    }

    /// The face a person calls `name`, exactly as [`InterfaceFace::name`]
    /// writes it; [`None`] for any other.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|face| face.name() == name)
    }

    /// The face at `place` in [`InterfaceFace::ALL`], as the wire carries
    /// it.
    pub fn at(place: usize) -> Option<Self> {
        Self::ALL.get(place).copied()
    }

    /// Its place in [`InterfaceFace::ALL`].
    pub fn place(self) -> usize {
        Self::ALL
            .iter()
            .position(|face| *face == self)
            .unwrap_or_default()
    }

    /// The style a document's element is given to be set in this face: the
    /// custom property the interface's stylesheets ask for.
    pub fn declaration(self) -> String {
        format!("--face: \"{}\"", self.family())
    }

    /// Set `document` in this face: its root element given
    /// [`InterfaceFace::declaration`] as its style.
    ///
    /// # Errors
    ///
    /// Why not, in words, when the document has no element at its root,
    /// which no template of the interface lacks.
    pub fn set_on(self, document: &mut Document) -> Result<(), String> {
        let root = document
            .document_element()
            .ok_or_else(|| "the document has no root element to set its face on".to_owned())?;
        document
            .set_attribute(root, "style", &self.declaration())
            .ok_or_else(|| "the face could not be set on the root element".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inter_is_the_default_and_first_of_five() {
        assert_eq!(InterfaceFace::default(), InterfaceFace::Inter);
        assert_eq!(
            InterfaceFace::ALL.map(InterfaceFace::name),
            [
                "Inter",
                "Geist",
                "DM Sans",
                "Plus Jakarta Sans",
                "Instrument Sans"
            ]
        );
    }

    #[test]
    fn each_is_asked_for_by_the_family_its_file_says_and_named_back() {
        assert_eq!(InterfaceFace::Inter.family(), "Inter Variable");
        assert_eq!(InterfaceFace::DmSans.family(), "DM Sans");
        let carried: Vec<String> = crate::typefaces::faces()
            .into_iter()
            .map(|face| face.family)
            .collect();
        for face in InterfaceFace::ALL {
            assert!(
                carried.iter().any(|family| family == face.family()),
                "{face:?}"
            );
            assert_eq!(InterfaceFace::named(face.name()), Some(face));
            assert_eq!(InterfaceFace::at(face.place()), Some(face));
        }
        // Only as written: no other case, no family, no stray space.
        for stranger in ["inter", "Inter Variable", " Geist", "Comic Sans", ""] {
            assert_eq!(InterfaceFace::named(stranger), None, "{stranger:?}");
        }
        assert_eq!(InterfaceFace::at(5), None);
    }

    #[test]
    fn its_declaration_sets_the_custom_property_the_sheets_ask_for() {
        assert_eq!(
            InterfaceFace::PlusJakartaSans.declaration(),
            "--face: \"Plus Jakarta Sans\""
        );
        assert_eq!(
            InterfaceFace::Inter.declaration(),
            "--face: \"Inter Variable\""
        );
        let mut document =
            alo_dom::parse_document("<!doctype html><html><body>words</body></html>");
        assert_eq!(InterfaceFace::Geist.set_on(&mut document), Ok(()));
        let root = document.document_element();
        assert_eq!(
            root.and_then(|root| document.attribute_by_name(root, "style")),
            Some("--face: \"Geist\"")
        );
    }
}

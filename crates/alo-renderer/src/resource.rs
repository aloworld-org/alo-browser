/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A picture a page asked for, already fetched: its name, its type and its
//! bytes.
//!
//! # Why the type travels with the bytes
//!
//! A raster picture is recognised by what its bytes begin with, because a
//! PNG's signature cannot be faked without the file being a PNG. An SVG file
//! cannot be recognised that way. It is text, and nearly any XML or HTML can be
//! made to look like one, so ADR 0027 § 1 makes a resource an SVG picture
//! **only** when its type says `image/svg+xml`. The pair of a `src` and some
//! bytes that the pipeline was handed until then had nowhere to say that, so
//! the type is carried beside them here.
//!
//! The type is the essence of a response's `Content-Type`. A resource that had
//! no response, such as a frozen corpus file or a `file:` URL, has the type its
//! file name's extension stands in for, as every engine reading a file from a
//! disk does ([`Resource::from_file`]). Only `.svg` stands in for anything:
//! a raster format is still decided by its bytes, so naming it from an
//! extension would be a second answer to a question already answered.

/// The one type that makes a resource an SVG picture (ADR 0027 § 1).
pub const SVG_TYPE: &str = "image/svg+xml";

/// A picture a page names, and what came back for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// The `src`, exactly as the page wrote it.
    pub src: String,
    /// The `Content-Type` it came with, if it came with one.
    pub content_type: Option<String>,
    /// What came back.
    pub bytes: Vec<u8>,
}

impl Resource {
    /// A resource that came with a `Content-Type`, or with none.
    pub fn new(src: impl Into<String>, content_type: Option<&str>, bytes: Vec<u8>) -> Self {
        Self {
            src: src.into(),
            content_type: content_type.map(str::to_owned),
            bytes,
        }
    }

    /// A resource read from a file, with no response to give it a type.
    ///
    /// The extension of `file_name`, the file that holds the bytes, stands in
    /// for the type: `.svg`, in any case, is `image/svg+xml`, and anything
    /// else is no type at all. The `src` is not consulted, because a name a
    /// page wrote says nothing about what is behind it.
    pub fn from_file(src: impl Into<String>, file_name: &str, bytes: Vec<u8>) -> Self {
        let is_svg = std::path::Path::new(file_name)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
        Self::new(src, is_svg.then_some(SVG_TYPE), bytes)
    }

    /// Whether this resource says it is an SVG picture.
    ///
    /// The essence of the type, compared without regard to ASCII case: the
    /// part before any `;`, with the white space around it trimmed.
    /// `image/svg+xml; charset=utf-8` says so and `text/xml` does not.
    pub fn is_svg(&self) -> bool {
        self.content_type.as_deref().is_some_and(|content_type| {
            content_type
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case(SVG_TYPE)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_essence_of_the_type_decides_and_its_case_does_not() {
        for said in [
            "image/svg+xml",
            "IMAGE/SVG+XML",
            " image/svg+xml ; charset=utf-8",
        ] {
            assert!(
                Resource::new("a", Some(said), Vec::new()).is_svg(),
                "{said}"
            );
        }
        for said in [
            "text/xml",
            "application/xml",
            "image/svg",
            "image/svg+xml-ish",
            "text/html; image/svg+xml",
            "",
        ] {
            assert!(
                !Resource::new("a", Some(said), Vec::new()).is_svg(),
                "{said}"
            );
        }
        assert!(!Resource::new("a.svg", None, Vec::new()).is_svg());
    }

    #[test]
    fn a_file_with_no_response_takes_its_type_from_its_own_extension() {
        assert!(Resource::from_file("/hand", "hand.svg", Vec::new()).is_svg());
        assert!(Resource::from_file("/hand", "HAND.SVG", Vec::new()).is_svg());
        assert!(!Resource::from_file("/hand.svg", "hand.png", Vec::new()).is_svg());
        assert!(!Resource::from_file("/hand.svg", "hand.svg.txt", Vec::new()).is_svg());
        assert!(!Resource::from_file("/hand.svg", "svg", Vec::new()).is_svg());
        assert_eq!(
            Resource::from_file("/x", "x.png", Vec::new()).content_type,
            None,
            "a raster format is decided by its bytes, not named here",
        );
    }
}

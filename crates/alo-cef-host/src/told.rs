/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the host tells alo beside the DevTools pipe (queue items 799 and
//! 798): what Chromium tells the host of a card's page that nothing on the
//! pipe carries — the pointer's shape the page asks for, a context menu
//! the page would open and what it is over, and a link a person asked to
//! open somewhere else.
//!
//! alo starts the host as it starts Chrome, through a shell that moves the
//! pipe to descriptors 3 and 4 (`alo-window`'s `chrome.rs`); it also moves
//! the host's error output, which alo reads, to descriptor 5, and the
//! host's own error output nowhere. So the host writes one line here per
//! thing told, to `/dev/fd/5`, opened once — and nothing when the
//! descriptor is not open: a host started by hand tells nobody. The
//! machine's Chrome writes nothing to it. For a windowless page's host the
//! descriptor is a socket, and alo tells the host of the page's view on it
//! the other way (`heard.rs`, queue item 979).
//!
//! # One line each
//!
//! A line is a word saying what it is, then its fields, each after a tab
//! ([`Told::line`]). A field is text a stranger may have chosen — a link's
//! address, the words a person selected on a page — so a backslash, a tab,
//! a line's end and a carriage return in it are written `\\`, `\t`, `\n`
//! and `\r`, and it is cut at [`LONGEST`] characters. alo reads each line
//! as words from a stranger, bounded, and refuses what it cannot read.
//!
//! Only a card's page tells anything, and only while the card is unseen
//! ([`crate::parked::Stage::tells`]): once expanded the page is in a window
//! a person works in, which Chromium answers itself.

use std::cell::RefCell;
use std::fs::File;
use std::io::Write as _;

/// Where the lines go: the descriptor alo moves its reading end to.
pub const DESCRIPTOR: &str = "/dev/fd/5";

/// The most characters of one field written: a link's address or the words
/// selected are cut here, past any address a person reads and any words
/// searched for.
pub const LONGEST: usize = 2048;

/// What the host tells alo of a card's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    /// The pointer's shape the page asks for where the pointer is, by its
    /// name in CSS (`pointer` the hand, `text` the I-beam, `default` the
    /// arrow).
    Cursor(&'static str),
    /// A press of the secondary button the page did not take for itself:
    /// where, in the page's CSS pixels, and what it was over.
    Menu {
        /// From the page's left edge.
        x: i32,
        /// From the page's top edge.
        y: i32,
        /// The link's address it was over, or nothing.
        link: String,
        /// The words selected on the page, or nothing.
        selection: String,
        /// Whether it was over a field a person types in.
        editable: bool,
    },
    /// A link a person asked to open somewhere else — pressed with ⌘, or
    /// with the middle button — which the page did not take for itself:
    /// its address, never followed here.
    Open(String),
}

impl Told {
    /// The line written for this, without its end.
    pub fn line(&self) -> String {
        match self {
            Self::Cursor(shape) => format!("cursor\t{}", field(shape)),
            Self::Menu {
                x,
                y,
                link,
                selection,
                editable,
            } => format!(
                "menu\t{x}\t{y}\t{}\t{}\t{}",
                field(link),
                field(selection),
                u8::from(*editable)
            ),
            Self::Open(address) => format!("open\t{}", field(address)),
        }
    }
}

/// `text` as a field: cut at [`LONGEST`] characters, its backslashes,
/// tabs, line ends and carriage returns written as two characters each.
pub fn field(text: &str) -> String {
    let mut written = String::with_capacity(text.len().min(LONGEST));
    for character in text.chars().take(LONGEST) {
        match character {
            '\\' => written.push_str("\\\\"),
            '\t' => written.push_str("\\t"),
            '\n' => written.push_str("\\n"),
            '\r' => written.push_str("\\r"),
            other => written.push(other),
        }
    }
    written
}

/// The descriptor, as far as the host has tried it.
#[derive(Debug, Default)]
enum ToAlo {
    /// Not yet opened.
    #[default]
    Untried,
    /// Open, and written to.
    Open(File),
    /// Not open, or alo stopped reading: nothing is told again.
    Shut,
}

thread_local! {
    /// The descriptor, on CEF's UI thread, where every callback that tells
    /// runs.
    static TO_ALO: RefCell<ToAlo> = const { RefCell::new(ToAlo::Untried) };
}

/// Tell alo `told`, if alo is listening; a write that fails stops the
/// telling for good, since alo has stopped reading.
pub fn tell(told: &Told) {
    TO_ALO.with_borrow_mut(|to| {
        if matches!(to, ToAlo::Untried) {
            *to = std::fs::OpenOptions::new()
                .append(true)
                .open(DESCRIPTOR)
                .map_or(ToAlo::Shut, ToAlo::Open);
        }
        let ToAlo::Open(open) = to else {
            return;
        };
        let mut line = told.line();
        line.push('\n');
        if open.write_all(line.as_bytes()).is_err() {
            *to = ToAlo::Shut;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_is_a_word_then_its_fields_after_tabs() {
        assert_eq!(Told::Cursor("pointer").line(), "cursor\tpointer");
        assert_eq!(
            Told::Menu {
                x: 40,
                y: -3,
                link: "https://example.com/a".to_owned(),
                selection: String::new(),
                editable: false,
            }
            .line(),
            "menu\t40\t-3\thttps://example.com/a\t\t0"
        );
        assert_eq!(
            Told::Open("https://example.com/b".to_owned()).line(),
            "open\thttps://example.com/b"
        );
    }

    #[test]
    fn a_strangers_tab_line_end_or_backslash_cannot_end_a_field() {
        assert_eq!(field("a\tb\nc\rd\\e"), "a\\tb\\nc\\rd\\\\e");
        let selected = Told::Menu {
            x: 0,
            y: 0,
            link: String::new(),
            selection: "two\nlines\tand more".to_owned(),
            editable: true,
        }
        .line();
        assert_eq!(selected.matches('\t').count(), 5, "{selected}");
        assert!(!selected.contains('\n'));
        assert!(selected.ends_with("\t1"));
    }

    #[test]
    fn a_field_is_cut_at_its_longest() {
        let long = "é".repeat(LONGEST + 10);
        assert_eq!(field(&long).chars().count(), LONGEST);
        assert_eq!(field("short"), "short");
    }
}

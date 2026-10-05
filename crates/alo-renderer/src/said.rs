/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How much one load may say about its page (queue item 243, after 242).
//!
//! Everything a load says crosses to the browser process in **one** message,
//! which the wire caps at [`LARGEST_MESSAGE`](crate::wire::LARGEST_MESSAGE). A
//! load whose answer was larger than that would have its whole answer refused:
//! the tab would see a renderer that failed, with every issue lost and no
//! reason given. And what a load says is chosen by whoever wrote the page.
//!
//! Two things make a page's markup say more than it is. **How many** lines: an
//! `<img>` is five bytes of page, and `an <img> with no src` is twenty-eight
//! bytes of answer with the eight that say its length, so eleven and a half
//! megabytes of them — far under the cap the page itself crossed in — would
//! make an answer the wire refuses. And **how long** a line is: a line quoting
//! what the page wrote quotes it escaped, and one control character is five
//! (`\u{1}`), so a picture's `src` of fourteen megabytes of them is a line of
//! seventy.
//!
//! So a load says at most [`MOST_SAID_OF_MARKUP`] things about its markup and
//! then how many more there were — the way it says at most
//! [`MOST_SAID`](crate::scripts::MOST_SAID) about its scripts — and **every**
//! line it says, of either half, is at most [`LONGEST_LINE`] characters and
//! then how many more there were. A ceiling on what is said is not a ceiling on
//! what is rendered: the page is laid out and drawn the same either way.

use std::fmt::{self, Display, Write};

use crate::pipeline::Rendered;

/// The most lines one load says about its markup.
///
/// As many as it says about its scripts, for the same reason: past a few
/// hundred, a page is saying the same thing again, and the count says how many
/// times.
pub const MOST_SAID_OF_MARKUP: usize = 256;

/// The most characters of one line a load says, before saying how many more
/// there were.
///
/// Long enough that no report of a throw is ever cut: its name and message are
/// each kept to 1024 code units already (by the event loop, as it describes
/// one), and its trace to 32 places of well under a hundred characters each,
/// which is well under eight thousand together. Short enough that
/// [`MOST_SAID_OF_MARKUP`] and [`MOST_SAID`](crate::scripts::MOST_SAID) lines
/// of it, at four bytes a character at most, are sixteen megabytes, against a
/// cap of sixty-four.
pub const LONGEST_LINE: usize = 8192;

/// `what` as a line one load may say: its first [`LONGEST_LINE`] characters,
/// and how many more it had.
///
/// The characters past the ceiling are counted as they are written and never
/// kept, so a line that would have been seventy megabytes costs a counter.
pub fn line(what: &dyn Display) -> String {
    let mut line = Line::default();
    // `Line` never refuses a write, so this fails only if `what`'s own
    // `Display` does — which is a bug in it, and what was kept is still true.
    let _ = write!(line, "{what}");
    if line.more > 0 {
        let _ = write!(
            line.text,
            "… and {} more characters not said: one line says at most {LONGEST_LINE}",
            line.more
        );
    }
    line.text
}

/// What a page's markup made the engine say — the document's, the sheets',
/// the pictures', the boxes', layout's and the fonts' — as one load says it:
/// at most [`MOST_SAID_OF_MARKUP`] lines, each by [`line`], and then how many
/// more there were.
///
/// Only the lines said are written out; the rest are counted, so a page of
/// three million refusals costs three million steps and no memory.
pub fn of_markup(rendered: &Rendered) -> Vec<String> {
    let mut said = Vec::new();
    let mut left_out = 0_usize;
    for issue in rendered.each_issue() {
        if said.len() < MOST_SAID_OF_MARKUP {
            said.push(line(issue));
        } else {
            left_out = left_out.saturating_add(1);
        }
    }
    if left_out > 0 {
        said.push(format!(
            "{left_out} more things about this page's markup were not said: one load says at \
             most {MOST_SAID_OF_MARKUP}"
        ));
    }
    said
}

/// A line as it is written: the characters kept, and a count of the rest.
///
/// Named for what it is rather than for what it does to its input: `alo-net`
/// already exports a `Kept`, which is the record of what an agent did, and
/// two unrelated types of that name in one repository is one too many.
#[derive(Default)]
struct Line {
    text: String,
    kept: usize,
    more: usize,
}

impl Write for Line {
    fn write_str(&mut self, written: &str) -> fmt::Result {
        for character in written.chars() {
            if self.kept < LONGEST_LINE {
                self.text.push(character);
                self.kept += 1;
            } else {
                self.more = self.more.saturating_add(1);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_line_is_said_whole() {
        assert_eq!(line(&"an <img> with no src"), "an <img> with no src");
    }

    #[test]
    fn a_line_of_exactly_the_ceiling_is_said_whole() {
        let exactly = "é".repeat(LONGEST_LINE);
        assert_eq!(line(&exactly), exactly);
    }

    #[test]
    fn a_long_line_keeps_the_ceiling_in_characters_and_counts_the_rest() {
        // Characters, not bytes: each `é` is two.
        let said = line(&"é".repeat(LONGEST_LINE + 976));
        let expected = format!(
            "{}… and 976 more characters not said: one line says at most 8192",
            "é".repeat(LONGEST_LINE)
        );
        assert_eq!(said, expected);
    }

    #[test]
    fn a_line_written_in_pieces_is_cut_inside_one() {
        // Three thousand pieces of three: the ceiling falls two characters
        // into the 2731st.
        struct Pieces;
        impl Display for Pieces {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for _ in 0..3000 {
                    f.write_str("abc")?;
                }
                Ok(())
            }
        }
        let expected = format!(
            "{}ab… and 808 more characters not said: one line says at most 8192",
            "abc".repeat(2730)
        );
        assert_eq!(line(&Pieces), expected);
    }
}

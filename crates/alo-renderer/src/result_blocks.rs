/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a finished run's result is made of, read from the words the model
//! wrote (queue item 883): its paragraphs, each keeping its lines, and its
//! lists, so the museum's hours are drawn as the lines they were written
//! as rather than one run of text.
//!
//! The model is told to write plain text with no markdown (ADR 0096), so
//! this reads only what plain text has: a line ends a line, an empty line
//! ends a paragraph, and a line that begins `- `, `* `, `• ` or `– ` is a
//! bulleted list's item, one that begins with a number and `. ` or `) ` a
//! numbered list's. Nothing else is read as structure — no emphasis, no
//! headings, no links — so a stranger's words never become anything but
//! text ([`crate::result_document`] makes each a text node).

/// One block of a result: a paragraph of lines, or a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A paragraph, each line as written.
    Lines(Vec<String>),
    /// A list: its items, each without its bullet; a numbered list's items
    /// keep their numbers as written, so a list that begins at 3 says 3.
    List {
        /// Whether its items were numbered.
        numbered: bool,
        /// Its items.
        items: Vec<String>,
    },
}

/// The blocks of `said`, in order; none for words that are only spaces.
#[must_use]
pub fn blocks(said: &str) -> Vec<Block> {
    let mut made = Vec::new();
    let mut open: Option<Block> = None;
    for line in said.lines().map(str::trim) {
        if line.is_empty() {
            made.extend(open.take());
            continue;
        }
        match (item(line), &mut open) {
            (
                Some((numbered, words)),
                Some(Block::List {
                    numbered: was,
                    items,
                }),
            ) if *was == numbered => {
                items.push(words.to_owned());
            }
            (Some((numbered, words)), _) => {
                made.extend(open.take());
                open = Some(Block::List {
                    numbered,
                    items: vec![words.to_owned()],
                });
            }
            (None, Some(Block::Lines(lines))) => lines.push(line.to_owned()),
            (None, _) => {
                made.extend(open.take());
                open = Some(Block::Lines(vec![line.to_owned()]));
            }
        }
    }
    made.extend(open);
    made
}

/// Whether `line` is a list's item: whether it is numbered, and what it
/// says — a bulleted item without its bullet, a numbered one whole.
fn item(line: &str) -> Option<(bool, &str)> {
    for bullet in ["- ", "* ", "\u{2022} ", "\u{2013} "] {
        if let Some(words) = line.strip_prefix(bullet) {
            let words = words.trim_start();
            return (!words.is_empty()).then_some((false, words));
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    // A year, a time or a price begins with digits too; a number of up to
    // three, then `. ` or `) ` and words, is a list's.
    if (1..=3).contains(&digits) {
        let after = line.get(digits..)?;
        let words = after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))?;
        return (!words.trim().is_empty()).then_some((true, line));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(each: &[&str]) -> Block {
        Block::Lines(each.iter().map(|&line| line.to_owned()).collect())
    }

    fn list(numbered: bool, each: &[&str]) -> Block {
        Block::List {
            numbered,
            items: each.iter().map(|&item| item.to_owned()).collect(),
        }
    }

    #[test]
    fn one_line_is_one_paragraph_of_one_line() {
        assert_eq!(
            blocks("Opened the article."),
            vec![lines(&["Opened the article."])]
        );
        assert_eq!(blocks("  \n \n "), Vec::<Block>::new());
    }

    #[test]
    fn lines_are_kept_and_an_empty_line_ends_a_paragraph() {
        let said = "Monday\u{2013}Saturday: 8:00 am\u{2013}6:00 pm\nSunday: Closed\n\n\nPublic holidays: 10:00 am\u{2013}6:00 pm\r\n";
        assert_eq!(
            blocks(said),
            vec![
                lines(&[
                    "Monday\u{2013}Saturday: 8:00 am\u{2013}6:00 pm",
                    "Sunday: Closed"
                ]),
                lines(&["Public holidays: 10:00 am\u{2013}6:00 pm"]),
            ]
        );
    }

    #[test]
    fn bullets_are_a_list_and_the_words_around_them_paragraphs() {
        let said = "The museum is open:\n- Monday to Saturday, 8 to 6\n* Sunday, closed\n\u{2022} Holidays, 10 to 6\nTickets are 5,000 shillings.";
        assert_eq!(
            blocks(said),
            vec![
                lines(&["The museum is open:"]),
                list(
                    false,
                    &[
                        "Monday to Saturday, 8 to 6",
                        "Sunday, closed",
                        "Holidays, 10 to 6"
                    ]
                ),
                lines(&["Tickets are 5,000 shillings."]),
            ]
        );
    }

    #[test]
    fn a_numbered_list_keeps_its_numbers_and_is_not_a_bulleted_one() {
        let said = "3. Third\n4) Fourth\n- a bullet";
        assert_eq!(
            blocks(said),
            vec![
                list(true, &["3. Third", "4) Fourth"]),
                list(false, &["a bullet"])
            ]
        );
    }

    #[test]
    fn what_only_begins_like_an_item_is_a_line() {
        for line in [
            "2026. A year.",
            "8:00 am to 6:00 pm",
            "-5 degrees",
            "-",
            "1.",
            "1234. Too many digits",
            "1.5 km away",
            "*emphasis*",
        ] {
            assert_eq!(blocks(line), vec![lines(&[line])], "{line}");
        }
    }
}

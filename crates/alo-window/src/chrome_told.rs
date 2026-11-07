/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the Chromium alo carries tells of a card's page beside the DevTools
//! pipe (queue items 799 and 798), read.
//!
//! Chromium tells its embedder things the DevTools protocol does not carry:
//! the pointer's shape the page asks for (CEF's `OnCursorChange`), a
//! context menu it would open and what it is over (`RunContextMenu`), a
//! link a person asked to open in a tab of its own (`OnOpenURLFromTab`).
//! alo's CEF host writes each as a line to descriptor 5, which
//! [`crate::chrome`] moves to the host's error output and reads on a thread
//! of its own, only while the card is unseen (the host's `told.rs` says
//! the words). The machine's Chrome tells nothing: through it the pointer
//! stays the arrow, and a secondary or ⌘ press is nobody's.
//!
//! A line is a word, then fields after tabs, a field's backslash, tab, line
//! end and carriage return written `\\`, `\t`, `\n` and `\r`. The host is
//! alo's, but what it passes on — a link's address, the words a person
//! selected — a stranger's page chose, so every line is read as a
//! stranger's: bounded ([`LONGEST_LINE`]), and anything not as written
//! refused as nothing, never a panic.

/// The longest line read; a longer one is refused. The host cuts each
/// field at 2,048 characters, so a menu's line is at most about 16 KiB of
/// four-byte characters written twice.
pub const LONGEST_LINE: usize = 20 * 1024;

/// The pointer shapes a card's page may ask for, by their names in CSS:
/// what the host tells, and what the conversation's window sets.
pub const SHAPES: [&str; 35] = [
    "default",
    "crosshair",
    "pointer",
    "text",
    "wait",
    "help",
    "e-resize",
    "n-resize",
    "ne-resize",
    "nw-resize",
    "s-resize",
    "se-resize",
    "sw-resize",
    "w-resize",
    "ns-resize",
    "ew-resize",
    "nesw-resize",
    "nwse-resize",
    "col-resize",
    "row-resize",
    "all-scroll",
    "move",
    "vertical-text",
    "cell",
    "context-menu",
    "alias",
    "progress",
    "no-drop",
    "copy",
    "none",
    "not-allowed",
    "zoom-in",
    "zoom-out",
    "grab",
    "grabbing",
];

/// What the Chromium alo carries told of a card's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    /// The pointer's shape the page asks for, one of [`SHAPES`].
    Cursor(&'static str),
    /// A press of the secondary button the page did not take for itself.
    Menu(Menu),
    /// A link a person pressed with ⌘ or the middle button, which the page
    /// did not take for itself: its address, as Chromium spells it.
    Open(String),
}

/// A press of the secondary button on a card's page, as Chromium found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    /// Where, in the page's CSS pixels.
    pub at: (i32, i32),
    /// The link's address it was over, if any.
    pub link: Option<String>,
    /// The words selected on the page, if any.
    pub selection: Option<String>,
    /// Whether it was over a field a person types in.
    pub editable: bool,
}

/// `line`, without its end, read — or nothing, when it is not one the host
/// writes.
pub fn read(line: &str) -> Option<Told> {
    if line.len() > LONGEST_LINE {
        return None;
    }
    let mut fields = line.split('\t');
    let told = match fields.next()? {
        "cursor" => {
            let named = unescaped(fields.next()?)?;
            Told::Cursor(SHAPES.iter().copied().find(|shape| *shape == named)?)
        }
        "menu" => {
            let x = fields.next()?.parse().ok()?;
            let y = fields.next()?.parse().ok()?;
            let address = unescaped(fields.next()?)?;
            let selection = unescaped(fields.next()?)?;
            let editable = match fields.next()? {
                "1" => true,
                "0" => false,
                _ => return None,
            };
            let some = |text: String| Some(text).filter(|text| !text.trim().is_empty());
            Told::Menu(Menu {
                at: (x, y),
                link: some(address),
                selection: some(selection),
                editable,
            })
        }
        "open" => Told::Open(Some(unescaped(fields.next()?)?).filter(|at| !at.is_empty())?),
        _ => return None,
    };
    // Nothing after the last field.
    fields.next().is_none().then_some(told)
}

/// A field as written, its two-character escapes read back; nothing when
/// it holds a backslash that escapes nothing the host escapes.
fn unescaped(field: &str) -> Option<String> {
    let mut read = String::with_capacity(field.len());
    let mut characters = field.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            read.push(character);
            continue;
        }
        read.push(match characters.next()? {
            '\\' => '\\',
            't' => '\t',
            'n' => '\n',
            'r' => '\r',
            _ => return None,
        });
    }
    Some(read)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_a_menu_and_a_link_are_read() {
        assert_eq!(read("cursor\tpointer"), Some(Told::Cursor("pointer")));
        assert_eq!(read("cursor\ttext"), Some(Told::Cursor("text")));
        assert_eq!(
            read("menu\t40\t-3\thttps://example.com/a\t\t0"),
            Some(Told::Menu(Menu {
                at: (40, -3),
                link: Some("https://example.com/a".to_owned()),
                selection: None,
                editable: false,
            }))
        );
        assert_eq!(
            read("menu\t1\t2\t\ttwo\\nlines\\tand a \\\\\t1"),
            Some(Told::Menu(Menu {
                at: (1, 2),
                link: None,
                selection: Some("two\nlines\tand a \\".to_owned()),
                editable: true,
            }))
        );
        assert_eq!(
            read("open\thttps://example.com/b"),
            Some(Told::Open("https://example.com/b".to_owned()))
        );
    }

    #[test]
    fn what_the_host_does_not_write_is_refused_as_nothing() {
        for line in [
            "",
            "cursor",
            "cursor\tsparkles",
            "cursor\tpointer\textra",
            "menu\t1\t2\t\t\t2",
            "menu\tone\t2\t\t\t0",
            "menu\t1\t2\t\t\t0\tmore",
            "menu\t1\t2\ta\\q\t\t0",
            "menu\t1\t2\ta\\\t\t0",
            "open",
            "open\t",
            "close\tx",
        ] {
            assert_eq!(read(line), None, "{line:?}");
        }
        let long = format!("open\t{}", "a".repeat(LONGEST_LINE));
        assert_eq!(read(&long), None, "a line past the longest");
    }

    #[test]
    fn every_shape_named_is_one_css_names() {
        // The host's names: each CSS keyword of `cursor` but `auto`, which
        // a page asks for and Chromium decides.
        assert_eq!(SHAPES.len(), 35);
        assert!(!SHAPES.contains(&"auto"));
        for shape in SHAPES {
            assert_eq!(read(&format!("cursor\t{shape}")), Some(Told::Cursor(shape)));
        }
    }
}

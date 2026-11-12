/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's words as a person reads them, answered as plain text with no
//! position (queue item 964, ADR 0112 requirement 7: *read page text*).
//!
//! `page_text` is answered from the same tree `read` answers
//! ([`crate::serving`]), so what is read is what a reader of the tree
//! would read, in the order the tree holds it — a person's reading order.
//! No rectangle crosses (ADR 0002): the answer is lines of words.
//!
//! # What is read
//!
//! - **A run of text** is its words, and a node named by what is inside it
//!   — a heading, a link, a button, a list item, a row, a cell
//!   ([`alo_agent::name::names_itself_from_content`]) — is its name, which
//!   is those words: the tree reads the text inside such a node through,
//!   so its name is the one place they are. A list item's marker comes
//!   first: `• Milk`, `3. Third`.
//! - **A field** is its name — the words of its label, which the tree reads
//!   through too — and then what it holds; never what a field holds whose
//!   name or hint says it is a secret ([`crate::agent_page::is_secret`]),
//!   and never a password's dots.
//! - **Nothing hidden**: a node the author hid from readers (`aria-hidden`),
//!   and one not drawn at all — a closed list's options — is left out with
//!   everything inside it. Chrome leaves out of its tree what is not shown
//!   (`display: none`, `visibility: hidden`), and this engine has no box
//!   for it, so neither is ever read here.
//! - **Not read**: the document's own name, its title, which is not words
//!   on the page; a picture's name, which is not words drawn; and the name
//!   of anything else — a region, a list, a group — which labels what is
//!   inside it, and is not itself on the page.
//!
//! # Lines
//!
//! A heading, a paragraph, a list item, a row and their like are each a
//! line of their own, everything inside them on it; a region, a list, a
//! table and their like begin and end lines. Words not inside any of those
//! — text straight in a `<div>`, which neither tree keeps as a node — are
//! joined on one line while they were drawn on one, and begin another when
//! they were drawn below it; that is the one use of where a node was
//! drawn, and it never crosses. A line break inside the words themselves —
//! a `<pre>`'s — is kept. Lines with nothing on them are not answered.
//!
//! # How much
//!
//! At most [`MOST_CHARACTERS`] characters. A page's text longer than that
//! is answered to there — cut at the last line break, or else the last
//! space, before it, so no word is cut in two — and how many characters
//! were not answered is said, so a caller knows it was not handed the
//! whole page rather than believing it was.

use alo_box::{KnownRole, Role};
use alo_layout::Rect;
use alo_renderer::{Snapshot, SnapshotNode};

/// The most characters of a page's text one `page_text` answers: about
/// twenty-five thousand tokens, as much as alo's own agent is sent of a
/// tree ([`crate::agent_page::MOST_CHARACTERS`]).
pub const MOST_CHARACTERS: usize = 100_000;

/// A page's text, as answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageText {
    /// The words, a line of the page to a line, at most [`MOST_CHARACTERS`]
    /// characters, with no newline at the end.
    pub text: String,
    /// How many characters of the page's text were not answered: none
    /// unless it was cut.
    pub more: usize,
}

impl PageText {
    /// Whether the page's text was cut at [`MOST_CHARACTERS`].
    pub fn cut(&self) -> bool {
        self.more > 0
    }

    /// How many lines were answered.
    pub fn lines(&self) -> usize {
        self.text.lines().count()
    }
}

/// How a node's role places its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placed {
    /// A line of its own, with everything inside it on that line.
    Line,
    /// Begins and ends lines; what is inside it is placed by its own roles.
    Block,
    /// A break between lines, and no words.
    Break,
    /// On the line it is met on.
    Inline,
}

/// What is waiting to be read.
enum Step<'a> {
    /// A node, and whether it is inside a line.
    Node(&'a SnapshotNode, bool),
    /// The end of a line or a block.
    Break,
}

/// The text of `snapshot`, as a person reads it.
pub fn page_text(snapshot: &Snapshot) -> PageText {
    let mut lines = Lines::default();
    // Walked without recursing, with a stack of its own: how deep a tree
    // is is the read's to bound, never this thread's stack's.
    let mut waiting: Vec<Step<'_>> = snapshot
        .root
        .iter()
        .map(|root| Step::Node(root, false))
        .collect();
    while let Some(step) = waiting.pop() {
        let (node, in_line) = match step {
            Step::Break => {
                lines.end_line();
                continue;
            }
            Step::Node(node, in_line) => (node, in_line),
        };
        // Hidden from readers, or not drawn: nothing inside it is read.
        // A document — the page's, or a frame's — is read however it was drawn.
        let undrawn = node.rects.is_empty() && node.role != Role::Known(KnownRole::Document);
        if node.states.hidden || undrawn {
            continue;
        }
        let placed = placed(&node.role);
        let inside = match placed {
            Placed::Break => {
                lines.end_line();
                continue;
            }
            Placed::Line => {
                lines.end_line();
                waiting.push(Step::Break);
                if let Some(marker) = &node.marker {
                    lines.words(marker, &node.rects, true);
                }
                true
            }
            Placed::Block => {
                lines.end_line();
                waiting.push(Step::Break);
                false
            }
            Placed::Inline => in_line,
        };
        let name = node.name.as_deref().filter(|name| !name.trim().is_empty());
        if node.role == Role::Known(KnownRole::Text) {
            if let Some(words) = name.filter(|words| !crate::agent_page::is_dots(words)) {
                lines.words(words, &node.rects, inside);
            }
            continue;
        }
        if is_a_field(node) {
            if let Some(label) = name {
                lines.words(label, &node.rects, inside);
            }
            if crate::agent_page::is_secret(node) {
                continue;
            }
            // What it holds, on the line its label is on.
            waiting.extend(
                node.children
                    .iter()
                    .rev()
                    .map(|child| Step::Node(child, true)),
            );
            continue;
        }
        if let Some(words) = name
            && alo_agent::name::names_itself_from_content(&node.role)
        {
            // Its words are its name: the tree read them through.
            lines.words(words, &node.rects, inside);
            continue;
        }
        waiting.extend(
            node.children
                .iter()
                .rev()
                .map(|child| Step::Node(child, inside)),
        );
    }
    lines.end_line();
    bounded(&lines.done.join("\n"))
}

/// Whether `node` is a field: it takes text, or is set by a person.
fn is_a_field(node: &SnapshotNode) -> bool {
    node.states.takes_text
        || matches!(
            node.role,
            Role::Known(
                KnownRole::TextBox
                    | KnownRole::SearchBox
                    | KnownRole::ComboBox
                    | KnownRole::ListBox
                    | KnownRole::Slider
                    | KnownRole::SpinButton
                    | KnownRole::Date
                    | KnownRole::Meter
                    | KnownRole::ProgressBar
            )
        )
}

/// How `role` places its words.
fn placed(role: &Role) -> Placed {
    match role {
        Role::Known(known) => match known {
            KnownRole::Heading
            | KnownRole::Paragraph
            | KnownRole::ListItem
            | KnownRole::Row
            | KnownRole::Option
            | KnownRole::MenuItem
            | KnownRole::Tab
            | KnownRole::Summary => Placed::Line,
            KnownRole::Document
            | KnownRole::Banner
            | KnownRole::Navigation
            | KnownRole::Main
            | KnownRole::Complementary
            | KnownRole::ContentInfo
            | KnownRole::Region
            | KnownRole::Form
            | KnownRole::Search
            | KnownRole::Article
            | KnownRole::List
            | KnownRole::Group
            | KnownRole::Blockquote
            | KnownRole::Figure
            | KnownRole::Dialog
            | KnownRole::Status
            | KnownRole::Grid
            | KnownRole::RowGroup
            | KnownRole::Table
            | KnownRole::TabList
            | KnownRole::TabPanel
            | KnownRole::Menu
            | KnownRole::Details
            | KnownRole::ListBox
            | KnownRole::RadioGroup => Placed::Block,
            KnownRole::Separator => Placed::Break,
            _ => Placed::Inline,
        },
        // Roles Chrome names that this browser has no word of its own for,
        // spelled as Chrome spells them, and ARIA's that it has none for.
        Role::Declared(declared) => match declared.as_ref() {
            "term"
            | "definition"
            | "DescriptionListTerm"
            | "DescriptionListDetail"
            | "caption"
            | "Caption"
            | "pre"
            | "Pre"
            | "treeitem" => Placed::Line,
            "Iframe"
            | "IframePresentational"
            | "DescriptionList"
            | "Section"
            | "SectionWithoutName"
            | "HeaderAsNonLandmark"
            | "FooterAsNonLandmark"
            | "alert"
            | "alertdialog"
            | "toolbar"
            | "tree"
            | "treegrid"
            | "feed"
            | "log"
            | "note"
            | "menubar"
            | "tooltip"
            | "application" => Placed::Block,
            _ => Placed::Inline,
        },
        Role::Generic | Role::Presentational => Placed::Inline,
    }
}

/// The lines read so far.
#[derive(Default)]
struct Lines {
    /// Every line ended.
    done: Vec<String>,
    /// The line being read.
    line: String,
    /// Where the last words on it were drawn last.
    last: Option<Rect>,
}

impl Lines {
    /// `words`, drawn in `rects`, put on the line — `in_line` when they are
    /// inside a line, and otherwise on a line of their own when they were
    /// drawn below the words before them.
    fn words(&mut self, words: &str, rects: &[Rect], in_line: bool) {
        // A break inside the words is kept: a `<pre>`'s lines.
        for (at, part) in words.split('\n').enumerate() {
            if at > 0 {
                self.end_line();
            }
            let part = part.split_whitespace().collect::<Vec<_>>().join(" ");
            if part.is_empty() {
                continue;
            }
            if !in_line && !self.line.is_empty() && !on_one_line(self.last, rects.first().copied())
            {
                self.end_line();
            }
            if spaced(&self.line, &part) {
                self.line.push(' ');
            }
            self.line.push_str(&part);
        }
        self.last = rects.last().copied().or(self.last);
    }

    /// End the line being read, if anything is on it.
    fn end_line(&mut self) {
        if !self.line.is_empty() {
            self.done.push(core::mem::take(&mut self.line));
        }
        self.last = None;
    }
}

/// Whether words drawn at `next` were drawn on the line of those drawn
/// last at `last`: their rectangles share some height, or begin at the
/// same height. Either not drawn anywhere known is taken as one line.
fn on_one_line(last: Option<Rect>, next: Option<Rect>) -> bool {
    let (Some(last), Some(next)) = (last, next) else {
        return true;
    };
    (next.top() < last.bottom() && last.top() < next.bottom())
        || (next.top() - last.top()).abs() < 1.0
}

/// Whether a space goes between `line` and `next`: not at a line's start,
/// after an opening bracket or quote, or before a closing one or a mark
/// that ends a phrase.
fn spaced(line: &str, next: &str) -> bool {
    let Some(before) = line.chars().last() else {
        return false;
    };
    let opens = matches!(
        before,
        '(' | '[' | '{' | '\u{201c}' | '\u{2018}' | '\u{ab}' | '¿' | '¡'
    );
    let closes = next.chars().next().is_some_and(|after| {
        matches!(
            after,
            '.' | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | ')'
                | ']'
                | '}'
                | '%'
                | '\u{201d}'
                | '\u{2019}'
                | '\u{bb}'
                | '\u{2026}'
        )
    });
    !before.is_whitespace() && !opens && !closes
}

/// `text`, cut at [`MOST_CHARACTERS`] when it is longer.
fn bounded(text: &str) -> PageText {
    let total = text.chars().count();
    if total <= MOST_CHARACTERS {
        return PageText {
            text: text.to_owned(),
            more: 0,
        };
    }
    let end = text
        .char_indices()
        .nth(MOST_CHARACTERS)
        .map_or(text.len(), |(at, _)| at);
    let taken = text.get(..end).unwrap_or(text);
    // Back to the last line break, or else the last space: no word cut.
    let at = taken
        .rfind('\n')
        .or_else(|| taken.rfind(' '))
        .filter(|at| *at > 0)
        .unwrap_or(taken.len());
    let answered = taken.get(..at).unwrap_or(taken).trim_end().to_owned();
    let more = total - answered.chars().count();
    PageText {
        text: answered,
        more,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::{BoxId, States};

    fn node(
        id: usize,
        role: Role,
        name: Option<&str>,
        children: Vec<SnapshotNode>,
    ) -> SnapshotNode {
        drawn(id, role, name, Rect::new(0.0, 0.0, 100.0, 20.0), children)
    }

    fn drawn(
        id: usize,
        role: Role,
        name: Option<&str>,
        at: Rect,
        children: Vec<SnapshotNode>,
    ) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role,
            name: name.map(str::to_owned),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: at,
            rects: vec![at],
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    fn known(role: KnownRole) -> Role {
        Role::Known(role)
    }

    fn text(id: usize, words: &str) -> SnapshotNode {
        node(id, known(KnownRole::Text), Some(words), Vec::new())
    }

    fn page(children: Vec<SnapshotNode>) -> Snapshot {
        Snapshot {
            root: Some(node(
                1,
                known(KnownRole::Document),
                Some("The title"),
                children,
            )),
        }
    }

    #[test]
    fn headings_paragraphs_and_list_items_are_lines_of_their_own_in_reading_order() {
        let item = |id, marker: &str, name: &str| {
            let mut item = node(id, known(KnownRole::ListItem), Some(name), Vec::new());
            item.marker = Some(marker.to_owned());
            item
        };
        let snapshot = page(vec![
            node(2, known(KnownRole::Heading), Some("Invoices"), Vec::new()),
            node(
                3,
                known(KnownRole::Paragraph),
                None,
                vec![
                    text(4, "Three are"),
                    node(5, known(KnownRole::Link), Some("unpaid"), Vec::new()),
                    text(6, "."),
                ],
            ),
            node(
                7,
                known(KnownRole::List),
                Some("Unpaid invoices"),
                vec![item(8, "1.", "Invoice 3"), item(9, "2.", "Invoice 4")],
            ),
        ]);
        let read = page_text(&snapshot);
        // The title, the list's own name: neither is words on the page.
        assert_eq!(
            read.text,
            "Invoices\nThree are unpaid.\n1. Invoice 3\n2. Invoice 4"
        );
        assert_eq!(read.lines(), 4);
        assert!(!read.cut());
    }

    #[test]
    fn nothing_hidden_or_undrawn_is_read() {
        let mut hidden = node(
            3,
            known(KnownRole::Paragraph),
            None,
            vec![text(4, "Hidden")],
        );
        hidden.states.hidden = true;
        let mut list = node(
            5,
            known(KnownRole::ComboBox),
            Some("Size"),
            vec![node(6, known(KnownRole::Option), Some("Large"), Vec::new())],
        );
        list.children[0].rects.clear();
        let snapshot = page(vec![
            node(2, known(KnownRole::Paragraph), None, vec![text(7, "Shown")]),
            hidden,
            list,
            node(8, known(KnownRole::Image), Some("A logo"), Vec::new()),
        ]);
        assert_eq!(page_text(&snapshot).text, "Shown\nSize");
    }

    #[test]
    fn a_field_is_its_label_and_what_it_holds_and_never_a_secret() {
        let mut name = node(
            2,
            known(KnownRole::TextBox),
            Some("Customer name:"),
            vec![text(3, "Ada")],
        );
        name.states.takes_text = true;
        let mut password = node(
            4,
            known(KnownRole::TextBox),
            Some("Password"),
            vec![text(5, "hunter2")],
        );
        password.states.takes_text = true;
        let mut dotted = node(
            6,
            known(KnownRole::TextBox),
            Some("Code"),
            vec![text(7, "••••")],
        );
        dotted.states.takes_text = true;
        let snapshot = page(vec![
            node(8, known(KnownRole::Paragraph), None, vec![name]),
            node(9, known(KnownRole::Paragraph), None, vec![password]),
            node(10, known(KnownRole::Paragraph), None, vec![dotted]),
        ]);
        let read = page_text(&snapshot).text;
        assert_eq!(read, "Customer name: Ada\nPassword\nCode");
        assert!(!read.contains("hunter2") && !read.contains('•'));
    }

    #[test]
    fn words_in_no_line_are_joined_while_drawn_on_one_line_and_parted_below() {
        // Two `<div>`s, which neither tree keeps: the first's words in two
        // runs drawn side by side, the second's below.
        let snapshot = page(vec![
            drawn(
                2,
                known(KnownRole::Text),
                Some("Hello"),
                Rect::new(0.0, 0.0, 40.0, 18.0),
                Vec::new(),
            ),
            drawn(
                3,
                known(KnownRole::Link),
                Some("there"),
                Rect::new(44.0, 1.0, 40.0, 16.0),
                Vec::new(),
            ),
            drawn(
                4,
                known(KnownRole::Text),
                Some("Below"),
                Rect::new(0.0, 20.0, 40.0, 18.0),
                Vec::new(),
            ),
        ]);
        assert_eq!(page_text(&snapshot).text, "Hello there\nBelow");
    }

    #[test]
    fn a_break_inside_the_words_is_kept_and_a_separator_ends_a_line() {
        let snapshot = page(vec![
            node(
                2,
                Role::Declared("pre".into()),
                None,
                vec![text(3, "one\n  two")],
            ),
            text(4, "after"),
            node(5, known(KnownRole::Separator), None, Vec::new()),
            text(6, "past the rule"),
        ]);
        assert_eq!(page_text(&snapshot).text, "one\ntwo\nafter\npast the rule");
    }

    #[test]
    fn a_page_with_nothing_in_it_has_no_text() {
        assert_eq!(page_text(&Snapshot::default()), PageText::default());
        assert_eq!(page_text(&page(Vec::new())), PageText::default());
    }

    #[test]
    fn past_the_most_characters_the_text_is_cut_between_lines_and_how_much_more_said() {
        let line = "word ".repeat(19) + "word";
        let lines = MOST_CHARACTERS / (line.chars().count() + 1) + 10;
        let snapshot = page(
            (0..lines)
                .map(|at| {
                    node(
                        at + 2,
                        known(KnownRole::Paragraph),
                        None,
                        vec![text(at + 100_000, &line)],
                    )
                })
                .collect(),
        );
        let read = page_text(&snapshot);
        assert!(read.cut());
        assert!(read.text.chars().count() <= MOST_CHARACTERS);
        assert!(
            read.text.lines().all(|each| each == line),
            "a line cut in two"
        );
        let whole = lines * (line.chars().count() + 1) - 1;
        assert_eq!(read.text.chars().count() + read.more, whole);
    }

    #[test]
    fn one_line_longer_than_the_most_is_cut_at_a_space() {
        let words = "a".repeat(9) + " ";
        let long = words.repeat(MOST_CHARACTERS / 10 + 5);
        let read = bounded(long.trim_end());
        assert!(read.cut());
        assert!(read.text.chars().count() <= MOST_CHARACTERS);
        assert!(read.text.ends_with("aaaaaaaaa"), "a word cut in two");
        assert_eq!(
            read.text.chars().count() + read.more,
            long.trim_end().chars().count()
        );
    }
}

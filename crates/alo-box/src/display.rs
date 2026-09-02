//! `display`, and what it decides.
//!
//! Three separate questions live in this one property, and keeping them
//! separate is what makes the box tree simple: does this element make a box at
//! all, does the box sit in a line or on its own, and how are its children
//! arranged inside it.
//!
//! The modern two-value syntax says exactly that — `inline flow-root` is
//! "sits in a line, lays its children out as blocks" — so it is what this
//! parses into, and the single keywords are the shorthands for it that they
//! actually are.
//!
//! **No table values.** `docs/features.md` puts CSS-table layout in stage 3.
//! `display: table` is refused rather than approximated, and the box falls back
//! to the initial value with the refusal recorded, because a table laid out as
//! a block looks like a rendering bug and would be reported as one.

use core::fmt;

/// How a box sits among its siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Outside {
    /// Takes a line of its own.
    #[default]
    Block,
    /// Sits in a line with the text around it.
    Inline,
}

/// How a box arranges what is inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Inside {
    /// One after another, in lines — ordinary text and blocks.
    #[default]
    Flow,
    /// The same, but establishing its own formatting context, so nothing
    /// inside it interacts with anything outside.
    FlowRoot,
    /// Flexbox.
    Flex,
    /// Grid.
    Grid,
}

/// What `display` says about an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// `display: none` — no box, and no boxes for anything inside it either.
    None,
    /// `display: contents` — no box of its own, but its children still make
    /// theirs and take its place.
    Contents,
    /// A box.
    Box {
        /// How it sits among its siblings.
        outside: Outside,
        /// How it arranges its children.
        inside: Inside,
        /// Whether it is a list item. Stage 1 draws no marker for one — that
        /// is `::marker`, and no pseudo-element is produced — so this changes
        /// nothing about the box today and is recorded so that it can.
        list_item: bool,
    },
}

impl Display {
    /// The initial value of `display`, which is what an element gets when
    /// nothing sets it and what an unparseable value falls back to.
    pub const INITIAL: Display = Display::Box {
        outside: Outside::Inline,
        inside: Inside::Flow,
        list_item: false,
    };

    /// `display` for the value as written, or [`None`] if this engine does not
    /// implement it.
    ///
    /// Returning [`None`] rather than guessing is the point: the caller records
    /// the refusal and falls back to the initial value, which is what CSS does
    /// with a value it cannot parse.
    pub fn parse(value: &str) -> Option<Self> {
        let mut outside = None;
        let mut inside = None;
        let mut list_item = false;
        let words = value.split_ascii_whitespace().count();

        // `none` and `contents` say everything on their own; beside anything
        // else the value is nonsense rather than a value with an extra word.
        if words == 1 {
            let word = value.trim();
            if word.eq_ignore_ascii_case("none") {
                return Some(Display::None);
            }
            if word.eq_ignore_ascii_case("contents") {
                return Some(Display::Contents);
            }
        }

        for word in value.split_ascii_whitespace() {
            match () {
                () if word.eq_ignore_ascii_case("block") => set(&mut outside, Outside::Block)?,
                () if word.eq_ignore_ascii_case("inline") => set(&mut outside, Outside::Inline)?,
                () if word.eq_ignore_ascii_case("flow") => set(&mut inside, Inside::Flow)?,
                () if word.eq_ignore_ascii_case("flow-root") => {
                    set(&mut inside, Inside::FlowRoot)?;
                }
                () if word.eq_ignore_ascii_case("flex") => set(&mut inside, Inside::Flex)?,
                () if word.eq_ignore_ascii_case("grid") => set(&mut inside, Inside::Grid)?,
                () if word.eq_ignore_ascii_case("list-item") => {
                    if list_item {
                        return None;
                    }
                    list_item = true;
                }
                // The single-keyword shorthands that are not simply one half.
                () if word.eq_ignore_ascii_case("inline-block") => {
                    set(&mut outside, Outside::Inline)?;
                    set(&mut inside, Inside::FlowRoot)?;
                }
                () if word.eq_ignore_ascii_case("inline-flex") => {
                    set(&mut outside, Outside::Inline)?;
                    set(&mut inside, Inside::Flex)?;
                }
                () if word.eq_ignore_ascii_case("inline-grid") => {
                    set(&mut outside, Outside::Inline)?;
                    set(&mut inside, Inside::Grid)?;
                }
                // Everything else, which is the legacy this engine refuses:
                // every `table-*` value, `ruby`, `-webkit-box`, and anything
                // misspelled. All of them are somebody's mistake or stage 3's
                // problem, and neither is answered by a guess.
                () => return None,
            }
        }

        if words == 0 {
            return None;
        }
        // `display: list-item` on its own is a block that would have a marker.
        Some(Display::Box {
            outside: outside.unwrap_or(Outside::Block),
            inside: inside.unwrap_or(Inside::Flow),
            list_item,
        })
    }

    /// Whether this element makes a box of its own.
    pub fn generates_a_box(self) -> bool {
        matches!(self, Display::Box { .. })
    }

    /// How the box sits among its siblings, if it makes one.
    pub fn outside(self) -> Option<Outside> {
        match self {
            Display::Box { outside, .. } => Some(outside),
            Display::None | Display::Contents => None,
        }
    }

    /// How the box arranges its children, if it makes one.
    pub fn inside(self) -> Option<Inside> {
        match self {
            Display::Box { inside, .. } => Some(inside),
            Display::None | Display::Contents => None,
        }
    }

    /// Whether the box sits in a line with the text around it.
    pub fn is_inline_level(self) -> bool {
        self.outside() == Some(Outside::Inline)
    }

    /// Whether the box takes a line of its own.
    pub fn is_block_level(self) -> bool {
        self.outside() == Some(Outside::Block)
    }

    /// Whether children of this box are laid out one after another in lines,
    /// rather than by flex or grid.
    ///
    /// This is the question anonymous boxes depend on: only a flow container
    /// wraps runs of inline children, because in flex and grid every child is
    /// an item in its own right.
    pub fn lays_children_out_in_flow(self) -> bool {
        matches!(self.inside(), Some(Inside::Flow | Inside::FlowRoot))
    }
}

impl Default for Display {
    fn default() -> Self {
        Display::INITIAL
    }
}

impl fmt::Display for Display {
    /// Writes the two-value form, which says what the value means rather than
    /// what it was written as.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Display::None => f.write_str("none"),
            Display::Contents => f.write_str("contents"),
            Display::Box {
                outside,
                inside,
                list_item,
            } => {
                let outside = match outside {
                    Outside::Block => "block",
                    Outside::Inline => "inline",
                };
                let inside = match inside {
                    Inside::Flow => "flow",
                    Inside::FlowRoot => "flow-root",
                    Inside::Flex => "flex",
                    Inside::Grid => "grid",
                };
                write!(f, "{outside} {inside}")?;
                if *list_item {
                    f.write_str(" list-item")?;
                }
                Ok(())
            }
        }
    }
}

/// Set a half of the value, refusing a value that sets it twice.
fn set<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    if slot.is_some() {
        return None;
    }
    *slot = Some(value);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(value: &str) -> String {
        Display::parse(value).map_or_else(|| "refused".to_owned(), |display| display.to_string())
    }

    #[test]
    fn a_single_keyword_is_the_shorthand_it_actually_is() {
        assert_eq!(parsed("block"), "block flow");
        assert_eq!(parsed("inline"), "inline flow");
        assert_eq!(parsed("flow-root"), "block flow-root");
        assert_eq!(parsed("inline-block"), "inline flow-root");
        assert_eq!(parsed("flex"), "block flex");
        assert_eq!(parsed("inline-flex"), "inline flex");
        assert_eq!(parsed("grid"), "block grid");
        assert_eq!(parsed("inline-grid"), "inline grid");
    }

    #[test]
    fn the_two_value_syntax_says_the_same_things() {
        assert_eq!(parsed("block flow"), "block flow");
        assert_eq!(parsed("inline flow-root"), "inline flow-root");
        assert_eq!(parsed("block flex"), "block flex");
        assert_eq!(parsed("flex block"), "block flex", "either order");
    }

    #[test]
    fn a_missing_half_takes_its_default() {
        assert_eq!(parsed("flow"), "block flow");
        assert_eq!(parsed("inline"), "inline flow");
    }

    #[test]
    fn none_and_contents_generate_no_box_and_stand_alone() {
        assert_eq!(Display::parse("none"), Some(Display::None));
        assert_eq!(Display::parse("contents"), Some(Display::Contents));
        assert_eq!(parsed("none block"), "refused");
        assert_eq!(parsed("inline contents"), "refused");
        assert!(!Display::None.generates_a_box());
        assert!(!Display::Contents.generates_a_box());
        assert_eq!(Display::None.outside(), None);
        assert_eq!(Display::Contents.inside(), None);
    }

    #[test]
    fn a_list_item_is_a_block_that_would_have_had_a_marker() {
        assert_eq!(parsed("list-item"), "block flow list-item");
        assert_eq!(parsed("inline list-item"), "inline flow list-item");
        assert_eq!(parsed("block flow list-item"), "block flow list-item");
    }

    #[test]
    fn the_legacy_this_engine_refuses_is_refused_rather_than_approximated() {
        for value in [
            "table",
            "inline-table",
            "table-row",
            "table-cell",
            "table-row-group",
            "ruby",
            "-webkit-box",
            "blck",
            "",
            "   ",
        ] {
            assert_eq!(parsed(value), "refused", "{value:?} should be refused");
        }
    }

    #[test]
    fn a_value_that_says_the_same_thing_twice_is_nonsense() {
        assert_eq!(parsed("block block"), "refused");
        assert_eq!(parsed("block inline"), "refused");
        assert_eq!(parsed("flex grid"), "refused");
        assert_eq!(parsed("list-item list-item"), "refused");
    }

    #[test]
    fn a_value_is_matched_however_it_is_capitalised() {
        assert_eq!(parsed("BLOCK"), "block flow");
        assert_eq!(parsed("Inline-Flex"), "inline flex");
    }

    #[test]
    fn the_initial_value_is_inline_flow() {
        assert_eq!(Display::INITIAL.to_string(), "inline flow");
        assert_eq!(Display::default(), Display::INITIAL);
        assert!(Display::INITIAL.is_inline_level());
        assert!(!Display::INITIAL.is_block_level());
    }

    #[test]
    fn only_a_flow_container_lays_its_children_out_in_lines() {
        let flow = Display::parse("block").expect("block");
        let flex = Display::parse("flex").expect("flex");
        let grid = Display::parse("grid").expect("grid");
        assert!(flow.lays_children_out_in_flow());
        assert!(
            Display::parse("inline-block")
                .expect("inline-block")
                .lays_children_out_in_flow(),
        );
        assert!(!flex.lays_children_out_in_flow());
        assert!(!grid.lays_children_out_in_flow());
        assert!(!Display::None.lays_children_out_in_flow());
    }
}

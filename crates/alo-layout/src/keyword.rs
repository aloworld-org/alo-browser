//! The keyword properties layout reads.
//!
//! One file for all of them because they are one kind of thing — a small
//! closed set of words, matched without regard to case, refused when unknown —
//! and because a keyword parsed slightly differently in two places is a bug
//! that only shows up on the property nobody tested.
//!
//! **Refused, not guessed.** Every one of these returns [`None`] for a value
//! this engine does not implement, and the caller records it and falls back to
//! the initial value. That is what CSS does, and it is what keeps `float: left`
//! from quietly becoming something that looks nearly right.

use core::fmt;

/// Define a keyword enum, its spellings, and the tests that keep them honest.
macro_rules! keywords {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$variant_meta:meta])* $variant:ident => $text:literal ),+ $(,)? }
        default = $default:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            /// Every value this engine has, in the order they are written here.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The value a keyword spells, or [`None`] if this engine does not
            /// have it.
            pub fn parse(text: &str) -> Option<Self> {
                let text = text.trim();
                $(
                    if text.eq_ignore_ascii_case($text) {
                        return Some($name::$variant);
                    }
                )+
                None
            }

            /// The keyword, as CSS writes it.
            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        #[cfg(test)]
        impl $name {
            /// Whether the derived `Default` is the value CSS calls initial.
            /// Checked by a test for every keyword type, because a wrong
            /// default is a layout that looks deliberate and is not.
            pub(crate) fn default_is_the_initial_value() -> bool {
                Self::default() == $name::$default
            }
        }
    };
}

keywords! {
    /// `position`.
    ///
    /// `sticky` is stage 2 (`docs/features.md`) and `fixed` needs a viewport,
    /// which layout has but paint owns; neither is here, and a sheet that asks
    /// for one is told.
    Positioning {
        /// In the flow, and offsets do nothing.
        #[default]
        Static => "static",
        /// In the flow, and offsets move it without moving anything else.
        Relative => "relative",
        /// Out of the flow, placed against its nearest positioned ancestor.
        Absolute => "absolute",
    }
    default = Static
}

keywords! {
    /// `overflow-x` and `overflow-y`.
    Overflow {
        /// Content may spill out and is drawn.
        #[default]
        Visible => "visible",
        /// Content is cut off at the box.
        Clip => "clip",
        /// Cut off, and scrollable by an agent even though no bar is drawn.
        Hidden => "hidden",
        /// Cut off, scrollable, with room kept for a scrollbar.
        Scroll => "scroll",
        /// Room kept for a scrollbar only if one is needed.
        Auto => "auto",
    }
    default = Visible
}

keywords! {
    /// `box-sizing`.
    BoxSizing {
        /// `width` is the content, and padding and border are added to it.
        #[default]
        ContentBox => "content-box",
        /// `width` includes the padding and the border.
        BorderBox => "border-box",
    }
    default = ContentBox
}

keywords! {
    /// `flex-direction`.
    FlexDirection {
        /// Along the inline axis.
        #[default]
        Row => "row",
        /// Along the inline axis, backwards.
        RowReverse => "row-reverse",
        /// Down the block axis.
        Column => "column",
        /// Up the block axis.
        ColumnReverse => "column-reverse",
    }
    default = Row
}

keywords! {
    /// `flex-wrap`.
    FlexWrap {
        /// One line, however tight.
        #[default]
        NoWrap => "nowrap",
        /// As many lines as it takes.
        Wrap => "wrap",
        /// As many lines as it takes, filled from the far end.
        WrapReverse => "wrap-reverse",
    }
    default = NoWrap
}

keywords! {
    /// `align-items`, `align-self`, `justify-items` and `justify-self`.
    ///
    /// One enum for all four because CSS box alignment gives them one set of
    /// values, and four copies of it would be four places to disagree.
    Alignment {
        /// Whatever the container says, or `stretch` if it says nothing.
        #[default]
        Normal => "normal",
        /// At the start of the axis.
        Start => "start",
        /// At the end.
        End => "end",
        /// At the start of the writing direction.
        FlexStart => "flex-start",
        /// At the end of the writing direction.
        FlexEnd => "flex-end",
        /// In the middle.
        Center => "center",
        /// Lined up on the text baseline.
        Baseline => "baseline",
        /// Filling the axis.
        Stretch => "stretch",
    }
    default = Normal
}

keywords! {
    /// `justify-content` and `align-content`: how the leftover space is shared.
    Distribution {
        /// Whatever the container says.
        #[default]
        Normal => "normal",
        /// All at the start.
        Start => "start",
        /// All at the end.
        End => "end",
        /// All at the start of the writing direction.
        FlexStart => "flex-start",
        /// All at the end of the writing direction.
        FlexEnd => "flex-end",
        /// Split evenly around the middle.
        Center => "center",
        /// Between the items, none at the ends.
        SpaceBetween => "space-between",
        /// Half as much at the ends as between.
        SpaceAround => "space-around",
        /// The same everywhere.
        SpaceEvenly => "space-evenly",
        /// Grown to fill.
        Stretch => "stretch",
    }
    default = Normal
}

keywords! {
    /// `grid-auto-flow`.
    GridAutoFlow {
        /// Fill rows, leaving holes.
        #[default]
        Row => "row",
        /// Fill columns, leaving holes.
        Column => "column",
        /// Fill rows, going back to fill holes.
        RowDense => "row dense",
        /// Fill columns, going back to fill holes.
        ColumnDense => "column dense",
    }
    default = Row
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every keyword type is checked the same way, because every one of them
    /// can go wrong the same way.
    macro_rules! check {
        ($name:ident) => {{
            for value in $name::ALL {
                assert_eq!(
                    $name::parse(value.as_str()),
                    Some(*value),
                    "{} should round trip",
                    value.as_str(),
                );
                assert_eq!(value.to_string(), value.as_str());
                assert_eq!(
                    $name::parse(&value.as_str().to_ascii_uppercase()),
                    Some(*value),
                    "and be matched however it is capitalised",
                );
                assert_eq!(
                    $name::parse(&format!("  {}  ", value.as_str())),
                    Some(*value),
                    "and with space around it",
                );
            }
            assert!(
                $name::default_is_the_initial_value(),
                "the derived default must be the value CSS calls initial",
            );
            for nonsense in ["", "   ", "banana", "left"] {
                assert_eq!($name::parse(nonsense), None, "{nonsense}");
            }
        }};
    }

    #[test]
    fn every_keyword_round_trips_and_refuses_what_it_does_not_have() {
        check!(Positioning);
        check!(Overflow);
        check!(BoxSizing);
        check!(FlexDirection);
        check!(FlexWrap);
        check!(Alignment);
        check!(Distribution);
        check!(GridAutoFlow);
    }

    #[test]
    fn what_this_engine_does_not_implement_is_refused_by_name() {
        // `docs/features.md` puts `position: sticky` in stage 2, and floats in
        // stage 3. Neither is a value here, so a sheet asking for one is told.
        assert_eq!(Positioning::parse("sticky"), None);
        assert_eq!(Positioning::parse("fixed"), None);
        assert_eq!(
            Distribution::parse("space-between "),
            Some(Distribution::SpaceBetween)
        );
    }

    #[test]
    fn the_dense_grid_flows_are_two_words_and_are_read_as_written() {
        assert_eq!(
            GridAutoFlow::parse("row dense"),
            Some(GridAutoFlow::RowDense)
        );
        assert_eq!(
            GridAutoFlow::parse("column dense"),
            Some(GridAutoFlow::ColumnDense),
        );
        assert_eq!(
            GridAutoFlow::parse("dense row"),
            None,
            "the other order is not a value CSS has",
        );
    }
}

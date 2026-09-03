//! What layout reads from a computed style.
//!
//! One struct, filled once per box, in this engine's own vocabulary. It exists
//! so that the file which talks to `taffy` is a translation and nothing else —
//! and so that replacing `taffy`, which ADR 0001 says is a judgement call taken
//! to get us laying out sooner, means rewriting one file rather than every
//! reader of every property.
//!
//! **A value this engine cannot read is recorded and then ignored**, and the
//! property falls back to its initial value. That is what CSS does, and the
//! recording is what makes a wrong layout diagnosable rather than mysterious.

use crate::keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning,
};
use crate::placement::GridPlacement;
use crate::sizing::{AutoLength, Sizing};
use crate::track::TrackList;
use alo_css::{IssueKind, Location, StyleIssue};
use alo_style::ComputedStyle;
use alo_value::{FontMetrics, LengthPercentage, parse_length_percentage, parse_number};

/// Everything layout needs to know about one box.
#[derive(Debug, Clone, Default)]
pub struct LayoutStyle {
    /// The font in force, for turning a length into a number.
    pub metrics: FontMetrics,
    /// `position`.
    pub position: Positioning,
    /// `box-sizing`.
    pub box_sizing: BoxSizing,
    /// `top`, `right`, `bottom`, `left`.
    pub inset: SideValues<AutoLength>,
    /// `width` and `height`.
    pub size: AxisValues<Sizing>,
    /// `min-width` and `min-height`.
    pub min_size: AxisValues<Sizing>,
    /// `max-width` and `max-height`.
    pub max_size: AxisValues<Sizing>,
    /// `margin`.
    pub margin: SideValues<AutoLength>,
    /// `padding`.
    pub padding: SideValues<LengthPercentage>,
    /// `border-width`.
    pub border: SideValues<LengthPercentage>,
    /// `overflow-x` and `overflow-y`.
    pub overflow: AxisValues<Overflow>,
    /// `column-gap` and `row-gap`.
    pub gap: AxisValues<LengthPercentage>,
    /// `aspect-ratio`.
    pub aspect_ratio: Option<f32>,
    /// The flexbox properties.
    pub flex: FlexStyle,
    /// The alignment properties, which flex and grid share.
    pub align: AlignStyle,
    /// The grid properties.
    pub grid: GridStyle,
}

/// A value on each of the four sides.
#[derive(Debug, Clone, Default)]
pub struct SideValues<T> {
    /// The top.
    pub top: T,
    /// The right.
    pub right: T,
    /// The bottom.
    pub bottom: T,
    /// The left.
    pub left: T,
}

/// A value on each axis.
#[derive(Debug, Clone, Default)]
pub struct AxisValues<T> {
    /// Across.
    pub horizontal: T,
    /// Down.
    pub vertical: T,
}

/// The flexbox properties.
#[derive(Debug, Clone, Default)]
pub struct FlexStyle {
    /// `flex-direction`.
    pub direction: FlexDirection,
    /// `flex-wrap`.
    pub wrap: FlexWrap,
    /// `flex-grow`.
    pub grow: f32,
    /// `flex-shrink`, which is one rather than zero when nothing says
    /// otherwise — the only one of these three whose initial value is not the
    /// obvious one.
    pub shrink: f32,
    /// `flex-basis`.
    pub basis: Sizing,
}

/// The alignment properties, shared by flex and grid.
#[derive(Debug, Clone, Default)]
pub struct AlignStyle {
    /// `align-items`.
    pub align_items: Alignment,
    /// `align-self`.
    pub align_self: Alignment,
    /// `justify-items`.
    pub justify_items: Alignment,
    /// `justify-self`.
    pub justify_self: Alignment,
    /// `align-content`.
    pub align_content: Distribution,
    /// `justify-content`.
    pub justify_content: Distribution,
}

/// The grid properties.
#[derive(Debug, Clone, Default)]
pub struct GridStyle {
    /// `grid-template-rows`.
    pub template_rows: TrackList,
    /// `grid-template-columns`.
    pub template_columns: TrackList,
    /// `grid-auto-rows`.
    pub auto_rows: TrackList,
    /// `grid-auto-columns`.
    pub auto_columns: TrackList,
    /// `grid-auto-flow`.
    pub auto_flow: GridAutoFlow,
    /// `grid-row`.
    pub row: GridPlacement,
    /// `grid-column`.
    pub column: GridPlacement,
}

/// Read everything layout needs from a computed style.
///
/// `issues` collects every value this engine could not read, with the property
/// it was on.
pub fn read(style: &ComputedStyle, issues: &mut Vec<StyleIssue>) -> LayoutStyle {
    let mut reader = Reader { style, issues };
    LayoutStyle {
        metrics: style.metrics(),
        position: reader.keyword("position"),
        box_sizing: reader.keyword("box-sizing"),
        // `top`, `right`, `bottom` and `left` start at `auto`, which is the
        // one place in this struct where `auto` is the initial value rather
        // than zero. Said here rather than derived, because deriving it once
        // put `auto` on every margin in the document.
        inset: SideValues {
            top: reader.value_or_default("top", AutoLength::Auto),
            right: reader.value_or_default("right", AutoLength::Auto),
            bottom: reader.value_or_default("bottom", AutoLength::Auto),
            left: reader.value_or_default("left", AutoLength::Auto),
        },
        size: AxisValues {
            horizontal: reader.value("width"),
            vertical: reader.value("height"),
        },
        min_size: AxisValues {
            horizontal: reader.value("min-width"),
            vertical: reader.value("min-height"),
        },
        max_size: AxisValues {
            horizontal: reader.value("max-width"),
            vertical: reader.value("max-height"),
        },
        margin: SideValues {
            top: reader.shorthand_side("margin", "margin-top", 0),
            right: reader.shorthand_side("margin", "margin-right", 1),
            bottom: reader.shorthand_side("margin", "margin-bottom", 2),
            left: reader.shorthand_side("margin", "margin-left", 3),
        },
        padding: SideValues {
            top: reader.shorthand_side("padding", "padding-top", 0),
            right: reader.shorthand_side("padding", "padding-right", 1),
            bottom: reader.shorthand_side("padding", "padding-bottom", 2),
            left: reader.shorthand_side("padding", "padding-left", 3),
        },
        border: SideValues {
            top: reader.border_width("top"),
            right: reader.border_width("right"),
            bottom: reader.border_width("bottom"),
            left: reader.border_width("left"),
        },
        overflow: reader.overflow(),
        gap: reader.gap(),
        aspect_ratio: reader.aspect_ratio(),
        flex: FlexStyle {
            direction: reader.keyword("flex-direction"),
            wrap: reader.keyword("flex-wrap"),
            grow: reader.number("flex-grow").unwrap_or(0.0),
            // `flex-shrink` starts at one: a flex item shrinks unless it is
            // told not to, which is the opposite of how it grows.
            shrink: reader.number("flex-shrink").unwrap_or(1.0),
            basis: reader.value("flex-basis"),
        },
        align: AlignStyle {
            align_items: reader.keyword("align-items"),
            align_self: reader.keyword("align-self"),
            justify_items: reader.keyword("justify-items"),
            justify_self: reader.keyword("justify-self"),
            align_content: reader.keyword("align-content"),
            justify_content: reader.keyword("justify-content"),
        },
        grid: GridStyle {
            template_rows: reader.value("grid-template-rows"),
            template_columns: reader.value("grid-template-columns"),
            auto_rows: reader.value("grid-auto-rows"),
            auto_columns: reader.value("grid-auto-columns"),
            auto_flow: reader.keyword("grid-auto-flow"),
            row: reader.value("grid-row"),
            column: reader.value("grid-column"),
        },
    }
}

/// Something layout can read from a property's text.
pub trait FromValue: Sized {
    /// Read it, or [`None`] if this engine does not implement it.
    fn from_value(text: &str) -> Option<Self>;
}

impl FromValue for Sizing {
    fn from_value(text: &str) -> Option<Self> {
        Sizing::parse(text)
    }
}

impl FromValue for AutoLength {
    fn from_value(text: &str) -> Option<Self> {
        AutoLength::parse(text)
    }
}

impl FromValue for LengthPercentage {
    fn from_value(text: &str) -> Option<Self> {
        parse_length_percentage(text)
    }
}

impl FromValue for TrackList {
    fn from_value(text: &str) -> Option<Self> {
        TrackList::parse(text)
    }
}

impl FromValue for GridPlacement {
    fn from_value(text: &str) -> Option<Self> {
        GridPlacement::parse(text)
    }
}

struct Reader<'a> {
    style: &'a ComputedStyle,
    issues: &'a mut Vec<StyleIssue>,
}

impl Reader<'_> {
    /// Read a property, falling back to its initial value and recording the
    /// refusal when its text is something this engine does not implement.
    ///
    /// Every reader below is this one: absent means initial, unreadable means
    /// initial *and* recorded, and the difference between those two is the
    /// whole point.
    fn read<T>(&mut self, name: &str, initial: T, parse: impl Fn(&str) -> Option<T>) -> T {
        let Some(text) = self.style.get(name) else {
            return initial;
        };
        if let Some(value) = parse(text) {
            return value;
        }
        self.refuse(name, text);
        initial
    }

    /// A property whose value is a keyword.
    fn keyword<T: KeywordValue>(&mut self, name: &str) -> T {
        self.read(name, T::default(), T::parse_keyword)
    }

    /// A property whose value is something the value layer can read.
    fn value<T: FromValue + Default>(&mut self, name: &str) -> T {
        self.read(name, T::default(), T::from_value)
    }

    /// A property whose initial value is not `T::default()`.
    fn value_or_default<T: FromValue>(&mut self, name: &str, initial: T) -> T {
        self.read(name, initial, T::from_value)
    }

    /// A property that is a plain number.
    fn number(&mut self, name: &str) -> Option<f32> {
        let text = self.style.get(name)?;
        if let Some(number) = parse_number(text) {
            return Some(number);
        }
        self.refuse(name, text);
        None
    }

    /// One side of a box-model shorthand, preferring the longhand when both
    /// were written.
    ///
    /// Stage 1 does not expand shorthands in the cascade, so `margin: 4px 8px`
    /// arrives here whole. Splitting it on the side that is being asked for is
    /// what makes the common way of writing a margin work at all, and the
    /// longhand still wins because the cascade already decided it should.
    fn shorthand_side<T: FromValue + Default>(
        &mut self,
        shorthand: &str,
        longhand: &str,
        side: usize,
    ) -> T {
        if self.style.get(longhand).is_some() {
            return self.value(longhand);
        }
        let Some(text) = self.style.get(shorthand) else {
            return T::default();
        };
        let parts: Vec<&str> = text.split_ascii_whitespace().collect();
        // One value is every side; two are vertical and horizontal; three add a
        // separate bottom; four are top, right, bottom, left.
        // One value is every side; two are vertical then horizontal; three add
        // a separate bottom; four are top, right, bottom, left.
        let index = match (parts.len(), side) {
            (1, _) | (2, 0 | 2) | (3, 0) => 0,
            (2, _) | (3, 1 | 3) => 1,
            (3, _) => 2,
            (4, side) => side,
            _ => {
                self.refuse(shorthand, text);
                return T::default();
            }
        };
        let Some(part) = parts.get(index) else {
            return T::default();
        };
        if let Some(value) = T::from_value(part) {
            return value;
        }
        self.refuse(shorthand, text);
        T::default()
    }

    /// How thick one border is.
    ///
    /// Three places can say it, and CSS's own order decides: the longhand
    /// beats the per-side shorthand, which beats `border`. A style sheet
    /// writes `border: 1px solid` and then `border-bottom-width: 2px`, and
    /// both have to be read for that to mean what it says.
    ///
    /// A border with a style of `none` is no border however thick it was
    /// declared, which is why a width alone never draws one.
    fn border_width(&mut self, side: &str) -> LengthPercentage {
        for (property, whole) in [
            (format!("border-{side}-width"), false),
            (format!("border-{side}"), true),
            ("border".to_owned(), true),
        ] {
            let Some(text) = self.style.get(&property) else {
                continue;
            };
            let width = if whole {
                let border = alo_value::parse_border(text);
                if border.style.as_deref().is_some_and(is_invisible_style) {
                    return LengthPercentage::ZERO;
                }
                border.width
            } else {
                parse_length_percentage(text)
            };
            match width {
                Some(width) => return width,
                None if !whole => {
                    self.refuse(&property, text);
                    return LengthPercentage::ZERO;
                }
                None => {}
            }
        }
        LengthPercentage::ZERO
    }

    /// `overflow`, `overflow-x` and `overflow-y`.
    fn overflow(&mut self) -> AxisValues<Overflow> {
        let both: Option<Overflow> = self.style.get("overflow").and_then(Overflow::parse);
        AxisValues {
            horizontal: self
                .style
                .get("overflow-x")
                .and_then(Overflow::parse)
                .or(both)
                .unwrap_or_default(),
            vertical: self
                .style
                .get("overflow-y")
                .and_then(Overflow::parse)
                .or(both)
                .unwrap_or_default(),
        }
    }

    /// `gap`, `row-gap` and `column-gap`.
    fn gap(&mut self) -> AxisValues<LengthPercentage> {
        let shorthand: Vec<LengthPercentage> = self
            .style
            .get("gap")
            .map(|text| {
                text.split_ascii_whitespace()
                    .filter_map(parse_length_percentage)
                    .collect()
            })
            .unwrap_or_default();
        // `gap: 8px 16px` is row then column, which is the opposite order to
        // every other two-value shorthand in CSS.
        let row = shorthand.first().cloned();
        let column = shorthand.get(1).or_else(|| shorthand.first()).cloned();
        AxisValues {
            horizontal: self
                .value_or::<LengthPercentage>("column-gap", column)
                .unwrap_or(LengthPercentage::ZERO),
            vertical: self
                .value_or::<LengthPercentage>("row-gap", row)
                .unwrap_or(LengthPercentage::ZERO),
        }
    }

    fn value_or<T: FromValue>(&mut self, name: &str, fallback: Option<T>) -> Option<T> {
        let Some(text) = self.style.get(name) else {
            return fallback;
        };
        if let Some(value) = T::from_value(text) {
            return Some(value);
        }
        self.refuse(name, text);
        fallback
    }

    /// `aspect-ratio`, which may be written as `3 / 2` or as a plain number.
    fn aspect_ratio(&mut self) -> Option<f32> {
        let text = self.style.get("aspect-ratio")?;
        if alo_value::is_keyword(text, "auto") {
            return None;
        }
        let ratio = match text.split_once('/') {
            Some((width, height)) => {
                let height = parse_number(height)?;
                if height == 0.0 {
                    None
                } else {
                    Some(parse_number(width)? / height)
                }
            }
            None => parse_number(text),
        };
        match ratio {
            Some(ratio) if ratio.is_finite() && ratio > 0.0 => Some(ratio),
            _ => {
                self.refuse("aspect-ratio", text);
                None
            }
        }
    }

    fn refuse(&mut self, property: &str, value: &str) {
        self.issues.push(StyleIssue {
            kind: IssueKind::UnsupportedValue,
            source: format!("{property}: {value}"),
            at: Location { line: 0, column: 0 },
        });
    }
}

/// Whether a border style draws nothing whatever its width.
fn is_invisible_style(style: &str) -> bool {
    style.eq_ignore_ascii_case("none") || style.eq_ignore_ascii_case("hidden")
}

/// A keyword property, so that one reader serves all of them.
pub trait KeywordValue: Default {
    /// Read the keyword.
    fn parse_keyword(text: &str) -> Option<Self>;
}

macro_rules! keyword_value {
    ($($name:ident),+ $(,)?) => {
        $(
            impl KeywordValue for $name {
                fn parse_keyword(text: &str) -> Option<Self> {
                    $name::parse(text)
                }
            }
        )+
    };
}

keyword_value!(
    Positioning,
    BoxSizing,
    Overflow,
    FlexDirection,
    FlexWrap,
    Alignment,
    Distribution,
    GridAutoFlow,
);

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal to within far less than a pixel. Comparing floats exactly is
    /// fragile in a way that has nothing to do with what these tests are
    /// checking.
    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet, resolve};

    /// The layout style of the one element in a document, and what could not
    /// be read.
    fn read_style(css: &str) -> (LayoutStyle, Vec<StyleIssue>) {
        let document = parse_document("<html><body><div id=x>t</div></body></html>");
        let sheet = parse_stylesheet(&format!("#x {{ {css} }}"));
        let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
        let tree = resolve(&document, &sheets, &MediaContext::default());
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .expect("the div");
        let style = tree.get(id).expect("a style");
        let mut issues = Vec::new();
        let read = read(style, &mut issues);
        (read, issues)
    }

    fn px(value: &LengthPercentage) -> f32 {
        value.to_px(FontMetrics::default(), 0.0)
    }

    #[test]
    fn a_box_with_no_style_gets_every_initial_value() {
        let (style, issues) = read_style("");
        assert!(issues.is_empty());
        assert_eq!(style.position, Positioning::Static);
        assert_eq!(style.box_sizing, BoxSizing::ContentBox);
        assert_eq!(style.size.horizontal, Sizing::Auto);
        assert!(close(style.flex.grow, 0.0));
        assert!(
            close(style.flex.shrink, 1.0),
            "a flex item shrinks by default"
        );
        assert_eq!(style.overflow.horizontal, Overflow::Visible);
        assert!(style.aspect_ratio.is_none());
    }

    #[test]
    fn the_box_model_shorthands_are_split_the_way_css_splits_them() {
        let one = read_style("padding: 4px").0.padding;
        assert!(close(px(&one.top), 4.0));
        assert!(close(px(&one.left), 4.0));

        let two = read_style("padding: 4px 8px").0.padding;
        assert!(close(px(&two.top), 4.0) && close(px(&two.right), 8.0));
        assert!(close(px(&two.bottom), 4.0) && close(px(&two.left), 8.0));

        let three = read_style("padding: 1px 2px 3px").0.padding;
        assert_eq!(
            (
                px(&three.top),
                px(&three.right),
                px(&three.bottom),
                px(&three.left)
            ),
            (1.0, 2.0, 3.0, 2.0),
        );

        let four = read_style("padding: 1px 2px 3px 4px").0.padding;
        assert_eq!(
            (
                px(&four.top),
                px(&four.right),
                px(&four.bottom),
                px(&four.left)
            ),
            (1.0, 2.0, 3.0, 4.0),
        );
    }

    #[test]
    fn a_longhand_wins_over_the_shorthand_beside_it() {
        let padding = read_style("padding: 4px; padding-left: 20px").0.padding;
        assert!(close(px(&padding.top), 4.0));
        assert!(close(px(&padding.left), 20.0));
    }

    #[test]
    fn a_margin_of_auto_survives_the_shorthand() {
        let margin = read_style("margin: 0 auto").0.margin;
        assert_eq!(margin.top, AutoLength::Length(LengthPercentage::ZERO));
        assert!(matches!(margin.left, AutoLength::Auto));
        assert!(matches!(margin.right, AutoLength::Auto));
    }

    #[test]
    fn gap_is_row_then_column_which_is_the_other_way_round_from_everything_else() {
        let gap = read_style("gap: 8px 16px").0.gap;
        assert!(
            close(px(&gap.vertical), 8.0),
            "the first value is the row gap"
        );
        assert!(close(px(&gap.horizontal), 16.0));

        let both = read_style("gap: 12px").0.gap;
        assert!(close(px(&both.vertical), 12.0));
        assert!(close(px(&both.horizontal), 12.0));

        let split = read_style("gap: 8px; column-gap: 2px").0.gap;
        assert!(close(px(&split.vertical), 8.0));
        assert!(close(px(&split.horizontal), 2.0));
    }

    #[test]
    fn overflow_on_one_axis_or_both() {
        let both = read_style("overflow: hidden").0.overflow;
        assert_eq!(both.horizontal, Overflow::Hidden);
        assert_eq!(both.vertical, Overflow::Hidden);

        let split = read_style("overflow: hidden; overflow-y: scroll")
            .0
            .overflow;
        assert_eq!(split.horizontal, Overflow::Hidden);
        assert_eq!(split.vertical, Overflow::Scroll);
    }

    #[test]
    fn an_aspect_ratio_may_be_a_fraction_or_a_number() {
        assert_eq!(read_style("aspect-ratio: 3 / 2").0.aspect_ratio, Some(1.5));
        assert_eq!(read_style("aspect-ratio: 1.5").0.aspect_ratio, Some(1.5));
        assert_eq!(read_style("aspect-ratio: auto").0.aspect_ratio, None);

        let (style, issues) = read_style("aspect-ratio: 1 / 0");
        assert_eq!(style.aspect_ratio, None);
        assert_eq!(issues.len(), 1, "and dividing by nothing is recorded");
    }

    #[test]
    fn a_value_this_engine_cannot_read_is_recorded_and_then_ignored() {
        let (style, issues) = read_style("position: sticky; width: 50dvh; flex-grow: banana");
        assert_eq!(style.position, Positioning::Static, "back to the initial");
        assert_eq!(style.size.horizontal, Sizing::Auto);
        assert!(close(style.flex.grow, 0.0));
        assert_eq!(issues.len(), 3);
        assert!(
            issues
                .iter()
                .all(|issue| issue.kind == IssueKind::UnsupportedValue)
        );
        assert!(issues[0].source.contains("sticky"));
    }

    #[test]
    fn the_grid_properties_are_read_whole() {
        let (style, issues) = read_style(
            "grid-template-columns: repeat(3, 1fr); grid-auto-flow: column dense; \
             grid-column: 1 / span 2",
        );
        assert!(issues.is_empty());
        assert_eq!(style.grid.template_columns.to_string(), "repeat(3, 1fr)");
        assert_eq!(style.grid.auto_flow, GridAutoFlow::ColumnDense);
        assert_eq!(style.grid.column.to_string(), "1 / span 2");
        assert!(style.grid.template_rows.is_empty());
    }

    #[test]
    fn the_font_in_force_comes_with_the_style_so_lengths_can_be_numbers() {
        let (style, _) = read_style("font-size: 20px; width: 2em");
        assert!(close(style.metrics.font_size, 20.0));
        match &style.size.horizontal {
            Sizing::Length(value) => assert!(close(value.to_px(style.metrics, 0.0), 40.0)),
            other => panic!("expected a length, got {other}"),
        }
    }
}

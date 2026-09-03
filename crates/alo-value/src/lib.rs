//! Values with numbers in them.
//!
//! The cascade produces **specified values as text** — `16px` is four
//! characters — because a computed style holds what an author wrote and
//! deciding what it means belongs with the code that knows which unit each
//! property wants. This is that code, in one place, so that layout and paint
//! ask the same question and get the same answer.
//!
//! # What is here
//!
//! - [`Length`] and [`Unit`]: every unit CSS has that does not need a window
//!   to answer. `vw` and `vh` are absent on purpose — they are relative to a
//!   viewport, and a viewport belongs to layout.
//! - [`LengthPercentage`]: a length, a percentage, or a `calc()`. A percentage
//!   is carried rather than resolved, because `50%` is half of *something* and
//!   which something depends on the property and the containing block.
//! - [`CalcNode`]: an expression, type-checked once when it is parsed and
//!   evaluated whenever a caller has a font and a basis. `calc(1px + 2)` is
//!   refused at parse time rather than producing three of something.
//! - [`FontMetrics`]: what `em` and `rem` are relative to. This is the reason
//!   this layer could not come before the cascade — until the cascade has run
//!   there is no font size to be relative to.
//!
//! # What is not here
//!
//! **Colours.** They are queue item 14, because they block paint rather than
//! layout: a layout pass has never needed to know what colour anything is.
//!
//! **Approximation.** A value this engine cannot read is refused, and the
//! caller falls back to the property's initial value — which is what CSS does
//! with a value it cannot parse. A guessed length is a wrong pixel, and law 3
//! says a wrong pixel is a bug rather than a task.

pub mod calc;
pub mod color;
pub mod gradient;
pub mod length;
pub mod parse;
pub mod shadow;
pub mod shorthand;
pub mod transform;
pub mod unit;

pub use calc::{CalcNode, Kind};
pub use color::{Color, Rgba, from_hsl};
pub use gradient::{Angle, Gradient, Stop};
pub use length::{FontMetrics, Length, LengthPercentage, Viewport};
pub use parse::{
    is_keyword, parse_box_shadows, parse_color, parse_gradient, parse_length,
    parse_length_percentage, parse_number, parse_text_shadows, parse_transform,
    parse_transform_origin,
};
pub use shadow::{DrawnShadow, Shadow};
pub use shorthand::{Border, parse_border};
pub use transform::{Function, Matrix, Transform};
pub use unit::Unit;

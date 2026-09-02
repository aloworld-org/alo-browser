//! `calc()`, as an expression that is checked once and evaluated later.
//!
//! `calc(var(--gap) * 2)` is what alo's design system is built out of, so this
//! is not an optional corner: the substitution happens in the cascade and what
//! arrives here is `calc(8px * 2)`, which still has to become `16`.
//!
//! # Why it is a tree rather than a number
//!
//! `calc(50% - 1em)` cannot be a number until somebody says what the percentage
//! is of and what the font size is, and those two are known in different places
//! — layout and the cascade. So the expression is parsed and **type-checked
//! once**, and evaluated whenever a caller has both. An engine that folded it
//! early would have to guess one of them.
//!
//! # What is checked
//!
//! CSS's calc has a type system, and the useful half of it is small: you may
//! add two lengths but not a length and a number; you may multiply a length by
//! a number but not by another length; you may divide by a number and by
//! nothing else. `calc(1px + 2)` is refused at parse time rather than
//! producing three of something.

use crate::length::{FontMetrics, Length};
use core::fmt;

/// What an expression works out to be — not its value, its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A plain number, with no unit.
    Number,
    /// A length, a percentage, or a sum that includes one.
    Length,
}

/// One node of a `calc()` expression.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcNode {
    /// A length.
    Length(Length),
    /// A percentage, as written: `50%` is `50.0`.
    Percentage(f32),
    /// A plain number.
    Number(f32),
    /// Everything added together.
    Sum(Vec<CalcNode>),
    /// Everything multiplied together.
    Product(Vec<CalcNode>),
    /// The negative of what is inside.
    Negate(Box<CalcNode>),
    /// One divided by what is inside. Only ever wrapped around a number, which
    /// [`CalcNode::kind`] is what enforces.
    Invert(Box<CalcNode>),
}

impl CalcNode {
    /// What this expression works out to be, or [`None`] if it is not a
    /// well-formed expression at all.
    ///
    /// This is the whole of the type checking, and it runs once, when the value
    /// is parsed. Evaluating an expression that passed it cannot fail.
    pub fn kind(&self) -> Option<Kind> {
        match self {
            CalcNode::Length(_) | CalcNode::Percentage(_) => Some(Kind::Length),
            CalcNode::Number(_) => Some(Kind::Number),
            CalcNode::Negate(inner) => inner.kind(),
            // One over a length is a per-length, which CSS has no use for.
            CalcNode::Invert(inner) => match inner.kind()? {
                Kind::Number => Some(Kind::Number),
                Kind::Length => None,
            },
            CalcNode::Sum(terms) => {
                let mut kind = None;
                for term in terms {
                    let term_kind = term.kind()?;
                    match kind {
                        None => kind = Some(term_kind),
                        // Adding a length to a number is the mistake this
                        // check exists to catch.
                        Some(held) if held != term_kind => return None,
                        Some(_) => {}
                    }
                }
                kind
            }
            CalcNode::Product(factors) => {
                let mut lengths = 0;
                for factor in factors {
                    match factor.kind()? {
                        Kind::Number => {}
                        // Two lengths multiplied would be an area.
                        Kind::Length => lengths += 1,
                    }
                }
                match lengths {
                    0 => Some(Kind::Number),
                    1 => Some(Kind::Length),
                    _ => None,
                }
            }
        }
    }

    /// The expression's value, in CSS pixels for a length and as itself for a
    /// number.
    ///
    /// `basis` is what a percentage is a percentage of; it is ignored by an
    /// expression that has none.
    pub fn evaluate(&self, metrics: FontMetrics, basis: f32) -> f32 {
        match self {
            CalcNode::Length(length) => length.to_px(metrics),
            CalcNode::Percentage(percent) => percent / 100.0 * basis,
            CalcNode::Number(number) => *number,
            CalcNode::Negate(inner) => -inner.evaluate(metrics, basis),
            CalcNode::Invert(inner) => {
                let divisor = inner.evaluate(metrics, basis);
                // Division by zero in CSS makes the whole expression invalid.
                // It cannot be refused here — the check already passed — so it
                // becomes zero, which is what an invalid length falls back to.
                if divisor == 0.0 { 0.0 } else { 1.0 / divisor }
            }
            CalcNode::Sum(terms) => terms.iter().map(|term| term.evaluate(metrics, basis)).sum(),
            CalcNode::Product(factors) => factors
                .iter()
                .map(|factor| factor.evaluate(metrics, basis))
                .product(),
        }
    }

    /// Whether any part of this expression is a percentage, and so needs a
    /// basis before it means anything.
    pub fn has_percentage(&self) -> bool {
        match self {
            CalcNode::Percentage(_) => true,
            CalcNode::Length(_) | CalcNode::Number(_) => false,
            CalcNode::Negate(inner) | CalcNode::Invert(inner) => inner.has_percentage(),
            CalcNode::Sum(nodes) | CalcNode::Product(nodes) => {
                nodes.iter().any(CalcNode::has_percentage)
            }
        }
    }
}

impl fmt::Display for CalcNode {
    /// Writes the expression back out, fully parenthesised. It is for
    /// diagnostics, not for round-tripping the author's spacing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcNode::Length(length) => write!(f, "{length}"),
            CalcNode::Percentage(percent) => write!(f, "{percent}%"),
            CalcNode::Number(number) => write!(f, "{number}"),
            CalcNode::Negate(inner) => write!(f, "-{inner}"),
            CalcNode::Invert(inner) => write!(f, "1/{inner}"),
            CalcNode::Sum(terms) => write_joined(f, terms, " + "),
            CalcNode::Product(factors) => write_joined(f, factors, " * "),
        }
    }
}

fn write_joined(f: &mut fmt::Formatter<'_>, nodes: &[CalcNode], separator: &str) -> fmt::Result {
    f.write_str("(")?;
    for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
            f.write_str(separator)?;
        }
        write!(f, "{node}")?;
    }
    f.write_str(")")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    fn px(value: f32) -> CalcNode {
        CalcNode::Length(Length::px(value))
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    #[test]
    fn a_sum_of_lengths_is_a_length() {
        let node = CalcNode::Sum(vec![px(8.0), px(4.0)]);
        assert_eq!(node.kind(), Some(Kind::Length));
        assert!(close(node.evaluate(FontMetrics::default(), 0.0), 12.0));
    }

    #[test]
    fn a_length_times_a_number_is_a_length() {
        let node = CalcNode::Product(vec![px(8.0), CalcNode::Number(2.0)]);
        assert_eq!(node.kind(), Some(Kind::Length));
        assert!(close(node.evaluate(FontMetrics::default(), 0.0), 16.0));
    }

    #[test]
    fn adding_a_length_to_a_number_is_refused() {
        let node = CalcNode::Sum(vec![px(1.0), CalcNode::Number(2.0)]);
        assert_eq!(
            node.kind(),
            None,
            "three of what? — the check exists to ask that",
        );
    }

    #[test]
    fn multiplying_two_lengths_is_refused_because_an_area_is_not_a_length() {
        let node = CalcNode::Product(vec![px(2.0), px(3.0)]);
        assert_eq!(node.kind(), None);
    }

    #[test]
    fn dividing_by_a_length_is_refused_and_dividing_by_a_number_is_not() {
        let by_length = CalcNode::Product(vec![px(8.0), CalcNode::Invert(Box::new(px(2.0)))]);
        assert_eq!(by_length.kind(), None);

        let by_number = CalcNode::Product(vec![
            px(8.0),
            CalcNode::Invert(Box::new(CalcNode::Number(2.0))),
        ]);
        assert_eq!(by_number.kind(), Some(Kind::Length));
        assert!(close(by_number.evaluate(FontMetrics::default(), 0.0), 4.0));
    }

    #[test]
    fn dividing_by_zero_becomes_zero_rather_than_an_infinity() {
        let node = CalcNode::Product(vec![
            px(8.0),
            CalcNode::Invert(Box::new(CalcNode::Number(0.0))),
        ]);
        assert!(close(node.evaluate(FontMetrics::default(), 0.0), 0.0));
    }

    #[test]
    fn a_percentage_may_be_added_to_a_length_and_needs_a_basis() {
        let node = CalcNode::Sum(vec![
            CalcNode::Percentage(50.0),
            CalcNode::Negate(Box::new(px(10.0))),
        ]);
        assert_eq!(node.kind(), Some(Kind::Length));
        assert!(node.has_percentage());
        assert!(close(node.evaluate(FontMetrics::default(), 400.0), 190.0));
        assert!(close(node.evaluate(FontMetrics::default(), 200.0), 90.0));
    }

    #[test]
    fn a_font_relative_length_inside_a_calc_uses_the_font_in_force() {
        let node = CalcNode::Product(vec![
            CalcNode::Length(Length {
                value: 2.0,
                unit: Unit::Em,
            }),
            CalcNode::Number(1.5),
        ]);
        let metrics = FontMetrics::estimated(20.0, 16.0);
        assert!(close(node.evaluate(metrics, 0.0), 60.0));
    }

    #[test]
    fn an_expression_without_a_percentage_says_so() {
        assert!(!CalcNode::Sum(vec![px(1.0), px(2.0)]).has_percentage());
        assert!(!px(1.0).has_percentage());
        assert!(CalcNode::Negate(Box::new(CalcNode::Percentage(1.0))).has_percentage());
    }

    #[test]
    fn a_sum_of_numbers_is_a_number() {
        let node = CalcNode::Sum(vec![CalcNode::Number(1.0), CalcNode::Number(2.5)]);
        assert_eq!(node.kind(), Some(Kind::Number));
        assert!(close(node.evaluate(FontMetrics::default(), 0.0), 3.5));
    }

    #[test]
    fn an_expression_writes_itself_back_out_for_a_diagnostic() {
        let node = CalcNode::Sum(vec![
            CalcNode::Percentage(50.0),
            CalcNode::Negate(Box::new(px(10.0))),
        ]);
        assert_eq!(node.to_string(), "(50% + -10px)");
    }

    #[test]
    fn an_empty_sum_has_no_kind_rather_than_a_guessed_one() {
        assert_eq!(CalcNode::Sum(Vec::new()).kind(), None);
        assert_eq!(
            CalcNode::Product(Vec::new()).kind(),
            Some(Kind::Number),
            "an empty product is one, which is a number",
        );
    }
}

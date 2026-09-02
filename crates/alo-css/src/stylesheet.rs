//! A style sheet: the rules we hold, and the order they were written in.
//!
//! Order is not decoration. When two rules of equal specificity set the same
//! property, the later one wins, so a structure that reordered rules would
//! cascade wrongly — which is why this is a list and not a map.

use crate::declaration::DeclarationBlock;
use crate::issue::{Location, StyleIssue};
use crate::media::{MediaContext, MediaQueryList};
use crate::selector::SelectorList;
use core::fmt;

/// A rule with a selector list and a block of declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
    /// The selectors the rule applies to.
    pub selectors: SelectorList,
    /// What it sets.
    pub declarations: DeclarationBlock,
    /// Where the rule was written.
    pub at: Location,
}

impl fmt::Display for StyleRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {{ {} }}", self.selectors, self.declarations)
    }
}

/// An `@media` rule and the rules inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaRule {
    /// The condition under which the rules inside apply.
    pub queries: MediaQueryList,
    /// The rules inside, which may themselves be `@media` rules.
    pub rules: Vec<Rule>,
    /// Where the rule was written.
    pub at: Location,
}

impl fmt::Display for MediaRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@media {} {{ ", self.queries)?;
        for (index, rule) in self.rules.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{rule}")?;
        }
        f.write_str(" }")
    }
}

/// An at-rule this engine does not implement, kept whole.
///
/// Keeping it is `docs/features.md`'s rule for unknown properties applied to
/// at-rules: a later stage should be able to implement `@supports` or
/// `@keyframes` without re-parsing every style sheet, and it cannot do that
/// with something that was thrown away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownAtRule {
    /// The rule's name, without the `@`, lowercased.
    pub name: Box<str>,
    /// Everything between the name and the block or semicolon, as written.
    pub prelude: String,
    /// The block, as written, without its braces. [`None`] for a rule that
    /// ended in a semicolon.
    pub block: Option<String>,
    /// Where the rule was written.
    pub at: Location,
}

impl fmt::Display for UnknownAtRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.name)?;
        if !self.prelude.is_empty() {
            write!(f, " {}", self.prelude)?;
        }
        match &self.block {
            Some(block) => write!(f, " {{{block}}}"),
            None => f.write_str(";"),
        }
    }
}

/// One rule in a style sheet.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    /// A selector and its declarations.
    Style(StyleRule),
    /// An `@media` rule.
    Media(MediaRule),
    /// An at-rule we do not implement, kept as written.
    Unknown(UnknownAtRule),
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::Style(rule) => write!(f, "{rule}"),
            Rule::Media(rule) => write!(f, "{rule}"),
            Rule::Unknown(rule) => write!(f, "{rule}"),
        }
    }
}

/// A parsed style sheet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stylesheet {
    rules: Vec<Rule>,
    issues: Vec<StyleIssue>,
}

impl Stylesheet {
    /// A style sheet with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// The rules, in the order they were written, `@media` rules unflattened.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Everything the sheet asked for that this engine did not do.
    pub fn issues(&self) -> &[StyleIssue] {
        &self.issues
    }

    /// Every style rule that applies to a device, in document order, with the
    /// rules inside matching `@media` blocks flattened in where they were
    /// written.
    ///
    /// A rule inside an `@media` block that does not match is not here, and a
    /// rule inside one this engine could not understand is not here either —
    /// an unknown condition is treated as not matching, so a rule that might
    /// have been for a dark theme never leaks into a light one.
    pub fn style_rules_for(&self, context: &MediaContext) -> Vec<&StyleRule> {
        let mut out = Vec::new();
        collect_style_rules(&self.rules, *context, &mut out);
        out
    }

    pub(crate) fn from_parts(rules: Vec<Rule>, issues: Vec<StyleIssue>) -> Self {
        Self { rules, issues }
    }
}

/// A `MediaContext` is two words and `Copy`, so the recursion takes one rather
/// than threading a reference through itself.
fn collect_style_rules<'a>(rules: &'a [Rule], context: MediaContext, out: &mut Vec<&'a StyleRule>) {
    for rule in rules {
        match rule {
            Rule::Style(rule) => out.push(rule),
            Rule::Media(media) => {
                if media.queries.matches(&context) {
                    collect_style_rules(&media.rules, context, out);
                }
            }
            Rule::Unknown(_) => {}
        }
    }
}

impl fmt::Display for Stylesheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, rule) in self.rules.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{rule}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ColorScheme;
    use crate::parse::parse_stylesheet;

    #[test]
    fn rules_keep_the_order_they_were_written_in() {
        let sheet = parse_stylesheet("a { color: red } b { color: blue } a { color: green }");
        let rules = sheet.style_rules_for(&MediaContext::default());
        assert_eq!(rules.len(), 3);
        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.selectors.to_string())
                .collect::<Vec<_>>(),
            vec!["a", "b", "a"],
        );
    }

    #[test]
    fn a_media_rule_contributes_only_when_its_condition_holds() {
        let sheet = parse_stylesheet(
            "a { color: red } @media (prefers-color-scheme: dark) { a { color: white } }",
        );
        assert_eq!(sheet.rules().len(), 2);

        let light = sheet.style_rules_for(&MediaContext::new(800.0, ColorScheme::Light));
        assert_eq!(light.len(), 1);

        let dark = sheet.style_rules_for(&MediaContext::new(800.0, ColorScheme::Dark));
        assert_eq!(dark.len(), 2);
        assert_eq!(dark[1].declarations.to_string(), "color: white;");
    }

    #[test]
    fn a_media_rule_inside_a_media_rule_needs_both_conditions() {
        let sheet = parse_stylesheet(
            "@media (min-width: 600px) { @media (prefers-color-scheme: dark) { a { color: white } } }",
        );
        assert!(
            sheet
                .style_rules_for(&MediaContext::new(400.0, ColorScheme::Dark))
                .is_empty(),
        );
        assert!(
            sheet
                .style_rules_for(&MediaContext::new(800.0, ColorScheme::Light))
                .is_empty(),
        );
        assert_eq!(
            sheet
                .style_rules_for(&MediaContext::new(800.0, ColorScheme::Dark))
                .len(),
            1,
        );
    }

    #[test]
    fn an_unknown_media_condition_keeps_its_rules_out_rather_than_guessing() {
        let sheet = parse_stylesheet("@media (orientation: landscape) { a { color: red } }");
        assert_eq!(sheet.rules().len(), 1, "the rule is kept");
        assert!(
            sheet.style_rules_for(&MediaContext::default()).is_empty(),
            "and it never applies",
        );
        assert_eq!(sheet.issues().len(), 1);
    }

    #[test]
    fn an_unknown_at_rule_contributes_nothing_and_is_still_there() {
        let sheet = parse_stylesheet("@supports (display: grid) { a { color: red } }");
        assert_eq!(sheet.rules().len(), 1);
        assert!(matches!(sheet.rules()[0], Rule::Unknown(_)));
        assert!(sheet.style_rules_for(&MediaContext::default()).is_empty());
    }

    #[test]
    fn an_empty_sheet_writes_as_nothing() {
        assert_eq!(Stylesheet::new().to_string(), "");
        assert!(Stylesheet::new().rules().is_empty());
        assert!(Stylesheet::new().issues().is_empty());
    }
}

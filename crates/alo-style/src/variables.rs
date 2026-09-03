/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Custom properties, and what `var()` resolves to.
//!
//! `alo-workplace`'s design system is custom properties throughout, so an
//! engine that cannot resolve them renders nothing of alo at all. That is why
//! `docs/decisions/0001` calls this stage 1's first hard requirement rather
//! than decoration, and why it is here rather than deferred.
//!
//! Two things this does that a naive substitution does not:
//!
//! - **A cycle is refused rather than looped.** `--a: var(--b)` and
//!   `--b: var(--a)` make every property in the ring invalid, which is what
//!   CSS says and is the only answer that terminates.
//! - **Substitution is textual, over the token stream.** The value between the
//!   `var()` calls comes through exactly as written, so
//!   `calc(var(--gap) * 2)` becomes `calc(8px * 2)` and not a re-serialised
//!   approximation of it. A value this engine does not yet understand is one
//!   it can still pass along intact.

use alo_css::{IssueKind, Location, StyleIssue};
use cssparser::{Parser as CssParser, ParserInput, Token};
use std::collections::{BTreeMap, BTreeSet};

/// The custom properties in force on one element, resolved.
///
/// Ordered by name so that iterating is the same twice, which matters for
/// tests and for anything that writes a style out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Variables {
    resolved: BTreeMap<Box<str>, String>,
}

impl Variables {
    /// No custom properties at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of a custom property, by its full name including the dashes.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.resolved.get(name).map(String::as_str)
    }

    /// Every custom property in force, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.resolved
            .iter()
            .map(|(name, value)| (&**name, &**value))
    }

    /// How many are in force.
    pub fn len(&self) -> usize {
        self.resolved.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.resolved.is_empty()
    }

    fn set(&mut self, name: &str, value: String) {
        self.resolved.insert(name.into(), value);
    }
}

/// What happened when a value was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// The value, with every `var()` replaced.
    Value(String),
    /// The value named something that is not set and gave no fallback. CSS
    /// calls this invalid at computed-value time, and the declaration is
    /// treated as though it had said `unset`.
    InvalidAtComputedValueTime,
}

/// Resolve the custom properties an element declares, on top of the ones it
/// inherits.
///
/// `declared` is what this element's own rules set, in cascade order — later
/// entries win, which is the caller's job to arrange. Each declared value may
/// use `var()`, including of another property declared on the same element,
/// so this resolves them in dependency order rather than in written order.
///
/// A property in a cycle is dropped entirely, taking neither its own value nor
/// the inherited one, because CSS says it becomes invalid rather than falling
/// back.
pub fn resolve_variables(
    inherited: &Variables,
    declared: &[(Box<str>, String)],
    at: Location,
    issues: &mut Vec<StyleIssue>,
) -> Variables {
    let sources: BTreeMap<&str, &str> = declared
        .iter()
        .map(|(name, value)| (&**name, &**value))
        .collect();

    let mut out = inherited.clone();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut in_a_cycle: BTreeSet<Box<str>> = BTreeSet::new();

    for name in sources.keys() {
        let mut visiting = Vec::new();
        resolve_one(
            name,
            &sources,
            inherited,
            &mut out,
            &mut done,
            &mut visiting,
            &mut in_a_cycle,
        );
    }

    for name in in_a_cycle {
        out.resolved.remove(&name);
        issues.push(StyleIssue {
            kind: IssueKind::VariableCycle,
            source: name.to_string(),
            at,
        });
    }
    out
}

/// Resolve one declared custom property, resolving anything it depends on
/// first. Returns without changing anything if the property is in a cycle.
fn resolve_one<'a>(
    name: &'a str,
    sources: &BTreeMap<&'a str, &'a str>,
    inherited: &Variables,
    out: &mut Variables,
    done: &mut BTreeSet<&'a str>,
    visiting: &mut Vec<&'a str>,
    in_a_cycle: &mut BTreeSet<Box<str>>,
) {
    if done.contains(name) {
        return;
    }
    if visiting.contains(&name) {
        // Everything from where this name first appeared to here is the ring.
        let start = visiting.iter().position(|held| *held == name).unwrap_or(0);
        for member in visiting.iter().skip(start) {
            in_a_cycle.insert((*member).into());
        }
        return;
    }
    let Some(source) = sources.get(name).copied() else {
        return;
    };

    visiting.push(name);
    for referenced in referenced_variables(source) {
        if let Some((dependency, _)) = sources.get_key_value(&*referenced) {
            resolve_one(
                dependency, sources, inherited, out, done, visiting, in_a_cycle,
            );
        }
    }
    visiting.pop();

    // A property that turned out to be in a ring is refused rather than
    // resolved: resolving it would bake in whichever half of the ring happened
    // to be looked at first.
    if in_a_cycle.contains(name) {
        return;
    }
    done.insert(name);
    match substitute(source, out) {
        Resolved::Value(value) => out.set(name, value),
        // Invalid at computed-value time. The inherited value, if any, is what
        // an element without a valid declaration has, so it stays.
        Resolved::InvalidAtComputedValueTime => {
            if inherited.get(name).is_none() {
                out.resolved.remove(name);
            }
        }
    }
}

/// Replace every `var()` in a value with what it names.
///
/// The text between substitutions is passed through exactly as written, so a
/// value this engine does not understand still arrives at the next stage
/// intact.
pub fn substitute(value: &str, variables: &Variables) -> Resolved {
    if !value.contains("var(")
        && !value.contains("VAR(")
        && !value.to_ascii_lowercase().contains("var(")
    {
        // The common case, and worth taking: most declarations use no
        // variables at all and re-tokenising them would be pure cost.
        return Resolved::Value(value.to_owned());
    }
    let mut out = String::with_capacity(value.len());
    let mut input = ParserInput::new(value);
    let mut parser = CssParser::new(&mut input);
    if rewrite(&mut parser, value, variables, &mut out, 0) {
        Resolved::Value(out)
    } else {
        Resolved::InvalidAtComputedValueTime
    }
}

/// How deep `var()` inside `var()` inside `var()` may go.
///
/// Cycles are already refused by name, so this catches only the case a cycle
/// check cannot: a fallback chain that is finite but absurd. Chromium uses a
/// similar limit for the same reason.
const MAX_SUBSTITUTION_DEPTH: u8 = 32;

/// Walk a value, writing it back out with every `var()` replaced. Returns
/// whether the value survived.
fn rewrite(
    input: &mut CssParser<'_, '_>,
    source: &str,
    variables: &Variables,
    out: &mut String,
    depth: u8,
) -> bool {
    if depth > MAX_SUBSTITUTION_DEPTH {
        return false;
    }
    let mut cursor = input.position().byte_index();
    loop {
        // Whitespace and comments come through as tokens, not skipped past:
        // the parser's position before a call is the token's own start only if
        // nothing is skipped, and that is what makes the text between
        // substitutions come out exactly as it was written.
        let before = input.state().position().byte_index();
        let Ok(token) = input.next_including_whitespace_and_comments() else {
            break;
        };
        let token = token.clone();
        let after_token = input.position().byte_index();

        match token {
            Token::Function(ref name) if name.eq_ignore_ascii_case("var") => {
                push_span(out, source, cursor, before);
                let mut replacement = String::new();
                let survived = input
                    .parse_nested_block(
                        |arguments| -> Result<bool, cssparser::ParseError<'_, ()>> {
                            Ok(resolve_var(
                                arguments,
                                source,
                                variables,
                                &mut replacement,
                                depth,
                            ))
                        },
                    )
                    .unwrap_or(false);
                if !survived {
                    return false;
                }
                out.push_str(&replacement);
                cursor = input.position().byte_index();
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                // Everything up to and including the opening delimiter, then
                // the block's own contents, then the delimiter that closes it.
                push_span(out, source, cursor, after_token);
                let closing = closing_delimiter(&token);
                let survived = input
                    .parse_nested_block(|inner| -> Result<bool, cssparser::ParseError<'_, ()>> {
                        Ok(rewrite(inner, source, variables, out, depth + 1))
                    })
                    .unwrap_or(false);
                if !survived {
                    return false;
                }
                out.push(closing);
                cursor = input.position().byte_index();
            }
            _ => {}
        }
    }
    push_span(out, source, cursor, input.position().byte_index());
    true
}

/// `var(--name)` or `var(--name, fallback)`, with the parser positioned inside
/// the parentheses.
fn resolve_var(
    arguments: &mut CssParser<'_, '_>,
    source: &str,
    variables: &Variables,
    out: &mut String,
    depth: u8,
) -> bool {
    let Ok(name) = arguments.expect_ident_cloned() else {
        return false;
    };
    if !name.starts_with("--") {
        // `var(color)` is not a variable reference at all.
        return false;
    }

    let has_fallback = arguments.try_parse(CssParser::expect_comma).is_ok();
    if let Some(value) = variables.get(&name) {
        out.push_str(value);
        // The fallback is not used, but it still has to be consumed, and a
        // fallback that is itself nonsense does not make this reference fail.
        while arguments.next().is_ok() {}
        return true;
    }
    if !has_fallback {
        return false;
    }
    // The fallback is everything after the comma, and the space that follows
    // the comma is part of that span rather than part of the value.
    let mut fallback = String::new();
    if !rewrite(arguments, source, variables, &mut fallback, depth + 1) {
        return false;
    }
    out.push_str(fallback.trim());
    true
}

fn closing_delimiter(token: &Token<'_>) -> char {
    match token {
        Token::SquareBracketBlock => ']',
        Token::CurlyBracketBlock => '}',
        _ => ')',
    }
}

fn push_span(out: &mut String, source: &str, from: usize, to: usize) {
    if let Some(text) = source.get(from..to) {
        out.push_str(text);
    }
}

/// Every custom property a value refers to, in the order they appear.
///
/// Used to resolve declarations in dependency order. It looks inside
/// fallbacks as well, because `var(--a, var(--b))` depends on both.
pub fn referenced_variables(value: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut input = ParserInput::new(value);
    collect_references(&mut CssParser::new(&mut input), &mut found, 0);
    found
}

fn collect_references(input: &mut CssParser<'_, '_>, found: &mut Vec<String>, depth: u8) {
    if depth > MAX_SUBSTITUTION_DEPTH {
        return;
    }
    while let Ok(token) = input.next() {
        let is_var = matches!(&token, Token::Function(name) if name.eq_ignore_ascii_case("var"));
        let opens_a_block = matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        );
        if !opens_a_block {
            continue;
        }
        let _ = input.parse_nested_block(|inner| -> Result<(), cssparser::ParseError<'_, ()>> {
            if is_var
                && let Ok(name) = inner.try_parse(CssParser::expect_ident_cloned)
                && name.starts_with("--")
            {
                found.push(name.to_string());
            }
            collect_references(inner, found, depth + 1);
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOMEWHERE: Location = Location { line: 1, column: 1 };

    fn variables(pairs: &[(&str, &str)]) -> Variables {
        let mut out = Variables::new();
        for (name, value) in pairs {
            out.set(name, (*value).to_owned());
        }
        out
    }

    fn declared(pairs: &[(&str, &str)]) -> Vec<(Box<str>, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).to_owned()))
            .collect()
    }

    fn resolved(value: &str, pairs: &[(&str, &str)]) -> Resolved {
        substitute(value, &variables(pairs))
    }

    fn text(value: &str, pairs: &[(&str, &str)]) -> String {
        match resolved(value, pairs) {
            Resolved::Value(value) => value,
            Resolved::InvalidAtComputedValueTime => {
                panic!("{value} should have resolved")
            }
        }
    }

    #[test]
    fn a_value_with_no_variables_comes_through_untouched() {
        assert_eq!(text("4px  8px", &[]), "4px  8px");
        assert_eq!(text("", &[]), "");
        assert_eq!(text("rgb(1, 2, 3)", &[]), "rgb(1, 2, 3)");
    }

    #[test]
    fn a_variable_is_replaced_by_its_value() {
        assert_eq!(text("var(--gap)", &[("--gap", "8px")]), "8px");
        assert_eq!(
            text("var(--a) var(--b)", &[("--a", "1px"), ("--b", "2px")]),
            "1px 2px",
        );
    }

    #[test]
    fn the_text_around_a_variable_survives_exactly_as_written() {
        assert_eq!(
            text("calc(var(--gap) * 2)", &[("--gap", "8px")]),
            "calc(8px * 2)",
        );
        assert_eq!(
            text("  0  var(--gap)   auto  ", &[("--gap", "8px")]),
            "  0  8px   auto  ",
        );
        assert_eq!(
            text(
                "color-mix(in oklab, var(--ink) 12%, transparent)",
                &[("--ink", "#101014")],
            ),
            "color-mix(in oklab, #101014 12%, transparent)",
        );
    }

    #[test]
    fn a_fallback_is_used_only_when_the_property_is_not_set() {
        assert_eq!(text("var(--gap, 4px)", &[]), "4px");
        assert_eq!(text("var(--gap, 4px)", &[("--gap", "8px")]), "8px");
        assert_eq!(text("var(--a, var(--b, 2px))", &[("--b", "16px")]), "16px",);
        assert_eq!(text("var(--a, var(--b, 2px))", &[]), "2px");
    }

    #[test]
    fn a_fallback_may_be_several_tokens_including_a_comma() {
        assert_eq!(text("var(--f, 1px, 2px)", &[]), "1px, 2px");
        assert_eq!(text("var(--f, rgb(1, 2, 3))", &[]), "rgb(1, 2, 3)");
    }

    #[test]
    fn a_variable_that_is_not_set_and_has_no_fallback_makes_the_value_invalid() {
        assert_eq!(
            resolved("var(--missing)", &[]),
            Resolved::InvalidAtComputedValueTime,
        );
        assert_eq!(
            resolved("1px solid var(--missing)", &[]),
            Resolved::InvalidAtComputedValueTime,
            "one missing reference invalidates the whole declaration",
        );
        assert_eq!(
            resolved("calc(var(--missing) * 2)", &[]),
            Resolved::InvalidAtComputedValueTime,
        );
    }

    #[test]
    fn var_of_something_that_is_not_a_custom_property_is_refused() {
        assert_eq!(
            resolved("var(color)", &[]),
            Resolved::InvalidAtComputedValueTime
        );
        assert_eq!(resolved("var()", &[]), Resolved::InvalidAtComputedValueTime);
    }

    #[test]
    fn a_value_a_variable_supplies_can_itself_hold_anything() {
        assert_eq!(
            text(
                "var(--shadow)",
                &[("--shadow", "0 1px 2px rgb(0 0 0 / 20%)")]
            ),
            "0 1px 2px rgb(0 0 0 / 20%)",
        );
    }

    #[test]
    fn a_variable_defined_on_a_parent_reaches_a_child_through_the_inherited_map() {
        let inherited = variables(&[("--surface", "#fff")]);
        let mut issues = Vec::new();
        let resolved = resolve_variables(&inherited, &declared(&[]), SOMEWHERE, &mut issues);
        assert_eq!(resolved.get("--surface"), Some("#fff"));
        assert!(issues.is_empty());
    }

    #[test]
    fn a_declaration_overrides_what_was_inherited() {
        let inherited = variables(&[("--surface", "#fff")]);
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &inherited,
            &declared(&[("--surface", "#101014")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(resolved.get("--surface"), Some("#101014"));
        assert!(issues.is_empty());
    }

    #[test]
    fn a_declaration_may_use_another_declared_on_the_same_element_whatever_the_order() {
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &Variables::new(),
            // `--big` is written before the property it depends on.
            &declared(&[("--big", "calc(var(--gap) * 2)"), ("--gap", "8px")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(resolved.get("--gap"), Some("8px"));
        assert_eq!(resolved.get("--big"), Some("calc(8px * 2)"));
        assert!(issues.is_empty());
    }

    #[test]
    fn a_declaration_may_build_on_the_inherited_value_of_another() {
        let inherited = variables(&[("--gap", "4px")]);
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &inherited,
            &declared(&[("--pad", "var(--gap)")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(resolved.get("--pad"), Some("4px"));
    }

    #[test]
    fn a_cycle_is_refused_rather_than_looped() {
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &Variables::new(),
            &declared(&[("--a", "var(--b)"), ("--b", "var(--a)")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(resolved.get("--a"), None);
        assert_eq!(resolved.get("--b"), None);
        assert_eq!(issues.len(), 2);
        assert!(
            issues
                .iter()
                .all(|issue| issue.kind == IssueKind::VariableCycle)
        );
    }

    #[test]
    fn a_property_that_refers_to_itself_is_a_cycle_of_one() {
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &Variables::new(),
            &declared(&[("--a", "var(--a)")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(resolved.get("--a"), None);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn a_long_ring_is_refused_whole_and_nothing_outside_it_is_harmed() {
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &Variables::new(),
            &declared(&[
                ("--a", "var(--b)"),
                ("--b", "var(--c)"),
                ("--c", "var(--a)"),
                ("--fine", "8px"),
            ]),
            SOMEWHERE,
            &mut issues,
        );
        for name in ["--a", "--b", "--c"] {
            assert_eq!(resolved.get(name), None, "{name} is in the ring");
        }
        assert_eq!(resolved.get("--fine"), Some("8px"));
        assert_eq!(issues.len(), 3);
    }

    #[test]
    fn a_cycle_refuses_the_ring_and_not_the_inherited_value_of_a_bystander() {
        let inherited = variables(&[("--kept", "1px")]);
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &inherited,
            &declared(&[("--a", "var(--a)")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(resolved.get("--kept"), Some("1px"));
    }

    #[test]
    fn a_declaration_that_cannot_resolve_leaves_the_inherited_value_alone() {
        let inherited = variables(&[("--gap", "4px")]);
        let mut issues = Vec::new();
        let resolved = resolve_variables(
            &inherited,
            &declared(&[("--gap", "var(--nowhere)")]),
            SOMEWHERE,
            &mut issues,
        );
        assert_eq!(
            resolved.get("--gap"),
            Some("4px"),
            "an invalid declaration is as though it were not written",
        );
    }

    #[test]
    fn the_references_in_a_value_are_found_including_inside_fallbacks() {
        assert_eq!(referenced_variables("8px"), Vec::<String>::new());
        assert_eq!(referenced_variables("var(--a)"), vec!["--a"]);
        assert_eq!(
            referenced_variables("calc(var(--a) + var(--b))"),
            vec!["--a", "--b"],
        );
        assert_eq!(
            referenced_variables("var(--a, var(--b))"),
            vec!["--a", "--b"],
            "a fallback is a dependency too",
        );
        assert_eq!(referenced_variables("var(color)"), Vec::<String>::new());
    }

    #[test]
    fn a_map_reports_what_it_holds() {
        let held = variables(&[("--b", "2"), ("--a", "1")]);
        assert_eq!(held.len(), 2);
        assert!(!held.is_empty());
        assert!(Variables::new().is_empty());
        assert_eq!(
            held.iter().collect::<Vec<_>>(),
            vec![("--a", "1"), ("--b", "2")],
            "iteration is by name, so it is the same twice",
        );
    }
}

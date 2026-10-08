/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A style sheet nested past the limit is refused rather than overflowing the
//! stack (queue item 330).
//!
//! A page's `<style>` is bytes from outside (`docs/autonomy/LOOP.md`, stage 2
//! § 2). Before this, a selector written `:is(` five thousand times aborted
//! the process. Each sheet here nests a selector, a declaration's value, a
//! `var()` fallback, a media condition or `@media` itself a hundred thousand
//! deep, and is parsed on a thread with the stack the renderer has: the
//! renderer parses style sheets on its process's main thread
//! (`alo-render`'s `main`), which macOS and Linux both give eight megabytes.
//! A debug build, which spends more stack per call than a release one, is
//! the harder of the two, and it is the one these run in.

use alo_css::{IssueKind, MediaContext, Rule, StyleIssue, Stylesheet, parse_stylesheet};

/// The stack `alo-render`'s main thread has on macOS and Linux.
const RENDERERS_STACK: usize = 8 * 1024 * 1024;

/// A hundred thousand: far past the limit, and past what the parsers survived
/// recursing into on any stack.
const DEEP: usize = 100_000;

/// Parse `text` on a thread with the renderer's stack. [`None`] if the thread
/// did not finish, which is what an overflow looks like from here — though an
/// overflow aborts the whole test binary, and that is the failure these tests
/// were written to see.
fn parse_on_the_renderers_stack(text: String) -> Option<Stylesheet> {
    std::thread::Builder::new()
        .stack_size(RENDERERS_STACK)
        .spawn(move || parse_stylesheet(&text))
        .ok()?
        .join()
        .ok()
}

/// The selectors of every style rule that applies, in order, as written.
fn selectors(sheet: &Stylesheet) -> Vec<String> {
    sheet
        .style_rules_for(&MediaContext::default())
        .iter()
        .map(|rule| rule.selectors.to_string())
        .collect()
}

/// The issues of one kind.
fn issues_of(sheet: &Stylesheet, kind: IssueKind) -> Vec<&StyleIssue> {
    sheet
        .issues()
        .iter()
        .filter(|issue| issue.kind == kind)
        .collect()
}

/// `open` `DEEP` times, then `inside`, then `close` as many times.
fn nested(open: &str, inside: &str, close: &str) -> String {
    format!("{}{inside}{}", open.repeat(DEEP), close.repeat(DEEP))
}

#[test]
fn a_selector_nested_too_deep_drops_its_rule_and_keeps_the_rest() {
    let text = format!(
        "a {{ color: red }}\n{} {{ color: green }}\nb {{ color: blue }}",
        nested(":is(", "p", ")"),
    );
    let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
    assert_eq!(selectors(&sheet), ["a", "b"]);
    let deep = issues_of(&sheet, IssueKind::NestedTooDeep);
    assert_eq!(deep.len(), 1, "{:?}", sheet.issues().len());
    assert_eq!(deep[0].at.line, 2, "the issue says where the rule was");
    assert!(deep[0].source.starts_with(":is(:is("));
    assert!(
        deep[0].to_string().contains("nested deeper than 32"),
        "the issue says why",
    );
    assert!(issues_of(&sheet, IssueKind::InvalidSelector).is_empty());
}

#[test]
fn a_declaration_nested_too_deep_is_dropped_and_its_neighbours_kept() {
    for value in [
        nested("(", "1", ")"),
        nested("[", "1", "]"),
        nested("calc(", "1px", ")"),
        nested("{", "", "}"),
    ] {
        let text = format!("a {{ color: red; width: {value}; margin: 0 }} b {{ color: blue }}");
        let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
        assert_eq!(selectors(&sheet), ["a", "b"]);
        let a = sheet.style_rules_for(&MediaContext::default())[0];
        let written: Vec<String> = a
            .declarations
            .iter()
            .map(|declaration| declaration.name.to_string())
            .collect();
        assert!(written.contains(&"color".to_owned()), "{written:?}");
        assert!(written.contains(&"margin".to_owned()), "{written:?}");
        assert!(!written.contains(&"width".to_owned()), "{written:?}");
        let deep = issues_of(&sheet, IssueKind::NestedTooDeep);
        assert_eq!(deep.len(), 1);
        assert!(
            deep[0].source.starts_with("width:"),
            "{}",
            &deep[0].source[..20]
        );
    }
}

#[test]
fn a_var_fallback_nested_too_deep_is_dropped_like_any_value() {
    let text = format!(
        "a {{ --ok: 1px; --deep: {}; color: var(--ok) }}",
        nested("var(--missing, ", "1px", ")"),
    );
    let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
    let a = sheet.style_rules_for(&MediaContext::default())[0];
    assert_eq!(a.declarations.custom_properties().count(), 1, "only --ok");
    assert_eq!(issues_of(&sheet, IssueKind::NestedTooDeep).len(), 1);
}

#[test]
fn a_media_condition_nested_too_deep_drops_its_rule_and_keeps_the_rest() {
    let text = format!(
        "a {{ color: red }}\n@media {} {{ p {{ color: green }} }}\nb {{ color: blue }}",
        nested("(", "min-width: 1px", ")"),
    );
    let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
    assert_eq!(selectors(&sheet), ["a", "b"]);
    assert_eq!(sheet.rules().len(), 2, "the @media rule is gone");
    let deep = issues_of(&sheet, IssueKind::NestedTooDeep);
    assert_eq!(deep.len(), 1);
    assert_eq!(deep[0].at.line, 2);
    assert!(issues_of(&sheet, IssueKind::UnknownMediaCondition).is_empty());
}

#[test]
fn media_inside_media_past_the_limit_is_dropped_and_the_levels_above_kept() {
    let text = format!(
        "a {{ color: red }} {} b {{ color: blue }}",
        nested("@media screen { ", "p { color: green } ", "} "),
    );
    let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
    // The innermost `p` is refused with the 33rd `@media`; `a` and `b` are
    // on either side of all of it.
    assert_eq!(selectors(&sheet), ["a", "b"]);
    assert_eq!(issues_of(&sheet, IssueKind::NestedTooDeep).len(), 1);

    // Thirty-two levels are kept, each holding the next.
    let mut depth = 0;
    let mut rules = sheet.rules();
    while let Some(Rule::Media(media)) = rules.iter().find(|rule| matches!(rule, Rule::Media(_))) {
        depth += 1;
        rules = &media.rules;
    }
    assert_eq!(depth, 32);
}

#[test]
fn media_nested_to_the_limit_still_applies() {
    let text = format!(
        "{}p {{ color: green }}{}",
        "@media screen { ".repeat(32),
        " }".repeat(32),
    );
    let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
    assert_eq!(selectors(&sheet), ["p"]);
    assert!(sheet.issues().is_empty(), "{:?}", sheet.issues());
}

#[test]
fn what_nests_to_the_limit_is_kept() {
    let selector = format!("{}p{}", ":is(".repeat(32), ")".repeat(32));
    let value = format!("{}1px{}", "calc(".repeat(32), ")".repeat(32));
    let condition = format!("{}min-width: 1px{}", "(".repeat(32), ")".repeat(32));
    let text =
        format!("{selector} {{ width: {value} }} @media {condition} {{ b {{ color: red }} }}");
    let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
    assert_eq!(sheet.rules().len(), 2);
    assert!(issues_of(&sheet, IssueKind::NestedTooDeep).is_empty());
    let rule = sheet.style_rules_for(&MediaContext::default())[0];
    assert_eq!(rule.declarations.len(), 1, "the 32-deep value is kept");
}

#[test]
fn a_sheet_that_ends_inside_its_blocks_is_refused_and_not_a_crash() {
    for text in [
        format!("a {{ color: red }} {}", ":is(".repeat(DEEP)),
        format!("a {{ color: red }} b {{ width: {}", "(".repeat(DEEP)),
        format!("a {{ color: red }} @media {}", "(".repeat(DEEP)),
        format!("a {{ color: red }} {}", "@media screen { ".repeat(DEEP)),
        format!("a {{ color: red }} {}", "{".repeat(DEEP)),
    ] {
        let sheet = parse_on_the_renderers_stack(text).expect("the parser finished");
        assert_eq!(selectors(&sheet).first().map(String::as_str), Some("a"));
    }
}

//! A style sheet in the shape alo's own design system is written in.
//!
//! Custom properties throughout, a light and a dark theme behind
//! `prefers-color-scheme`, a width breakpoint, and one at-rule this engine
//! does not implement. If this sheet does not survive parsing intact, nothing
//! of alo renders — which is why it is the case that is committed rather than
//! a tidier one.

use alo_css::{ColorScheme, IssueKind, MediaContext, PropertyName, Rule, parse_stylesheet};

const SHEET: &str = r"
:root {
  --surface: #ffffff;
  --ink: #101014;
  --gap: 8px;
  color-scheme: light dark;
}

@media (prefers-color-scheme: dark) {
  :root {
    --surface: #101014;
    --ink: #f4f4f5;
  }
}

body {
  background: var(--surface);
  color: var(--ink);
  margin: 0;
}

.row {
  display: flex;
  gap: var(--gap);
  padding-inline: calc(var(--gap) * 2);
}

.row[aria-selected='true'] {
  background: color-mix(in oklab, var(--ink) 12%, transparent);
}

.row > .label:not(.muted) {
  font-weight: 600 !important;
}

@media (min-width: 900px) {
  .row {
    gap: calc(var(--gap) * 1.5);
  }
}

@supports (container-type: inline-size) {
  .row { container-type: inline-size }
}
";

#[test]
fn the_whole_sheet_parses_into_rules_we_hold() {
    let sheet = parse_stylesheet(SHEET);
    assert_eq!(sheet.rules().len(), 8);

    let kinds: Vec<&str> = sheet
        .rules()
        .iter()
        .map(|rule| match rule {
            Rule::Style(_) => "style",
            Rule::Media(_) => "media",
            Rule::Unknown(_) => "unknown",
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "style", "media", "style", "style", "style", "style", "media", "unknown",
        ],
    );
}

#[test]
fn the_only_thing_the_sheet_asked_for_that_we_did_not_do_is_the_at_rule() {
    let sheet = parse_stylesheet(SHEET);
    let kinds: Vec<IssueKind> = sheet.issues().iter().map(|issue| issue.kind).collect();
    assert_eq!(
        kinds,
        vec![IssueKind::UnknownAtRule],
        "everything else in this sheet is understood: {:?}",
        sheet
            .issues()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    );
}

#[test]
fn the_custom_properties_are_all_there_with_their_values() {
    let sheet = parse_stylesheet(SHEET);
    let light = sheet.style_rules_for(&MediaContext::new(1280.0, ColorScheme::Light));
    let root = light.first().expect("the :root rule comes first");
    let named: Vec<(&str, &str)> = root
        .declarations
        .custom_properties()
        .map(|declaration| (declaration.name.as_str(), &*declaration.value))
        .collect();
    assert_eq!(
        named,
        vec![
            ("--surface", "#ffffff"),
            ("--ink", "#101014"),
            ("--gap", "8px")
        ],
    );
}

#[test]
fn the_dark_theme_arrives_only_when_it_is_asked_for() {
    let sheet = parse_stylesheet(SHEET);
    let surface = |scheme| {
        sheet
            .style_rules_for(&MediaContext::new(1280.0, scheme))
            .iter()
            .filter(|rule| rule.selectors.to_string() == ":root")
            .filter_map(|rule| rule.declarations.get(&PropertyName::parse("--surface")))
            .map(|declaration| declaration.value.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(surface(ColorScheme::Light), vec!["#ffffff"]);
    assert_eq!(
        surface(ColorScheme::Dark),
        vec!["#ffffff", "#101014"],
        "both rules apply in dark, and the later one is the one the cascade will take",
    );
}

#[test]
fn a_width_breakpoint_adds_a_rule_and_narrower_does_not() {
    let sheet = parse_stylesheet(SHEET);
    let count = |width| {
        sheet
            .style_rules_for(&MediaContext::new(width, ColorScheme::Light))
            .len()
    };
    assert_eq!(count(600.0), 5);
    assert_eq!(count(900.0), 6, "the breakpoint is inclusive");
    assert_eq!(count(1400.0), 6);
}

#[test]
fn a_value_with_functions_and_nested_parentheses_survives_whole() {
    let sheet = parse_stylesheet(SHEET);
    let rules = sheet.style_rules_for(&MediaContext::default());
    let row = rules
        .iter()
        .find(|rule| rule.selectors.to_string() == ".row")
        .expect("the .row rule");
    assert_eq!(
        row.declarations
            .get(&PropertyName::parse("padding-inline"))
            .map(|declaration| &*declaration.value),
        Some("calc(var(--gap) * 2)"),
    );

    let selected = rules
        .iter()
        .find(|rule| rule.selectors.to_string().contains("aria-selected"))
        .expect("the selected-row rule");
    assert_eq!(
        selected
            .declarations
            .get(&PropertyName::parse("background"))
            .map(|declaration| &*declaration.value),
        Some("color-mix(in oklab, var(--ink) 12%, transparent)"),
    );
}

#[test]
fn important_is_recorded_separately_from_the_value() {
    let sheet = parse_stylesheet(SHEET);
    let rules = sheet.style_rules_for(&MediaContext::default());
    let label = rules
        .iter()
        .find(|rule| rule.selectors.to_string().contains(".label"))
        .expect("the label rule");
    let weight = label
        .declarations
        .get(&PropertyName::parse("font-weight"))
        .expect("font-weight");
    assert_eq!(weight.value, "600");
    assert!(weight.importance.is_important());
}

#[test]
fn the_at_rule_we_do_not_implement_is_kept_whole_and_contributes_nothing() {
    let sheet = parse_stylesheet(SHEET);
    let Some(Rule::Unknown(rule)) = sheet.rules().last() else {
        panic!("the last rule is the one we do not implement");
    };
    assert_eq!(&*rule.name, "supports");
    assert_eq!(rule.prelude, "(container-type: inline-size)");
    assert!(
        rule.block
            .as_deref()
            .is_some_and(|block| block.contains("container-type: inline-size")),
        "kept whole, so a later stage can implement it without re-parsing",
    );
    assert!(
        !sheet
            .style_rules_for(&MediaContext::default())
            .iter()
            .any(|styled| styled
                .declarations
                .get(&PropertyName::parse("container-type"))
                .is_some()),
        "and nothing inside it applies today",
    );
}

#[test]
fn a_sheet_of_nothing_but_things_we_refuse_still_parses() {
    let sheet = parse_stylesheet(
        "p:has(a) { color: red } @charset 'utf-8'; ::-webkit-thing { color: blue }",
    );
    assert!(
        sheet.style_rules_for(&MediaContext::default()).is_empty(),
        "nothing applies",
    );
    assert_eq!(sheet.issues().len(), 3, "and everything is accounted for");
}

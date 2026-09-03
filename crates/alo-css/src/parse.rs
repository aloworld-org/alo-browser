//! Reading a style sheet: text in, rules out.
//!
//! `cssparser` decides where one rule ends and the next begins, which is
//! genuinely fiddly and entirely specified. What happens to each rule once it
//! has been found is this engine's decision, and it follows one principle:
//! **nothing disappears in silence.** A property we do not implement is kept
//! with its value; an at-rule we do not implement is kept whole; a selector we
//! cannot evaluate takes its rule down, because that is what CSS says and
//! because a rule nobody can evaluate would match everything or nothing. Each
//! of those is recorded as a [`StyleIssue`] with the text that caused it.

use crate::declaration::{Declaration, DeclarationBlock, Importance};
use crate::issue::{IssueKind, Location, StyleIssue};
use crate::media::MediaQueryList;
use crate::selector::SelectorList;
use crate::stylesheet::{MediaRule, Rule, StyleRule, Stylesheet, UnknownAtRule};
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser as CssParser, ParserInput,
    ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, SourceLocation,
    SourcePosition, StyleSheetParser, Token,
};
use selectors::parser::SelectorParseErrorKind;

/// Parse a style sheet.
///
/// Never fails. Everything that could not be used is in
/// [`Stylesheet::issues`], with the text it came from.
pub fn parse_stylesheet(text: &str) -> Stylesheet {
    let mut input = ParserInput::new(text);
    let mut parser = CssParser::new(&mut input);
    let mut rules = Vec::new();
    let mut top = TopLevel::default();

    {
        let mut iterator = StyleSheetParser::new(&mut parser, &mut top);
        while let Some(result) = iterator.next() {
            match result {
                Ok(rule) => rules.push(rule),
                Err((error, source)) => iterator.parser.record_dropped_rule(&error, source),
            }
        }
    }

    Stylesheet::from_parts(rules, top.issues)
}

/// Where a parser is, as an issue reports it.
fn location_of(at: SourceLocation) -> Location {
    Location {
        // `cssparser` counts lines from zero and columns from one. A person
        // reading an error counts both from one.
        line: at.line + 1,
        column: at.column,
    }
}

/// Everything left in the parser, as the text it was written with.
///
/// Draining rather than parsing is the point: this is how an at-rule we do not
/// implement is kept whole instead of thrown away.
fn drain_to_source<'i>(input: &mut CssParser<'i, '_>) -> &'i str {
    let start = input.position();
    while input.next().is_ok() {}
    input.slice_from(start)
}

/// The prelude of an at-rule, once we know which at-rule it is.
enum AtRulePrelude {
    /// `@media`, with its condition.
    Media {
        queries: MediaQueryList,
        at: Location,
    },
    /// An at-rule this engine does not implement, kept as written.
    Unknown {
        name: Box<str>,
        prelude: String,
        at: Location,
    },
}

/// The rules at the top level of a sheet, and inside an `@media` block — they
/// are the same grammar, which is why one parser serves both.
#[derive(Default)]
struct TopLevel {
    issues: Vec<StyleIssue>,
}

impl TopLevel {
    fn record_dropped_rule(
        &mut self,
        error: &ParseError<'_, SelectorParseErrorKind<'_>>,
        source: &str,
    ) {
        self.issues.push(StyleIssue {
            kind: IssueKind::InvalidSelector,
            source: source.trim().to_owned(),
            at: location_of(error.location),
        });
    }

    /// Parse the rules inside a block, keeping every issue they raise.
    fn parse_nested_rules(&mut self, input: &mut CssParser<'_, '_>) -> Vec<Rule> {
        let mut rules = Vec::new();
        let mut dropped = Vec::new();
        {
            for result in StyleSheetParser::new(input, self) {
                match result {
                    Ok(rule) => rules.push(rule),
                    Err((error, source)) => {
                        dropped.push((location_of(error.location), source.trim().to_owned()));
                    }
                }
            }
        }
        for (at, source) in dropped {
            self.issues.push(StyleIssue {
                kind: IssueKind::InvalidSelector,
                source,
                at,
            });
        }
        rules
    }
}

impl<'i> QualifiedRuleParser<'i> for TopLevel {
    type Prelude = (SelectorList, Location);
    type QualifiedRule = Rule;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut CssParser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let at = location_of(input.current_source_location());
        Ok((SelectorList::parse(input)?, at))
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut CssParser<'i, 't>,
    ) -> Result<Rule, ParseError<'i, Self::Error>> {
        let (selectors, at) = prelude;
        for selector in &selectors {
            if let Some(pseudo) = selector.pseudo_element() {
                self.issues.push(StyleIssue {
                    kind: IssueKind::PseudoElementNotProduced,
                    source: format!("{selector} names {}", pseudo.as_str()),
                    at,
                });
            }
        }
        Ok(Rule::Style(StyleRule {
            selectors,
            declarations: parse_declarations(input, &mut self.issues),
            at,
        }))
    }
}

impl<'i> AtRuleParser<'i> for TopLevel {
    type Prelude = AtRulePrelude;
    type AtRule = Rule;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut CssParser<'i, 't>,
    ) -> Result<AtRulePrelude, ParseError<'i, Self::Error>> {
        let at = location_of(input.current_source_location());
        if name.eq_ignore_ascii_case("media") {
            return Ok(AtRulePrelude::Media {
                queries: MediaQueryList::parse(input, &mut self.issues),
                at,
            });
        }
        let prelude = drain_to_source(input).trim().to_owned();
        let name: Box<str> = name.to_ascii_lowercase().into();
        self.issues.push(StyleIssue {
            kind: IssueKind::UnknownAtRule,
            source: format!("@{name} {prelude}").trim_end().to_owned(),
            at,
        });
        Ok(AtRulePrelude::Unknown { name, prelude, at })
    }

    fn rule_without_block(
        &mut self,
        prelude: AtRulePrelude,
        _start: &ParserState,
    ) -> Result<Rule, ()> {
        match prelude {
            // `@media` without a block is not a rule at all.
            AtRulePrelude::Media { .. } => Err(()),
            AtRulePrelude::Unknown { name, prelude, at } => Ok(Rule::Unknown(UnknownAtRule {
                name,
                prelude,
                block: None,
                at,
            })),
        }
    }

    fn parse_block<'t>(
        &mut self,
        prelude: AtRulePrelude,
        _start: &ParserState,
        input: &mut CssParser<'i, 't>,
    ) -> Result<Rule, ParseError<'i, Self::Error>> {
        match prelude {
            AtRulePrelude::Media { queries, at } => Ok(Rule::Media(MediaRule {
                queries,
                rules: self.parse_nested_rules(input),
                at,
            })),
            AtRulePrelude::Unknown { name, prelude, at } => Ok(Rule::Unknown(UnknownAtRule {
                name,
                prelude,
                block: Some(drain_to_source(input).to_owned()),
                at,
            })),
        }
    }
}

/// Read the declarations between one pair of braces.
fn parse_declarations(
    input: &mut CssParser<'_, '_>,
    issues: &mut Vec<StyleIssue>,
) -> DeclarationBlock {
    let mut block = DeclarationBlock::new();
    let mut declarations = Declarations::default();
    {
        let mut iterator = RuleBodyParser::new(input, &mut declarations);
        while let Some(result) = iterator.next() {
            match result {
                Ok(declaration) => block.push(declaration),
                Err((error, source)) => iterator
                    .parser
                    .dropped
                    .push((location_of(error.location), source.trim().to_owned())),
            }
        }
    }
    for (at, source) in declarations.dropped {
        issues.push(StyleIssue {
            kind: IssueKind::InvalidDeclaration,
            source,
            at,
        });
    }
    block
}

/// The declarations inside one block.
#[derive(Default)]
struct Declarations {
    dropped: Vec<(Location, String)>,
}

impl<'i> DeclarationParser<'i> for Declarations {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut CssParser<'i, 't>,
        _start: &ParserState,
    ) -> Result<Declaration, ParseError<'i, ()>> {
        let start = input.position();
        let (value_end, importance) = scan_value(input);
        let whole = input.slice_from(start);
        let value = whole
            .get(..value_end.byte_index().saturating_sub(start.byte_index()))
            .unwrap_or(whole);
        // An unknown property is kept, value and all, and ignored — which is
        // what `docs/features.md` asks for, so that a later stage can
        // implement it without re-parsing the sheet. Knowing which properties
        // exist is the cascade's business, not the parser's.
        Ok(Declaration::new(&name, value, importance))
    }
}

/// Walk to the end of a declaration's value, and report where the value ends
/// and whether `!important` followed it.
///
/// The scan cannot stop at the first `!important` it sees: `color: red
/// !important !important` has one that is not final, and only a trailing one
/// counts.
fn scan_value(input: &mut CssParser<'_, '_>) -> (SourcePosition, Importance) {
    let mut value_end = input.position();
    let mut importance = Importance::Normal;
    loop {
        let before = input.state();
        if input.try_parse(cssparser::parse_important).is_ok() {
            if input.is_exhausted() {
                importance = Importance::Important;
                value_end = before.position();
                break;
            }
            input.reset(&before);
        }
        if !consume_one_value_token(input) {
            break;
        }
        value_end = input.position();
        importance = Importance::Normal;
    }
    (value_end, importance)
}

/// Consume one token, and all of it: a `rgb(` or a `[` opens a block, and
/// stepping over it rather than through it is how `rgb(1, 2, 3)` becomes the
/// value `rgb(` — the position after a block is only right once the block has
/// been entered and left.
///
/// Returns whether there was a token to consume.
fn consume_one_value_token(input: &mut CssParser<'_, '_>) -> bool {
    let opens_a_block = match input.next() {
        Ok(token) => matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ),
        Err(_) => return false,
    };
    if opens_a_block {
        let _ = input.parse_nested_block(|inner| -> Result<(), ParseError<'_, ()>> {
            while consume_one_value_token(inner) {}
            Ok(())
        });
    }
    true
}

// A declaration block may contain at-rules and, in stage 2, nested rules.
// `docs/features.md` puts nesting in stage 2, so neither is parsed here and
// both default to being refused — which drops them with an issue rather than
// silently treating them as declarations.
impl AtRuleParser<'_> for Declarations {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl QualifiedRuleParser<'_> for Declarations {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl RuleBodyItemParser<'_, Declaration, ()> for Declarations {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declaration::PropertyName;
    use crate::media::MediaContext;

    fn only_declarations(text: &str) -> DeclarationBlock {
        let sheet = parse_stylesheet(text);
        match sheet.rules() {
            [Rule::Style(rule)] => rule.declarations.clone(),
            other => panic!("expected one style rule, got {other:?}"),
        }
    }

    #[test]
    fn a_declaration_keeps_the_value_it_was_written_with() {
        let block = only_declarations("a { padding: 4px  8px; color: rgb(1, 2, 3) }");
        // Six, not two: the `padding` shorthand is written down as its four
        // longhands as well, so that an author's `padding` and a user agent's
        // `padding-left` compete as the same property rather than as two.
        assert_eq!(block.len(), 6);
        assert_eq!(
            block
                .get(&PropertyName::parse("padding-left"))
                .map(|d| &*d.value),
            Some("8px"),
            "the shorthand's horizontal value should be the left one"
        );
        assert_eq!(
            block
                .get(&PropertyName::parse("padding"))
                .map(|d| &*d.value),
            Some("4px  8px"),
        );
        assert_eq!(
            block.get(&PropertyName::parse("color")).map(|d| &*d.value),
            Some("rgb(1, 2, 3)"),
        );
    }

    #[test]
    fn important_is_taken_off_the_value_and_recorded() {
        let block = only_declarations("a { color: red !important; margin: 0 }");
        let color = block.get(&PropertyName::parse("color")).expect("color");
        assert_eq!(color.value, "red");
        assert_eq!(color.importance, Importance::Important);

        let margin = block.get(&PropertyName::parse("margin")).expect("margin");
        assert_eq!(margin.importance, Importance::Normal);
    }

    #[test]
    fn an_important_that_is_not_last_is_part_of_the_value() {
        let block = only_declarations("a { font: x !important !important }");
        let font = block.get(&PropertyName::parse("font")).expect("font");
        assert_eq!(font.importance, Importance::Important);
        assert_eq!(font.value, "x !important");
    }

    #[test]
    fn an_unknown_property_is_kept_rather_than_dropped() {
        let block = only_declarations("a { color: red; -alo-nonexistent: 3 fish; margin: 0 }");
        assert_eq!(
            block.len(),
            7,
            "the property we do not implement is here, and the margin brought its four sides"
        );
        assert_eq!(
            block
                .get(&PropertyName::parse("-alo-nonexistent"))
                .map(|d| &*d.value),
            Some("3 fish"),
        );
    }

    #[test]
    fn a_custom_property_keeps_its_case_and_its_value() {
        let block =
            only_declarations("a { --Surface-Raised: #101014; color: var(--Surface-Raised) }");
        assert_eq!(
            block
                .get(&PropertyName::Custom("--Surface-Raised".into()))
                .map(|d| &*d.value),
            Some("#101014"),
        );
        assert_eq!(block.custom_properties().count(), 1);
    }

    #[test]
    fn a_rule_with_a_selector_we_cannot_evaluate_is_dropped_and_recorded() {
        let sheet =
            parse_stylesheet("a { color: red } p:has(b) { color: blue } i { color: green }");
        assert_eq!(sheet.rules().len(), 2, "the middle rule is gone");
        assert_eq!(sheet.issues().len(), 1);
        assert_eq!(sheet.issues()[0].kind, IssueKind::InvalidSelector);
        assert!(sheet.issues()[0].source.starts_with("p:has(b)"));
    }

    #[test]
    fn a_rule_naming_a_pseudo_element_is_kept_and_says_it_will_not_match() {
        let sheet = parse_stylesheet("p::before { color: red }");
        assert_eq!(sheet.rules().len(), 1);
        assert_eq!(sheet.issues().len(), 1);
        assert_eq!(sheet.issues()[0].kind, IssueKind::PseudoElementNotProduced,);
        assert!(sheet.issues()[0].source.contains("::before"));
    }

    #[test]
    fn an_unknown_at_rule_with_a_block_is_kept_whole() {
        let sheet = parse_stylesheet("@supports (display: grid) { a { color: red } }");
        match sheet.rules() {
            [Rule::Unknown(rule)] => {
                assert_eq!(&*rule.name, "supports");
                assert_eq!(rule.prelude, "(display: grid)");
                assert_eq!(rule.block.as_deref(), Some(" a { color: red } "));
            }
            other => panic!("expected one unknown at-rule, got {other:?}"),
        }
        assert_eq!(sheet.issues()[0].kind, IssueKind::UnknownAtRule);
    }

    #[test]
    fn an_unknown_at_rule_without_a_block_is_kept_too() {
        let sheet = parse_stylesheet("@import url(other.css);");
        match sheet.rules() {
            [Rule::Unknown(rule)] => {
                assert_eq!(&*rule.name, "import");
                assert_eq!(rule.prelude, "url(other.css)");
                assert_eq!(rule.block, None);
                assert_eq!(rule.to_string(), "@import url(other.css);");
            }
            other => panic!("expected one unknown at-rule, got {other:?}"),
        }
    }

    #[test]
    fn a_media_rule_holds_the_rules_inside_it() {
        let sheet = parse_stylesheet(
            "@media screen and (min-width: 600px) { a { color: red } b { color: blue } }",
        );
        match sheet.rules() {
            [Rule::Media(rule)] => {
                assert_eq!(rule.queries.to_string(), "screen and (min-width: 600px)");
                assert_eq!(rule.rules.len(), 2);
            }
            other => panic!("expected one media rule, got {other:?}"),
        }
        assert!(sheet.issues().is_empty());
    }

    #[test]
    fn a_bad_rule_inside_a_media_block_does_not_take_the_block_with_it() {
        let sheet = parse_stylesheet("@media screen { a:has(b) { color: red } b { color: blue } }");
        match sheet.rules() {
            [Rule::Media(rule)] => assert_eq!(rule.rules.len(), 1),
            other => panic!("expected one media rule, got {other:?}"),
        }
        assert_eq!(sheet.issues().len(), 1);
        assert_eq!(sheet.issues()[0].kind, IssueKind::InvalidSelector);
    }

    #[test]
    fn an_issue_says_which_line_it_was_on() {
        let sheet = parse_stylesheet("a { color: red }\n\np:has(b) { color: blue }");
        assert_eq!(sheet.issues().len(), 1);
        assert_eq!(sheet.issues()[0].at.line, 3);
    }

    #[test]
    fn an_empty_sheet_and_a_sheet_of_whitespace_both_parse_to_nothing() {
        for text in ["", "   \n\t ", "/* just a comment */"] {
            let sheet = parse_stylesheet(text);
            assert!(sheet.rules().is_empty(), "{text:?}");
            assert!(sheet.issues().is_empty(), "{text:?}");
        }
    }

    #[test]
    fn a_truncated_sheet_keeps_what_came_before_it() {
        let sheet = parse_stylesheet("a { color: red } b { color:");
        assert_eq!(sheet.style_rules_for(&MediaContext::default()).len(), 2);
    }

    #[test]
    fn a_declaration_with_nothing_after_the_colon_is_kept_as_empty() {
        let block = only_declarations("a { color: ; margin: 0 }");
        assert_eq!(
            block.get(&PropertyName::parse("color")).map(|d| &*d.value),
            Some(""),
        );
    }
}

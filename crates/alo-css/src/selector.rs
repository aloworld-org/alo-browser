//! What a selector is in this engine.
//!
//! Selector syntax and matching semantics are rented from the `selectors`
//! crate: they are specification, they are hard to get right, and getting them
//! right carries none of this engine's value (ADR 0001). What is ours is the
//! decision about *which* selectors exist here — the modern subset — and what
//! each pseudo-class is allowed to mean in a renderer with no scripting and no
//! input.
//!
//! Matching a selector against our document tree is [`crate::matching`]; this
//! file is only what a selector *is*.

use crate::ident::Ident;
use core::fmt;
use cssparser::{Parser as CssParser, SourceLocation, ToCss};
use selectors::parser::{
    NonTSPseudoClass as NonTSPseudoClassTrait, ParseRelative, PseudoElement as PseudoElementTrait,
    SelectorParseErrorKind,
};

/// The selector dialect this engine speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AloSelectors;

impl selectors::SelectorImpl for AloSelectors {
    type ExtraMatchingData<'a> = ();
    type AttrValue = Ident;
    type Identifier = Ident;
    type LocalName = Ident;
    type NamespaceUrl = Ident;
    type NamespacePrefix = Ident;
    type BorrowedNamespaceUrl = str;
    type BorrowedLocalName = str;
    type NonTSPseudoClass = PseudoClass;
    type PseudoElement = PseudoElement;
}

/// The pseudo-classes that are not tree-structural — the ones about state
/// rather than position.
///
/// The tree-structural ones (`:first-child`, `:nth-child`, `:only-of-type`,
/// `:empty`, `:root`, `:not`, `:is`, `:where`) are not here: the `selectors`
/// crate evaluates those itself against the tree we give it.
///
/// Anything not in this list makes the selector invalid and the rule is
/// dropped with an issue recorded, which is what CSS says to do and is far
/// better than a rule that matches something nobody meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoClass {
    /// `:link` — a hyperlink with an `href`.
    Link,
    /// `:any-link` — the same, and the one to prefer.
    AnyLink,
    /// `:visited`. **Never matches, deliberately and permanently.** Whether a
    /// link has been visited is history, and a style that depends on it is
    /// readable from the page. Every engine made this decision; we make it at
    /// the start rather than after the bug report.
    Visited,
    /// `:hover`.
    Hover,
    /// `:active`.
    Active,
    /// `:focus`.
    Focus,
    /// `:focus-visible`.
    FocusVisible,
    /// `:focus-within`.
    FocusWithin,
    /// `:target`.
    Target,
    /// `:disabled`.
    Disabled,
    /// `:enabled`.
    Enabled,
    /// `:checked`.
    Checked,
    /// `:required`.
    Required,
    /// `:optional`.
    Optional,
    /// `:read-only`.
    ReadOnly,
    /// `:read-write`.
    ReadWrite,
}

impl PseudoClass {
    /// The pseudo-class a name spells, if this engine has one.
    pub fn from_name(name: &str) -> Option<Self> {
        match_ignoring_ascii_case(name)
    }

    /// The name, with its leading colon.
    pub fn as_str(self) -> &'static str {
        match self {
            PseudoClass::Link => ":link",
            PseudoClass::AnyLink => ":any-link",
            PseudoClass::Visited => ":visited",
            PseudoClass::Hover => ":hover",
            PseudoClass::Active => ":active",
            PseudoClass::Focus => ":focus",
            PseudoClass::FocusVisible => ":focus-visible",
            PseudoClass::FocusWithin => ":focus-within",
            PseudoClass::Target => ":target",
            PseudoClass::Disabled => ":disabled",
            PseudoClass::Enabled => ":enabled",
            PseudoClass::Checked => ":checked",
            PseudoClass::Required => ":required",
            PseudoClass::Optional => ":optional",
            PseudoClass::ReadOnly => ":read-only",
            PseudoClass::ReadWrite => ":read-write",
        }
    }

    /// Whether this pseudo-class describes a state a person is producing right
    /// now — hovering, focusing, activating.
    ///
    /// **Stage 1 has no input**, so none of these can be true of a document we
    /// are asked to render. They parse, because a style sheet that mentions
    /// `:hover` must not be thrown away, and they match nothing until there is
    /// input to match on.
    pub fn is_interaction_state(self) -> bool {
        matches!(
            self,
            PseudoClass::Hover
                | PseudoClass::Active
                | PseudoClass::Focus
                | PseudoClass::FocusVisible
                | PseudoClass::FocusWithin
                | PseudoClass::Target
        )
    }

    /// Whether this pseudo-class can never be true of a document stage 1 is
    /// asked to render.
    ///
    /// Two reasons, one answer. `:visited` is refused permanently: whether a
    /// link has been followed is history, and a style that depends on it is
    /// readable back off the page. The interaction states are refused for now:
    /// nobody is hovering a document being rendered to a file, and they become
    /// answerable when there is input to answer them with.
    pub fn never_matches_in_stage_one(self) -> bool {
        self == PseudoClass::Visited || self.is_interaction_state()
    }
}

fn match_ignoring_ascii_case(name: &str) -> Option<PseudoClass> {
    const ALL: &[PseudoClass] = &[
        PseudoClass::Link,
        PseudoClass::AnyLink,
        PseudoClass::Visited,
        PseudoClass::Hover,
        PseudoClass::Active,
        PseudoClass::Focus,
        PseudoClass::FocusVisible,
        PseudoClass::FocusWithin,
        PseudoClass::Target,
        PseudoClass::Disabled,
        PseudoClass::Enabled,
        PseudoClass::Checked,
        PseudoClass::Required,
        PseudoClass::Optional,
        PseudoClass::ReadOnly,
        PseudoClass::ReadWrite,
    ];
    ALL.iter()
        .copied()
        .find(|candidate| candidate.as_str()[1..].eq_ignore_ascii_case(name))
}

impl ToCss for PseudoClass {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        dest.write_str(self.as_str())
    }
}

impl NonTSPseudoClassTrait for PseudoClass {
    type Impl = AloSelectors;

    fn is_active_or_hover(&self) -> bool {
        matches!(self, PseudoClass::Active | PseudoClass::Hover)
    }

    fn is_user_action_state(&self) -> bool {
        matches!(
            self,
            PseudoClass::Active | PseudoClass::Hover | PseudoClass::Focus
        )
    }
}

/// The pseudo-elements a style sheet may name.
///
/// **None of them is produced in stage 1.** They are parsed so that a rule
/// naming one does not take the rest of the style sheet down with it, and the
/// rule is recorded as targeting something that does not exist rather than
/// silently doing nothing. Generated content and selection painting are not in
/// `docs/features.md`; when they arrive, the selectors are already understood.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoElement {
    /// `::before`.
    Before,
    /// `::after`.
    After,
    /// `::marker`.
    Marker,
    /// `::placeholder`.
    Placeholder,
    /// `::selection`.
    Selection,
    /// `::first-line`.
    FirstLine,
    /// `::first-letter`.
    FirstLetter,
}

impl PseudoElement {
    /// The pseudo-element a name spells, if this engine knows it.
    pub fn from_name(name: &str) -> Option<Self> {
        const ALL: &[PseudoElement] = &[
            PseudoElement::Before,
            PseudoElement::After,
            PseudoElement::Marker,
            PseudoElement::Placeholder,
            PseudoElement::Selection,
            PseudoElement::FirstLine,
            PseudoElement::FirstLetter,
        ];
        ALL.iter()
            .copied()
            .find(|candidate| candidate.as_str()[2..].eq_ignore_ascii_case(name))
    }

    /// The name, with its two leading colons.
    pub fn as_str(self) -> &'static str {
        match self {
            PseudoElement::Before => "::before",
            PseudoElement::After => "::after",
            PseudoElement::Marker => "::marker",
            PseudoElement::Placeholder => "::placeholder",
            PseudoElement::Selection => "::selection",
            PseudoElement::FirstLine => "::first-line",
            PseudoElement::FirstLetter => "::first-letter",
        }
    }
}

impl ToCss for PseudoElement {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        dest.write_str(self.as_str())
    }
}

impl PseudoElementTrait for PseudoElement {
    type Impl = AloSelectors;

    fn accepts_state_pseudo_classes(&self) -> bool {
        // `::selection:hover` and the like. Stage 1 matches neither half, and
        // saying yes here keeps such a selector parseable rather than fatal.
        true
    }

    fn valid_after_slotted(&self) -> bool {
        // There is no shadow DOM here, so `::slotted()` never parses and
        // nothing can follow it.
        false
    }
}

/// How specific a selector is, unpacked into the three counts CSS defines.
///
/// The `selectors` crate keeps this as one packed number. The cascade compares
/// them as a triple and a test wants to read them as one, so this is where the
/// packing stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Specificity {
    /// How many id selectors — the `a` of `a,b,c`.
    pub ids: u32,
    /// How many class, attribute and pseudo-class selectors — the `b`.
    pub classes: u32,
    /// How many type and pseudo-element selectors — the `c`.
    pub elements: u32,
}

/// Ten bits per count, which is what the `selectors` crate packs into.
const COUNT_MASK: u32 = (1 << 10) - 1;

impl Specificity {
    /// Unpack the number the `selectors` crate computed.
    fn from_packed(packed: u32) -> Self {
        Self {
            ids: packed >> 20,
            classes: (packed >> 10) & COUNT_MASK,
            elements: packed & COUNT_MASK,
        }
    }
}

impl fmt::Display for Specificity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.ids, self.classes, self.elements)
    }
}

/// One selector, as written, with what the cascade needs to know about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    inner: selectors::parser::Selector<AloSelectors>,
}

impl Selector {
    /// How specific this selector is.
    pub fn specificity(&self) -> Specificity {
        Specificity::from_packed(self.inner.specificity())
    }

    /// The pseudo-element this selector targets, if it targets one.
    ///
    /// A selector that targets a pseudo-element matches nothing in stage 1:
    /// there is no box for it to match. See [`PseudoElement`].
    pub fn pseudo_element(&self) -> Option<PseudoElement> {
        self.inner.pseudo_element().copied()
    }

    /// The selector as CSS text.
    pub fn to_css_string(&self) -> String {
        self.inner.to_css_string()
    }

    pub(crate) fn inner(&self) -> &selectors::parser::Selector<AloSelectors> {
        &self.inner
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_css_string())
    }
}

/// A comma-separated list of selectors, as a rule's prelude is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorList {
    selectors: Vec<Selector>,
}

impl SelectorList {
    /// Parse a selector list from the input at the parser's position.
    ///
    /// # Errors
    ///
    /// Returns the parse error if any selector in the list is one this engine
    /// does not have. CSS says an invalid selector invalidates the whole list,
    /// and the caller drops the rule and records why.
    pub fn parse<'i>(
        input: &mut CssParser<'i, '_>,
    ) -> Result<Self, cssparser::ParseError<'i, SelectorParseErrorKind<'i>>> {
        let parsed = selectors::parser::SelectorList::parse(
            &SelectorParser,
            input,
            // A style sheet's selectors stand on their own. Nesting and `:has`
            // relative selectors are stage 2 (`docs/features.md`).
            ParseRelative::No,
        )?;
        Ok(Self {
            selectors: parsed
                .slice()
                .iter()
                .cloned()
                .map(|inner| Selector { inner })
                .collect(),
        })
    }

    /// The selectors in the list, in the order they were written.
    pub fn iter(&self) -> core::slice::Iter<'_, Selector> {
        self.selectors.iter()
    }

    /// How many selectors are in the list.
    pub fn len(&self) -> usize {
        self.selectors.len()
    }

    /// Whether the list is empty. It never is when it came from `parse`.
    pub fn is_empty(&self) -> bool {
        self.selectors.is_empty()
    }
}

impl<'a> IntoIterator for &'a SelectorList {
    type Item = &'a Selector;
    type IntoIter = core::slice::Iter<'a, Selector>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for SelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, selector) in self.selectors.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{selector}")?;
        }
        Ok(())
    }
}

/// The decisions about which selectors this engine has.
struct SelectorParser;

impl<'i> selectors::parser::Parser<'i> for SelectorParser {
    type Impl = AloSelectors;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_is_and_where(&self) -> bool {
        // Modern, widely used, and free: the `selectors` crate evaluates them.
        true
    }

    fn parse_nth_child_of(&self) -> bool {
        true
    }

    fn parse_has(&self) -> bool {
        // `docs/features.md` puts `:has()` in stage 2. It is not free — it
        // inverts the direction matching runs in — and stage 1 does not need it.
        false
    }

    fn parse_parent_selector(&self) -> bool {
        // Nesting is stage 2 as well.
        false
    }

    fn parse_slotted(&self) -> bool {
        false
    }

    fn parse_part(&self) -> bool {
        false
    }

    fn parse_host(&self) -> bool {
        false
    }

    fn allow_forgiving_selectors(&self) -> bool {
        // Inside `:is()` and `:where()`, an unknown selector is dropped and the
        // rest of the list still works. That is what the specification says,
        // and it is the behaviour that keeps a style sheet written for several
        // engines working in this one.
        true
    }

    fn parse_non_ts_pseudo_class(
        &self,
        location: SourceLocation,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<PseudoClass, cssparser::ParseError<'i, Self::Error>> {
        PseudoClass::from_name(&name).ok_or_else(|| {
            location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
                name,
            ))
        })
    }

    fn parse_pseudo_element(
        &self,
        location: SourceLocation,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<PseudoElement, cssparser::ParseError<'i, Self::Error>> {
        PseudoElement::from_name(&name).ok_or_else(|| {
            location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
                name,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::ParserInput;

    fn parse(text: &str) -> Option<SelectorList> {
        let mut input = ParserInput::new(text);
        SelectorList::parse(&mut CssParser::new(&mut input)).ok()
    }

    fn specificity(text: &str) -> Specificity {
        parse(text)
            .and_then(|list| list.iter().next().map(Selector::specificity))
            .unwrap_or_else(|| panic!("{text} should parse"))
    }

    #[test]
    fn specificity_counts_ids_classes_and_elements_separately() {
        assert_eq!(specificity("*").to_string(), "(0, 0, 0)");
        assert_eq!(specificity("li").to_string(), "(0, 0, 1)");
        assert_eq!(specificity("ul li").to_string(), "(0, 0, 2)");
        assert_eq!(specificity(".lead").to_string(), "(0, 1, 0)");
        assert_eq!(specificity("[type=\"text\"]").to_string(), "(0, 1, 0)");
        assert_eq!(specificity("li:first-child").to_string(), "(0, 1, 1)");
        assert_eq!(specificity("#main").to_string(), "(1, 0, 0)");
        assert_eq!(specificity("#main .row a").to_string(), "(1, 1, 1)");
    }

    #[test]
    fn specificity_orders_the_way_the_cascade_needs() {
        assert!(specificity("#main") > specificity(".lead.big.wide"));
        assert!(specificity(".lead") > specificity("ul li a span"));
    }

    #[test]
    fn a_list_keeps_every_selector_in_order() {
        let list = parse("h1, h2 , .lead").expect("a valid list");
        assert_eq!(list.len(), 3);
        assert!(!list.is_empty());
        assert_eq!(list.to_string(), "h1, h2, .lead");
        assert_eq!(
            list.iter().map(Selector::specificity).collect::<Vec<_>>(),
            vec![
                Specificity {
                    ids: 0,
                    classes: 0,
                    elements: 1
                },
                Specificity {
                    ids: 0,
                    classes: 0,
                    elements: 1
                },
                Specificity {
                    ids: 0,
                    classes: 1,
                    elements: 0
                },
            ],
        );
    }

    #[test]
    fn the_modern_combinators_all_parse() {
        for text in [
            "a b",
            "a > b",
            "a + b",
            "a ~ b",
            "a.b#c[d]",
            ":root",
            ":not(.a)",
            ":is(h1, h2)",
            ":where(.a, .b) .c",
            "li:nth-child(2n + 1)",
            "li:nth-last-of-type(odd)",
            "p:empty",
            "input:disabled",
            "a:any-link",
        ] {
            assert!(parse(text).is_some(), "{text} should parse");
        }
    }

    #[test]
    fn what_stage_1_does_not_have_is_refused_rather_than_guessed_at() {
        for text in [
            "a:has(b)",       // features.md: stage 2
            "& .nested",      // nesting: stage 2
            "::slotted(a)",   // no shadow DOM
            "p::-webkit-any", // a vendor prefix, which is stage 3 if ever
            ":totally-made-up",
            "p >",
        ] {
            assert!(parse(text).is_none(), "{text} should not parse");
        }
    }

    #[test]
    fn a_pseudo_element_parses_and_says_which_one_it_is() {
        let list = parse("p::before").expect("a known pseudo-element parses");
        let selector = list.iter().next().expect("one selector");
        assert_eq!(selector.pseudo_element(), Some(PseudoElement::Before));
        assert_eq!(selector.to_css_string(), "p::before");

        let plain = parse("p").expect("a plain selector");
        assert_eq!(plain.iter().next().and_then(Selector::pseudo_element), None,);
    }

    #[test]
    fn a_forgiving_list_keeps_the_half_it_understands() {
        let list = parse(":is(h1, :not-a-real-thing) .x").expect("`:is` is forgiving");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn every_pseudo_class_name_round_trips() {
        for text in [
            ":link",
            ":any-link",
            ":visited",
            ":hover",
            ":active",
            ":focus",
            ":focus-visible",
            ":focus-within",
            ":target",
            ":disabled",
            ":enabled",
            ":checked",
            ":required",
            ":optional",
            ":read-only",
            ":read-write",
        ] {
            let class = PseudoClass::from_name(&text[1..]).expect("a name we listed");
            assert_eq!(class.as_str(), text);
            assert_eq!(class.to_css_string(), text);
        }
        assert_eq!(PseudoClass::from_name("HOVER"), Some(PseudoClass::Hover));
        assert_eq!(PseudoClass::from_name("nonsense"), None);
    }

    #[test]
    fn every_pseudo_element_name_round_trips() {
        for text in [
            "::before",
            "::after",
            "::marker",
            "::placeholder",
            "::selection",
            "::first-line",
            "::first-letter",
        ] {
            let element = PseudoElement::from_name(&text[2..]).expect("a name we listed");
            assert_eq!(element.as_str(), text);
            assert_eq!(element.to_css_string(), text);
        }
        assert_eq!(
            PseudoElement::from_name("BEFORE"),
            Some(PseudoElement::Before),
        );
        assert_eq!(PseudoElement::from_name("nonsense"), None);
    }

    #[test]
    fn the_interaction_states_are_named_as_such() {
        assert!(PseudoClass::Hover.is_interaction_state());
        assert!(PseudoClass::FocusWithin.is_interaction_state());
        assert!(!PseudoClass::Disabled.is_interaction_state());
        assert!(!PseudoClass::Visited.is_interaction_state());
    }

    #[test]
    fn what_can_never_match_is_the_interaction_states_and_visited() {
        assert!(PseudoClass::Visited.never_matches_in_stage_one());
        assert!(PseudoClass::Hover.never_matches_in_stage_one());
        assert!(PseudoClass::Target.never_matches_in_stage_one());
        assert!(!PseudoClass::Disabled.never_matches_in_stage_one());
        assert!(!PseudoClass::AnyLink.never_matches_in_stage_one());
    }
}

//! Matching a selector against our document tree.
//!
//! The `selectors` crate walks selectors; this tells it what our tree looks
//! like. The adapter is the whole of the coupling between CSS and the DOM in
//! this engine, which is why it is one file: when the box tree arrives and
//! wants to be matched against as well, this is the file that learns about it.
//!
//! Two refusals are written into the adapter rather than left to chance:
//!
//! - **A selector that names a pseudo-element never matches.** Stage 1
//!   produces no box for one, so there is nothing it could match, and the
//!   style sheet was told so when it was parsed.
//! - **An interaction state never matches.** Nobody is hovering a document
//!   being rendered to a PNG. `:hover` parses so that a style sheet survives;
//!   it matches when there is input, which is not stage 1.

use crate::selector::{AloSelectors, PseudoClass, PseudoElement, Selector, SelectorList};
use crate::state;
use alo_dom::{Document, Element, NodeId};
use core::fmt;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches,
};
use selectors::matching::ElementSelectorFlags;
use selectors::{Element as SelectorsElement, OpaqueElement};

/// One element of one document, as the selector machinery sees it.
#[derive(Clone, Copy)]
struct ElementRef<'a> {
    document: &'a Document,
    id: NodeId,
    element: &'a Element,
}

impl<'a> ElementRef<'a> {
    /// The element a node id names, or [`None`] if it names something that is
    /// not an element — a text node, a comment, the document itself.
    fn new(document: &'a Document, id: NodeId) -> Option<Self> {
        Some(Self {
            document,
            id,
            element: document.element(id)?,
        })
    }

    /// The nearest preceding sibling that is an element.
    fn sibling(self, forwards: bool) -> Option<Self> {
        let mut current = if forwards {
            self.document.next_sibling(self.id)
        } else {
            self.document.previous_sibling(self.id)
        };
        while let Some(id) = current {
            if let Some(found) = Self::new(self.document, id) {
                return Some(found);
            }
            current = if forwards {
                self.document.next_sibling(id)
            } else {
                self.document.previous_sibling(id)
            };
        }
        None
    }
}

impl fmt::Debug for ElementRef<'_> {
    /// The element and its id — never the whole document, which is what
    /// deriving this would print for every element of every match.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} {}>", self.element.name, self.id)
    }
}

impl SelectorsElement for ElementRef<'_> {
    type Impl = AloSelectors;

    fn opaque(&self) -> OpaqueElement {
        // Stable for as long as the document is: every element is owned by the
        // document's arena and nothing moves while matching runs.
        OpaqueElement::new(self.element)
    }

    fn parent_element(&self) -> Option<Self> {
        let parent = self.document.parent(self.id)?;
        Self::new(self.document, parent)
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        // There is no shadow DOM here; `docs/features.md` does not list one.
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.sibling(false)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.sibling(true)
    }

    fn first_element_child(&self) -> Option<Self> {
        self.document
            .children(self.id)
            .find_map(|child| Self::new(self.document, child))
    }

    fn is_html_element_in_html_document(&self) -> bool {
        // Every document this engine parses is an HTML document, so this asks
        // only whether the element is in the HTML namespace — an inline
        // `<svg>` is not, and its names stay case sensitive.
        self.element.name.ns == alo_dom::Namespace::Html
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        &*self.element.name.local == local_name
    }

    fn has_namespace(&self, ns: &str) -> bool {
        self.element.name.ns.as_str() == ns
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.element.name.local == other.element.name.local
            && self.element.name.ns == other.element.name.ns
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&crate::Ident>,
        local_name: &crate::Ident,
        operation: &AttrSelectorOperation<&crate::Ident>,
    ) -> bool {
        self.element.attrs.iter().any(|attr| {
            let namespace_matches = match ns {
                NamespaceConstraint::Any => true,
                NamespaceConstraint::Specific(url) => attr.name.ns.as_str() == url.as_str(),
            };
            namespace_matches
                && &*attr.name.local == local_name.as_str()
                && operation.eval_str(&attr.value)
        })
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &PseudoClass,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        match pc {
            PseudoClass::Link | PseudoClass::AnyLink => state::is_link(self.element),
            PseudoClass::Disabled => state::is_disabled(self.document, self.id, self.element),
            PseudoClass::Enabled => state::is_enabled(self.document, self.id, self.element),
            PseudoClass::Checked => state::is_checked(self.element),
            PseudoClass::Required => state::is_required(self.element),
            PseudoClass::Optional => state::is_optional(self.element),
            PseudoClass::ReadWrite => state::is_read_write(self.document, self.id, self.element),
            PseudoClass::ReadOnly => !state::is_read_write(self.document, self.id, self.element),
            // `:visited` by decision and the interaction states for now — see
            // `PseudoClass::never_matches_in_stage_one`, which is where the
            // two reasons are written down. Listing them here rather than
            // catching all makes adding a pseudo-class a compile error, which
            // is how one stops being answered by accident.
            _ if pc.never_matches_in_stage_one() => false,
            PseudoClass::Visited
            | PseudoClass::Hover
            | PseudoClass::Active
            | PseudoClass::Focus
            | PseudoClass::FocusVisible
            | PseudoClass::FocusWithin
            | PseudoClass::Target => false,
        }
    }

    fn match_pseudo_element(
        &self,
        _pe: &PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        // Stage 1 produces no boxes for pseudo-elements, so there is nothing
        // here to match. The style sheet was told when it was parsed.
        false
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {
        // These mark where a later DOM change would need styles recomputed.
        // Stage 1's tree does not change after it is parsed — mutation is a
        // stage 2 feature — so there is nothing to remember.
    }

    fn is_link(&self) -> bool {
        state::is_link(self.element)
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &crate::Ident, case_sensitivity: CaseSensitivity) -> bool {
        self.element
            .attr("id")
            .is_some_and(|value| case_sensitivity.eq(value.as_bytes(), id.as_str().as_bytes()))
    }

    fn has_class(&self, name: &crate::Ident, case_sensitivity: CaseSensitivity) -> bool {
        self.element.attr("class").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|class| case_sensitivity.eq(class.as_bytes(), name.as_str().as_bytes()))
        })
    }

    fn has_custom_state(&self, _name: &crate::Ident) -> bool {
        // Custom states come from scripting, which stage 1 does not have.
        false
    }

    fn imported_part(&self, _name: &crate::Ident) -> Option<crate::Ident> {
        None
    }

    fn is_part(&self, _name: &crate::Ident) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        !self.document.children(self.id).any(|child| {
            self.document.get(child).is_some_and(|node| {
                node.element().is_some() || node.text().is_some_and(|text| !text.is_empty())
            })
        })
    }

    fn is_root(&self) -> bool {
        self.document.parent(self.id) == Some(self.document.root())
    }

    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        // The bloom filter is an optimisation for skipping ancestors that
        // cannot match. Nothing here builds one, so there is nothing to add
        // and saying so is the honest answer.
        false
    }
}

/// Matching selectors against one document.
///
/// Holding this across several matches is worth doing: the selector machinery
/// caches work between them — `:nth-child` counting in particular — and a
/// fresh context throws that away.
pub struct MatchContext<'a> {
    document: &'a Document,
    caches: SelectorCaches,
}

impl<'a> MatchContext<'a> {
    /// A context for matching against this document.
    pub fn new(document: &'a Document) -> Self {
        Self {
            document,
            caches: SelectorCaches::default(),
        }
    }

    /// Whether one selector matches one node.
    ///
    /// [`false`] for a node that is not an element, and for a selector that
    /// names a pseudo-element.
    pub fn matches(&mut self, selector: &Selector, id: NodeId) -> bool {
        if selector.pseudo_element().is_some() {
            return false;
        }
        let Some(element) = ElementRef::new(self.document, id) else {
            return false;
        };
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut self.caches,
            // Law 1: there is no quirks mode. `alo_dom` records what the
            // parser thought and this never reads it.
            QuirksMode::NoQuirks,
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        selectors::matching::matches_selector(selector.inner(), 0, None, &element, &mut context)
    }

    /// The most specific selector in a list that matches this node, if any.
    ///
    /// The cascade needs the specificity of the selector that actually
    /// matched, not of the list it was written in — `h1, #title` contributes
    /// its declarations at `(1, 0, 0)` to an `<h1 id=title>` and at
    /// `(0, 0, 1)` to any other heading. Ties go to the first, so the answer
    /// does not depend on iteration order.
    pub fn most_specific_match<'l>(
        &mut self,
        list: &'l SelectorList,
        id: NodeId,
    ) -> Option<&'l Selector> {
        // `min_by_key` over a reversed key is the greatest specificity, and it
        // returns the *first* of equals — which is what makes the answer
        // depend on the sheet rather than on iteration order.
        list.iter()
            .filter(|selector| self.matches(selector, id))
            .min_by_key(|selector| core::cmp::Reverse(selector.specificity()))
    }
}

/// Whether any selector in a list matches a node.
///
/// Convenient for one question; for many, make a [`MatchContext`] and keep it.
pub fn matches(document: &Document, id: NodeId, list: &SelectorList) -> bool {
    let mut context = MatchContext::new(document);
    list.iter().any(|selector| context.matches(selector, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selector::SelectorList;
    use alo_dom::parse_document;
    use cssparser::{Parser as CssParser, ParserInput};

    fn selectors(text: &str) -> SelectorList {
        let mut input = ParserInput::new(text);
        SelectorList::parse(&mut CssParser::new(&mut input))
            .unwrap_or_else(|_| panic!("{text} should parse"))
    }

    /// Every element the selector matches, by its `id` attribute.
    fn matched(html: &str, selector: &str) -> Vec<String> {
        let document = parse_document(html);
        let list = selectors(selector);
        let mut context = MatchContext::new(&document);
        document
            .descendants(document.root())
            .filter(|id| list.iter().any(|s| context.matches(s, *id)))
            .filter_map(|id| {
                document
                    .element(id)
                    .and_then(|element| element.attr("id"))
                    .map(str::to_owned)
            })
            .collect()
    }

    const LIST: &str = "<ul id=list class='rows dense'>\
         <li id=one class=row>one</li>\
         <li id=two class='row selected'>two</li>\
         <li id=three class=row><a id=link href='/x'>three</a></li>\
         </ul>";

    #[test]
    fn a_type_selector_matches_by_name() {
        assert_eq!(matched(LIST, "li"), vec!["one", "two", "three"]);
        assert_eq!(matched(LIST, "ul"), vec!["list"]);
        assert!(matched(LIST, "table").is_empty());
    }

    #[test]
    fn class_and_id_selectors_match_what_they_name() {
        assert_eq!(matched(LIST, ".selected"), vec!["two"]);
        assert_eq!(matched(LIST, "#three"), vec!["three"]);
        assert_eq!(matched(LIST, ".rows.dense"), vec!["list"]);
        assert!(matched(LIST, ".rows.missing").is_empty());
    }

    #[test]
    fn combinators_walk_the_tree_the_way_they_say() {
        assert_eq!(matched(LIST, "ul li"), vec!["one", "two", "three"]);
        assert_eq!(matched(LIST, "ul > li"), vec!["one", "two", "three"]);
        assert_eq!(matched(LIST, "ul > a"), Vec::<String>::new());
        assert_eq!(matched(LIST, "li a"), vec!["link"]);
        assert_eq!(matched(LIST, "#one + li"), vec!["two"]);
        assert_eq!(matched(LIST, "#one ~ li"), vec!["two", "three"]);
    }

    #[test]
    fn the_tree_structural_pseudo_classes_count_elements_not_nodes() {
        assert_eq!(matched(LIST, "li:first-child"), vec!["one"]);
        assert_eq!(matched(LIST, "li:last-child"), vec!["three"]);
        assert_eq!(matched(LIST, "li:nth-child(2)"), vec!["two"]);
        assert_eq!(matched(LIST, "li:nth-child(odd)"), vec!["one", "three"]);
        assert_eq!(matched(LIST, "a:only-child"), vec!["link"]);
        assert_eq!(matched(LIST, "html:root"), vec![] as Vec<String>);
    }

    #[test]
    fn root_is_the_document_element() {
        let document = parse_document("<html id=doc><body id=body></body></html>");
        let list = selectors(":root");
        let mut context = MatchContext::new(&document);
        let matches: Vec<_> = document
            .descendants(document.root())
            .filter(|id| list.iter().any(|s| context.matches(s, *id)))
            .filter_map(|id| document.element(id).map(|e| e.name.local.to_string()))
            .collect();
        assert_eq!(matches, vec!["html"]);
    }

    #[test]
    fn empty_means_no_elements_and_no_text() {
        let html = "<p id=a></p><p id=b> </p><p id=c><span></span></p><p id=d>x</p>";
        assert_eq!(matched(html, "p:empty"), vec!["a"]);
    }

    #[test]
    fn not_and_is_and_where_all_work() {
        assert_eq!(matched(LIST, "li:not(.selected)"), vec!["one", "three"]);
        assert_eq!(matched(LIST, ":is(#one, #two)"), vec!["one", "two"]);
        assert_eq!(matched(LIST, ":where(ul) > #one"), vec!["one"]);
    }

    #[test]
    fn attribute_selectors_use_every_operator() {
        let html = "<a id=a href='/docs/one'></a><a id=b href='https://x/y'></a>\
             <p id=c lang='en-GB'></p><p id=d data-tags='alpha beta'></p>";
        assert_eq!(matched(html, "[href]"), vec!["a", "b"]);
        assert_eq!(matched(html, "[href='/docs/one']"), vec!["a"]);
        assert_eq!(matched(html, "[href^='https']"), vec!["b"]);
        assert_eq!(matched(html, "[href$='/y']"), vec!["b"]);
        assert_eq!(matched(html, "[href*='docs']"), vec!["a"]);
        assert_eq!(matched(html, "[lang|='en']"), vec!["c"]);
        assert_eq!(matched(html, "[data-tags~='beta']"), vec!["d"]);
    }

    #[test]
    fn an_attribute_value_can_be_matched_without_regard_to_case_when_asked() {
        let html = "<p id=a data-x='VALUE'></p>";
        assert!(matched(html, "[data-x='value']").is_empty());
        assert_eq!(matched(html, "[data-x='value' i]"), vec!["a"]);
    }

    #[test]
    fn a_class_is_case_sensitive_and_a_tag_name_is_not() {
        let html = "<p id=a class=Lead></p>";
        assert_eq!(matched(html, ".Lead"), vec!["a"]);
        assert!(matched(html, ".lead").is_empty());
        assert_eq!(matched(html, "P"), vec!["a"]);
    }

    #[test]
    fn state_pseudo_classes_read_the_markup() {
        let html = "<form><input id=a><input id=b disabled><input id=c type=checkbox checked>\
             <input id=d required><textarea id=e readonly></textarea></form>";
        assert_eq!(matched(html, ":disabled"), vec!["b"]);
        assert_eq!(matched(html, ":checked"), vec!["c"]);
        assert_eq!(matched(html, ":required"), vec!["d"]);
        assert_eq!(matched(html, "input:read-write"), vec!["a", "d"]);
        assert_eq!(matched(html, "textarea:read-only"), vec!["e"]);
    }

    #[test]
    fn a_link_matches_any_link_and_never_visited() {
        assert_eq!(matched(LIST, "a:any-link"), vec!["link"]);
        assert_eq!(matched(LIST, "a:link"), vec!["link"]);
        assert!(
            matched(LIST, "a:visited").is_empty(),
            "visitedness is history, and is never readable from a page here",
        );
    }

    #[test]
    fn an_interaction_state_never_matches_because_nobody_is_interacting() {
        for selector in [
            "a:hover",
            "a:focus",
            "a:active",
            "a:focus-visible",
            "a:focus-within",
            "a:target",
        ] {
            assert!(matched(LIST, selector).is_empty(), "{selector}");
        }
    }

    #[test]
    fn a_selector_naming_a_pseudo_element_never_matches() {
        assert!(matched(LIST, "li::before").is_empty());
        assert!(matched(LIST, "li::after").is_empty());
    }

    #[test]
    fn a_node_that_is_not_an_element_matches_nothing() {
        let document = parse_document("<p>text</p>");
        let list = selectors("*");
        let mut context = MatchContext::new(&document);
        let text = document
            .descendants(document.root())
            .find(|id| document.get(*id).is_some_and(|node| node.text().is_some()))
            .expect("a text node");
        assert!(!list.iter().any(|s| context.matches(s, text)));
        assert!(!matches(&document, text, &list));
        assert!(!matches(&document, document.root(), &list));
    }

    #[test]
    fn foreign_content_keeps_its_own_namespace_and_case() {
        let html = "<svg id=s viewBox='0 0 1 1'><circle id=c></circle></svg>";
        assert_eq!(matched(html, "svg"), vec!["s"]);
        assert_eq!(matched(html, "circle"), vec!["c"]);
        assert_eq!(
            matched(html, "[viewBox]"),
            vec!["s"],
            "an attribute on foreign content keeps the case it was written in",
        );
    }

    #[test]
    fn the_convenience_function_asks_the_same_question_as_a_context() {
        let document = parse_document(LIST);
        let list = selectors("li.selected");
        let two = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("two"))
            })
            .expect("the selected row");
        assert!(matches(&document, two, &list));

        let mut context = MatchContext::new(&document);
        assert!(list.iter().any(|s| context.matches(s, two)));
    }
}

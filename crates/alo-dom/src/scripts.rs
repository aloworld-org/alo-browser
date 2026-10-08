/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The scripts a page carries, and the policies written beside them.
//!
//! [`crate::sheets`]'s twin: this reads what the markup says and runs
//! nothing. Which of these runs, and whether the page's policy lets it, is the
//! renderer's (queue item 236); what a script *is* is the engine's.
//!
//! # One list, in document order
//!
//! Scripts and `<meta http-equiv="Content-Security-Policy">` together, for the
//! same reason [`crate::sheets`] keeps `<style>` and `<link>` together: **the
//! order is the meaning.** A policy written in a `<meta>` governs what comes
//! after it and not what came before, so a page whose first script runs
//! before its policy is stated is relying on exactly that, and a page whose
//! policy comes first is relying on the opposite. Two lists would lose which.
//!
//! # What is a script here, by HTML's *prepare the script element*
//!
//! - **The type.** `type`, trimmed; if it is absent or empty, `language`
//!   prefixed with `text/`; if both are absent or empty, a classic script. A
//!   type that is one of HTML's JavaScript MIME type essences, compared
//!   ignoring ASCII case and **with no parameters** — the specification's
//!   *essence match*, so `text/javascript; charset=utf-8` is not one — is a
//!   classic script; `module` is a module; `importmap` is an import map; and
//!   anything else is a **data block**, which is not a script and is left
//!   out.
//! - **`nomodule`** on a classic script means *I am the fallback for an engine
//!   that has no modules*, and HTML has every engine that has modules skip
//!   it. This engine is one that will (queue item 77), so it skips it now
//!   rather than run a page's fallback today and its module tomorrow.
//! - **`src`** makes it a script to fetch, whatever its text says.
//! - **Only HTML's `<script>`.** An SVG `<script>` does not run, and drawing
//!   SVG does not change that (ADR 0022 § 7). A `<script>` inside
//!   `<template>` is inert, as a template's contents are.
//!
//! # The nonce, and when markup cannot be trusted with one
//!
//! A script presents its `nonce` by Content Security Policy's *is element
//! nonceable* ([`crate::nonce`]), which a `<style>` shares: an element whose
//! markup is in a shape an injection leaves presents no nonce at all.
//!
//! # One element at a time, as the parser reaches it
//!
//! [`carried`] reads a whole document. A page whose scripts run reads it a
//! step at a time instead (ADR 0017 § 7): the parser stops at each script's
//! end tag ([`crate::parse::Parsing`]), and [`prepared`] says what that
//! element is, by the same rules — plus HTML's *if the element is not
//! connected, return*, since a script can take the element the parser was
//! inserting into out of the document, and a script the parser then puts
//! there never runs. [`stated`] says what policy a `<meta>` the parser made
//! states, which is checked as the parser makes it rather than when a later
//! script runs: removing a `<meta>` does not take back its policy.
//!
//! # A `<meta>` policy, and where it counts
//!
//! Only a `<meta>` that is a child of `<head>` states a policy — HTML says so
//! in as many words — and only one with a non-empty `content`. Its text is
//! handed on unread: parsing a policy is `alo-net`'s, and so is the rule that
//! a policy this engine cannot read makes things stricter.

use crate::document::Document;
use crate::node::{Element, NodeId};

/// Something in a page that bears on its script: a script, or a policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carried {
    /// A `<script>` that is a script rather than a data block.
    Script(Script),
    /// The text of a `<meta http-equiv="Content-Security-Policy">` in `<head>`,
    /// which governs every script after it.
    Policy(String),
}

/// A script a page carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// What kind of script.
    pub kind: Kind,
    /// Where its text is.
    pub source: Source,
    /// The nonce it presents to a policy — absent where it wrote none, and
    /// where its markup is in a shape an injection leaves (see the module).
    pub nonce: Option<String>,
}

/// What kind of script, by its `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A classic script.
    Classic,
    /// A module script.
    Module,
    /// An import map.
    ImportMap,
}

/// Where a script's text is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Written into the element: its child text, untrimmed, because a hash in
    /// a policy is of exactly these characters.
    Written(String),
    /// Somewhere else, named by `src` — exactly as the page wrote it, which may
    /// be empty, and unresolved, because resolving it needs the page's own
    /// address.
    Linked {
        /// The `src`.
        src: String,
    },
}

/// HTML's JavaScript MIME type essences, every one of which means a classic
/// script.
const JAVASCRIPT: [&str; 16] = [
    "application/ecmascript",
    "application/javascript",
    "application/x-ecmascript",
    "application/x-javascript",
    "text/ecmascript",
    "text/javascript",
    "text/javascript1.0",
    "text/javascript1.1",
    "text/javascript1.2",
    "text/javascript1.3",
    "text/javascript1.4",
    "text/javascript1.5",
    "text/jscript",
    "text/livescript",
    "text/x-ecmascript",
    "text/x-javascript",
];

/// Every script and `<meta>` policy a page carries, in document order.
pub fn carried(document: &Document) -> Vec<Carried> {
    let mut found = Vec::new();
    for id in document.descendants(document.root()) {
        let Some(element) = document.element(id) else {
            continue;
        };
        if element.name.is_html("script") {
            if let Some(script) = script(document, id, element) {
                found.push(Carried::Script(script));
            }
        } else if element.name.is_html("meta") {
            if let Some(policy) = meta_policy(document, id, element) {
                found.push(Carried::Policy(policy));
            }
        }
    }
    found
}

/// What the `<script>` element `id` is, as the parser reaches its end tag —
/// or [`None`] if it is not a script to run: not an HTML `<script>`, not in
/// the document (inside a `<template>`, or in a tree a script took out of
/// it), a data block, a classic script marked `nomodule`, or empty.
pub fn prepared(document: &Document, id: NodeId) -> Option<Script> {
    let element = document.element(id)?;
    if !element.name.is_html("script") || !document.is_attached(id) {
        return None;
    }
    script(document, id, element)
}

/// The policy the `<meta>` element `id` states, if it is a Content Security
/// Policy in `<head>` with something in it, and in the document — a
/// `<head>` a script took out is not. Asked as the parser makes it: HTML
/// checks where the element is when it is inserted.
pub fn stated(document: &Document, id: NodeId) -> Option<String> {
    let element = document.element(id)?;
    if !element.name.is_html("meta") || !document.is_attached(id) {
        return None;
    }
    meta_policy(document, id, element)
}

/// What a `<script>` element is, or [`None`] if it is not a script to run.
fn script(document: &Document, id: NodeId, element: &Element) -> Option<Script> {
    let kind = kind(element)?;
    if kind == Kind::Classic && element.attr("nomodule").is_some() {
        return None;
    }
    let source = if let Some(src) = element.attr("src") {
        Source::Linked {
            src: src.to_owned(),
        }
    } else {
        let text = child_text(document, id);
        // HTML: no `src` and no text is nothing to prepare. Whitespace is a
        // script — an empty one, which a hash can still name.
        if text.is_empty() {
            return None;
        }
        Source::Written(text)
    };
    Some(Script {
        kind,
        source,
        nonce: crate::nonce::presented(element),
    })
}

/// The kind of script a `<script>`'s `type` and `language` make it, or
/// [`None`] for a data block.
fn kind(element: &Element) -> Option<Kind> {
    let written = match element.attr("type") {
        Some(kind) if !kind.is_empty() => kind.to_owned(),
        _ => match element.attr("language") {
            Some(language) if !language.is_empty() => format!("text/{language}"),
            _ => return Some(Kind::Classic),
        },
    };
    let kind = written.trim_matches(is_ascii_whitespace);
    if JAVASCRIPT
        .iter()
        .any(|essence| essence.eq_ignore_ascii_case(kind))
    {
        Some(Kind::Classic)
    } else if kind.eq_ignore_ascii_case("module") {
        Some(Kind::Module)
    } else if kind.eq_ignore_ascii_case("importmap") {
        Some(Kind::ImportMap)
    } else {
        None
    }
}

/// HTML's ASCII whitespace: tab, line feed, form feed, carriage return, space.
fn is_ascii_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{c}' | '\r' | ' ')
}

/// The text of a node's own text children, joined — HTML's *child text
/// content*, which is what a script's source is.
fn child_text(document: &Document, id: NodeId) -> String {
    document
        .children(id)
        .filter_map(|child| document.get(child).and_then(|node| node.text()))
        .collect()
}

/// The policy a `<meta>` states, if it is a Content Security Policy in
/// `<head>` with something in it.
fn meta_policy(document: &Document, id: NodeId, element: &Element) -> Option<String> {
    let says = element.attr("http-equiv")?;
    if !says
        .trim_matches(is_ascii_whitespace)
        .eq_ignore_ascii_case("content-security-policy")
    {
        return None;
    }
    let in_head = document
        .parent(id)
        .and_then(|parent| document.element(parent))
        .is_some_and(|parent| parent.name.is_html("head"));
    if !in_head {
        return None;
    }
    let content = element.attr("content")?;
    if content.is_empty() {
        None
    } else {
        Some(content.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_document;

    fn of(html: &str) -> Vec<Carried> {
        carried(&parse_document(html))
    }

    fn written(text: &str, nonce: Option<&str>) -> Carried {
        Carried::Script(Script {
            kind: Kind::Classic,
            source: Source::Written(text.to_owned()),
            nonce: nonce.map(ToOwned::to_owned),
        })
    }

    #[test]
    fn scripts_and_policies_come_in_document_order() {
        assert_eq!(
            of("<head><script>a</script>\
                <meta http-equiv=Content-Security-Policy content=\"script-src 'none'\">\
                </head><body><script>b</script>"),
            vec![
                written("a", None),
                Carried::Policy("script-src 'none'".to_owned()),
                written("b", None),
            ],
        );
    }

    #[test]
    fn every_javascript_type_is_classic_and_parameters_make_a_data_block() {
        for kind in [
            "",
            "text/javascript",
            " TEXT/JavaScript\n",
            "application/ecmascript",
            "text/livescript",
        ] {
            assert_eq!(
                of(&format!("<script type='{kind}'>x</script>")),
                vec![written("x", None)],
                "{kind:?}",
            );
        }
        for kind in [
            "text/javascript; charset=utf-8",
            "text/plain",
            "application/json",
            "text/template",
        ] {
            assert_eq!(
                of(&format!("<script type='{kind}'>x</script>")),
                Vec::new(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn language_is_read_only_when_type_says_nothing() {
        assert_eq!(
            of("<script language=JavaScript>x</script>"),
            vec![written("x", None)]
        );
        assert_eq!(of("<script language=vbscript>x</script>"), Vec::new());
        assert_eq!(
            of("<script type=text/javascript language=vbscript>x</script>"),
            vec![written("x", None)],
        );
    }

    #[test]
    fn modules_and_import_maps_are_named_and_nomodule_is_skipped() {
        let found = of("<script type=module>m</script>\
                        <script type=importmap>{}</script>\
                        <script nomodule>fallback</script>\
                        <script type=module nomodule>still</script>");
        let kinds: Vec<Kind> = found
            .iter()
            .filter_map(|carried| match carried {
                Carried::Script(script) => Some(script.kind),
                Carried::Policy(_) => None,
            })
            .collect();
        assert_eq!(kinds, vec![Kind::Module, Kind::ImportMap, Kind::Module]);
    }

    #[test]
    fn src_wins_over_text_and_is_kept_as_written() {
        assert_eq!(
            of("<script src=' a.js '>ignored</script><script src=''></script>"),
            vec![
                Carried::Script(Script {
                    kind: Kind::Classic,
                    source: Source::Linked {
                        src: " a.js ".to_owned()
                    },
                    nonce: None,
                }),
                Carried::Script(Script {
                    kind: Kind::Classic,
                    source: Source::Linked { src: String::new() },
                    nonce: None,
                }),
            ],
        );
    }

    #[test]
    fn an_empty_script_is_nothing_and_whitespace_is_a_script() {
        assert_eq!(of("<script></script>"), Vec::new());
        assert_eq!(of("<script> </script>"), vec![written(" ", None)]);
    }

    #[test]
    fn a_templates_script_and_an_svg_script_are_not_here() {
        assert_eq!(
            of("<template><script>t</script></template>\
                <svg><script>s</script></svg>"),
            Vec::new(),
        );
    }

    #[test]
    fn a_nonce_is_presented_by_markup_the_page_could_have_written() {
        assert_eq!(
            of("<script nonce=abc>x</script>"),
            vec![written("x", Some("abc"))]
        );
    }

    #[test]
    fn a_nonce_beside_swallowed_markup_is_not_presented() {
        // What `<script src=//evil x="` does to the page's own
        // `<script nonce=abc>` that follows it.
        assert_eq!(
            of("<script x=\"<script \" nonce=abc>x</script>"),
            vec![written("x", None)]
        );
        assert_eq!(
            of("<script <style nonce=abc>x</script>"),
            vec![written("x", None)]
        );
        assert_eq!(
            of("<script x='<STYLE' nonce=abc>x</script>"),
            vec![written("x", None)]
        );
    }

    #[test]
    fn a_nonce_in_a_tag_that_repeated_an_attribute_is_not_presented() {
        assert_eq!(
            of("<script nonce=abc nonce=def>x</script>"),
            vec![written("x", None)]
        );
        assert_eq!(
            of("<script id=a nonce=abc id=b>x</script>"),
            vec![written("x", None)]
        );
    }

    /// Every HTML element named `name`, template contents included, in the
    /// order the parser made them.
    fn made(document: &Document, name: &str) -> Vec<NodeId> {
        (0..document.node_count())
            .map(NodeId)
            .filter(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.name.is_html(name))
            })
            .collect()
    }

    #[test]
    fn a_prepared_script_is_what_carried_says_it_is() {
        let document = parse_document(
            "<script nonce=n>a</script><script type=text/plain>data</script>\
             <script nomodule>fallback</script><script></script>\
             <script type=module src=m.js></script>",
        );
        let prepared: Vec<Script> = made(&document, "script")
            .into_iter()
            .filter_map(|id| prepared(&document, id))
            .collect();
        let carried: Vec<Script> = carried(&document)
            .into_iter()
            .filter_map(|carried| match carried {
                Carried::Script(script) => Some(script),
                Carried::Policy(_) => None,
            })
            .collect();
        assert_eq!(prepared, carried);
        assert_eq!(prepared.len(), 2);
    }

    #[test]
    fn a_script_out_of_the_document_is_not_prepared() {
        let mut document = parse_document(
            "<template><script>t</script></template><svg><script>s</script></svg>\
             <div><script>moved out</script></div>",
        );
        let scripts = made(&document, "script");
        assert_eq!(scripts.len(), 2, "the SVG one is not HTML's");
        let (Some(inert), Some(moved)) = (scripts.first(), scripts.get(1)) else {
            panic!("no scripts");
        };
        assert_eq!(prepared(&document, *inert), None, "a template's");
        assert!(prepared(&document, *moved).is_some());
        let div = document.parent(*moved);
        assert!(div.is_some_and(|div| document.remove(div)));
        assert_eq!(prepared(&document, *moved), None, "in a detached tree");
        let svg_script = (0..document.node_count()).map(NodeId).find(|id| {
            document.element(*id).is_some_and(|element| {
                &*element.name.local == "script" && !element.name.is_html("script")
            })
        });
        assert!(svg_script.is_some());
        assert_eq!(svg_script.and_then(|id| prepared(&document, id)), None);
        let body = made(&document, "body").first().copied();
        assert_eq!(
            body.and_then(|id| prepared(&document, id)),
            None,
            "not a script"
        );
    }

    #[test]
    fn a_meta_states_a_policy_where_carried_says_it_does_and_in_the_document() {
        let mut document = parse_document(
            "<head><meta http-equiv=content-security-policy content=\"script-src 'none'\">\
             <meta http-equiv=refresh content=5></head>\
             <body><meta http-equiv=content-security-policy content=x>",
        );
        let metas = made(&document, "meta");
        let stated_now: Vec<Option<String>> =
            metas.iter().map(|id| stated(&document, *id)).collect();
        assert_eq!(
            stated_now,
            vec![Some("script-src 'none'".to_owned()), None, None]
        );
        let head = made(&document, "head").first().copied();
        assert!(head.is_some_and(|head| document.remove(head)));
        assert_eq!(
            metas.first().and_then(|id| stated(&document, *id)),
            None,
            "a `<head>` out of the document states nothing"
        );
        let body = made(&document, "body").first().copied();
        assert_eq!(
            body.and_then(|id| stated(&document, id)),
            None,
            "not a meta"
        );
    }

    #[test]
    fn a_meta_policy_counts_only_in_head_and_only_with_content() {
        assert_eq!(
            of("<body><meta http-equiv=content-security-policy content=\"script-src 'none'\">"),
            Vec::new(),
        );
        assert_eq!(
            of("<head><meta http-equiv=content-security-policy content=''>\
                <meta http-equiv=content-security-policy>\
                <meta http-equiv=refresh content=5>"),
            Vec::new(),
        );
        assert_eq!(
            of(
                "<head><meta http-equiv=' CONTENT-SECURITY-POLICY ' content=\"default-src 'self'\">"
            ),
            vec![Carried::Policy("default-src 'self'".to_owned())],
        );
    }
}

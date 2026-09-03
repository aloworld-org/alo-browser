//! The style sheets a page carries inside itself.
//!
//! # Why this did not exist until a real page arrived
//!
//! Every case in the corpus until now was markup we wrote, with its style in a
//! file beside it — which is how `alo`'s own screens are built and which made
//! the question invisible. The first page taken off the web put its whole style
//! sheet in a `<style>` element in its `<head>`, because that is what pages do.
//!
//! It is a small function and it is worth the note: the gap was not in anything
//! anybody had thought about and refused. It was in the shape of the corpus, and
//! it went unseen for as long as the corpus was ours.
//!
//! # `<style>` and `<link>` together, in document order
//!
//! Both, and **in one list**, because a later sheet overrides an earlier one
//! and the order is the meaning. A page that writes a `<link>` and then a
//! `<style>` correcting it is relying on exactly that, and collecting the two
//! kinds separately would silently reorder every such page.
//!
//! What is *not* here is the fetching. This says a sheet is linked and where
//! from; getting the bytes is somebody else's job, because what a page does
//! while its style is still arriving is a real decision about unstyled content
//! rather than a parsing detail.

use crate::document::Document;

/// A style sheet a page asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sheet {
    /// Written into the markup, in a `<style>` element.
    Written(String),
    /// Somewhere else, named by a `<link>`.
    Linked {
        /// The `href`, exactly as the page wrote it — unresolved, because
        /// resolving it needs the page's own address and this does not have
        /// one.
        href: String,
    },
}

/// Every style sheet a page asks for, in document order.
///
/// A `<style>` or `<link>` inside `<template>` is deliberately not here: a
/// template's contents are inert until something clones them, and applying
/// their style to the page would style things the page never showed.
pub fn asked_for(document: &Document) -> Vec<Sheet> {
    let mut found = Vec::new();
    for id in document.descendants(document.root()) {
        let Some(element) = document.element(id) else {
            continue;
        };
        if element.name.local.eq_ignore_ascii_case("link") {
            if let Some(href) = linked_sheet(element) {
                found.push(Sheet::Linked { href });
            }
            continue;
        }
        if !element.name.local.eq_ignore_ascii_case("style") {
            continue;
        }
        // A `type` that is not CSS means a sheet meant for something else, and
        // applying it would be applying whatever a page wrote for some other
        // engine. Absent means CSS, which is what the specification says.
        let kind = element
            .attrs
            .iter()
            .find(|attribute| attribute.name.local.eq_ignore_ascii_case("type"))
            .map(|attribute| attribute.value.trim().to_ascii_lowercase());
        if let Some(kind) = kind {
            if !kind.is_empty() && kind != "text/css" {
                continue;
            }
        }
        let text = document.text_content(id);
        if !text.trim().is_empty() {
            found.push(Sheet::Written(text));
        }
    }
    found
}

/// Where a `<link>` points, if it is a style sheet at all.
///
/// `rel` is a space-separated list of keywords, so `rel="stylesheet alternate"`
/// contains `stylesheet` and is not one this engine should apply — an alternate
/// sheet is one a person chooses, and applying it as well would be applying
/// two. Only a plain `stylesheet` is taken.
fn linked_sheet(element: &crate::node::Element) -> Option<String> {
    let attribute = |wanted: &str| {
        element
            .attrs
            .iter()
            .find(|attribute| attribute.name.local.eq_ignore_ascii_case(wanted))
            .map(|attribute| attribute.value.trim().to_owned())
    };
    let rel = attribute("rel")?.to_ascii_lowercase();
    let keywords: Vec<&str> = rel.split_ascii_whitespace().collect();
    if !keywords.contains(&"stylesheet") || keywords.contains(&"alternate") {
        return None;
    }
    let href = attribute("href")?;
    if href.is_empty() { None } else { Some(href) }
}

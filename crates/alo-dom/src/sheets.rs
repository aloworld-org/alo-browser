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
//! # What is here and what is not
//!
//! `<style>` elements, in document order, because a later sheet overrides an
//! earlier one and the order **is** the meaning.
//!
//! Not `<link rel="stylesheet">`: a linked sheet is a second thing to fetch,
//! and what a page does while its style is still arriving is a real decision
//! about flashes of unstyled content rather than a parsing detail. It waits for
//! its own item.

use crate::document::Document;

/// Every style sheet written into the markup, in document order.
///
/// A `<style>` inside `<template>` is deliberately not here: a template's
/// contents are inert until something clones them, and applying its style to
/// the page would style things the page never showed.
pub fn written_into(document: &Document) -> Vec<String> {
    let mut found = Vec::new();
    for id in document.descendants(document.root()) {
        let Some(element) = document.element(id) else {
            continue;
        };
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
            found.push(text);
        }
    }
    found
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The walk round a tree.
//!
//! ADR 0017 § 3 decides that a node is alive exactly while its **tree** is,
//! and that a `<template>`'s contents are part of their template's tree,
//! since the template reaches them. So *a tree* here is the DOM standard's
//! host-including one: up through a parent, and from a contents fragment up
//! to its template; down into a template's contents and then its children.
//!
//! # Nothing here allocates, and nothing here trusts the arena
//!
//! The walk is taken inside a collection — while it marks and while it
//! sweeps — and ADR 0014 § 8 says a collection allocates
//! nothing it has not already got. So the walk keeps no stack: it climbs back
//! by the parent link. And every walk is given a **budget** of steps, the
//! arena's size twice over, which a well-formed tree never spends: `alo-dom`'s
//! validity rules make a cycle impossible, and a walk that runs out has found
//! a document that is not what they promise, which the caller answers by
//! keeping things alive rather than by freeing what it cannot see round.

use alo_dom::{Document, NodeId};

/// How many steps a walk of `document` may take before it gives up.
pub fn budget(document: &Document) -> usize {
    document.node_count().saturating_mul(2).saturating_add(2)
}

/// The node after `node` in its tree's order, going round: from the last
/// node back to `root`, so that a walk from any node returns to it.
///
/// The order is the host-including pre-order — a node, then its template's
/// contents, then its children — and `spent` counts every step taken, the
/// climb included. [`None`] once `spent` reaches `limit`, or if the climb
/// meets a node with no way up that is not `root`, which a tree rooted there
/// does not have.
pub fn next(
    document: &Document,
    node: NodeId,
    root: NodeId,
    spent: &mut usize,
    limit: usize,
) -> Option<NodeId> {
    if let Some(contents) = document
        .element(node)
        .and_then(|element| element.template_contents)
        .filter(|contents| document.get(*contents).is_some())
    {
        return step(spent, limit, contents);
    }
    if let Some(child) = document.first_child(node) {
        return step(spent, limit, child);
    }
    let mut at = node;
    loop {
        if at == root {
            return step(spent, limit, root);
        }
        if let Some(sibling) = document.next_sibling(at) {
            return step(spent, limit, sibling);
        }
        if let Some(parent) = document.parent(at) {
            at = step(spent, limit, parent)?;
            continue;
        }
        // A template's contents come before its children, so climbing out of
        // them lands on the template's first child, if it has one.
        let host = document.host(at)?;
        match document.first_child(host) {
            Some(child) => return step(spent, limit, child),
            None => at = step(spent, limit, host)?,
        }
    }
}

/// Count one step, and refuse it past the limit.
fn step(spent: &mut usize, limit: usize, to: NodeId) -> Option<NodeId> {
    *spent = spent.saturating_add(1);
    (*spent <= limit).then_some(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every node of the tree rooted at `root`, in order, by going round once.
    fn round(document: &Document, root: NodeId) -> Vec<NodeId> {
        let mut seen = vec![root];
        let mut spent = 0;
        let limit = budget(document);
        let mut at = next(document, root, root, &mut spent, limit).unwrap();
        while at != root {
            seen.push(at);
            at = next(document, at, root, &mut spent, limit).unwrap();
        }
        seen
    }

    #[test]
    fn a_walk_goes_round_through_a_templates_contents_first() {
        let mut document = Document::new();
        let outer = document.create_element("div").unwrap();
        let template = document.create_element("template").unwrap();
        let contents = document
            .element(template)
            .and_then(|e| e.template_contents)
            .unwrap();
        let inside = document.create_element("i").unwrap();
        let child = document.create_element("b").unwrap();
        let last = document.create_text_node("z");
        document.append_child(outer, template).unwrap();
        document.append_child(contents, inside).unwrap();
        document.append_child(template, child).unwrap();
        document.append_child(outer, last).unwrap();

        assert_eq!(
            round(&document, outer),
            vec![outer, template, contents, inside, child, last]
        );
        assert_eq!(round(&document, last), vec![last], "a leaf of its own");

        let root = document.root();
        document.append_child(root, outer).unwrap();
        assert_eq!(
            round(&document, root),
            vec![root, outer, template, contents, inside, child, last],
            "the same walk, from the document now"
        );
    }

    #[test]
    fn an_empty_template_climbs_out_to_what_follows_it() {
        let mut document = Document::new();
        let outer = document.create_element("div").unwrap();
        let template = document.create_element("template").unwrap();
        let contents = document
            .element(template)
            .and_then(|e| e.template_contents)
            .unwrap();
        let after = document.create_element("p").unwrap();
        document.append_child(outer, template).unwrap();
        document.append_child(outer, after).unwrap();

        assert_eq!(
            round(&document, outer),
            vec![outer, template, contents, after]
        );
    }

    #[test]
    fn a_walk_that_is_out_of_steps_says_so() {
        let mut document = Document::new();
        let outer = document.create_element("div").unwrap();
        let inner = document.create_element("p").unwrap();
        document.append_child(outer, inner).unwrap();

        let mut spent = 0;
        assert_eq!(next(&document, outer, outer, &mut spent, 0), None);
        let mut spent = 0;
        assert_eq!(next(&document, outer, outer, &mut spent, 1), Some(inner));
        assert_eq!(next(&document, inner, outer, &mut spent, 1), None);
    }
}

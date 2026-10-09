/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 332 (ADR 0031 § 2): every builtin a realm is furnished with —
//! the engine's own, and every interface, constructor, accessor and method
//! this crate installs for a document and a `navigator` — keeps no more than
//! `bounds::KEPT_BY_A_BUILTIN` values across the calls it asks for.
//!
//! A builtin declaring more is refused where its function is made, so a realm
//! with one could not be furnished at all. This walks the heap once the realm
//! is, so the bound is checked against what was actually installed rather
//! than against a list somebody has to remember to extend.

use alo_bindings::{Identity, adopt, install, introduce};
use alo_dom::parse_document;
use alo_js::bounds;
use alo_js::interpret::Engine;

/// How many builtins the heap holds, and the first that keeps too many.
fn audit(engine: &mut Engine) -> (usize, Option<String>) {
    let mut count = 0_usize;
    let mut over = None;
    for native in engine.objects().natives() {
        count += 1;
        if native.kept() > bounds::KEPT_BY_A_BUILTIN && over.is_none() {
            over = Some(format!("{} keeps {}", native.name(), native.kept()));
        }
    }
    (count, over)
}

#[test]
fn every_builtin_installed_for_a_page_keeps_no_more_than_eight() {
    let Ok(mut engine) = alo_bindings::engine(None) else {
        panic!("an engine");
    };
    let (engines, over) = audit(&mut engine);
    assert_eq!(over, None);

    let Ok(cell) = adopt(
        engine.objects(),
        parse_document("<!doctype html><title>t</title><p class=a>x</p>"),
    ) else {
        panic!("the document is adopted");
    };
    let _root = engine.objects().heap_mut().root(cell);
    assert!(
        install(&mut engine, cell).is_ok(),
        "the document is installed"
    );
    let identity = Identity {
        user_agent: "Mozilla/5.0 (Macintosh) alo/0.1",
        platform: "MacIntel",
    };
    assert!(
        introduce(&mut engine, cell, identity).is_ok(),
        "navigator is introduced"
    );

    let (all, over) = audit(&mut engine);
    assert_eq!(over, None);
    // The walk saw this crate's builtins as well as the engine's: every
    // interface's constructor and each of its members is one.
    assert!(
        all > engines + 50,
        "the walk found {all} builtins, {engines} of them the engine's"
    );
}

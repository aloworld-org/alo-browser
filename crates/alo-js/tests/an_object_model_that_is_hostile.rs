/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a script that is trying to get out does to the object model.
//!
//! `LOOP.md`'s stage 2 clause 2 and ADR 0013 § 4: a script is a stranger's
//! bytes and **every allocation it can cause has a ceiling we chose**. Queue
//! item 206 names the one that is new here — *an unbounded number of distinct
//! property keys, which is a refusal or a collection rather than a heap that
//! grows for ever* — and it has three parts, because a page can mint names in
//! three shapes and only one of them is bounded by the heap:
//!
//! - names it **keeps**, which are cells the heap counts and the ceiling
//!   bounds;
//! - names it **drops**, which are collected, so the intern table must let go
//!   of them rather than remembering every name a page has ever written;
//! - properties it **deletes**, where the table's own bookkeeping is what would
//!   grow.
//!
//! Every case here ends in a number rather than in "it did not crash". The
//! numbers are deliberately small enough to run in a second and large enough
//! that a table which grew without bound would fail by orders of magnitude.

use alo_js::object::{Found, Objects, Property, Value};

macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused}", stringify!($call)),
        }
    };
}

fn units(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}

#[test]
fn an_unbounded_number_of_distinct_names_is_collected_rather_than_kept() {
    // The intern table is weak (ADR 0014 § 11), so a page that mints names and
    // uses none of them holds nothing. A strong table would be a leak a
    // stranger's script controls, and it would look exactly like this test
    // passing except for the two numbers at the end.
    // The count is what makes this a bound rather than a hope: it is far past
    // `bounds::COLLECT_AFTER`, so the collector runs **on its own** during the
    // loop rather than because this test asked it to. Nobody has to ask, which
    // is the whole claim.
    let mut objects = Objects::new();
    let names = 200_000_u32;
    for at in 0..names {
        let name = units(&format!("name-{at}"));
        let key = ok!(objects.key(&name));
        assert!(key.as_text().is_some());
    }
    assert!(
        objects.heap().collections() > 0,
        "the trigger fired without being asked"
    );
    assert!(
        objects.heap().slots() < usize::try_from(names).unwrap_or(usize::MAX) / 2,
        "the arena filled slots again rather than growing one per name: {} slots for {names} names",
        objects.heap().slots()
    );

    objects.heap_mut().collect();
    assert!(objects.prune());
    assert_eq!(objects.heap().check(), Ok(()));
    assert_eq!(objects.heap().live(), 0, "not one name was kept");
    assert_eq!(objects.interned(), 0, "and the table let go of all of them");
}

#[test]
fn the_names_a_page_keeps_are_counted_against_the_heaps_ceiling() {
    // The other half: a name that *is* used is a cell, the object's table grows
    // with it, and both are counted — which is what makes ADR 0014 § 9's
    // ceiling a bound on this rather than a bound on something else. The
    // ceiling is a gibibyte and is not reached here; what is asserted is that
    // the count rises, because a footprint nobody counts is a ceiling nobody
    // enforces.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    let empty = objects.heap().held();

    for at in 0..20_000_u32 {
        let name = units(&format!("kept-{at}"));
        assert!(ok!(objects.define_named(
            object,
            &name,
            Property::plain(Value::Number(f64::from(at)))
        )));
    }

    objects.heap_mut().collect();
    assert_eq!(objects.heap().check(), Ok(()));
    assert_eq!(objects.heap().live(), 20_001, "the object and its names");
    assert!(
        objects.heap().held() > empty + 20_000 * 8,
        "the names and the table they are in are counted: {} bytes",
        objects.heap().held()
    );

    objects.heap_mut().release(held);
    objects.heap_mut().collect();
    assert_eq!(objects.heap().live(), 0);
    assert_eq!(objects.heap().held(), 0, "and go with the object");
}

#[test]
fn adding_and_deleting_a_property_in_a_loop_does_not_grow_for_ever() {
    // The table keeps insertion order, so a deletion leaves a hole rather than
    // moving every key after it. Holes that were never closed up would be this
    // loop growing without end — a leak in the one structure a page has the
    // most direct control over.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    assert!(ok!(objects.define_named(
        object,
        &units("kept"),
        Property::plain(Value::Null)
    )));
    let settled = objects.heap().held();

    for at in 0..20_000_u32 {
        let name = units(&format!("passing-{at}"));
        assert!(ok!(objects.define_named(
            object,
            &name,
            Property::plain(Value::Null)
        )));
        let key = ok!(objects.key(&name));
        assert!(ok!(objects.delete(object, key)));
    }

    objects.heap_mut().collect();
    assert!(objects.prune());
    assert_eq!(objects.heap().check(), Ok(()));
    assert_eq!(objects.own_keys(object).map(|keys| keys.len()), Ok(1));
    assert!(
        objects.heap().held() <= settled * 4,
        "the table was compacted rather than grown: {} bytes against {settled}",
        objects.heap().held()
    );
    assert_eq!(
        objects.heap().live(),
        2,
        "the object and the one name it kept"
    );

    objects.heap_mut().release(held);
}

#[test]
fn a_prototype_chain_a_script_chose_the_depth_of_is_walked_without_recursing() {
    // Item 204's finding, restated for a graph rather than for a program:
    // `Object.create` in a loop is four bytes of somebody else's file and a
    // stack overflow in ours, which is an abort rather than a refusal. Both the
    // marker (item 71) and this walk are loops, and this is the half item 206
    // added.
    let mut objects = Objects::new();
    let scope = objects.heap_mut().open();

    let root = ok!(objects.object(None));
    objects.heap_mut().hold(root);
    assert!(ok!(objects.define_named(
        root,
        &units("far"),
        Property::plain(Value::Number(1.0))
    )));

    let mut deepest = root;
    for _ in 0..100_000_u32 {
        deepest = ok!(objects.object(Some(deepest)));
        objects.heap_mut().hold(deepest);
    }

    let key = ok!(objects.key(&units("far")));
    assert_eq!(
        ok!(objects.get(deepest, key)),
        Found::Value(Value::Number(1.0)),
        "a hundred thousand links walked without a single recursive call"
    );
    let missing = ok!(objects.key(&units("nowhere")));
    assert_eq!(ok!(objects.get(deepest, missing)), Found::Missing);

    objects.heap_mut().collect();
    assert_eq!(objects.heap().check(), Ok(()));

    objects.heap_mut().close(scope);
    objects.heap_mut().collect();
    assert_eq!(objects.heap().live(), 0, "and all of it is reclaimed");
}

#[test]
fn a_name_that_is_a_hundred_digits_long_is_a_string_key_rather_than_a_saturated_one() {
    // The reading in `key::array_index` is the one place a hostile name could
    // become the *wrong* key: a number too large for an index that saturated
    // into one would put a property under a name the page did not write, where
    // a later `a[4294967294]` would find it.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);

    let enormous = "9".repeat(100);
    assert!(ok!(objects.define_named(
        object,
        &units(&enormous),
        Property::plain(Value::Number(1.0))
    )));
    assert!(ok!(objects.define_named(
        object,
        &units("4294967294"),
        Property::plain(Value::Number(2.0))
    )));

    let keys = ok!(objects.own_keys(object));
    assert_eq!(keys.len(), 2);
    assert_eq!(
        keys.first().and_then(|key| key.as_index()),
        Some(4_294_967_294),
        "the largest index is an index"
    );
    assert!(
        keys.get(1).and_then(|key| key.as_text()).is_some(),
        "and a hundred nines is a name"
    );
    objects.heap_mut().release(held);
}

#[test]
fn a_name_made_of_lone_surrogates_is_a_name_like_any_other() {
    // A page can build a string that stands for no text at all, and it is a
    // legal property key. What must not happen is that two different ones
    // become the same key by being repaired on the way in — which is what a
    // model storing names as Rust `String`s would do.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);

    let first = vec![0xD800_u16];
    let second = vec![0xD801_u16];
    assert!(ok!(objects.define_named(
        object,
        &first,
        Property::plain(Value::Number(1.0))
    )));
    assert!(ok!(objects.define_named(
        object,
        &second,
        Property::plain(Value::Number(2.0))
    )));

    assert_eq!(ok!(objects.own_keys(object)).len(), 2, "two names, not one");
    assert_eq!(
        ok!(objects.get_named(object, &first)),
        Found::Value(Value::Number(1.0))
    );
    assert_eq!(
        ok!(objects.get_named(object, &second)),
        Found::Value(Value::Number(2.0))
    );
    objects.heap_mut().release(held);
}

#[test]
fn ten_thousand_objects_sharing_one_name_share_one_cell() {
    // What interning is *for*, asserted as a number: a page with ten thousand
    // records all having a `title` holds one string, not ten thousand.
    let mut objects = Objects::new();
    let scope = objects.heap_mut().open();

    for _ in 0..10_000_u32 {
        let object = ok!(objects.object(None));
        objects.heap_mut().hold(object);
        assert!(ok!(objects.define_named(
            object,
            &units("title"),
            Property::plain(Value::Null)
        )));
    }

    objects.heap_mut().collect();
    assert_eq!(objects.heap().check(), Ok(()));
    assert_eq!(
        objects.heap().live(),
        10_001,
        "ten thousand objects and one name"
    );
    assert_eq!(objects.interned(), 1);

    objects.heap_mut().close(scope);
    objects.heap_mut().collect();
    assert_eq!(objects.heap().live(), 0);
}

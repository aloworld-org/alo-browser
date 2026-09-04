/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page can observe about an object, asserted rather than assumed.
//!
//! ADR 0014 § 11 and queue item 206. Three of these close the item and the rest
//! are the rules that would otherwise be discovered by a page:
//!
//! - **the order** a page enumerates is the specification's, from keys of all
//!   three kinds minted out of order;
//! - **a prototype chain** answers a lookup, and a cycle in one is refused
//!   rather than looped — both halves, because the refusal is the specification
//!   and the bound is the defence against an embedder that does not obey it;
//! - and the hostile half is `an_object_model_that_is_hostile.rs`.
//!
//! # The embedder's object is here rather than in a crate that does not exist
//!
//! ADR 0013 § 6 gives `alo-js` no dependency on `alo-dom` and `alo-dom` none on
//! this crate, so the thing that joins them is the bindings crate — queue item
//! 80. Until it exists, [`Node`] below is what an embedder's object is: an
//! [`Exotic`] with internal methods of its own, in this heap, in the same graph.
//! What is being asserted is the engine's half, which is all of it this item
//! owns.

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Found, Internal, Key, Objects, Ordinary, Property, Set, Value};

/// Take what a call answered, reporting a refusal as the failure it is.
///
/// A macro rather than a function for the reason every test file in this crate
/// gives: the panic family is denied outside a `#[test]`, and a helper function
/// is not one. Expanded at the call site, the failure is reported by the test
/// that caused it.
macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused}", stringify!($call)),
        }
    };
}

/// The code units of an ASCII name, which is what a property is keyed by.
fn units(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}

/// An embedder's object: an ordinary one inside, and one rule of its own.
///
/// It refuses to be given properties when `sealed`, which is what a DOM object
/// with a fixed shape does — and it gets get, set, has, delete, the prototype
/// walk and the enumeration order for free, from the trait, which is ADR 0014
/// § 11's *one mechanism rather than two* in the only form that can be checked.
#[derive(Debug)]
struct Node {
    own: Ordinary,
    sealed: bool,
}

impl Node {
    fn new(sealed: bool) -> Self {
        Self {
            own: Ordinary::with_prototype(None),
            sealed,
        }
    }
}

impl Internal for Node {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.own.own_property(key)
    }

    fn own_property_mut(&mut self, key: Key) -> Option<&mut Property> {
        self.own.own_property_mut(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        !self.sealed && self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.own.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.own.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.own.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.own.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.own.prevent_extensions()
    }
}

impl Trace for Node {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for Node {
    fn describe(&self) -> &'static str {
        "a node"
    }
}

#[test]
fn the_order_a_page_enumerates_is_the_specifications() {
    // Queue item 206's first closing condition: keys of all three kinds, minted
    // in the worst order for the rule. Integer-like keys ascending — whatever
    // order they arrived in — then strings in insertion order, then symbols in
    // insertion order.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);

    let first = ok!(objects.symbol(None));
    let second = ok!(objects.symbol(None));
    let first = ok!(objects.symbol_key(first));
    let second = ok!(objects.symbol_key(second));

    for name in ["zebra", "2", "apple", "0", "10", "1"] {
        let key = ok!(objects.key(&units(name)));
        assert!(ok!(objects.define(
            object,
            key,
            Property::plain(Value::Null)
        )));
    }
    assert!(ok!(objects.define(
        object,
        first,
        Property::plain(Value::Null)
    )));
    assert!(ok!(objects.define(
        object,
        second,
        Property::plain(Value::Null)
    )));

    let expected = vec![
        ok!(Key::index(0).ok_or("0 is an index")),
        ok!(Key::index(1).ok_or("1 is an index")),
        ok!(Key::index(2).ok_or("2 is an index")),
        ok!(Key::index(10).ok_or("10 is an index")),
        ok!(objects.key(&units("zebra"))),
        ok!(objects.key(&units("apple"))),
        first,
        second,
    ];
    assert_eq!(ok!(objects.own_keys(object)), expected);
    objects.heap_mut().release(held);
}

#[test]
fn a_name_that_only_looks_like_an_index_keeps_its_place_among_the_strings() {
    // The near misses, which every engine has to get right and which a page can
    // see the moment it enumerates: only the canonical decimal of an index is
    // an index.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);

    for name in ["01", "4294967295", " 1", "1.0", "-0", "5"] {
        let key = ok!(objects.key(&units(name)));
        assert!(ok!(objects.define(
            object,
            key,
            Property::plain(Value::Null)
        )));
    }

    let keys = ok!(objects.own_keys(object));
    let names: Vec<Option<String>> = keys
        .iter()
        .map(|key| {
            key.as_text()
                .and_then(|held| objects.units(held))
                .map(String::from_utf16_lossy)
        })
        .collect();
    assert_eq!(keys.first().and_then(|key| key.as_index()), Some(5));
    assert_eq!(
        names,
        vec![
            None,
            Some("01".to_owned()),
            Some("4294967295".to_owned()),
            Some(" 1".to_owned()),
            Some("1.0".to_owned()),
            Some("-0".to_owned()),
        ]
    );
    objects.heap_mut().release(held);
}

#[test]
fn a_prototype_chain_answers_a_lookup() {
    // Queue item 206's second closing condition, first half.
    let mut objects = Objects::new();
    let grandparent = ok!(objects.object(None));
    let held = objects.heap_mut().root(grandparent);
    let parent = ok!(objects.object(Some(grandparent)));
    let child = ok!(objects.object(Some(parent)));
    let child_held = objects.heap_mut().root(child);

    let inherited = ok!(objects.key(&units("far")));
    assert!(ok!(objects.define(
        grandparent,
        inherited,
        Property::plain(Value::Number(7.0))
    )));
    let missing = ok!(objects.key(&units("nowhere")));

    assert_eq!(
        ok!(objects.get(child, inherited)),
        Found::Value(Value::Number(7.0)),
        "a lookup follows the chain to the top"
    );
    assert!(ok!(objects.has(child, inherited)));
    assert_eq!(ok!(objects.get(child, missing)), Found::Missing);
    assert!(!ok!(objects.has(child, missing)));
    assert!(
        ok!(objects.own_property(child, inherited)).is_none(),
        "and an inherited property is not an own one"
    );

    objects.heap_mut().release(held);
    objects.heap_mut().release(child_held);
}

#[test]
fn a_prototype_cycle_is_refused_rather_than_made() {
    // The second closing condition's other half, and it is the specification's
    // own rule: the assignment that would close the loop answers `false`, so no
    // chain in this heap ever has a cycle in it.
    let mut objects = Objects::new();
    let first = ok!(objects.object(None));
    let held = objects.heap_mut().root(first);
    let second = ok!(objects.object(Some(first)));
    let second_held = objects.heap_mut().root(second);

    assert!(
        !ok!(objects.set_prototype(first, Some(second))),
        "closing the loop is refused"
    );
    assert!(
        !ok!(objects.set_prototype(first, Some(first))),
        "and so is an object being its own prototype"
    );
    assert_eq!(ok!(objects.prototype(first)), None, "nothing changed");

    let third = ok!(objects.object(None));
    assert!(
        ok!(objects.set_prototype(first, Some(third))),
        "and a prototype that closes no loop is allowed"
    );
    assert_eq!(ok!(objects.prototype(first)), Some(third));

    objects.heap_mut().release(held);
    objects.heap_mut().release(second_held);
}

#[test]
fn a_chain_an_embedder_lied_about_is_refused_rather_than_walked_for_ever() {
    // `Objects::set_prototype` refuses a cycle, so nothing a *page* writes can
    // make one. An embedder answers `[[GetPrototypeOf]]` for itself, and a
    // renderer that hung on one would be a denial of service in the process
    // that parses hostile bytes (ADR 0005). So the walk is bounded, and the
    // bound is exact: a chain longer than there are slots has visited one
    // twice.
    let mut objects = Objects::new();
    let node = ok!(objects.foreign(Box::new(Node::new(false))));
    let held = objects.heap_mut().root(node);

    // Straight through the heap rather than through `set_prototype`, which is
    // what an embedder that does not consult the engine amounts to.
    let lied = objects
        .heap_mut()
        .write(node, |cell, barrier| {
            cell.internal_mut()
                .is_some_and(|internal| internal.set_prototype(barrier, Some(node)))
        })
        .unwrap_or(false);
    assert!(lied, "the embedder set its own prototype");

    let key = ok!(objects.key(&units("anything")));
    assert_eq!(
        objects.get(node, key),
        Err(alo_js::object::Fault::ChainDoesNotEnd)
    );
    assert_eq!(
        objects.has(node, key),
        Err(alo_js::object::Fault::ChainDoesNotEnd)
    );
    objects.heap_mut().release(held);
}

#[test]
fn an_embedder_object_is_an_object_by_the_same_mechanism() {
    // ADR 0014 § 11: *internal methods are a trait, and it is the same trait
    // ADR 0013 § 6 promised the embedder.* A `Node` implements the own-property
    // questions and gets get, set, has, delete and the order for nothing.
    let mut objects = Objects::new();
    let node = ok!(objects.foreign(Box::new(Node::new(false))));
    let sealed = ok!(objects.foreign(Box::new(Node::new(true))));
    let held = objects.heap_mut().root(node);
    let sealed_held = objects.heap_mut().root(sealed);

    let key = ok!(objects.key(&units("id")));
    assert!(ok!(objects.define(
        node,
        key,
        Property::plain(Value::Number(1.0))
    )));
    assert_eq!(
        ok!(objects.get(node, key)),
        Found::Value(Value::Number(1.0))
    );
    assert_eq!(ok!(objects.own_keys(node)), vec![key]);

    assert!(
        !ok!(objects.define(sealed, key, Property::plain(Value::Number(1.0)))),
        "and an exotic object's own rule is answered by the exotic object"
    );
    assert_eq!(ok!(objects.get(sealed, key)), Found::Missing);

    objects.heap_mut().release(held);
    objects.heap_mut().release(sealed_held);
}

#[test]
fn the_same_name_is_the_same_key_and_a_symbol_is_never_a_name() {
    let mut objects = Objects::new();
    let first = ok!(objects.key(&units("colour")));
    let second = ok!(objects.key(&units("colour")));
    assert_eq!(first, second, "one string cell per distinct name");
    assert_ne!(first, ok!(objects.key(&units("color"))));
    assert_eq!(objects.interned(), 2);

    // Two symbols with the same description are two keys, which is the whole of
    // what a symbol is.
    let description = ok!(objects.text(units("tag")));
    let held = objects.heap_mut().root(description);
    let one = ok!(objects.symbol(Some(description)));
    let two = ok!(objects.symbol(Some(description)));
    assert_ne!(ok!(objects.symbol_key(one)), ok!(objects.symbol_key(two)));
    objects.heap_mut().release(held);
}

#[test]
fn an_index_key_allocates_nothing() {
    // The other half of why an index is a different key rather than a faster
    // one: `a[i]` in a loop interns no strings at all.
    let mut objects = Objects::new();
    let before = objects.heap().live();
    for at in 0..100_u32 {
        let key = ok!(objects.key(&units(&at.to_string())));
        assert_eq!(key.as_index(), Some(at));
    }
    assert_eq!(objects.heap().live(), before, "no cell was made");
    assert_eq!(objects.interned(), 0, "and nothing was interned");
}

#[test]
fn a_property_that_may_not_be_written_refuses_a_store() {
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    let key = ok!(objects.key(&units("frozen")));
    assert!(ok!(objects.define(
        object,
        key,
        Property::data(Value::Number(1.0), false, true, false)
    )));

    assert_eq!(
        ok!(objects.set(object, key, Value::Number(2.0))),
        Set::Refused
    );
    assert_eq!(
        ok!(objects.get(object, key)),
        Found::Value(Value::Number(1.0))
    );
    objects.heap_mut().release(held);
}

#[test]
fn a_property_that_may_not_be_configured_refuses_a_deletion_and_a_redefinition() {
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    let key = ok!(objects.key(&units("fixed")));
    assert!(ok!(objects.define(
        object,
        key,
        Property::data(Value::Number(1.0), false, true, false)
    )));

    assert!(!ok!(objects.delete(object, key)));
    assert!(!ok!(objects.define(
        object,
        key,
        Property::plain(Value::Number(2.0))
    )));
    assert_eq!(
        ok!(objects.get(object, key)),
        Found::Value(Value::Number(1.0))
    );

    let absent = ok!(objects.key(&units("never-was")));
    assert!(
        ok!(objects.delete(object, absent)),
        "and deleting what was not there succeeded, which is what the language says"
    );
    objects.heap_mut().release(held);
}

#[test]
fn an_object_that_prevents_extensions_takes_no_more_properties() {
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    let existing = ok!(objects.key(&units("here")));
    assert!(ok!(objects.define(
        object,
        existing,
        Property::plain(Value::Number(1.0))
    )));

    assert!(ok!(objects.prevent_extensions(object)));
    assert!(!ok!(objects.is_extensible(object)));

    let fresh = ok!(objects.key(&units("new")));
    assert!(!ok!(objects.define(
        object,
        fresh,
        Property::plain(Value::Null)
    )));
    assert_eq!(ok!(objects.set(object, fresh, Value::Null)), Set::Refused);
    assert_eq!(
        ok!(objects.set(object, existing, Value::Number(2.0))),
        Set::Done,
        "what was already there may still be written"
    );
    let other = ok!(objects.object(None));
    assert!(
        !ok!(objects.set_prototype(object, Some(other))),
        "and its prototype is fixed too"
    );
    assert!(
        ok!(objects.set_prototype(object, None)),
        "though setting it to the one it already has changes nothing and says so"
    );
    objects.heap_mut().release(held);
}

#[test]
fn an_accessor_hands_back_the_function_to_call() {
    // ADR 0013 § 3, *absent beats approximate*: nothing can call anything until
    // queue item 72, so reading an accessor answers the getter rather than
    // pretending to have called it.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    let getter = ok!(objects.object(None));
    let getter_held = objects.heap_mut().root(getter);

    let key = ok!(objects.key(&units("computed")));
    assert!(ok!(objects.define(
        object,
        key,
        Property::accessor(Value::Object(getter), Value::Undefined, true, true)
    )));

    assert_eq!(
        ok!(objects.get(object, key)),
        Found::Getter(Value::Object(getter))
    );
    assert_eq!(
        ok!(objects.set(object, key, Value::Null)),
        Set::Setter(Value::Undefined),
        "and a setter that is undefined is a refusal the caller gets to name"
    );

    objects.heap_mut().release(held);
    objects.heap_mut().release(getter_held);
}

#[test]
fn a_prototypes_property_is_shadowed_rather_than_written_through() {
    // The rule that keeps one object from quietly editing another's: a writable
    // data property found on a prototype becomes an own property here.
    let mut objects = Objects::new();
    let parent = ok!(objects.object(None));
    let held = objects.heap_mut().root(parent);
    let child = ok!(objects.object(Some(parent)));
    let child_held = objects.heap_mut().root(child);

    let key = ok!(objects.key(&units("shared")));
    assert!(ok!(objects.define(
        parent,
        key,
        Property::plain(Value::Number(1.0))
    )));

    assert_eq!(ok!(objects.set(child, key, Value::Number(2.0))), Set::Done);
    assert_eq!(
        ok!(objects.get(parent, key)),
        Found::Value(Value::Number(1.0)),
        "the parent is untouched"
    );
    assert_eq!(
        ok!(objects.get(child, key)),
        Found::Value(Value::Number(2.0))
    );
    assert_eq!(ok!(objects.own_keys(child)), vec![key]);

    objects.heap_mut().release(held);
    objects.heap_mut().release(child_held);
}

#[test]
fn a_prototypes_read_only_property_refuses_the_store_rather_than_shadowing_it() {
    let mut objects = Objects::new();
    let parent = ok!(objects.object(None));
    let held = objects.heap_mut().root(parent);
    let child = ok!(objects.object(Some(parent)));
    let child_held = objects.heap_mut().root(child);

    let key = ok!(objects.key(&units("constant")));
    assert!(ok!(objects.define(
        parent,
        key,
        Property::data(Value::Number(1.0), false, true, true)
    )));

    assert_eq!(
        ok!(objects.set(child, key, Value::Number(2.0))),
        Set::Refused
    );
    assert!(ok!(objects.own_keys(child)).is_empty());

    objects.heap_mut().release(held);
    objects.heap_mut().release(child_held);
}

#[test]
fn a_string_is_in_the_heap_and_is_not_something_that_has_properties() {
    // `"abc".length` reads a property of a *wrapper* the interpreter makes
    // (queue item 73); the string itself has none at all, which is the language
    // rather than a shortcut.
    let mut objects = Objects::new();
    let text = ok!(objects.text(vec![0xD83D, 0x0041]));
    let held = objects.heap_mut().root(text);
    assert_eq!(
        objects.units(text),
        Some([0xD83D, 0x0041].as_slice()),
        "a lone surrogate survives, which a Rust String could not hold"
    );

    let key = ok!(objects.key(&units("length")));
    assert_eq!(
        objects.get(text, key),
        Err(alo_js::object::Fault::NotAnObject)
    );
    objects.heap_mut().release(held);
}

#[test]
fn what_an_object_holds_is_kept_alive_by_it_and_nothing_else() {
    // The join between this item and item 71: a property's *name* is an edge as
    // much as its value is, which is what makes the intern table safe to be
    // weak.
    let mut objects = Objects::new();
    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    let value = ok!(objects.object(None));
    let key = ok!(objects.key(&units("child")));

    assert!(ok!(objects.define(
        object,
        key,
        Property::plain(Value::Object(value))
    )));
    objects.heap_mut().collect();
    assert_eq!(objects.heap().check(), Ok(()));
    assert_eq!(
        objects.heap().live(),
        3,
        "the object, the name and the value"
    );
    assert_eq!(
        ok!(objects.get(object, key)),
        Found::Value(Value::Object(value))
    );

    assert!(ok!(objects.delete(object, key)));
    objects.heap_mut().collect();
    assert_eq!(objects.heap().check(), Ok(()));
    assert_eq!(
        objects.heap().live(),
        1,
        "and letting go of the property let go of both"
    );
    assert!(objects.prune());
    assert_eq!(objects.interned(), 0, "the intern table let go too");

    objects.heap_mut().release(held);
}

#[test]
fn the_model_holds_together_when_a_collection_happens_at_every_allocation() {
    // ADR 0014 § 10's stress mode, which is the only thing that finds a
    // reference nobody rooted — applied to the object model, since every
    // interning is an allocation and therefore a safepoint.
    let mut objects = Objects::new();
    objects.heap_mut().stress(true);

    let object = ok!(objects.object(None));
    let held = objects.heap_mut().root(object);
    for at in 0..16_u32 {
        let name = units(&format!("name{at}"));
        // `define_named` interns and defines with nothing in between, which is
        // the discipline written as one call rather than remembered.
        assert!(ok!(objects.define_named(
            object,
            &name,
            Property::plain(Value::Number(f64::from(at)))
        )));
    }

    assert_eq!(objects.heap().check(), Ok(()));
    for at in 0..16_u32 {
        let name = units(&format!("name{at}"));
        assert_eq!(
            ok!(objects.get_named(object, &name)),
            Found::Value(Value::Number(f64::from(at))),
            "every property survived a collection at every allocation"
        );
    }
    assert_eq!(objects.heap().stale_edges(), 0);
    objects.heap_mut().release(held);
}

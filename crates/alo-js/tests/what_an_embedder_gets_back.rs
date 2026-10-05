/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0017 § 4: an embedder may ask for its own exotic object back by type
//! (queue item 248).
//!
//! A DOM method reaches its node only through the object it was called on, so
//! the engine must answer *is this one of yours, and may I have it?* — for
//! exactly the embedder's type, and [`None`] for everything else, without
//! learning the type to answer.

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property};

macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused}", stringify!($call)),
        }
    };
}

/// Two embedder types that are alike in everything but their type, so that
/// only the type can tell them apart.
macro_rules! embedded {
    ($name:ident, $describe:literal) => {
        #[derive(Debug)]
        struct $name {
            own: Ordinary,
            count: u32,
            held: Option<Ref>,
            /// What it says it owns beyond its slot.
            weighs: usize,
        }

        impl Internal for $name {
            fn own_property(&self, key: Key) -> Option<&Property> {
                self.own.own_property(key)
            }
            fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
                self.own.define_own(barrier, key, property)
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

        impl Trace for $name {
            fn trace(&self, tracer: &mut Tracer) {
                self.own.trace(tracer);
                if let Some(held) = self.held {
                    tracer.edge(held);
                }
            }
            fn footprint(&self) -> usize {
                self.weighs
            }
        }

        impl Exotic for $name {
            fn describe(&self) -> &'static str {
                $describe
            }
        }

        impl $name {
            fn new(count: u32) -> Self {
                Self {
                    own: Ordinary::with_prototype(None),
                    count,
                    held: None,
                    weighs: 0,
                }
            }
        }
    };
}

embedded!(Mine, "one of mine");
embedded!(Theirs, "one of theirs");

#[test]
fn an_embedder_has_its_own_object_back_and_nothing_else() {
    let mut objects = Objects::new();
    let mine = ok!(objects.foreign(Box::new(Mine::new(3))));
    let theirs = ok!(objects.foreign(Box::new(Theirs::new(4))));
    let plain = ok!(objects.object(None));
    let text = ok!(objects.text("mine".encode_utf16().collect()));
    let roots = [mine, theirs, plain, text].map(|held| objects.heap_mut().root(held));

    assert_eq!(objects.embedded::<Mine>(mine).map(|m| m.count), Some(3));
    assert_eq!(objects.embedded::<Theirs>(theirs).map(|t| t.count), Some(4));
    assert!(objects.embedded::<Mine>(theirs).is_none(), "another type");
    assert!(objects.embedded::<Theirs>(mine).is_none());
    assert!(
        objects.embedded::<Mine>(plain).is_none(),
        "an ordinary object"
    );
    assert!(
        objects.embedded::<Mine>(text).is_none(),
        "not an object at all"
    );

    assert_eq!(
        objects.write_embedded::<Mine, _>(mine, |m, _| {
            m.count += 1;
            m.count
        }),
        Some(4)
    );
    assert_eq!(objects.embedded::<Mine>(mine).map(|m| m.count), Some(4));
    assert_eq!(
        objects.write_embedded::<Mine, _>(theirs, |m, _| m.count = 0),
        None,
        "nothing written through the wrong type"
    );
    assert_eq!(objects.embedded::<Theirs>(theirs).map(|t| t.count), Some(4));

    for root in roots {
        objects.heap_mut().release(root);
    }
}

#[test]
fn a_reference_that_names_nothing_answers_nothing() {
    let mut objects = Objects::new();
    let gone = ok!(objects.foreign(Box::new(Mine::new(1))));
    objects.heap_mut().collect();
    assert!(!objects.heap().live_at(gone));
    assert!(objects.embedded::<Mine>(gone).is_none());
    assert_eq!(
        objects.write_embedded::<Mine, _>(gone, |m, _| m.count),
        None
    );

    // Its slot filled again by something of the same type: the old reference
    // still names nothing, since the slot is in a new life.
    let next = ok!(objects.foreign(Box::new(Mine::new(2))));
    let root = objects.heap_mut().root(next);
    assert!(objects.embedded::<Mine>(gone).is_none());
    assert_eq!(objects.embedded::<Mine>(next).map(|m| m.count), Some(2));
    objects.heap_mut().release(root);
}

#[test]
fn a_reference_stored_through_the_borrow_is_traced() {
    // What an embedder writes through the typed borrow is part of the graph:
    // the barrier hears of it, and the collector follows it.
    let mut objects = Objects::new();
    let mine = ok!(objects.foreign(Box::new(Mine::new(0))));
    let root = objects.heap_mut().root(mine);
    let kept = ok!(objects.object(None));
    let stores = objects.heap().stores();
    objects.write_embedded::<Mine, _>(mine, |m, barrier| {
        barrier.stored(m.held, Some(kept));
        m.held = Some(kept);
    });
    assert!(objects.heap().stores() > stores);
    objects.heap_mut().collect();
    assert!(objects.heap().live_at(kept));
    if let Err(broken) = objects.heap().check() {
        panic!("{broken:?}");
    }
    objects.heap_mut().release(root);
}

#[test]
fn an_embedder_reads_the_realms_intrinsics_as_a_script_sees_them() {
    use alo_js::builtin::error::Family;
    use alo_js::interpret::Engine;
    use alo_js::object::{Found, Value};
    use alo_js::script;

    let mut engine = ok!(Engine::new());
    let mut seen = |source: &str| match engine.evaluate(&ok!(script(source))) {
        Ok(Value::Object(held)) => held,
        other => panic!("{source} answered {other:?}"),
    };
    let object = seen("({}).__proto__");
    let function = seen("(function () {}).__proto__");
    let error = seen("Error.prototype");

    let (intrinsics, objects) = engine.intrinsics();
    assert_eq!(intrinsics.object_prototype(objects), Ok(object));
    assert_eq!(intrinsics.function_prototype(objects), Ok(function));
    let constructor = ok!(intrinsics.error_constructor(objects, Family::Error));
    let Some(key) = objects.existing_key(&"prototype".encode_utf16().collect::<Vec<_>>()) else {
        panic!("`prototype` is interned by the realm");
    };
    assert_eq!(
        objects.get(constructor, key),
        Ok(Found::Value(Value::Object(error)))
    );
}

#[test]
fn an_object_the_heap_refuses_goes_back_whole_to_whoever_made_it() {
    // ADR 0017 § 2: a page's document moves into the heap as one object, and
    // a heap too full to take it must not destroy the only copy. An object
    // that says it owns more than the whole heap may hold is refused at once,
    // and comes back as its maker's own type with everything it held.
    let mut objects = Objects::new();
    let mut huge = Mine::new(7);
    huge.weighs = alo_js::bounds::HEAP_CEILING;
    let Err((refused, back)) = objects.foreign_or_back(huge) else {
        panic!("an object weighing the whole heap was taken");
    };
    assert!(
        matches!(refused, alo_js::object::Refused::Full(_)),
        "{refused}"
    );
    let Some(back) = back else {
        panic!("the object was lost rather than handed back");
    };
    assert_eq!((back.count, back.weighs), (7, alo_js::bounds::HEAP_CEILING));

    // And one that fits is taken, as `foreign` takes it.
    let taken = ok!(objects
        .foreign_or_back(Mine::new(8))
        .map_err(|(why, _)| why));
    assert_eq!(objects.embedded::<Mine>(taken).map(|m| m.count), Some(8));
}

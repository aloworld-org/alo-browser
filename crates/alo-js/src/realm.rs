/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The global object, the global `let` bindings, and the order a name is looked
//! for in.
//!
//! A realm is what a script's top level *is*. It is two stores rather than one,
//! and the language can tell them apart:
//!
//! - **`var` and function declarations become properties of the global
//!   object**, which is why `var a = 1` and `globalThis.a` are one thing, and
//!   why a `var` can be redeclared without complaint.
//! - **`let` and `const` do not.** They live in a declarative record beside the
//!   global object, they have a dead zone, a `const` refuses assignment, and
//!   nothing can enumerate or delete them.
//!
//! Looking a name up asks the declarative record first and the global object
//! second, so a `let` shadows a property of the same name. Getting that order
//! the other way round is the sort of mistake that shows up as one page in a
//! thousand behaving oddly.
//!
//! # The bindings are in the heap, and their names are not
//!
//! A binding's *value* is a reference the collector must walk (ADR 0014 § 2), so
//! the values are a [`Slots`](crate::object::Slots) cell and the realm holds a
//! [`Root`] on it. A binding's *name* is only ever compared, never traced, so
//! the table from names to slots is an ordinary map on this side of the heap —
//! which also means a lookup costs no allocation and can happen in the middle of
//! an instruction.
//!
//! # Four properties are here and the rest of the builtins are not
//!
//! `undefined`, `NaN`, `Infinity` and `globalThis` are **value** properties of
//! the global object rather than functions, and they are here because without
//! them the language has no way to *write* three of its own values: `undefined`
//! is a name rather than a literal, and so are the other two. Every one of them
//! has the attributes the specification gives it — the first three are not
//! writable, not enumerable and not configurable, which is what makes
//! `undefined = 1` do nothing.
//!
//! Nothing else is here: no `Object`, no `Array`, no `Math`, no `console`.
//! ADR 0013 § 3 — *absent beats approximate* — and queue item 73 is where they
//! arrive. An embedder may put its own things on the global object today, which
//! is how a test harness reaches a script.

use std::collections::HashMap;

use crate::abrupt::{Escape, Missing};
use crate::heap::{Ref, Root};
use crate::object::{Found, Held, Objects, Property, Refused, Set, Value};

/// One `let` or `const` the realm holds.
#[derive(Debug, Clone, Copy)]
struct Binding {
    /// Which slot of the record holds its value.
    slot: usize,
    /// Whether it may be assigned to again.
    mutable: bool,
}

/// Where a script's top level lives.
#[derive(Debug)]
pub struct Realm {
    global: Root,
    record: Root,
    bindings: HashMap<Vec<u16>, Binding>,
}

/// What a name resolved to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolved {
    /// A lexical binding with a value.
    Lexical(Value),
    /// A lexical binding that is still in its dead zone.
    Dead,
    /// A property of the global object.
    Property(Value),
    /// Nothing at all, which reading is a `ReferenceError` and `typeof` is not.
    Nothing,
}

impl Realm {
    /// A realm with an empty global object and nothing declared.
    ///
    /// # Errors
    ///
    /// [`Refused`] if the heap cannot hold two cells, which is a heap that was
    /// full before anything ran.
    pub fn new(objects: &mut Objects) -> Result<Self, Refused> {
        // No prototype: `Object.prototype` is a builtin (queue item 73), and an
        // object pretending to have one would be an object whose `toString` a
        // page could find and this engine could not call.
        let global = objects.object(None)?;
        let global = objects.heap_mut().root(global);
        let record = objects.slots()?;
        let record = objects.heap_mut().root(record);
        let realm = Self {
            global,
            record,
            bindings: HashMap::new(),
        };
        realm.name_the_values(objects)?;
        Ok(realm)
    }

    /// The four value properties of the global object.
    ///
    /// Three of them are the only way to write three of the language's values,
    /// and `globalThis` is how a script reaches the object the other three are
    /// on. Their attributes are the specification's, which is why
    /// `undefined = 1` is silently nothing in sloppy code and a `TypeError` in
    /// strict code rather than an assignment.
    fn name_the_values(&self, objects: &mut Objects) -> Result<(), Refused> {
        let Some(global) = objects.heap().holding(&self.global) else {
            return Ok(());
        };
        let fixed = [
            ("undefined", Value::Undefined),
            ("NaN", Value::Number(f64::NAN)),
            ("Infinity", Value::Number(f64::INFINITY)),
        ];
        for (name, value) in fixed {
            let units: Vec<u16> = name.encode_utf16().collect();
            let key = objects.key(&units)?;
            let _ = objects.define(global, key, Property::data(value, false, false, false));
        }
        let units: Vec<u16> = "globalThis".encode_utf16().collect();
        let key = objects.key(&units)?;
        let _ = objects.define(
            global,
            key,
            Property::data(Value::Object(global), true, false, true),
        );
        Ok(())
    }

    /// The global object, which is what an embedder puts its own things on.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the root no longer names it, which is this
    /// engine's own bug rather than a page's (ADR 0014 § 3).
    pub fn global(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.global)
            .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))
    }

    /// The list of lexical values.
    fn record(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.record)
            .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))
    }

    /// Declare a `var`: a property of the global object, if it is not already
    /// there.
    ///
    /// Not configurable, which is the rule that makes `delete a` answer `false`
    /// for a `var` and `true` for a property somebody assigned into existence.
    ///
    /// # Errors
    ///
    /// A `SyntaxError`-shaped refusal if the name is already a lexical binding,
    /// which is the early error the language specifies for
    /// `let a; var a;` across two scripts.
    pub fn declare_var(
        &mut self,
        objects: &mut Objects,
        name: &[u16],
        at: usize,
    ) -> Result<(), Escape> {
        if self.bindings.contains_key(name) {
            return Err(Escape::type_error(
                format!(
                    "'{}' is already declared with let or const in this realm",
                    show(name)
                ),
                at,
            ));
        }
        let global = self.global(objects)?;
        let key = objects.key(name).map_err(|why| Escape::refused(why, at))?;
        if objects.own_property(global, key)?.is_some() {
            return Ok(());
        }
        objects
            .define(
                global,
                key,
                Property::data(Value::Undefined, true, true, false),
            )
            .map_err(Escape::fault)?;
        Ok(())
    }

    /// Declare a `let` or a `const`, in its dead zone.
    ///
    /// # Errors
    ///
    /// A refusal if the realm already has that name, either lexically or as a
    /// property of the global object — the early error that keeps two scripts
    /// from declaring one name twice.
    pub fn declare_lexical(
        &mut self,
        objects: &mut Objects,
        name: &[u16],
        mutable: bool,
        at: usize,
    ) -> Result<(), Escape> {
        if self.bindings.contains_key(name) {
            return Err(Escape::type_error(
                format!("'{}' is already declared in this realm", show(name)),
                at,
            ));
        }
        let global = self.global(objects)?;
        let key = objects.key(name).map_err(|why| Escape::refused(why, at))?;
        if objects.own_property(global, key)?.is_some() {
            return Err(Escape::type_error(
                format!(
                    "'{}' is already a property of the global object",
                    show(name)
                ),
                at,
            ));
        }
        let record = self.record(objects)?;
        let slot = objects.slot_count(record).unwrap_or_default();
        objects
            .with_slots(record, |slots, _| slots.push_uninitialized())
            .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))?;
        self.bindings
            .insert(name.to_vec(), Binding { slot, mutable });
        Ok(())
    }

    /// What a name means here.
    ///
    /// # Errors
    ///
    /// [`Missing::ACall`] for a property with a getter, and a fault for a
    /// reference this engine has lost.
    pub fn resolve(&self, objects: &Objects, name: &[u16]) -> Result<Resolved, Escape> {
        if let Some(binding) = self.bindings.get(name) {
            let record = self.record(objects)?;
            return Ok(match objects.slot(record, binding.slot) {
                Some(Held::Value(value)) => Resolved::Lexical(value),
                Some(Held::Uninitialized) => Resolved::Dead,
                None => return Err(Escape::fault(crate::object::Fault::Gone)),
            });
        }
        let global = self.global(objects)?;
        let Some(key) = objects.existing_key(name) else {
            // A name nothing has ever interned cannot be a property of
            // anything, so this is a miss rather than a reason to intern it —
            // which matters because `typeof somethingUndeclared` must not
            // allocate.
            return Ok(Resolved::Nothing);
        };
        match objects.get(global, key)? {
            Found::Missing => Ok(Resolved::Nothing),
            Found::Value(value) => Ok(Resolved::Property(value)),
            Found::Getter(_) => Err(Escape::NotBuiltYet(Missing::ACall)),
        }
    }

    /// Give a lexical binding its first value.
    ///
    /// # Errors
    ///
    /// A fault if the name is not one this realm declared, which is the
    /// compiler and the realm disagreeing and so this engine's own bug.
    pub fn initialize(
        &mut self,
        objects: &mut Objects,
        name: &[u16],
        value: Value,
    ) -> Result<(), Escape> {
        let Some(binding) = self.bindings.get(name).copied() else {
            return Err(Escape::fault(crate::object::Fault::Gone));
        };
        let record = self.record(objects)?;
        let wrote = objects
            .with_slots(record, |slots, barrier| {
                slots.set(barrier, binding.slot, value)
            })
            .unwrap_or_default();
        if wrote {
            Ok(())
        } else {
            Err(Escape::fault(crate::object::Fault::Gone))
        }
    }

    /// Assign to a name.
    ///
    /// The order is the lookup's, and the last case is the one that differs
    /// between the two modes: in sloppy code an assignment to a name nothing
    /// declares **makes a property of the global object**, and in strict code
    /// it is a `ReferenceError`. That single rule is most of what `"use strict"`
    /// is for.
    ///
    /// # Errors
    ///
    /// A `TypeError` for a `const` or a property that refuses, a
    /// `ReferenceError` for a dead zone or for strict code assigning to
    /// nothing, and [`Missing::ACall`] for a setter.
    pub fn assign(
        &mut self,
        objects: &mut Objects,
        name: &[u16],
        value: Value,
        strict: bool,
        at: usize,
    ) -> Result<(), Escape> {
        if let Some(binding) = self.bindings.get(name).copied() {
            let record = self.record(objects)?;
            match objects.slot(record, binding.slot) {
                Some(Held::Uninitialized) => {
                    return Err(Escape::reference_error(
                        format!("'{}' is used before it is declared", show(name)),
                        at,
                    ));
                }
                Some(Held::Value(_)) => {}
                None => return Err(Escape::fault(crate::object::Fault::Gone)),
            }
            if !binding.mutable {
                return Err(Escape::type_error(
                    format!("'{}' is a constant and cannot be assigned to", show(name)),
                    at,
                ));
            }
            objects
                .with_slots(record, |slots, barrier| {
                    slots.set(barrier, binding.slot, value)
                })
                .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))?;
            return Ok(());
        }

        let global = self.global(objects)?;
        let known = objects.existing_key(name);
        let there = match known {
            Some(key) => objects.has(global, key)?,
            None => false,
        };
        if !there && strict {
            return Err(Escape::reference_error(
                format!("'{}' is not defined", show(name)),
                at,
            ));
        }
        let key = match known {
            Some(key) => key,
            None => objects.key(name).map_err(|why| Escape::refused(why, at))?,
        };
        match objects.set(global, key, value)? {
            Set::Done => Ok(()),
            Set::Setter(_) => Err(Escape::NotBuiltYet(Missing::ACall)),
            Set::Refused => {
                if strict {
                    return Err(Escape::type_error(
                        format!("'{}' cannot be assigned to", show(name)),
                        at,
                    ));
                }
                // Sloppy code is told nothing, which is the language's oldest
                // and least defensible rule and is not ours to change.
                Ok(())
            }
        }
    }

    /// `delete a`, which sloppy code may write.
    ///
    /// # Errors
    ///
    /// A fault for a reference this engine has lost.
    pub fn delete(&mut self, objects: &mut Objects, name: &[u16]) -> Result<bool, Escape> {
        if self.bindings.contains_key(name) {
            // A declarative binding is not deletable, and the language answers
            // `false` rather than throwing.
            return Ok(false);
        }
        let global = self.global(objects)?;
        let Some(key) = objects.existing_key(name) else {
            return Ok(true);
        };
        Ok(objects.delete(global, key)?)
    }
}

/// A name as text, for a message a person reads.
fn show(name: &[u16]) -> String {
    String::from_utf16_lossy(name)
}

#[cfg(test)]
mod tests {
    use super::{Realm, Resolved};
    use crate::object::{Objects, Value};

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_let_shadows_a_property_of_the_global_object() {
        let mut objects = Objects::new();
        let Ok(mut realm) = Realm::new(&mut objects) else {
            panic!("an empty heap holds a realm");
        };
        let name = units("a");
        // The property is put there by an embedder rather than by a `var`,
        // because a `var` of that name would refuse the `let` below.
        let Ok(global) = realm.global(&objects) else {
            panic!("the realm has a global object");
        };
        let Ok(true) = objects.define_named(
            global,
            &name,
            crate::object::Property::plain(Value::Number(1.0)),
        ) else {
            panic!("an empty object takes a property");
        };
        assert_eq!(
            realm.resolve(&objects, &name),
            Ok(Resolved::Property(Value::Number(1.0)))
        );

        // Declaring a `let` over a property the embedder put there is the early
        // error the language specifies, which is why this realm refuses it.
        assert!(realm.declare_lexical(&mut objects, &name, true, 0).is_err());

        let other = units("b");
        assert!(realm.declare_lexical(&mut objects, &other, true, 0).is_ok());
        assert_eq!(realm.resolve(&objects, &other), Ok(Resolved::Dead));
        assert!(
            realm
                .initialize(&mut objects, &other, Value::Bool(true))
                .is_ok()
        );
        assert_eq!(
            realm.resolve(&objects, &other),
            Ok(Resolved::Lexical(Value::Bool(true)))
        );
    }

    #[test]
    fn a_constant_refuses_to_be_assigned_to() {
        let mut objects = Objects::new();
        let Ok(mut realm) = Realm::new(&mut objects) else {
            panic!("an empty heap holds a realm");
        };
        let name = units("a");
        assert!(realm.declare_lexical(&mut objects, &name, false, 0).is_ok());
        // In its dead zone it is a `ReferenceError` rather than a `TypeError`,
        // which is the order the specification checks in.
        assert!(
            realm
                .assign(&mut objects, &name, Value::Null, false, 0)
                .is_err()
        );
        assert!(realm.initialize(&mut objects, &name, Value::Null).is_ok());
        assert!(
            realm
                .assign(&mut objects, &name, Value::Bool(false), false, 0)
                .is_err()
        );
    }

    #[test]
    fn assigning_to_nothing_is_a_property_in_sloppy_code_and_an_error_in_strict() {
        let mut objects = Objects::new();
        let Ok(mut realm) = Realm::new(&mut objects) else {
            panic!("an empty heap holds a realm");
        };
        let name = units("loose");
        assert!(
            realm
                .assign(&mut objects, &name, Value::Number(1.0), false, 0)
                .is_ok()
        );
        assert_eq!(
            realm.resolve(&objects, &name),
            Ok(Resolved::Property(Value::Number(1.0)))
        );

        let strict = units("tight");
        assert!(
            realm
                .assign(&mut objects, &strict, Value::Number(1.0), true, 0)
                .is_err()
        );
        assert_eq!(realm.resolve(&objects, &strict), Ok(Resolved::Nothing));
    }
}

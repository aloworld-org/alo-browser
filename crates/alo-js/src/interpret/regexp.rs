/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Op::RegExp`: a regular expression literal, evaluated (queue item 74).
//!
//! Each evaluation makes a **new** object — `RegExpCreate` — so two passes of
//! a loop over `/a/g` have two `lastIndex`es, which is the language since
//! ES5. The compiled pattern is the unit's, made once when the script
//! compiled, and shared by every object the literal makes (ADR 0029 § 5).

use std::sync::Arc;

use crate::abrupt::{Escape, Internal};
use crate::object::{Property, Value};

use super::Engine;
use super::frame::Run;

impl Engine {
    /// Make the object, with its `lastIndex` at zero, and push it.
    pub(super) fn make_regexp(
        &mut self,
        run: &mut Run,
        which: u32,
        at: usize,
    ) -> Result<(), Escape> {
        let program = run
            .loaded()?
            .unit
            .pattern(which)
            .map(Arc::clone)
            .ok_or(Escape::Broken(Internal::JumpIsWrong))?;
        let above = self.realm.intrinsics().regexp_prototype(&self.objects)?;
        // Interning the name may allocate, and so does the object: the name
        // is held across the second, and the object goes on the stack before
        // it is given the property, so neither is only in a Rust local at a
        // safepoint.
        let scope = self.objects.heap_mut().open();
        let made = self.regexp_with_last_index(program, above, at);
        self.objects.heap_mut().close(scope);
        let held = made?;
        self.push(run, Value::Object(held))
    }

    /// `RegExpAlloc` and `RegExpInitialize`, with a scope open: `lastIndex`
    /// is writable, not enumerable and not configurable, and starts at zero.
    fn regexp_with_last_index(
        &mut self,
        program: Arc<crate::regexp::Program>,
        above: crate::heap::Ref,
        at: usize,
    ) -> Result<crate::heap::Ref, Escape> {
        let units: Vec<u16> = "lastIndex".encode_utf16().collect();
        let key = self
            .objects
            .key(&units)
            .map_err(|why| Escape::refused(why, at))?;
        if let Some(name) = key.reference() {
            self.objects.heap_mut().hold(name);
        }
        let held = self
            .objects
            .regexp(Some(above), program)
            .map_err(|why| Escape::refused(why, at))?;
        self.objects.heap_mut().hold(held);
        let defined = self.objects.define(
            held,
            key,
            Property::data(Value::Number(0.0), true, false, false),
        )?;
        if !defined {
            return Err(Escape::Broken(Internal::BuiltinIsWrong));
        }
        Ok(held)
    }
}

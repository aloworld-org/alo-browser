/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `FormData` (ADR 0051 § 7, queue item 472): a form's entry list, as an
//! object a page holds.
//!
//! `new FormData(form, submitter)` takes its list from **the function a
//! submission takes it from** — `alo-dom`'s *constructing the entry list*
//! ([`alo_dom::submission::entries`]) — so a page reading its own form and
//! the browser sending it cannot disagree about what the form holds. The
//! constructor and members are [`crate::interface::form_data`]'s.
//!
//! Every value here is text. A file field's entry is a `File`, which is the
//! File API's and not built (ADR 0051 § 8), so a form holding one is refused
//! by name when it is read rather than handed over with something that is
//! not a file.

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};

/// A `FormData`.
#[derive(Debug)]
pub struct FormData {
    own: Ordinary,
    list: Vec<(String, String)>,
}

impl FormData {
    /// An empty one, inheriting from `prototype`.
    pub fn new(prototype: Option<Ref>) -> Self {
        Self {
            own: Ordinary::with_prototype(prototype),
            list: Vec::new(),
        }
    }

    /// Its entry list, in order.
    pub fn list(&self) -> &[(String, String)] {
        &self.list
    }

    /// Its entry list, replaced by `list`: what the constructor does once.
    pub(crate) fn fill(&mut self, list: Vec<(String, String)>) {
        self.list = list;
    }
}

/// How the `FormData` constructor makes its instance.
pub fn make_form_data(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(FormData::new(prototype))
}

impl Internal for FormData {
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

impl Trace for FormData {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        let held: usize = self
            .list
            .iter()
            .map(|(name, value)| name.capacity().saturating_add(value.capacity()))
            .sum();
        self.own.footprint().saturating_add(held).saturating_add(
            self.list
                .capacity()
                .saturating_mul(size_of::<(String, String)>()),
        )
    }
}

impl Exotic for FormData {
    fn describe(&self) -> &'static str {
        "a FormData"
    }
}

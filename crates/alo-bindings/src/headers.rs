/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A response's `Headers` (ADR 0032 § 4, queue item 335): the headers the
//! browser process let the page read, in the order they came.
//!
//! **Read-only, and made only with a `Response`** ([`crate::response::make`]).
//! There is no `Headers` constructor on the global object, and no `append`,
//! `set` or `delete`: a `Headers` a page builds is its own item when a page
//! needs one. The members are [`crate::interface::headers`]'s.

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};

/// A `Headers`, read-only.
#[derive(Debug)]
pub struct Headers {
    own: Ordinary,
    list: Vec<(String, String)>,
}

impl Headers {
    /// The headers `list` holds, inheriting from `prototype`.
    pub(crate) fn new(prototype: Ref, list: Vec<(String, String)>) -> Self {
        Self {
            own: Ordinary::with_prototype(Some(prototype)),
            list,
        }
    }

    /// Fetch's *get* of `name`: every value under it, in order, joined by
    /// `", "`; [`None`] when it has none. The name is matched ignoring ASCII
    /// case, as a header name is.
    pub fn get(&self, name: &str) -> Option<String> {
        let values: Vec<&str> = self
            .list
            .iter()
            .filter(|(held, _)| held.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect();
        (!values.is_empty()).then(|| values.join(", "))
    }

    /// Whether it has any value under `name`.
    pub fn has(&self, name: &str) -> bool {
        self.list
            .iter()
            .any(|(held, _)| held.eq_ignore_ascii_case(name))
    }
}

impl Internal for Headers {
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

impl Trace for Headers {
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

impl Exotic for Headers {
    fn describe(&self) -> &'static str {
        "a Headers"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_is_every_value_under_its_name_ignoring_case() {
        let headers = Headers {
            own: Ordinary::with_prototype(None),
            list: vec![
                ("Accept".to_owned(), "a".to_owned()),
                ("content-type".to_owned(), "text/plain".to_owned()),
                ("ACCEPT".to_owned(), "b".to_owned()),
            ],
        };
        assert_eq!(headers.get("accept").as_deref(), Some("a, b"));
        assert_eq!(headers.get("Content-Type").as_deref(), Some("text/plain"));
        assert_eq!(headers.get("x-nothing"), None);
        assert!(headers.has("CONTENT-TYPE"));
        assert!(!headers.has("x-nothing"));
    }
}

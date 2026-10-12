/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `URLSearchParams` (queue item 472): a list of names and values, in
//! order, that a page builds and sends.
//!
//! The URL Standard's object holds its **list** and, for one a `URL` made, a
//! link back to that URL. No `URL` exists in this engine, so every one here
//! is a page's own, made by `new URLSearchParams(init)`, and holds the list
//! alone. Its constructor and members are
//! [`crate::interface::url_search_params`]'s, and reading `init` is
//! [`crate::search_params_init`]'s, which fills the list as it reads, so
//! that what it has converted is in the instance across every call it asks
//! for (ADR 0031 § 6).
//!
//! Written out it is the URL Standard's `application/x-www-form-urlencoded`
//! serializer over the list ([`alo_url::form::urlencoded`]) — the same
//! function a form's submission is written by in the browser process — and
//! that is both its `toString()` and the bytes of a fetch or a beacon whose
//! body it is ([`crate::body_init`]).

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};

/// A `URLSearchParams`.
#[derive(Debug)]
pub struct SearchParams {
    own: Ordinary,
    list: Vec<(String, String)>,
}

impl SearchParams {
    /// An empty one, inheriting from `prototype`.
    pub fn new(prototype: Option<Ref>) -> Self {
        Self {
            own: Ordinary::with_prototype(prototype),
            list: Vec::new(),
        }
    }

    /// Its list, in order.
    pub fn list(&self) -> &[(String, String)] {
        &self.list
    }

    /// Put `name` and `value` at the end of its list.
    pub(crate) fn append(&mut self, name: String, value: String) {
        self.list.push((name, value));
    }

    /// Set the first `name` in its list to `value`, or append the pair when
    /// it has none — what a record's conversion does with a second key that
    /// converts to a name already there.
    pub(crate) fn set_first(&mut self, name: String, value: String) {
        match self.list.iter_mut().find(|(held, _)| *held == name) {
            Some((_, held)) => *held = value,
            None => self.list.push((name, value)),
        }
    }

    /// Its list written as `application/x-www-form-urlencoded`.
    pub fn serialized(&self) -> String {
        alo_url::form::urlencoded(
            self.list
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )
    }
}

/// How the `URLSearchParams` constructor makes its instance.
pub fn make_search_params(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(SearchParams::new(prototype))
}

impl Internal for SearchParams {
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

impl Trace for SearchParams {
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

impl Exotic for SearchParams {
    fn describe(&self) -> &'static str {
        "a URLSearchParams"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_written_as_a_form_is() {
        let mut params = SearchParams::new(None);
        assert_eq!(params.serialized(), "");
        for (name, value) in [
            ("website", ""),
            ("name", "Ada Lovelace"),
            ("email", "ada@example.com"),
            ("message", "Two kilos a week, please.\nThanks & é"),
        ] {
            params.append(name.to_owned(), value.to_owned());
        }
        // What Chrome 154 sent as the contact form listener's body.
        assert_eq!(
            params.serialized(),
            "website=&name=Ada+Lovelace&email=ada%40example.com\
             &message=Two+kilos+a+week%2C+please.%0AThanks+%26+%C3%A9"
        );
    }

    #[test]
    fn a_second_key_of_the_same_name_sets_the_first() {
        let mut params = SearchParams::new(None);
        params.set_first("a".to_owned(), "1".to_owned());
        params.set_first("b".to_owned(), "2".to_owned());
        params.set_first("a".to_owned(), "3".to_owned());
        assert_eq!(params.serialized(), "a=3&b=2");
        params.append("a".to_owned(), "4".to_owned());
        assert_eq!(params.list().len(), 3);
    }
}

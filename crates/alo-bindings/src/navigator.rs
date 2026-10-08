/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `navigator`: what the browser says it is, as a page's script reads it
//! (ADR 0030, queue item 325).
//!
//! An embedder cell holding the two values the browser process told the
//! renderer with the page: the default `User-Agent` value and the platform
//! (§ 4). **Nothing here composes either.** They are composed in one file in
//! `alo-net`, sent as the `User-Agent` header from there, and handed to
//! [`introduce`] as an [`Identity`]; a renderer can tell a page only what it
//! was told. Every member derived from them — `appVersion` — is derived here
//! by HTML's steps rather than sent, so the two can never disagree.
//!
//! The members are [`crate::interface::navigator`]'s, on the prototype the
//! document cell holds like every other interface's.
//!
//! # `navigator` is a value, not yet a getter
//!
//! On a `Window`, `navigator` is a `[Replaceable]` attribute: reading it
//! answers the one `Navigator`, and assigning to it replaces it with what was
//! assigned. Until the global object is a `Window` (item 251) it is a data
//! property that is writable, enumerable and configurable — which is what
//! reading, assigning and deleting can observe of `[Replaceable]`, as
//! [`crate::install`] says of `document`.

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::interpret::Engine;
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property, Value};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::interface::Interface;

/// What the browser process says this browser is: ADR 0030 §§ 1–2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity<'a> {
    /// The default `User-Agent` value, which the header carries too.
    pub user_agent: &'a str,
    /// What `navigator.platform` answers.
    pub platform: &'a str,
}

/// The `Navigator` a page's script holds.
#[derive(Debug)]
pub struct Navigator {
    own: Ordinary,
    user_agent: Vec<u16>,
    platform: Vec<u16>,
}

impl Navigator {
    /// The default `User-Agent` value, as the page reads it.
    pub fn user_agent(&self) -> &[u16] {
        &self.user_agent
    }

    /// The platform.
    pub fn platform(&self) -> &[u16] {
        &self.platform
    }

    /// `appVersion`, by HTML's steps in the Gecko compatibility mode
    /// (ADR 0030 § 5).
    pub fn app_version(&self) -> Vec<u16> {
        app_version(&String::from_utf16_lossy(&self.user_agent))
            .encode_utf16()
            .collect()
    }
}

/// HTML's `appVersion` getter steps, for the Gecko compatibility mode, over
/// a user agent string: empty unless it starts `Mozilla/5.0 (`; then what
/// follows `Mozilla/`, `"5.0 (Windows)"` for any Windows, and otherwise up
/// to the first `;` with a `)` after it.
pub fn app_version(user_agent: &str) -> String {
    let Some(trail) = user_agent
        .strip_prefix("Mozilla/")
        .filter(|trail| trail.starts_with("5.0 ("))
    else {
        return String::new();
    };
    if trail.starts_with("5.0 (Windows") {
        return "5.0 (Windows)".to_owned();
    }
    let prefix = trail.split(';').next().unwrap_or(trail);
    format!("{prefix})")
}

impl Internal for Navigator {
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

impl Trace for Navigator {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own
            .footprint()
            .saturating_add(self.user_agent.capacity().saturating_mul(2))
            .saturating_add(self.platform.capacity().saturating_mul(2))
    }
}

impl Exotic for Navigator {
    fn describe(&self) -> &'static str {
        "a Navigator"
    }
}

/// Make the page's `Navigator` from `identity` and put it on `engine`'s
/// global object as `navigator`, answering it.
///
/// Called after [`crate::install`], whose document cell holds
/// `Navigator.prototype`.
///
/// **A safepoint.** `cell` must be rooted by the caller.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold it; a fault when `cell` is not
/// a document cell or its interfaces were never made; a `TypeError` when the
/// global object already has a `navigator` — an embedder introducing twice.
pub fn introduce(engine: &mut Engine, cell: Ref, identity: Identity<'_>) -> Result<Ref, Escape> {
    let global = engine.global()?;
    let objects = engine.objects();
    let name: Vec<u16> = "navigator".encode_utf16().collect();
    // A configurable property would take a second definition, so the first
    // is looked for: a page has had no chance to run, and one already there
    // is an embedder introducing twice.
    if let Some(key) = objects.existing_key(&name)
        && objects.own_property(global, key)?.is_some()
    {
        return Err(Escape::type_error("this realm already has a navigator", 0));
    }
    let prototype = objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .interfaces()
        .prototype(Interface::Navigator)
        .ok_or(Escape::fault(Fault::Gone))?;
    let navigator = Navigator {
        own: Ordinary::with_prototype(Some(prototype)),
        user_agent: identity.user_agent.encode_utf16().collect(),
        platform: identity.platform.encode_utf16().collect(),
    };
    let made = objects
        .foreign(Box::new(navigator))
        .map_err(|why| Escape::refused(why, 0))?;
    let scope = objects.heap_mut().open();
    objects.heap_mut().hold(made);
    let property = Property::data(Value::Object(made), true, true, true);
    let defined = objects.define_named(global, &name, property);
    objects.heap_mut().close(scope);
    match defined {
        Ok(true) => Ok(made),
        // A fresh global object refusing a new property is a reference to
        // something else: this crate's bug, not a page's.
        Ok(false) => Err(Escape::fault(Fault::NotAnObject)),
        Err(named) => Err(Escape::named(named, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_is_htmls_gecko_steps_for_each_row_of_adr_0030() {
        for (user_agent, answer) in [
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) alo/0.0",
                "5.0 (Macintosh)",
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) alo/0.0",
                "5.0 (Windows)",
            ),
            ("Mozilla/5.0 (X11; Linux x86_64) alo/0.0", "5.0 (X11)"),
        ] {
            assert_eq!(app_version(user_agent), answer, "{user_agent}");
        }
    }

    #[test]
    fn app_version_is_empty_for_a_string_html_does_not_read() {
        for user_agent in ["", "alo/0.0", "Mozilla/4.0 (X11)", "Mozilla/5.0(X11)"] {
            assert_eq!(app_version(user_agent), "", "{user_agent:?}");
        }
        // No `;`: the whole trail, closed.
        assert_eq!(app_version("Mozilla/5.0 (X11"), "5.0 (X11)");
    }
}

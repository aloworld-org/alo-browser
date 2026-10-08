/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The `Response` a fetch is settled with (ADR 0032 § 4, queue item 335).
//!
//! Both are what the browser process **let through**: it filtered the
//! response before the bytes left it, so an opaque one arrives as status 0
//! with no URL, no headers and no body, and nothing here could show a page
//! more than that because nothing more is in this process. This cell holds
//! it as it came, with its headers in a [`Headers`] of their own, and the
//! members are [`crate::interface::response`]'s.
//!
//! **Read-only, and made only by a fetch.** No `Response` or `Headers`
//! constructor is on the global object, and nothing changes either once it
//! is made: `Request`, `Headers` and `Response` as objects a page constructs
//! are each their own item when a page needs one (ADR 0032, *What this does
//! not decide*). The body is read once, whole, by `text()`; a second read is
//! refused as Fetch refuses one.

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::headers::Headers;
use crate::interface::Interface;

/// Which of Fetch's filtered responses an answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The document's own origin.
    Basic,
    /// Another origin that agreed to be read.
    Cors,
    /// Another origin, not readable.
    Opaque,
    /// A redirect the page asked to stop at.
    OpaqueRedirect,
}

impl Kind {
    /// What `Response.type` answers.
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Basic => "basic",
            Kind::Cors => "cors",
            Kind::Opaque => "opaque",
            Kind::OpaqueRedirect => "opaqueredirect",
        }
    }
}

/// A response as the page may see it — what the browser process sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Responded {
    /// Which filtered response it is.
    pub kind: Kind,
    /// The status, or 0.
    pub status: u16,
    /// The status text, as the server said it.
    pub status_text: String,
    /// Where it came from, after redirects, or nothing.
    pub url: Option<String>,
    /// Whether a redirect was followed on the way.
    pub redirected: bool,
    /// The headers the page may read, in order.
    pub headers: Vec<(String, String)>,
    /// The body, whole.
    pub body: Vec<u8>,
}

/// A `Response`.
#[derive(Debug)]
pub struct Response {
    own: Ordinary,
    kind: Kind,
    status: u16,
    status_text: String,
    url: String,
    redirected: bool,
    body: Vec<u8>,
    /// Whether `text()` has read the body.
    used: bool,
    /// Its one `Headers`, made with it.
    headers: Field,
}

impl Response {
    /// Which filtered response it is.
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The status, or 0.
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Whether the status is in the range 200 to 299.
    pub const fn ok(&self) -> bool {
        self.status >= 200 && self.status <= 299
    }

    /// The status text.
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Its URL, or the empty string for none.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Whether a redirect was followed on the way.
    pub const fn redirected(&self) -> bool {
        self.redirected
    }

    /// Whether its body has been read.
    pub const fn body_used(&self) -> bool {
        self.used
    }

    /// Its `Headers`.
    pub const fn headers(&self) -> Option<Ref> {
        self.headers.get()
    }

    /// Read the body, once: its bytes as text, decoded as UTF-8 with a
    /// leading byte order mark dropped — Fetch's *UTF-8 decode* — or [`None`]
    /// if it has been read before.
    pub(crate) fn read_text(&mut self) -> Option<Vec<u16>> {
        if self.used {
            return None;
        }
        self.used = true;
        let bytes = self
            .body
            .strip_prefix(b"\xEF\xBB\xBF")
            .unwrap_or(&self.body);
        let text = String::from_utf8_lossy(bytes).encode_utf16().collect();
        // Read, and never read again: the bytes are let go of now.
        self.body = Vec::new();
        Some(text)
    }
}

/// Make the `Response` `responded` describes, and its `Headers`, inheriting
/// from the interfaces' prototypes the document `cell` holds.
///
/// **A safepoint.** `cell` must be rooted by the caller. The `Headers` is
/// held in a scope until the `Response` holds it; the `Response` is in a
/// Rust local once this answers, and the caller puts it somewhere the
/// collector walks before anything else allocates.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold them, and a fault when `cell`
/// is not a document cell or its interfaces were never made.
pub fn make(objects: &mut Objects, cell: Ref, responded: Responded) -> Result<Ref, Escape> {
    let prototype = |objects: &Objects, interface: Interface| {
        objects
            .embedded::<DocumentCell>(cell)
            .ok_or(Escape::fault(Fault::NotAnObject))?
            .interfaces()
            .prototype(interface)
            .ok_or(Escape::fault(Fault::Gone))
    };
    let headers_prototype = prototype(objects, Interface::Headers)?;
    let response_prototype = prototype(objects, Interface::Response)?;
    let Responded {
        kind,
        status,
        status_text,
        url,
        redirected,
        headers,
        body,
    } = responded;
    let headers = objects
        .foreign(Box::new(Headers::new(headers_prototype, headers)))
        .map_err(|why| Escape::refused(why, 0))?;
    let scope = objects.heap_mut().open();
    objects.heap_mut().hold(headers);
    let made = objects.foreign(Box::new(Response {
        own: Ordinary::with_prototype(Some(response_prototype)),
        kind,
        status,
        status_text,
        url: url.unwrap_or_default(),
        redirected,
        body,
        used: false,
        headers: Field::holding(headers),
    }));
    objects.heap_mut().close(scope);
    made.map_err(|why| Escape::refused(why, 0))
}

impl Internal for Response {
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

impl Trace for Response {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
        self.headers.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own
            .footprint()
            .saturating_add(self.status_text.capacity())
            .saturating_add(self.url.capacity())
            .saturating_add(self.body.capacity())
    }
}

impl Exotic for Response {
    fn describe(&self) -> &'static str {
        "a Response"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_read_once_as_utf8_without_its_byte_order_mark() {
        let mut response = Response {
            own: Ordinary::with_prototype(None),
            kind: Kind::Basic,
            status: 200,
            status_text: "OK".to_owned(),
            url: String::new(),
            redirected: false,
            body: b"\xEF\xBB\xBFh\xC3\xA9\xFF".to_vec(),
            used: false,
            headers: Field::empty(),
        };
        assert!(response.ok() && !response.body_used());
        let read = response
            .read_text()
            .map(|units| String::from_utf16_lossy(&units));
        assert_eq!(read.as_deref(), Some("hé\u{FFFD}"));
        assert!(response.body_used());
        assert_eq!(response.read_text(), None);
    }

    #[test]
    fn ok_is_the_two_hundreds_and_nothing_else() {
        for (status, ok) in [
            (0, false),
            (199, false),
            (200, true),
            (299, true),
            (300, false),
        ] {
            let response = Response {
                own: Ordinary::with_prototype(None),
                kind: Kind::Basic,
                status,
                status_text: String::new(),
                url: String::new(),
                redirected: false,
                body: Vec::new(),
                used: false,
                headers: Field::empty(),
            };
            assert_eq!(response.ok(), ok, "{status}");
        }
    }
}

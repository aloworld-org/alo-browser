/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page may never put on a request it makes: Fetch's forbidden
//! methods and forbidden request headers, and what a method and a header are
//! at all.
//!
//! # Why these are a list a page cannot reach
//!
//! The browser sets these, and a page that could set them would be a page
//! speaking as the browser. A `Cookie` a script wrote is a session it
//! invented, an `Origin` is an identity it claimed, a `Host` or a
//! `Content-Length` is a request it reframed. `TRACE` echoes a request back,
//! credentials and all, to whoever sent it; `CONNECT` turns a proxy into a
//! tunnel. Fetch forbids each of them, and the specification's lists are taken
//! here as they are.
//!
//! # Who asks
//!
//! Two sides, with one list. A page's bindings drop a forbidden header as the
//! specification's `Headers` guard drops one, so an honest renderer never
//! sends one. The browser process asks again of every ask a renderer sends
//! (ADR 0032 § 2), and there an answer of *forbidden* is not corrected but
//! refused: no page can produce one, so only a renderer that was taken over
//! can. Two spellings of this list would be one of those sides being wrong.

use crate::http::is_token_byte;

/// The methods no page may use, compared without regard to case.
const FORBIDDEN_METHODS: [&str; 3] = ["CONNECT", "TRACE", "TRACK"];

/// The methods Fetch writes in capitals whatever case a page wrote them in.
/// Every other method is sent as the page spelled it.
const NORMALISED_METHODS: [&str; 6] = ["DELETE", "GET", "HEAD", "OPTIONS", "POST", "PUT"];

/// The request headers only the browser sets, lowercased.
const FORBIDDEN_NAMES: [&str; 21] = [
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "expect",
    "host",
    "keep-alive",
    "origin",
    "referer",
    "set-cookie",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "via",
];

/// Headers that name a method a server may obey instead of the real one, and
/// so are forbidden when the method they name is.
const NAMING_A_METHOD: [&str; 3] = [
    "x-http-method",
    "x-http-method-override",
    "x-method-override",
];

/// Whether `method` is a method at all: one or more of HTTP's token bytes.
pub fn is_a_method(method: &str) -> bool {
    !method.is_empty() && method.bytes().all(is_token_byte)
}

/// Whether `method` is one no page may use.
pub fn is_forbidden_method(method: &str) -> bool {
    FORBIDDEN_METHODS
        .iter()
        .any(|forbidden| forbidden.eq_ignore_ascii_case(method))
}

/// `method` as Fetch normalises it: one of the six it knows in capitals,
/// anything else as written.
pub fn normalised_method(method: &str) -> String {
    NORMALISED_METHODS
        .iter()
        .find(|known| known.eq_ignore_ascii_case(method))
        .map_or_else(|| method.to_owned(), |known| (*known).to_owned())
}

/// Whether `name` is a header name at all: one or more token bytes.
pub fn is_a_header_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_token_byte)
}

/// Whether `value` is a header value as Fetch keeps one: no NUL, carriage
/// return or line feed anywhere, which is what would end the header early and
/// start another, and no space or tab at either end, which Fetch strips
/// before it keeps a value.
pub fn is_a_header_value(value: &str) -> bool {
    let bytes = value.as_bytes();
    let blank = |byte: &u8| matches!(byte, b' ' | b'\t');
    !bytes
        .iter()
        .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
        && !bytes.first().is_some_and(blank)
        && !bytes.last().is_some_and(blank)
}

/// Whether a request header with this name and value is one only the browser
/// may set.
pub fn is_forbidden_request_header(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if FORBIDDEN_NAMES.contains(&name.as_str())
        || name.starts_with("proxy-")
        || name.starts_with("sec-")
    {
        return true;
    }
    NAMING_A_METHOD.contains(&name.as_str())
        && value
            .split(',')
            .any(|named| is_forbidden_method(named.trim_matches([' ', '\t'])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browsers_own_headers_are_forbidden_in_any_case() {
        for name in [
            "Cookie",
            "HOST",
            "origin",
            "Referer",
            "Content-Length",
            "Transfer-Encoding",
            "Sec-Fetch-Mode",
            "Proxy-Authorization",
            "set-cookie",
        ] {
            assert!(is_forbidden_request_header(name, "x"), "{name}");
        }
        for name in [
            "Accept",
            "Content-Type",
            "X-Requested-With",
            "Authorization",
        ] {
            assert!(!is_forbidden_request_header(name, "x"), "{name}");
        }
    }

    #[test]
    fn a_header_naming_a_forbidden_method_is_forbidden_and_naming_another_is_not() {
        assert!(is_forbidden_request_header(
            "X-HTTP-Method-Override",
            "GET, trace"
        ));
        assert!(is_forbidden_request_header(
            "x-method-override",
            "\tCONNECT "
        ));
        assert!(!is_forbidden_request_header(
            "X-HTTP-Method-Override",
            "DELETE"
        ));
    }

    #[test]
    fn methods_are_tokens_three_are_forbidden_and_six_are_written_in_capitals() {
        assert!(is_a_method("PATCH"));
        assert!(!is_a_method(""));
        assert!(!is_a_method("GET /evil HTTP/1.1\r\nHost: x"));
        assert!(is_forbidden_method("trace"));
        assert!(is_forbidden_method("Connect"));
        assert!(!is_forbidden_method("POST"));
        assert_eq!(normalised_method("post"), "POST");
        assert_eq!(normalised_method("Delete"), "DELETE");
        assert_eq!(normalised_method("patch"), "patch", "not one of the six");
    }

    #[test]
    fn a_header_that_would_end_early_or_carries_blank_ends_is_not_a_header() {
        assert!(is_a_header_name("X-Thing"));
        assert!(!is_a_header_name("X Thing"));
        assert!(!is_a_header_name("X-Thing:"));
        assert!(!is_a_header_name(""));
        assert!(is_a_header_value("a b"));
        assert!(is_a_header_value(""));
        assert!(!is_a_header_value("a\r\nHost: evil"));
        assert!(!is_a_header_value("a\0"));
        assert!(!is_a_header_value(" a"));
        assert!(!is_a_header_value("a\t"));
    }
}

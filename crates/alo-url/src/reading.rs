/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page reads of a URL, part by part.
//!
//! The URL Standard's API answers nine strings about a URL — `href`,
//! `origin`, `protocol`, `host`, `hostname`, `port`, `pathname`, `search` and
//! `hash` — and HTML's `Location` (queue item 360), its hyperlink elements'
//! `a.hostname` and the `URL` interface all answer the same nine by the same
//! steps. So the steps are written once, here, over our [`Url`], and every
//! interface that reads a URL asks this file rather than taking its parts
//! apart again.
//!
//! Each is a string and never a failure: every [`Url`] is one the parser
//! already accepted, and a part it does not have is the empty string, as the
//! standard says.

use crate::origin::Origin;
use crate::parts::Url;

/// `href`: the whole URL, serialised.
pub fn href(url: &Url) -> String {
    url.serialised.clone()
}

/// `origin`: the serialisation of the URL's origin — `null` for an opaque
/// one, which is every scheme without a host and a port ([`Origin::of`]).
pub fn origin(url: &Url) -> String {
    Origin::of(url).to_string()
}

/// `protocol`: the scheme and a `:`.
pub fn protocol(url: &Url) -> String {
    format!("{}:", url.scheme)
}

/// `host`: the host, and `:` and the port when one is written that is not
/// the scheme's own; empty for a URL with no host.
pub fn host(url: &Url) -> String {
    match (&url.host, url.port) {
        (None, _) => String::new(),
        (Some(host), None) => host.to_string(),
        (Some(host), Some(port)) => format!("{host}:{port}"),
    }
}

/// `hostname`: the host alone — an IPv6 address in its brackets, as the
/// standard serialises one — or empty.
pub fn hostname(url: &Url) -> String {
    url.host
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// `port`: the port as decimal digits when one is written that is not the
/// scheme's own, and otherwise empty.
pub fn port(url: &Url) -> String {
    url.port.map(|port| port.to_string()).unwrap_or_default()
}

/// `pathname`: the path, as it is serialised — `/` and up for a URL with a
/// hierarchical path, and the opaque path itself for one like `about:blank`.
pub fn pathname(url: &Url) -> String {
    url.path.clone()
}

/// `search`: `?` and the query, or empty when there is no query or it is
/// empty.
pub fn search(url: &Url) -> String {
    prefixed('?', url.query.as_deref())
}

/// `hash`: `#` and the fragment, or empty when there is no fragment or it is
/// empty.
pub fn hash(url: &Url) -> String {
    prefixed('#', url.fragment.as_deref())
}

/// `mark` and `part`, or the empty string for a part that is absent or empty
/// — the standard's rule for both `search` and `hash`.
fn prefixed(mark: char, part: Option<&str>) -> String {
    match part {
        None | Some("") => String::new(),
        Some(part) => format!("{mark}{part}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    /// All nine readings of `input`, in the order the standard lists them.
    fn read(input: &str) -> [String; 9] {
        let Ok(url) = parse(input) else {
            return core::array::from_fn(|_| format!("{input} did not parse"));
        };
        [
            href(&url),
            origin(&url),
            protocol(&url),
            host(&url),
            hostname(&url),
            port(&url),
            pathname(&url),
            search(&url),
            hash(&url),
        ]
    }

    #[test]
    fn an_address_with_every_part_reads_each_one() {
        assert_eq!(
            read("https://shop.example.com:8443/a/b?x=1&y=2#top"),
            [
                "https://shop.example.com:8443/a/b?x=1&y=2#top",
                "https://shop.example.com:8443",
                "https:",
                "shop.example.com:8443",
                "shop.example.com",
                "8443",
                "/a/b",
                "?x=1&y=2",
                "#top",
            ]
        );
    }

    #[test]
    fn an_address_with_only_a_host_reads_the_rest_as_empty() {
        assert_eq!(
            read("https://nordwind.alosites.com/"),
            [
                "https://nordwind.alosites.com/",
                "https://nordwind.alosites.com",
                "https:",
                "nordwind.alosites.com",
                "nordwind.alosites.com",
                "",
                "/",
                "",
                "",
            ]
        );
    }

    #[test]
    fn the_schemes_own_port_is_never_read_as_written() {
        // `:443` on `https` is the same URL as none, so nothing says a port.
        let [href, origin, _, host, _, port, ..] = read("https://example.com:443/");
        assert_eq!(href, "https://example.com/");
        assert_eq!(origin, "https://example.com");
        assert_eq!(host, "example.com");
        assert_eq!(port, "");
        // And on `http`, a port that is `https`'s is not the default.
        let [_, origin, _, host, _, port, ..] = read("http://example.com:443/");
        assert_eq!(origin, "http://example.com:443");
        assert_eq!(host, "example.com:443");
        assert_eq!(port, "443");
    }

    #[test]
    fn an_empty_query_or_fragment_reads_as_none_at_all() {
        let [href, .., search, hash] = read("https://example.com/p?#");
        // The serialisation keeps both marks; the readings do not.
        assert_eq!(href, "https://example.com/p?#");
        assert_eq!(search, "");
        assert_eq!(hash, "");
    }

    #[test]
    fn an_ipv6_host_is_read_in_its_brackets() {
        let [_, origin, _, host, hostname, port, ..] = read("http://[::1]:8080/");
        assert_eq!(origin, "http://[::1]:8080");
        assert_eq!(host, "[::1]:8080");
        assert_eq!(hostname, "[::1]");
        assert_eq!(port, "8080");
    }

    #[test]
    fn about_blank_has_an_opaque_origin_and_an_opaque_path() {
        assert_eq!(
            read("about:blank"),
            ["about:blank", "null", "about:", "", "", "", "blank", "", ""]
        );
        // The made one reads as the parsed one does.
        assert_eq!(pathname(&Url::about_blank()), "blank");
        assert_eq!(origin(&Url::about_blank()), "null");
    }

    #[test]
    fn a_data_url_reads_as_a_scheme_and_a_path() {
        let [_, origin, protocol, host, _, _, pathname, ..] = read("data:text/plain,hi");
        assert_eq!(origin, "null");
        assert_eq!(protocol, "data:");
        assert_eq!(host, "");
        assert_eq!(pathname, "text/plain,hi");
    }

    #[test]
    fn what_the_parser_escaped_is_read_escaped() {
        let [_, .., pathname, search, hash] = read("https://example.com/a b?c d#e f");
        assert_eq!(pathname, "/a%20b");
        assert_eq!(search, "?c%20d");
        assert_eq!(hash, "#e%20f");
    }
}

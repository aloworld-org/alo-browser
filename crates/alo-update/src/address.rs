/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where updates come from: an address a build is given when it is made,
//! never one written in this repository (ADR 0116 § 3, the owner's rule of
//! 9 November 2026).
//!
//! # Two files under it, per platform
//!
//! ```text
//! <address>/macos-arm64/manifest
//! <address>/macos-arm64/manifest.minisig
//! ```
//!
//! The host is anything that serves a file. alo trusts the signature, not
//! the host or its certificate — but it still asks over HTTPS, so a network
//! between does not learn which version is fetched. Plain HTTP is read only
//! for a loopback host (`127.0.0.1`, `[::1]`, `localhost`): a feed on this
//! machine, as a test's is. The same rule holds for a manifest's `package`
//! ([`allowed`]).
//!
//! # Nothing about the person in it
//!
//! No query and no fragment, in the address or in a package's: the check
//! is two plain `GET`s that carry no identifier (§ 5), and an address with
//! a query would be one.

use alo_url::{Host, Url};

/// The most bytes an address is read to.
pub const LONGEST: usize = 2048;

/// The address updates come from, as a build was given it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// It, ending in no `/`.
    base: String,
}

impl Address {
    /// The address `text` names, or why it is not one updates may come from.
    ///
    /// # Errors
    ///
    /// Why, in words: not a URL, a scheme other than HTTPS (or HTTP to this
    /// machine), a query or a fragment, or longer than [`LONGEST`].
    pub fn read(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.len() > LONGEST {
            return Err(format!("an address longer than {LONGEST} bytes"));
        }
        let url = alo_url::parse(text).map_err(|_| format!("{text:?} is not an address"))?;
        allowed(&url)?;
        Ok(Self {
            base: url.serialised.trim_end_matches('/').to_owned(),
        })
    }

    /// Where this platform's manifest is.
    ///
    /// # Errors
    ///
    /// Why, if the joined address does not parse, which no address read
    /// here and no platform word can cause.
    pub fn manifest(&self, platform: &str) -> Result<Url, String> {
        self.under(platform, "manifest")
    }

    /// Where this platform's manifest's signature is.
    ///
    /// # Errors
    ///
    /// As [`Address::manifest`].
    pub fn signature(&self, platform: &str) -> Result<Url, String> {
        self.under(platform, "manifest.minisig")
    }

    /// `<address>/<platform>/<file>`.
    fn under(&self, platform: &str, file: &str) -> Result<Url, String> {
        let text = format!("{}/{platform}/{file}", self.base);
        alo_url::parse(&text).map_err(|why| why.to_string())
    }

    /// The address as it was read.
    pub fn as_str(&self) -> &str {
        &self.base
    }
}

/// Whether an update may be fetched from `url`: HTTPS, or HTTP to this
/// machine; no query, no fragment, no user name or password.
///
/// # Errors
///
/// Why not, in words.
pub fn allowed(url: &Url) -> Result<(), String> {
    let loopback = match &url.host {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(name)) => name == "localhost",
        None => return Err(format!("{} names no host", url.serialised)),
    };
    match url.scheme.as_str() {
        "https" => {}
        "http" if loopback => {}
        other => {
            return Err(format!(
                "{} is fetched over {other}, and an update comes over HTTPS",
                url.serialised
            ));
        }
    }
    if url.query.is_some() || url.fragment.is_some() {
        return Err(format!(
            "{} carries a query or a fragment, and an update's address carries nothing",
            url.serialised
        ));
    }
    if alo_url::includes_credentials(url) {
        return Err(format!("{} carries a user name", url.serialised));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_https_address_names_its_two_files_per_platform() {
        let address = Address::read("https://updates.example.org/alo/").expect("an address");
        assert_eq!(address.as_str(), "https://updates.example.org/alo");
        assert_eq!(
            address
                .manifest("macos-arm64")
                .map(|url| url.serialised)
                .as_deref(),
            Ok("https://updates.example.org/alo/macos-arm64/manifest")
        );
        assert_eq!(
            address
                .signature("macos-arm64")
                .map(|url| url.serialised)
                .as_deref(),
            Ok("https://updates.example.org/alo/macos-arm64/manifest.minisig")
        );
    }

    #[test]
    fn plain_http_only_to_this_machine_and_never_a_query() {
        assert!(Address::read("http://127.0.0.1:8123").is_ok());
        assert!(Address::read("http://localhost:8123/feed").is_ok());
        assert!(Address::read("http://[::1]:8123/feed").is_ok());
        for refused in [
            "http://updates.example.org/",
            "ftp://updates.example.org/",
            "file:///tmp/feed",
            "https://updates.example.org/?id=1",
            "https://updates.example.org/#me",
            "https://me:secret@updates.example.org/",
            "not an address",
            "",
        ] {
            assert!(Address::read(refused).is_err(), "{refused}");
        }
        assert!(Address::read(&format!("https://e.org/{}", "a".repeat(LONGEST))).is_err());
    }
}

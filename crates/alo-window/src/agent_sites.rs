/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which sites an agent outside this browser may reach (ADR 0055 § 8, queue
//! item 497).
//!
//! # A refusal, not a grant
//!
//! alo OS's distinction, and binding wherever this runs: a grant their
//! kernel cannot see is not enumerated, visible, revocable or expiring in
//! their sense, so what is held here is **ours, and only ever narrows**.
//! Whatever lets a program open the socket at all — the operating system's
//! permissions on it ([`crate::socket_place`]) and any grant over this
//! application — is the ceiling, and nothing here can raise it. With no
//! sites named, [`AgentSites::Every`] refuses nothing that ceiling allows;
//! with some named, every other site is refused by name.
//!
//! It is never presented to a person as another system's grant, and the
//! words this browser says about it say whose it is.
//!
//! # Made by a person, never by an agent
//!
//! ADR 0026: *no agent can make, answer or revoke* a grant. So the sites are
//! named by the person who starts the browser (`alo --agent-site ADDRESS`),
//! and nothing over the socket can name, widen or read them. They hold for
//! every agent connected and end with the process.
//!
//! # A site is ADR 0005's
//!
//! The scheme and the registrable domain ([`Site`]), so naming
//! `https://example.com` lets an agent reach `https://www.example.com` and
//! not `http://example.com`. A document whose origin is opaque — `data:`,
//! `about:blank`, a local file — has no site to name, so it cannot be named,
//! and under a restriction an agent never reaches one.

use alo_renderer::site::Site;
use alo_url::Url;
use core::fmt;

/// Which sites an agent may reach.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AgentSites {
    /// Every site the socket's own grant reaches: nothing here narrows it.
    #[default]
    Every,
    /// These and no others.
    Only(Vec<Site>),
}

/// Why an address a person named cannot be one of the sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotASite {
    /// It is not an address.
    NotAnAddress(String),
    /// Its origin is opaque, so it has no site to name.
    NoSite(String),
}

impl fmt::Display for NotASite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotASite::NotAnAddress(named) => write!(f, "{named:?} is not an address"),
            NotASite::NoSite(named) => write!(
                f,
                "{named:?} has no site an agent could be kept to: its origin is opaque"
            ),
        }
    }
}

impl AgentSites {
    /// The sites of `addresses`, which a person named; every site if they
    /// named none.
    ///
    /// # Errors
    ///
    /// The first that is not an address, or has no site.
    pub fn named<'a>(addresses: impl IntoIterator<Item = &'a str>) -> Result<Self, NotASite> {
        let mut sites: Vec<Site> = Vec::new();
        for named in addresses {
            let url =
                alo_url::parse(named).map_err(|_| NotASite::NotAnAddress(named.to_owned()))?;
            let site = Site::of(&url);
            if site.is_alone() {
                return Err(NotASite::NoSite(named.to_owned()));
            }
            if !sites.contains(&site) {
                sites.push(site);
            }
        }
        Ok(if sites.is_empty() {
            Self::Every
        } else {
            Self::Only(sites)
        })
    }

    /// The site of the page at `address`, if an agent may reach it, or the
    /// site it may not.
    ///
    /// # Errors
    ///
    /// The site, when it is not one of these.
    pub fn reach(&self, address: &Url) -> Result<Site, Site> {
        let site = Site::of(address);
        match self {
            Self::Every => Ok(site),
            Self::Only(sites) if sites.contains(&site) => Ok(site),
            Self::Only(_) => Err(site),
        }
    }

    /// Whether an agent may reach the page at `address`.
    pub fn allows(&self, address: &Url) -> bool {
        self.reach(address).is_ok()
    }
}

impl fmt::Display for AgentSites {
    /// What a person is told, in this browser's own words.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Every => f.write_str("every site"),
            Self::Only(sites) => {
                f.write_str("only ")?;
                for (at, site) in sites.iter().enumerate() {
                    if at > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{site}")?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
    }

    #[test]
    fn nothing_named_narrows_nothing() {
        assert_eq!(AgentSites::named([]), Ok(AgentSites::Every));
        assert!(AgentSites::Every.allows(&url("https://example.com/")));
        assert!(AgentSites::Every.allows(&url("data:text/html,hi")));
    }

    #[test]
    fn a_site_named_is_the_scheme_and_the_registrable_domain() {
        let sites = AgentSites::named(["https://example.com/a", "https://www.example.com"]);
        let Ok(sites) = sites else {
            panic!("{sites:?}");
        };
        assert_eq!(sites.to_string(), "only https://example.com", "said once");
        assert!(sites.allows(&url("https://shop.example.com/basket")));
        assert!(sites.allows(&url("https://example.com:8443/")));
        assert!(!sites.allows(&url("http://example.com/")), "another scheme");
        assert!(!sites.allows(&url("https://example.org/")));
        assert_eq!(
            sites
                .reach(&url("https://www.example.org/"))
                .map_err(|site| site.to_string()),
            Err("https://example.org".to_owned())
        );
    }

    #[test]
    fn under_a_restriction_a_document_with_no_site_is_never_reached() {
        let Ok(sites) = AgentSites::named(["https://example.com"]) else {
            panic!("a site");
        };
        assert!(!sites.allows(&url("data:text/html,hi")));
        assert!(!sites.allows(&Url::about_blank()));
        assert!(!sites.allows(&url("file:///tmp/page.html")));
    }

    #[test]
    fn what_cannot_be_a_site_is_refused_by_name() {
        assert_eq!(
            AgentSites::named(["https://example.com", "not an address"]),
            Err(NotASite::NotAnAddress("not an address".to_owned()))
        );
        assert_eq!(
            AgentSites::named(["about:blank"]),
            Err(NotASite::NoSite("about:blank".to_owned()))
        );
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether alo's agent may open a site, and whether the person is asked
//! first (queue item 821, ADR 0103, `docs/design/shell.md`, *Beginning with
//! no page*; the owner's design of 8 November 2026). Decided by the browser
//! process from what the person asked, what the run found by searching and
//! the person's limits ([`Limits`]) — never by the model, which only
//! proposes a result's number or an address.
//!
//! # In order
//!
//! 1. **An address**: a result's, by its number among the run's searches
//!    ([`FoundSites`]), or what the model wrote, read as the box reads what
//!    a person types (`alo_renderer::typed::address`) — so only `http` and
//!    `https` are opened, never `file:` or any other scheme, typed or not,
//!    as the box itself opens none.
//! 2. **Named by the person**: an address in what they asked, read the
//!    same way, of the same host — or, for a site on the public internet,
//!    the same registrable domain.
//! 3. **Never this machine or its network unless the person named it**:
//!    `localhost`, a name ending `.localhost` or `.local`, an address that
//!    is not the public internet's (`alo-net`'s
//!    [`is_public`](alo_net::resolve::is_public)), and **a public name that
//!    resolves only to such an address** — asked of the machine's resolver,
//!    as every request's is (ADR 0008), with the rebinding rule
//!    ([`Reach::PublicOnly`]). A name that cannot be found is not opened.
//! 4. **Kept to the sites a person named** with `--agent-site`, as every
//!    agent's ask is (ADR 0055).
//! 5. **At most [`Limits::sites`] sites** a run: a site is its host's
//!    registrable domain (`alo_url::site::of`, the public suffix list), or
//!    its host where it has none; a site the run is already on or has been
//!    on is not counted again, and a link followed inside a site opens
//!    none. The page a run began on is one it has been on.
//! 6. **Asked, or not**: a site neither found by the run's search nor
//!    named by the person is asked first — *alo wants to open …* — and,
//!    when the person chose to be asked before any site, every one is.
//!
//! # Its limit
//!
//! The resolver is asked here, and Chrome resolves the name again as it
//! opens the page: a name whose answers change between the two — a public
//! address here, a private one a moment later — is not caught by step 3
//! (ADR 0103 § 2, queue item 881).

use crate::agent_found::FoundSites;
use crate::agent_protocol::Begin;
use crate::agent_sites::AgentSites;
use alo_net::resolve::{Reach, Unresolved, is_public};
use alo_renderer::agent_limits::{Asks, Limits};
use alo_url::{Host, Url};
use std::collections::BTreeSet;
use std::net::IpAddr;

/// How a name is resolved: the machine's resolver, or a test's.
pub type Resolve<'a> = &'a mut dyn FnMut(&str, u16, Reach) -> Result<(), Unresolved>;

/// The sites a run has been on, and how many of them it opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Visited {
    /// Every site it has been on, by registrable domain.
    sites: BTreeSet<String>,
    /// How many of them it opened itself.
    opened: usize,
}

impl Visited {
    /// The run has been on the page at `url` — it began there, or a link
    /// took it there: its site is not counted again, nor counted now.
    pub fn been_on(&mut self, url: &Url) {
        if let Some(site) = site_of(url) {
            self.sites.insert(site);
        }
    }

    /// The run opened the page at `url`: its site counted, if it is new.
    pub fn opened(&mut self, url: &Url) {
        if let Some(site) = site_of(url)
            && self.sites.insert(site)
        {
            self.opened += 1;
        }
    }

    /// How many sites it opened.
    pub fn opened_count(&self) -> usize {
        self.opened
    }

    /// Whether it has been on the site of `url`.
    fn holds(&self, url: &Url) -> bool {
        site_of(url).is_some_and(|site| self.sites.contains(&site))
    }
}

/// The site of `url`: its host's registrable domain, or its host.
fn site_of(url: &Url) -> Option<String> {
    url.host.as_ref().map(alo_url::site::of)
}

/// What the browser process judged of a site the model proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// Opened, with no question.
    Open {
        /// Where.
        url: Url,
        /// What the card's bubble says: a result's title, or the host.
        said: String,
    },
    /// Asked of the person first.
    Ask {
        /// Where.
        url: Url,
        /// What the card's bubble says.
        said: String,
        /// The question, in a person's words.
        what: String,
    },
    /// Not opened: why, which the model is told and the step says.
    Refused(String),
}

/// What is judged against: what the person asked, what the run found, the
/// sites it has been on, the person's limits and the sites agents are kept
/// to.
#[derive(Debug, Clone, Copy)]
pub struct Against<'a> {
    /// What the person asked, in their words.
    pub asked: &'a str,
    /// What the run's searches found.
    pub found: &'a FoundSites,
    /// The sites the run has been on.
    pub visited: &'a Visited,
    /// How far the run may go, read as it started.
    pub limits: Limits,
    /// The sites a person named with `--agent-site`.
    pub agent_sites: &'a AgentSites,
}

/// Judge opening `begin` — a result's number or an address — against
/// `against`, resolving a name with `resolve`. A search is not an opening,
/// and is refused as one.
pub fn judge(begin: &Begin, against: &Against<'_>, resolve: Resolve<'_>) -> Judged {
    let (url, said, from_a_search) = match begin {
        Begin::Search(_) => return Judged::Refused("a search opens nothing".to_owned()),
        Begin::Result(number) => match against.found.numbered(*number) {
            Some(found) => (found.url.clone(), found.title.clone(), true),
            None => {
                return Judged::Refused(format!(
                    "alo found no site numbered {number}: open a site by a number a search \
                     gave, or by its address"
                ));
            }
        },
        Begin::Address(written) => match alo_renderer::typed::address(written) {
            Some(url) => {
                let said = alo_url::reading::host(&url);
                let found = against.found.holds(&url);
                (url, said, found)
            }
            None => {
                return Judged::Refused(format!(
                    "\u{201c}{written}\u{201d} is not a web address alo opens: alo opens only \
                     http and https addresses"
                ));
            }
        },
    };
    let Some(host) = url.host.clone() else {
        return Judged::Refused(format!("{} has no host to open", url.serialised));
    };
    let shown = alo_url::reading::host(&url);
    let named = named_by(against.asked, &url);
    // Never this machine or its network, unless the person named it.
    if !named {
        if on_this_network(&host) {
            return Judged::Refused(local(&shown));
        }
        if let Host::Domain(name) = &host {
            let port = url.effective_port().unwrap_or(443);
            match resolve(name, port, Reach::PublicOnly) {
                Ok(()) => {}
                Err(Unresolved::RefusedTheAnswer { .. }) => {
                    return Judged::Refused(local(&shown));
                }
                Err(Unresolved::NoSuchName { .. }) => {
                    return Judged::Refused(format!(
                        "{shown} could not be found, so alo did not open it"
                    ));
                }
            }
        }
    }
    if !against.agent_sites.allows(&url) {
        return Judged::Refused(format!(
            "alo was started kept to {}, and {shown} is not one of them",
            against.agent_sites
        ));
    }
    let most = usize::from(against.limits.sites);
    if !against.visited.holds(&url) && against.visited.opened_count() >= most {
        return Judged::Refused(format!(
            "alo has opened the {most} {} one task may open (Settings, alo at work), so it did \
             not open {shown}: go on with the pages opened, or finish",
            if most == 1 { "site" } else { "sites" }
        ));
    }
    let unnamed = !named && !from_a_search;
    // Asked before as Settings says (queue item 884): before no site by
    // default, before one neither found nor named, or before every one.
    let asks = match against.limits.asks {
        Asks::Never => false,
        Asks::Unnamed => unnamed,
        Asks::Every => true,
    };
    if asks {
        let what = if unnamed {
            format!("alo wants to open {shown}. It is not a site a search found or one you named.")
        } else {
            format!("alo wants to open {shown}.")
        };
        return Judged::Ask { url, said, what };
    }
    Judged::Open { url, said }
}

/// What the run is told when a site is on this machine or its network.
fn local(shown: &str) -> String {
    format!(
        "{shown} is on this machine or its network, and alo opens such an address only when \
         the person wrote it"
    )
}

/// Whether the person named the site at `url` in `asked`: an address in
/// their words, read as the box reads one, of the same host — or, both on
/// the public internet, of the same registrable domain.
pub fn named_by(asked: &str, url: &Url) -> bool {
    let Some(host) = url.host.as_ref() else {
        return false;
    };
    asked
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|each: char| {
                matches!(
                    each,
                    ',' | ';'
                        | ':'
                        | '!'
                        | '?'
                        | '('
                        | ')'
                        | '"'
                        | '\''
                        | '\u{201c}'
                        | '\u{201d}'
                        | '\u{2018}'
                        | '\u{2019}'
                        | '<'
                        | '>'
                ) || each == '.'
            })
        })
        .filter_map(alo_renderer::typed::address)
        .filter_map(|named| named.host)
        .any(|named| {
            same_host(&named, host)
                || (!on_this_network(&named)
                    && !on_this_network(host)
                    && alo_url::site::of(&named) == alo_url::site::of(host))
        })
}

/// Whether two hosts are one, whatever their case or a final dot.
fn same_host(one: &Host, other: &Host) -> bool {
    match (one, other) {
        (Host::Domain(one), Host::Domain(other)) => one
            .trim_end_matches('.')
            .eq_ignore_ascii_case(other.trim_end_matches('.')),
        _ => one == other,
    }
}

/// Whether `host`, by itself, is this machine or its network's: written
/// as an address that is not the public internet's, or named so.
fn on_this_network(host: &Host) -> bool {
    match host {
        Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost"
                || name
                    .rsplit_once('.')
                    .is_some_and(|(_, last)| matches!(last, "localhost" | "local"))
                || name
                    .parse::<IpAddr>()
                    .is_ok_and(|address| !is_public(address))
        }
        Host::Ipv4(four) => !is_public(IpAddr::V4(*four)),
        Host::Ipv6(six) => !is_public(IpAddr::V6(*six)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::results::{Found, Showing};
    use std::net::Ipv4Addr;

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap()
    }

    fn found(addresses: &[&str]) -> FoundSites {
        let mut found = FoundSites::default();
        let _ = found.searched(
            "museum",
            Showing::Found(
                addresses
                    .iter()
                    .map(|address| Found {
                        title: format!("Title of {address}"),
                        url: url(address),
                        description: String::new(),
                        name: String::new(),
                        mark: None,
                    })
                    .collect(),
            ),
        );
        found
    }

    /// A resolver answering every name with a public address, but those
    /// named in `private`, which answer only a private one.
    fn resolver(
        private: &'static [&'static str],
    ) -> impl FnMut(&str, u16, Reach) -> Result<(), Unresolved> {
        move |name, _, reach| {
            if name == "nowhere.example" {
                return Err(Unresolved::NoSuchName {
                    host: name.to_owned(),
                    why: "no such name".to_owned(),
                });
            }
            if private.contains(&name) && reach == Reach::PublicOnly {
                return Err(Unresolved::RefusedTheAnswer {
                    host: name.to_owned(),
                    address: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
                });
            }
            Ok(())
        }
    }

    fn judged(
        begin: &Begin,
        asked: &str,
        found: &FoundSites,
        visited: &Visited,
        limits: Limits,
    ) -> Judged {
        let mut resolve = resolver(&["intranet.example"]);
        judge(
            begin,
            &Against {
                asked,
                found,
                visited,
                limits,
                agent_sites: &AgentSites::Every,
            },
            &mut resolve,
        )
    }

    #[test]
    fn a_search_result_and_a_site_the_person_named_open_with_no_question() {
        let found = found(&["https://ugandamuseum.go.ug/", "https://visit.example/"]);
        let visited = Visited::default();
        assert_eq!(
            judged(
                &Begin::Result(1),
                "hours of the Uganda Museum",
                &found,
                &visited,
                Limits::default()
            ),
            Judged::Open {
                url: url("https://ugandamuseum.go.ug/"),
                said: "Title of https://ugandamuseum.go.ug/".to_owned(),
            }
        );
        // Named: by its address, or another page of the same registrable
        // domain.
        for asked in [
            "what does www.bbc.co.uk say today?",
            "open https://www.bbc.co.uk/news, and read it",
            "news on bbc.co.uk.",
        ] {
            assert!(
                matches!(
                    judged(
                        &Begin::Address("www.bbc.co.uk/news".to_owned()),
                        asked,
                        &found,
                        &visited,
                        Limits::default()
                    ),
                    Judged::Open { .. }
                ),
                "{asked}"
            );
        }
    }

    #[test]
    fn by_default_any_public_site_opens_with_no_question() {
        // Queue item 884: the owner's decision, *Never* the default — the
        // site neither found nor named that 821's hand run was asked about
        // opens, as a result and a site named do.
        let found = found(&["https://ugandamuseum.go.ug/"]);
        let visited = Visited::default();
        assert_eq!(Limits::default().asks, Asks::Never);
        for (begin, asked) in [
            (
                Begin::Address("en.wikipedia.org/wiki/Uganda_Museum".to_owned()),
                "the Uganda Museum",
            ),
            (Begin::Address("gov.co.uk".to_owned()), "read bbc.co.uk"),
            (Begin::Result(1), "museum hours"),
        ] {
            assert!(
                matches!(
                    judged(&begin, asked, &found, &visited, Limits::default()),
                    Judged::Open { .. }
                ),
                "{begin:?}"
            );
        }
    }

    #[test]
    fn a_site_neither_found_nor_named_is_asked_and_every_site_when_set_so() {
        let found = found(&["https://ugandamuseum.go.ug/"]);
        let visited = Visited::default();
        let unnamed = Limits::checked(3, 20, Asks::Unnamed).unwrap();
        assert_eq!(
            judged(
                &Begin::Address("wikipedia.org".to_owned()),
                "museum hours",
                &found,
                &visited,
                unnamed
            ),
            Judged::Ask {
                url: url("https://wikipedia.org/"),
                said: "wikipedia.org".to_owned(),
                what: "alo wants to open wikipedia.org. It is not a site a search found or one \
                       you named."
                    .to_owned(),
            }
        );
        // Another registrable domain under one public suffix is not named.
        assert!(matches!(
            judged(
                &Begin::Address("gov.co.uk".to_owned()),
                "read bbc.co.uk",
                &found,
                &visited,
                unnamed
            ),
            Judged::Ask { .. }
        ));
        // A result is opened with no question.
        assert!(matches!(
            judged(&Begin::Result(1), "museum hours", &found, &visited, unnamed),
            Judged::Open { .. }
        ));
        let every = Limits::checked(3, 20, Asks::Every).unwrap();
        assert_eq!(
            judged(&Begin::Result(1), "museum hours", &found, &visited, every),
            Judged::Ask {
                url: url("https://ugandamuseum.go.ug/"),
                said: "Title of https://ugandamuseum.go.ug/".to_owned(),
                what: "alo wants to open ugandamuseum.go.ug.".to_owned(),
            }
        );
        // An address the model wrote that a search found is a result.
        assert!(matches!(
            judged(
                &Begin::Address("https://ugandamuseum.go.ug/".to_owned()),
                "hours",
                &found,
                &visited,
                unnamed
            ),
            Judged::Open { .. }
        ));
    }

    #[test]
    fn this_machine_and_its_network_are_refused_unless_the_person_wrote_them() {
        // Under every choice of asking, *Never* the default among them
        // (queue item 884): not a setting.
        for asks in Asks::ALL {
            the_network_refused_asking(Limits::checked(3, 20, asks).unwrap());
        }
    }

    fn the_network_refused_asking(limits: Limits) {
        let found = found(&["http://192.168.1.20/admin", "https://intranet.example/"]);
        let visited = Visited::default();
        let refused = |judged: Judged| matches!(judged, Judged::Refused(why) if why.contains("on this machine or its network"));
        for address in [
            "localhost:8080",
            "127.0.0.1",
            "http://10.0.0.1/",
            "[::1]:3000",
            "printer.local",
            "api.localhost",
            "intranet.example",
        ] {
            assert!(
                refused(judged(
                    &Begin::Address(address.to_owned()),
                    "check the page",
                    &found,
                    &visited,
                    limits
                )),
                "{address}"
            );
        }
        // A search's result on a private address, or a public name that
        // resolves only to one, is refused too.
        assert!(refused(judged(
            &Begin::Result(1),
            "admin",
            &found,
            &visited,
            limits
        )));
        assert!(refused(judged(
            &Begin::Result(2),
            "intranet",
            &found,
            &visited,
            limits
        )));
        // Written by the person, it is not refused: opened, or under *Any
        // site* asked about.
        let opens = |judged: Judged| matches!(judged, Judged::Open { .. } | Judged::Ask { .. });
        assert!(opens(judged(
            &Begin::Address("http://127.0.0.1:8080/page.html".to_owned()),
            "read 127.0.0.1:8080/page.html",
            &found,
            &visited,
            limits
        )));
        // Another port on this machine is the same host, named.
        assert!(opens(judged(
            &Begin::Address("localhost:9000".to_owned()),
            "open localhost:8080",
            &found,
            &visited,
            limits
        )));
        assert!(matches!(
            judged(&Begin::Address("nowhere.example".to_owned()), "x", &found, &visited, limits),
            Judged::Refused(why) if why.contains("could not be found")
        ));
    }

    #[test]
    fn a_file_or_any_other_scheme_is_never_opened() {
        let found = FoundSites::default();
        let visited = Visited::default();
        for address in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "ftp://example.com/",
            "chrome://settings",
            "not an address",
        ] {
            assert!(
                matches!(
                    judged(&Begin::Address(address.to_owned()), &format!("open {address}"), &found, &visited, Limits::default()),
                    Judged::Refused(why) if why.contains("not a web address")
                ),
                "{address}"
            );
        }
        assert!(matches!(
            judged(&Begin::Result(9), "x", &found, &visited, Limits::default()),
            Judged::Refused(why) if why.contains("no site numbered 9")
        ));
    }

    #[test]
    fn at_most_the_sites_set_are_opened_a_run_and_a_site_been_on_is_not_counted() {
        let found = found(&[
            "https://a.example/",
            "https://b.example/",
            "https://c.example/",
            "https://d.example/",
            "https://www.a.example/more",
        ]);
        let mut visited = Visited::default();
        // The page it began on, and one a link took it to, are not counted.
        visited.been_on(&url("https://start.example/"));
        assert_eq!(visited.opened_count(), 0);
        for number in 1..=3 {
            let Judged::Open { url, .. } = judged(
                &Begin::Result(number),
                "x",
                &found,
                &visited,
                Limits::default(),
            ) else {
                panic!("result {number} not opened");
            };
            visited.opened(&url);
        }
        assert_eq!(visited.opened_count(), 3);
        assert!(matches!(
            judged(&Begin::Result(4), "x", &found, &visited, Limits::default()),
            Judged::Refused(why) if why.starts_with("alo has opened the 3 sites one task may open")
        ));
        // Another page of a site opened is not another site.
        assert!(matches!(
            judged(&Begin::Result(5), "x", &found, &visited, Limits::default()),
            Judged::Open { .. }
        ));
        assert!(matches!(
            judged(
                &Begin::Address("start.example/page".to_owned()),
                "start.example",
                &found,
                &visited,
                Limits::default()
            ),
            Judged::Open { .. }
        ));
        // Set to one, the second is refused.
        let one = Limits::checked(1, 20, Asks::Unnamed).unwrap();
        let mut visited = Visited::default();
        visited.opened(&url("https://a.example/"));
        assert!(matches!(
            judged(&Begin::Result(2), "x", &found, &visited, one),
            Judged::Refused(why) if why.starts_with("alo has opened the 1 site one task")
        ));
    }

    #[test]
    fn kept_to_the_sites_named_at_start() {
        let found = found(&["https://a.example/"]);
        let sites = AgentSites::named(["https://b.example/"]).unwrap();
        let mut resolve = resolver(&[]);
        let judged = judge(
            &Begin::Result(1),
            &Against {
                asked: "x",
                found: &found,
                visited: &Visited::default(),
                limits: Limits::default(),
                agent_sites: &sites,
            },
            &mut resolve,
        );
        assert!(
            matches!(&judged, Judged::Refused(why) if why.contains("kept to")),
            "{judged:?}"
        );
    }
}

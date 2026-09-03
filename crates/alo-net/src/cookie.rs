//! One cookie, and reading the header that sets it.
//!
//! ADR 0007 is the decision this implements. The parsing is the small half; the
//! rules that make a cookie *refusable* are the point:
//!
//! - A `__Host-` cookie that does not meet the prefix's conditions is
//!   **rejected**, not stored under a name with the prefix stripped. The whole
//!   value of a prefix is that a server can trust the name, and a browser that
//!   quietly relaxes it has taken that value away without telling anybody.
//! - `SameSite=None` without `Secure` is rejected, because a cross-site cookie
//!   sent in the clear is one any network can read and replay.
//! - A cookie **carries its partition**. There is no constructor that produces
//!   one without, which is how ADR 0007's central promise is kept by the type
//!   system rather than by everybody remembering.

use crate::httpdate;
use alo_url::Url;
use core::fmt;
use std::time::{Duration, SystemTime};

/// The most bytes in one cookie's name and value together.
///
/// A bound, because without one a server chooses how much memory a browser
/// spends per site. Four kilobytes is what every browser settled on.
pub const LARGEST_COOKIE: usize = 4096;

/// The longest a cookie may live, however far ahead its expiry says.
///
/// Four hundred days. A cookie set to expire in the year 9999 is not a
/// preference, it is a permanent identifier, and the cap is what makes
/// "forever" mean something a person could outlive.
pub const LONGEST_LIFE: Duration = Duration::from_secs(400 * 24 * 60 * 60);

/// When a cookie may be sent on a request that came from another site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
    /// Never across a site boundary.
    Strict,
    /// Only when the person navigated here — not on an embedded request.
    ///
    /// The default when a site does not say. The specification's historical
    /// default was `None`; a cookie with no `SameSite` is one whose author did
    /// not think about cross-site use, and the safe reading of "did not think
    /// about it" is not "send it everywhere" (ADR 0007).
    #[default]
    Lax,
    /// Across any boundary. Requires `Secure`.
    None,
}

impl fmt::Display for SameSite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        })
    }
}

/// Which top-level site a cookie belongs to.
///
/// The second half of ADR 0007's key. A cookie `ads.example` sets inside
/// `news.example` carries `Partition("news.example")`; the one it sets inside
/// `shop.example` carries `Partition("shop.example")`; neither can see the
/// other, and nothing can join them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Partition(String);

impl Partition {
    /// The partition a page at this URL creates for everything inside it.
    ///
    /// The **host**, which is stricter than the specification's registrable
    /// domain: it makes `a.example.com` and `b.example.com` separate
    /// partitions where a browser with a public suffix list would treat them as
    /// one. Stricter is the safe direction to be wrong in while queue item 156
    /// is outstanding, and it is written here rather than assumed.
    pub fn of(top_level: &Url) -> Self {
        Self(
            top_level
                .host
                .as_ref()
                .map_or_else(|| "opaque".to_owned(), ToString::to_string),
        )
    }

    /// The site this is.
    pub fn site(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Partition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A cookie, as it is held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// Its name.
    pub name: String,
    /// Its value, exactly as sent.
    pub value: String,
    /// The host it was set by, or the domain it asked for.
    pub domain: String,
    /// Whether it applies to subdomains — true when the server sent `Domain`.
    pub covers_subdomains: bool,
    /// The path prefix it applies to.
    pub path: String,
    /// When it stops existing, or [`None`] for "when this session ends".
    pub expires: Option<SystemTime>,
    /// Only over a secure connection.
    pub secure: bool,
    /// Not readable by script, from the day there is script (ADR 0007).
    pub http_only: bool,
    /// When it may cross a site boundary.
    pub same_site: SameSite,
    /// Which top-level site it belongs to. ADR 0007's central promise, kept by
    /// the type rather than by everybody remembering.
    pub partition: Partition,
}

/// Why a `Set-Cookie` was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// In words, for somebody looking at why a login did not stick.
    pub why: String,
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.why)
    }
}

impl std::error::Error for Rejected {}

impl Cookie {
    /// Read one `Set-Cookie` header.
    ///
    /// `from` is the URL that sent it and `within` is the top-level site the
    /// person was looking at — both are required, because a cookie that does
    /// not know either is a cookie nobody can decide anything about.
    ///
    /// # Errors
    ///
    /// [`Rejected`], in words, for anything this browser will not store.
    pub fn parse(header: &str, from: &Url, within: &Partition) -> Result<Self, Rejected> {
        let refuse = |why: &str| Rejected {
            why: why.to_owned(),
        };
        let host = from
            .host
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| refuse("a cookie from a URL with no host"))?;

        let (pair, attributes) = match header.split_once(';') {
            Some((pair, rest)) => (pair, rest),
            None => (header, ""),
        };
        // A `Set-Cookie` with no `=` at all is a nameless cookie, which is
        // legal in the wild and which this browser refuses: a cookie whose name
        // is empty cannot be told apart from any other nameless one.
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(|| refuse("a cookie with no name"))?;
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() {
            return Err(refuse("a cookie with an empty name"));
        }
        if name.len() + value.len() > LARGEST_COOKIE {
            return Err(refuse("a cookie larger than this browser holds"));
        }
        if name
            .bytes()
            .any(|byte| byte < 0x21 || byte == b';' || byte == b'=')
        {
            return Err(refuse(
                "a cookie name with a character that cannot be sent back",
            ));
        }

        let mut found = Cookie {
            name: name.to_owned(),
            value: value.to_owned(),
            domain: host.clone(),
            covers_subdomains: false,
            path: default_path(&from.path),
            expires: None,
            secure: from.scheme == "https",
            http_only: false,
            same_site: SameSite::default(),
            partition: within.clone(),
        };
        // `Secure` is *asked for*, not inherited. Starting from the scheme
        // above would mark every cookie from an https page secure, which is
        // stricter than the specification and would surprise a server.
        found.secure = false;

        let mut max_age: Option<i64> = None;
        let mut expires_at: Option<SystemTime> = None;
        for attribute in attributes.split(';') {
            let (key, argument) = match attribute.split_once('=') {
                Some((key, argument)) => (key.trim(), argument.trim()),
                None => (attribute.trim(), ""),
            };
            match key.to_ascii_lowercase().as_str() {
                "secure" => found.secure = true,
                "httponly" => found.http_only = true,
                "path" if argument.starts_with('/') => argument.clone_into(&mut found.path),
                "domain" if !argument.is_empty() => {
                    let asked = argument.trim_start_matches('.').to_ascii_lowercase();
                    if !covers(&asked, &host) {
                        return Err(refuse(
                            "a cookie for a domain the page it came from is not part of",
                        ));
                    }
                    // A single label is a public suffix in every arrangement
                    // that matters — `Domain=com` would be a cookie for the
                    // whole internet. The general case wants the public suffix
                    // list, which is queue item 156.
                    if !asked.contains('.') {
                        return Err(refuse("a cookie for a domain that is a public suffix"));
                    }
                    found.domain = asked;
                    found.covers_subdomains = true;
                }
                "max-age" => max_age = argument.parse::<i64>().ok(),
                "expires" => expires_at = httpdate::parse(argument),
                "samesite" => {
                    found.same_site = match argument.to_ascii_lowercase().as_str() {
                        "strict" => SameSite::Strict,
                        "none" => SameSite::None,
                        // Anything else, including a spelling nobody knows, is
                        // the default. A `SameSite` we cannot read is a site
                        // that did not successfully say anything.
                        _ => SameSite::Lax,
                    };
                }
                _ => {}
            }
        }

        // `Max-Age` beats `Expires` where both are given, and a zero or
        // negative one means "gone now", which is how a cookie is deleted.
        found.expires = match max_age {
            Some(seconds) if seconds <= 0 => Some(SystemTime::UNIX_EPOCH),
            Some(seconds) => u64::try_from(seconds)
                .ok()
                .map(|seconds| SystemTime::now() + Duration::from_secs(seconds).min(LONGEST_LIFE)),
            None => expires_at.map(|at| at.min(SystemTime::now() + LONGEST_LIFE)),
        };

        if found.same_site == SameSite::None && !found.secure {
            return Err(refuse(
                "a cookie that asked to cross sites without asking to be secure",
            ));
        }
        check_the_prefix(&found, &host)?;
        Ok(found)
    }

    /// Whether this cookie is gone at this moment.
    pub fn has_expired(&self, now: SystemTime) -> bool {
        self.expires.is_some_and(|at| at <= now)
    }

    /// Whether it lasts only as long as this session.
    pub fn is_for_this_session_only(&self) -> bool {
        self.expires.is_none()
    }

    /// What identifies this cookie: ADR 0007's two-part key, plus the name.
    pub fn key(&self) -> (Partition, String, String, String) {
        (
            self.partition.clone(),
            self.domain.clone(),
            self.path.clone(),
            self.name.clone(),
        )
    }
}

/// The prefixes, enforced rather than parsed.
///
/// A `__Host-` cookie that does not meet the conditions is **rejected**. The
/// whole value of a prefix is that a server reading the name back can trust
/// what it implies; a browser that stored it anyway would have removed that
/// value without telling anybody it had.
fn check_the_prefix(cookie: &Cookie, host: &str) -> Result<(), Rejected> {
    if cookie.name.starts_with("__Host-") {
        if !cookie.secure {
            return Err(Rejected {
                why: "a __Host- cookie that is not Secure".to_owned(),
            });
        }
        if cookie.covers_subdomains {
            return Err(Rejected {
                why: "a __Host- cookie with a Domain, which is what the prefix forbids".to_owned(),
            });
        }
        if cookie.path != "/" {
            return Err(Rejected {
                why: "a __Host- cookie whose Path is not /".to_owned(),
            });
        }
        if cookie.domain != host {
            return Err(Rejected {
                why: "a __Host- cookie for a host other than the one that set it".to_owned(),
            });
        }
    }
    if cookie.name.starts_with("__Secure-") && !cookie.secure {
        return Err(Rejected {
            why: "a __Secure- cookie that is not Secure".to_owned(),
        });
    }
    Ok(())
}

/// Whether `domain` covers `host` — the same host, or a parent of it.
///
/// The dot matters: `evil-example.com` must not be covered by `example.com`,
/// and a comparison by suffix alone says it is.
pub fn covers(domain: &str, host: &str) -> bool {
    if domain == host {
        return true;
    }
    host.strip_suffix(domain)
        .is_some_and(|rest| rest.ends_with('.'))
}

/// The path a cookie applies to when the server did not say.
///
/// Everything up to the last `/`, which is the directory the page was in — not
/// the page itself, and not `/`.
fn default_path(path: &str) -> String {
    if !path.starts_with('/') {
        return "/".to_owned();
    }
    match path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(cut) => path.get(..cut).unwrap_or("/").to_owned(),
    }
}

/// Whether a cookie's path applies to a request's path.
pub fn path_applies(cookie_path: &str, request_path: &str) -> bool {
    if cookie_path == request_path {
        return true;
    }
    let Some(rest) = request_path.strip_prefix(cookie_path) else {
        return false;
    };
    cookie_path.ends_with('/') || rest.starts_with('/')
}

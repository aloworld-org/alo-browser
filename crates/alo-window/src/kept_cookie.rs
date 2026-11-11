/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One cookie a person's sign-in is kept as, and the line it is kept in
//! (ADR 0111 § 2, queue item 935).
//!
//! A cookie is read out of a card's Chromium as its DevTools protocol
//! gives it (`Storage.getCookies`, [`crate::chrome_protocol::cookies`]) and
//! put back into the next card's the same way (`Storage.setCookies`): its
//! name, value, domain, path, when it expires — or that it is a session's,
//! which is never written to a disk — and the attributes Chromium keys and
//! sends it by. Nothing of it is read or changed by alo: a value is the
//! site's, kept as the site set it.
//!
//! # The line
//!
//! [`KeptCookie::line`]: ten fields between tabs, each with `%`, a tab, a
//! line's end and every other control character written as `%` and two
//! hexadecimal digits, so no value can end a field or a line early. Read
//! back by [`KeptCookie::of_line`], which refuses anything else — a line
//! of another length, a field past [`LONGEST_FIELD`], an escape that is
//! not two digits, a word it does not know — as [`None`], never a panic.
//!
//! # Changes
//!
//! [`Changes::between`] is what a card's Chromium did to the cookies it was
//! last seen holding: those it set or changed, and those it removed — a
//! site signing a person out. [`Changes::apply`] does the same to a jar.
//! A cookie is the same cookie by its name, domain and path ([`Named`]),
//! as Chromium's own store keys an unpartitioned cookie.

use core::fmt::Write as _;

/// The longest field a line is read with, in bytes: Chromium refuses a
/// cookie whose name and value together pass 4,096 bytes, and an
/// attribute past 1,024, so a field this long is not a cookie.
pub const LONGEST_FIELD: usize = 8192;

/// How many fields a line holds.
const FIELDS: usize = 10;

/// Which sites a cookie is sent from, as the site said (`SameSite`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SameSite {
    /// `Strict`.
    Strict,
    /// `Lax`.
    Lax,
    /// `None`: from any site, a secure cookie only.
    Any,
}

impl SameSite {
    /// The word Chromium's protocol says it with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Strict => "Strict",
            Self::Lax => "Lax",
            Self::Any => "None",
        }
    }

    /// The one `word` names, as Chromium's protocol says it.
    pub fn of_word(word: &str) -> Option<Self> {
        [Self::Strict, Self::Lax, Self::Any]
            .into_iter()
            .find(|each| each.word() == word)
    }
}

/// Which of a site's cookies Chromium removes first when it holds too many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Priority {
    /// `Low`.
    Low,
    /// `Medium`: what a cookie that says none has.
    #[default]
    Medium,
    /// `High`.
    High,
}

impl Priority {
    /// The word Chromium's protocol says it with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    /// The one `word` names.
    pub fn of_word(word: &str) -> Option<Self> {
        [Self::Low, Self::Medium, Self::High]
            .into_iter()
            .find(|each| each.word() == word)
    }
}

/// Whether the page that set a cookie was secure, as Chromium records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Scheme {
    /// Not recorded.
    #[default]
    Unset,
    /// Set by a page over `http:`.
    NonSecure,
    /// Set by a page over `https:`.
    Secure,
}

impl Scheme {
    /// The word Chromium's protocol says it with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Unset => "Unset",
            Self::NonSecure => "NonSecure",
            Self::Secure => "Secure",
        }
    }

    /// The one `word` names.
    pub fn of_word(word: &str) -> Option<Self> {
        [Self::Unset, Self::NonSecure, Self::Secure]
            .into_iter()
            .find(|each| each.word() == word)
    }
}

/// A cookie, as a card's Chromium held it.
#[derive(Debug, Clone, PartialEq)]
pub struct KeptCookie {
    /// Its name.
    pub name: String,
    /// Its value, as the site set it.
    pub value: String,
    /// Its domain: with a leading `.` it is sent to the domain's every
    /// host, without one to that host alone.
    pub domain: String,
    /// Its path.
    pub path: String,
    /// When it expires, in seconds since 1970 — or [`None`] for a
    /// session's cookie, kept only while alo runs.
    pub expires: Option<f64>,
    /// Whether a page's script may not read it.
    pub http_only: bool,
    /// Whether it is sent over a secure connection alone.
    pub secure: bool,
    /// Which sites it is sent from, if the site said.
    pub same_site: Option<SameSite>,
    /// Which go first when a site holds too many.
    pub priority: Priority,
    /// Whether the page that set it was secure.
    pub source_scheme: Scheme,
    /// The port of the page that set it; `-1` when not recorded.
    pub source_port: i64,
}

/// A cookie's identity: what makes two the same cookie.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Named {
    /// Its name.
    pub name: String,
    /// Its domain.
    pub domain: String,
    /// Its path.
    pub path: String,
}

impl KeptCookie {
    /// Its identity.
    pub fn named(&self) -> Named {
        Named {
            name: self.name.clone(),
            domain: self.domain.clone(),
            path: self.path.clone(),
        }
    }

    /// Whether it is a session's: kept while alo runs, never on a disk.
    pub fn is_a_sessions(&self) -> bool {
        self.expires.is_none()
    }

    /// Whether it has expired by `now`, in seconds since 1970.
    pub fn has_expired(&self, now: f64) -> bool {
        self.expires.is_some_and(|expires| expires <= now)
    }

    /// The host it is for, without a domain cookie's leading `.`.
    pub fn host(&self) -> &str {
        self.domain.strip_prefix('.').unwrap_or(&self.domain)
    }

    /// The line it is kept in.
    pub fn line(&self) -> String {
        let expires = self
            .expires
            .map_or_else(|| "session".to_owned(), |at| at.to_string());
        let flags = match (self.http_only, self.secure) {
            (true, true) => "hs",
            (true, false) => "h",
            (false, true) => "s",
            (false, false) => "-",
        };
        let fields = [
            escaped(&self.name),
            escaped(&self.value),
            escaped(&self.domain),
            escaped(&self.path),
            expires,
            flags.to_owned(),
            self.same_site.map_or("-", SameSite::word).to_owned(),
            self.priority.word().to_owned(),
            self.source_scheme.word().to_owned(),
            self.source_port.to_string(),
        ];
        fields.join("\t")
    }

    /// The cookie `line` keeps, if it is a line [`KeptCookie::line`] wrote.
    pub fn of_line(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('\t').collect();
        let [
            name,
            value,
            domain,
            path,
            expires,
            flags,
            same_site,
            priority,
            scheme,
            port,
        ] = <[&str; FIELDS]>::try_from(fields).ok()?;
        if [name, value, domain, path]
            .iter()
            .any(|field| field.len() > LONGEST_FIELD)
        {
            return None;
        }
        let expires = match expires {
            "session" => None,
            at => Some(at.parse::<f64>().ok().filter(|at| at.is_finite())?),
        };
        let (http_only, secure) = match flags {
            "hs" => (true, true),
            "h" => (true, false),
            "s" => (false, true),
            "-" => (false, false),
            _ => return None,
        };
        let same_site = match same_site {
            "-" => None,
            word => Some(SameSite::of_word(word)?),
        };
        let cookie = Self {
            name: unescaped(name)?,
            value: unescaped(value)?,
            domain: unescaped(domain)?,
            path: unescaped(path)?,
            expires,
            http_only,
            secure,
            same_site,
            priority: Priority::of_word(priority)?,
            source_scheme: Scheme::of_word(scheme)?,
            source_port: port
                .parse()
                .ok()
                .filter(|port| (-1..=65535).contains(port))?,
        };
        // A cookie with no name and no value, or for no host, is not one.
        let something = !(cookie.name.is_empty() && cookie.value.is_empty());
        (something && !cookie.host().is_empty()).then_some(cookie)
    }
}

/// `field` with `%` and every control character written as `%` and two
/// hexadecimal digits.
fn escaped(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    for c in field.chars() {
        if c == '%' || c.is_control() {
            let mut bytes = [0; 4];
            for byte in c.encode_utf8(&mut bytes).bytes() {
                let _ = write!(out, "%{byte:02X}");
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `field` as it was before [`escaped`]: [`None`] when an escape is not
/// two hexadecimal digits, a control character stands unescaped, or what
/// it spells is not UTF-8.
fn unescaped(field: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(field.len());
    let mut rest = field.as_bytes();
    while let Some((&first, after)) = rest.split_first() {
        if first == b'%' {
            let (digits, after) = after.split_at_checked(2)?;
            let digits = core::str::from_utf8(digits).ok()?;
            bytes.push(u8::from_str_radix(digits, 16).ok()?);
            rest = after;
        } else if first.is_ascii_control() {
            return None;
        } else {
            bytes.push(first);
            rest = after;
        }
    }
    String::from_utf8(bytes).ok()
}

/// What a Chromium did to the cookies it was last seen holding.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    /// The cookies it removed.
    pub gone: Vec<Named>,
    /// The cookies it set, or changed: each as it holds it now.
    pub made: Vec<KeptCookie>,
}

impl Changes {
    /// What became of `before` to make `after`.
    pub fn between(before: &[KeptCookie], after: &[KeptCookie]) -> Self {
        let made = after
            .iter()
            .filter(|cookie| !before.contains(cookie))
            .cloned()
            .collect();
        let held: std::collections::HashSet<Named> = after.iter().map(KeptCookie::named).collect();
        let gone = before
            .iter()
            .map(KeptCookie::named)
            .filter(|named| !held.contains(named))
            .collect();
        Self { gone, made }
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.gone.is_empty() && self.made.is_empty()
    }

    /// Make the same changes to `jar`: what is gone taken out, what was
    /// made put in, in place of any it was before.
    pub fn apply(&self, jar: &mut Vec<KeptCookie>) {
        let out: std::collections::HashSet<Named> = self
            .gone
            .iter()
            .cloned()
            .chain(self.made.iter().map(KeptCookie::named))
            .collect();
        jar.retain(|cookie| !out.contains(&cookie.named()));
        jar.extend(self.made.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(name: &str, value: &str) -> KeptCookie {
        KeptCookie {
            name: name.to_owned(),
            value: value.to_owned(),
            domain: "127.0.0.1".to_owned(),
            path: "/".to_owned(),
            expires: Some(1_800_000_000.5),
            http_only: true,
            secure: false,
            same_site: Some(SameSite::Lax),
            priority: Priority::Medium,
            source_scheme: Scheme::NonSecure,
            source_port: 8080,
        }
    }

    #[test]
    fn a_cookie_reads_back_from_its_line_whatever_its_value_holds() {
        let mut odd = cookie("session id", "a\tb\nc%41\u{7f}é;=,\"");
        odd.domain = ".example.com".to_owned();
        odd.expires = None;
        odd.same_site = None;
        odd.http_only = false;
        odd.secure = true;
        odd.priority = Priority::High;
        odd.source_scheme = Scheme::Secure;
        odd.source_port = -1;
        for each in [cookie("first", "1"), odd] {
            let line = each.line();
            assert!(!line.contains('\n'), "{line}");
            assert_eq!(line.split('\t').count(), FIELDS, "{line}");
            assert_eq!(KeptCookie::of_line(&line), Some(each));
        }
    }

    #[test]
    fn a_line_that_is_not_a_cookies_is_refused_never_a_panic() {
        let good = cookie("first", "1").line();
        let refused = [
            String::new(),
            "\t".repeat(FIELDS),
            good.replacen('\t', "", 1),
            format!("{good}\textra"),
            good.replace("1800000000.5", "soon"),
            good.replace("1800000000.5", "inf"),
            good.replace("\th\t", "\tx\t"),
            good.replace("\tLax\t", "\tSometimes\t"),
            good.replace("\tMedium\t", "\tUrgent\t"),
            good.replace("\tNonSecure\t", "\tMaybe\t"),
            good.replace("\t8080", "\t70000"),
            good.replace("first", "fi%4"),
            good.replace("first", "fi%zz"),
            good.replace("first", "fi%FFst"),
            good.replace("first", "fi\u{1}st"),
            good.replace("127.0.0.1", ""),
            good.replace("127.0.0.1", "."),
            good.replace("first", &"x".repeat(LONGEST_FIELD + 1)),
            "first\t\t127.0.0.1\t/\tsession\t-\t-\tLow\tUnset".to_owned(),
        ];
        for line in refused {
            assert_eq!(KeptCookie::of_line(&line), None, "{line:?}");
        }
        // A name alone, or a value alone, is a cookie.
        assert!(KeptCookie::of_line(&cookie("", "1").line()).is_some());
        assert!(KeptCookie::of_line(&cookie("first", "").line()).is_some());
        assert_eq!(KeptCookie::of_line(&cookie("", "").line()), None);
    }

    #[test]
    fn changes_are_what_was_set_changed_and_removed_and_apply_alike() {
        let first = cookie("first", "1");
        let second = cookie("second", "2");
        let again = cookie("first", "again");
        let third = cookie("third", "3");
        let before = vec![first.clone(), second.clone()];
        let after = vec![again.clone(), third.clone()];
        let changes = Changes::between(&before, &after);
        assert_eq!(changes.made, vec![again.clone(), third.clone()]);
        assert_eq!(changes.gone, vec![second.named()]);
        let mut jar = vec![first, second, cookie("kept", "k")];
        changes.apply(&mut jar);
        jar.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(jar, vec![again, cookie("kept", "k"), third]);
        assert!(Changes::between(&jar, &jar).is_empty());
        // The same name on another path is another cookie.
        let mut elsewhere = cookie("first", "1");
        elsewhere.path = "/app".to_owned();
        let changes = Changes::between(&[cookie("first", "1")], &[elsewhere.clone()]);
        assert_eq!(changes.gone, vec![cookie("first", "1").named()]);
        assert_eq!(changes.made, vec![elsewhere]);
    }

    #[test]
    fn a_session_and_an_expired_cookie_are_told_apart() {
        let mut session = cookie("s", "1");
        session.expires = None;
        assert!(session.is_a_sessions());
        assert!(!session.has_expired(f64::MAX));
        let kept = cookie("k", "1");
        assert!(!kept.is_a_sessions());
        assert!(!kept.has_expired(1_700_000_000.0));
        assert!(kept.has_expired(1_800_000_000.5));
        let mut domain = cookie("d", "1");
        domain.domain = ".example.com".to_owned();
        assert_eq!(domain.host(), "example.com");
    }
}

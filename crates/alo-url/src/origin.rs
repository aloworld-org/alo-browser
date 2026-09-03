//! The origin: the value every security decision in a browser is made against.
//!
//! "May this page read that response?" "May this script touch that frame?"
//! "Does this cookie belong here?" All one question — *are these the same
//! origin?* — and a browser that answers it wrongly has no security model at
//! all, however careful everything above it is.
//!
//! # Two kinds, and the second is the one that matters
//!
//! A **tuple origin** is a scheme, a host and a port. Two of them are the same
//! when all three match, and that is the whole rule.
//!
//! An **opaque origin** is what a `data:` URL has, and a sandboxed frame, and
//! anything else with nothing to compare. WHATWG's rule is that an opaque
//! origin is the same origin as **itself and nothing else** — not even another
//! opaque origin made the same way a moment later. Two `data:` URLs with
//! identical bytes are two origins.
//!
//! That is easy to get backwards, and getting it backwards is a same-origin
//! bypass: every `data:` frame on a page would be able to read every other
//! one. So it is a type with an identity in it rather than a convention
//! somebody has to remember, and `PartialEq` does the right thing without
//! being asked.

use crate::parts::{Host, Url, default_port};
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

/// The identity of one opaque origin.
///
/// Minted once, never reused, and compared by that identity alone — the same
/// argument ADR 0003 makes about node identity, for the same reason: a value
/// that could be recreated could be *impersonated*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Opaque(u64);

impl Opaque {
    /// A new opaque origin, the same as no other.
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for Opaque {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Opaque {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // WHATWG serialises every opaque origin as "null". The number is ours,
        // for a person reading a log and wondering which one.
        write!(f, "null")?;
        if f.alternate() {
            write!(f, " #{}", self.0)?;
        }
        Ok(())
    }
}

/// Where a document or a request came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Origin {
    /// A scheme, a host and a port.
    Tuple {
        /// The scheme, lowercased.
        scheme: String,
        /// The host, in its ASCII form.
        host: Host,
        /// The port actually in effect, the scheme's default included.
        ///
        /// Resolved rather than as-written, so that
        /// `https://example.com:443` and `https://example.com` are one origin.
        port: u16,
    },
    /// An origin that is the same as itself and nothing else.
    Opaque(Opaque),
}

impl Origin {
    /// The origin of a URL.
    ///
    /// Opaque for anything without a host and a port to speak of — which is
    /// `data:`, `about:`, `blob:` and every scheme this engine has not been
    /// told about. **Unknown means opaque**, never "probably fine": a scheme
    /// nobody considered should not inherit anybody's privileges.
    ///
    /// `file:` is opaque too, and deliberately. One local file being able to
    /// read every other one is the oldest exfiltration bug there is, and
    /// treating every `file:` document as its own origin is what modern
    /// browsers settled on after finding that out.
    pub fn of(url: &Url) -> Self {
        let Some(host) = url.host.clone() else {
            return Origin::Opaque(Opaque::new());
        };
        if url.scheme == "file" {
            return Origin::Opaque(Opaque::new());
        }
        let Some(port) = url.port.or_else(|| default_port(&url.scheme)) else {
            return Origin::Opaque(Opaque::new());
        };
        Origin::Tuple {
            scheme: url.scheme.clone(),
            host,
            port,
        }
    }

    /// Whether this origin is one nothing else can match.
    pub fn is_opaque(&self) -> bool {
        matches!(self, Origin::Opaque(_))
    }

    /// Whether two origins are the same one.
    ///
    /// The same as `==`, spelled the way the specification asks the question,
    /// so that a caller reading a security check reads what it is checking.
    pub fn is_same_origin(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Display for Origin {
    /// What WHATWG calls serialising an origin: `https://example.com`, with
    /// the port only when it is not the scheme's own, and `null` for opaque.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Origin::Opaque(opaque) => fmt::Display::fmt(opaque, f),
            Origin::Tuple { scheme, host, port } => {
                write!(f, "{scheme}://{host}")?;
                if default_port(scheme) != Some(*port) {
                    write!(f, ":{port}")?;
                }
                Ok(())
            }
        }
    }
}

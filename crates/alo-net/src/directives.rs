//! `Cache-Control`, from both ends.
//!
//! # Why a request's directives and a response's are one type
//!
//! They are not the same set — `only-if-cached` means nothing from a server and
//! `must-revalidate` means nothing from a client — but they are the same
//! *syntax*, and the mistakes are in the syntax. A directive with a quoted
//! argument, a directive repeated, a `max-age` that is not a number, an unknown
//! directive that must be ignored rather than refused: those are the same
//! problems whichever end sent them, and solving them twice is solving them
//! differently.
//!
//! What each end is *allowed* to say is a matter for the code that acts on it,
//! which is [`crate::freshness`].

/// A directive that is either said or not said.
///
/// These are a **set**, not seven independent fields. Writing them as fields
/// invites the bug where a caller reads `no_cache` and means `no_store`; naming
/// them once, here, means every place that asks does so in the same words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// Do not store this at all, in any form, anywhere.
    NoStore,
    /// It may be stored, but not used without asking the server first.
    ///
    /// **Not** "do not cache", which is what the name suggests and what people
    /// reach for when they mean [`Flag::NoStore`].
    NoCache,
    /// For one user. A shared cache must not keep it; this engine is a private
    /// cache, so it may.
    Private,
    /// Explicitly for everyone, which can make cacheable something that would
    /// not otherwise be.
    Public,
    /// Once stale it must not be used — not even when the network is gone.
    MustRevalidate,
    /// It will not change while it is fresh, so do not revalidate it even if
    /// somebody reloads.
    Immutable,
    /// A request that wants a cached answer or none at all — never the network.
    OnlyIfCached,
}

impl Flag {
    /// The bit this flag occupies.
    const fn bit(self) -> u16 {
        1 << (self as u16)
    }

    /// The name a message would use.
    pub const fn name(self) -> &'static str {
        match self {
            Flag::NoStore => "no-store",
            Flag::NoCache => "no-cache",
            Flag::Private => "private",
            Flag::Public => "public",
            Flag::MustRevalidate => "must-revalidate",
            Flag::Immutable => "immutable",
            Flag::OnlyIfCached => "only-if-cached",
        }
    }
}

/// What a `Cache-Control` header asked for.
///
/// Everything here is what was actually said. Nothing is a decision; the
/// decisions are made from these in one place, where they can be read together.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Directives {
    said: u16,
    /// How many seconds it stays fresh.
    pub max_age: Option<u64>,
    /// The same, for shared caches only. Read so it can be *ignored* knowingly.
    pub s_maxage: Option<u64>,
    /// A request asking for something that will stay fresh this much longer.
    pub min_fresh: Option<u64>,
    /// A request willing to take something this far past its expiry. Present
    /// with no number means "any amount", which is why it is nested.
    pub max_stale: Option<Option<u64>>,
}

impl Directives {
    /// Whether a directive was said.
    pub const fn says(&self, flag: Flag) -> bool {
        self.said & flag.bit() != 0
    }

    /// Read every `Cache-Control` a message carried.
    ///
    /// Repeats and commas are one list, the way every other HTTP list header
    /// works. An unknown directive is **ignored**, which the specification
    /// requires and which is also the only forward-compatible thing to do.
    pub fn of<'a>(values: impl Iterator<Item = &'a str>) -> Self {
        let mut found = Directives::default();
        for value in values {
            for directive in split_outside_quotes(value) {
                let (name, argument) = match directive.split_once('=') {
                    Some((name, argument)) => (name.trim(), Some(unquote(argument.trim()))),
                    None => (directive.trim(), None),
                };
                let seconds = || {
                    argument
                        .as_deref()
                        .and_then(|text| text.parse::<u64>().ok())
                };
                let mut set = |flag: Flag| found.said |= flag.bit();
                match name.to_ascii_lowercase().as_str() {
                    "no-store" => set(Flag::NoStore),
                    "no-cache" => set(Flag::NoCache),
                    "private" => set(Flag::Private),
                    "public" => set(Flag::Public),
                    // `proxy-revalidate` says the same thing to a shared cache,
                    // and this engine holds itself to the stricter reading.
                    "must-revalidate" | "proxy-revalidate" => set(Flag::MustRevalidate),
                    "immutable" => set(Flag::Immutable),
                    "only-if-cached" => set(Flag::OnlyIfCached),
                    "max-age" => found.max_age = seconds(),
                    "s-maxage" => found.s_maxage = seconds(),
                    "min-fresh" => found.min_fresh = seconds(),
                    "max-stale" => found.max_stale = Some(seconds()),
                    _ => {}
                }
            }
        }
        found
    }
}

/// Split on commas that are not inside a quoted string.
///
/// `no-cache="Set-Cookie, X-Thing"` is one directive with a comma in it. A
/// split on every comma turns it into two, the second of which is nonsense —
/// and nonsense is ignored, so the failure is silent.
fn split_outside_quotes(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (index, byte) in value.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b',' if !quoted => {
                if let Some(part) = value.get(start..index) {
                    parts.push(part);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    if let Some(rest) = value.get(start..) {
        parts.push(rest);
    }
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect()
}

/// A quoted argument, unquoted. `max-age="60"` is legal and means sixty.
fn unquote(text: &str) -> String {
    let trimmed = text.trim();
    match trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(inside) => inside.replace("\\\"", "\""),
        None => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(value: &str) -> Directives {
        Directives::of(std::iter::once(value))
    }

    #[test]
    fn the_ordinary_ones() {
        let found = read("max-age=600, public");
        assert_eq!(found.max_age, Some(600));
        assert!(found.says(Flag::Public));
        assert!(!found.says(Flag::NoStore));
    }

    /// The two whose names suggest each other's meaning, and which is which
    /// decides whether a response is written to disk at all.
    #[test]
    fn no_cache_and_no_store_are_different_things() {
        let stored_but_checked = read("no-cache");
        assert!(stored_but_checked.says(Flag::NoCache));
        assert!(
            !stored_but_checked.says(Flag::NoStore),
            "no-cache is not no-store"
        );

        let never_kept = read("no-store");
        assert!(never_kept.says(Flag::NoStore));
        assert!(!never_kept.says(Flag::NoCache));
    }

    /// A split on every comma turns this into two directives, the second of
    /// which is nonsense — and nonsense is ignored, so the bug is silent.
    #[test]
    fn a_quoted_argument_may_contain_a_comma() {
        let found = read("no-cache=\"Set-Cookie, X-Thing\", max-age=60");
        assert!(found.says(Flag::NoCache));
        assert_eq!(
            found.max_age,
            Some(60),
            "the comma inside the quotes split the list"
        );
    }

    #[test]
    fn a_quoted_number_is_a_number() {
        assert_eq!(read("max-age=\"60\"").max_age, Some(60));
    }

    /// Required by the specification, and the only forward-compatible choice.
    #[test]
    fn an_unknown_directive_is_ignored_rather_than_refused() {
        let found = read("max-age=60, stale-while-revalidate=30, something-new");
        assert_eq!(found.max_age, Some(60));
    }

    /// A `max-age` that is not a number is not a `max-age`, and must not become
    /// a zero — zero means "always revalidate", which is a decision the server
    /// did not make.
    #[test]
    fn a_max_age_that_is_not_a_number_is_absent_rather_than_zero() {
        assert_eq!(read("max-age=soon").max_age, None);
        assert_eq!(read("max-age=").max_age, None);
        assert_eq!(read("max-age=-5").max_age, None);
    }

    /// Present with no number means any amount of staleness, which is a
    /// different thing from not being present at all.
    #[test]
    fn max_stale_with_no_number_means_any_amount() {
        assert_eq!(read("max-stale").max_stale, Some(None));
        assert_eq!(read("max-stale=60").max_stale, Some(Some(60)));
        assert_eq!(read("max-age=5").max_stale, None);
    }

    #[test]
    fn repeats_and_commas_are_one_list() {
        let split = Directives::of(["max-age=60", "no-cache"].into_iter());
        let together = read("max-age=60, no-cache");
        assert_eq!(split, together);
    }

    #[test]
    fn names_are_case_insensitive() {
        assert!(read("No-Store").says(Flag::NoStore));
        assert_eq!(read("MAX-AGE=30").max_age, Some(30));
    }
}

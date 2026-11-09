/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which search service and which AI service alo asks, as configuration
//! names them (ADR 0106 § 3, queue item 904).
//!
//! Three lines, each optional, in `settings.txt` ([`crate::settings_file`])
//! and in an application's own defaults, `Contents/Resources/services.txt`
//! ([`crate::services_made`]):
//!
//! ```text
//! search brave            # or: search searxng
//! searxng https://search.example.org
//! answers openai
//! ```
//!
//! What a line names is [`Named`]; the person's lines over the
//! application's over the built-in defaults are [`Services`]. Adding a
//! connector is a variant here — its word, its name in *Settings*' chooser,
//! and, if it takes a key, its [`Keyed`] — and its arm where the service is
//! made.
//!
//! # Names
//!
//! A service's own name ([`SearchChoice::name`], [`AiChoice::name`]) is
//! said in one place a person reads: *Settings*' chooser, where a service
//! is chosen by it (ADR 0106 § 5). Everywhere else is *the search service*
//! and *the AI service*.

use crate::carried_keys::Slot;
use alo_url::Url;

/// The word a line choosing the search service begins with.
pub const SEARCH: &str = "search";

/// The word a line giving a `SearXNG`'s address begins with.
pub const SEARXNG: &str = "searxng";

/// The word a line choosing the AI service begins with.
pub const ANSWERS: &str = "answers";

/// The longest address read for a `SearXNG`.
pub const LONGEST_ADDRESS: usize = 512;

/// Which search service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchChoice {
    /// The Brave Search API (ADR 0082 § 0): the default.
    #[default]
    Brave,
    /// A `SearXNG`, at the address its own line gives.
    Searxng,
}

impl SearchChoice {
    /// Each, in the order the chooser draws them.
    pub const ALL: [Self; 2] = [Self::Brave, Self::Searxng];

    /// Its word in a settings file.
    pub fn word(self) -> &'static str {
        match self {
            Self::Brave => "brave",
            Self::Searxng => "searxng",
        }
    }

    /// The service named by `word`, if alo has its connector.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.word() == word)
    }

    /// Its own name, said in *Settings*' chooser alone.
    pub fn name(self) -> &'static str {
        match self {
            Self::Brave => "Brave Search",
            Self::Searxng => "SearXNG",
        }
    }

    /// Its key, if it takes one.
    pub fn keyed(self) -> Option<Keyed> {
        match self {
            Self::Brave => Some(Keyed::BraveSearch),
            Self::Searxng => None,
        }
    }
}

/// Which AI service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiChoice {
    /// `OpenAI`'s Responses API (ADR 0081): the default, and the one.
    #[default]
    OpenAi,
}

impl AiChoice {
    /// Each, in the order the chooser draws them.
    pub const ALL: [Self; 1] = [Self::OpenAi];

    /// Its word in a settings file.
    pub fn word(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
        }
    }

    /// The service named by `word`, if alo has its connector.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.word() == word)
    }

    /// Its own name, said in *Settings*' chooser alone.
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
        }
    }

    /// Its key: every AI service takes one.
    pub fn keyed(self) -> Keyed {
        match self {
            Self::OpenAi => Keyed::OpenAi,
        }
    }
}

/// A connector that takes a key: where the person's own is kept, where an
/// application carries one, and where an older alo looked (ADR 0106 § 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyed {
    /// The Brave Search API's subscription token.
    BraveSearch,
    /// `OpenAI`'s API key.
    OpenAi,
}

impl Keyed {
    /// Each.
    pub const ALL: [Self; 2] = [Self::BraveSearch, Self::OpenAi];

    /// The keychain item's service holding the person's own key, given in
    /// *Settings*.
    pub fn service(self) -> &'static str {
        match self {
            Self::BraveSearch => "alo-search-brave",
            Self::OpenAi => "alo-ai-openai",
        }
    }

    /// The keychain item's service an older alo read, still read after the
    /// carried key so nothing set up before breaks; alo never writes it.
    pub fn older(self) -> &'static str {
        match self {
            Self::BraveSearch => crate::search_key::SERVICE,
            Self::OpenAi => crate::answer_key::SERVICE,
        }
    }

    /// The slot a release carries its key in.
    pub fn slot(self) -> Slot {
        match self {
            Self::BraveSearch => Slot::Search,
            Self::OpenAi => Slot::Answer,
        }
    }

    /// Which service's key it is, in words a person reads: never its name.
    pub fn of(self) -> &'static str {
        match self {
            Self::BraveSearch => "the search service",
            Self::OpenAi => "the AI service",
        }
    }

    /// What its keychain item is labelled, as *Keychain Access* shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::BraveSearch => "alo: your key for the search service",
            Self::OpenAi => "alo: your key for the AI service",
        }
    }
}

/// What a file names, line by line: each [`None`] where it names nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Named {
    /// The search service, if named.
    pub search: Option<SearchChoice>,
    /// A `SearXNG`'s address, if named.
    pub searxng: Option<Url>,
    /// The AI service, if named.
    pub answers: Option<AiChoice>,
}

impl Named {
    /// Read the line `word rest` into this, if it is one of the three:
    /// whether it was.
    ///
    /// # Errors
    ///
    /// Why it does not read: a service alo has no connector for, an address
    /// that is not a site's, or a line named twice.
    pub fn read_line(&mut self, word: &str, rest: &str) -> Result<bool, String> {
        match word {
            SEARCH => {
                let chosen = SearchChoice::of_word(rest).ok_or_else(|| {
                    format!("it names a search service alo does not have: {rest:?}")
                })?;
                if self.search.replace(chosen).is_some() {
                    return Err("it names the search service twice".to_owned());
                }
            }
            SEARXNG => {
                let at = address(rest)?;
                if self.searxng.replace(at).is_some() {
                    return Err("it names the search service's address twice".to_owned());
                }
            }
            ANSWERS => {
                let chosen = AiChoice::of_word(rest)
                    .ok_or_else(|| format!("it names an AI service alo does not have: {rest:?}"))?;
                if self.answers.replace(chosen).is_some() {
                    return Err("it names the AI service twice".to_owned());
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Its lines, each ending in a line's end, in the order they are read:
    /// only those named.
    pub fn lines(&self) -> String {
        let mut lines = Vec::new();
        if let Some(search) = self.search {
            lines.push(format!("{SEARCH} {}\n", search.word()));
        }
        if let Some(at) = &self.searxng {
            lines.push(format!("{SEARXNG} {}\n", at.serialised));
        }
        if let Some(answers) = self.answers {
            lines.push(format!("{ANSWERS} {}\n", answers.word()));
        }
        lines.concat()
    }

    /// What this names, and what `under` names where this names nothing.
    #[must_use]
    pub fn over(self, under: &Self) -> Self {
        Self {
            search: self.search.or(under.search),
            searxng: self.searxng.or_else(|| under.searxng.clone()),
            answers: self.answers.or(under.answers),
        }
    }

    /// What it comes to, with the built-in defaults where nothing is named.
    pub fn services(&self) -> Services {
        Services {
            search: self.search.unwrap_or_default(),
            searxng: self.searxng.clone(),
            answers: self.answers.unwrap_or_default(),
        }
    }
}

/// The services alo asks, every choice made.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Services {
    /// The search service.
    pub search: SearchChoice,
    /// A `SearXNG`'s address, if one was given: asked only when
    /// [`Services::search`] is [`SearchChoice::Searxng`].
    pub searxng: Option<Url>,
    /// The AI service.
    pub answers: AiChoice,
}

/// A `SearXNG`'s address as `written`: an `http` or `https` site's, with no
/// user, password, query or fragment, at most [`LONGEST_ADDRESS`] long.
///
/// # Errors
///
/// Why it is not one, in words that say back at most what was written.
pub fn address(written: &str) -> Result<Url, String> {
    let written = written.trim();
    let not = || {
        let said: String = written.chars().take(40).collect();
        format!("it gives an address that is not a site's: {said:?}")
    };
    if written.is_empty() || written.len() > LONGEST_ADDRESS {
        return Err(not());
    }
    let url = alo_url::parse(written).map_err(|_| not())?;
    let plain = matches!(url.scheme.as_str(), "http" | "https")
        && url.host.is_some()
        && !alo_url::includes_credentials(&url)
        && url.query.is_none()
        && url.fragment.is_none();
    if plain { Ok(url) } else { Err(not()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_reads_once_and_comes_back_as_written() {
        let mut named = Named::default();
        assert_eq!(named.read_line("search", "searxng"), Ok(true));
        assert_eq!(
            named.read_line("searxng", "https://search.example.org/"),
            Ok(true)
        );
        assert_eq!(named.read_line("answers", "openai"), Ok(true));
        assert_eq!(named.read_line("font", "Inter"), Ok(false), "not ours");
        assert_eq!(
            named.lines(),
            "search searxng\nsearxng https://search.example.org/\nanswers openai\n"
        );
        let mut again = Named::default();
        for line in named.lines().lines() {
            let (word, rest) = line.split_once(' ').unwrap();
            assert_eq!(again.read_line(word, rest), Ok(true));
        }
        assert_eq!(again, named);
        assert_eq!(
            named.read_line("search", "brave").err().as_deref(),
            Some("it names the search service twice")
        );
        assert_eq!(
            Named::default().lines(),
            "",
            "nothing named, nothing written"
        );
    }

    #[test]
    fn a_service_alo_has_no_connector_for_or_an_odd_address_does_not_read() {
        let mut named = Named::default();
        assert_eq!(
            named.read_line("search", "google").err().as_deref(),
            Some("it names a search service alo does not have: \"google\"")
        );
        assert_eq!(
            named.read_line("answers", "OpenAI").err().as_deref(),
            Some("it names an AI service alo does not have: \"OpenAI\"")
        );
        for odd in [
            "",
            "search.example.org",
            "ftp://search.example.org/",
            "https://someone:secret@search.example.org/",
            "https://search.example.org/?q=x",
            "https://search.example.org/#top",
            "javascript:alert(1)",
        ] {
            assert!(address(odd).is_err(), "{odd:?}");
        }
        let long = format!("https://{}.example/", "a".repeat(600));
        let said = address(&long).err().unwrap_or_default();
        assert!(said.len() < 100, "said back cut: {said}");
        assert_eq!(
            address(" http://127.0.0.1:8888 ").map(|url| url.serialised),
            Ok("http://127.0.0.1:8888/".to_owned())
        );
        assert_eq!(
            address("https://example.org/searx").map(|url| url.serialised),
            Ok("https://example.org/searx".to_owned()),
            "an instance under a path"
        );
    }

    #[test]
    fn the_persons_lines_go_over_the_applications_and_the_defaults_under_both() {
        let person = Named {
            answers: Some(AiChoice::OpenAi),
            ..Named::default()
        };
        let application = Named {
            search: Some(SearchChoice::Searxng),
            searxng: address("https://search.example.org").ok(),
            answers: None,
        };
        let services = person.over(&application).services();
        assert_eq!(services.search, SearchChoice::Searxng);
        assert_eq!(services.answers, AiChoice::OpenAi);
        assert!(services.searxng.is_some());
        let person = Named {
            search: Some(SearchChoice::Brave),
            ..Named::default()
        };
        assert_eq!(
            person.over(&application).services().search,
            SearchChoice::Brave
        );
        assert_eq!(
            Named::default().services(),
            Services {
                search: SearchChoice::Brave,
                searxng: None,
                answers: AiChoice::OpenAi,
            },
            "Brave and OpenAI, as every build before"
        );
    }

    #[test]
    fn every_key_has_an_item_of_its_own_an_older_one_and_a_slot() {
        assert_eq!(Keyed::BraveSearch.service(), "alo-search-brave");
        assert_eq!(Keyed::BraveSearch.older(), "alo-brave-search");
        assert_eq!(Keyed::OpenAi.service(), "alo-ai-openai");
        assert_eq!(Keyed::OpenAi.older(), "alo-openai");
        assert_eq!(Keyed::BraveSearch.slot().name(), "search");
        assert_eq!(Keyed::OpenAi.slot().name(), "answer");
        assert_eq!(SearchChoice::Searxng.keyed(), None);
        for keyed in Keyed::ALL {
            for said in [keyed.of(), keyed.label()] {
                let said = said.to_lowercase();
                assert!(
                    !said.contains("brave") && !said.contains("openai"),
                    "{said}"
                );
            }
        }
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The search service and the AI service made from what configuration
//! names and the keys there are (ADR 0106 §§ 3–4, queue items 904 and
//! 906): the one place a connector is chosen.
//!
//! # Which
//!
//! The person's lines in `settings.txt` ([`crate::settings_file`]), over
//! the application's own defaults — [`DEFAULTS`] in its
//! `Contents/Resources`, put there by `alo-bundle --services FILE` as a
//! release is made ([`defaults_in`]) — over the built-in Brave and
//! `OpenAI` ([`crate::services::Named::services`]). A defaults file that
//! does not read is said, and the built-in defaults are used under the
//! person's lines.
//!
//! # With whose key
//!
//! [`crate::service_keys::Keys`]: the person's own, the application's, an
//! older alo's. A service with none is not set up, and says why.

use crate::ai_service::AiService;
use crate::bundle::Bundle;
use crate::search_service::SearchService;
use crate::service_keys::Keys;
use crate::services::{AiChoice, Keyed, Named, SearchChoice, Services};
use std::io::Read as _;
use std::path::Path;

/// The application's own defaults, in its `Contents/Resources`.
pub const DEFAULTS: &str = "services.txt";

/// The most bytes a defaults file is read to.
pub const LARGEST: u64 = 4096;

/// What a defaults file says first, as `alo-bundle` writes it.
pub const HEADER: &str = "# alo: the services this application asks unless a person chooses others";

/// The services named by `text`, a defaults file's: the three lines of
/// [`crate::services`], and nothing else.
///
/// # Errors
///
/// Why it does not read, as a settings file's line would not.
pub fn read_defaults(text: &str) -> Result<Named, String> {
    let mut named = Named::default();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (word, rest) = line
            .split_once(' ')
            .map_or((line, ""), |(word, rest)| (word, rest.trim()));
        if !named.read_line(word, rest)? {
            let said: String = line.chars().take(40).collect();
            return Err(format!("it holds a line alo does not read: {said:?}"));
        }
    }
    if named.search == Some(SearchChoice::Searxng) && named.searxng.is_none() {
        return Err("it chooses a search service that needs an address, and gives none".to_owned());
    }
    Ok(named)
}

/// What `alo-bundle` writes for `named`.
pub fn text_of(named: &Named) -> String {
    format!("{HEADER}\n{}", named.lines())
}

/// The defaults the application whose `Contents/Resources` is `resources`
/// carries: none when it carries no file.
///
/// # Errors
///
/// Why its file does not read, naming it.
pub fn defaults_in(resources: &Path) -> Result<Named, String> {
    let file = resources.join(DEFAULTS);
    let unread = |why: String| {
        format!("the services this application carries ({DEFAULTS}) do not read: {why}")
    };
    let opened = match std::fs::File::open(&file) {
        Ok(opened) => opened,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Named::default()),
        Err(why) => return Err(unread(why.to_string())),
    };
    let mut bytes = Vec::new();
    opened
        .take(LARGEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| unread(why.to_string()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LARGEST {
        return Err(unread(format!("it is larger than {LARGEST} bytes")));
    }
    let text = String::from_utf8(bytes).map_err(|_| unread("it is not text".to_owned()))?;
    read_defaults(&text).map_err(unread)
}

/// The services alo asks, for the person's `named` lines, inside `bundle`
/// if alo runs inside one: and what is to be said, if its defaults did not
/// read.
pub fn chosen(named: &Named, bundle: Option<&Bundle>) -> (Services, Option<String>) {
    let (defaults, said) = match bundle.map(|bundle| defaults_in(bundle.resources())) {
        Some(Ok(defaults)) => (defaults, None),
        Some(Err(why)) => (Named::default(), Some(why)),
        None => (Named::default(), None),
    };
    (named.clone().over(&defaults).services(), said)
}

/// Every key `services` are asked with, in the order they are taken.
pub fn keyed(services: &Services) -> Vec<Keyed> {
    services
        .search
        .keyed()
        .into_iter()
        .chain(core::iter::once(services.answers.keyed()))
        .collect()
}

/// The search service `services` name, asked with the key `keys` gives.
///
/// # Errors
///
/// Why it is not set up, in words a person reads.
pub fn search(services: &Services, keys: &Keys) -> Result<SearchService, String> {
    match services.search {
        SearchChoice::Brave => {
            let key = keys.key(Keyed::BraveSearch)?;
            crate::brave_search::Provider::brave(key).map(SearchService::from)
        }
        SearchChoice::Searxng => {
            let at = services
                .searxng
                .as_ref()
                .ok_or_else(|| "the search service chosen in Settings has no address".to_owned())?;
            crate::searxng_search::Provider::at(at).map(SearchService::from)
        }
    }
}

/// The AI service `services` name, asked with the key `keys` gives.
///
/// # Errors
///
/// Why it is not set up, in words a person reads.
pub fn ai(services: &Services, keys: &Keys) -> Result<AiService, String> {
    match services.answers {
        AiChoice::OpenAi => {
            let key = keys.key(Keyed::OpenAi)?;
            crate::openai_answer::Provider::openai(key).map(AiService::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keychain::Keychain;
    use crate::search_service::Searched;

    #[test]
    fn a_defaults_file_reads_the_three_lines_and_nothing_else() {
        let named = read_defaults(&format!(
            "{HEADER}\n\nsearch searxng\nsearxng https://search.example.org\n"
        ))
        .expect("read");
        assert_eq!(named.search, Some(SearchChoice::Searxng));
        assert_eq!(read_defaults(&text_of(&named)), Ok(named));
        assert_eq!(
            read_defaults("font Inter\n").err().as_deref(),
            Some("it holds a line alo does not read: \"font Inter\"")
        );
        assert_eq!(
            read_defaults("search searxng\n").err().as_deref(),
            Some("it chooses a search service that needs an address, and gives none")
        );
        assert_eq!(
            read_defaults("search\n").err().as_deref(),
            Some("it names a search service alo does not have: \"\"")
        );
    }

    #[test]
    fn an_application_carrying_no_defaults_is_brave_and_openai_and_one_that_does_not_read_is_said()
    {
        let resources = std::env::temp_dir().join(format!(
            "alo-services-made-{}-resources",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&resources);
        std::fs::create_dir_all(&resources).expect("made");
        assert_eq!(defaults_in(&resources), Ok(Named::default()));
        std::fs::write(
            resources.join(DEFAULTS),
            "search searxng\nsearxng http://127.0.0.1:8888\n",
        )
        .expect("written");
        assert_eq!(
            defaults_in(&resources).map(|named| named.search),
            Ok(Some(SearchChoice::Searxng))
        );
        std::fs::write(resources.join(DEFAULTS), "search google\n").expect("written");
        assert_eq!(
            defaults_in(&resources).err().as_deref(),
            Some(
                "the services this application carries (services.txt) do not read: it names a \
                 search service alo does not have: \"google\""
            )
        );
        let _ = std::fs::remove_dir_all(&resources);
    }

    #[test]
    fn each_choice_makes_its_connector_and_a_searxng_needs_no_key_but_an_address() {
        // No keychain is asked: an account no tool will say, and no
        // application.
        let keys = Keys::in_keychain(Keychain::Theirs, "", None);
        let searxng = Services {
            search: SearchChoice::Searxng,
            searxng: crate::services::address("https://search.example.org").ok(),
            answers: AiChoice::OpenAi,
        };
        let made = search(&searxng, &keys).expect("no key needed");
        assert!(made.searches(Searched::News) && !made.searches(Searched::Images));
        assert_eq!(
            search(
                &Services {
                    searxng: None,
                    ..searxng.clone()
                },
                &keys
            )
            .err()
            .as_deref(),
            Some("the search service chosen in Settings has no address")
        );
        assert_eq!(
            search(&Services::default(), &keys).err().as_deref(),
            Some("add a key for the search service in Settings")
        );
        assert_eq!(
            ai(&Services::default(), &keys).err().as_deref(),
            Some("add a key for the AI service in Settings")
        );
        assert_eq!(
            keyed(&Services::default()),
            vec![Keyed::BraveSearch, Keyed::OpenAi]
        );
        assert_eq!(keyed(&searxng), vec![Keyed::OpenAi], "SearXNG takes none");
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-bundle --keys-from` puts the keys a release searches and answers
//! with inside the application, sealed, and the browser in it searches and
//! asks for answers with the ones it carries (queue items 748 and 773, ADR
//! 0091).
//!
//! The real command, given the programs this test was built with and a
//! directory holding two fake keys — never the keychain, so no real key is
//! ever in a test — makes `alo browser.app` with no Chromium. The keys are
//! at `Contents/Resources/keys/search` and `…/keys/answer`, neither plain
//! there nor in any program the application holds; `codesign` accepts the
//! seal, and refuses it once a key's file is changed. The browser's own
//! reading of the application it is in finds each key carried, and a search
//! request, and a request for an answer, made with them carry each in its
//! one header. A directory missing either key refuses, saying which, and
//! nothing is made. No request is sent anywhere.

use alo_net::cause::Identities;
use alo_window::answer_key;
use alo_window::brave_search::Provider;
use alo_window::bundle::Bundle;
use alo_window::openai_answer;
use alo_window::search_key::{Held, for_this_application};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A key no provider has ever issued.
const NOT_A_KEY: &str = "not-a-real-key-for-the-bundle-test-89";

/// A second, for the answer's slot (queue item 773).
const NOT_AN_ANSWER_KEY: &str = "not-a-real-answer-key-for-the-bundle-test-73";

/// A directory of this test's own, empty.
fn directory(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-keys-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    Ok(root)
}

/// `alo-bundle`, making the application into `into` from the programs this
/// test was built with, its keys from `keys`, and no CEF distribution.
fn assemble(into: &Path, keys: &Path) -> Result<Output, String> {
    let alo = Path::new(env!("CARGO_BIN_EXE_alo"));
    let programs = alo.parent().ok_or("alo is in no directory")?;
    Command::new(env!("CARGO_BIN_EXE_alo-bundle"))
        .arg("--programs")
        .arg(programs)
        .arg("--into")
        .arg(into)
        .arg("--keys-from")
        .arg(keys)
        .env_remove("CEF_PATH")
        .output()
        .map_err(|why| format!("alo-bundle did not run: {why}"))
}

/// Whether `bytes` hold `needle` anywhere.
fn holds(bytes: &[u8], needle: &str) -> bool {
    bytes
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// Whether `codesign` accepts `application`'s seal.
fn sealed(application: &Path) -> Result<bool, String> {
    Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(application)
        .output()
        .map(|ran| ran.status.success())
        .map_err(|why| format!("codesign did not run: {why}"))
}

#[test]
fn the_application_carries_its_keys_sealed_and_asks_with_them() -> Result<(), String> {
    let root = directory("made")?;
    let keys = root.join("keys given");
    std::fs::create_dir_all(&keys).map_err(|why| why.to_string())?;
    std::fs::write(keys.join("search"), format!("{NOT_A_KEY}\n")).map_err(|why| why.to_string())?;
    std::fs::write(keys.join("answer"), format!("{NOT_AN_ANSWER_KEY}\n"))
        .map_err(|why| why.to_string())?;
    let into = root.join("into");
    std::fs::create_dir_all(&into).map_err(|why| why.to_string())?;

    let made = assemble(&into, &keys)?;
    let said = String::from_utf8_lossy(&made.stderr).into_owned();
    assert!(made.status.success(), "{said}");
    assert_eq!(
        said,
        format!(
            "alo-bundle: CEF_PATH is not set, so the application carries no Chromium: it draws \
             its page with this browser's own engine, and says so\n\
             alo-bundle: carries the search key, the OpenAI key, taken from {}\n\
             alo-bundle: made, sealed ad hoc and checked by codesign\n",
            keys.display()
        )
    );
    for key in [NOT_A_KEY, NOT_AN_ANSWER_KEY] {
        assert!(!said.contains(key));
        assert!(!String::from_utf8_lossy(&made.stdout).contains(key));
    }

    let application = into.join("alo browser.app");
    let contents = application.join("Contents");
    let carried = contents.join("Resources/keys/search");
    let file = std::fs::read(&carried).map_err(|why| format!("the key's file: {why}"))?;
    assert!(!holds(&file, NOT_A_KEY), "the key is plain in its file");
    let answer_file = std::fs::read(contents.join("Resources/keys/answer"))
        .map_err(|why| format!("the answer key's file: {why}"))?;
    assert!(
        !holds(&answer_file, NOT_AN_ANSWER_KEY),
        "the answer key is plain in its file"
    );
    for program in ["alo", "alo-render", "alo-ask"] {
        let bytes = std::fs::read(contents.join("MacOS").join(program))
            .map_err(|why| format!("{program}: {why}"))?;
        assert!(!holds(&bytes, NOT_A_KEY), "{program} holds the key");
        assert!(
            !holds(&bytes, NOT_AN_ANSWER_KEY),
            "{program} holds the answer key"
        );
    }
    assert!(sealed(&application)?, "the seal");

    // The browser inside it: the key it carries, in the one header.
    let bundle = Bundle::around(&contents.join("MacOS").join("alo"))
        .ok_or("alo does not know it is inside the application")?;
    let key = for_this_application(Some(&bundle))?;
    assert_eq!(key.held(), Held::Carried);
    assert!(!format!("{key:?}").contains(NOT_A_KEY));
    let request = Provider::brave(key)?.request("words", &mut Identities::default())?;
    let tokens: Vec<&str> = request
        .headers
        .iter()
        .filter(|header| header.name == "X-Subscription-Token")
        .map(|header| header.value.as_str())
        .collect();
    assert_eq!(tokens, [NOT_A_KEY]);

    // And the answer's key, in its own slot, in its one header (773).
    let answering = answer_key::for_this_application(Some(&bundle))?;
    assert_eq!(answering.held(), Held::Carried);
    assert!(!format!("{answering:?}").contains(NOT_AN_ANSWER_KEY));
    let request = openai_answer::Provider::openai(answering)?.request(
        "words",
        &[],
        &mut Identities::default(),
    );
    let bearers: Vec<&str> = request
        .headers
        .iter()
        .filter(|header| header.name == "Authorization")
        .map(|header| header.value.as_str())
        .collect();
    assert_eq!(bearers, [format!("Bearer {NOT_AN_ANSWER_KEY}").as_str()]);
    assert!(!holds(&request.body, NOT_AN_ANSWER_KEY));

    // The key changed after the seal: known to be.
    std::fs::write(&carried, b"alo carried key 1\nchanged").map_err(|why| why.to_string())?;
    assert!(!sealed(&application)?, "a changed key was accepted");
    assert_eq!(
        for_this_application(Some(&bundle)).err().as_deref(),
        Some("the search key this application carries could not be read (keys/search)"),
        "a damaged key is said, and never swapped for the keychain's"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn a_key_not_given_is_refused_and_nothing_is_made() -> Result<(), String> {
    let root = directory("refused")?;
    let keys = root.join("no keys");
    std::fs::create_dir_all(&keys).map_err(|why| why.to_string())?;
    let into = root.join("into");
    std::fs::create_dir_all(&into).map_err(|why| why.to_string())?;

    let made = assemble(&into, &keys)?;
    assert_eq!(made.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&made.stderr),
        format!(
            "alo-bundle: CEF_PATH is not set, so the application carries no Chromium: it draws \
             its page with this browser's own engine, and says so\n\
             alo-bundle: no search key at {} for the application to carry\n",
            keys.join("search").display()
        )
    );

    // The search key alone: the answer's is missing, and said.
    std::fs::write(keys.join("search"), NOT_A_KEY).map_err(|why| why.to_string())?;
    let made = assemble(&into, &keys)?;
    assert_eq!(made.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&made.stderr),
        format!(
            "alo-bundle: CEF_PATH is not set, so the application carries no Chromium: it draws \
             its page with this browser's own engine, and says so\n\
             alo-bundle: no OpenAI key at {} for the application to carry\n",
            keys.join("answer").display()
        )
    );
    let left: Vec<_> = std::fs::read_dir(&into)
        .map_err(|why| why.to_string())?
        .collect();
    assert!(left.is_empty(), "something was made: {left:?}");
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

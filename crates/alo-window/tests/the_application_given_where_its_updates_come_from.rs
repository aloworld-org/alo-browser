/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0116 §§ 3 and 6 (queue item 721): `alo-bundle --updates DIRECTORY`
//! puts where the application's updates come from — an address and two
//! public keys — in `Contents/Resources/update` before the seal, so the
//! seal covers them; and **refuses either of the worthless test keys**,
//! saying so and making nothing, so no application it makes trusts a key
//! anybody can sign with.
//!
//! The built `alo-bundle`, run as a release is made, into folders of this
//! test's own, with no CEF distribution and no keys from any keychain. The
//! address is an example's; the keys two strangers', made with the test
//! keys and built into nothing else. Even given them, a build of this
//! repository — version `0.0.0` — does not update itself.

use alo_update::given::{self, Updating};
use alo_update::{Version, test_key};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Two strangers' public keys: the one in use, then the spare.
const KEYS: &str = "RWSnhCT54ZPK6PIMYN2tCd7c4c3o0XBhFzBi8pI/aNszPoU7TSXh7QIr\n\
                    RWQIu18dDhmlGBp3BBqO/pi/FVDRaAuNrqv121/Ga9EiKO1JOzyyXcuh\n";

/// A directory of this test's own, empty.
fn directory(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-updates-made-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    Ok(root)
}

/// `alo-bundle`, making the application into `into` from the programs this
/// test was built with, its updates from `updates`.
fn assemble(into: &Path, updates: &Path) -> Result<Output, String> {
    let alo = Path::new(env!("CARGO_BIN_EXE_alo"));
    let programs = alo.parent().ok_or("alo is in no directory")?;
    Command::new(env!("CARGO_BIN_EXE_alo-bundle"))
        .arg("--programs")
        .arg(programs)
        .arg("--into")
        .arg(into)
        .arg("--updates")
        .arg(updates)
        .env_remove("CEF_PATH")
        .output()
        .map_err(|why| format!("alo-bundle did not run: {why}"))
}

/// `DIRECTORY/address` and `DIRECTORY/keys`, as `--updates` names them.
fn updates_in(root: &Path, keys: &str) -> Result<PathBuf, String> {
    let named = root.join("updates");
    std::fs::create_dir_all(&named).map_err(|why| why.to_string())?;
    std::fs::write(named.join("address"), "https://updates.example.org/alo\n")
        .map_err(|why| why.to_string())?;
    std::fs::write(named.join("keys"), keys).map_err(|why| why.to_string())?;
    Ok(named)
}

#[test]
fn the_address_and_keys_are_carried_sealed_and_read_back() -> Result<(), String> {
    let root = directory("given")?;
    let into = root.join("into");
    std::fs::create_dir_all(&into).map_err(|why| why.to_string())?;
    let made = assemble(&into, &updates_in(&root, KEYS)?)?;
    let said = String::from_utf8_lossy(&made.stderr).into_owned();
    eprintln!("{said}");
    assert!(made.status.success(), "{said}");
    assert!(
        said.contains("alo-bundle: updates itself from the address and the two keys in"),
        "{said}"
    );
    let application = into.join("alo browser.app");
    let resources = application.join("Contents").join("Resources");
    assert_eq!(
        std::fs::read_to_string(resources.join("update").join("address"))
            .ok()
            .as_deref(),
        Some("https://updates.example.org/alo\n")
    );
    assert_eq!(
        std::fs::read_to_string(resources.join("update").join("keys"))
            .ok()
            .as_deref(),
        Some(KEYS)
    );
    let sealed = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&application)
        .output()
        .map(|ran| ran.status.success())
        .map_err(|why| why.to_string())?;
    assert!(sealed, "the seal covers what was put in");
    // A release of its own version would update itself from there; this
    // repository's build, `0.0.0`, never does.
    let root_of_system = root.join("system");
    std::fs::create_dir_all(&root_of_system).map_err(|why| why.to_string())?;
    let released = given::read(
        Some(&resources),
        Version::of(0, 4, 2),
        Some("macos-arm64"),
        &root_of_system,
    );
    assert!(
        matches!(&released, Updating::Itself { address, .. } if address.as_str() == "https://updates.example.org/alo"),
        "{released:?}"
    );
    assert_eq!(
        given::read(
            Some(&resources),
            Version::UNRELEASED,
            Some("macos-arm64"),
            &root_of_system
        ),
        Updating::NotItself { why: None }
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn the_worthless_test_keys_are_refused_and_nothing_is_made() -> Result<(), String> {
    for (name, keys) in [
        (
            "in-use",
            format!(
                "{}\n{}",
                test_key::PUBLIC,
                KEYS.lines().nth(1).unwrap_or_default()
            ),
        ),
        (
            "spare",
            format!(
                "{}\n{}",
                KEYS.lines().next().unwrap_or_default(),
                test_key::SPARE
            ),
        ),
    ] {
        let root = directory(name)?;
        let into = root.join("into");
        std::fs::create_dir_all(&into).map_err(|why| why.to_string())?;
        let made = assemble(&into, &updates_in(&root, &keys)?)?;
        let said = String::from_utf8_lossy(&made.stderr).into_owned();
        eprintln!("{said}");
        assert_eq!(made.status.code(), Some(2), "{said}");
        assert!(
            said.contains("it was given the worthless test key, which anybody can sign with"),
            "{said}"
        );
        assert!(!into.join("alo browser.app").exists(), "nothing made");
        let _ = std::fs::remove_dir_all(&root);
    }
    Ok(())
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the macOS application refuses that a developer's hand run may name
//! (queue item 973, ADR 0111 § 6).
//!
//! `alo --conversation --sign-ins DIR` keeps a hand run's sign-ins in `DIR`
//! and their key in a keychain file there, unlocked by a password written
//! in alo's own source ([`crate::sign_in_key::scratch_keychain`]): not a
//! secret, which is fine for a developer at a terminal and wrong for
//! anything a person is given. So inside `alo browser.app` the option is
//! **refused before anything starts** — no directory made, no keychain
//! made or unlocked, no window — and the refusal says what to name
//! instead: `--data DIR`, which keeps no secret at all (ADR 0112 § 5), or
//! nothing, which keeps the sign-ins under a key of their own in the
//! person's own keychain (ADR 0111 § 3).
//!
//! It is decided where the program is ([`Bundle::around`]), as everything
//! else the application does differently is: a cargo feature would leave
//! the shipped configuration the one the gate, run with every feature on,
//! never builds.

use crate::bundle::Bundle;
use crate::opening::Opening;

/// Why the application inside `bundle` refuses what `opening` asks, or
/// [`None`] when it does not — and always outside an application, where a
/// developer's hand run is what `--sign-ins` is for.
pub fn refused(bundle: Option<&Bundle>, opening: &Opening) -> Option<String> {
    if bundle.is_none() || opening.sign_ins.is_none() {
        return None;
    }
    Some(
        "--sign-ins is for a developer's hand run, and alo browser refuses it: it keeps the \
         sign-ins' key in a keychain file whose password is written in alo's source. Use \
         --data DIR to keep this alo in a directory of its own, which keeps no secrets (its \
         sign-ins last until it quits), or name neither, to keep them sealed under a key in \
         your own keychain."
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::PAGE;
    use std::path::{Path, PathBuf};

    /// An application of this test's own, carrying its page: its `alo`.
    fn shaped(name: &str) -> Option<PathBuf> {
        let root =
            std::env::temp_dir().join(format!("alo-bundle-refused-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let contents = root.join("alo browser.app").join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).ok()?;
        std::fs::create_dir_all(contents.join("Resources")).ok()?;
        std::fs::write(contents.join("Resources").join(PAGE), "<p>here").ok()?;
        Some(contents.join("MacOS").join("alo"))
    }

    /// Remove what [`shaped`] made around `alo`.
    fn unshape(alo: &Path) {
        if let Some(root) = alo.ancestors().nth(4) {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    fn opening(words: &[&str]) -> Opening {
        match Opening::from_arguments(words.iter().map(|word| (*word).to_owned())) {
            Ok(opening) => opening,
            Err(why) => panic!("{words:?} are arguments: {why}"),
        }
    }

    #[test]
    fn the_application_refuses_a_hand_runs_sign_ins_and_says_what_to_use() {
        let Some(alo) = shaped("sign-ins") else {
            panic!("no directory");
        };
        let bundle = Bundle::around(&alo);
        assert!(bundle.is_some(), "shaped as the application is");
        let Some(why) = refused(
            bundle.as_ref(),
            &opening(&["--conversation", "--sign-ins", "/tmp/hand run/sign-ins"]),
        ) else {
            panic!("--sign-ins inside the application is refused");
        };
        assert!(why.starts_with("--sign-ins is for a developer's hand run"));
        assert!(why.contains("--data DIR"), "names what to use: {why}");
        assert!(why.contains("your own keychain"), "{why}");
        // Everything else the application is given is not this refusal's.
        for words in [
            &[][..],
            &["--conversation"][..],
            &["--conversation", "--data", "/tmp/a"][..],
            &["--conversation", "--chats", "/tmp/c"][..],
            &["--chrome", "https://example.com/"][..],
        ] {
            assert_eq!(refused(bundle.as_ref(), &opening(words)), None, "{words:?}");
        }
        unshape(&alo);
    }

    #[test]
    fn outside_an_application_a_hand_run_keeps_its_sign_ins_where_it_named() {
        let asked = opening(&["--conversation", "--sign-ins", "/tmp/hand run/sign-ins"]);
        assert_eq!(refused(None, &asked), None);
    }
}

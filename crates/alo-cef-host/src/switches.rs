/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the host reads from the command line `alo-window` starts it with,
//! and what it adds before Chromium reads it.
//!
//! `alo-window` starts the host exactly as it starts Chrome (`chrome.rs`):
//! `--remote-debugging-pipe`, `--user-data-dir=PROFILE`, ADR 0073's
//! switches, and `--headless about:blank` or `--app=data:,`. Chromium reads
//! all of them itself; the host reads only three things, and none of them is
//! a decision Chromium would make differently.
//!
//! # Whose process this is
//!
//! The same program is the browser process and every helper — CEF starts
//! each helper with `--type=` (renderer, GPU, utility …), which a browser
//! process is never given ([`Role`]).
//!
//! # Where everything is written: the profile, and nowhere else
//!
//! CEF takes its profile as `root_cache_path` and `cache_path`, not as
//! `--user-data-dir`, and with neither it writes to
//! `~/Library/Application Support/CEF/User Data` (the probe saw it, ADR 0079
//! § 8a). So the host takes the profile alo gives it as both, and **refuses
//! to start without one**, or with one that is not an absolute path, rather
//! than write anywhere else ([`profile`]).
//!
//! Three more things were found written outside it, measured on this Mac
//! with CEF 156.0.14 (queue item 725), and each is moved or stopped by what
//! [`again`] adds:
//!
//! - **Chromium's temporary directories** — the process singleton's socket
//!   and its fetcher's scratch, `$TMPDIR/<bundle id>.<random>` — which a host
//!   closed over the pipe leaves behind, because `Browser.close` ends the
//!   process inside `AppKit` (`-[NSApplication terminate:]`, which exits)
//!   before Chromium's own clean-up runs. On macOS Chromium takes its
//!   temporary directory from `MAC_CHROMIUM_TMPDIR` before the system's
//!   (`base/files/file_util_apple.mm`), so the host sets it to
//!   [`temporary`], inside the profile, which alo removes with it.
//! - **A key in the person's login keychain**, `Chromium Safe Storage`,
//!   which encrypts the profile's cookies — written on the first start, and
//!   asked for on every later start of a differently signed build with a
//!   dialog that holds the network until a person answers it.
//!   `--use-mock-keychain`, Chromium's own switch for programs driven by
//!   another, keeps the key in the process: the profile is thrown away with
//!   the browser, so nothing needs a key that outlives it.
//! - **A clone of the whole application** under the system's
//!   `…/X/<bundle id>.code_sign_clone/`, which Chromium makes on macOS so a
//!   running browser survives an update replacing its files, and removes
//!   only on an orderly exit. The feature is `MacAppCodeSignClone`, added to
//!   the one `--disable-features` alo passes ([`disable_features`]):
//!   Chromium reads only the last one it is given (measured on Chrome 156,
//!   ADR 0073), so the host's is that one with this feature added, never a
//!   second list that would turn alo's back on.
//!
//! Setting an environment variable in this process is `unsafe` in Rust's
//! 2024 edition, so the host starts itself again in its own place, with the
//! variable set and the two switches added (`main.rs`): the same process,
//! the same descriptors 3 and 4, so the program alo holds is still the
//! browser. [`again`] says whether that is still to do.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The variable Chromium on macOS takes its temporary directory from.
pub const TEMPORARY_VARIABLE: &str = "MAC_CHROMIUM_TMPDIR";

/// The directory, inside the profile, that the host's temporary files go
/// to.
pub const TEMPORARY: &str = "tmp";

/// The feature that clones the application on macOS, switched off.
pub const NO_CLONE: &str = "MacAppCodeSignClone";

/// The switch that keeps the profile's cookie key out of the keychain.
pub const MOCK_KEYCHAIN: &str = "--use-mock-keychain";

/// Which process this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The browser process alo started.
    Browser,
    /// A helper CEF started: a renderer, the GPU process, a utility.
    Helper,
}

/// Which process the command line `args` (without the program) starts.
pub fn role(args: &[OsString]) -> Role {
    let helper = args
        .iter()
        .any(|arg| arg.to_str().is_some_and(|arg| arg.starts_with("--type=")));
    if helper { Role::Helper } else { Role::Browser }
}

/// The profile alo gave the host: the last `--user-data-dir`, as Chromium
/// takes the last of a switch — when it is an absolute path.
pub fn profile(args: &[OsString]) -> Option<PathBuf> {
    let given = args
        .iter()
        .filter_map(|arg| value_of(arg, "--user-data-dir="))
        .next_back()?;
    let given = PathBuf::from(given);
    given.is_absolute().then_some(given)
}

/// Where, inside `profile`, the host's temporary files go.
pub fn temporary(profile: &Path) -> PathBuf {
    profile.join(TEMPORARY)
}

/// What the host must start itself again with, for `profile`, given the
/// command line `args` (without the program) and the value of
/// [`TEMPORARY_VARIABLE`] it was started with — or [`None`] once it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Again {
    /// The value of [`TEMPORARY_VARIABLE`]: [`temporary`].
    pub temporary: PathBuf,
    /// The switches added after alo's.
    pub added: Vec<OsString>,
}

/// Whether the host is still to start itself again ([`Again`]).
pub fn again(args: &[OsString], profile: &Path, variable: Option<&OsStr>) -> Option<Again> {
    let temporary = temporary(profile);
    if variable == Some(temporary.as_os_str()) {
        return None;
    }
    Some(Again {
        temporary,
        added: vec![OsString::from(MOCK_KEYCHAIN), disable_features(args)],
    })
}

/// The one `--disable-features` the host passes: the last one in `args`,
/// which is the one Chromium reads, with [`NO_CLONE`] added unless it is
/// there.
pub fn disable_features(args: &[OsString]) -> OsString {
    let given = args
        .iter()
        .filter_map(|arg| value_of(arg, "--disable-features="))
        .next_back()
        .unwrap_or_default();
    let mut features: Vec<&str> = given.split(',').filter(|name| !name.is_empty()).collect();
    if !features.contains(&NO_CLONE) {
        features.push(NO_CLONE);
    }
    OsString::from(format!("--disable-features={}", features.join(",")))
}

/// The value of the switch `arg` is, when it begins with `named`
/// (`--name=`).
fn value_of<'a>(arg: &'a OsStr, named: &str) -> Option<&'a str> {
    arg.to_str()?.strip_prefix(named)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_helper_is_told_its_type_and_the_browser_never_is() {
        assert_eq!(role(&line(&["--type=renderer", "--lang=en"])), Role::Helper);
        assert_eq!(role(&line(&["--type=gpu-process"])), Role::Helper);
        assert_eq!(
            role(&line(&[
                "--remote-debugging-pipe",
                "--headless",
                "about:blank"
            ])),
            Role::Browser
        );
        // A page's address that mentions the switch is not the switch.
        assert_eq!(role(&line(&["https://a.test/?--type=x"])), Role::Browser);
    }

    #[test]
    fn the_profile_is_the_last_user_data_dir_and_only_an_absolute_one() {
        assert_eq!(
            profile(&line(&["--user-data-dir=/a", "--user-data-dir=/b/c"])),
            Some(PathBuf::from("/b/c"))
        );
        assert_eq!(profile(&line(&["--user-data-dir=relative"])), None);
        assert_eq!(profile(&line(&["--user-data-dir="])), None);
        assert_eq!(profile(&line(&["--remote-debugging-pipe"])), None);
    }

    #[test]
    fn temporary_files_go_inside_the_profile() {
        assert_eq!(temporary(Path::new("/p")), PathBuf::from("/p/tmp"));
    }

    #[test]
    fn alos_features_are_kept_and_the_clone_is_added_once() {
        let alos = "--disable-features=AimEnabled,PreconnectToSearch,AsyncDns";
        assert_eq!(
            disable_features(&line(&["--disable-features=Old", alos])),
            OsString::from(
                "--disable-features=AimEnabled,PreconnectToSearch,AsyncDns,MacAppCodeSignClone"
            )
        );
        assert_eq!(
            disable_features(&line(&["--headless"])),
            OsString::from("--disable-features=MacAppCodeSignClone")
        );
        assert_eq!(
            disable_features(&line(&["--disable-features=MacAppCodeSignClone,A"])),
            OsString::from("--disable-features=MacAppCodeSignClone,A")
        );
    }

    #[test]
    fn the_host_starts_again_once_with_its_temporary_directory_set() {
        let args = line(&["--user-data-dir=/p", "--disable-features=A"]);
        let first = again(&args, Path::new("/p"), None);
        assert_eq!(
            first,
            Some(Again {
                temporary: PathBuf::from("/p/tmp"),
                added: line(&[
                    "--use-mock-keychain",
                    "--disable-features=A,MacAppCodeSignClone"
                ]),
            })
        );
        // Set to somewhere else — the person's own, say — it is set again.
        assert!(again(&args, Path::new("/p"), Some(OsStr::new("/elsewhere"))).is_some());
        // Started again with it set, it goes on.
        assert_eq!(
            again(&args, Path::new("/p"), Some(OsStr::new("/p/tmp"))),
            None
        );
    }
}

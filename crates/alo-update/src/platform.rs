/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which platform's manifest this build reads (ADR 0116 § 3): its word in
//! the address and in the manifest's `platform`.
//!
//! Only macOS updates itself today. Linux outside a Flatpak is off until it
//! is asked for, and inside one Flatpak updates alo (§ 3, *Linux, alo OS
//! and Windows*); Windows has no build yet (728). So only macOS has a word.

/// This build's platform word, if this platform updates itself.
pub fn this() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-arm64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("macos-x64")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_macos_updates_itself() {
        assert_eq!(super::this().is_some(), cfg!(target_os = "macos"));
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(super::this(), Some("macos-arm64"));
        }
    }
}

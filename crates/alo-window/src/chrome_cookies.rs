/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a Chromium keeps of sites' cookies, written into its profile
//! before it starts (ADR 0108 § 1, queue item 920).
//!
//! Every Chromium alo drives starts on a profile of its own, made empty
//! for it ([`crate::chrome_profile`]), so what it keeps of cookies is what
//! that profile's preferences say on its first start. This writes them —
//! one file, `Preferences`, holding Chromium's own two preferences and
//! nothing else:
//!
//! - **`profile.cookie_controls_mode`** — Chromium's
//!   `prefs::kCookieControlsMode`
//!   (`components/content_settings/core/common/pref_names.h`), the
//!   `CookieControlsMode` enum of
//!   `components/content_settings/core/browser/cookie_settings.h`: `0`
//!   off, `1` third-party cookies blocked, `2` blocked in incognito only
//!   (the registered default). `CookieSettings::ShouldBlockThirdPartyCookiesInternal`
//!   (`cookie_settings.cc`) blocks another site's cookies for `1`.
//! - **`profile.default_content_setting_values.cookies`** — the default of
//!   the content setting `cookies` (`content_settings_registry.cc`
//!   registers it as `"cookies"`, `CONTENT_SETTING_ALLOW`;
//!   `website_settings_info.cc` names its default
//!   `profile.default_content_setting_values.` and the name), `2` being
//!   `CONTENT_SETTING_BLOCK` (`content_settings.h`): every site's cookies
//!   refused, its own included.
//!
//! Read in Chromium's source at `main` on 10 November 2026, and measured
//! the same day against Chrome 157.0.8103.50 on this Mac, headless, with a
//! server of the measurement's own setting a first-party cookie on
//! `127.0.0.1` and a `SameSite=None; Secure` one in a frame from
//! `localhost`, each kept an hour, and the page loaded twice: with no file,
//! and with mode `0`, both came back; with mode `1`, the first-party one
//! alone; with the cookies' default `2`, neither. `tests/cookies_refused_
//! as_chosen_through_chrome.rs` holds that through both Chromiums.
//!
//! # Where the file goes
//!
//! Chromium takes `--user-data-dir` as the directory above its profiles,
//! and its first profile is `Default` in it: the file is
//! `Default/Preferences`. So it is for alo's CEF host too, which gives CEF
//! that directory as both `root_cache_path` and `cache_path`
//! (`crates/alo-cef-host`'s `host.rs`) — measured with CEF 156.0.14 the
//! same day: the file written there alone, every choice held; written as
//! `Preferences` in the directory itself alone, both Chromiums passed it
//! over and kept another site's cookies under *Refuse other sites'
//! cookies*, which the test catches.

use alo_renderer::settings_privacy::Cookies;
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

/// The name Chromium gives a profile's preferences.
pub const PREFERENCES_FILE: &str = "Preferences";

/// The machine's Chrome's first profile, in the directory it is given.
pub const FIRST_PROFILE: &str = "Default";

/// What a profile's preferences say of cookies, for `cookies`.
pub fn preferences(cookies: Cookies) -> &'static str {
    match cookies {
        Cookies::RefuseOthers => r#"{"profile":{"cookie_controls_mode":1}}"#,
        Cookies::RefuseAll => {
            r#"{"profile":{"cookie_controls_mode":1,"default_content_setting_values":{"cookies":2}}}"#
        }
        Cookies::KeepAll => r#"{"profile":{"cookie_controls_mode":0}}"#,
    }
}

/// Write what `cookies` asks into the profile at `profile`, which no
/// Chromium has started on yet: its first profile's `Preferences`,
/// readable by this user alone, in a directory only this user can enter.
///
/// # Errors
///
/// What making the first profile's directory or writing the file failed
/// with — one already there included.
pub fn write_into(profile: &Path, cookies: Cookies) -> std::io::Result<()> {
    let first = profile.join(FIRST_PROFILE);
    std::fs::DirBuilder::new().mode(0o700).create(&first)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(first.join(PREFERENCES_FILE))?;
    file.write_all(preferences(cookies).as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_choice_is_chromiums_own_two_preferences() {
        assert_eq!(
            preferences(Cookies::RefuseOthers),
            r#"{"profile":{"cookie_controls_mode":1}}"#
        );
        assert!(preferences(Cookies::RefuseAll).contains(r#""cookies":2"#));
        assert!(preferences(Cookies::RefuseAll).contains(r#""cookie_controls_mode":1"#));
        assert_eq!(
            preferences(Cookies::KeepAll),
            r#"{"profile":{"cookie_controls_mode":0}}"#
        );
    }

    #[test]
    fn the_file_is_the_first_profiles_only_this_users_and_never_over_another() {
        use std::os::unix::fs::PermissionsExt as _;
        let profile =
            std::env::temp_dir().join(format!("alo-chrome-cookies-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&profile);
        assert!(std::fs::create_dir(&profile).is_ok());
        assert!(write_into(&profile, Cookies::RefuseAll).is_ok());
        let file = profile.join(FIRST_PROFILE).join(PREFERENCES_FILE);
        assert_eq!(
            std::fs::read_to_string(&file).ok().as_deref(),
            Some(preferences(Cookies::RefuseAll))
        );
        let mode = std::fs::metadata(&file).map(|about| about.permissions().mode() & 0o777);
        assert_eq!(mode.ok(), Some(0o600));
        let mode = std::fs::metadata(profile.join(FIRST_PROFILE))
            .map(|about| about.permissions().mode() & 0o777);
        assert_eq!(mode.ok(), Some(0o700));
        assert!(!profile.join(PREFERENCES_FILE).exists());
        // A profile already written is not written again.
        assert!(write_into(&profile, Cookies::KeepAll).is_err());
        let _ = std::fs::remove_dir_all(&profile);
    }
}

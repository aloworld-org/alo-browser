/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The Chromium alo carries plays no patent-encumbered codec and holds no
//! DRM (queue item 726, ADR 0079 § 5 and § 8a, the ROADMAP's refusals).
//!
//! alo's own CEF host is started as the application starts it, and a page
//! of the test's own — frozen, so it touches no network — asks its media
//! stack, as any page can, what it plays: `MediaSource.isTypeSupported`
//! and `canPlayType` for H.264, AAC and HEVC in MP4, and for VP9, AV1 and
//! Opus in `WebM`; and `navigator.requestMediaKeySystemAccess` for Widevine,
//! beside Clear Key, the one key system every Chromium carries, so that a
//! refusal of Widevine is Widevine's and not the page's being unable to
//! ask. The page writes each answer into itself, and it is read as the
//! tree an agent reads.
//!
//! The same page in the machine's Chrome, which is Google's and licenses
//! H.264, plays it: the reading reads, and says something different there.
//!
//! # When there is no CEF distribution here
//!
//! As `tests/alos_cef_host_driven_as_chrome.rs`: with `CEF_PATH` unset, or
//! not naming an unpacked distribution, the host's test says why on its
//! error output and returns.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::time::{Duration, Instant};

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long the page's answers, the key systems' among them, are waited
/// for.
const AT_MOST: Duration = Duration::from_secs(30);

/// Each media type asked of the page's media stack, and what it is.
const TYPES: [(&str, &str); 6] = [
    ("H.264", r#"video/mp4; codecs="avc1.42E01E""#),
    ("AAC", r#"audio/mp4; codecs="mp4a.40.2""#),
    ("HEVC", r#"video/mp4; codecs="hvc1.1.6.L93.B0""#),
    ("VP9", r#"video/webm; codecs="vp9""#),
    ("AV1", r#"video/webm; codecs="av01.0.05M.08""#),
    ("Opus", r#"audio/webm; codecs="opus""#),
];

/// The page: a line for each of [`TYPES`], saying whether `MediaSource`
/// takes it and what `canPlayType` says of it, and a line for each key
/// system once its answer is in.
fn page() -> String {
    let asked: Vec<String> = TYPES
        .iter()
        .map(|(name, kind)| format!("[{name:?}, {kind:?}]"))
        .collect();
    format!(
        "<!doctype html><title>Plays</title><div id=said></div><script>\
         const said = document.getElementById('said');\
         function say(line) {{ const p = document.createElement('p');\
           p.textContent = line; said.appendChild(p); }}\
         for (const [name, kind] of [{}]) {{\
           const can = document.createElement('video').canPlayType(kind);\
           say(name + ' source ' + MediaSource.isTypeSupported(kind)\
             + ' element ' + (can === '' ? 'no' : can));\
         }}\
         const wanted = [{{ initDataTypes: ['webm'], videoCapabilities:\
           [{{ contentType: 'video/webm; codecs=\"vp9\"' }}] }}];\
         for (const system of ['com.widevine.alpha', 'org.w3.clearkey']) {{\
           navigator.requestMediaKeySystemAccess(system, wanted).then(\
             () => say(system + ' granted'),\
             (why) => say(system + ' refused ' + why.name));\
         }}</script>",
        asked.join(", ")
    )
}

/// Every line the page wrote, read through `chromium` as the tree an agent
/// reads, once both key systems have answered.
fn lines_through(chromium: &Chromium) -> Result<Vec<String>, String> {
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/plays",
        page().into_bytes(),
        SIZE,
    )
    .map_err(|why| why.to_string())?;
    let until = Instant::now() + AT_MOST;
    loop {
        let read: Vec<String> = tab
            .read()
            .map_err(|why| why.to_string())?
            .nodes()
            .iter()
            .map(ToString::to_string)
            .collect();
        let answered = |system: &str| read.iter().any(|line| line.contains(system));
        if answered("com.widevine.alpha") && answered("org.w3.clearkey") {
            return Ok(read);
        }
        if Instant::now() >= until {
            return Err(format!("the key systems never answered: {read:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Whether `lines` hold one that says `said`.
fn says(lines: &[String], said: &str) -> bool {
    lines.iter().any(|line| line.contains(said))
}

#[test]
fn alos_cef_host_plays_no_patented_codec_and_grants_no_widevine() {
    let Some(host) = carried::host("726").unwrap() else {
        return;
    };
    let lines = lines_through(&host.chromium()).unwrap();
    for refused in ["H.264", "AAC", "HEVC"] {
        assert!(
            says(&lines, &format!("{refused} source false element no")),
            "{refused} is played: {lines:#?}"
        );
    }
    for played in ["VP9", "AV1", "Opus"] {
        assert!(
            says(&lines, &format!("{played} source true element probably")),
            "{played} is not played: {lines:#?}"
        );
    }
    assert!(
        says(&lines, "com.widevine.alpha refused NotSupportedError"),
        "{lines:#?}"
    );
    assert!(says(&lines, "org.w3.clearkey granted"), "{lines:#?}");
}

#[test]
fn the_machines_chrome_plays_h264_on_the_same_page() {
    let lines = lines_through(&Chromium::Machines).unwrap();
    assert!(
        says(&lines, "H.264 source true element probably"),
        "{lines:#?}"
    );
    assert!(says(&lines, "org.w3.clearkey granted"), "{lines:#?}");
}

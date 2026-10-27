/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the macOS application says of itself: its `Contents/Info.plist`
//! (queue item 687; ADR 0067 § 6).
//!
//! The keys are the ones ADR 0067 § 6 lists, and the two no application
//! opens without — which program it runs and which version of the format
//! this is. **No `CFBundleDocumentTypes`**: the application opens no file
//! the Finder hands it until item 684 is built, and saying it did would be
//! lying to the Finder (ADR 0067 § 4). `CFBundleIconFile` names the icon
//! this engine drew, [`crate::bundle_icon::FILE`] (item 689), which the
//! bundle holds.
//!
//! It is written as Apple's XML property list, a format small enough that
//! a few lines of ours write it; every value is a string, escaped.

/// The application's name, the Finder's and the menu bar's.
pub const NAME: &str = "alo browser";

/// The one name no two applications may both claim, chosen by the owner
/// for queue item 483.
pub const IDENTIFIER: &str = "org.aloworld.Browser";

/// The program the application runs, in `Contents/MacOS`.
pub const EXECUTABLE: &str = "alo";

/// The application's version: the workspace's own, said as it is.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Each key of the property list and its value, in the order written.
pub fn entries() -> [(&'static str, &'static str); 10] {
    [
        ("CFBundleDevelopmentRegion", "en"),
        ("CFBundleDisplayName", NAME),
        ("CFBundleExecutable", EXECUTABLE),
        ("CFBundleIconFile", crate::bundle_icon::FILE),
        ("CFBundleIdentifier", IDENTIFIER),
        ("CFBundleInfoDictionaryVersion", "6.0"),
        ("CFBundleName", NAME),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleShortVersionString", VERSION),
        ("CFBundleVersion", VERSION),
    ]
}

/// The property list's text.
pub fn text() -> String {
    let mut text = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n",
    );
    for (key, value) in entries() {
        text.push_str("\t<key>");
        text.push_str(&escaped(key));
        text.push_str("</key>\n\t<string>");
        text.push_str(&escaped(value));
        text.push_str("</string>\n");
    }
    text.push_str("</dict>\n</plist>\n");
    text
}

/// `words` as XML character data.
fn escaped(words: &str) -> String {
    words
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_says_what_adr_0067_lists_and_nothing_it_refuses() {
        let text = text();
        for (key, value) in [
            ("CFBundleIdentifier", "org.aloworld.Browser"),
            ("CFBundleName", "alo browser"),
            ("CFBundleDisplayName", "alo browser"),
            ("CFBundlePackageType", "APPL"),
            ("CFBundleExecutable", "alo"),
            ("CFBundleIconFile", "icon.icns"),
            ("CFBundleShortVersionString", "0.0.0"),
            ("CFBundleVersion", "0.0.0"),
        ] {
            assert!(
                text.contains(&format!("\t<key>{key}</key>\n\t<string>{value}</string>\n")),
                "{key}: {text}"
            );
        }
        assert!(!text.contains("CFBundleDocumentTypes"), "item 684");
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(text.ends_with("</dict>\n</plist>\n"));
        assert_eq!(text.matches("<key>").count(), entries().len());
    }

    #[test]
    fn a_value_is_written_as_character_data() {
        assert_eq!(escaped("a<b>&c"), "a&lt;b&gt;&amp;c");
        assert_eq!(escaped("alo browser"), "alo browser");
    }
}

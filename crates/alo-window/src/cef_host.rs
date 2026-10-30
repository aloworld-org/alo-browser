/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's CEF host laid out as the application macOS needs it to be (queue
//! item 725, ADR 0079 § 3 option H).
//!
//! CEF is a library, loaded by a program of alo's own, `alo-cef-host`
//! (`crates/alo-cef-host`), and on macOS the layout around that program is
//! fixed by CEF and Chromium ([`assemble`]):
//!
//! ```text
//! alo Chromium.app/Contents/
//!   Info.plist                                  says alo
//!   MacOS/alo Chromium                          the host: the browser process
//!   Frameworks/Chromium Embedded Framework.framework
//!   Frameworks/alo Chromium Helper.app          the host again: utilities
//!   Frameworks/alo Chromium Helper (GPU).app    … the GPU process
//!   Frameworks/alo Chromium Helper (Renderer).app
//!   Frameworks/alo Chromium Helper (Plugin).app
//!   Frameworks/alo Chromium Helper (Alerts).app
//! ```
//!
//! Chromium finds each helper by the browser program's own name with
//! ` Helper` and the kind after it, so the names are the program's, not
//! chosen one by one. The program is [`EXECUTABLE`], `alo Chromium`, so
//! that a list of processes tells it from the browser's own `alo`; what a
//! person sees in the menu bar and the Dock is the bundle's name,
//! [`NAME`], `alo` (ADR 0079 § 4) — never CEF's or Chromium's.
//!
//! Each helper is the same program: it knows it is one by the `--type=`
//! CEF starts it with. The framework is CEF's published build, unchanged,
//! cloned in (`cp -c`) so a bundle costs no disk beside the distribution.
//!
//! # What this is not yet
//!
//! It is what a test puts together to start the host as `alo-window`
//! starts Chrome. Carrying it inside `alo browser.app`, sealing it with the
//! owner's identity and saying CEF's licence are queue item 720's.

use crate::info_plist::text_of;
use core::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The host's name, the menu bar's and the Dock's.
pub const NAME: &str = "alo";

/// The host's program, in `Contents/MacOS`; each helper's is this with
/// ` Helper` and its kind after it.
pub const EXECUTABLE: &str = "alo Chromium";

/// The one name the host claims, under the application's own
/// ([`crate::info_plist::IDENTIFIER`]).
pub const IDENTIFIER: &str = "org.aloworld.Browser.Chromium";

/// CEF's framework, as its distribution names it.
pub const FRAMEWORK: &str = "Chromium Embedded Framework.framework";

/// Each helper Chromium starts, as the kind after ` Helper` in its name and
/// the last part of its identifier.
pub const HELPERS: [(&str, &str); 5] = [
    ("", "helper"),
    (" (GPU)", "helper.gpu"),
    (" (Renderer)", "helper.renderer"),
    (" (Plugin)", "helper.plugin"),
    (" (Alerts)", "helper.alerts"),
];

/// The host's version: the workspace's own, said as it is.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The name of the helper of `kind` (one of [`HELPERS`]).
pub fn helper(kind: &str) -> String {
    format!("{EXECUTABLE} Helper{kind}")
}

/// The text of the property list of the program named `executable`, whose
/// identifier is `identifier`: the host's own, or a helper's, which shows
/// no icon in the Dock.
pub fn info_plist(executable: &str, identifier: &str, is_helper: bool) -> String {
    let mut entries = vec![
        ("CFBundleDevelopmentRegion", "en"),
        ("CFBundleDisplayName", NAME),
        ("CFBundleExecutable", executable),
        ("CFBundleIdentifier", identifier),
        ("CFBundleInfoDictionaryVersion", "6.0"),
        ("CFBundleName", NAME),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleShortVersionString", VERSION),
        ("CFBundleVersion", VERSION),
    ];
    if is_helper {
        entries.push(("LSUIElement", "1"));
    }
    text_of(&entries)
}

/// Why the host's application was not put together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unassembled {
    /// What was being done.
    pub doing: String,
    /// What went wrong.
    pub kind: ErrorKind,
}

impl fmt::Display for Unassembled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "alo's CEF host was not put together: {}: {}",
            self.doing, self.kind
        )
    }
}

/// Put the host built at `program` together with CEF's framework at
/// `framework` as the application `application` (`… .app`), which must not
/// be there yet: the path of the program alo starts in it.
///
/// # Errors
///
/// [`Unassembled`], naming the step, when the framework is not at
/// `framework`, or a directory, a copy or the framework's clone could not
/// be made.
pub fn assemble(
    program: &Path,
    framework: &Path,
    application: &Path,
) -> Result<PathBuf, Unassembled> {
    if !framework.is_dir() {
        return Err(Unassembled {
            doing: format!("finding CEF's framework at {}", framework.display()),
            kind: ErrorKind::NotFound,
        });
    }
    let contents = application.join("Contents");
    let frameworks = contents.join("Frameworks");
    let host = contents.join("MacOS").join(EXECUTABLE);
    place(
        program,
        &host,
        &info_plist(EXECUTABLE, IDENTIFIER, false),
        &contents,
    )?;
    for (kind, last) in HELPERS {
        let name = helper(kind);
        let helpers = frameworks.join(format!("{name}.app")).join("Contents");
        let identifier = format!("{IDENTIFIER}.{last}");
        place(
            program,
            &helpers.join("MacOS").join(&name),
            &info_plist(&name, &identifier, true),
            &helpers,
        )?;
    }
    clone(framework, &frameworks.join(FRAMEWORK))?;
    Ok(host)
}

/// `program` copied to `to`, with `plist` as the `Info.plist` of the
/// `contents` it is in.
fn place(program: &Path, to: &Path, plist: &str, contents: &Path) -> Result<(), Unassembled> {
    let programs = to.parent().unwrap_or(contents);
    std::fs::create_dir_all(programs).map_err(|why| failed("making", programs, &why))?;
    std::fs::copy(program, to).map_err(|why| failed("copying the host to", to, &why))?;
    let plist_at = contents.join("Info.plist");
    std::fs::write(&plist_at, plist).map_err(|why| failed("writing", &plist_at, &why))
}

/// Why `doing` to `path` failed.
fn failed(doing: &str, path: &Path, why: &std::io::Error) -> Unassembled {
    Unassembled {
        doing: format!("{doing} {}", path.display()),
        kind: why.kind(),
    }
}

/// CEF's framework at `framework` cloned to `to`, copy on write.
fn clone(framework: &Path, to: &Path) -> Result<(), Unassembled> {
    let doing = format!("cloning {} to {}", framework.display(), to.display());
    let cloned = Command::new("/bin/cp")
        .arg("-c")
        .arg("-R")
        .arg(framework)
        .arg(to)
        .status()
        .map_err(|why| Unassembled {
            doing: doing.clone(),
            kind: why.kind(),
        })?;
    if cloned.success() {
        Ok(())
    } else {
        Err(Unassembled {
            doing,
            kind: ErrorKind::Other,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_and_every_helper_say_alo() {
        let host = info_plist(EXECUTABLE, IDENTIFIER, false);
        for (key, value) in [
            ("CFBundleName", "alo"),
            ("CFBundleDisplayName", "alo"),
            ("CFBundleExecutable", "alo Chromium"),
            ("CFBundleIdentifier", "org.aloworld.Browser.Chromium"),
            ("CFBundlePackageType", "APPL"),
        ] {
            assert!(
                host.contains(&format!("\t<key>{key}</key>\n\t<string>{value}</string>\n")),
                "{key}: {host}"
            );
        }
        assert!(!host.contains("LSUIElement"), "the host may be in the Dock");
        assert!(!host.to_lowercase().contains("cef"), "{host}");
        let renderer = info_plist(
            &helper(" (Renderer)"),
            "org.aloworld.Browser.Chromium.helper.renderer",
            true,
        );
        assert!(renderer.contains(
            "\t<key>CFBundleExecutable</key>\n\t<string>alo Chromium Helper (Renderer)</string>\n"
        ));
        assert!(renderer.contains("\t<key>CFBundleName</key>\n\t<string>alo</string>\n"));
        assert!(renderer.contains("\t<key>LSUIElement</key>\n\t<string>1</string>\n"));
    }

    #[test]
    fn the_helpers_are_named_after_the_program_as_chromium_looks_for_them() {
        let names: Vec<String> = HELPERS.iter().map(|(kind, _)| helper(kind)).collect();
        assert_eq!(
            names,
            [
                "alo Chromium Helper",
                "alo Chromium Helper (GPU)",
                "alo Chromium Helper (Renderer)",
                "alo Chromium Helper (Plugin)",
                "alo Chromium Helper (Alerts)",
            ]
        );
    }

    #[test]
    fn a_missing_framework_is_refused_naming_the_step() {
        let scratch = std::env::temp_dir().join(format!("alo-cef-host-{}", std::process::id()));
        let refused = assemble(
            Path::new("/bin/sh"),
            &scratch.join("no framework here"),
            &scratch.join("alo Chromium.app"),
        )
        .unwrap_err();
        assert_eq!(refused.kind, ErrorKind::NotFound);
        assert!(
            refused.doing.starts_with("finding CEF's framework at "),
            "{refused}"
        );
        // Looked for first, so nothing was put together without it.
        assert!(!scratch.exists());
    }
}

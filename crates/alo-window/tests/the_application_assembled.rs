/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-bundle` makes the macOS application, and `codesign` accepts it
//! (queue item 687; ADR 0067 § 6) — here with no CEF distribution named, so
//! carrying no Chromium and saying so (queue item 720; the application
//! carrying one is `tests/the_application_carrying_its_chromium.rs`).
//!
//! The real command, given the programs this test was built with, makes
//! `alo browser.app` in a directory of the test's own. What it holds is read
//! back file by file: the three programs, the two pages byte for byte as
//! the corpus has them, an `Info.plist` that Apple's own `plutil` reads
//! the identifier, the program, the version and the icon out of, and an
//! `.icns` whose every picture is the corpus's reference byte for byte and
//! which Apple's own `iconutil` reads back as an icon of every size (queue
//! item 689), and the licences of the typefaces `alo` carries byte for byte
//! as published with the fonts (queue item 706). `codesign` checks
//! the seal on its own, strictly; and a page changed after sealing is
//! refused by it, so the seal covers what the application carries. Made a
//! second time into the same place, it is refused and the first left as it
//! was.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A directory of this test's own, empty.
fn directory(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-assembled-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    Ok(root)
}

/// `alo-bundle`, making the application into `into` from the programs this
/// test was built with, and with no CEF distribution named — so carrying
/// no Chromium (`tests/the_application_carrying_its_chromium.rs` names one).
fn assemble(into: &Path) -> Result<Output, String> {
    let alo = Path::new(env!("CARGO_BIN_EXE_alo"));
    let programs = alo.parent().ok_or("alo is in no directory")?;
    Command::new(env!("CARGO_BIN_EXE_alo-bundle"))
        .arg("--programs")
        .arg(programs)
        .arg("--into")
        .arg(into)
        .env_remove("CEF_PATH")
        .output()
        .map_err(|why| format!("alo-bundle did not run: {why}"))
}

/// The licences of the typefaces `alo` carries (item 706, ADR 0084), in the
/// application's `Contents` at `contents`: the files published beside the
/// fonts, byte for byte.
fn the_typefaces_licences_are_the_published_ones(contents: &Path) -> Result<(), String> {
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-renderer/interface/fonts");
    for name in [
        "OFL-InstrumentSans.txt",
        "OFL-Newsreader.txt",
        "OFL-NotoColorEmoji.txt",
    ] {
        let published = std::fs::read(fonts.join(name)).map_err(|why| format!("{name}: {why}"))?;
        let carried = std::fs::read(contents.join("Resources/licences").join(name))
            .map_err(|why| format!("{name}: {why}"))?;
        assert!(
            published == carried,
            "{name} is not the licence published with its font"
        );
    }
    Ok(())
}

/// Every file under `root`, relative to it, in order.
fn files_under(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut left = vec![root.to_path_buf()];
    while let Some(directory) = left.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                left.push(path);
            } else if let Ok(relative) = path.strip_prefix(root) {
                found.push(relative.display().to_string());
            }
        }
    }
    found.sort();
    found
}

/// What `plutil` reads as `key` in the property list at `list`.
fn read_by_plutil(list: &Path, key: &str) -> Result<String, String> {
    let read = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(list)
        .output()
        .map_err(|why| format!("plutil did not run: {why}"))?;
    Ok(String::from_utf8_lossy(&read.stdout).trim().to_owned())
}

/// Whether `codesign` accepts `application`'s seal, and what it said.
fn checked(application: &Path) -> Result<(bool, String), String> {
    let ran = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(application)
        .output()
        .map_err(|why| format!("codesign did not run: {why}"))?;
    Ok((
        ran.status.success(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
    ))
}

/// Assert the `.icns` at `icon` holds each picture of the corpus case
/// `the-applications-icon` byte for byte, in the entry for its size, and
/// that `iconutil` reads it back as an icon set of every size.
fn the_icon_is_the_corpus_pictures(cases: &Path, icon: &Path) -> Result<(), String> {
    let icns = std::fs::read(icon).map_err(|why| format!("the icon: {why}"))?;
    let read = alo_window::icns::entries(&icns).ok_or("the icon is not an .icns")?;
    let held: Vec<(String, u32)> = alo_window::bundle_icon::ENTRIES
        .iter()
        .map(|(kind, side)| (String::from_utf8_lossy(kind).into_owned(), *side))
        .collect();
    assert_eq!(
        held,
        [
            ("icp4", 16),
            ("icp5", 32),
            ("ic11", 32),
            ("ic12", 64),
            ("ic07", 128),
            ("ic13", 256),
            ("ic08", 256),
            ("ic14", 512),
            ("ic09", 512),
            ("ic10", 1024),
        ]
        .map(|(kind, side)| (kind.to_owned(), side))
    );
    assert_eq!(read.len(), held.len());
    for ((kind, bytes), (wanted, side)) in read.iter().zip(&held) {
        assert_eq!(String::from_utf8_lossy(kind), *wanted);
        let reference = cases
            .join("the-applications-icon")
            .join(format!("icon-{side}.png"));
        let drawn = std::fs::read(&reference).map_err(|why| format!("{side}: {why}"))?;
        assert!(
            *bytes == drawn.as_slice(),
            "{wanted} is not icon-{side}.png"
        );
    }

    // Read back outside the application, whose seal is checked after.
    let set = directory("read back")?.join("icon.iconset");
    let ran = Command::new("/usr/bin/iconutil")
        .args(["-c", "iconset"])
        .arg(icon)
        .arg("-o")
        .arg(&set)
        .output()
        .map_err(|why| format!("iconutil did not run: {why}"))?;
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let mut sizes = Vec::new();
    for points in [16, 32, 128, 256, 512] {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let picture = set.join(format!("icon_{points}x{points}{suffix}.png"));
            let png = std::fs::read(&picture).map_err(|why| format!("{points}{suffix}: {why}"))?;
            let width = png
                .get(16..20)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(u32::from_be_bytes);
            sizes.push((points, scale, width));
        }
    }
    if let Some(own) = set.parent() {
        let _ = std::fs::remove_dir_all(own);
    }
    for (points, scale, width) in sizes {
        assert_eq!(width, Some(points * scale), "{points} points at {scale}");
    }
    Ok(())
}

#[test]
fn the_command_makes_a_sealed_application_holding_what_it_should() -> Result<(), String> {
    let root = directory("made")?;
    let made = assemble(&root)?;
    let said = String::from_utf8_lossy(&made.stderr).into_owned();
    assert!(made.status.success(), "{said}");
    let application = root.join("alo browser.app");
    assert_eq!(
        String::from_utf8_lossy(&made.stdout),
        format!("{}\n", application.display())
    );
    assert_eq!(
        said,
        "alo-bundle: CEF_PATH is not set, so the application carries no Chromium: it draws \
         its page with this browser's own engine, and says so\n\
         alo-bundle: made, sealed ad hoc and checked by codesign\n"
    );
    assert_eq!(
        files_under(&root),
        [
            "alo browser.app/Contents/Info.plist",
            "alo browser.app/Contents/MacOS/alo",
            "alo browser.app/Contents/MacOS/alo-ask",
            "alo browser.app/Contents/MacOS/alo-render",
            "alo browser.app/Contents/Resources/icon.icns",
            "alo browser.app/Contents/Resources/licences/OFL-InstrumentSans.txt",
            "alo browser.app/Contents/Resources/licences/OFL-Newsreader.txt",
            "alo browser.app/Contents/Resources/licences/OFL-NotoColorEmoji.txt",
            "alo browser.app/Contents/Resources/page.html",
            "alo browser.app/Contents/Resources/without-chrome.html",
            "alo browser.app/Contents/_CodeSignature/CodeResources",
        ],
        "the application, and nothing half-made beside it"
    );

    let contents = application.join("Contents");
    let list = contents.join("Info.plist");
    for (key, value) in [
        ("CFBundleIdentifier", "org.aloworld.Browser"),
        ("CFBundleExecutable", "alo"),
        ("CFBundleIconFile", "icon.icns"),
        ("CFBundleName", "alo browser"),
        ("CFBundleDisplayName", "alo browser"),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleShortVersionString", env!("CARGO_PKG_VERSION")),
        ("CFBundleVersion", env!("CARGO_PKG_VERSION")),
    ] {
        assert_eq!(read_by_plutil(&list, key)?, value, "{key}");
    }
    assert_eq!(
        read_by_plutil(&list, "CFBundleDocumentTypes")?,
        "",
        "item 684"
    );

    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases");
    for (case, name) in [
        ("the-applications-page", "page.html"),
        (
            "the-applications-page-without-chrome",
            "without-chrome.html",
        ),
    ] {
        let corpus = std::fs::read(cases.join(case).join("page.html"))
            .map_err(|why| format!("{case}: {why}"))?;
        let carried = std::fs::read(contents.join("Resources").join(name))
            .map_err(|why| format!("{name}: {why}"))?;
        assert!(corpus == carried, "{name} is not {case}'s page");
    }
    the_icon_is_the_corpus_pictures(&cases, &contents.join("Resources/icon.icns"))?;
    the_typefaces_licences_are_the_published_ones(&contents)?;
    for program in ["alo", "alo-render", "alo-ask"] {
        assert!(
            alo_window::beside::is_program(&contents.join("MacOS").join(program)),
            "{program}"
        );
    }
    assert!(
        alo_window::bundle::Bundle::around(&contents.join("MacOS").join("alo")).is_some(),
        "alo knows it is inside the application"
    );

    let (accepted, said) = checked(&application)?;
    assert!(accepted, "{said}");

    // Made again into the same place: refused, and the first kept.
    let again = assemble(&root)?;
    assert_eq!(again.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&again.stderr).ends_with("is not replaced: remove it first\n"),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert!(checked(&application)?.0, "the first is as it was");

    // A page changed after it was sealed is known to be.
    let page = contents.join("Resources").join("page.html");
    let mut changed = std::fs::read(&page).map_err(|why| why.to_string())?;
    changed.extend_from_slice(b"<p>changed");
    std::fs::write(&page, changed).map_err(|why| why.to_string())?;
    let (accepted, said) = checked(&application)?;
    assert!(!accepted, "a changed page was accepted");
    assert!(
        said.contains("a sealed resource is missing or invalid"),
        "{said}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn programs_not_built_are_named_and_nothing_is_made() -> Result<(), String> {
    let root = directory("unbuilt")?.join("nothing built");
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    let made = Command::new(env!("CARGO_BIN_EXE_alo-bundle"))
        .arg("--programs")
        .arg(&root)
        .arg("--into")
        .arg(&root)
        .env_remove("CEF_PATH")
        .output()
        .map_err(|why| format!("alo-bundle did not run: {why}"))?;
    assert_eq!(made.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&made.stderr),
        format!(
            "alo-bundle: CEF_PATH is not set, so the application carries no Chromium: it draws \
             its page with this browser's own engine, and says so\n\
             alo-bundle: {} is not there: build the programs first, with cargo build --release\n",
            root.join("alo").display()
        )
    );
    assert_eq!(files_under(&root), Vec::<String>::new());
    if let Some(parent) = root.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
    Ok(())
}

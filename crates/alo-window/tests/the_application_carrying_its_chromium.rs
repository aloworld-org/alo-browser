/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-bundle` makes the macOS application carrying CEF's Chromium, inside
//! alo's own host, checked against its pin and sealed (queue item 720; ADR
//! 0077, ADR 0079).
//!
//! The host is built here against the distribution `CEF_PATH` names, as
//! `alos_cef_host_driven_as_chrome.rs` builds it, and the real `alo-bundle`
//! makes `alo browser.app` with it. Read back: the host's program is at the
//! place the application starts its Chromium, and is the host built — the
//! same bytes once each one's signature is taken off; each helper is there,
//! and CEF's framework, every file of it but the code `codesign` signed
//! again byte for byte the distribution's, whose own digest is the pin;
//! CEF's licence and Chromium's credits are in `Contents/Resources/licences`,
//! byte for byte the distribution's and at their pinned SHA-256, and the
//! application's page links both. `codesign --verify --deep --strict`
//! accepts it, the GPU and renderer helpers hold the JIT entitlement and
//! the others none. Then the refusals, each making nothing: a distribution
//! with one byte of its framework changed, one whose `archive.json` names
//! another archive, a directory holding none, and a host not built.
//!
//! # When there is no CEF distribution here
//!
//! The host cannot be built without one, so with `CEF_PATH` unset, or not
//! naming an unpacked distribution, this test says so on its error output
//! and returns — **skipped, saying why**, as the host's own tests are.
//! `tests/the_application_assembled.rs` makes the application without one,
//! which carries no Chromium.

#![cfg(target_os = "macos")]

use alo_net::digest::Digest;
use alo_window::bundle_carried::{LICENCES, host_application};
use alo_window::cef_host::{EXECUTABLE, FRAMEWORK, HELPERS, helper};
use alo_window::cef_pin::{self, ARCHIVE_JSON, FRAMEWORK_SHA256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The CEF distribution to build against, or why there is none.
fn distribution() -> Result<PathBuf, String> {
    let Some(named) = std::env::var_os("CEF_PATH") else {
        return Err("CEF_PATH is not set".to_owned());
    };
    let named = PathBuf::from(named);
    if named.join(FRAMEWORK).is_dir() && named.join(ARCHIVE_JSON).is_file() {
        Ok(named)
    } else {
        Err(format!(
            "CEF_PATH names {}, which holds no unpacked CEF distribution",
            named.display()
        ))
    }
}

/// The host built against the distribution at `cef`: its program.
fn built(cef: &Path) -> Result<PathBuf, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = repository.join("target/alo-cef-host");
    let status = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(repository.join("crates/alo-cef-host/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("CEF_PATH", cef)
        .status()
        .map_err(|why| format!("cargo did not start: {why}"))?;
    if !status.success() {
        return Err(format!("the host did not build: {status}"));
    }
    Ok(target.join("debug/alo-cef-host"))
}

/// A directory of this test's own, empty.
fn directory(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-carrying-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    Ok(root)
}

/// The test's own directories, removed when the test ends however it ends.
struct RemovedAtTheEnd(Vec<PathBuf>);

impl Drop for RemovedAtTheEnd {
    fn drop(&mut self) {
        for directory in &self.0 {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

/// `alo-bundle`, making the application into `into` from the programs this
/// test was built with, carrying the host at `host` and the distribution at
/// `cef`.
fn assemble(into: &Path, cef: &Path, host: &Path) -> Result<Output, String> {
    let alo = Path::new(env!("CARGO_BIN_EXE_alo"));
    let programs = alo.parent().ok_or("alo is in no directory")?;
    Command::new(env!("CARGO_BIN_EXE_alo-bundle"))
        .arg("--programs")
        .arg(programs)
        .arg("--into")
        .arg(into)
        .arg("--host")
        .arg(host)
        .env("CEF_PATH", cef)
        .output()
        .map_err(|why| format!("alo-bundle did not run: {why}"))
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

/// The SHA-256 of the file at `path`, in hex.
fn sha256(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|why| format!("{}: {why}", path.display()))?;
    let digest = Digest::Sha256
        .of_read(file)
        .map_err(|why| format!("{}: {why}", path.display()))?;
    Ok(cef_pin::hex(&digest))
}

/// Run `codesign` with `options` on `path`: whether it agreed, and what it
/// said on each output.
fn codesign(options: &[&str], path: &Path) -> Result<(bool, String), String> {
    let ran = Command::new("/usr/bin/codesign")
        .args(options)
        .arg(path)
        .output()
        .map_err(|why| format!("codesign did not run: {why}"))?;
    Ok((
        ran.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&ran.stdout),
            String::from_utf8_lossy(&ran.stderr)
        ),
    ))
}

/// The bytes of the program at `program` with its signature taken off, from
/// a copy at `copy`.
fn unsigned(program: &Path, copy: &Path) -> Result<Vec<u8>, String> {
    std::fs::copy(program, copy).map_err(|why| format!("{}: {why}", program.display()))?;
    let (removed, said) = codesign(&["--remove-signature"], copy)?;
    if !removed {
        return Err(format!("{}: {said}", program.display()));
    }
    std::fs::read(copy).map_err(|why| why.to_string())
}

/// Whether the file at `path` is Mach-O code, by its first four bytes.
fn is_code(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        bytes
            .get(..4)
            .is_some_and(|magic| magic == [0xcf, 0xfa, 0xed, 0xfe])
    })
}

/// Assert CEF's framework carried at `carried` is the distribution's at
/// `original`: the same files, `codesign`'s seal added; every file that is
/// not code the same bytes; and the code, signed again, accepted by
/// `codesign`. The code files: their names.
fn the_framework_is_the_distributions(
    carried: &Path,
    original: &Path,
) -> Result<Vec<String>, String> {
    let held: Vec<String> = files_under(carried)
        .into_iter()
        .filter(|file| !file.starts_with("_CodeSignature/"))
        .collect();
    assert_eq!(held, files_under(original), "the framework's files");
    let mut code = Vec::new();
    for file in &held {
        let (ours, theirs) = (carried.join(file), original.join(file));
        if is_code(&theirs) {
            let (accepted, said) = codesign(&["--verify", "--strict"], &ours)?;
            assert!(accepted, "{file}: {said}");
            code.push(file.clone());
        } else {
            assert_eq!(sha256(&ours)?, sha256(&theirs)?, "{file}");
        }
    }
    Ok(code)
}

/// What `codesign` says `bundle` is entitled to, as property-list text.
fn entitlements(bundle: &Path) -> Result<String, String> {
    let (_, said) = codesign(&["-d", "--entitlements", "-", "--xml"], bundle)?;
    Ok(said)
}

/// Assert the application at `application` carries the licences of the
/// distribution at `cef`, linked from its page, and is sealed inside out.
fn the_licences_are_linked_and_the_seal_holds(
    application: &Path,
    cef: &Path,
) -> Result<(), String> {
    let contents = application.join("Contents");
    let hosted = host_application(application);
    // Its licences, beside the page that links them.
    let page = std::fs::read_to_string(contents.join("Resources/page.html"))
        .map_err(|why| why.to_string())?;
    for (name, pinned) in cef_pin::LICENCES {
        let carried = contents.join("Resources").join(LICENCES).join(name);
        assert_eq!(sha256(&carried)?, pinned, "{name}");
        assert_eq!(sha256(&carried)?, sha256(&cef.join(name))?, "{name}");
        assert!(
            page.contains(&format!("href=\"{LICENCES}/{name}\"")),
            "the page links {name}"
        );
    }

    // Sealed, inside out: accepted whole, strictly.
    let (accepted, said) = codesign(&["--verify", "--deep", "--strict"], application)?;
    assert!(accepted, "{said}");
    for (kind, _) in HELPERS {
        let helper = hosted
            .join("Contents/Frameworks")
            .join(format!("{}.app", helper(kind)));
        let held = entitlements(&helper)?;
        let jit = held.contains("<key>com.apple.security.cs.allow-jit</key><true/>");
        assert_eq!(
            jit,
            kind == " (GPU)" || kind == " (Renderer)",
            "{kind}: {held}"
        );
    }
    Ok(())
}

#[test]
fn the_application_carries_cefs_chromium_checked_against_its_pin_and_sealed() -> Result<(), String>
{
    let cef = match distribution() {
        Ok(cef) => cef,
        Err(why) => {
            eprintln!(
                "skipped: the application carries CEF's Chromium only when there is a CEF \
                 distribution to build alo's host against, and {why} (queue item 720)"
            );
            return Ok(());
        }
    };
    let host = built(&cef)?;
    let root = directory("made")?;
    let _removed = RemovedAtTheEnd(vec![root.clone()]);
    let made = assemble(&root, &cef, &host)?;
    let said = String::from_utf8_lossy(&made.stderr).into_owned();
    assert!(made.status.success(), "{said}");
    assert_eq!(
        said,
        format!(
            "alo-bundle: carries {}, checked against its pinned SHA-256\n\
             alo-bundle: made, sealed ad hoc and checked by codesign\n",
            cef_pin::VERSION
        )
    );
    let application = root.join("alo browser.app");
    assert_eq!(
        String::from_utf8_lossy(&made.stdout),
        format!("{}\n", application.display())
    );
    let contents = application.join("Contents");

    // The host, where the application starts its Chromium: the host built.
    let alo = contents.join("MacOS/alo");
    let bundle = alo_window::bundle::Bundle::around(&alo).ok_or("not in the application")?;
    let program = bundle.chromium();
    let hosted = host_application(&application);
    assert_eq!(program, hosted.join("Contents/MacOS").join(EXECUTABLE));
    assert!(alo_window::beside::is_program(&program));
    assert!(
        unsigned(&program, &root.join("carried"))? == unsigned(&host, &root.join("built"))?,
        "the host carried is not the host built"
    );
    for (kind, _) in HELPERS {
        let name = helper(kind);
        let helper = hosted
            .join("Contents/Frameworks")
            .join(format!("{name}.app/Contents/MacOS"))
            .join(&name);
        assert!(alo_window::beside::is_program(&helper), "{name}");
    }

    // CEF's framework: the pinned one, carried.
    let original = cef.join(FRAMEWORK);
    assert_eq!(
        cef_pin::framework_digest(&original).map_err(|why| why.to_string())?,
        FRAMEWORK_SHA256,
        "the distribution is the pinned one, still, after it was carried"
    );
    let code = the_framework_is_the_distributions(
        &hosted.join("Contents/Frameworks").join(FRAMEWORK),
        &original,
    )?;
    assert_eq!(
        code,
        [
            "Chromium Embedded Framework",
            "Libraries/libcef_sandbox.dylib",
            "Libraries/libvk_swiftshader.dylib",
            "Libraries/libvulkan.dylib",
        ]
    );

    the_licences_are_linked_and_the_seal_holds(&application, &cef)?;
    let size = Command::new("/usr/bin/du")
        .arg("-sk")
        .arg(&application)
        .output()
        .map_err(|why| why.to_string())?;
    eprintln!(
        "the application carrying CEF's Chromium, in KiB: {}",
        String::from_utf8_lossy(&size.stdout).trim()
    );
    Ok(())
}

/// A distribution of this test's own under `root`, the one at `cef` cloned
/// (`cp -c`, costing no disk): its directory.
fn cloned(cef: &Path, root: &Path) -> Result<PathBuf, String> {
    let clone = root.join("cef");
    std::fs::create_dir_all(&clone).map_err(|why| why.to_string())?;
    for name in [FRAMEWORK, ARCHIVE_JSON, "LICENSE.txt", "CREDITS.html"] {
        let copied = Command::new("/bin/cp")
            .arg("-cR")
            .arg(cef.join(name))
            .arg(clone.join(name))
            .status()
            .map_err(|why| why.to_string())?;
        if !copied.success() {
            return Err(format!("{name} was not cloned"));
        }
    }
    Ok(clone)
}

/// `alo-bundle` refused, saying `wanted` after `alo-bundle: `, and nothing
/// made in `into`.
fn refused(made: &Output, wanted: &str, into: &Path) {
    assert_eq!(made.status.code(), Some(2), "{made:?}");
    assert_eq!(
        String::from_utf8_lossy(&made.stderr),
        format!("alo-bundle: {wanted}\n")
    );
    assert_eq!(
        std::fs::read_dir(into).map_or(1, Iterator::count),
        0,
        "nothing made"
    );
}

#[test]
fn a_distribution_not_pinned_or_a_host_not_built_is_refused_and_nothing_is_made()
-> Result<(), String> {
    let cef = match distribution() {
        Ok(cef) => cef,
        Err(why) => {
            eprintln!("skipped: refusing a CEF distribution needs one to change, and {why}");
            return Ok(());
        }
    };
    let host = built(&cef)?;
    let root = directory("refused")?;
    let _removed = RemovedAtTheEnd(vec![root.clone()]);
    let into = root.join("into");
    std::fs::create_dir_all(&into).map_err(|why| why.to_string())?;

    // One byte of one file in the framework changed.
    let changed = cloned(&cef, &root)?;
    let framework = changed.join(FRAMEWORK);
    let file = framework.join("Libraries/vk_swiftshader_icd.json");
    let mut bytes = std::fs::read(&file).map_err(|why| why.to_string())?;
    bytes.push(b'\n');
    std::fs::write(&file, bytes).map_err(|why| why.to_string())?;
    let found = cef_pin::framework_digest(&framework).map_err(|why| why.to_string())?;
    assert_ne!(found, FRAMEWORK_SHA256);
    refused(
        &assemble(&into, &changed, &host)?,
        &format!(
            "{} is not the CEF alo pinned: its SHA-256 is {found}, not {FRAMEWORK_SHA256}, so \
             it is not carried",
            framework.display()
        ),
        &into,
    );

    // Its `archive.json` naming another archive.
    std::fs::write(
        changed.join(ARCHIVE_JSON),
        "{\"name\": \"cef_binary_155.0.1_macosarm64_minimal.tar.bz2\", \"sha1\": \"0\"}",
    )
    .map_err(|why| why.to_string())?;
    refused(
        &assemble(&into, &changed, &host)?,
        &format!(
            "the CEF distribution at {} is cef_binary_155.0.1_macosarm64_minimal.tar.bz2 with \
             SHA-1 0, not the one alo carries, {} with SHA-1 {}",
            changed.display(),
            cef_pin::ARCHIVE,
            cef_pin::ARCHIVE_SHA1
        ),
        &into,
    );

    // A directory holding no distribution.
    refused(
        &assemble(&into, &into, &host)?,
        &format!(
            "{} holds no unpacked CEF distribution: CEF's framework and its archive.json are \
             not both there",
            into.display()
        ),
        &into,
    );

    // The host not built.
    let nowhere = root.join("no host");
    refused(
        &assemble(&into, &cef, &nowhere)?,
        &format!(
            "alo's CEF host is not at {}: build it first, with {}",
            nowhere.display(),
            alo_window::bundle_carried::BUILD_THE_HOST
        ),
        &into,
    );
    Ok(())
}

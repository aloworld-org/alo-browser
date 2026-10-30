/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-bundle`: make the macOS application, `alo browser.app` (queue items
//! 687 and 720; ADR 0067 § 6, ADR 0077, ADR 0079).
//!
//! `alo-bundle [--programs DIRECTORY] [--into DIRECTORY] [--host PROGRAM]`
//! puts `alo`, `alo-render` and `alo-ask` from the programs' directory into
//! a new `alo browser.app` in the other, with the application's two pages
//! and its `Info.plist` ([`alo_window::bundle_made`]); carries the Chromium
//! the application draws the web with — alo's CEF host and CEF's framework
//! from the distribution `CEF_PATH` names, checked against its pin first,
//! with CEF's licences ([`alo_window::bundle_carried`]); seals it ad hoc with
//! `codesign`, inside out, and checks the seal
//! ([`alo_window::bundle_seal`]); and prints where it is. Both directories
//! are the one this program is in unless named, so after
//!
//! ```sh
//! cargo build --release
//! CEF_PATH=… cargo build --release --locked \
//!   --manifest-path crates/alo-cef-host/Cargo.toml --target-dir target/alo-cef-host
//! CEF_PATH=… target/release/alo-bundle
//! ```
//!
//! the application is `target/release/alo browser.app`. The host is
//! `target/alo-cef-host/release/alo-cef-host` there — beside the programs'
//! directory, under `alo-cef-host`, in the directory of the programs' own
//! name — unless `--host` names it.
//!
//! **Without `CEF_PATH`** (unset or empty) it makes the application with no
//! Chromium, and says so: that application draws its page with this
//! browser's own engine, on the page saying its Chromium is missing (queue
//! item 719). **With `CEF_PATH` naming something that is not the pinned
//! distribution, or no host built**, it refuses with a sentence saying what
//! is wrong, and makes nothing: a Chromium asked for is never quietly left
//! out.
//!
//! It builds nothing: a program not there is named, with the `cargo`
//! command that makes it. An application already there is refused and left
//! as it is.

use alo_window::bundle_carried::{self, Carried};
use alo_window::bundle_made::{PROGRAMS, make};
use alo_window::bundle_seal::{seal, seal_carried};
use alo_window::cef_pin;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

/// What was named on the command line.
struct Named {
    /// The programs' directory.
    programs: PathBuf,
    /// Where the application goes.
    into: PathBuf,
    /// alo's CEF host's program, if named.
    host: Option<PathBuf>,
}

/// The directories and the host named, or where they are by default.
fn named() -> Result<Named, String> {
    let mut programs = None;
    let mut into = None;
    let mut host = None;
    let mut words = std::env::args_os().skip(1);
    while let Some(word) = words.next() {
        let slot = match word.to_str() {
            Some("--programs") => &mut programs,
            Some("--into") => &mut into,
            Some("--host") => &mut host,
            _ => {
                return Err(format!(
                    "{} is not an option of this",
                    word.to_string_lossy()
                ));
            }
        };
        let Some(path) = words.next() else {
            return Err(format!("{} names nothing", word.to_string_lossy()));
        };
        *slot = Some(PathBuf::from(path));
    }
    let here = || {
        std::env::current_exe()
            .ok()
            .and_then(|program| program.parent().map(PathBuf::from))
            .ok_or("where this program is is not known: name both directories")
    };
    let programs = match programs {
        Some(named) => named,
        None => here()?,
    };
    let into = match into {
        Some(named) => named,
        None => here()?,
    };
    Ok(Named {
        programs,
        into,
        host,
    })
}

/// Where `cargo` puts the host built beside the programs in `programs`:
/// `target/alo-cef-host/release/alo-cef-host` for `target/release`.
fn host_beside(programs: &std::path::Path) -> Option<PathBuf> {
    let profile = programs.file_name()?;
    let target = programs.parent()?;
    Some(
        target
            .join("alo-cef-host")
            .join(profile)
            .join("alo-cef-host"),
    )
}

/// The Chromium to carry: none when `CEF_PATH` names nothing, or the host
/// and the distribution, both checked.
fn chromium(named: &Named) -> Result<Option<Carried>, String> {
    let distribution = std::env::var_os("CEF_PATH").filter(|named| !named.is_empty());
    let Some(distribution) = distribution.map(|named: OsString| PathBuf::from(named)) else {
        eprintln!(
            "alo-bundle: CEF_PATH is not set, so the application carries no Chromium: it draws \
             its page with this browser's own engine, and says so"
        );
        return Ok(None);
    };
    let host = match &named.host {
        Some(host) => host.clone(),
        None => host_beside(&named.programs).ok_or(
            "where alo's CEF host is is not known from the programs' directory: name it with --host",
        )?,
    };
    let carried = bundle_carried::checked(&distribution, &host).map_err(|why| why.to_string())?;
    eprintln!(
        "alo-bundle: carries {}, checked against its pinned SHA-256",
        cef_pin::VERSION
    );
    Ok(Some(carried))
}

fn main() -> ExitCode {
    let named = match named() {
        Ok(named) => named,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            eprintln!(
                "alo-bundle: alo-bundle [--programs DIRECTORY] [--into DIRECTORY] [--host PROGRAM]"
            );
            return ExitCode::from(2);
        }
    };
    let carried = match chromium(&named) {
        Ok(carried) => carried,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            return ExitCode::from(2);
        }
    };
    let made = match make(&named.programs, &named.into) {
        Ok(made) => made,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            return ExitCode::from(2);
        }
    };
    if let Some(carried) = carried {
        let application = made.application();
        if let Err(why) = carried.put_in(&application) {
            eprintln!("alo-bundle: {why}");
            return ExitCode::FAILURE;
        }
        let host = bundle_carried::host_application(&application);
        if let Err(why) = seal_carried(&host, made.making()) {
            eprintln!("alo-bundle: {why}");
            return ExitCode::FAILURE;
        }
    }
    let nested: Vec<&str> = PROGRAMS.iter().skip(1).copied().collect();
    if let Err(why) = seal(&made.application(), &nested) {
        eprintln!("alo-bundle: {why}");
        return ExitCode::FAILURE;
    }
    match made.put_in_place() {
        Ok(application) => {
            eprintln!("alo-bundle: made, sealed ad hoc and checked by codesign");
            println!("{}", application.display());
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            ExitCode::FAILURE
        }
    }
}

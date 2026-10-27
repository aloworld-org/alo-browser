/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-bundle`: make the macOS application, `alo browser.app` (queue item
//! 687; ADR 0067 § 6).
//!
//! `alo-bundle [--programs DIRECTORY] [--into DIRECTORY]` puts `alo`,
//! `alo-render` and `alo-ask` from the programs' directory into a new
//! `alo browser.app` in the other, with the application's two pages and its
//! `Info.plist` ([`alo_window::bundle_made`]); seals it ad hoc with
//! `codesign` and checks the seal ([`alo_window::bundle_seal`]); and prints
//! where it is. Both directories are the one this program is in unless
//! named, so after
//!
//! ```sh
//! cargo build --release
//! target/release/alo-bundle
//! ```
//!
//! the application is `target/release/alo browser.app`. It builds nothing:
//! a program not there is named, with the `cargo` command that makes it.
//! An application already there is refused and left as it is.

use alo_window::bundle_made::{PROGRAMS, make};
use alo_window::bundle_seal::seal;
use std::path::PathBuf;
use std::process::ExitCode;

/// The directories named, or the one this program is in.
fn directories() -> Result<(PathBuf, PathBuf), String> {
    let mut programs = None;
    let mut into = None;
    let mut words = std::env::args_os().skip(1);
    while let Some(word) = words.next() {
        let slot = match word.to_str() {
            Some("--programs") => &mut programs,
            Some("--into") => &mut into,
            _ => {
                return Err(format!(
                    "{} is not an option of this",
                    word.to_string_lossy()
                ));
            }
        };
        let Some(directory) = words.next() else {
            return Err(format!("{} names no directory", word.to_string_lossy()));
        };
        *slot = Some(PathBuf::from(directory));
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
    Ok((programs, into))
}

fn main() -> ExitCode {
    let (programs, into) = match directories() {
        Ok(directories) => directories,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            eprintln!("alo-bundle: alo-bundle [--programs DIRECTORY] [--into DIRECTORY]");
            return ExitCode::from(2);
        }
    };
    let made = match make(&programs, &into) {
        Ok(made) => made,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            return ExitCode::from(2);
        }
    };
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

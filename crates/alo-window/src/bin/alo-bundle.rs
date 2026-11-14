/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-bundle`: make the macOS application, `alo browser.app` (queue items
//! 687 and 720; ADR 0067 § 6, ADR 0077, ADR 0079).
//!
//! `alo-bundle [--programs DIRECTORY] [--into DIRECTORY] [--host PROGRAM]
//! [--keys-from keychain|DIRECTORY] [--services FILE] [--updates DIRECTORY]` puts `alo`, `alo-render` and `alo-ask` from the programs' directory into
//! a new `alo browser.app` in the other, with the application's two pages
//! and its `Info.plist` ([`alo_window::bundle_made`]); carries the Chromium
//! the application draws the web with — alo's CEF host and CEF's framework
//! from the distribution `CEF_PATH` names, checked against its pin first,
//! with CEF's licences ([`alo_window::bundle_carried`]); carries the keys
//! a release searches with, taken from outside the repository
//! ([`alo_window::bundle_keys`], ADR 0091); seals it ad hoc with
//! `codesign`, inside out, and checks the seal
//! ([`alo_window::bundle_seal`]); and prints where it is. Both directories
//! are the one this program is in unless named, so after
//!
//! ```sh
//! cargo build --release
//! CEF_PATH=… cargo build --release --locked \
//!   --manifest-path crates/alo-cef-host/Cargo.toml --target-dir target/alo-cef-host
//! CEF_PATH=… target/release/alo-bundle --keys-from keychain
//! ```
//!
//! the application is `target/release/alo browser.app`. That is how a
//! release is made. The host is
//! `target/alo-cef-host/release/alo-cef-host` there — beside the programs'
//! directory, under `alo-cef-host`, in the directory of the programs' own
//! name — unless `--host` names it.
//!
//! **The services** (ADR 0106 §§ 3, 6): `--services FILE` names the
//! search and AI services the application asks unless a person chooses
//! others in *Settings* — the `search`, `searxng` and `answers` lines of a
//! settings file, and nothing else — checked, and carried as
//! `Contents/Resources/services.txt`. Without it the application asks
//! Brave and `OpenAI`, as every build before. This is how a release made
//! elsewhere chooses its services without changing this repository.
//!
//! **The keys** (ADR 0091, ADR 0106 § 4): `--keys-from keychain` takes the
//! key of each service the application asks — by `--services`, else Brave
//! and `OpenAI`; a `SearXNG` takes none — from this Mac's login keychain, the
//! service's own item first (`alo-search-brave`, `alo-ai-openai`) and an
//! older alo's after (`alo-brave-search`, `alo-openai`), for the person
//! running it; `--keys-from DIRECTORY` from the file in `DIRECTORY` named
//! as its slot (`search`, `answer`). They go in `Contents/Resources/keys`,
//! masked, before the seal; a key missing refuses, and nothing is made.
//! Without `--keys-from` the application carries no key, says so, and asks
//! with a person's own or this Mac's keychain's when it runs, as a
//! development build does. A key is never printed, and never comes from
//! the repository.
//!
//! **Without `CEF_PATH`** (unset or empty) it makes the application with no
//! Chromium, and says so: that application draws its page with this
//! browser's own engine, on the page saying its Chromium is missing (queue
//! item 719). **With `CEF_PATH` naming something that is not the pinned
//! distribution, or no host built**, it refuses with a sentence saying what
//! is wrong, and makes nothing: a Chromium asked for is never quietly left
//! out.
//!
//! **Updates** (ADR 0116 § 3): `--updates DIRECTORY` names where the
//! application's updates come from — `DIRECTORY/address` — and the two
//! public keys they are signed with — `DIRECTORY/keys`, the key in use and
//! the spare — checked, the worthless test keys refused, and carried in
//! `Contents/Resources/update` before the seal
//! ([`alo_window::bundle_update`]). Without it the application does not
//! update itself, and says so in *Settings*.
//!
//! It builds nothing: a program not there is named, with the `cargo`
//! command that makes it. An application already there is refused and left
//! as it is.

use alo_window::bundle_carried::{self, Carried};
use alo_window::bundle_keys::{self, Source, Taken};
use alo_window::bundle_made::{PROGRAMS, make};
use alo_window::bundle_seal::{seal, seal_carried};
use alo_window::bundle_update;
use alo_window::cef_pin;
use alo_window::services::Named as ServicesNamed;
use alo_window::services_made;
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
    /// Where the keys it carries are taken from, if named.
    keys: Option<Source>,
    /// The file naming the services it asks by default, if named.
    services: Option<PathBuf>,
    /// The directory naming where its updates come from, if named.
    updates: Option<PathBuf>,
}

/// The directories and the host named, or where they are by default.
fn named() -> Result<Named, String> {
    let mut programs = None;
    let mut into = None;
    let mut host = None;
    let mut keys = None;
    let mut services = None;
    let mut updates = None;
    let mut words = std::env::args_os().skip(1);
    while let Some(word) = words.next() {
        let slot = match word.to_str() {
            Some("--programs") => &mut programs,
            Some("--into") => &mut into,
            Some("--host") => &mut host,
            Some("--keys-from") => &mut keys,
            Some("--services") => &mut services,
            Some("--updates") => &mut updates,
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
        keys: keys.map(|named: PathBuf| Source::named(&named)),
        services,
        updates,
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

/// The services the application asks by default: none named when
/// `--services` names nothing, or what its file names, checked.
fn services(named: &Named) -> Result<Option<ServicesNamed>, String> {
    let Some(file) = &named.services else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(file)
        .map_err(|why| format!("{} could not be read: {why}", file.display()))?;
    let services = services_made::read_defaults(&text)
        .map_err(|why| format!("{} does not read: {why}", file.display()))?;
    let resolved = services.services();
    eprintln!(
        "alo-bundle: asks {} and {} unless a person chooses others",
        resolved.search.name(),
        resolved.answers.name()
    );
    Ok(Some(services))
}

/// The keys to carry: none when `--keys-from` names nothing, or the key of
/// every service the application asks by default, taken from where it
/// names — said by slot, never by key.
fn keys(named: &Named, services: Option<&ServicesNamed>) -> Result<Option<Taken>, String> {
    let Some(source) = &named.keys else {
        eprintln!(
            "alo-bundle: --keys-from is not named, so the application carries no keys: it \
             searches with this Mac's keychain's when it runs"
        );
        return Ok(None);
    };
    let resolved = services.cloned().unwrap_or_default().services();
    let taken = bundle_keys::taken(source, &services_made::keyed(&resolved))?;
    let from = match source {
        Source::Keychain(_) => "this Mac's keychain".to_owned(),
        Source::Directory(directory) => directory.display().to_string(),
    };
    eprintln!("alo-bundle: carries {}, taken from {from}", taken.said());
    Ok(Some(taken))
}

/// Where updates come from: none when `--updates` names nothing, or its
/// address and keys, checked.
fn updates(named: &Named) -> Result<Option<(String, String)>, String> {
    let Some(directory) = &named.updates else {
        eprintln!("alo-bundle: --updates is not named, so the application does not update itself");
        return Ok(None);
    };
    let taken = bundle_update::taken(directory)?;
    eprintln!(
        "alo-bundle: updates itself from the address and the two keys in {}",
        directory.display()
    );
    Ok(Some(taken))
}

/// Put the services named and where updates come from in `application`,
/// before it is sealed: the services' file, and the update address and
/// keys (ADR 0116 § 3).
fn carry_files(
    application: &std::path::Path,
    services: Option<&ServicesNamed>,
    updates: Option<&(String, String)>,
) -> Result<(), String> {
    if let Some((address, keys)) = updates {
        bundle_update::put_in(application, (address, keys))?;
    }
    if let Some(services) = services {
        let file = application
            .join("Contents")
            .join("Resources")
            .join(services_made::DEFAULTS);
        file.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&file, services_made::text_of(services)))
            .map_err(|why| format!("{} could not be written: {why}", file.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let named = match named() {
        Ok(named) => named,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            eprintln!(
                "alo-bundle: alo-bundle [--programs DIRECTORY] [--into DIRECTORY] [--host PROGRAM] \
                 [--keys-from keychain|DIRECTORY] [--services FILE] [--updates DIRECTORY]"
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
    let services = match services(&named) {
        Ok(services) => services,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            return ExitCode::from(2);
        }
    };
    let keys = match keys(&named, services.as_ref()) {
        Ok(keys) => keys,
        Err(why) => {
            eprintln!("alo-bundle: {why}");
            return ExitCode::from(2);
        }
    };
    let updates = match updates(&named) {
        Ok(updates) => updates,
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
    if let Some(keys) = keys
        && let Err(why) = keys.put_in(&made.application())
    {
        eprintln!("alo-bundle: {why}");
        return ExitCode::FAILURE;
    }
    if let Err(why) = carry_files(&made.application(), services.as_ref(), updates.as_ref()) {
        eprintln!("alo-bundle: {why}");
        return ExitCode::FAILURE;
    }
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

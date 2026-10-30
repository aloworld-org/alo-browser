/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own CEF host, built and laid out for a test that drives it as it
//! drives Chrome (queue item 726, ADR 0079 § 8): the `Chromium::Carried`
//! a Chrome test is run again with.
//!
//! Each test program that drives the host names this module; what it
//! shares is here once, as the host's own two tests had it.
//!
//! The host (`crates/alo-cef-host`) is built against the CEF distribution
//! `CEF_PATH` names — so nothing is downloaded while it is built — and laid
//! out as macOS needs it by `cef_host::assemble`, in a scratch directory of
//! the test's own that goes with the [`Carried`] value. With `CEF_PATH`
//! unset, or not naming an unpacked distribution, [`host`] says so on the
//! error output and answers nothing: the test is **skipped, saying why**,
//! never quietly passed. Run it with `--no-capture` (or `cargo nextest …
//! --success-output immediate`) to see the line.

use alo_window::beside::Chromium;
use alo_window::cef_host::{self, EXECUTABLE, FRAMEWORK};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// How many hosts this test program has laid out, so that two tests of one
/// program run at once never share a directory.
static LAID_OUT: AtomicU32 = AtomicU32::new(0);

/// The host, built and laid out: gone, with its directory, when dropped.
pub struct Carried {
    scratch: PathBuf,
    application: PathBuf,
    program: PathBuf,
}

impl Carried {
    /// The Chromium a Chrome test starts in its place: the host's program,
    /// inside its application.
    pub fn chromium(&self) -> Chromium {
        Chromium::Carried(self.program.clone())
    }
}

impl Drop for Carried {
    fn drop(&mut self) {
        // A helper still ending as the test ends is waited for, so the
        // directory it runs from goes with nothing left in it.
        let until = Instant::now() + Duration::from_secs(10);
        while running_inside(&self.application) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_dir_all(&self.scratch);
    }
}

/// Whether any process is running a program inside `application`.
fn running_inside(application: &Path) -> bool {
    Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(application)
        .output()
        .is_ok_and(|found| !found.stdout.is_empty())
}

/// alo's CEF host built and laid out, for the test of queue item `item` —
/// or nothing, said on the error output, when there is no CEF distribution
/// here to build it against.
///
/// # Errors
///
/// Why it was not built or laid out, when there is a distribution.
pub fn host(item: &str) -> Result<Option<Carried>, String> {
    let cef = match distribution() {
        Ok(cef) => cef,
        Err(why) => {
            eprintln!(
                "skipped: alo's CEF host needs a CEF distribution to be built against, \
                 and {why} (queue item {item})"
            );
            return Ok(None);
        }
    };
    let built = built(&cef)?;
    let scratch = std::env::temp_dir().join(format!(
        "alo-cef-carried-{}-{}",
        std::process::id(),
        LAID_OUT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    let application = scratch.join(format!("{EXECUTABLE}.app"));
    let program = cef_host::assemble(&built, &cef.join(FRAMEWORK), &application)
        .map_err(|why| why.to_string())?;
    Ok(Some(Carried {
        scratch,
        application,
        program,
    }))
}

/// The CEF distribution to build against, or why there is none.
fn distribution() -> Result<PathBuf, String> {
    let Some(named) = std::env::var_os("CEF_PATH") else {
        return Err("CEF_PATH is not set".to_owned());
    };
    let named = PathBuf::from(named);
    if named.join(FRAMEWORK).is_dir() && named.join("archive.json").is_file() {
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

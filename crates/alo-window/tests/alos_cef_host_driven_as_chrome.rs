/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own CEF host, started and driven exactly as Chrome is (queue item
//! 725, ADR 0079 § 3 option H).
//!
//! The host (`crates/alo-cef-host`) is built here against the CEF
//! distribution `CEF_PATH` names — so nothing is downloaded while it is
//! built — and laid out as macOS needs it by `cef_host::assemble`. Then
//! `Chrome::start_from(&Chromium::Carried(…))`, the very call the
//! application makes for the Chromium it carries, starts it through
//! `/bin/sh` with the pipe on descriptors 3 and 4, and it is driven as Chrome
//! is: `Browser.getVersion` answered, a page from a server of this test's
//! own opened in a new target through Chrome's network and read as our
//! tree, and closed over the pipe.
//!
//! **Nothing is written outside the profile alo gave it**, asserted at every
//! place a CEF program was seen to write on this Mac: CEF's own profile
//! under `~/Library/Application Support/CEF` and `~/Library/Caches/CEF`,
//! Chromium's temporary directories, its application clone and its caches
//! under the system's per-person directories (named by the host's
//! identifier), the state macOS keeps for a restored window, and the working
//! directory — but for one empty directory macOS's Metal framework makes
//! for any program that draws with the GPU, which is said and removed
//! ([`is_metals_empty_cache`]). And nothing it started is left running.
//!
//! # When there is no CEF distribution here
//!
//! The host cannot be built without one, so with `CEF_PATH` unset, or not
//! naming an unpacked distribution, this test says so on its error output
//! and returns — **skipped, saying why**, never quietly passing: run it with
//! `--no-capture` (or `cargo nextest … --success-output immediate`) to see
//! the line. Setting `CEF_PATH` to CEF 156.0.14's macOS arm64 *minimal*
//! distribution, unpacked as `cef-dll-sys` expects (an `archive.json` beside
//! the framework), runs it.

#![cfg(target_os = "macos")]

use alo_window::beside::Chromium;
use alo_window::cef_host::{self, EXECUTABLE, FRAMEWORK, IDENTIFIER};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// What the server's page says.
const SAID: &str = "Drawn by the Chromium alo carries";

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

/// A server on this machine that answers every request with one page: its
/// port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                // A connection Chrome opened ahead of need asked for nothing.
                if asked.is_empty() {
                    return;
                }
                let body = format!("<!doctype html><title>{SAID}</title><h1>{SAID}</h1>");
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

/// The system's directory of this person's `name` (`DARWIN_USER_TEMP_DIR`,
/// `DARWIN_USER_CACHE_DIR`): where Chromium on macOS puts what it does not
/// put in its profile, whatever `TMPDIR` says.
fn per_person(name: &str) -> Result<PathBuf, String> {
    let said = Command::new("/usr/bin/getconf")
        .arg(name)
        .output()
        .map_err(|why| why.to_string())?;
    let said = String::from_utf8_lossy(&said.stdout).trim().to_owned();
    if said.is_empty() {
        return Err(format!("getconf {name} said nothing"));
    }
    Ok(PathBuf::from(said))
}

/// Every place outside the profile a CEF program was seen to write on this
/// Mac, with what is there now: each entry's path.
fn written_outside() -> Result<BTreeSet<PathBuf>, String> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("no HOME")?);
    let temporary = per_person("DARWIN_USER_TEMP_DIR")?;
    let clones = temporary
        .parent()
        .ok_or("the temporary directory is the root")?
        .join("X");
    let mut found = BTreeSet::new();
    for directory in [
        temporary,
        clones,
        per_person("DARWIN_USER_CACHE_DIR")?,
        home.join("Library/Saved Application State"),
    ] {
        for entry in std::fs::read_dir(&directory)
            .into_iter()
            .flatten()
            .flatten()
        {
            if entry.file_name().to_string_lossy().starts_with(IDENTIFIER) {
                found.insert(entry.path());
            }
        }
    }
    for cefs in [
        home.join("Library/Application Support/CEF"),
        home.join("Library/Caches/CEF"),
    ] {
        if cefs.exists() {
            found.insert(cefs);
        }
    }
    let working = std::env::current_dir().map_err(|why| why.to_string())?;
    for entry in std::fs::read_dir(&working)
        .map_err(|why| why.to_string())?
        .flatten()
    {
        found.insert(entry.path());
    }
    Ok(found)
}

/// Whether any process is running a program inside `application`.
fn running_inside(application: &Path) -> bool {
    Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(application)
        .output()
        .is_ok_and(|found| !found.stdout.is_empty())
}

#[test]
fn alos_cef_host_is_started_and_driven_as_chrome_and_writes_only_its_profile() {
    let cef = match distribution() {
        Ok(cef) => cef,
        Err(why) => {
            eprintln!(
                "skipped: alo's CEF host needs a CEF distribution to be built against, \
                 and {why} (queue item 725)"
            );
            return;
        }
    };
    let program = built(&cef).unwrap();
    let scratch = std::env::temp_dir().join(format!("alo-cef-host-test-{}", std::process::id()));
    let application = scratch.join(format!("{EXECUTABLE}.app"));
    let carried = cef_host::assemble(&program, &cef.join(FRAMEWORK), &application).unwrap();
    let plist = std::fs::read_to_string(application.join("Contents/Info.plist")).unwrap();
    assert!(plist.contains("<key>CFBundleName</key>\n\t<string>alo</string>"));

    let before = written_outside().unwrap();
    let port = serve().unwrap();
    let profile;
    {
        let mut chrome = Chrome::start_from(&Chromium::Carried(carried.clone()), false).unwrap();
        profile = chrome.profile().to_path_buf();
        let version = chrome.version().unwrap();
        assert!(version.product.starts_with("Chrome/156."), "{version:?}");
        // Started as `chrome.rs` starts Chrome: headless, and the program
        // the process table names is the host inside the application.
        assert!(version.user_agent.contains("HeadlessChrome"), "{version:?}");
        let started = Command::new("/bin/ps")
            .args(["-o", "comm=", "-p", &chrome.process().to_string()])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&started.stdout).trim(),
            carried.to_string_lossy()
        );

        let address = alo_url::parse(&format!("http://127.0.0.1:{port}/page.html")).unwrap();
        let mut tab = Tab::open(&mut chrome, &address, SIZE).unwrap();
        let snapshot = tab.read().unwrap();
        assert!(
            snapshot
                .nodes()
                .iter()
                .any(|node| node.to_string().contains(SAID)),
            "{snapshot:?}"
        );
        assert_eq!(tab.refusals(), &[], "nothing here breaks a rule");
        // Where Chromium put its temporary files while it ran: the profile.
        assert!(profile.join("tmp").is_dir(), "{}", profile.display());
    }
    // Closed over the pipe: the profile is gone with it, nothing it started
    // is still running, and nothing was written anywhere else.
    assert!(!profile.exists(), "{}", profile.display());
    let until = Instant::now() + Duration::from_secs(10);
    while running_inside(&application) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!running_inside(&application), "a helper outlived the host");
    let after = written_outside().unwrap();
    let (metal, new): (Vec<_>, Vec<_>) = after
        .difference(&before)
        .partition(|path| is_metals_empty_cache(path));
    assert!(new.is_empty(), "written outside the profile: {new:?}");
    for made in metal {
        std::fs::remove_dir_all(made).unwrap();
    }
    std::fs::remove_dir_all(&scratch).unwrap();
}

/// Whether `path` is the one thing found outside the profile that is not
/// Chromium's but macOS's own: the per-person cache directory named by the
/// host's identifier, holding nothing but the empty directories macOS's
/// Metal framework makes there for a program that draws with the GPU
/// (`com.apple.metalfe`, measured on this Mac with CEF 156.0.14 headless).
/// Not a file is in it; one would fail the test. Whether alo keeps even
/// that is queue item 729.
fn is_metals_empty_cache(path: &Path) -> bool {
    let Ok(caches) = per_person("DARWIN_USER_CACHE_DIR") else {
        return false;
    };
    let Ok(held) = std::fs::read_dir(path) else {
        return false;
    };
    path == caches.join(IDENTIFIER)
        && held.flatten().all(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("com.apple.metal")
                && std::fs::read_dir(entry.path()).is_ok_and(|mut held| held.next().is_none())
        })
}

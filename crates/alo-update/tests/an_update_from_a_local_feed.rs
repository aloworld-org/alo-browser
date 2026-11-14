/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The whole road of an update against a feed of this test's own on
//! `127.0.0.1` (ADR 0116 § 6), signed with the worthless test keys: the
//! check, the download, the unpacking beside a stand-in `alo browser.app`
//! in a temporary folder, and the swap at quit.
//!
//! Every refusal ADR 0116 lists is made here with the installed copy
//! checked **byte for byte** after it: a manifest signed by another key or
//! with a byte changed, a package a byte changed, short or long, a serial
//! replayed, an expired manifest, an older or equal version, another
//! platform, an installed version below the minimum. A download cut half
//! way leaves a `.partial` and resumes with a `Range`; a server that
//! answers a range with the whole file is handled too; a swap refused by a
//! folder that cannot be written leaves the old application in place.
//!
//! Nothing reaches the network, the keychain or the person's own alo: the
//! applications are folders in `$TMPDIR`, never `/Applications`, and the
//! updater's own folder is a temporary one.

mod worthless_test_key;

use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Trust};
use alo_update::address::Address;
use alo_update::kept::{Kept, Serial};
use alo_update::quitting::{self, Installed};
use alo_update::signature::{Key, Keys};
use alo_update::{Found, Instant, Updater, Version, stage, test_key};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use worthless_test_key::{IN_USE, SPARE, STRANGER, TRUSTED};

/// The moment every check here is made: an hour after the manifest's
/// `published`.
const NOW: Instant = Instant::at(1_794_650_400);

/// The piece size the tests download in, so a small package is several.
const PIECE: u64 = 16 * 1024;

/// How the feed's package is served.
#[derive(Debug, Clone, Default)]
struct Serving {
    /// The manifest's bytes.
    manifest: Vec<u8>,
    /// Its signature's.
    signature: Vec<u8>,
    /// The package's.
    package: Vec<u8>,
    /// Close the connection after this many bytes of the package's body,
    /// on the package request of this place, counting from zero.
    cut: Option<(usize, usize)>,
    /// How many package requests have been answered.
    asked: usize,
    /// Answer every range with the whole package, as `200`.
    ignores_ranges: bool,
}

/// The feed: what it serves, and the head of every request it was sent.
#[derive(Clone)]
struct Feed {
    serving: Arc<Mutex<Serving>>,
    heads: Arc<Mutex<Vec<String>>>,
    port: u16,
}

impl Feed {
    /// A feed on `127.0.0.1` serving `serving`.
    fn start(serving: Serving) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
        let port = listener.local_addr().map_err(|why| why.to_string())?.port();
        let feed = Feed {
            serving: Arc::new(Mutex::new(serving)),
            heads: Arc::new(Mutex::new(Vec::new())),
            port,
        };
        let serving = feed.clone();
        std::thread::spawn(move || {
            for socket in listener.incoming() {
                let Ok(socket) = socket else { return };
                serving.answer(socket);
            }
        });
        Ok(feed)
    }

    /// Answer one connection's one request, and close it.
    fn answer(&self, mut socket: TcpStream) {
        let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
        let Some(head) = head_of(&mut socket) else {
            return;
        };
        if let Ok(mut heads) = self.heads.lock() {
            heads.push(head.clone());
        }
        let path = head
            .lines()
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .unwrap_or("")
            .to_owned();
        let Ok(mut serving) = self.serving.lock() else {
            return;
        };
        let (status, extra, body): (&str, String, Vec<u8>) = match path.as_str() {
            "/feed/macos-arm64/manifest" => ("200 OK", String::new(), serving.manifest.clone()),
            "/feed/macos-arm64/manifest.minisig" => {
                ("200 OK", String::new(), serving.signature.clone())
            }
            "/package/alo.zip" => {
                let whole = serving.package.clone();
                match range_of(&head) {
                    Some((first, last)) if !serving.ignores_ranges => {
                        let last = last.min(whole.len().saturating_sub(1));
                        let piece = whole.get(first..=last).unwrap_or_default().to_vec();
                        (
                            "206 Partial Content",
                            format!("Content-Range: bytes {first}-{last}/{}\r\n", whole.len()),
                            piece,
                        )
                    }
                    _ => ("200 OK", String::new(), whole),
                }
            }
            _ => ("404 Not Found", String::new(), b"none".to_vec()),
        };
        let cut = if path == "/package/alo.zip" {
            let place = serving.asked;
            serving.asked += 1;
            serving
                .cut
                .filter(|(at, _)| *at == place)
                .map(|(_, bytes)| bytes)
        } else {
            None
        };
        drop(serving);
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{extra}\
             Connection: close\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(head.as_bytes());
        let sent = cut.map_or(body.as_slice(), |cut| body.get(..cut).unwrap_or_default());
        let _ = socket.write_all(sent);
        let _ = socket.flush();
    }

    /// Every request's head so far.
    fn heads(&self) -> Vec<String> {
        self.heads
            .lock()
            .map(|heads| heads.clone())
            .unwrap_or_default()
    }

    /// The feed's address, as a build would be given it.
    fn address(&self) -> Result<Address, String> {
        Address::read(&format!("http://127.0.0.1:{}/feed", self.port))
    }

    /// Where its package is.
    fn package(&self) -> String {
        format!("http://127.0.0.1:{}/package/alo.zip", self.port)
    }
}

/// A request's head, read off `socket`.
fn head_of(socket: &mut TcpStream) -> Option<String> {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    while !seen.ends_with(b"\r\n\r\n") {
        match socket.read(&mut byte) {
            Ok(0) | Err(_) => return None,
            Ok(_) => seen.extend_from_slice(&byte),
        }
        if seen.len() > 16 * 1024 {
            return None;
        }
    }
    Some(String::from_utf8_lossy(&seen).into_owned())
}

/// The `Range: bytes=FIRST-LAST` a head asks, if any.
fn range_of(head: &str) -> Option<(usize, usize)> {
    let line = head
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("range:"))?;
    let (first, last) = line.split_once('=')?.1.trim().split_once('-')?;
    Some((first.parse().ok()?, last.parse().ok()?))
}

/// A stand-in `alo browser.app` of `version` at `at`, holding `which` and
/// `filler` bytes of a resource, sealed ad hoc as alo's own are.
fn stand_in(at: &Path, version: &str, which: &str, filler: usize) -> Result<(), String> {
    let resources = at.join("Contents").join("Resources");
    std::fs::create_dir_all(&resources).map_err(|why| why.to_string())?;
    std::fs::write(
        at.join("Contents").join("Info.plist"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \
             \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\"><dict>\n\
             <key>CFBundleIdentifier</key><string>org.aloworld.Browser.update-test</string>\n\
             <key>CFBundleName</key><string>alo browser</string>\n\
             <key>CFBundlePackageType</key><string>APPL</string>\n\
             <key>CFBundleShortVersionString</key><string>{version}</string>\n\
             <key>CFBundleVersion</key><string>{version}</string>\n\
             </dict></plist>\n"
        ),
    )
    .map_err(|why| why.to_string())?;
    std::fs::write(resources.join("which"), which).map_err(|why| why.to_string())?;
    // Bytes that do not compress, so the package is several pieces.
    let mut state: u32 = 0x9e37_79b9;
    let filling: Vec<u8> = (0..filler)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state.to_le_bytes().first().copied().unwrap_or(0)
        })
        .collect();
    std::fs::write(resources.join("filler"), filling).map_err(|why| why.to_string())?;
    let sealed = Command::new("/usr/bin/codesign")
        .args(["--force", "--sign", "-"])
        .arg(at)
        .output()
        .map_err(|why| why.to_string())?;
    if !sealed.status.success() {
        return Err(String::from_utf8_lossy(&sealed.stderr).into_owned());
    }
    Ok(())
}

/// `application` of `version`, zipped as a release's package is.
fn package_of(scratch: &Path, version: &str) -> Result<Vec<u8>, String> {
    let folder = scratch.join(format!("made-{version}"));
    let _ = std::fs::remove_dir_all(&folder);
    let app = folder.join("alo browser.app");
    stand_in(&app, version, &format!("alo {version}"), 40_000)?;
    let zip = folder.join("package.zip");
    let made = Command::new("/usr/bin/ditto")
        .args(["-c", "-k", "--keepParent"])
        .arg(&app)
        .arg(&zip)
        .output()
        .map_err(|why| why.to_string())?;
    if !made.status.success() {
        return Err(String::from_utf8_lossy(&made.stderr).into_owned());
    }
    std::fs::read(&zip).map_err(|why| why.to_string())
}

/// The manifest for `package` at `feed`, with `changes` made to its lines.
fn manifest_for(feed: &Feed, package: &[u8], changes: &[(&str, &str)]) -> String {
    let sha = sha256_hex(package);
    let mut text = format!(
        "alo-update 1\nplatform macos-arm64\nversion 0.4.2\nserial 118\n\
         published 2026-11-14T09:00:00Z\nexpires 2026-11-28T09:00:00Z\nminimum 0.3.0\n\
         chromium 156.0.8078.96\npackage {}\nsize {}\nsha256 {sha}\nsecurity yes\nbehind no\n\
         notes Chromium 156.0.8078.96, with Chrome\u{2019}s security fixes.\n",
        feed.package(),
        package.len()
    );
    for (from, to) in changes {
        text = text.replacen(from, to, 1);
    }
    text
}

/// SHA-256 of `bytes` as hexadecimal, through `shasum`, macOS's own — a
/// second implementation beside the one `alo-net` rents.
fn sha256_hex(bytes: &[u8]) -> String {
    let child = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        return String::new();
    };
    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(bytes);
    }
    child
        .wait_with_output()
        .ok()
        .and_then(|out| {
            String::from_utf8(out.stdout)
                .ok()?
                .split_whitespace()
                .next()
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// Every file under `folder`, by path, with its bytes: the installed copy,
/// to be compared byte for byte.
fn every_byte(folder: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut waiting = vec![folder.to_path_buf()];
    while let Some(next) = waiting.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                waiting.push(path);
            } else {
                let relative = path.strip_prefix(folder).unwrap_or(&path).to_path_buf();
                found.insert(relative, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    found
}

/// One test's world: a folder of its own with an installed alo 0.4.1, the
/// updater's folder, and an updater for the feed.
struct World {
    root: PathBuf,
    application: PathBuf,
    updater: Updater,
}

impl World {
    fn new(name: &str, feed: &Feed, installed: &str) -> Result<Self, String> {
        let root =
            std::env::temp_dir().join(format!("alo-update-feed-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let application = root.join("Applications").join("alo browser.app");
        stand_in(&application, installed, "alo installed", 1000)?;
        let updater = Updater {
            address: feed.address()?,
            keys: Keys {
                in_use: Key::read(test_key::PUBLIC)?,
                spare: Key::read(test_key::SPARE)?,
            },
            kept: Kept::at(root.join("Updates")),
            application: application.clone(),
            version: Version::read(installed).ok_or("a version")?,
            platform: "macos-arm64".to_owned(),
            piece: PIECE,
        };
        Ok(Self {
            root,
            application,
            updater,
        })
    }

    fn check(&self, pool: &mut Pool, now: Instant) -> Found {
        self.updater.check(pool, &a_person(), now)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = Command::new("/bin/chmod")
            .args(["-R", "u+w"])
            .arg(&self.root)
            .output();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The cause of every request: the person, who left the switch on.
fn a_person() -> Cause {
    Cause::Person {
        tab: Identities::default().a_tab(),
    }
}

fn pool() -> Pool {
    Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| no_trust()))
        .patient_for(Duration::from_secs(5))
}

fn no_trust() -> Trust {
    Trust::of(&[]).unwrap_or_else(|_| no_trust())
}

/// A feed serving alo 0.4.2's package, its manifest changed by `changes`
/// and signed by `key`.
fn feed_of(
    scratch: &str,
    changes: &[(&str, &str)],
    key: worthless_test_key::TestKey,
) -> Result<(Feed, Vec<u8>), String> {
    let made =
        std::env::temp_dir().join(format!("alo-update-made-{}-{scratch}", std::process::id()));
    let package = package_of(&made, "0.4.2")?;
    let _ = std::fs::remove_dir_all(&made);
    let feed = Feed::start(Serving {
        package: package.clone(),
        ..Serving::default()
    })?;
    let manifest = manifest_for(&feed, &package, changes);
    if let Ok(mut serving) = feed.serving.lock() {
        serving.signature = key.sign(manifest.as_bytes()).into_bytes();
        serving.manifest = manifest.into_bytes();
    }
    Ok((feed, package))
}

#[test]
fn the_whole_road_checks_downloads_unpacks_and_swaps_in_at_quit() {
    assert_eq!(IN_USE.public(), test_key::PUBLIC);
    let (feed, _) = feed_of("road", &[], IN_USE).expect("a feed");
    let world = World::new("road", &feed, "0.4.1").expect("a world");
    let before = every_byte(&world.application);
    let mut pool = pool();
    let found = world.check(&mut pool, NOW);
    assert_eq!(
        found,
        Found::Ready {
            version: Version::of(0, 4, 2),
            chromium: "156.0.8078.96".into(),
            security: true,
            behind: false,
        },
        "{}",
        found.said()
    );
    eprintln!("found: {}", found.said());
    // Nothing of the installed application touched, byte for byte.
    assert_eq!(every_byte(&world.application), before);
    let staged = stage::ready_beside(&world.application).expect("staged");
    assert_eq!(staged.version, Version::of(0, 4, 2));
    assert!(
        staged
            .bundle
            .starts_with(world.root.join("Applications").join(".alo-update"))
    );
    // The serial believed, the check kept, the package's zip gone.
    assert_eq!(world.updater.kept.serial(), Serial::Highest(118));
    assert_eq!(world.updater.kept.believed(), Some(NOW));
    assert_eq!(world.updater.kept.checked(), Some((NOW, found.said())));
    assert!(!world.root.join("Updates").join("0.4.2.zip").exists());
    // What was sent: the two GETs, then the package in ranges — each
    // saying `alo-update` and no version, carrying no cookie and no query.
    let heads = feed.heads();
    let lines: Vec<&str> = heads
        .iter()
        .filter_map(|head| head.lines().next())
        .collect();
    eprintln!("sent: {lines:?}");
    assert_eq!(
        lines.get(..2),
        Some(
            &[
                "GET /feed/macos-arm64/manifest HTTP/1.1",
                "GET /feed/macos-arm64/manifest.minisig HTTP/1.1"
            ][..]
        )
    );
    assert!(
        lines
            .iter()
            .skip(2)
            .all(|line| *line == "GET /package/alo.zip HTTP/1.1")
    );
    assert!(lines.len() > 3, "the package came in more than one piece");
    for head in &heads {
        let lower = head.to_ascii_lowercase();
        assert!(lower.contains("\r\nuser-agent: alo-update\r\n"), "{head}");
        assert!(!lower.contains("cookie"), "{head}");
        assert!(!head.contains('?'), "{head}");
    }
    // At quit: swapped, the new one at the path and the old one hidden.
    assert_eq!(
        quitting::install(&world.application, Version::of(0, 4, 1)),
        Installed::Swapped(Version::of(0, 4, 2))
    );
    let which = |at: &Path| std::fs::read_to_string(at.join("Contents/Resources/which")).ok();
    assert_eq!(which(&world.application).as_deref(), Some("alo 0.4.2"));
    assert_eq!(which(&staged.bundle).as_deref(), Some("alo installed"));
    assert_eq!(
        stage::ready_beside(&world.application),
        None,
        "never swapped back"
    );
    // The next launch removes the old one.
    assert!(stage::sweep(&world.application));
    assert!(!world.root.join("Applications").join(".alo-update").exists());
    // And a check now finds the new one up to date — two GETs, nothing more.
    let mut now_new = world.updater.clone();
    now_new.version = Version::of(0, 4, 2);
    let sent = feed.heads().len();
    assert_eq!(
        now_new.check(&mut pool, &a_person(), NOW),
        Found::UpToDate {
            version: Version::of(0, 4, 2),
            behind: false
        }
    );
    assert_eq!(feed.heads().len(), sent + 2);
}

/// One refusal ADR 0116 § 6 lists.
struct Case {
    name: &'static str,
    /// Changes to the manifest's lines, before it is signed.
    changes: &'static [(&'static str, &'static str)],
    key: worthless_test_key::TestKey,
    installed: &'static str,
    /// Tampering with what is served, after it is signed.
    served: fn(&mut Serving),
    /// The serial already believed.
    highest: Option<u64>,
    now: Instant,
    /// What the person is told, in part.
    said: &'static str,
}

fn nothing(_: &mut Serving) {}

fn manifest_byte(serving: &mut Serving) {
    if let Some(byte) = serving.manifest.iter_mut().find(|byte| **byte == b'8') {
        *byte = b'9';
    }
}

fn package_byte(serving: &mut Serving) {
    if let Some(byte) = serving.package.get_mut(100) {
        *byte ^= 1;
    }
}

fn package_short(serving: &mut Serving) {
    serving.package.pop();
}

fn package_long(serving: &mut Serving) {
    serving.package.push(0);
}

fn long_and_whole(serving: &mut Serving) {
    serving.package.push(0);
    serving.ignores_ranges = true;
}

/// A case signed by the key in use, of 0.4.1, nothing believed, now.
const fn case(name: &'static str, served: fn(&mut Serving), said: &'static str) -> Case {
    Case {
        name,
        changes: &[],
        key: IN_USE,
        installed: "0.4.1",
        served,
        highest: None,
        now: NOW,
        said,
    }
}

/// The same, installed as `installed`.
const fn installed(name: &'static str, installed: &'static str, said: &'static str) -> Case {
    Case {
        installed,
        ..case(name, nothing, said)
    }
}

fn refusals() -> [Case; 12] {
    let not_ours = "it is not signed by alo\u{2019}s key";
    let size = "not the size the manifest says";
    [
        Case {
            key: STRANGER,
            ..case("stranger", nothing, not_ours)
        },
        case("manifest-byte", manifest_byte, not_ours),
        case("package-byte", package_byte, "SHA-256 is not the manifest"),
        case("package-short", package_short, size),
        case("package-long", package_long, size),
        case("package-long-whole", long_and_whole, size),
        Case {
            highest: Some(119),
            ..case(
                "replayed",
                nothing,
                "an older release (118, after 119), sent again",
            )
        },
        Case {
            now: Instant::at(1_795_856_400),
            ..case(
                "expired",
                nothing,
                "alo could not check for updates: the update\u{2019}s manifest expired",
            )
        },
        installed("older", "0.5.0", "alo 0.4.2 is older than alo 0.5.0"),
        installed("equal", "0.4.2", "alo 0.4.2 is up to date."),
        Case {
            changes: &[("platform macos-arm64", "platform linux-x64")],
            ..case(
                "platform",
                nothing,
                "it is for \"linux-x64\", not this computer",
            )
        },
        installed(
            "minimum",
            "0.2.9",
            "Download alo again from where you got it",
        ),
    ]
}

#[test]
fn every_refusal_leaves_the_installed_copy_as_it_was() {
    for case in refusals() {
        let (feed, _) = feed_of(case.name, case.changes, case.key).expect("a feed");
        if let Ok(mut serving) = feed.serving.lock() {
            (case.served)(&mut serving);
        }
        let world = World::new(case.name, &feed, case.installed).expect("a world");
        if let Some(highest) = case.highest {
            world
                .updater
                .kept
                .believe(highest, Instant::at(0))
                .expect("kept");
        }
        let before = every_byte(&world.application);
        let found = world.check(&mut pool(), case.now);
        eprintln!("{}: {}", case.name, found.said());
        assert!(
            found.said().contains(case.said),
            "{}: {}",
            case.name,
            found.said()
        );
        assert!(
            !matches!(found, Found::Ready { .. }),
            "{}: {found:?}",
            case.name
        );
        assert_eq!(every_byte(&world.application), before, "{}", case.name);
        assert_eq!(
            stage::ready_beside(&world.application),
            None,
            "{}",
            case.name
        );
        assert_eq!(
            quitting::install(&world.application, world.updater.version),
            Installed::Nothing,
            "{}",
            case.name
        );
        assert_eq!(every_byte(&world.application), before, "{}", case.name);
        let partial = world.root.join("Updates").join("0.4.2.partial");
        assert!(
            !partial.exists(),
            "{}: a refused package is removed",
            case.name
        );
    }
}

#[test]
fn a_download_cut_half_way_keeps_its_partial_and_resumes_with_a_range() {
    let (feed, package) = feed_of("cut", &[], IN_USE).expect("a feed");
    let piece = usize::try_from(PIECE).unwrap_or(0);
    assert!(package.len() > 2 * piece, "{} bytes", package.len());
    // The second piece's connection closed half way through it.
    if let Ok(mut serving) = feed.serving.lock() {
        serving.cut = Some((1, piece / 2));
    }
    let world = World::new("cut", &feed, "0.4.1").expect("a world");
    let before = every_byte(&world.application);
    let mut pool = pool();
    let found = world.check(&mut pool, NOW);
    eprintln!("cut: {}", found.said());
    assert!(matches!(found, Found::Coming(_)), "{found:?}");
    let partial = world.root.join("Updates").join("0.4.2.partial");
    assert_eq!(
        std::fs::metadata(&partial).map(|meta| meta.len()).ok(),
        Some(PIECE),
        "the first piece kept, the cut one not"
    );
    assert_eq!(every_byte(&world.application), before);
    assert_eq!(stage::ready_beside(&world.application), None);
    // The next try resumes from there, with a range, and finishes.
    let sent = feed.heads().len();
    let found = world.check(&mut pool, NOW);
    assert!(matches!(found, Found::Ready { .. }), "{}", found.said());
    let resumed = feed.heads().get(sent + 2).cloned().unwrap_or_default();
    eprintln!(
        "resumed with: {:?}",
        resumed.lines().find(|line| line.starts_with("Range"))
    );
    assert!(
        resumed.contains(&format!("\r\nRange: bytes={PIECE}-{}\r\n", 2 * PIECE - 1)),
        "{resumed}"
    );
    assert!(!partial.exists());
    assert_eq!(every_byte(&world.application), before);
}

#[test]
fn a_server_that_answers_a_range_with_the_whole_file_is_handled() {
    let (feed, _) = feed_of("whole", &[], IN_USE).expect("a feed");
    let piece = usize::try_from(PIECE).unwrap_or(0);
    let world = World::new("whole", &feed, "0.4.1").expect("a world");
    // A piece kept first, then a server that ignores ranges altogether:
    // its whole file replaces what was held, never appended to it.
    if let Ok(mut serving) = feed.serving.lock() {
        serving.cut = Some((1, piece / 2));
    }
    let mut pool = pool();
    assert!(matches!(world.check(&mut pool, NOW), Found::Coming(_)));
    if let Ok(mut serving) = feed.serving.lock() {
        serving.ignores_ranges = true;
    }
    let found = world.check(&mut pool, NOW);
    assert!(matches!(found, Found::Ready { .. }), "{}", found.said());
    // Signed by the spare, the same.
    let (spare_feed, package) = feed_of("spare", &[], SPARE).expect("a feed");
    let manifest = manifest_for(&spare_feed, &package, &[]);
    if let Ok(mut serving) = spare_feed.serving.lock() {
        serving.signature = SPARE.sign_saying(manifest.as_bytes(), TRUSTED).into_bytes();
    }
    let spare_world = World::new("spare", &spare_feed, "0.4.1").expect("a world");
    let found = spare_world.check(&mut pool, NOW);
    assert!(matches!(found, Found::Ready { .. }), "{}", found.said());
}

#[test]
fn a_swap_refused_by_a_folder_that_cannot_be_written_leaves_the_old_one_and_says_why() {
    let (feed, _) = feed_of("readonly", &[], IN_USE).expect("a feed");
    let world = World::new("readonly", &feed, "0.4.1").expect("a world");
    assert!(matches!(world.check(&mut pool(), NOW), Found::Ready { .. }));
    let before = every_byte(&world.application);
    let folder = world.root.join("Applications");
    let locked = Command::new("/bin/chmod")
        .arg("a-w")
        .arg(&folder)
        .output()
        .map(|out| out.status.success());
    assert_eq!(locked.ok(), Some(true));
    let installed = quitting::install(&world.application, Version::of(0, 4, 1));
    eprintln!("refused: {installed:?}");
    assert!(
        matches!(&installed, Installed::Refused(why) if why.contains("alo 0.4.1 is as it was")),
        "{installed:?}"
    );
    assert_eq!(every_byte(&world.application), before);
    // Still ready, for the next quit, once the folder can be written.
    let unlocked = Command::new("/bin/chmod")
        .arg("u+w")
        .arg(&folder)
        .output()
        .map(|out| out.status.success());
    assert_eq!(unlocked.ok(), Some(true));
    assert_eq!(
        quitting::install(&world.application, Version::of(0, 4, 1)),
        Installed::Swapped(Version::of(0, 4, 2))
    );
}

#[test]
fn a_folder_that_cannot_be_written_is_not_updated_and_the_person_is_told_what_to_do() {
    let (feed, _) = feed_of("unwritable", &[], IN_USE).expect("a feed");
    let world = World::new("unwritable", &feed, "0.4.1").expect("a world");
    let folder = world.root.join("Applications");
    let before = every_byte(&world.application);
    let locked = Command::new("/bin/chmod")
        .arg("a-w")
        .arg(&folder)
        .output()
        .map(|out| out.status.success());
    assert_eq!(locked.ok(), Some(true));
    let found = world.check(&mut pool(), NOW);
    eprintln!("unwritable: {}", found.said());
    assert!(
        found.said().contains(
            "Move alo to a folder you can write, or ask the computer\u{2019}s administrator."
        ),
        "{}",
        found.said()
    );
    assert_eq!(every_byte(&world.application), before);
}

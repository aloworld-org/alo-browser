/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The sites a person switched blocking off for (queue item 807, ADR 0094
//! § 6 and its amendment of 6 November 2026).
//!
//! A card's shield is a switch: pressed, the lists alo carries are no
//! longer asked of any request a page **of that site** makes — its
//! registrable domain, as every per-site decision here is made
//! ([`alo_url::site::of`]) — and no hiding sheet is put into its documents;
//! pressed again, they are. The record still writes each request let go.
//!
//! # Held by the browser process, asked before the lists
//!
//! [`Off`] is a value the browser process holds — one for the process,
//! [`process`] — and the thread holding a card's Chromium asks
//! ([`Off::is_off`]) in [`crate::chrome_tab`] before the lists, as ADR 0094
//! § 6 designed it for 705. The window sets it; no page, renderer or agent
//! can.
//!
//! # Kept on this machine, and nowhere else
//!
//! In a file of words, [`FILE`], beside the chats in alo's own directory
//! (ADR 0092 § 1) — so `--chats DIR` keeps a hand run's switches apart from
//! a person's too — one site a line, readable by this user alone, written
//! whole or not at all as a chat is. A line that is not a site is passed
//! over; nothing in it leaves the machine.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

/// The switches of this process: what the window sets and every card's
/// tab asks. Empty until the window reads them.
pub fn process() -> &'static Off {
    static PROCESS: std::sync::OnceLock<Off> = std::sync::OnceLock::new();
    PROCESS.get_or_init(Off::default)
}

/// The file the switches are kept in, in alo's directory.
pub const FILE: &str = "sites-without-blocking.txt";

/// What the file says first.
const HEADER: &str = "# alo: the sites trackers and adverts are not blocked on (ADR 0094)";

/// The site a page at `address` is of, by which blocking is switched: its
/// registrable domain, or its host where it has none; [`None`] for an
/// address with no host.
pub fn site_of(address: &str) -> Option<String> {
    let url = alo_url::parse(address).ok()?;
    url.host.as_ref().map(alo_url::site::of)
}

/// The sites blocking is off for, shared between the window and the
/// threads that judge a card's requests.
#[derive(Debug, Clone, Default)]
pub struct Off {
    sites: Arc<RwLock<BTreeSet<String>>>,
}

impl PartialEq for Off {
    /// Two are alike when they hold the same sites now.
    fn eq(&self, other: &Self) -> bool {
        self.sites() == other.sites()
    }
}

impl Off {
    /// Off for these `sites`.
    pub fn of(sites: impl IntoIterator<Item = String>) -> Self {
        Self {
            sites: Arc::new(RwLock::new(sites.into_iter().collect())),
        }
    }

    /// Whether blocking is off for the page at `address`'s site.
    pub fn is_off(&self, address: &str) -> bool {
        site_of(address).is_some_and(|site| {
            self.sites
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&site)
        })
    }

    /// Whether blocking is off for `site`.
    pub fn is_off_for(&self, site: &str) -> bool {
        self.sites
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(site)
    }

    /// Switch blocking `off` for `site`, or on again.
    pub fn switch(&self, site: &str, off: bool) {
        let mut sites = self.sites.write().unwrap_or_else(PoisonError::into_inner);
        if off {
            sites.insert(site.to_owned());
        } else {
            sites.remove(site);
        }
    }

    /// Every site blocking is off for, in order.
    pub fn sites(&self) -> Vec<String> {
        self.sites
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }
}

/// The file of switches in `directory`.
pub fn file_in(directory: &Path) -> PathBuf {
    directory.join(FILE)
}

/// The sites kept in `file`: none when there is no file.
///
/// # Errors
///
/// Why it could not be read, in words.
pub fn read(file: &Path) -> Result<Vec<String>, String> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter(|line| site_of(&format!("https://{line}/")).as_deref() == Some(*line))
            .map(str::to_owned)
            .collect()),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(why) => Err(format!("{} could not be read: {why}", file.display())),
    }
}

/// Keep `off`'s sites in `file`, whole or not at all, readable by this user
/// alone.
///
/// # Errors
///
/// Why it was not written, in words; the file is then as it was.
pub fn write(file: &Path, off: &Off) -> Result<(), String> {
    let failed = |why: std::io::Error| format!("{} could not be written: {why}", file.display());
    let mut writing = file.as_os_str().to_owned();
    writing.push(".writing");
    let writing = PathBuf::from(writing);
    let text: String = core::iter::once(HEADER.to_owned())
        .chain(off.sites())
        .flat_map(|line| [line, "\n".to_owned()])
        .collect();
    let mut out = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&writing)
        .map_err(failed)?;
    out.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(failed)?;
    out.write_all(text.as_bytes()).map_err(failed)?;
    out.sync_all().map_err(failed)?;
    drop(out);
    fs::rename(&writing, file).map_err(failed)?;
    if let Some(directory) = file.parent() {
        File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(failed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_is_its_registrable_domain_or_its_host() {
        assert_eq!(
            site_of("https://www.theguardian.com/international").as_deref(),
            Some("theguardian.com")
        );
        assert_eq!(
            site_of("https://news.bbc.co.uk/x").as_deref(),
            Some("bbc.co.uk")
        );
        assert_eq!(
            site_of("http://127.0.0.1:8080/").as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(site_of("about:blank"), None);
    }

    #[test]
    fn switched_off_for_a_site_is_off_for_every_page_of_it_and_on_again() {
        let off = Off::default();
        assert!(!off.is_off("https://www.theguardian.com/"));
        off.switch("theguardian.com", true);
        let shared = off.clone();
        assert!(shared.is_off("https://www.theguardian.com/international"));
        assert!(shared.is_off("https://theguardian.com/"));
        assert!(!shared.is_off("https://www.bbc.com/news"));
        assert!(shared.is_off_for("theguardian.com"));
        off.switch("theguardian.com", false);
        assert!(!shared.is_off("https://www.theguardian.com/"));
    }

    #[test]
    fn the_switches_are_kept_in_words_only_this_users_and_read_back() {
        let directory =
            std::env::temp_dir().join(format!("alo-blocking-off-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let file = file_in(&directory);
        assert_eq!(read(&file), Ok(Vec::new()), "no file: nothing off");
        let off = Off::of(["theguardian.com".to_owned(), "127.0.0.1".to_owned()]);
        write(&file, &off).unwrap();
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.starts_with(HEADER), "{text}");
        assert_eq!(
            read(&file),
            Ok(vec!["127.0.0.1".to_owned(), "theguardian.com".to_owned()])
        );
        // A line that is not a site is passed over.
        fs::write(&file, "# x\n\nwww.bbc.com\nnot a site\nbbc.com\n").unwrap();
        assert_eq!(read(&file), Ok(vec!["bbc.com".to_owned()]));
        let _ = fs::remove_dir_all(&directory);
    }
}

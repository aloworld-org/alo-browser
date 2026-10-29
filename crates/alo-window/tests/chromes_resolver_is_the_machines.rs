/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome asks the machine's resolver and never upgrades it to encrypted DNS
//! on its own (queue item 635, ADR 0008, ADR 0060 § 4).
//!
//! What Chrome resolves through is read from Chrome itself: its net log, which
//! writes the secure DNS mode its configuration is in and the mode each lookup
//! is asked in, as `"secure_dns_mode":` and a number — 0 off, 1 automatic, 2
//! secure. The installed Chrome is started as the browser starts it, through a
//! program standing where Chrome would that adds the net log's two flags and
//! runs the real one. No page is opened and nothing here asks the network for
//! anything: Chrome reads its configuration, and asks for its own things, as it
//! starts.
//!
//! A second Chrome, whose stand-in writes `automatic` over the profile's file
//! before Chrome reads it, is read as mode 1 — so the reading tells the modes
//! apart, and it is the file the browser writes that keeps Chrome in mode 0.
//!
//! Mode 0 is not yet the machine's resolver: Chrome's own DNS client asks the
//! machine's nameservers itself, beside a VPN's scoped resolvers and the
//! system's own encrypted DNS. So the net log is also read for *how* each
//! name was looked up (queue item 650): every lookup must be a
//! `HOST_RESOLVER_SYSTEM_TASK` — `getaddrinfo`, the system's resolver — and
//! none a `HOST_RESOLVER_DNS_TASK` or a `DNS_TRANSACTION`, Chrome's own
//! client. The names looked up are Chrome's own two requests, which ADR 0073
//! lets leave and which it makes as it starts.

#![cfg(unix)]

use alo_window::beside::{NotInstalled, chrome_in, chrome_places};
use alo_window::chrome::{Chrome, LONGEST_SILENCE};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long Chrome is held once it has looked up its first name, for the
/// lookups that follow it: ADR 0073 measured its own two requests to start
/// within three seconds of each other.
const AFTER_FIRST_LOOKUP: Duration = Duration::from_secs(5);

/// The Chrome on this machine, where the browser looks for it. These tests
/// drive it (ADR 0058), so one that is not there fails them.
fn installed() -> Result<PathBuf, NotInstalled> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    chrome_in(&chrome_places(home.as_deref()))
}

/// A directory of the test's own holding a program that runs `chrome` with
/// its net log written to `netlog.json` beside it, after running `before`.
struct LoggedChrome {
    directory: PathBuf,
}

impl LoggedChrome {
    fn new(name: &str, chrome: &Path, before: &str) -> Result<Self, String> {
        let directory =
            std::env::temp_dir().join(format!("alo-chrome-resolver-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|why| format!("{why}"))?;
        let quoted = |path: &Path| -> Result<String, String> {
            let text = path.to_str().ok_or("a path that is not text")?;
            if text.contains('\'') {
                return Err(format!("a path with a quote in it: {text}"));
            }
            Ok(format!("'{text}'"))
        };
        let program = directory.join("chrome");
        let script = format!(
            "#!/bin/sh\n{before}\nexec {} \"$@\" --log-net-log={} --net-log-capture-mode=Default\n",
            quoted(chrome)?,
            quoted(&directory.join("netlog.json"))?
        );
        std::fs::write(&program, script).map_err(|why| format!("{why}"))?;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
            .map_err(|why| format!("{why}"))?;
        Ok(Self { directory })
    }

    fn program(&self) -> PathBuf {
        self.directory.join("chrome")
    }

    fn netlog(&self) -> String {
        std::fs::read_to_string(self.directory.join("netlog.json")).unwrap_or_default()
    }
}

impl Drop for LoggedChrome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Every secure DNS mode the net log `text` names, in the order it names them.
fn modes(text: &str) -> Vec<String> {
    const KEY: &str = "\"secure_dns_mode\":";
    text.match_indices(KEY)
        .map(|(at, _)| {
            text.get(at + KEY.len()..)
                .unwrap_or_default()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect()
        })
        .collect()
}

/// Start the Chrome at `program` as the browser does, keep it until its net log
/// names a secure DNS mode, close it, and answer every mode the whole net log
/// names — or why there were none.
fn modes_chrome_ran_in(logged: &LoggedChrome) -> Result<Vec<String>, String> {
    let mut chrome = Chrome::start_at(&logged.program()).map_err(|why| format!("{why}"))?;
    chrome.version().map_err(|why| format!("{why}"))?;
    let until = Instant::now() + LONGEST_SILENCE;
    while modes(&logged.netlog()).is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    // Closed, so that the net log is written to its end.
    drop(chrome);
    let named = modes(&logged.netlog());
    if named.is_empty() {
        return Err(format!(
            "Chrome's net log named no secure DNS mode in {} s",
            LONGEST_SILENCE.as_secs()
        ));
    }
    Ok(named)
}

/// Only the events of the net log `text`: the lines between `"events": [`
/// and `"polledData"`.
fn events(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .skip_while(|line| !line.starts_with("\"events\""))
        .skip(1)
        .take_while(|line| !line.starts_with("\"polledData\""))
}

/// The number the net log `text` writes events of type `name` as, from its
/// constants' `logEventTypes` — and only from there, since another of its
/// tables, `logSourceType`, gives some of the same names other numbers.
fn event_type(text: &str, name: &str) -> Option<String> {
    const TABLE: &str = "\"logEventTypes\":{";
    let start = text.find(TABLE)? + TABLE.len();
    let table = text.get(start..)?;
    let table = table.get(..table.find('}')?)?;
    let key = format!("\"{name}\":");
    let at = table.find(&key)? + key.len();
    let digits: String = table
        .get(at..)?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!digits.is_empty()).then_some(digits)
}

/// The type of the event on `line`: its last `"type":`, since Chrome writes an
/// event's keys in order and `type` is last of them, after the `source`
/// whose own `type` is a source's.
fn type_of(line: &str) -> Option<&str> {
    const KEY: &str = "\"type\":";
    let at = line.rfind(KEY)? + KEY.len();
    let rest = line.get(at..)?;
    rest.get(..rest.find(|c: char| !c.is_ascii_digit())?)
}

/// How Chrome looked names up, by its net log.
#[derive(Debug, Default, PartialEq, Eq)]
struct Lookups {
    /// Events of `HOST_RESOLVER_SYSTEM_TASK`: a name asked of `getaddrinfo`.
    system: usize,
    /// Events of `HOST_RESOLVER_DNS_TASK`: a name asked of Chrome's own client.
    own_client: usize,
    /// Events of `DNS_TRANSACTION`: a query Chrome's own client sent.
    transactions: usize,
}

/// How the net log `text` says names were looked up — or why it cannot say.
fn lookups(text: &str) -> Result<Lookups, String> {
    let number = |name: &str| {
        event_type(text, name).ok_or_else(|| format!("the net log has no event type {name}"))
    };
    let system = number("HOST_RESOLVER_SYSTEM_TASK")?;
    let own_client = number("HOST_RESOLVER_DNS_TASK")?;
    let transaction = number("DNS_TRANSACTION")?;
    let mut found = Lookups::default();
    for kind in events(text).filter_map(type_of) {
        if kind == system {
            found.system += 1;
        } else if kind == own_client {
            found.own_client += 1;
        } else if kind == transaction {
            found.transactions += 1;
        }
    }
    Ok(found)
}

/// Start the Chrome at `program` as the browser does, keep it until its net log
/// names a lookup and [`AFTER_FIRST_LOOKUP`] longer, close it, and answer how
/// the whole net log says it looked names up.
fn how_chrome_looked_up(logged: &LoggedChrome) -> Result<Lookups, String> {
    let mut chrome = Chrome::start_at(&logged.program()).map_err(|why| format!("{why}"))?;
    chrome.version().map_err(|why| format!("{why}"))?;
    let until = Instant::now() + LONGEST_SILENCE;
    let looked_up = |found: &Lookups| found.system + found.own_client > 0;
    while !lookups(&logged.netlog()).is_ok_and(|found| looked_up(&found)) {
        if Instant::now() >= until {
            return Err(format!(
                "Chrome's net log named no lookup in {} s",
                LONGEST_SILENCE.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(AFTER_FIRST_LOOKUP);
    // Closed, so that the net log is written to its end.
    drop(chrome);
    lookups(&logged.netlog())
}

#[test]
fn chrome_asks_the_machines_resolver_and_never_its_own_encrypted_dns() {
    let chrome = installed().unwrap();
    let logged = LoggedChrome::new("pinned", &chrome, "").unwrap();
    let named = modes_chrome_ran_in(&logged).unwrap();
    assert!(
        named.iter().all(|mode| mode == "0"),
        "Chrome ran in secure DNS modes {named:?}, where 0 is off"
    );
}

#[test]
fn the_reading_tells_chromes_automatic_mode_from_off() {
    let chrome = installed().unwrap();
    // Writes `automatic` over the file the browser wrote, in the profile
    // Chrome is handed as `--user-data-dir=`, before Chrome reads it.
    let before = r#"for given in "$@"; do
  case "$given" in --user-data-dir=*)
    printf '{"dns_over_https":{"mode":"automatic"}}' > "${given#--user-data-dir=}/Local State";;
  esac
done"#;
    let logged = LoggedChrome::new("automatic", &chrome, before).unwrap();
    let named = modes_chrome_ran_in(&logged).unwrap();
    assert!(
        named.iter().any(|mode| mode == "1"),
        "Chrome told to be automatic ran in secure DNS modes {named:?}"
    );
}

#[test]
fn a_mode_is_read_as_its_digits() {
    assert_eq!(
        modes(r#"{"a":1,"secure_dns_mode":0,"b":{"secure_dns_mode":12}},"secure_dns_mode":"#),
        vec!["0", "12", ""]
    );
    assert!(modes("{}").is_empty());
}

#[test]
fn chrome_asks_every_name_of_the_systems_resolver_and_none_of_its_own_client() {
    let chrome = installed().unwrap();
    let logged = LoggedChrome::new("system", &chrome, "").unwrap();
    let found = how_chrome_looked_up(&logged).unwrap();
    assert!(
        found.system > 0 && found.own_client == 0 && found.transactions == 0,
        "Chrome looked names up as {found:?}"
    );
}

/// Events as Chrome 156 writes them, cut short: a name asked of the system's
/// resolver, one of Chrome's own client and its query.
const WRITTEN: &str = concat!(
    "{\"constants\":{\"logEventTypes\":{\"DNS_TRANSACTION\":484,",
    "\"HOST_RESOLVER_DNS_TASK\":21,\"HOST_RESOLVER_SYSTEM_TASK\":20},",
    "\"logSourceType\":{\"DNS_TRANSACTION\":45}},\n",
    "\"events\": [\n",
    "{\"phase\":1,\"source\":{\"id\":19,\"start_time\":\"1\",\"type\":14},\"time\":\"1\",\"type\":20},\n",
    "{\"params\":{\"results\":[{\"type\":\"data\"}]},\"phase\":2,",
    "\"source\":{\"id\":19,\"start_time\":\"1\",\"type\":14},\"time\":\"2\",\"type\":20},\n",
    "{\"phase\":1,\"source\":{\"id\":21,\"start_time\":\"3\",\"type\":14},\"time\":\"3\",\"type\":21},\n",
    "{\"phase\":1,\"source\":{\"id\":22,\"start_time\":\"3\",\"type\":45},\"time\":\"3\",\"type\":484},\n",
    "{\"phase\":0,\"source\":{\"id\":23,\"start_time\":\"4\",\"type\":20},\"time\":\"4\",\"type\":45}],\n",
    "\"polledData\": {\"type\":20}\n",
    "}\n",
);

#[test]
fn a_lookup_is_read_by_its_events_own_type() {
    assert_eq!(
        event_type(WRITTEN, "DNS_TRANSACTION").as_deref(),
        Some("484")
    );
    assert_eq!(
        event_type(WRITTEN, "HOST_RESOLVER_DNS_TASK").as_deref(),
        Some("21")
    );
    assert_eq!(event_type(WRITTEN, "NOTHING"), None);
    assert_eq!(
        type_of(r#"{"source":{"id":9,"type":14},"time":"1","type":20},"#),
        Some("20")
    );
    assert_eq!(type_of(r#"{"type":20}"#), Some("20"));
    assert_eq!(type_of("{}"), None);
    // A source's `type`, a result's and the summary after the events are not
    // events' types: 45 is a transaction's source, not a transaction.
    assert_eq!(
        lookups(WRITTEN),
        Ok(Lookups {
            system: 2,
            own_client: 1,
            transactions: 1,
        })
    );
    assert!(lookups("{}").is_err());
}

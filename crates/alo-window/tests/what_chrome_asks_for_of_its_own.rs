/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome, started as the browser starts it, asks for nothing of its own but
//! the two requests ADR 0073 § 2 names (queue item 649).
//!
//! The installed Chrome is started through a program standing where Chrome
//! would that adds the net log's flags and runs the real one, as
//! `chromes_resolver_is_the_machines.rs` does. A page — a document, a style
//! sheet and a picture — is opened from a server of the test's own on this
//! machine and held open for [`HELD`], the length of the ADR's runs, then
//! Chrome is closed so that its net log is written to its end.
//!
//! The net log is read one event to a line, as Chrome writes it, grouped by
//! the source each event belongs to. Every address any event names, and
//! every host any event resolves or connects to, must be the page's or one
//! of what ADR 0073 lets leave:
//!
//! - `accounts.google.com/ListAccounts`, asking which Google accounts the
//!   profile is signed into;
//! - an on-demand install of the on-device models' manifest
//!   ([`MODELS`]): its update check, which names the component it asks for
//!   in an `X-Goog-Update-AppId` header — logged before anything is sent,
//!   so it is read even on a machine with no network; the download its
//!   answer points to, which names the component in its path; and the ping
//!   reporting the install, which names nothing and is the one update
//!   request with no query. A scheduled update would name every component
//!   at once, and is what `--disable-component-update` stops.
//!
//! Chrome's IPv6 reachability check, a UDP `connect` to a public address
//! that sends no packet (ADR 0073 § 1, 6), names neither an address nor a
//! host and is not read.
//!
//! A Chrome that updates itself and starts asking for something new fails
//! here, and that is the point (ADR 0073 § 2).

#![cfg(unix)]

use alo_window::beside::{NotInstalled, chrome_in, chrome_places};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The on-device models' manifest (`OptimizationGuideModelsManifest`), the
/// one component Chrome installs on demand (ADR 0073 § 1, 4).
const MODELS: &str = "ceofaddefefcbblgcgnibnonglccbfja";

/// Where Chrome checks for a component's update, and pings once it has one.
const UPDATE: &str = "https://update.googleapis.com/service/update2/json";

/// What Chrome asks of an empty profile's Google accounts.
const LIST_ACCOUNTS: &str = "https://accounts.google.com/ListAccounts?";

/// The hosts ADR 0073 lets Chrome's own requests reach.
const NAMED_HOSTS: [&str; 3] = [
    "accounts.google.com",
    "update.googleapis.com",
    "edgedl.me.gvt1.com",
];

/// How long the page is held open: the twelve seconds of ADR 0073's runs.
const HELD: Duration = Duration::from_secs(12);

/// How long the page's three requests are waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// The Chrome on this machine, where the browser looks for it. These tests
/// drive it (ADR 0058), so one that is not there fails them.
fn installed() -> Result<PathBuf, NotInstalled> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    chrome_in(&chrome_places(home.as_deref()))
}

/// A directory of the test's own holding a program that runs `chrome` with
/// its net log written to `netlog.json` beside it.
struct LoggedChrome {
    directory: PathBuf,
}

impl LoggedChrome {
    fn new(chrome: &Path) -> Result<Self, String> {
        let directory = std::env::temp_dir().join(format!("alo-chrome-own-{}", std::process::id()));
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
            "#!/bin/sh\nexec {} \"$@\" --log-net-log={} --net-log-capture-mode=Default\n",
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

/// The paths the server has received.
type Log = Arc<Mutex<Vec<String>>>;

/// The server's answer to `path`: its type and body.
fn page(path: &str) -> Option<(&'static str, &'static str)> {
    Some(match path {
        "/page.html" => (
            "text/html; charset=utf-8",
            "<!doctype html><title>Own</title><link rel=stylesheet href=/style.css>\
             <h1>Asked for</h1><img src=/picture.svg width=20 height=20>",
        ),
        "/style.css" => ("text/css", "h1 { color: rgb(0, 0, 128) }"),
        "/picture.svg" => (
            "image/svg+xml",
            "<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'>\
             <rect width='20' height='20' fill='black'/></svg>",
        ),
        _ => return None,
    })
}

/// A server on this machine: its port, and what it received.
fn serve() -> Result<(u16, Log), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| format!("{why}"))?;
    let port = listener
        .local_addr()
        .map_err(|why| format!("{why}"))?
        .port();
    let log: Log = Arc::default();
    let logging = Arc::clone(&log);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let logging = Arc::clone(&logging);
            std::thread::spawn(move || {
                if socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .is_err()
                {
                    return;
                }
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let path = asked.split(' ').nth(1).unwrap_or_default().to_owned();
                // A connection Chrome opened ahead of need and closed unused
                // asked for nothing.
                if path.is_empty() {
                    return;
                }
                if let Ok(mut log) = logging.lock() {
                    log.push(path.clone());
                }
                let (status, kind, body) = match page(&path) {
                    Some((kind, body)) => ("200 OK", kind, body),
                    None => ("404 Not Found", "text/plain", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, log))
}

/// What one source in the net log named: the addresses its events carry,
/// and the components its requests' `X-Goog-Update-AppId` headers name.
#[derive(Debug, Default, PartialEq, Eq)]
struct Source {
    addresses: Vec<String>,
    components: Vec<String>,
}

/// Every JSON string value in `line` that follows `key`, written with its
/// quotes and colon (`"url":"`), up to its closing quote.
fn values_after<'l>(line: &'l str, key: &str) -> Vec<&'l str> {
    line.match_indices(key)
        .filter_map(|(at, _)| {
            let rest = line.get(at + key.len()..)?;
            let mut escaped = false;
            let end = rest.char_indices().find_map(|(i, c)| {
                let ends = c == '"' && !escaped;
                escaped = c == '\\' && !escaped;
                ends.then_some(i)
            })?;
            rest.get(..end)
        })
        .collect()
}

/// The source id an event's line belongs to.
fn source_of(line: &str) -> Option<u64> {
    const KEY: &str = "\"source\":{\"id\":";
    let at = line.find(KEY)?;
    let digits: String = line
        .get(at + KEY.len()..)?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Only the events of the net log `text`: the lines between `"events": [`
/// and `"polledData"`, which is a summary of state rather than something
/// that happened.
fn events(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .skip_while(|line| !line.starts_with("\"events\""))
        .skip(1)
        .take_while(|line| !line.starts_with("\"polledData\""))
}

/// Every source the net log `text` names an address of the network for —
/// `http`, `https`, `ws` or `wss` — and what each named.
fn sources(text: &str) -> BTreeMap<u64, Source> {
    let mut found: BTreeMap<u64, Source> = BTreeMap::new();
    for line in events(text) {
        let Some(id) = source_of(line) else {
            continue;
        };
        let lower = line.to_ascii_lowercase();
        let named: Vec<String> = values_after(line, "\"url\":\"")
            .into_iter()
            .filter(|url| {
                ["http://", "https://", "ws://", "wss://"]
                    .iter()
                    .any(|scheme| url.starts_with(scheme))
            })
            .map(str::to_owned)
            .collect();
        let components: Vec<String> = lower
            .match_indices("x-goog-update-appid: ")
            .filter_map(|(at, key)| {
                let rest = lower.get(at + key.len()..)?;
                rest.split('"').next()
            })
            .flat_map(|list| list.split(',').map(|id| id.trim().to_owned()))
            .collect();
        if named.is_empty() && components.is_empty() {
            continue;
        }
        let source = found.entry(id).or_default();
        for address in named {
            if !source.addresses.contains(&address) {
                source.addresses.push(address);
            }
        }
        source.components.extend(components);
    }
    found
}

/// Every host the net log `text` resolves, connects to or checks a
/// certificate for, without its scheme or port.
fn hosts(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in events(text) {
        for host in values_after(line, "\"host\":\"") {
            let bare = host.split_once("://").map_or(host, |(_, rest)| rest);
            let bare = match bare.strip_prefix('[') {
                Some(rest) => rest.split(']').next().unwrap_or_default(),
                None => bare.split(':').next().unwrap_or_default(),
            };
            if !found.iter().any(|seen| seen == bare) {
                found.push(bare.to_owned());
            }
        }
    }
    found
}

/// Every address of the network the net log `text` names, with the
/// components named by every source that names it. A request's address is
/// carried by more than one source — the request's own, and the stream job
/// that connects it, which carries no headers — and an update check's is
/// its own alone, made so by the key in its query.
fn requests(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for source in sources(text).into_values() {
        for address in source.addresses {
            let components = found.entry(address).or_default();
            for id in &source.components {
                if !components.contains(id) {
                    components.push(id.clone());
                }
            }
        }
    }
    found
}

/// Why a request at `address`, naming `components`, is not one ADR 0073
/// lets leave, or nothing when it is — or when it is the page's, at `page`.
fn refused(address: &str, components: &[String], page: &str) -> Option<String> {
    let only_models = !components.is_empty() && components.iter().all(|id| id == MODELS);
    let admitted = address.starts_with(page)
        || address.starts_with(LIST_ACCOUNTS)
        // The on-demand install's check, naming only the models.
        || (address.starts_with(&format!("{UPDATE}?")) && only_models)
        // Its ping, which names nothing, and whose address has no query.
        || (address == UPDATE && components.is_empty())
        // Its download, which names the models in its path.
        || ((address.starts_with("http://edgedl.me.gvt1.com/")
            || address.starts_with("https://edgedl.me.gvt1.com/"))
            && address.contains(&format!("/{MODELS}/")));
    (!admitted).then(|| format!("{address} (components {components:?})"))
}

#[test]
fn chrome_asks_for_nothing_of_its_own_but_the_two_named() {
    let chrome = installed().unwrap();
    let logged = LoggedChrome::new(&chrome).unwrap();
    let (port, log) = serve().unwrap();
    let page = format!("http://127.0.0.1:{port}/");
    let address = alo_url::parse(&format!("{page}page.html")).unwrap();
    let mut chrome = Chrome::start_at(&logged.program()).unwrap();
    {
        let mut tab = Tab::open(&mut chrome, &address, SIZE).unwrap();
        let awaited = ["/page.html", "/style.css", "/picture.svg"];
        let until = Instant::now() + AT_MOST;
        loop {
            // Each read answers every request paused meanwhile.
            tab.read().unwrap();
            let seen = log.lock().map(|log| log.clone()).unwrap_or_default();
            if awaited
                .iter()
                .all(|path| seen.iter().any(|got| got == path))
            {
                break;
            }
            assert!(Instant::now() < until, "the page asked for only {seen:?}");
            std::thread::sleep(Duration::from_millis(100));
        }
        let until = Instant::now() + HELD;
        while Instant::now() < until {
            tab.read().unwrap();
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    // Closed, so that the net log is written to its end.
    drop(chrome);
    let text = logged.netlog();
    let asked = requests(&text);

    // The reading reads: the page's own three requests are in it.
    for path in ["page.html", "style.css", "picture.svg"] {
        let wanted = format!("{page}{path}");
        assert!(
            asked.contains_key(&wanted),
            "the net log names no request for {wanted}: {asked:?}"
        );
    }
    let refusals: Vec<String> = asked
        .iter()
        .filter_map(|(address, components)| refused(address, components, &page))
        .collect();
    assert!(
        refusals.is_empty(),
        "Chrome asked of its own for what ADR 0073 does not name: {refusals:#?}"
    );
    let unnamed: Vec<String> = hosts(&text)
        .into_iter()
        .filter(|host| host != "127.0.0.1" && !NAMED_HOSTS.contains(&host.as_str()))
        .collect();
    assert!(
        unnamed.is_empty(),
        "Chrome reached hosts ADR 0073 does not name: {unnamed:?}"
    );
}

/// Events as Chrome 156 writes them, cut short: the on-demand install's
/// check, the stream job connecting it, its download and the page's
/// request.
const WRITTEN: &str = concat!(
    "{\"constants\":{\"x\":{\"url\":\"https://elsewhere.example/\"}},\n",
    "\"events\": [\n",
    "{\"params\":{\"request_headers\":{\"headers\":[\"X-Goog-Update-AppId: ceofaddefefcbblgcgnibnonglccbfja\"]},",
    "\"url\":\"https://update.googleapis.com/service/update2/json?cup2key=k\"},",
    "\"phase\":1,\"source\":{\"id\":31,\"type\":1},\"type\":2},\n",
    "{\"params\":{\"host\":\"https://update.googleapis.com\",",
    "\"url\":\"https://update.googleapis.com/service/update2/json?cup2key=k\"},",
    "\"source\":{\"id\":32,\"type\":3},\"type\":5},\n",
    "{\"params\":{\"url\":\"http://edgedl.me.gvt1.com/edgedl/diffgen-puffin/ceofaddefefcbblgcgnibnonglccbfja/a.crx3\"},",
    "\"source\":{\"id\":65,\"type\":1},\"type\":2},\n",
    "{\"params\":{\"host\":\"[2001:db8::1]:443\",\"url\":\"http://127.0.0.1:9/page.html\"},\"source\":{\"id\":70,\"type\":1},\"type\":2}\n",
    "],\n",
    "\"polledData\": {\"url\":\"https://www.google.com/\",\"host\":\"www.google.com\"}\n",
    "}\n",
);

#[test]
fn a_net_log_is_read_as_its_sources_and_hosts() {
    let named = sources(WRITTEN);
    assert_eq!(
        named.keys().copied().collect::<Vec<_>>(),
        vec![31, 32, 65, 70]
    );
    assert_eq!(
        named.get(&31).map(|source| source.components.clone()),
        Some(vec![MODELS.to_owned()])
    );
    assert_eq!(
        named.get(&32).map(|source| source.components.clone()),
        Some(vec![])
    );
    let asked = requests(WRITTEN);
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(
        asked.get(&format!("{UPDATE}?cup2key=k")),
        Some(&vec![MODELS.to_owned()])
    );
    assert_eq!(
        hosts(WRITTEN),
        vec!["update.googleapis.com".to_owned(), "2001:db8::1".to_owned()]
    );
    let page = "http://127.0.0.1:9/";
    assert!(
        asked
            .iter()
            .all(|(address, components)| refused(address, components, page).is_none())
    );
}

#[test]
fn what_the_adr_does_not_name_is_refused() {
    let page = "http://127.0.0.1:9/";
    let refuses = |address: &str, components: &[&str]| {
        let components: Vec<String> = components.iter().map(|c| (*c).to_owned()).collect();
        refused(address, &components, page).is_some()
    };
    let check = format!("{UPDATE}?cup2key=k");
    let other = "hfnkpimlhhgieaddgfemjhofmfblmnib";
    assert!(!refuses(&check, &[MODELS]));
    // A scheduled update names every component at once.
    assert!(refuses(&check, &[MODELS, other]));
    // A check naming nothing is not a ping, which has no query.
    assert!(refuses(&check, &[]));
    assert!(!refuses(UPDATE, &[]));
    assert!(refuses(UPDATE, &[other]));
    // A download of another component.
    assert!(refuses(
        &format!("http://edgedl.me.gvt1.com/edgedl/x/{other}/a.crx3"),
        &[]
    ));
    for elsewhere in [
        "https://clientservices.googleapis.com/chrome-variations/seed?osname=mac",
        "https://www.google.com/async/folae?async=_fmt:pb",
        "http://clients2.google.com/time/1/current?cup2key=k",
        "https://www.gstatic.com/ohttp_gateway/hpke_public_keys/v1/sbc_prod",
        "http://127.0.0.2:9/page.html",
    ] {
        assert!(refuses(elsewhere, &[]), "{elsewhere}");
    }
    assert!(!refuses(
        "https://accounts.google.com/ListAccounts?gpsia=1",
        &[]
    ));
}

#[test]
fn a_value_ends_at_its_unescaped_quote() {
    assert_eq!(
        values_after(r#"{"url":"a\"b","url":"c"}"#, "\"url\":\""),
        vec![r#"a\"b"#, "c"]
    );
    assert_eq!(source_of(r#"{"source":{"id":417,"type":1}}"#), Some(417));
    assert_eq!(source_of("{}"), None);
}

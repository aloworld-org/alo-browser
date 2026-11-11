/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's frames read into the tree an agent reads, through Chrome:
//! queue item 937, ADR 0109 § 1.
//!
//! Google Sheets' `/htmlview` holds its grid in a frame of its own, and
//! Chrome's `Accessibility.getFullAXTree` reads one document: the
//! `<iframe>` is a node with no children. So the page here is shaped as the
//! spike found that one, and as the web holds others: a server of this
//! test's own, reached by two names so that it is two sites (`127.0.0.1`
//! and `localhost`), serves a page holding a frame from its own site and a
//! frame from the other.
//!
//! - **Both frames' documents are read**, each the one child of its
//!   `<iframe>` node, with their words, as the page's own are.
//! - **The same-site frame is acted in by name**: text put into its field
//!   and a press of its button are heard by the frame's own listeners, which
//!   write what they heard into the frame — read back through the tree.
//! - **The other site's frame is read and not acted in**: its nodes'
//!   ids are kept apart from the page's (its process counts its own, from
//!   one), drawn where the frame is on the page, and a verb on one is
//!   refused before anything is sent — its button's listener would ask the
//!   server for `/pressed-in-other`, and the server is never asked.

use alo_agent::verb::{Target, Verb};
use alo_box::BoxId;
use alo_renderer::{Snapshot, SnapshotNode};
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_iframes::{self, APART};
use alo_window::chrome_tab::Tab;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size the page is laid out at.
const SIZE: (u32, u32) = (800, 600);

/// How long a frame's words are waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the server received: each request's host, without its port, and
/// its path.
type Log = Arc<Mutex<Vec<(String, String)>>>;

/// The server's answer to `path`, on port `port`.
fn answer(path: &str, port: u16) -> String {
    match path {
        "/page.html" => format!(
            "<!doctype html><html lang=en><title>Frames</title><body style=margin:0>\
             <h1>The page's own words</h1>\
             <iframe title=Own src=/own.html style=\"width:360px;height:200px;border:0\"></iframe>\
             <iframe title=Other src=\"http://localhost:{port}/other.html\" \
             style=\"width:300px;height:200px;margin-left:40px;border:4px solid\"></iframe>"
        ),
        "/own.html" => "<!doctype html><html lang=en><title>Own frame</title><body>\
             <p>Words of the own frame</p>\
             <label>Cell <input></label> <button>Go</button><p id=out>Nothing yet</p>\
             <script>\
             const out = document.getElementById('out');\
             const field = document.querySelector('input');\
             field.addEventListener('input', () => { out.textContent = 'Typed ' + field.value; });\
             document.querySelector('button').addEventListener('click', \
               () => { out.textContent = 'Pressed with ' + field.value; });\
             </script>"
            .to_owned(),
        "/other.html" => "<!doctype html><html lang=en><title>Other frame</title><body>\
             <p>Words of the other site</p><button>Other button</button>\
             <script>document.querySelector('button').addEventListener('click', \
               () => fetch('/pressed-in-other'));</script>"
            .to_owned(),
        _ => String::new(),
    }
}

/// A server on this machine: its port, and what it received.
fn serve() -> Option<(u16, Log)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
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
                let host = asked
                    .lines()
                    .find_map(|line| line.strip_prefix("Host: "))
                    .and_then(|host| host.split(':').next())
                    .unwrap_or_default()
                    .to_owned();
                // A connection Chrome opened ahead of need and closed unused
                // asked for nothing.
                if path.is_empty() {
                    return;
                }
                if let Ok(mut log) = logging.lock() {
                    log.push((host, path.clone()));
                }
                let body = answer(&path, port);
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Some((port, log))
}

/// Every node of `snapshot`, in the order a person reads them.
fn every(snapshot: &Snapshot) -> Vec<&SnapshotNode> {
    let mut nodes = Vec::new();
    let mut waiting: Vec<&SnapshotNode> = snapshot.root.iter().collect();
    while let Some(node) = waiting.pop() {
        nodes.push(node);
        waiting.extend(node.children.iter().rev());
    }
    nodes
}

/// The node of `snapshot` named `name`, if one is.
fn named<'s>(snapshot: &'s Snapshot, name: &str) -> Option<&'s SnapshotNode> {
    every(snapshot)
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))
}

/// Whether `inside` is `node` or under it.
fn holds(node: &SnapshotNode, inside: BoxId) -> bool {
    node.id == inside || node.children.iter().any(|child| holds(child, inside))
}

/// The tab read again and again until `name` is in its tree.
fn read_until(tab: &mut Tab<'_>, name: &str) -> Result<Snapshot, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let read = tab.read().map_err(|why| why.to_string())?;
        if named(&read, name).is_some() {
            return Ok(read);
        }
        if Instant::now() > until {
            return Err(format!("{name:?} never read: {read:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn both_frames_are_read_the_own_sites_acted_in_and_the_other_sites_left_alone() {
    let Some((port, log)) = serve() else {
        panic!("a server on this machine");
    };
    let mut chrome = Chrome::start().unwrap();
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/page.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &address, SIZE).unwrap();
    let read = read_until(&mut tab, "Words of the other site").unwrap();

    // Each frame's document is the one child of its `<iframe>`, with its
    // words.
    let frames: Vec<&SnapshotNode> = every(&read)
        .into_iter()
        .filter(|node| node.role.as_str() == "Iframe")
        .collect();
    assert_eq!(frames.len(), 2, "{read:?}");
    let own = named(&read, "Words of the own frame").unwrap();
    let other = named(&read, "Words of the other site").unwrap();
    assert!(named(&read, "The page's own words").is_some());
    for (frame, words, title) in [
        (frames[0], own.id, "Own frame"),
        (frames[1], other.id, "Other frame"),
    ] {
        assert_eq!(frame.children.len(), 1, "{frame:?}");
        assert_eq!(frame.children[0].role.as_str(), "document");
        assert_eq!(frame.children[0].name.as_deref(), Some(title));
        assert!(holds(frame, words), "{title}'s words under its own frame");
    }

    // The own site's nodes are the page's process's, numbered as the
    // page's; the other site's are kept apart from them, and drawn inside
    // its frame's content box on the page.
    let field = named(&read, "Cell").unwrap();
    assert!(!chrome_iframes::in_another_sites_frame(field.id));
    let button = named(&read, "Other button").unwrap();
    assert!(chrome_iframes::in_another_sites_frame(button.id));
    assert!(u64::try_from(button.id.as_usize()).unwrap() >= APART);
    let frame = frames[1].rect;
    let drawn = button.rect;
    assert!(
        drawn.left() >= frame.left() + 4.0
            && drawn.top() >= frame.top() + 4.0
            && drawn.right() <= frame.right() - 4.0
            && drawn.bottom() <= frame.bottom() - 4.0,
        "the other site's button {drawn:?} inside its frame's content box, within {frame:?}"
    );

    // The own site's frame takes text and a press by name, and its own
    // listeners hear both.
    tab.act(
        &Target::Named("Cell".to_owned()),
        &Verb::PutText("C29".to_owned()),
    )
    .unwrap();
    read_until(&mut tab, "Typed C29").unwrap();
    tab.act(&Target::Named("Go".to_owned()), &Verb::Activate)
        .unwrap();
    read_until(&mut tab, "Pressed with C29").unwrap();

    // The other site's frame is read, and a verb there refused before
    // anything is sent: its listener's request never arrives.
    let refused = tab.act(&Target::Named("Other button".to_owned()), &Verb::Activate);
    assert!(
        matches!(refused, Err(Unacted::InAnotherSitesFrame { node }) if node == button.id),
        "{refused:?}"
    );
    std::thread::sleep(Duration::from_millis(500));
    tab.read().unwrap();
    let asked = log.lock().map(|log| log.clone()).unwrap_or_default();
    assert!(
        !asked.iter().any(|(_, path)| path == "/pressed-in-other"),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .any(|(host, path)| host == "localhost" && path == "/other.html"),
        "the other site's frame came from the other site: {asked:?}"
    );
}

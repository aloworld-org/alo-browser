//! Downloads that stop, and the second half being asked for rather than the
//! whole thing again.
//!
//! Queue item 154's closing condition, end to end: *a download interrupted
//! halfway resumes and the bytes are the same as an uninterrupted one, and a
//! server that answers a range request with the whole thing is noticed rather
//! than believed.*
//!
//! The servers are a few lines each, on `127.0.0.1`, and they misbehave
//! deliberately — one hangs up in the middle of a body, one ignores `Range`
//! altogether, one answers a range starting a byte away from where it was
//! asked. Nothing reaches the network.
//!
//! The rules about *where a byte goes* are asserted without a socket, in
//! `alo_net::download`'s own tests. What is here is that the loop over a real
//! connection reaches them.

use alo_net::{Pool, Request, Trust};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The thing being downloaded: long enough that half of it is unmistakable.
const FILE: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Where the first answer stops.
const STOPS_AT: usize = 25;

/// Read one request off a socket, and hand back what it said.
fn take_a_request(socket: &mut TcpStream) -> Option<String> {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match socket.read(&mut byte) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        seen.push(byte.first().copied().unwrap_or(0));
        if seen.ends_with(b"\r\n\r\n") {
            return Some(String::from_utf8_lossy(&seen).into_owned());
        }
        if seen.len() > 16 * 1024 {
            return None;
        }
    }
}

/// Which byte a request's `Range` asks to start at, when it asks.
fn asked_from(request: &str) -> Option<u64> {
    let line = request
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("range:"))?;
    let value = line.split_once(':')?.1.trim();
    let rest = value.strip_prefix("bytes=")?;
    rest.strip_suffix('-')?.parse().ok()
}

/// Whether a request carries this header, however it was capitalised.
fn says(request: &str, name: &str, value: &str) -> bool {
    request.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with(&format!("{}:", name.to_ascii_lowercase()))
            && line
                .to_ascii_lowercase()
                .contains(&value.to_ascii_lowercase())
    })
}

fn serve(behaviour: impl Fn(TcpStream, usize) + Send + 'static) -> (u16, Arc<AtomicUsize>) {
    let sockets = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&sockets);
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return (0, sockets);
    };
    let Ok(address) = listener.local_addr() else {
        return (0, sockets);
    };
    let port = address.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { return };
            let which = counted.fetch_add(1, Ordering::SeqCst);
            behaviour(socket, which);
        }
    });
    (port, sockets)
}

fn url(port: u16, path: &str) -> alo_url::Url {
    let text = format!("http://127.0.0.1:{port}{path}");
    alo_url::parse(&text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

fn pool() -> Pool {
    // Nothing here is `https`, so trusting nobody costs nothing and saves
    // reading the machine's certificate store. Half a second of patience so a
    // server that goes quiet does not make the whole suite wait.
    Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| no_trust()))
        .patient_for(std::time::Duration::from_millis(500))
}

fn no_trust() -> Trust {
    Trust::of(&[]).unwrap_or_else(|_| no_trust())
}

/// The head of a `200` for the whole file, with everything a resume needs.
fn whole_head() -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v1\"\r\nAccept-Ranges: bytes\r\n\r\n",
        FILE.len()
    )
}

/// A server that answers the whole file every time, and never stops early.
fn a_server_that_works() -> u16 {
    let (port, _) = serve(|mut socket, _| {
        while take_a_request(&mut socket).is_some() {
            let mut out = whole_head().into_bytes();
            out.extend_from_slice(FILE);
            if socket.write_all(&out).is_err() {
                return;
            }
            let _ = socket.flush();
        }
    });
    port
}

#[test]
fn a_download_interrupted_half_way_is_the_same_bytes_as_an_uninterrupted_one() {
    // **The item's closing condition.** The first answer promises the whole
    // file and hangs up in the middle of it; the second is asked for from
    // where the first stopped.
    let (port, sockets) = serve(|mut socket, _| {
        let Some(request) = take_a_request(&mut socket) else {
            return;
        };
        match asked_from(&request) {
            None => {
                // The whole thing, promised — and then cut off.
                let mut out = whole_head().into_bytes();
                out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
                let _ = socket.write_all(&out);
                let _ = socket.flush();
                // And go away, which is what a connection dropped in the
                // middle of a download actually looks like.
            }
            Some(from) => {
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let rest = FILE.get(from..).unwrap_or_default();
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
                     Content-Range: bytes {}-{}/{}\r\nETag: \"v1\"\r\n\r\n",
                    rest.len(),
                    from,
                    FILE.len() - 1,
                    FILE.len()
                );
                let mut out = head.into_bytes();
                out.extend_from_slice(rest);
                let _ = socket.write_all(&out);
                let _ = socket.flush();
            }
        }
    });

    let mut held = pool();
    let resumed = held
        .download(&Request::get(url(port, "/big.bin")))
        .expect("a download that resumed");

    let mut plain = pool();
    let uninterrupted = plain
        .download(&Request::get(url(a_server_that_works(), "/big.bin")))
        .expect("a download that never stopped");

    assert_eq!(
        resumed.body, uninterrupted.body,
        "the same bytes as an uninterrupted one"
    );
    assert_eq!(resumed.body, FILE);
    assert_eq!(
        resumed.headers.get("Content-Length"),
        Some(FILE.len().to_string().as_str()),
        "and it says how long it is rather than how long one piece was"
    );
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        2,
        "one socket for each half, because the first was hung up on"
    );
}

#[test]
fn a_download_never_asks_for_something_it_could_not_splice() {
    // A byte range of a compressed stream is a range nobody can decompress, so
    // a download asks for `identity` from its **first** request rather than
    // from the resumed one.
    let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen = Arc::clone(&asked);
    let (port, _) = serve(move |mut socket, _| {
        let Some(request) = take_a_request(&mut socket) else {
            return;
        };
        if let Ok(mut held) = seen.lock() {
            held.push(request.clone());
        }
        match asked_from(&request) {
            None => {
                let mut out = whole_head().into_bytes();
                out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
                let _ = socket.write_all(&out);
                let _ = socket.flush();
            }
            Some(from) => {
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let rest = FILE.get(from..).unwrap_or_default();
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
                     Content-Range: bytes {}-{}/{}\r\n\r\n",
                    rest.len(),
                    from,
                    FILE.len() - 1,
                    FILE.len()
                );
                let mut out = head.into_bytes();
                out.extend_from_slice(rest);
                let _ = socket.write_all(&out);
                let _ = socket.flush();
            }
        }
    });

    let mut held = pool();
    assert!(held.download(&Request::get(url(port, "/big.bin"))).is_ok());

    let held = asked
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let first = held.first().map(String::as_str).unwrap_or_default();
    let second = held.get(1).map(String::as_str).unwrap_or_default();
    assert!(
        says(first, "Accept-Encoding", "identity"),
        "the first ask, not only the resumed one: {first:?}"
    );
    assert!(
        !says(first, "Accept-Encoding", "gzip"),
        "and nothing else beside it: {first:?}"
    );
    assert!(
        says(second, "Range", "bytes=25-"),
        "from where it stopped: {second:?}"
    );
    assert!(
        says(second, "If-Range", "\"v1\""),
        "so the server can say the file changed underneath: {second:?}"
    );
}

#[test]
fn a_server_that_answers_a_range_with_the_whole_thing_is_noticed_rather_than_believed() {
    // The other half of the closing condition. This server ignores `Range`
    // completely, which is common and is not misbehaviour — a client that
    // appended its answer would produce a file with its own first 25 bytes
    // repeated in the middle of it.
    let (port, sockets) = serve(|mut socket, which| {
        let Some(_) = take_a_request(&mut socket) else {
            return;
        };
        let mut out = whole_head().into_bytes();
        if which == 0 {
            out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
        } else {
            out.extend_from_slice(FILE);
        }
        let _ = socket.write_all(&out);
        let _ = socket.flush();
    });

    let mut held = pool();
    let downloaded = held
        .download(&Request::get(url(port, "/big.bin")))
        .expect("a download that started again");
    assert_eq!(downloaded.body, FILE, "not the first 25 bytes twice over");
    assert_eq!(downloaded.body.len(), FILE.len());
    assert_eq!(sockets.load(Ordering::SeqCst), 2);
}

#[test]
fn a_range_that_begins_a_byte_away_from_where_we_stopped_is_refused() {
    // One byte off is one byte missing from the middle of a file, and a file
    // of the right length that is not the thing is the failure nothing
    // downstream could notice.
    let (port, _) = serve(|mut socket, _| {
        let Some(request) = take_a_request(&mut socket) else {
            return;
        };
        if asked_from(&request).is_none() {
            let mut out = whole_head().into_bytes();
            out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
            let _ = socket.write_all(&out);
            let _ = socket.flush();
            return;
        }
        // Asked from 25; answers from 26.
        let rest = FILE.get(STOPS_AT + 1..).unwrap_or_default();
        let head = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
             Content-Range: bytes {}-{}/{}\r\n\r\n",
            rest.len(),
            STOPS_AT + 1,
            FILE.len() - 1,
            FILE.len()
        );
        let mut out = head.into_bytes();
        out.extend_from_slice(rest);
        let _ = socket.write_all(&out);
        let _ = socket.flush();
    });

    let mut held = pool();
    let why = held
        .download(&Request::get(url(port, "/big.bin")))
        .err()
        .unwrap_or_else(|| "it was accepted".to_owned());
    assert!(why.contains("26") && why.contains("25"), "{why:?}");
}

#[test]
fn a_server_that_never_gets_any_further_stops_rather_than_being_asked_for_ever() {
    // Every answer is the same first 25 bytes and then a hang-up. Without a
    // bound this is a loop nobody wrote a way out of.
    let (port, sockets) = serve(|mut socket, _| {
        if take_a_request(&mut socket).is_none() {
            return;
        }
        let mut out = whole_head().into_bytes();
        out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
        let _ = socket.write_all(&out);
        let _ = socket.flush();
    });

    let mut held = pool();
    let why = held
        .download(&Request::get(url(port, "/big.bin")))
        .err()
        .unwrap_or_else(|| "it finished somehow".to_owned());
    assert!(why.contains("attempts"), "{why:?}");
    assert!(
        sockets.load(Ordering::SeqCst) <= 8,
        "it gave up rather than going on: {} sockets",
        sockets.load(Ordering::SeqCst)
    );
}

#[test]
fn a_server_that_says_it_will_not_resume_fails_rather_than_asking_anyway() {
    let (port, sockets) = serve(|mut socket, _| {
        if take_a_request(&mut socket).is_none() {
            return;
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v1\"\r\n\
             Accept-Ranges: none\r\n\r\n",
            FILE.len()
        );
        let mut out = head.into_bytes();
        out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
        let _ = socket.write_all(&out);
        let _ = socket.flush();
    });

    let mut held = pool();
    let why = held
        .download(&Request::get(url(port, "/big.bin")))
        .err()
        .unwrap_or_else(|| "it was accepted".to_owned());
    assert!(why.contains("will not resume"), "{why:?}");
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        1,
        "asking anyway would spend a round trip to be told no"
    );
}

#[test]
fn a_download_follows_a_redirect_and_starts_at_where_it_lands() {
    let (port, _) = serve(|mut socket, _| {
        let Some(request) = take_a_request(&mut socket) else {
            return;
        };
        if request.starts_with("GET /moved ") {
            let _ = socket.write_all(
                b"HTTP/1.1 301 Moved Permanently\r\nLocation: /big.bin\r\nContent-Length: 0\r\n\r\n",
            );
            let _ = socket.flush();
            return;
        }
        match asked_from(&request) {
            None => {
                let mut out = whole_head().into_bytes();
                out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
                let _ = socket.write_all(&out);
                let _ = socket.flush();
            }
            Some(from) => {
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let rest = FILE.get(from..).unwrap_or_default();
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
                     Content-Range: bytes {}-{}/{}\r\n\r\n",
                    rest.len(),
                    from,
                    FILE.len() - 1,
                    FILE.len()
                );
                let mut out = head.into_bytes();
                out.extend_from_slice(rest);
                let _ = socket.write_all(&out);
                let _ = socket.flush();
            }
        }
    });

    let mut held = pool();
    let downloaded = held
        .download(&Request::get(url(port, "/moved")))
        .expect("a download that followed a redirect and then resumed");
    assert_eq!(downloaded.body, FILE);
    assert!(
        downloaded.url.serialised.ends_with("/big.bin"),
        "where it actually came from: {}",
        downloaded.url
    );
}

#[test]
fn a_request_that_must_not_happen_twice_is_never_downloaded() {
    // A download asks more than once by definition. A `POST` that stopped half
    // way is a thing that has happened, and asking again is asking for it to
    // happen twice — the same rule the pool's retry is built on.
    let (port, sockets) = serve(|mut socket, _| {
        let _ = take_a_request(&mut socket);
    });
    let mut held = pool();
    let mut posting = Request::get(url(port, "/pay"));
    posting.method = "POST".to_owned();
    let why = held
        .download(&posting)
        .err()
        .unwrap_or_else(|| "it was downloaded".to_owned());
    assert!(why.contains("POST"), "{why:?}");
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        0,
        "and it was refused before a socket was opened"
    );
}

#[test]
fn nothing_a_server_can_answer_a_range_request_with_makes_this_panic() {
    // Not an assertion about what each means — an assertion that a download
    // driven by a stranger is a result rather than a crash.
    let heads = [
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 25-61/62\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Range: */62\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/0\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 25-18446744073709551615/18446744073709551615\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 25-61/62\r\nContent-Range: bytes 25-61/62\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */62\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 25-61/62\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n",
    ];
    for head in heads {
        let answer = head.to_owned();
        let (port, _) = serve(move |mut socket, _| {
            let Some(request) = take_a_request(&mut socket) else {
                return;
            };
            if asked_from(&request).is_none() {
                let mut out = whole_head().into_bytes();
                out.extend_from_slice(FILE.get(..STOPS_AT).unwrap_or_default());
                let _ = socket.write_all(&out);
            } else {
                let _ = socket.write_all(answer.as_bytes());
            }
            let _ = socket.flush();
        });
        let mut held = pool();
        // Refusing is an answer and so is finishing. What may never happen is
        // a body that is not *the file* — bytes spliced from two answers, or
        // the same bytes twice. So whatever comes back is a prefix of the real
        // thing, which a spliced body could not be.
        if let Ok(downloaded) = held.download(&Request::get(url(port, "/big.bin"))) {
            assert!(
                FILE.starts_with(&downloaded.body),
                "answered {head:?} and produced {:?}",
                String::from_utf8_lossy(&downloaded.body)
            );
        }
    }
}

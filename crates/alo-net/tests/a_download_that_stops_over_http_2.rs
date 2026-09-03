//! A download that stops over HTTP/2, and the second half being asked for
//! rather than the whole thing again.
//!
//! Queue item 185's closing condition: *a download over HTTP/2 interrupted
//! halfway opens one range request rather than starting again, in the same
//! shape of test as `a_download_that_stops_half_way.rs`.* Same shape, and the
//! same file being downloaded, so the two can be read side by side — what
//! changed is the protocol underneath and nothing else.
//!
//! # Why the loop is here rather than a `Pool`
//!
//! This engine speaks HTTP/2 only over TLS, because ALPN is the only way it
//! will learn a server speaks it (`Connection::protocol`, and the reason is in
//! queue item 162: no request may be sent twice to find out). Starting a TLS
//! server needs `rustls`, and ADR 0001 allows that name in `alo-net/src/tls.rs`
//! and nowhere else — including here.
//!
//! So the server below speaks HTTP/2 on a plain socket and the download is
//! driven by `alo_net::whole_of`, which is the **same loop** `Pool::download`
//! runs: it takes the exchange as an argument and never learns what carried it.
//! What is under test is therefore the real loop over the real HTTP/2 client,
//! with the pool's kept connection swapped for a fresh socket per exchange —
//! which is what the pool does anyway after a body that stopped short.
//!
//! Nothing here touches the network.

use alo_net::h2::client;
use alo_net::h2::frame::{self, Frame, Setting};
use alo_net::h2::hpack::{self, Field, Table};
use alo_net::{Answered, Request};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The thing being downloaded — the same bytes as the HTTP/1.1 test, so the two
/// can be read against each other.
const FILE: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Where the first answer stops.
const STOPS_AT: usize = 25;

/// How long a test waits on a server that has gone quiet.
const PATIENCE: Duration = Duration::from_millis(500);

// --- A server that speaks just enough HTTP/2 ---------------------------------

/// Read exactly this many bytes, or give up.
fn exactly(socket: &mut TcpStream, how_many: usize) -> Option<Vec<u8>> {
    let mut got = vec![0u8; how_many];
    socket.read_exact(&mut got).ok()?;
    Some(got)
}

/// Take a request off a fresh connection: the preface, the settings, and the
/// first `HEADERS` block, decoded.
fn take_a_request(socket: &mut TcpStream, reading: &mut Table) -> Option<Vec<Field>> {
    if exactly(socket, frame::PREFACE.len()).as_deref() != Some(frame::PREFACE) {
        return None;
    }
    loop {
        let got = frame::read(socket, frame::LARGEST_BY_DEFAULT).ok()?;
        match got {
            Frame::Settings { ack: false, .. } => {
                socket
                    .write_all(&frame::write(&Frame::Settings {
                        ack: false,
                        values: vec![(Setting::MAX_CONCURRENT_STREAMS, 100)],
                    }))
                    .ok()?;
                socket
                    .write_all(&frame::write(&Frame::Settings {
                        ack: true,
                        values: Vec::new(),
                    }))
                    .ok()?;
            }
            Frame::Headers { block, .. } => return hpack::decode(&block, reading).ok(),
            _ => {}
        }
    }
}

/// What a request asked for, by header name.
fn asked(fields: &[Field], name: &str) -> Option<String> {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| field.value.clone())
}

/// Which byte a request's `range` asks to start at, when it asks.
fn asked_from(fields: &[Field]) -> Option<usize> {
    asked(fields, "range")?
        .strip_prefix("bytes=")?
        .strip_suffix('-')?
        .parse()
        .ok()
}

/// The stream a client's request opened. It is always 1 here: each exchange
/// gets a fresh connection, and a client opens on odd numbers from one.
const THEIRS: u32 = 1;

/// Answer with a header block and however much body, on the client's stream.
fn answer(socket: &mut TcpStream, writing: &mut Table, head: &[Field], body: &[u8], whole: bool) {
    let block = hpack::encode(head, writing);
    let _ = socket.write_all(&frame::write(&Frame::Headers {
        stream: THEIRS,
        block,
        end_stream: false,
        end_headers: true,
        priority: None,
    }));
    let _ = socket.write_all(&frame::write(&Frame::Data {
        stream: THEIRS,
        data: body.to_vec(),
        end_stream: whole,
    }));
    let _ = socket.flush();
}

/// The head of a `200` for the whole file, with everything a resume needs.
fn whole_head() -> Vec<Field> {
    vec![
        Field::new(":status", "200"),
        Field::new("content-length", FILE.len().to_string()),
        Field::new("etag", "\"v1\""),
        Field::new("accept-ranges", "bytes"),
    ]
}

/// The head of a `206` carrying the file from `from` to its end.
fn range_head(from: usize) -> Vec<Field> {
    vec![
        Field::new(":status", "206"),
        Field::new(
            "content-length",
            FILE.len().saturating_sub(from).to_string(),
        ),
        Field::new(
            "content-range",
            format!("bytes {}-{}/{}", from, FILE.len() - 1, FILE.len()),
        ),
        Field::new("etag", "\"v1\""),
    ]
}

/// Stop sending, without stopping listening.
///
/// A `shutdown` rather than dropping the socket, and the difference is not
/// cosmetic: this client is still writing window updates for the bytes it just
/// took, and a peer that closed outright would reset the connection and take
/// the frames it had already sent with it. A half-close is what a server that
/// has finished talking actually does, and it is the only version of "it hung
/// up" that says something about the client rather than about TCP.
fn stop_sending(socket: &mut TcpStream) {
    let _ = socket.flush();
    let _ = socket.shutdown(Shutdown::Write);
    // Drain whatever the client says next, so the close is orderly.
    let mut ignored = [0u8; 1024];
    let _ = socket.set_read_timeout(Some(Duration::from_millis(100)));
    let _ = socket.read(&mut ignored);
}

/// Start a server on loopback, and count the connections it takes.
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

// --- The client side ---------------------------------------------------------

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

/// One exchange over HTTP/2, on a socket of its own.
fn over_http_2(port: u16, request: &Request) -> Result<Answered, String> {
    let mut socket =
        TcpStream::connect(("127.0.0.1", port)).map_err(|why| format!("connect: {why}"))?;
    socket
        .set_read_timeout(Some(PATIENCE))
        .and_then(|()| socket.set_write_timeout(Some(PATIENCE)))
        .map_err(|why| format!("a timeout: {why}"))?;
    let mut speaking = client::Speaking::new();
    let ended = client::exchange_however_it_ends(&mut socket, &mut speaking, request)
        .map_err(|why| why.why)?;
    Ok(Answered {
        response: ended.response,
        short: ended.short.is_some(),
    })
}

/// Download something over HTTP/2, through the loop `Pool::download` runs.
fn download(port: u16, path: &str) -> Result<alo_net::Response, String> {
    alo_net::whole_of(&Request::get(url(port, path)), |asking| {
        over_http_2(port, asking)
    })
}

// --- The item's closing condition --------------------------------------------

#[test]
fn a_download_over_http_2_that_stops_half_way_resumes_rather_than_restarting() {
    // The first answer promises the whole file and stops sending in the middle
    // of it without ever setting END_STREAM; the second is asked for from where
    // the first stopped.
    let (port, sockets) = serve(|mut socket, _| {
        let mut reading = Table::new(4096);
        let mut writing = Table::new(4096);
        let Some(fields) = take_a_request(&mut socket, &mut reading) else {
            return;
        };
        match asked_from(&fields) {
            None => {
                answer(
                    &mut socket,
                    &mut writing,
                    &whole_head(),
                    FILE.get(..STOPS_AT).unwrap_or_default(),
                    false,
                );
                stop_sending(&mut socket);
            }
            Some(from) => {
                answer(
                    &mut socket,
                    &mut writing,
                    &range_head(from),
                    FILE.get(from..).unwrap_or_default(),
                    true,
                );
                stop_sending(&mut socket);
            }
        }
    });
    assert!(port != 0, "no server");

    let downloaded = download(port, "/big.bin").unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        downloaded.body, FILE,
        "the same bytes as an uninterrupted download"
    );
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        2,
        "one exchange for each half — a third would mean it started again"
    );
    assert_eq!(
        downloaded.headers.get("content-length"),
        Some(FILE.len().to_string().as_str()),
        "and it says how long it is rather than how long one piece was"
    );
}

#[test]
fn the_second_ask_is_a_range_from_where_the_first_stopped() {
    // The other half of "resumes rather than restarting": what actually went
    // out. A download that asked for the whole thing again would still produce
    // the right bytes and would be the defect this item is about.
    let asks = Arc::new(std::sync::Mutex::new(Vec::<Vec<Field>>::new()));
    let seen = Arc::clone(&asks);
    let (port, _) = serve(move |mut socket, _| {
        let mut reading = Table::new(4096);
        let mut writing = Table::new(4096);
        let Some(fields) = take_a_request(&mut socket, &mut reading) else {
            return;
        };
        if let Ok(mut held) = seen.lock() {
            held.push(fields.clone());
        }
        match asked_from(&fields) {
            None => {
                answer(
                    &mut socket,
                    &mut writing,
                    &whole_head(),
                    FILE.get(..STOPS_AT).unwrap_or_default(),
                    false,
                );
            }
            Some(from) => {
                answer(
                    &mut socket,
                    &mut writing,
                    &range_head(from),
                    FILE.get(from..).unwrap_or_default(),
                    true,
                );
            }
        }
        stop_sending(&mut socket);
    });
    assert!(port != 0, "no server");
    assert!(download(port, "/big.bin").is_ok());

    let held = asks
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(held.len(), 2, "one ask for each half");
    let first = held.first().cloned().unwrap_or_default();
    let second = held.get(1).cloned().unwrap_or_default();
    assert_eq!(asked(&first, "range"), None, "nothing to continue yet");
    assert_eq!(
        asked(&first, "accept-encoding"),
        Some("identity".to_owned()),
        "from the first ask: a byte range of a compressed stream is a range \
         nobody can decompress"
    );
    assert_eq!(
        asked(&second, "range"),
        Some(format!("bytes={STOPS_AT}-")),
        "from where it stopped"
    );
    assert_eq!(
        asked(&second, "if-range"),
        Some("\"v1\"".to_owned()),
        "so the server can say the file changed underneath"
    );
}

#[test]
fn a_server_that_gives_up_on_the_stream_is_resumed_from_too() {
    // The second of the two ways an HTTP/2 stream ends early, and the one that
    // has no HTTP/1.1 equivalent: the connection is fine and the *stream* is
    // over. A client that read that as a failure would throw away bytes that
    // arrived on a connection still open enough to ask for the rest on.
    let (port, sockets) = serve(|mut socket, _| {
        let mut reading = Table::new(4096);
        let mut writing = Table::new(4096);
        let Some(fields) = take_a_request(&mut socket, &mut reading) else {
            return;
        };
        match asked_from(&fields) {
            None => {
                answer(
                    &mut socket,
                    &mut writing,
                    &whole_head(),
                    FILE.get(..STOPS_AT).unwrap_or_default(),
                    false,
                );
                let _ = socket.write_all(&frame::write(&Frame::ResetStream {
                    stream: THEIRS,
                    error: alo_net::h2::ErrorCode::InternalError,
                }));
                stop_sending(&mut socket);
            }
            Some(from) => {
                answer(
                    &mut socket,
                    &mut writing,
                    &range_head(from),
                    FILE.get(from..).unwrap_or_default(),
                    true,
                );
                stop_sending(&mut socket);
            }
        }
    });
    assert!(port != 0, "no server");

    let downloaded = download(port, "/big.bin").unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(downloaded.body, FILE);
    assert_eq!(sockets.load(Ordering::SeqCst), 2);
}

#[test]
fn a_page_over_http_2_still_refuses_a_body_that_stopped() {
    // The rule item 53 made and item 185 must not have weakened: half a page is
    // not a page, and only a download may see one. `client::exchange` is what
    // every ordinary load goes through.
    let (port, _) = serve(|mut socket, _| {
        let mut reading = Table::new(4096);
        let mut writing = Table::new(4096);
        if take_a_request(&mut socket, &mut reading).is_none() {
            return;
        }
        answer(
            &mut socket,
            &mut writing,
            &whole_head(),
            FILE.get(..STOPS_AT).unwrap_or_default(),
            false,
        );
        stop_sending(&mut socket);
    });
    assert!(port != 0, "no server");

    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap_or_else(|why| panic!("{why}"));
    let _ = socket.set_read_timeout(Some(PATIENCE));
    let mut speaking = client::Speaking::new();
    let refused = client::exchange(
        &mut socket,
        &mut speaking,
        &Request::get(url(port, "/page.html")),
    );
    let Err(why) = refused else {
        panic!("half a page was handed up as a page");
    };
    assert!(
        why.why.contains("connection ended"),
        "and it says which way it ended: {:?}",
        why.why
    );
}

#[test]
fn a_stream_that_stops_before_its_headers_is_no_response_at_all() {
    // There is nothing to hand up and no byte to resume from, so this is an
    // error rather than a short response — and it is what the pool's retry is
    // for, since nothing arrived.
    let (port, _) = serve(|mut socket, _| {
        let mut reading = Table::new(4096);
        if take_a_request(&mut socket, &mut reading).is_none() {
            return;
        }
        stop_sending(&mut socket);
    });
    assert!(port != 0, "no server");

    let why = download(port, "/big.bin")
        .err()
        .unwrap_or_else(|| "it produced a file out of nothing".to_owned());
    assert!(why.contains("connection ended"), "{why:?}");
}

#[test]
fn nothing_a_server_can_stop_in_the_middle_of_makes_this_splice() {
    // The hostile half. A server that stops at every point there is to stop at,
    // including in the middle of a frame it has already declared the length of.
    // Refusing is an answer and so is finishing; what may never happen is a
    // body that is not the file — bytes from two answers spliced, or the same
    // bytes twice. So whatever comes back is a prefix of the real thing.
    for stopping in 0..7usize {
        let (port, _) = serve(move |mut socket, which| {
            let mut reading = Table::new(4096);
            let mut writing = Table::new(4096);
            let Some(fields) = take_a_request(&mut socket, &mut reading) else {
                return;
            };
            if which > 0 && asked_from(&fields).is_some() {
                // The resumed half, answered properly, so that what is under
                // test is where the *first* one stopped.
                let from = asked_from(&fields).unwrap_or(usize::MAX);
                answer(
                    &mut socket,
                    &mut writing,
                    &range_head(from),
                    FILE.get(from..).unwrap_or_default(),
                    true,
                );
                stop_sending(&mut socket);
                return;
            }
            match stopping {
                // Nothing at all after the settings.
                0 => {}
                // A header block cut in half, which is a frame declaring a
                // length it never sends.
                1 => {
                    let block = hpack::encode(&whole_head(), &mut writing);
                    let whole = frame::write(&Frame::Headers {
                        stream: THEIRS,
                        block,
                        end_stream: false,
                        end_headers: true,
                        priority: None,
                    });
                    let half = whole.len() / 2;
                    let _ = socket.write_all(whole.get(..half).unwrap_or_default());
                }
                // Headers, and then nothing.
                2 => {
                    let block = hpack::encode(&whole_head(), &mut writing);
                    let _ = socket.write_all(&frame::write(&Frame::Headers {
                        stream: THEIRS,
                        block,
                        end_stream: false,
                        end_headers: true,
                        priority: None,
                    }));
                }
                // A DATA frame cut in half: nine bytes of header promising more
                // than follows.
                3 => {
                    let block = hpack::encode(&whole_head(), &mut writing);
                    let _ = socket.write_all(&frame::write(&Frame::Headers {
                        stream: THEIRS,
                        block,
                        end_stream: false,
                        end_headers: true,
                        priority: None,
                    }));
                    let data = frame::write(&Frame::Data {
                        stream: THEIRS,
                        data: FILE.to_vec(),
                        end_stream: true,
                    });
                    let _ = socket.write_all(data.get(..20).unwrap_or_default());
                }
                // Some of the body, and then silence.
                4 => answer(
                    &mut socket,
                    &mut writing,
                    &whole_head(),
                    FILE.get(..STOPS_AT).unwrap_or_default(),
                    false,
                ),
                // A reset with no body at all behind it.
                5 => {
                    let block = hpack::encode(&whole_head(), &mut writing);
                    let _ = socket.write_all(&frame::write(&Frame::Headers {
                        stream: THEIRS,
                        block,
                        end_stream: false,
                        end_headers: true,
                        priority: None,
                    }));
                    let _ = socket.write_all(&frame::write(&Frame::ResetStream {
                        stream: THEIRS,
                        error: alo_net::h2::ErrorCode::Cancel,
                    }));
                }
                // A goodbye, and then the connection.
                _ => {
                    let _ = socket.write_all(&frame::write(&Frame::GoAway {
                        last_stream: 0,
                        error: alo_net::h2::ErrorCode::NoError,
                        debug: Vec::new(),
                    }));
                }
            }
            stop_sending(&mut socket);
        });
        assert!(port != 0, "no server");

        if let Ok(downloaded) = download(port, "/big.bin") {
            assert!(
                FILE.starts_with(&downloaded.body),
                "stopping at {stopping} produced {:?}",
                String::from_utf8_lossy(&downloaded.body)
            );
        }
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Streams, flow control, and what a peer that is not being friendly can do.
//!
//! The bounds went in before the happy path, so this file is mostly them. Every
//! way a peer spends our memory over HTTP/2 is a **count** rather than one
//! oversized thing — streams opened and never used, `CONTINUATION` frames that
//! never end, closed streams remembered forever — and a count is only bounded
//! by something that keeps it.

use alo_net::h2::ErrorCode;
use alo_net::h2::flow::{CEILING, Window};
use alo_net::h2::frame::{Frame, LARGEST_ALLOWED, LARGEST_BY_DEFAULT, Setting};
use alo_net::h2::session::{Delivered, LONGEST_HEADER_BLOCK, MOST_OPEN, Session};
use alo_net::h2::stream::State;

fn headers(stream: u32, end_stream: bool, end_headers: bool, block: &[u8]) -> Frame {
    Frame::Headers {
        stream,
        block: block.to_vec(),
        end_stream,
        end_headers,
        priority: None,
    }
}

fn data(stream: u32, bytes: usize, end_stream: bool) -> Frame {
    Frame::Data {
        stream,
        data: vec![0u8; bytes],
        end_stream,
    }
}

/// A session with one stream open and its headers already through.
fn with_one_open() -> (Session, u32) {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    let _ = session.arrived(headers(id, false, true, b"a block"));
    (session, id)
}

/// A session and a stream number nothing has happened on — which is what a
/// header block has to start on. Starting a second block on a stream that
/// already has one is *trailers*, and trailers are a different rule.
fn with_one_waiting() -> (Session, u32) {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    let _ = session.arrived(headers(id, false, true, b"a block"));
    (session, id + 2)
}

// --- The CONTINUATION flood --------------------------------------------------

/// Each frame is inside the frame-size limit and nothing limits how many there
/// are. The bound has to be on the **total across frames**, which is why it
/// lives in the session rather than in the frame reader.
#[test]
fn a_header_block_spread_across_endless_frames_is_refused() {
    let (mut session, id) = with_one_waiting();
    let mut sent = 0usize;
    let piece = vec![0u8; 4096];

    let mut refused = None;
    for _ in 0..100 {
        let frame = if sent == 0 {
            headers(id, false, false, &piece)
        } else {
            Frame::Continuation {
                stream: id,
                block: piece.clone(),
                end_headers: false,
            }
        };
        sent += piece.len();
        match session.arrived(frame) {
            Ok(_) => {}
            Err(why) => {
                refused = Some(why);
                break;
            }
        }
    }
    let why = refused.unwrap_or_else(|| panic!("four hundred kilobytes of headers was assembled"));
    assert!(
        sent <= LONGEST_HEADER_BLOCK + 4096,
        "it kept going to {sent} bytes before stopping"
    );
    assert!(why.fatal, "a flood should end the connection");
}

/// A CONTINUATION sequence is uninterruptible. A peer that could interleave
/// could make two header blocks into one.
#[test]
fn nothing_may_arrive_in_the_middle_of_a_header_block() {
    let (mut session, id) = with_one_waiting();
    let _ = session.arrived(headers(id, false, false, b"the start"));

    let interrupted = session.arrived(data(id, 10, false));
    let why = interrupted
        .err()
        .unwrap_or_else(|| panic!("a DATA frame was accepted inside a header block"));
    assert_eq!(why.error, ErrorCode::ProtocolError);
    assert!(why.fatal);
}

#[test]
fn a_continuation_for_another_stream_mid_block_is_refused() {
    let (mut session, id) = with_one_waiting();
    let _ = session.arrived(headers(id, false, false, b"the start"));
    let wrong = session.arrived(Frame::Continuation {
        stream: id + 2,
        block: b"elsewhere".to_vec(),
        end_headers: true,
    });
    assert!(
        wrong.is_err(),
        "a CONTINUATION for another stream was accepted"
    );
}

#[test]
fn a_continuation_with_nothing_in_progress_is_refused() {
    let (mut session, id) = with_one_open();
    let stray = session.arrived(Frame::Continuation {
        stream: id,
        block: b"from nowhere".to_vec(),
        end_headers: true,
    });
    assert!(stray.is_err());
}

#[test]
fn a_block_split_across_frames_arrives_as_one() {
    let (mut session, id) = with_one_waiting();
    assert_eq!(
        session
            .arrived(headers(id, true, false, b"first "))
            .unwrap_or_default(),
        None,
        "an unfinished block should deliver nothing"
    );
    let done = session
        .arrived(Frame::Continuation {
            stream: id,
            block: b"second".to_vec(),
            end_headers: true,
        })
        .unwrap_or_default();
    assert_eq!(
        done,
        Some(Delivered::Headers {
            stream: id,
            block: b"first second".to_vec(),
            end_stream: true,
        })
    );
}

// --- How many streams ---------------------------------------------------------

#[test]
fn a_peer_cannot_open_more_streams_than_this_end_allows() {
    let mut session = Session::new();
    let mut refused = None;
    for n in 0..u32::try_from(MOST_OPEN + 10).unwrap_or(110) {
        let id = n * 2 + 2; // server-initiated numbers
        match session.arrived(headers(id, false, true, b"x")) {
            Ok(_) => {}
            Err(why) => {
                refused = Some(why);
                break;
            }
        }
    }
    let why = refused.unwrap_or_else(|| panic!("a peer opened more than {MOST_OPEN} streams"));
    assert_eq!(why.error, ErrorCode::RefusedStream);
    assert!(
        !why.fatal,
        "too many streams ends the stream, not the connection"
    );
    assert!(session.open_streams() <= MOST_OPEN);
}

/// Stream numbers only go up. A frame on a lower one is either confusion or an
/// attempt to reach a stream that is gone.
#[test]
fn a_stream_number_below_one_already_used_is_refused() {
    let mut session = Session::new();
    let _ = session.arrived(headers(10, false, true, b"x"));
    let backwards = session.arrived(headers(4, false, true, b"x"));
    let why = backwards
        .err()
        .unwrap_or_else(|| panic!("a stream number went backwards and was accepted"));
    assert_eq!(why.error, ErrorCode::ProtocolError);
}

/// A peer that opens and resets streams forever must not leave an entry for
/// each.
#[test]
fn closed_streams_are_forgotten_rather_than_remembered_forever() {
    let mut session = Session::new();
    for n in 0..500u32 {
        let id = n * 2 + 2;
        let _ = session.arrived(headers(id, false, true, b"x"));
        let _ = session.arrived(Frame::ResetStream {
            stream: id,
            error: ErrorCode::Cancel,
        });
    }
    assert_eq!(session.open_streams(), 0);
    // The most recent are still known; the rest are gone.
    assert!(
        session.stream(2).is_none(),
        "the first stream is still remembered"
    );
    assert!(
        session.stream(1000).is_some(),
        "the last stream was forgotten"
    );
}

// --- The states --------------------------------------------------------------

/// A request that has been fully sent while its response is still arriving is
/// the normal state of every request a browser makes.
#[test]
fn a_stream_is_half_closed_in_the_direction_that_finished() {
    let (mut session, id) = with_one_open();
    assert_eq!(session.stream(id).map(|s| s.state), Some(State::Open));

    let _ = session.arrived(data(id, 10, true));
    assert_eq!(
        session.stream(id).map(|s| s.state),
        Some(State::HalfClosedRemote),
        "the response ended and the stream did not notice"
    );
}

/// Collapsing the two directions into a boolean is how a DATA frame after a
/// response becomes a body silently appended to a page.
#[test]
fn data_after_the_response_ended_is_refused_rather_than_appended() {
    let (mut session, id) = with_one_open();
    let _ = session.arrived(data(id, 10, true));

    let late = session.arrived(data(id, 10, false));
    let why = late
        .err()
        .unwrap_or_else(|| panic!("bytes arriving after the response ended were appended"));
    assert_eq!(why.error, ErrorCode::StreamClosed);
    assert!(
        !why.fatal,
        "one stream's problem should not end the connection"
    );
}

/// Trailers are a second header block, and they end the stream by definition —
/// a second one that did not would be a third waiting to arrive.
#[test]
fn a_second_header_block_that_does_not_end_the_stream_is_refused() {
    let (mut session, id) = with_one_open();
    let trailers = session.arrived(headers(id, true, true, b"trailers"));
    assert!(
        trailers.is_ok(),
        "legitimate trailers were refused: {trailers:?}"
    );

    let (mut session, id) = with_one_open();
    let wrong = session.arrived(headers(id, false, true, b"more"));
    assert!(
        wrong.is_err(),
        "a third header block was going to be allowed"
    );
}

// --- Flow control ------------------------------------------------------------

#[test]
fn more_data_than_the_connection_window_allowed_ends_the_connection() {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    let _ = session.arrived(headers(id, false, true, b"x"));
    let room = usize::try_from(session.receiving().room()).unwrap_or(0);

    let overrun = session.arrived(data(id, room + 1, false));
    let why = overrun
        .err()
        .unwrap_or_else(|| panic!("a peer overran the connection window and was believed"));
    assert_eq!(why.error, ErrorCode::FlowControlError);
    assert!(why.fatal, "the connection's window is everybody's");
}

/// The one that surprises people: lowering `INITIAL_WINDOW_SIZE` applies to
/// streams that already exist, retroactively, and data already in flight can
/// leave a window below zero. That is not an error.
#[test]
fn lowering_the_initial_window_may_put_an_existing_stream_below_zero() {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    assert_eq!(session.stream(id).map(|s| s.sending.room()), Some(65_535));

    // Sixty-five thousand bytes are already on their way, leaving 535.
    assert_eq!(session.room_to_send(id, 65_000), 65_000);
    assert_eq!(session.stream(id).map(|s| s.sending.room()), Some(535));

    // Now the peer lowers the initial size. It applies as a *difference* to
    // every stream that already exists, so 535 minus 65,435 lands below zero —
    // which is legal, and refusing it would break a peer that did nothing
    // wrong. A stream that had spent none of its window would simply land on
    // the new size.
    let lowered = session.arrived(Frame::Settings {
        ack: false,
        values: vec![(Setting::INITIAL_WINDOW_SIZE, 100)],
    });
    assert!(
        lowered.is_ok(),
        "a retroactive lowering was refused: {lowered:?}"
    );
    assert_eq!(
        session.stream(id).map(|s| s.sending.room()),
        Some(535 - 65_435),
        "the difference was not applied to the stream that already existed"
    );
    assert_eq!(session.stream(id).map(|s| s.sending.is_open()), Some(false));
    assert_eq!(
        session.room_to_send(id, 1),
        0,
        "it sent against a window that is below zero"
    );
}

/// A `DATA` frame counts against both windows. A sender that checked only the
/// stream would overrun the connection on the second stream.
#[test]
fn sending_counts_against_the_stream_and_the_connection_together() {
    let mut session = Session::new();
    let first = session.open().unwrap_or(1);
    let second = session.open().unwrap_or(3);
    assert_eq!(session.room_to_send(first, 65_535), 65_535);
    assert_eq!(
        session.room_to_send(second, 1),
        0,
        "the second stream sent against a connection window the first had emptied"
    );
}

#[test]
fn a_window_widened_past_the_ceiling_ends_the_connection() {
    let mut session = Session::new();
    let mut walked = None;
    for _ in 0..40 {
        if let Err(why) = session.arrived(Frame::WindowUpdate {
            stream: 0,
            increase: 1 << 30,
        }) {
            walked = Some(why);
            break;
        }
    }
    let why = walked.unwrap_or_else(|| panic!("the connection window grew past the ceiling"));
    assert_eq!(why.error, ErrorCode::FlowControlError);
    assert!(why.fatal);
    assert_eq!(Window::of(CEILING).room(), CEILING);
}

/// A window update for a stream that is gone is a race nobody lost: the peer
/// sent it before it knew. Ignoring it is correct; refusing would end
/// connections over nothing.
#[test]
fn a_window_update_for_a_stream_that_is_gone_is_ignored() {
    let mut session = Session::new();
    let update = session.arrived(Frame::WindowUpdate {
        stream: 999,
        increase: 1000,
    });
    assert!(
        update.is_ok(),
        "a crossed window update ended the connection"
    );
}

// --- Push, which this engine said it would not take --------------------------

/// We send `ENABLE_PUSH: 0`. A server reaching here has ignored what it was
/// told, and honouring it would be accepting a response to a request nobody
/// made.
#[test]
fn a_push_promise_is_refused_because_this_engine_said_it_would_not_take_one() {
    let (mut session, id) = with_one_open();
    let pushed = session.arrived(Frame::PushPromise {
        stream: id,
        promised: 2,
        block: b"a request nobody made".to_vec(),
        end_headers: true,
    });
    let why = pushed
        .err()
        .unwrap_or_else(|| panic!("a push was accepted after we said we would not take one"));
    assert_eq!(why.error, ErrorCode::ProtocolError);
    assert!(why.fatal);
}

// --- Going away ----------------------------------------------------------------

#[test]
fn no_new_streams_after_the_server_says_it_is_going_away() {
    let mut session = Session::new();
    assert!(session.open().is_ok());
    let _ = session.arrived(Frame::GoAway {
        last_stream: 1,
        error: ErrorCode::NoError,
        debug: Vec::new(),
    });
    assert!(session.is_going_away());
    let refused = session.open();
    assert_eq!(
        refused.err().map(|why| why.error),
        Some(ErrorCode::RefusedStream)
    );
}

/// What the peer allows us is not what we allow the peer. Two numbers, and
/// mixing them up means either refusing our own requests or accepting an
/// unbounded number of theirs.
#[test]
fn what_the_peer_allows_us_is_not_what_we_allow_the_peer() {
    let mut session = Session::new();
    let _ = session.arrived(Frame::Settings {
        ack: false,
        values: vec![(Setting::MAX_CONCURRENT_STREAMS, 2)],
    });
    assert!(session.open().is_ok());
    assert!(session.open().is_ok());
    assert!(
        session.open().is_err(),
        "we opened a third stream when the server allowed two"
    );
}

// ---- how large a frame this end may send ----

/// The peer's number, and ours to obey when sending — nothing to do with what
/// this end will *accept*, which is its own setting and is never raised here.
#[test]
fn the_peer_decides_how_large_a_frame_it_will_read() {
    let mut session = Session::new();
    assert_eq!(
        session.most_in_one_frame(),
        LARGEST_BY_DEFAULT as usize,
        "a session that has heard nothing should use the protocol's own default"
    );
    let told = session.arrived(Frame::Settings {
        ack: false,
        values: vec![(Setting::MAX_FRAME_SIZE, 32_768)],
    });
    assert!(told.is_ok(), "{told:?}");
    assert_eq!(session.most_in_one_frame(), 32_768);
}

/// Refused rather than clamped, at both ends of the range. Below the floor is a
/// peer asking us to cut a body into frames whose headers cost more than their
/// payloads; above the ceiling is a number that cannot be written into a frame
/// header's three bytes at all, so believing it would mean sending something
/// unreadable.
#[test]
fn a_frame_size_outside_what_the_protocol_allows_is_refused() {
    for outside in [0, 1, LARGEST_BY_DEFAULT - 1, LARGEST_ALLOWED + 1, u32::MAX] {
        let mut session = Session::new();
        let told = session.arrived(Frame::Settings {
            ack: false,
            values: vec![(Setting::MAX_FRAME_SIZE, outside)],
        });
        assert_eq!(
            told.err().map(|why| why.error),
            Some(ErrorCode::ProtocolError),
            "a maximum frame size of {outside} was believed"
        );
        assert_eq!(
            session.most_in_one_frame(),
            LARGEST_BY_DEFAULT as usize,
            "a refused setting was applied anyway"
        );
    }
}

// ---- when this end has finished with a stream ----

/// A request that is fully sent is *half-closed locally*, which is the normal
/// state of every request a browser makes. A stream left open in our own
/// bookkeeping is one this engine would count against the peer's concurrency
/// limit for ever.
#[test]
fn a_request_that_is_fully_sent_leaves_the_stream_half_closed_rather_than_open() {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    assert_eq!(session.stream(id).map(|s| s.state), Some(State::Open));
    session.finished_sending(id);
    assert_eq!(
        session.stream(id).map(|s| s.state),
        Some(State::HalfClosedLocal)
    );
    assert_eq!(
        session.open_streams(),
        1,
        "it is not over until both ends are"
    );

    // The answer arrives and ends it, and then it is over on both sides.
    let _ = session.arrived(headers(id, true, true, b"a block"));
    assert_eq!(session.stream(id).map(|s| s.state), Some(State::Closed));
    assert_eq!(session.open_streams(), 0);
}

/// A server may answer before it has read the request. The rest of the body is
/// then bytes nobody wants, and the stream is over on both sides once we say so.
#[test]
fn a_request_this_end_gave_up_on_stops_counting_as_open() {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    session.gave_up_on(id);
    assert_eq!(session.stream(id).map(|s| s.state), Some(State::Closed));
    assert_eq!(session.open_streams(), 0);
    assert_eq!(
        session.stream(id).and_then(|s| s.ended_by),
        Some(ErrorCode::Cancel),
        "a stream we abandoned should say why"
    );
}

/// An interim response is not the response, so the block after it is not
/// trailers. Nothing below the HPACK decoder can tell a `103` from a `200`,
/// which is why the session is told rather than working it out.
#[test]
fn a_block_after_an_interim_one_is_still_the_answer_rather_than_trailers() {
    let mut session = Session::new();
    let id = session.open().unwrap_or(1);
    // The `103`.
    let _ = session.arrived(headers(id, false, true, b"a block"));
    session.headers_were_interim(id);
    // The `200`, which does not end the stream because a body follows it.
    let answer = session.arrived(headers(id, false, true, b"a block"));
    assert!(
        answer.is_ok(),
        "the answer after an early hint was refused as trailers: {answer:?}"
    );
}

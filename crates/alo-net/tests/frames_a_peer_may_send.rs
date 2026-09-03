//! HTTP/2 framing, and the frames a peer sends when it is not being friendly.
//!
//! The happy path is a few lines. What this file mostly asserts is the
//! refusals — because a frame is a length somebody else chose, several thousand
//! times a page, and this is where a peer gets to decide how much memory we
//! allocate.

use alo_net::h2::ErrorCode;
use alo_net::h2::frame::{
    self, Frame, LARGEST_ALLOWED, LARGEST_BY_DEFAULT, PREFACE, Priority, Setting,
};

/// Build the nine-byte header and a payload, the way a peer would.
fn wire(kind: u8, flags: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(length.to_be_bytes().get(1..4).unwrap_or(&[0, 0, 0]));
    out.push(kind);
    out.push(flags);
    out.extend_from_slice(&stream.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn read(bytes: &[u8]) -> Result<Frame, String> {
    frame::read(&mut &bytes[..], LARGEST_BY_DEFAULT).map_err(|why| why.why)
}

/// Read a frame and say which error code it would send back.
fn refusal(bytes: &[u8]) -> Option<(ErrorCode, bool)> {
    frame::read(&mut &bytes[..], LARGEST_BY_DEFAULT)
        .err()
        .map(|why| (why.error, why.fatal))
}

// --- The shape of a frame ----------------------------------------------------

#[test]
fn the_preface_is_what_makes_a_wrong_server_fail_immediately() {
    // `PRI * HTTP/2.0` on purpose: an HTTP/1.1 server reading it sees a method
    // it does not know and gives up, rather than half-parsing a frame.
    assert!(PREFACE.starts_with(b"PRI * HTTP/2.0\r\n\r\n"));
    assert!(PREFACE.ends_with(b"SM\r\n\r\n"));
    assert_eq!(PREFACE.len(), 24);
}

/// The top bit of the stream identifier is reserved. A sender that sets it is
/// not to be argued with, and a reader that fails to mask it sees stream
/// numbers near two billion.
#[test]
fn the_reserved_bit_in_a_stream_id_is_ignored_rather_than_refused() {
    let bytes = wire(0x0, 0x1, 0x8000_0001, b"hello");
    let Ok(Frame::Data { stream, data, .. }) = read(&bytes) else {
        panic!("the reserved bit made a DATA frame unreadable");
    };
    assert_eq!(stream, 1, "the reserved bit was read as part of the number");
    assert_eq!(data, b"hello");
}

#[test]
fn every_frame_round_trips_through_writing_and_reading() {
    let frames = vec![
        Frame::Data {
            stream: 1,
            data: b"a body".to_vec(),
            end_stream: true,
        },
        Frame::Headers {
            stream: 3,
            block: b"an hpack block".to_vec(),
            end_stream: false,
            end_headers: true,
            priority: None,
        },
        Frame::Headers {
            stream: 5,
            block: b"with a priority".to_vec(),
            end_stream: true,
            end_headers: true,
            priority: Some(Priority {
                depends_on: 3,
                exclusive: true,
                weight: 200,
            }),
        },
        Frame::Priority {
            stream: 7,
            priority: Priority {
                depends_on: 1,
                exclusive: false,
                weight: 16,
            },
        },
        Frame::ResetStream {
            stream: 9,
            error: ErrorCode::Cancel,
        },
        Frame::Settings {
            ack: false,
            values: vec![
                (Setting::ENABLE_PUSH, 0),
                (Setting::MAX_CONCURRENT_STREAMS, 100),
            ],
        },
        Frame::Settings {
            ack: true,
            values: Vec::new(),
        },
        Frame::PushPromise {
            stream: 1,
            promised: 2,
            block: b"promised".to_vec(),
            end_headers: true,
        },
        Frame::Ping {
            ack: false,
            data: [1, 2, 3, 4, 5, 6, 7, 8],
        },
        Frame::GoAway {
            last_stream: 11,
            error: ErrorCode::EnhanceYourCalm,
            debug: b"too many".to_vec(),
        },
        Frame::WindowUpdate {
            stream: 0,
            increase: 65_535,
        },
        Frame::Continuation {
            stream: 13,
            block: b"the rest".to_vec(),
            end_headers: true,
        },
        Frame::Unknown {
            kind: 0x63,
            stream: 0,
            payload: b"an extension".to_vec(),
        },
    ];
    for original in frames {
        let bytes = frame::write(&original);
        let back = read(&bytes).unwrap_or_else(|why| panic!("{original:?} did not survive: {why}"));
        assert_eq!(back, original, "{original:?} changed on the way round");
    }
}

/// An error code nobody knows is carried rather than flattened: a peer sending
/// one is telling us something, and "an error" is less than it said.
#[test]
fn an_error_code_this_engine_cannot_name_is_carried_as_sent() {
    let bytes = wire(0x3, 0, 1, &4242u32.to_be_bytes());
    let Ok(Frame::ResetStream { error, .. }) = read(&bytes) else {
        panic!("a RST_STREAM with an unknown code was unreadable");
    };
    assert_eq!(error, ErrorCode::Unknown(4242));
    assert_eq!(
        error.number(),
        4242,
        "it would not go back out as it came in"
    );
}

// --- Padding, which is the classic parser bug --------------------------------

/// The first byte says how much of the rest is padding, and nothing stops it
/// saying more than there is. Subtracting without checking underflows.
#[test]
fn padding_longer_than_the_frame_is_refused_rather_than_subtracted() {
    // One byte of payload, claiming 200 bytes of padding.
    let bytes = wire(0x0, 0x8, 1, &[200]);
    assert_eq!(
        refusal(&bytes),
        Some((ErrorCode::ProtocolError, true)),
        "a padding length longer than the frame was accepted"
    );
    // The boundary, on the legal side: padding equal to the whole payload
    // including its own length byte.
    let bytes = wire(0x0, 0x8, 1, &[5, 1, 2, 3, 4]);
    assert_eq!(
        refusal(&bytes),
        Some((ErrorCode::ProtocolError, true)),
        "padding as long as the frame itself was accepted"
    );
}

/// A frame that is nothing but padding is a real thing a server sends, to
/// disguise how large a response is. It is legal and its body is empty — and a
/// check written one off would refuse it.
#[test]
fn a_frame_that_is_entirely_padding_is_legal_and_carries_nothing() {
    let bytes = wire(0x0, 0x8, 1, &[4, 0, 0, 0, 0]);
    let Ok(Frame::Data { data, .. }) = read(&bytes) else {
        panic!("a frame of pure padding was refused");
    };
    assert!(data.is_empty());
}

#[test]
fn a_padded_frame_with_no_room_for_the_length_byte_is_refused() {
    let bytes = wire(0x0, 0x8, 1, &[]);
    assert_eq!(refusal(&bytes), Some((ErrorCode::ProtocolError, true)));
}

#[test]
fn padding_that_fits_is_removed_and_the_data_is_what_is_left() {
    // Two bytes of padding after four of data.
    let bytes = wire(0x0, 0x8, 1, &[2, b'd', b'a', b't', b'a', 0, 0]);
    let Ok(Frame::Data { data, .. }) = read(&bytes) else {
        panic!("a correctly padded frame was refused");
    };
    assert_eq!(data, b"data");
}

// --- Frames that must be exactly one length ----------------------------------

#[test]
fn a_frame_of_a_length_it_may_not_have_is_refused_with_a_size_error() {
    for (kind, what, wrong) in [
        (0x2u8, "PRIORITY", vec![0u8; 4]),
        (0x2, "PRIORITY", vec![0u8; 6]),
        (0x3, "RST_STREAM", vec![0u8; 3]),
        (0x3, "RST_STREAM", vec![0u8; 5]),
        (0x8, "WINDOW_UPDATE", vec![0u8; 5]),
    ] {
        let bytes = wire(kind, 0, 1, &wrong);
        assert_eq!(
            refusal(&bytes).map(|(error, _)| error),
            Some(ErrorCode::FrameSizeError),
            "a {what} of {} bytes was accepted",
            wrong.len()
        );
    }
    // PING is on the connection, so it needs stream 0 to reach its length check.
    let bytes = wire(0x6, 0, 0, &[0u8; 7]);
    assert_eq!(
        refusal(&bytes).map(|(error, _)| error),
        Some(ErrorCode::FrameSizeError)
    );
}

#[test]
fn a_settings_frame_that_is_not_a_whole_number_of_settings_is_refused() {
    let bytes = wire(0x4, 0, 0, &[0u8; 7]);
    assert_eq!(
        refusal(&bytes).map(|(error, _)| error),
        Some(ErrorCode::FrameSizeError)
    );
}

/// An acknowledgement acknowledges. Carrying settings as well is a message that
/// says two things.
#[test]
fn a_settings_acknowledgement_carrying_settings_is_refused() {
    let bytes = wire(0x4, 0x1, 0, &[0u8; 6]);
    assert_eq!(
        refusal(&bytes).map(|(error, _)| error),
        Some(ErrorCode::FrameSizeError)
    );
}

/// The same setting twice is legal, and the last one is what it means — so the
/// order has to survive parsing.
#[test]
fn settings_keep_the_order_they_were_sent_in() {
    let mut payload = Vec::new();
    for value in [100u32, 200, 300] {
        payload.extend_from_slice(&Setting::MAX_CONCURRENT_STREAMS.0.to_be_bytes());
        payload.extend_from_slice(&value.to_be_bytes());
    }
    let bytes = wire(0x4, 0, 0, &payload);
    let Ok(Frame::Settings { values, .. }) = read(&bytes) else {
        panic!("a repeated setting was unreadable");
    };
    assert_eq!(
        values.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
        vec![100, 200, 300],
        "the order was lost, so the last value is no longer knowable"
    );
}

// --- Which frames belong where -----------------------------------------------

#[test]
fn a_frame_about_a_stream_is_refused_on_stream_zero() {
    for (kind, what) in [
        (0x0u8, "DATA"),
        (0x1, "HEADERS"),
        (0x2, "PRIORITY"),
        (0x3, "RST_STREAM"),
        (0x5, "PUSH_PROMISE"),
        (0x9, "CONTINUATION"),
    ] {
        let bytes = wire(kind, 0, 0, &[0u8; 8]);
        assert_eq!(
            refusal(&bytes).map(|(error, _)| error),
            Some(ErrorCode::ProtocolError),
            "a {what} on stream 0 was accepted"
        );
    }
}

#[test]
fn a_frame_about_the_connection_is_refused_on_a_stream() {
    for (kind, what) in [(0x4u8, "SETTINGS"), (0x6, "PING"), (0x7, "GOAWAY")] {
        let bytes = wire(kind, 0, 1, &[0u8; 8]);
        assert_eq!(
            refusal(&bytes).map(|(error, _)| error),
            Some(ErrorCode::ProtocolError),
            "a {what} on stream 1 was accepted"
        );
    }
}

/// Room for nothing is not room. Left unchecked it is a peer that can make this
/// end wait for a window that will never open.
#[test]
fn a_window_update_offering_nothing_is_refused() {
    let on_a_stream = wire(0x8, 0, 1, &0u32.to_be_bytes());
    assert_eq!(
        refusal(&on_a_stream),
        Some((ErrorCode::ProtocolError, false)),
        "a zero window update on a stream should not kill the connection"
    );
    let on_the_connection = wire(0x8, 0, 0, &0u32.to_be_bytes());
    assert_eq!(
        refusal(&on_the_connection),
        Some((ErrorCode::ProtocolError, true)),
        "a zero window update on the connection is everybody's problem"
    );
}

// --- Bounds ------------------------------------------------------------------

/// The bound is checked before the payload is reserved. Without it, three bytes
/// on the wire are sixteen megabytes of allocation.
#[test]
fn a_frame_longer_than_this_end_accepts_is_refused_before_it_is_read() {
    let mut bytes = wire(0x0, 0, 1, &[]);
    // Say sixteen megabytes, and send nothing.
    let claimed = LARGEST_ALLOWED.to_be_bytes();
    if let Some(head) = bytes.get_mut(..3) {
        head.copy_from_slice(claimed.get(1..4).unwrap_or(&[0, 0, 0]));
    }
    assert_eq!(
        refusal(&bytes).map(|(error, _)| error),
        Some(ErrorCode::FrameSizeError),
        "a frame far larger than the limit was read rather than refused"
    );
}

#[test]
fn a_frame_that_stops_in_the_middle_is_refused() {
    let whole = wire(0x0, 0, 1, b"a body that is cut off");
    let half = whole.get(..whole.len() / 2).unwrap_or_default();
    assert!(read(half).is_err(), "half a frame was read as a whole one");
}

/// A peer using an extension we have not heard of is not misbehaving — and its
/// bytes are still consumed exactly, which is what makes ignoring it safe.
#[test]
fn an_unknown_frame_type_is_ignored_without_losing_the_stream() {
    let mut stream = wire(0x63, 0, 0, b"an extension nobody knows");
    stream.extend_from_slice(&wire(0x0, 0x1, 1, b"the frame after it"));
    let mut source = &stream[..];

    let Ok(Frame::Unknown { kind, .. }) = frame::read(&mut source, LARGEST_BY_DEFAULT) else {
        panic!("an unknown frame was refused rather than ignored");
    };
    assert_eq!(kind, 0x63);

    let Ok(Frame::Data { data, .. }) = frame::read(&mut source, LARGEST_BY_DEFAULT) else {
        panic!("the frame after the unknown one was lost");
    };
    assert_eq!(data, b"the frame after it");
}

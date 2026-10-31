/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A streamed frame read off Chrome's pipe without its picture being parsed
//! as JSON (ADR 0076 B, queue item 717).
//!
//! Every message from Chrome is parsed whole by [`crate::chrome_protocol`],
//! and an event's `params` written again for whoever reads them. A frame of
//! the card's page is half a megabyte of base64 up to sixty times a second,
//! and parsing and writing it again took the reading thread longer than the
//! frames took to come in a debug build — so an agent's answer, read by the
//! same thread in the order Chrome sent it, waited behind frames for tens of
//! milliseconds (measured, `tests/a_page_as_a_card_through_chrome.rs`).
//!
//! So a message that **begins exactly** as Chrome writes a frame —
//! `{"method":"Page.screencastFrame","params":{"data":"` — has its picture
//! taken as everything up to where its metadata begins (`","metadata":{`,
//! looked for back from the message's end), and only **the rest** — the
//! message with the picture taken out — is parsed as JSON, as every other
//! message is. A message begun so whose rest is not that is [`None`], and is
//! read the long way. Of a frame Chrome wrote, what this reads is what the
//! long way would: the same method and session, and `params` written with
//! the picture first, as `serde_json` orders an object's keys.
//!
//! The picture itself is not scanned — scanning half a megabyte a letter at
//! a time was the reading thread's largest cost after parsing it — so a
//! picture holding anything but base64 is taken as it is; it is then not
//! base64, and its frame is refused where it is decoded
//! ([`crate::chrome_screencast::Frame::png`]), never read as anything
//! else.

use crate::chrome_protocol::{self, Message};

/// What follows a frame's picture, as Chrome writes it.
const METADATA: &[u8] = br#"","metadata":{"#;

/// How far from a frame's end its metadata is looked for: Chrome's is about
/// two hundred bytes, its numbers and its session's id.
const LOOKED_BACK: usize = 4096;

/// How Chrome begins every frame it streams.
const BEGINS: &[u8] = br#"{"method":"Page.screencastFrame","params":{"data":""#;

/// `message`, a frame Chrome streamed, read as [`chrome_protocol::read`]
/// would read it — or [`None`] for a message that is not one begun exactly
/// as Chrome begins a frame, which is to be read the long way.
pub fn read(message: &[u8]) -> Option<Message> {
    let after = message.strip_prefix(BEGINS)?;
    // The picture ends where the frame's metadata begins, a few hundred
    // bytes from the message's end: looked for from there, so the picture
    // itself is never scanned here.
    let from = after.len().saturating_sub(LOOKED_BACK);
    let end = from
        + after
            .get(from..)?
            .windows(METADATA.len())
            .rposition(|key| key == METADATA)?;
    let (picture, rest) = (after.get(..end)?, after.get(end..)?);
    // An empty picture is read the long way, which is how the rest is.
    if picture.is_empty() {
        return None;
    }
    let without: Vec<u8> = [BEGINS, rest].concat();
    let Ok(Message::Event {
        method,
        session,
        params,
    }) = chrome_protocol::read(&without)
    else {
        return None;
    };
    // `{"data":"",…}` as written again: the picture goes between the quotes.
    let tail = params.strip_prefix(br#"{"data":"""#)?;
    let params = [br#"{"data":""#.as_slice(), picture, b"\"", tail].concat();
    Some(Message::Event {
        method,
        session,
        params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: &[u8] = br#"{"method":"Page.screencastFrame","params":{"data":"iVBORw0K+/==","metadata":{"offsetTop":0,"pageScaleFactor":1},"sessionId":7},"sessionId":"ABC"}"#;

    #[test]
    fn a_frame_is_read_as_the_long_way_reads_it() {
        assert_eq!(read(FRAME), chrome_protocol::read(FRAME).ok());
        let Some(Message::Event { params, .. }) = read(FRAME) else {
            panic!("not read");
        };
        assert!(params.starts_with(br#"{"data":"iVBORw0K+/==","metadata""#));
        assert!(params.ends_with(br#""sessionId":7}"#));
    }

    #[test]
    fn a_picture_that_is_not_base64_is_taken_as_it_is_and_refused_when_decoded() {
        let odd = br#"{"method":"Page.screencastFrame","params":{"data":"a\"b}","metadata":{},"sessionId":1}}"#;
        let Some(Message::Event { params, .. }) = read(odd) else {
            panic!("not read");
        };
        let frame = chrome_protocol::cast_frame(&params);
        assert!(
            frame
                .as_ref()
                .is_ok_and(|frame| crate::chrome_screencast::decoded(&frame.data).is_none()),
            "{frame:?}"
        );
    }

    #[test]
    fn anything_not_begun_as_chrome_begins_a_frame_is_left_to_the_long_way() {
        for other in [
            br#"{"params":{"data":"iVBO"},"method":"Page.screencastFrame"}"#.as_slice(),
            br#"{"method":"Page.screencastFrame","params":{"data":"iVBO"},"sessionId":"A"}"#,
            br#"{"method":"Page.screencastFrame","params":{"data":"iVBO"#,
            br#"{"method":"Page.screencastFrame","params":{"data":"","metadata":{},"sessionId":1}}"#,
            br#"{"method":"Page.screencastFrameX","params":{"data":"iVBO"}}"#,
            br#"{"id":1,"result":{}}"#,
        ] {
            assert_eq!(read(other), None, "{}", String::from_utf8_lossy(other));
        }
        // Begun as a frame, but not JSON after the picture.
        assert_eq!(
            read(br#"{"method":"Page.screencastFrame","params":{"data":"iVBO"}"#),
            None
        );
    }
}

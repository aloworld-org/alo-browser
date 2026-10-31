/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab's page streamed as PNG frames, for the card the conversation shows
//! it in (ADR 0076 B, queue item 717).
//!
//! Chrome is asked to stream (`Page.startScreencast`, PNG, at most the
//! card's size times the window's scale) and tells of each frame as an
//! event on the tab's session. **Every frame is answered at once**
//! (`Page.screencastFrameAck`), as it is read, wherever the tab reads it —
//! between two orders, or while an agent's command waits for its answer —
//! because Chrome sends no more until it is, and an unanswered stream would
//! stall rather than queue. Of the frames read, **only the newest is kept**:
//! a card shows one picture, and one older than another that has arrived is
//! never worth drawing.
//!
//! The pipe the frames come down is the agent's ([`crate::chrome`]), so
//! what an agent waits for is queued behind them: ADR 0076 B2 measured a few
//! milliseconds a command at the card's size, and
//! `tests/a_page_as_a_card_through_chrome.rs` holds the agent's reads to it.
//!
//! # Stopping
//!
//! [`Casting::stop`] is called once `Page.stopScreencast` is answered; a
//! frame Chrome had in flight then is still read, answered — so Chrome is
//! left owing nothing — and counted ([`Casting::after_stopping`]), but
//! never kept or shown.
//!
//! # What a frame is
//!
//! Base64 of a PNG, as Chrome wrote it, kept so: only its first 32 letters
//! are decoded here, for the PNG's signature and the size its header
//! states. Whoever shows it decodes the rest ([`Frame::png`]) on a thread
//! of its own, never the one an agent's commands wait on; its bytes go on to
//! the first screen's renderer to be read, as every picture is, in its
//! confined process.

use std::time::Instant;

/// What every PNG begins with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A frame Chrome streamed of the page, as Chrome sent it.
///
/// Its picture stays base64 until whoever shows it decodes it
/// ([`Frame::png`]): the thread holding Chrome's pipe is the agent's too,
/// and decodes nothing of a frame but its PNG header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The PNG, in base64.
    pub data: String,
    /// Its size in device pixels, as its header states.
    pub size: (u32, u32),
    /// When it was read off the pipe.
    pub arrived: Instant,
}

impl Frame {
    /// The PNG's bytes.
    ///
    /// # Errors
    ///
    /// [`Unkept::NotBase64`], for data that is not base64 after its header.
    pub fn png(&self) -> Result<Vec<u8>, Unkept> {
        decoded(&self.data).ok_or(Unkept::NotBase64)
    }
}

/// Why a frame told of was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unkept {
    /// Its data was not base64.
    NotBase64,
    /// Its bytes are not a PNG with a header.
    NotAPng,
}

impl core::fmt::Display for Unkept {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NotBase64 => "a frame Chrome streamed was not base64",
            Self::NotAPng => "a frame Chrome streamed was not a PNG",
        })
    }
}

/// A tab's stream, as the tab keeps it.
#[derive(Debug, Default)]
pub struct Casting {
    /// Whether Chrome was asked to stream and has not been told to stop.
    on: bool,
    /// The newest frame read and not yet taken, as Chrome sent it: base64,
    /// decoded only once it is taken, so a frame a newer one replaces costs
    /// nothing but its answer, and an agent's command waiting behind a frame
    /// waits for no decoding.
    newest: Option<(String, Instant)>,
    /// How many frames were read while streaming, and their PNG bytes.
    frames: u64,
    bytes: u64,
    /// How many were read after the stream was stopped.
    after: u64,
    /// The commands answering frames, by id, whose replies are nobody's.
    answering: Vec<u64>,
}

impl Casting {
    /// Chrome was asked to stream.
    pub fn start(&mut self) {
        self.on = true;
    }

    /// Chrome has answered that it stopped: what is read from now on is
    /// counted and dropped, and any frame not yet taken is dropped too.
    pub fn stop(&mut self) {
        self.on = false;
        self.newest = None;
    }

    /// Whether the page is being streamed.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// A frame told of with `data`, base64 as Chrome sent it, read: kept as
    /// the newest while streaming, counted after.
    pub fn read(&mut self, data: String) {
        if !self.on {
            self.after = self.after.saturating_add(1);
            return;
        }
        self.frames = self.frames.saturating_add(1);
        self.bytes = self.bytes.saturating_add(decoded_length(&data));
        self.newest = Some((data, Instant::now()));
    }

    /// The newest frame read since this was last asked, if one was, its
    /// header read.
    ///
    /// # Errors
    ///
    /// [`Unkept`], for a frame that is not base64 of a PNG, which is then
    /// dropped.
    pub fn take(&mut self) -> Result<Option<Frame>, Unkept> {
        let Some((data, arrived)) = self.newest.take() else {
            return Ok(None);
        };
        // The signature and the header's size: the first 24 bytes, which
        // are the first 32 letters.
        let header = decoded(data.get(..32).ok_or(Unkept::NotAPng)?).ok_or(Unkept::NotBase64)?;
        let size = png_size(&header).ok_or(Unkept::NotAPng)?;
        Ok(Some(Frame {
            data,
            size,
            arrived,
        }))
    }

    /// The frame answered by the command sent as `id`: its reply is
    /// nobody's to wait for.
    pub fn answering(&mut self, id: u64) {
        self.answering.push(id);
    }

    /// Whether the reply to `id` answers a frame, forgetting it if so.
    pub fn answered(&mut self, id: u64) -> bool {
        match self.answering.iter().position(|each| *each == id) {
            Some(at) => {
                self.answering.swap_remove(at);
                true
            }
            None => false,
        }
    }

    /// How many frames were read while streaming, and their PNGs' bytes.
    pub fn read_so_far(&self) -> (u64, u64) {
        (self.frames, self.bytes)
    }

    /// How many frames Chrome had in flight when it stopped, read after.
    pub fn after_stopping(&self) -> u64 {
        self.after
    }
}

/// The size a PNG's header states, in pixels: [`None`] for bytes that are
/// not a PNG beginning with its header — of which the first 24 are enough.
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if !png.starts_with(PNG_SIGNATURE) || png.get(12..16) != Some(b"IHDR") {
        return None;
    }
    let number = |at: usize| {
        png.get(at..at + 4)
            .and_then(|four| <[u8; 4]>::try_from(four).ok())
            .map(u32::from_be_bytes)
    };
    Some((number(16)?, number(20)?))
}

/// How many bytes base64 `text` of the standard length stands for, without
/// decoding it: three for every four letters, less one for each `=`.
fn decoded_length(text: &str) -> u64 {
    let whole = (text.len() / 4 * 3) as u64;
    let padding = text
        .bytes()
        .rev()
        .take(2)
        .filter(|&letter| letter == b'=')
        .count() as u64;
    whole.saturating_sub(padding)
}

/// The bytes base64 `text` stands for, in the standard alphabet with its
/// padding, as Chrome writes a frame; [`None`] for anything else.
///
/// Each letter by a `match` rather than a search of the alphabet, since a
/// frame is half a megabyte of it up to sixty times a second.
pub fn decoded(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let body = text.trim_end_matches('=').as_bytes();
    if bytes.len() - body.len() > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() / 4 * 3 + 2);
    let mut held: u32 = 0;
    let mut bits: u32 = 0;
    for &letter in body {
        let six = sextet(letter)?;
        held = (held << 6) | u32::from(six);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((held >> bits) & 0xff).ok()?);
        }
    }
    Some(out)
}

/// The six bits a letter of base64's standard alphabet stands for.
fn sextet(letter: u8) -> Option<u8> {
    match letter {
        b'A'..=b'Z' => Some(letter - b'A'),
        b'a'..=b'z' => Some(letter - b'a' + 26),
        b'0'..=b'9' => Some(letter - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG's signature and header for a picture `width` by `height`.
    fn header(width: u32, height: u32) -> Vec<u8> {
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&width.to_be_bytes());
        png.extend_from_slice(&height.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        png
    }

    /// `bytes` in base64, by hand: what Chrome sends.
    fn encoded(bytes: &[u8]) -> String {
        const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for three in bytes.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|at| three.get(at).copied().unwrap_or(0));
            let all = u32::from(a) << 16 | u32::from(b) << 8 | u32::from(c);
            for (at, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
                let letter = *LETTERS
                    .get(usize::try_from(all >> shift & 63).unwrap())
                    .unwrap();
                out.push(if at <= three.len() {
                    char::from(letter)
                } else {
                    '='
                });
            }
        }
        out
    }

    #[test]
    fn base64_is_read_as_chrome_writes_it() {
        assert_eq!(decoded(""), Some(Vec::new()));
        assert_eq!(decoded("YQ=="), Some(b"a".to_vec()));
        assert_eq!(decoded("YWI="), Some(b"ab".to_vec()));
        assert_eq!(decoded("YWJj"), Some(b"abc".to_vec()));
        assert_eq!(decoded("//4A"), Some(vec![0xff, 0xfe, 0x00]));
        let every: Vec<u8> = (0..=255).collect();
        assert_eq!(decoded(&encoded(&every)), Some(every));
        for refused in ["YQ=", "Y===", "YW-j", "YW j", "YWJj\n"] {
            assert_eq!(decoded(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn a_length_is_counted_without_decoding() {
        for length in 0..10 {
            let bytes: Vec<u8> = (0..length).collect();
            assert_eq!(decoded_length(&encoded(&bytes)), u64::from(length));
        }
    }

    #[test]
    fn a_pngs_size_is_its_headers() {
        assert_eq!(png_size(&header(1756, 1040)), Some((1756, 1040)));
        assert_eq!(png_size(b"GIF89a"), None);
        assert_eq!(png_size(&header(1, 1)[..20]), None);
    }

    #[test]
    fn only_the_newest_frame_is_kept_and_none_after_stopping() {
        let mut casting = Casting::default();
        casting.read(encoded(&header(2, 2)));
        assert_eq!(casting.take(), Ok(None), "nothing kept before streaming");
        assert_eq!(casting.after_stopping(), 1);
        casting.start();
        assert!(casting.is_on());
        for width in [1, 2, 3] {
            casting.read(encoded(&header(width, 9)));
        }
        let taken = casting.take().ok().flatten();
        assert_eq!(taken.map(|frame| frame.size), Some((3, 9)));
        assert_eq!(casting.take(), Ok(None));
        assert_eq!(casting.read_so_far(), (3, 3 * 29), "a header is 29 bytes");
        casting.read("not base64! not base64! not base64!".to_owned());
        assert_eq!(casting.take(), Err(Unkept::NotBase64));
        casting.read(encoded(b"GIF89a, and enough more to be 24 bytes"));
        assert_eq!(casting.take(), Err(Unkept::NotAPng));
        casting.read(encoded(b"short"));
        assert_eq!(casting.take(), Err(Unkept::NotAPng));
        let whole = [header(6, 7), b"and the rest".to_vec()].concat();
        casting.read(encoded(&whole));
        let taken = casting.take().ok().flatten();
        assert_eq!(taken.as_ref().map(|frame| frame.size), Some((6, 7)));
        assert_eq!(taken.map(|frame| frame.png()), Some(Ok(whole)));
        casting.read(encoded(&header(4, 4)));
        casting.stop();
        assert_eq!(casting.take(), Ok(None), "a frame not taken is dropped");
        casting.read(encoded(&header(5, 5)));
        assert_eq!(casting.take(), Ok(None));
        assert_eq!(casting.after_stopping(), 2);
    }

    #[test]
    fn the_reply_to_a_frames_answer_is_nobodys() {
        let mut casting = Casting::default();
        casting.answering(4);
        casting.answering(9);
        assert!(casting.answered(9));
        assert!(!casting.answered(9));
        assert!(!casting.answered(5));
        assert!(casting.answered(4));
    }
}

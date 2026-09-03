//! Where a response's body ends.
//!
//! Three ways to know, and a message that says two of them is refused before
//! it gets here ([`crate::http`]). What is left is reading the one it says.
//!
//! # Why this is its own file
//!
//! Framing is the half of HTTP where being wrong is a **security** bug rather
//! than a rendering one. A body read one byte short leaves that byte to be read
//! as the start of the next response; a chunk length trusted without a bound is
//! an allocation somebody else chooses the size of. Both belong somewhere a
//! person can read all of at once.

use crate::headers::Headers;
use crate::http::Malformed;
use crate::response::Status;
use std::io::Read;

/// The most bytes this engine will hold for one response.
///
/// A bound rather than a limit somebody hit. Without one, `Content-Length:
/// 99999999999999` is a request to allocate a hundred terabytes, and a server
/// that never sends them costs itself nothing.
pub const LARGEST_BODY: u64 = 256 * 1024 * 1024;

/// The most bytes in one chunk of a chunked body.
const LARGEST_CHUNK: u64 = 64 * 1024 * 1024;

/// How a response says where its body ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// There is no body, whatever the headers say — a `204`, a `304`, or a
    /// response to `HEAD`. Reading one anyway would read the next response.
    Empty,
    /// Exactly this many bytes.
    Exactly(u64),
    /// Chunks, each announcing its own length, until one announces none.
    Chunked,
    /// Until the connection closes, which is the only thing left when nobody
    /// said. It cannot tell a finished body from a truncated one, which is why
    /// it is last.
    UntilClose,
}

impl Framing {
    /// What a head says about its body.
    ///
    /// # Errors
    ///
    /// [`Malformed`] for a `Content-Length` that is not a length, or one
    /// larger than this engine will hold.
    pub fn of(status: Status, headers: &Headers) -> Result<Self, Malformed> {
        // These have no body by definition, and a header claiming otherwise
        // does not change that. A parser that believed the header would read
        // the next response as this one's body.
        if status.0 == 204 || status.0 == 304 || (100..200).contains(&status.0) {
            return Ok(Framing::Empty);
        }
        if headers
            .all("Transfer-Encoding")
            .any(|held| held.eq_ignore_ascii_case("chunked"))
        {
            return Ok(Framing::Chunked);
        }
        if let Some(length) = headers.get("Content-Length") {
            let length: u64 = length.trim().parse().map_err(|_| Malformed {
                why: format!("{length:?} is not a Content-Length"),
            })?;
            if length > LARGEST_BODY {
                return Err(Malformed {
                    why: format!("a body of {length} bytes, which is more than this engine holds"),
                });
            }
            return Ok(Framing::Exactly(length));
        }
        Ok(Framing::UntilClose)
    }
}

/// Read a body, however it says it ends.
///
/// # Errors
///
/// [`Malformed`] when the body does not arrive as the head said it would —
/// **including when it simply stops**, which is the difference between an
/// error and a short page.
pub fn read(source: &mut impl Read, framing: Framing) -> Result<Vec<u8>, Malformed> {
    match framing {
        Framing::Empty => Ok(Vec::new()),
        Framing::Exactly(length) => read_exactly(source, length),
        Framing::Chunked => read_chunked(source),
        Framing::UntilClose => {
            let mut out = Vec::new();
            source
                .take(LARGEST_BODY)
                .read_to_end(&mut out)
                .map_err(|why| Malformed {
                    why: why.to_string(),
                })?;
            Ok(out)
        }
    }
}

/// Exactly this many bytes, and a truncated body is an error rather than a
/// short page.
fn read_exactly(source: &mut impl Read, length: u64) -> Result<Vec<u8>, Malformed> {
    let mut out = Vec::new();
    let read = source
        .take(length)
        .read_to_end(&mut out)
        .map_err(|why| Malformed {
            why: why.to_string(),
        })?;
    if read as u64 != length {
        return Err(Malformed {
            why: format!("the body stopped after {read} bytes of the {length} it said it would be"),
        });
    }
    Ok(out)
}

/// Chunks, each announcing its own length.
fn read_chunked(source: &mut impl Read) -> Result<Vec<u8>, Malformed> {
    let mut out: Vec<u8> = Vec::new();
    loop {
        let line = read_chunk_line(source)?;
        // A chunk line may carry extensions after a semicolon; nothing reads
        // them and the length is what matters.
        let size = line.split(';').next().unwrap_or_default().trim();
        if size.is_empty() {
            return Err(Malformed {
                why: "a chunk with no length".to_owned(),
            });
        }
        let size = u64::from_str_radix(size, 16).map_err(|_| Malformed {
            why: format!("{size:?} is not a chunk length"),
        })?;
        if size > LARGEST_CHUNK {
            return Err(Malformed {
                why: format!("a chunk of {size} bytes, which is more than this engine holds"),
            });
        }
        if size == 0 {
            // The end. Trailers may follow; they are read to the blank line
            // and dropped, because nothing here acts on one and leaving them
            // in the stream would leave the connection unusable.
            while !read_chunk_line(source)?.is_empty() {}
            return Ok(out);
        }
        if out.len() as u64 + size > LARGEST_BODY {
            return Err(Malformed {
                why: "the chunks add up to more than this engine holds".to_owned(),
            });
        }
        let mut chunk = Vec::new();
        let read = source
            .take(size)
            .read_to_end(&mut chunk)
            .map_err(|why| Malformed {
                why: why.to_string(),
            })?;
        if read as u64 != size {
            return Err(Malformed {
                why: format!("a chunk stopped after {read} bytes of {size}"),
            });
        }
        out.append(&mut chunk);
        // The blank line after each chunk.
        if !read_chunk_line(source)?.is_empty() {
            return Err(Malformed {
                why: "a chunk did not end where it said".to_owned(),
            });
        }
    }
}

/// One line of a chunked body's own framing.
fn read_chunk_line(source: &mut impl Read) -> Result<String, Malformed> {
    const LONGEST: usize = 1024;
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        let read = source.read(&mut byte).map_err(|why| Malformed {
            why: why.to_string(),
        })?;
        if read == 0 {
            return Err(Malformed {
                why: "the connection ended in the middle of a chunked body".to_owned(),
            });
        }
        let byte = byte.first().copied().unwrap_or(b'\n');
        if byte == b'\n' {
            break;
        }
        if bytes.len() >= LONGEST {
            return Err(Malformed {
                why: "a chunk line that never ends".to_owned(),
            });
        }
        if byte != b'\r' {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|_| Malformed {
        why: "a chunk line that is not text".to_owned(),
    })
}

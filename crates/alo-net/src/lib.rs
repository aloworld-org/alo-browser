//! Loading: how bytes reach a page.
//!
//! **This lives in the browser process.** ADR 0005 gives a renderer no
//! filesystem, no network and no way to name anything outside itself, so
//! fetching is not a division of labour — it is a privilege boundary. A
//! renderer is *given* a page; this is what gives it one.
//!
//! # The shape first, the network later
//!
//! There is no network in this crate yet, on purpose. A request, a response, a
//! status, headers, a media type and a body are the same whether the bytes came
//! from a socket, a file or the URL itself — so they are built and tested
//! against the two schemes that need nothing (`data:` and `file:`), and HTTP
//! arrives in queue item 53 as **one more way of filling a `Response`** rather
//! than as a second pipeline beside this one.
//!
//! # What is rented and where
//!
//! **Character encodings are `encoding_rs`'s**, and [`encoding`] is the only
//! file that names it. Which byte means which character in
//! `windows-1252`, `shift_jis` or `euc-kr` is a set of tables that took the
//! industry twenty years to agree on, and ADR 0001 is unambiguous about that
//! kind of thing.
//!
//! What is *ours* is the algorithm that decides **which** encoding a page is
//! in, because that is a sequence of rules rather than a table, and getting it
//! wrong shows up as mojibake on somebody's news site.

pub mod body;
pub mod cache;
pub mod certificate;
pub mod connection;
pub mod cookie;
pub mod cors;
pub mod decompress;
pub mod directives;
pub mod encoding;
pub mod fetch;
pub mod freshness;
pub mod h2;
pub mod headers;
pub mod http;
pub mod httpdate;
pub mod jar;
pub mod media_type;
pub mod pool;
pub mod redirect;
pub mod request;
pub mod resolve;
pub mod response;
pub mod schemes;
pub mod tls;

pub use body::Framing;
pub use cache::{Answer, Cache};
pub use certificate::{Fault, Refused};
pub use connection::{Connection, Exchanged, Protocol, exchange};
pub use cookie::{Cookie, Partition, SameSite};
pub use cors::{Credentials, Mode};
pub use decompress::{Encoding, undo, undo_within, what_was_applied};
pub use directives::{Directives, Flag};
pub use encoding::{Decoded, decode, sniff};
pub use fetch::{FetchError, fetch};
pub use freshness::{Stored, Verdict};
pub use headers::Headers;
pub use http::{Head, Malformed, read_head, write_request};
pub use jar::{How, Jar};
pub use media_type::MediaType;
pub use pool::Pool;
pub use redirect::{Next, Refusal, Trail};
pub use request::{Purpose, Request};
pub use resolve::{Reach, Resolver, Unresolved};
pub use response::{Response, Status};
pub use tls::{Secured, TlsError, Trust, secure};

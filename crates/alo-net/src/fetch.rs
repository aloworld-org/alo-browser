//! Fetching, dispatched by scheme.
//!
//! One function, one error, and a `match` that grows by one arm when queue
//! item 53 brings HTTP. That is the whole reason the shape was built before
//! the network: the network is an arm, not a second pipeline.

use crate::request::Request;
use crate::response::Response;
use crate::schemes;
use alo_url::Url;
use core::fmt;

/// Why a fetch did not produce a response.
///
/// A response that *says* it failed — a 404 — is not this. This is nothing
/// having come back at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// A scheme this engine cannot fetch.
    ///
    /// Named rather than silently empty, because "we do not do `ftp:`" and
    /// "the server did not answer" are different things to tell somebody.
    UnsupportedScheme {
        /// Which one.
        scheme: String,
    },
    /// The scheme is one we fetch, and this particular fetch did not work.
    Failed {
        /// What was being fetched.
        url: String,
        /// What went wrong, in words.
        why: String,
    },
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::UnsupportedScheme { scheme } => {
                write!(f, "this browser does not fetch {scheme}: URLs")
            }
            FetchError::Failed { url, why } => write!(f, "could not fetch {url}: {why}"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Fetch one thing.
///
/// # Errors
///
/// [`FetchError`] when nothing came back at all. A server that answered with a
/// 404 is a [`Response`], not an error — the difference matters to everything
/// above, which shows a page for one and a message for the other.
pub fn fetch(request: &Request) -> Result<Response, FetchError> {
    let url = &request.url;
    match url.scheme.as_str() {
        "data" => schemes::data(url).map_err(|why| failed(url, why)),
        "file" => schemes::file(url).map_err(|why| failed(url, why)),
        "http" | "https" => {
            // What this machine trusts, read once per fetch. Queue item 54's
            // pool is where it starts being held rather than re-read.
            let trust = crate::tls::Trust::from_this_machine().map_err(|why| failed(url, why))?;
            crate::connection::fetch(request, &trust).map_err(|why| failed(url, why))
        }
        other => Err(FetchError::UnsupportedScheme {
            scheme: other.to_owned(),
        }),
    }
}

fn failed(url: &Url, why: String) -> FetchError {
    FetchError::Failed {
        url: url.serialised.clone(),
        why,
    }
}

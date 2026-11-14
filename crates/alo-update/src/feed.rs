/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What is asked of the feed, through `alo-net` — the browser process's own
//! network, never Chromium's (ADR 0116 §§ 3 and 5).
//!
//! # What a check sends
//!
//! Two `GET`s, the manifest and its signature; then, for an update, the
//! package's pieces. Each carries **nothing about the person**:
//!
//! - no cookie (the pool is asked directly, never through a jar), no query
//!   string (an address with one is refused, [`crate::address`]), no
//!   identifier, no install id and no version in the address;
//! - `User-Agent: alo-update`, with no version;
//! - `Accept-Encoding: identity`, so a byte range is a range of the file.
//!
//! No report is sent back of success, failure or anything else. Each request
//! carries the cause its asker gives — the person's, who left the switch on
//! or pressed *Check now* — and so is a line in the record as every request
//! is (ADRs 0012, 0060).
//!
//! # Redirects
//!
//! A host such as GitHub's releases answers with a redirect to where the
//! file is kept. At most [`MOST_HOPS`] are followed, each to an address an
//! update may come from ([`crate::address::allowed`]) and never in a
//! circle; the headers above are set again on each hop.

use crate::address;
use alo_net::redirect::{self, Next, Trail};
use alo_net::{Cause, Pool, Request, Response};
use alo_url::Url;

/// The `User-Agent` every request of the updater's says.
pub const USER_AGENT: &str = "alo-update";

/// The most redirects one request follows.
pub const MOST_HOPS: usize = 5;

/// One `GET` of `url`, caused by `cause`, reading at most `most` bytes of
/// its body, with `range` as its `Range` if any — following redirects as
/// this file allows.
///
/// # Errors
///
/// Why, in words: the network's, a redirect refused, or a body longer than
/// `most`.
pub fn get(
    pool: &mut Pool,
    (url, cause): (&Url, &Cause),
    most: u64,
    range: Option<&str>,
) -> Result<Response, String> {
    let mut asking = Request::get(url.clone(), cause.clone()).reading_at_most(most);
    let mut trail = Trail::from(url);
    for _ in 0..=MOST_HOPS {
        asking.headers.replace("User-Agent", USER_AGENT);
        asking.headers.replace("Accept-Encoding", "identity");
        if let Some(range) = range {
            asking.headers.replace("Range", range);
        }
        let response = pool.fetch(&asking)?;
        match redirect::next(&asking, &response).map_err(|refusal| refusal.to_string())? {
            Next::Keep => return Ok(response),
            Next::Follow(hop) => {
                address::allowed(&hop.url)?;
                trail
                    .and_then(&hop.url)
                    .map_err(|refusal| refusal.to_string())?;
                asking = Request::get(hop.url, cause.clone()).reading_at_most(most);
            }
        }
    }
    Err(format!(
        "{} redirected more than {MOST_HOPS} times",
        url.serialised
    ))
}

/// The whole of a small file at `url`: its body, if it answered `200`.
///
/// # Errors
///
/// Why not, in words.
pub fn small(pool: &mut Pool, (url, cause): (&Url, &Cause), most: u64) -> Result<Vec<u8>, String> {
    let response = get(pool, (url, cause), most, None)?;
    if response.status.0 != 200 {
        return Err(format!("{} answered {}", url.serialised, response.status.0));
    }
    Ok(response.body)
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Each site's own mark, fetched by the browser process (queue item 747,
//! ADR 0088).
//!
//! Brave names a result's icon at its own image proxy (`meta_url.favicon`,
//! [`crate::search_protocol`]). This fetches each, one after another, over
//! the search thread's `alo-net` pool, by the rules a thumbnail is fetched
//! by ([`crate::search_thumbnails`], ADR 0086 § 3):
//!
//! - **Only from the proxy.** An icon named anywhere but the one origin the
//!   provider's thumbnails come from is not fetched: showing a site's mark
//!   contacts no site but Brave's — never the site itself, which would learn
//!   from the request that this person searched for something it holds.
//! - **Nothing of the person's.** `Accept: image/*` and what `alo-net`
//!   always sends; no key, no cookie, no `Referer`; no redirect followed.
//! - **Bounded.** Each distinct icon is fetched once, however many results
//!   name it — a tab of one site's videos is one icon — and at most
//!   [`MOST_MARKS`] for one answer; each at most [`LARGEST_MARK`] bytes and
//!   only bytes that begin as a picture. A mark costs no paid request: the
//!   proxy needs no key.
//! - **Never decoded here.** The bytes are a stranger's, decoded only in the
//!   confined renderer that draws them; one that does not decode there is
//!   drawn as the site's letter.
//!
//! A result whose mark is not fetched, or is left out, keeps its letter;
//! why each was left out is kept in words ([`Marked::left_out`]).

use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::results::{LARGEST_MARK, MOST_MARKS};
use alo_url::Url;

/// What came of fetching an answer's marks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marked {
    /// Each result's mark, by its place, if one was fetched and kept.
    pub marks: Vec<Option<Vec<u8>>>,
    /// Why each mark left out was, by the result's place.
    pub left_out: Vec<(usize, String)>,
}

/// Fetch the mark at each of `wanted` through `pool`, from `from`'s origin
/// alone; with no `from`, none. Each distinct address once, and at most
/// [`MOST_MARKS`] of them.
pub fn fetch(
    pool: &mut Pool,
    identities: &mut Identities,
    from: Option<&Url>,
    wanted: &[Option<Url>],
) -> Marked {
    let mut marked = Marked::default();
    // Each address asked, and what came of it.
    let mut asked: Vec<(String, Result<Vec<u8>, String>)> = Vec::new();
    for (at, address) in wanted.iter().enumerate() {
        let Some(address) = address else {
            marked.marks.push(None);
            continue;
        };
        let Some(from) = from else {
            marked.marks.push(None);
            marked.left_out.push((
                at,
                "no image proxy is named, so no mark was fetched".to_owned(),
            ));
            continue;
        };
        // A mark named anywhere but the proxy is no fetch, and counts
        // toward nothing.
        let origin = alo_url::reading::origin(address);
        if origin != alo_url::reading::origin(from) {
            marked.marks.push(None);
            marked.left_out.push((
                at,
                format!(
                    "a mark at {origin}, which is not the search's image proxy, was not fetched"
                ),
            ));
            continue;
        }
        let came = match asked.iter().find(|(each, _)| *each == address.serialised) {
            Some((_, came)) => came.clone(),
            None if asked.len() >= MOST_MARKS => Err(format!(
                "a mark past the first {MOST_MARKS} for one answer was not fetched"
            )),
            None => {
                let came = crate::search_thumbnails::picture(
                    pool,
                    identities,
                    from,
                    address,
                    LARGEST_MARK,
                );
                asked.push((address.serialised.clone(), came.clone()));
                came
            }
        };
        match came {
            Ok(bytes) => marked.marks.push(Some(bytes)),
            Err(why) => {
                marked.marks.push(None);
                marked.left_out.push((at, why));
            }
        }
    }
    marked
}

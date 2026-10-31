/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The thumbnails of a search's images, fetched by the browser process
//! (queue item 743, ADR 0086 § 3).
//!
//! An image search names each image's thumbnail at Brave's own image proxy
//! (`imgs.search.brave.com`). This fetches each, one after another, over
//! the search thread's `alo-net` pool, and keeps what may be drawn:
//!
//! - **Only from the proxy.** A thumbnail whose address is not at the one
//!   origin the provider names ([`crate::brave_search::Provider::thumbnails`])
//!   is not fetched: showing a search's results contacts no site but
//!   Brave's. A redirect is not followed ([`alo_net::Pool::fetch`] follows
//!   none), so an answer of 3xx leaves the image without its picture.
//! - **The person's, with nothing of theirs.** `Accept: image/*` and what
//!   `alo-net` sends with every request; no key, no cookie, no `Referer`.
//! - **Bounded.** At most [`MOST_IMAGES`] — the read already keeps no more —
//!   and each at most [`LARGEST_THUMBNAIL`] bytes; a larger answer is left
//!   out. `alo-net` reads a body whole before it is weighed (bounded by its
//!   own most and by the pool's patience).
//! - **Only a picture.** Bytes that do not begin as a picture this engine
//!   reads ([`Format::of`]) are left out. They are never decoded here: the
//!   bytes are a stranger's, and are decoded only in the confined renderer
//!   that draws them, by the picture readers every `<img>` goes through
//!   (ADR 0005). A thumbnail that begins as a picture and does not decode
//!   is drawn there as its tile's colour.
//!
//! An image whose thumbnail is left out is still shown, as its title in
//! its tile; why each was left out is kept in words ([`Fetched::left_out`]).

use crate::search_protocol::ImageFound;
use alo_net::Pool;
use alo_net::Request;
use alo_net::cause::{Cause, Identities};
use alo_paint::picture::Format;
use alo_renderer::results::{LARGEST_THUMBNAIL, MOST_IMAGES, Pictured};
use alo_url::Url;

/// What came of fetching a search's thumbnails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The images, best first, each holding its thumbnail if it was kept.
    pub images: Vec<Pictured>,
    /// Why each thumbnail left out was, by the image's place.
    pub left_out: Vec<(usize, String)>,
}

/// Fetch the thumbnail of each of `found` — at most [`MOST_IMAGES`] —
/// through `pool`, from `from`'s origin alone.
pub fn fetch(
    pool: &mut Pool,
    identities: &mut Identities,
    from: &Url,
    found: Vec<ImageFound>,
) -> Fetched {
    let mut fetched = Fetched {
        images: Vec::new(),
        left_out: Vec::new(),
    };
    for (at, image) in found.into_iter().take(MOST_IMAGES).enumerate() {
        let ImageFound {
            mut pictured,
            thumbnail,
        } = image;
        if let Some(thumbnail) = thumbnail {
            match picture(pool, identities, from, &thumbnail) {
                Ok(bytes) => pictured.picture = Some(bytes),
                Err(why) => fetched.left_out.push((at, why)),
            }
        }
        fetched.images.push(pictured);
    }
    fetched
}

/// The bytes of the thumbnail at `thumbnail`, if it is at `from`'s origin,
/// answered 200, is not larger than [`LARGEST_THUMBNAIL`] and begins as a
/// picture this engine reads.
///
/// # Errors
///
/// Why not, in words.
fn picture(
    pool: &mut Pool,
    identities: &mut Identities,
    from: &Url,
    thumbnail: &Url,
) -> Result<Vec<u8>, String> {
    let origin = alo_url::reading::origin(thumbnail);
    if origin != alo_url::reading::origin(from) {
        return Err(format!(
            "a thumbnail at {origin}, which is not the search's image proxy, was not fetched"
        ));
    }
    let mut request = Request::get(
        thumbnail.clone(),
        Cause::Person {
            tab: identities.a_tab(),
        },
    );
    request.headers.add("Accept", "image/*");
    let response = pool
        .fetch(&request)
        .map_err(|why| format!("a thumbnail could not be fetched: {why}"))?;
    if response.status.0 != 200 {
        return Err(format!("a thumbnail was answered {}", response.status.0));
    }
    let bytes = response.body;
    if bytes.len() > LARGEST_THUMBNAIL {
        return Err(format!(
            "a thumbnail of {} bytes was left out; at most {LARGEST_THUMBNAIL} are kept",
            bytes.len()
        ));
    }
    if Format::of(&bytes).is_none() {
        return Err("a thumbnail that is not a picture this engine reads was left out".to_owned());
    }
    Ok(bytes)
}

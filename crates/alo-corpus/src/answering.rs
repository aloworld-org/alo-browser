/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A case's fetches, answered from what it froze (ADR 0032 § 7, queue item
//! 335).
//!
//! A corpus case never touches the network (`LOOP.md`, stage 2 § 1), so a
//! page whose script fetches is answered here, in the browser process's
//! place, **through the browser process's own two steps**: each ask is
//! decided by `alo-renderer`'s `fetch_decide` from the case's own address,
//! and each frozen response is filtered by its `fetch_filter` before the
//! renderer is sent it. A frozen cross-origin response is therefore as
//! opaque to the page as a live one, and an ask the browser would refuse is
//! refused here by the same rule.
//!
//! **A fetch the case froze no response for is a network error**, as it is
//! for a browser that is offline, and its URL is answered back so that the
//! case's `origin.txt` can say which ([`Answered::unfrozen`]). A reference
//! render of a page that greys its buttons when a file is missing is then
//! read as what the page does offline.
//!
//! A frozen response is a file holding the bytes an HTTP/1.1 server sent —
//! its status line, its headers, a blank line and its body, decoded of any
//! transfer coding — read with `alo-net`'s own reader of a head. Only final
//! answers are frozen: a redirect the page asked to follow would need the
//! response it led to, and a case freezing one is refused by name rather
//! than answered half way.

use std::collections::VecDeque;
use std::io::Cursor;

use alo_net::cause::{Cause, Identities};
use alo_net::csp::Policies;
use alo_net::http;
use alo_net::redirect;
use alo_net::response::Response;
use alo_renderer::fetch::{FetchAsk, Fetched};
use alo_renderer::fetch_decide::{self, Asker, Decided};
use alo_renderer::fetch_filter;
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_url::Url;

/// The most answers one case is delivered before it is refused as a page
/// that fetches for ever.
///
/// Every answer is a task whose reactions may ask again, so a page can keep
/// a loop of fetches going indefinitely, and a suite cannot wait for one to
/// stop. Two hundred and fifty-six is four times what the browser process
/// lets one document have in flight at once (`MOST_IN_FLIGHT`), and far
/// more than any frozen page has needed.
pub const MOST_ANSWERED: usize = 256;

/// What answering a case's fetches came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answered {
    /// How many answers were delivered.
    pub delivered: usize,
    /// The URLs fetched that the case froze no response for, each answered
    /// as a network error, in the order they were asked for.
    pub unfrozen: Vec<String>,
    /// What the deliveries' tasks said.
    pub issues: Vec<String>,
}

/// Answer every ask in `asks`, and every ask their answers' reactions make
/// in turn, from `frozen` — the URL each response was taken from, and its
/// bytes — for the page `renderer` holds at `address`.
///
/// # Errors
///
/// What makes a case unanswerable: a frozen response that does not read as
/// one, a frozen redirect, a renderer that did not answer a delivery as
/// one, or a page still asking after [`MOST_ANSWERED`] answers.
pub fn answer(
    renderer: &mut Renderer,
    address: &Url,
    frozen: &[(String, Vec<u8>)],
    asks: Vec<FetchAsk>,
) -> Result<Answered, String> {
    let policies = Policies::none();
    let asker = Asker {
        url: address,
        policies: &policies,
    };
    let cause = Cause::Document {
        document: Identities::default().a_document(),
    };
    let mut waiting: VecDeque<FetchAsk> = asks.into();
    let mut answered = Answered::default();
    while let Some(ask) = waiting.pop_front() {
        if answered.delivered >= MOST_ANSWERED {
            return Err(format!(
                "its page was still fetching after {MOST_ANSWERED} answers"
            ));
        }
        let fetched = match fetch_decide::decide(&ask, &asker, &cause) {
            Decided::Refused(refusal) => refusal.answer(),
            Decided::Make(fetch) => {
                let url = fetch.request.url.serialised.clone();
                if let Some((_, bytes)) = frozen.iter().find(|(held, _)| *held == url) {
                    frozen_answer(&fetch, bytes)?
                } else {
                    answered.unfrozen.push(url);
                    Fetched::failed(fetch.number)
                }
            }
        };
        match renderer.handle(ToRenderer::Fetched(Box::new(fetched))) {
            FromRenderer::Delivered {
                mut issues,
                fetches,
                ..
            } => {
                answered.issues.append(&mut issues);
                waiting.extend(fetches);
            }
            other => return Err(format!("a delivery was answered with {other:?}")),
        }
        answered.delivered = answered.delivered.saturating_add(1);
    }
    Ok(answered)
}

/// The frozen response `bytes`, filtered for the page that asked as `fetch`.
fn frozen_answer(fetch: &fetch_decide::Fetch, bytes: &[u8]) -> Result<Fetched, String> {
    let mut reading = Cursor::new(bytes);
    let head = http::read_head(&mut reading)
        .map_err(|why| format!("a frozen response is not one: {why}"))?;
    let body = bytes
        .get(usize::try_from(reading.position()).unwrap_or(usize::MAX)..)
        .unwrap_or_default()
        .to_vec();
    if head.status.is_redirect() && fetch.redirect == redirect::Mode::Follow {
        return Err(format!(
            "the response frozen for {} is a redirect, and the corpus freezes only final answers",
            fetch.request.url.serialised
        ));
    }
    let response = Response {
        url: fetch.request.url.clone(),
        status: head.status,
        headers: head.headers,
        body,
    };
    Ok(fetch_filter::filter(fetch, &response, &head.reason, false).fetched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_layout::Size;
    use alo_renderer::Page;

    const PAGE: &str = "<p id=out>waiting</p><script>
        var out = document.getElementById('out');
        fetch('/frozen.txt').then(function (r) { return r.text(); })
          .then(function (t) {
            out.textContent = t;
            return fetch('https://elsewhere.example/a', { mode: 'no-cors' });
          })
          .then(function (r) { out.textContent += ' ' + r.type + ' ' + r.status; })
          .then(function () { return fetch('/missing'); })
          .catch(function (e) { out.textContent += ' / ' + e.name; });
        </script>";

    fn loaded(renderer: &mut Renderer, address: &Url) -> Vec<FetchAsk> {
        let mut page = Page::new(PAGE, Size::new(200.0, 50.0));
        page.url = address.clone();
        match renderer.handle(ToRenderer::Load(Box::new(page))) {
            FromRenderer::Loaded { fetches, .. } => fetches,
            other => panic!("not loaded: {other:?}"),
        }
    }

    fn text(renderer: &Renderer) -> String {
        let Some(document) = renderer.document() else {
            panic!("no document");
        };
        document
            .descendants(document.root())
            .find(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.attr("id") == Some("out"))
            })
            .map(|node| document.text_content(node))
            .unwrap_or_default()
    }

    #[test]
    fn a_page_is_answered_from_what_was_frozen_and_offline_for_the_rest() {
        let address = alo_url::parse("https://example.com/page").unwrap();
        let mut renderer = Renderer::new(crate::corpus_fonts());
        let asks = loaded(&mut renderer, &address);
        assert_eq!(asks.len(), 1);
        let frozen = vec![
            (
                "https://example.com/frozen.txt".to_owned(),
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nthawed".to_vec(),
            ),
            (
                "https://elsewhere.example/a".to_owned(),
                b"HTTP/1.1 200 OK\r\n\r\nsecret".to_vec(),
            ),
        ];
        let answered = answer(&mut renderer, &address, &frozen, asks).unwrap();
        assert_eq!(answered.delivered, 3);
        assert_eq!(answered.unfrozen, ["https://example.com/missing"]);
        assert_eq!(text(&renderer), "thawed opaque 0 / TypeError");
    }

    #[test]
    fn a_frozen_redirect_or_a_broken_response_is_refused_by_name() {
        let address = alo_url::parse("https://example.com/page").unwrap();
        for (bytes, said) in [
            (
                &b"HTTP/1.1 302 Found\r\nLocation: /x\r\n\r\n"[..],
                "a redirect",
            ),
            (&b"not http at all"[..], "is not one"),
        ] {
            let mut renderer = Renderer::new(crate::corpus_fonts());
            let asks = loaded(&mut renderer, &address);
            let frozen = vec![("https://example.com/frozen.txt".to_owned(), bytes.to_vec())];
            let refused = answer(&mut renderer, &address, &frozen, asks).unwrap_err();
            assert!(refused.contains(said), "{refused}");
        }
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A case's fetches and style sheets, answered from what it froze (ADR 0032
//! § 7, queue item 335; ADR 0035 § 6, queue item 349).
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
//!
//! # Its style sheets
//!
//! A loaded page **asks** for its linked sheets as it asks for a fetch, and
//! each ask is answered from the files its `linked.txt` froze, by
//! [`crate::sheets`] — decided and checked by the browser process's own rules
//! there. Sheets are answered ahead of fetches, as the browser process queues
//! a document's sheets ahead of its fetches (`fetch_answering`), and every
//! answer is drawn from: the reference is the page after all of them, as the
//! window shows it once ADR 0035 § 5 lets it.
//!
//! **A file frozen for a URL the page never asked for is refused by name.**
//! A loaded page asks only for its sheets — its pictures are not asked for
//! until queue item 350 — so a case freezing one would be rendered without it
//! and committed that way.

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
use alo_renderer::sheet::SheetAsk;
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_url::Url;

use crate::sheets;

/// The most answers one case is delivered before it is refused as a page
/// that fetches for ever.
///
/// Every answer is a task whose reactions may ask again, so a page can keep
/// a loop of fetches going indefinitely, and a suite cannot wait for one to
/// stop. Two hundred and fifty-six is four times what the browser process
/// lets one document have in flight at once (`MOST_IN_FLIGHT`), and far
/// more than any frozen page has needed.
pub const MOST_ANSWERED: usize = 256;

/// What answering a case's fetches and sheets came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answered {
    /// How many answers were delivered, fetches and sheets together.
    pub delivered: usize,
    /// How many of them were style sheets.
    pub sheets: usize,
    /// The URLs fetched or linked that the case froze nothing for, each
    /// answered as a network error or a sheet that did not arrive, in the
    /// order they were answered.
    pub unfrozen: Vec<String>,
    /// What the deliveries' tasks said.
    pub issues: Vec<String>,
    /// What the load itself said, before any answer: its scripts, and its
    /// first draw. Filled by whoever loaded the page ([`crate::Rendering`]),
    /// since the load's answer is not this module's to read.
    pub loaded: Vec<String>,
    /// What the browser process would have told the person about the page's
    /// sheets: why one did not arrive, or the `charset` one was not read in.
    pub said: Vec<String>,
}

/// What a renderer's answer asked for, still to be answered.
#[derive(Debug, Clone, Default)]
pub struct Asks {
    /// The page's fetches, in the order it asked.
    pub fetches: Vec<FetchAsk>,
    /// The page's linked style sheets, in document order.
    pub sheets: Vec<SheetAsk>,
}

/// What a case froze beside its page to answer it with.
#[derive(Debug, Clone, Copy)]
pub struct Beside<'a> {
    /// Each response, by the URL it was taken from.
    pub responses: &'a [(String, Vec<u8>)],
    /// Each file its `linked.txt` names.
    pub files: &'a [crate::case::Frozen],
}

/// Answer every ask in `asks`, and every ask their answers' reactions make
/// in turn, from what was `frozen`, for the page `renderer` holds at
/// `address`.
///
/// # Errors
///
/// What makes a case unanswerable: a frozen response that does not read as
/// one, a frozen redirect, a renderer that did not answer a delivery as
/// one, a page still asking after [`MOST_ANSWERED`] answers, or a file
/// frozen for a URL the page never asked for.
pub fn answer(
    renderer: &mut Renderer,
    address: &Url,
    frozen: Beside<'_>,
    asks: Asks,
) -> Result<Answered, String> {
    let policies = Policies::none();
    let asker = Asker {
        url: address,
        policies: &policies,
    };
    let cause = Cause::Document {
        document: Identities::default().a_document(),
    };
    let mut fetches: VecDeque<FetchAsk> = asks.fetches.into();
    let mut linked: VecDeque<SheetAsk> = asks.sheets.into();
    let mut asked_for = Vec::new();
    let mut answered = Answered::default();
    loop {
        let message = if let Some(ask) = linked.pop_front() {
            if answered.delivered >= MOST_ANSWERED {
                return Err(still_asking());
            }
            asked_for.push(ask.url.clone());
            let sheet = sheets::answer(&ask, &asker, &cause, frozen.files);
            answered.unfrozen.extend(sheet.unfrozen);
            answered.said.extend(sheet.said);
            answered.sheets = answered.sheets.saturating_add(1);
            ToRenderer::Sheet(Box::new(sheet.answer))
        } else if let Some(ask) = fetches.pop_front() {
            if answered.delivered >= MOST_ANSWERED {
                return Err(still_asking());
            }
            ToRenderer::Fetched(Box::new(fetched(
                &ask,
                &asker,
                &cause,
                frozen.responses,
                &mut answered,
            )?))
        } else {
            break;
        };
        match renderer.handle(message) {
            FromRenderer::Delivered {
                mut issues,
                fetches: more,
                sheets: more_sheets,
                ..
            } => {
                answered.issues.append(&mut issues);
                fetches.extend(more);
                linked.extend(more_sheets);
            }
            other => return Err(format!("a delivery was answered with {other:?}")),
        }
        answered.delivered = answered.delivered.saturating_add(1);
    }
    if let Some(file) = frozen.files.iter().find(|file| {
        sheets::served_at(address, &file.name).is_none_or(|url| !asked_for.contains(&url))
    }) {
        return Err(format!(
            "its linked.txt freezes {:?} as {}, which its page never asked for: a page loaded \
             by a renderer asks only for its style sheets, and for no picture until queue item \
             350",
            file.name, file.file
        ));
    }
    Ok(answered)
}

/// A page that has not stopped asking.
fn still_asking() -> String {
    format!("its page was still fetching after {MOST_ANSWERED} answers")
}

/// The answer to the fetch `ask`, from `responses`, with a URL the case froze
/// nothing for noted in `answered`.
fn fetched(
    ask: &FetchAsk,
    asker: &Asker<'_>,
    cause: &Cause,
    responses: &[(String, Vec<u8>)],
    answered: &mut Answered,
) -> Result<Fetched, String> {
    Ok(match fetch_decide::decide(ask, asker, cause) {
        Decided::Refused(refusal) => refusal.answer(),
        Decided::Make(fetch) => {
            let url = fetch.request.url.serialised.clone();
            if let Some((_, bytes)) = responses.iter().find(|(held, _)| *held == url) {
                frozen_answer(&fetch, bytes)?
            } else {
                answered.unfrozen.push(url);
                Fetched::failed(fetch.number)
            }
        }
    })
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
        reason: head.reason,
        headers: head.headers,
        body,
    };
    // No redirect is followed here, so the answer left the page's origin
    // exactly when the ask went to another.
    let route = fetch_filter::Route {
        redirected: false,
        left_the_origin: fetch.is_cross_origin(),
        tainted: false,
    };
    Ok(fetch_filter::filter(fetch, &response, route).fetched)
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

    fn beside(responses: &[(String, Vec<u8>)]) -> Beside<'_> {
        Beside {
            responses,
            files: &[],
        }
    }

    fn asked(fetches: Vec<FetchAsk>) -> Asks {
        Asks {
            fetches,
            sheets: Vec::new(),
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
        let answered = answer(&mut renderer, &address, beside(&frozen), asked(asks)).unwrap();
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
            let refused =
                answer(&mut renderer, &address, beside(&frozen), asked(asks)).unwrap_err();
            assert!(refused.contains(said), "{refused}");
        }
    }
}

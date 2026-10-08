/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Making a style sheet the browser process decided a page may have, and
//! sending the renderer only a style sheet (ADR 0035 § 3, queue item 348).
//!
//! [`crate::sheet_decide`] says whether; this makes the request through the
//! same hops as a page's fetch ([`crate::fetch_make::hops`]) — every hop's
//! `Referer` and cookies decided again, a `cors` sheet asked about and checked
//! as a fetch is, a redirect judged by `style-src` with the `<link>`'s nonce,
//! each request a line in the session's record — and then applies the one rule
//! that is a sheet's own.
//!
//! # Only a style sheet crosses
//!
//! The body is sent **only when the response is a style sheet**: its status
//! is in the 200 range and its `Content-Type`'s essence is `text/css`. HTML
//! requires exactly this of every sheet in a document that is not in quirks
//! mode, and this engine has no quirks mode (law 1), so it is required of all
//! of them, same-origin as well.
//!
//! It is what keeps `<link rel=stylesheet>` from being a way to read another
//! site. A `no-cors` sheet's body must cross whatever origin it is from — no
//! page using a sheet from another host would have style otherwise — so the
//! same-origin policy cannot be the filter. This is: the only cross-origin
//! bytes a renderer ever holds from a style request are bytes their server
//! said were CSS, which are bytes it published to be applied by anybody. An
//! error page, a redirect to a login form, a JSON document, a page a hostile
//! `href` named — each is a failure, and **its bytes never leave this
//! process**.
//!
//! A failure crosses with no reason, and the reason comes back beside it in
//! [`Made::said`] for whoever shows the person. A body too large to cross in
//! one message is a failure, not a truncation.
//!
//! # A `file:` sheet
//!
//! Read off this machine, beside a `file:` page that linked it, typed by its
//! file's extension as `alo-net` types a file — the one way a file can say it
//! is a sheet — and held to the same rule. It goes over no network, so it is
//! not a line in the session's record, which is a record of what was asked of
//! the network; it is the same read `alo-window`'s `opening.rs` makes of a
//! sheet named on the command line.

use alo_net::response::Response;

use crate::fetch_decide::Fetch;
use crate::fetch_make::{Network, hops};
use crate::sheet::SheetAnswer;
use crate::wire::{LARGEST_MESSAGE, sheet_answer_size};

/// What making a sheet came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// What the renderer is sent.
    pub answer: SheetAnswer,
    /// What to tell the person: why it did not arrive, when it did not, each
    /// cookie a server set that was not kept, and a `charset` the sheet was
    /// not read in. Never for the page.
    pub said: Vec<String>,
}

/// Make `sheet` through `network`, and answer with its bytes only if they are
/// a style sheet.
pub fn make(sheet: &Fetch, network: &mut Network) -> Made {
    let mut said = Vec::new();
    let url = &sheet.request.url;
    let response = if url.scheme == "file" {
        alo_net::schemes::file(url)
    } else {
        hops(sheet, network, &mut said).map(|(response, _)| response)
    };
    let answer = match response.and_then(|response| style_sheet(sheet.number, response)) {
        Ok((answer, charset)) => {
            if let Some(charset) = charset {
                said.push(format!(
                    "the page's style sheet at {url} was sent as {charset:?}, and was read as \
                     UTF-8, the only encoding this browser reads a style sheet in"
                ));
            }
            answer
        }
        Err(why) => {
            said.push(format!(
                "the page's style sheet at {url} did not arrive: {why}"
            ));
            SheetAnswer::failed(sheet.number)
        }
    };
    Made { answer, said }
}

/// `response` as the answer to sheet `number` — with the `charset` it was
/// sent in, when that is not UTF-8 — if it is a style sheet that fits in one
/// message; or why it is not one.
fn style_sheet(number: u64, response: Response) -> Result<(SheetAnswer, Option<String>), String> {
    if !response.status.is_ok() {
        return Err(format!(
            "the server answered {}, and only a success is a style sheet",
            response.status
        ));
    }
    let media = response.media_type();
    let essence = media.as_ref().map(alo_net::media_type::MediaType::essence);
    if essence.as_deref() != Some("text/css") {
        return Err(match essence {
            Some(essence) => {
                format!("it was sent as {essence}, and only text/css is applied as a style sheet")
            }
            None => {
                "it was sent with no type, and only text/css is applied as a style sheet".to_owned()
            }
        });
    }
    let charset = media
        .as_ref()
        .and_then(alo_net::media_type::MediaType::charset)
        .filter(|charset| {
            !charset.eq_ignore_ascii_case("utf-8") && !charset.eq_ignore_ascii_case("utf8")
        })
        .map(ToOwned::to_owned);
    let answer = SheetAnswer {
        number,
        bytes: Some(response.body),
    };
    let size = sheet_answer_size(&answer);
    if size > LARGEST_MESSAGE {
        return Err(format!(
            "it is {size} bytes as a message, and one message may carry at most {LARGEST_MESSAGE}"
        ));
    }
    Ok((answer, charset))
}

#[cfg(test)]
mod tests {
    use alo_net::Status;
    use alo_net::cause::{Cause, Identities};
    use alo_net::cors::{Credentials, Mode};
    use alo_net::csp::Policies;
    use alo_net::{Pool, Trust};
    use alo_url::Url;

    use super::*;
    use crate::fetch_decide::Asker;
    use crate::sheet::SheetAsk;
    use crate::sheet_decide::{Decided, decide};

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    fn answered(status: u16, content_type: Option<&str>, body: &[u8]) -> Response {
        let mut response = Response::ok(url("https://bank.example/statement"), body.to_vec());
        response.status = Status(status);
        if let Some(content_type) = content_type {
            response.headers.add("Content-Type", content_type);
        }
        response
    }

    #[test]
    fn only_a_successful_text_css_answer_is_a_style_sheet() {
        let (answer, charset) = style_sheet(3, answered(200, Some("text/css"), b"p{}")).unwrap();
        assert_eq!(answer.bytes.as_deref(), Some(&b"p{}"[..]));
        assert_eq!(charset, None);
        let (_, charset) = style_sheet(
            3,
            answered(203, Some(" TEXT/CSS ; charset=Windows-1252"), b""),
        )
        .unwrap();
        assert_eq!(charset.as_deref(), Some("Windows-1252"));
        assert!(
            style_sheet(3, answered(200, Some("text/css; charset=UTF-8"), b""))
                .unwrap()
                .1
                .is_none()
        );
        for (status, content_type, why) in [
            (200, Some("text/html"), "sent as text/html"),
            (200, Some("application/json"), "application/json"),
            (200, Some("text/plain"), "text/plain"),
            (200, None, "no type"),
            (200, Some("css"), "no type"),
            (404, Some("text/css"), "404"),
            (302, Some("text/css"), "302"),
            (500, Some("text/css"), "500"),
        ] {
            let refused = style_sheet(3, answered(status, content_type, b"<secret/>")).unwrap_err();
            assert!(
                refused.contains(why),
                "{status} {content_type:?}: {refused}"
            );
        }
    }

    #[test]
    fn a_sheet_too_large_for_one_message_is_a_failure_not_a_truncation() {
        let body = vec![b' '; LARGEST_MESSAGE];
        let refused = style_sheet(3, answered(200, Some("text/css"), &body)).unwrap_err();
        assert!(refused.contains("one message may carry"), "{refused}");
    }

    fn network() -> Network {
        Network::over(Pool::with_trust(Trust::of(&[]).unwrap()))
    }

    fn decided_file(page: &Url, sheet: &Url) -> Fetch {
        let policies = Policies::none();
        let ask = SheetAsk {
            number: 6,
            url: sheet.serialised.clone(),
            mode: Mode::NoCors,
            credentials: Credentials::Include,
            referrer: None,
            nonce: None,
        };
        let cause = Cause::Document {
            document: Identities::default().a_document(),
        };
        match decide(
            &ask,
            &Asker {
                url: page,
                policies: &policies,
            },
            &cause,
        ) {
            Decided::Make(sheet) => *sheet,
            Decided::Refused(refusal) => panic!("{refusal}"),
        }
    }

    #[test]
    fn a_file_pages_sheet_is_read_off_the_machine_and_held_to_the_same_rule() {
        let folder = std::env::temp_dir().join(format!("alo-sheet-make-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("site.css"), "p { color: red }").unwrap();
        std::fs::write(folder.join("notes.txt"), "a secret").unwrap();
        let at = |name: &str| url(&format!("file://{}", folder.join(name).display()));
        let page = at("index.html");

        let made = make(&decided_file(&page, &at("site.css")), &mut network());
        assert_eq!(made.answer.bytes.as_deref(), Some(&b"p { color: red }"[..]));
        assert!(made.said.is_empty(), "{:?}", made.said);

        let made = make(&decided_file(&page, &at("notes.txt")), &mut network());
        assert_eq!(
            made.answer,
            SheetAnswer::failed(6),
            "a text file's bytes stay here"
        );
        assert!(made.said[0].contains("text/plain"), "{:?}", made.said);

        let made = make(&decided_file(&page, &at("gone.css")), &mut network());
        assert_eq!(made.answer, SheetAnswer::failed(6));
        assert!(made.said[0].contains("did not arrive"), "{:?}", made.said);
        let _ = std::fs::remove_dir_all(&folder);
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A loaded case's style sheets, answered from the files frozen beside it
//! (ADR 0035 § 6, queue item 349).
//!
//! A page whose script runs is loaded by a renderer, and a renderer is handed
//! no sheet: it **asks** for each `<link rel=stylesheet>` by the URL it
//! resolved (ADR 0035 § 1). The browser process decides the ask and sends the
//! bytes only if they are a style sheet. Here the corpus stands in the browser
//! process's place, through **its own two steps**: `alo-renderer`'s
//! `sheet_decide` decides the ask from the case's address, and its
//! `sheet_make::style_sheet` — the rule that only a 2xx `text/css` answer
//! crosses — is applied to what was frozen. A frozen file that is not a style
//! sheet is therefore refused here exactly as a live one would be.
//!
//! **What was frozen is found by URL**: each name in the case's `linked.txt`
//! resolved against its `address.txt`, so `/assets/site.css` beside a page
//! served from `https://nordwind.alosites.com/` answers an ask for
//! `https://nordwind.alosites.com/assets/site.css`. A frozen file had no
//! response, so its status is `200` and its type is what its file name's
//! extension stands for, as `alo-net` types a `file:` URL — the same guess,
//! from the same table.
//!
//! **A sheet the case froze nothing for did not arrive**, as for a browser
//! that is offline, and its URL is answered back so the case can say which
//! ([`Sheet::unfrozen`]).

use std::path::Path;

use alo_net::cause::Cause;
use alo_net::response::Response;
use alo_renderer::fetch_decide::Asker;
use alo_renderer::sheet::{SheetAnswer, SheetAsk};
use alo_renderer::sheet_decide::{self, Decided};
use alo_renderer::sheet_make;
use alo_url::Url;

use crate::case::Frozen;

/// What became of one sheet a loaded case's page asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// What the renderer is sent.
    pub answer: SheetAnswer,
    /// The URL asked for, when the case froze nothing for it.
    pub unfrozen: Option<String>,
    /// What the browser process would tell the person: why it did not
    /// arrive, or the `charset` it was not read in. Never for the page.
    pub said: Option<String>,
}

/// Where the frozen file called `name` would have been served from, for a
/// page at `address`.
pub fn served_at(address: &Url, name: &str) -> Option<String> {
    alo_url::join(address, name).ok().map(|url| url.serialised)
}

/// Answer `ask`, from the page at `asker` under `cause`, with what `frozen`
/// holds for the URL it names.
pub fn answer(ask: &SheetAsk, asker: &Asker<'_>, cause: &Cause, frozen: &[Frozen]) -> Sheet {
    let sheet = match sheet_decide::decide(ask, asker, cause) {
        Decided::Make(sheet) => sheet,
        Decided::Refused(refusal) => {
            return Sheet {
                answer: refusal.answer(),
                unfrozen: None,
                said: Some(refusal.to_string()),
            };
        }
    };
    let url = &sheet.request.url;
    let Some(file) = frozen
        .iter()
        .find(|file| served_at(asker.url, &file.name).as_deref() == Some(&url.serialised))
    else {
        return Sheet {
            answer: SheetAnswer::failed(sheet.number),
            unfrozen: Some(url.serialised.clone()),
            said: None,
        };
    };
    let mut response = Response::ok(url.clone(), file.bytes.clone());
    if let Some(media) = alo_net::schemes::from_extension(Path::new(&file.file)) {
        response.headers.add("Content-Type", media);
    }
    match sheet_make::style_sheet(sheet.number, response) {
        Ok((answer, charset)) => Sheet {
            answer,
            unfrozen: None,
            said: charset.map(|charset| {
                format!(
                    "the page's style sheet at {url} was sent as {charset:?}, and was read as \
                     UTF-8, the only encoding this browser reads a style sheet in"
                )
            }),
        },
        Err(why) => Sheet {
            answer: SheetAnswer::failed(sheet.number),
            unfrozen: None,
            said: Some(format!(
                "the page's style sheet at {url} did not arrive: {why}"
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::cause::Identities;
    use alo_net::cors::{Credentials, Mode};
    use alo_net::csp::Policies;

    const PAGE: &str = "https://nordwind.alosites.com/";

    fn ask(url: &str) -> SheetAsk {
        SheetAsk {
            number: 4,
            url: url.to_owned(),
            mode: Mode::NoCors,
            credentials: Credentials::Include,
            referrer: None,
            nonce: None,
        }
    }

    fn frozen(name: &str, file: &str, bytes: &[u8]) -> Frozen {
        Frozen {
            name: name.to_owned(),
            file: file.to_owned(),
            bytes: bytes.to_vec(),
        }
    }

    fn answered(url: &str, files: &[Frozen]) -> Sheet {
        let page = alo_url::parse(PAGE).unwrap();
        let policies = Policies::none();
        let asker = Asker {
            url: &page,
            policies: &policies,
        };
        let cause = Cause::Document {
            document: Identities::default().a_document(),
        };
        answer(&ask(url), &asker, &cause, files)
    }

    #[test]
    fn a_frozen_sheet_answers_the_url_its_name_resolves_to() {
        let files = [frozen("/assets/site.css", "site.css", b"p{}")];
        let sheet = answered("https://nordwind.alosites.com/assets/site.css", &files);
        assert_eq!(sheet.answer.number, 4);
        assert_eq!(sheet.answer.bytes.as_deref(), Some(&b"p{}"[..]));
        assert_eq!(sheet.unfrozen, None);
        assert_eq!(sheet.said, None);
    }

    #[test]
    fn a_sheet_frozen_for_nothing_did_not_arrive_and_its_url_is_said() {
        let files = [frozen("/assets/site.css", "site.css", b"p{}")];
        let url = "https://nordwind.alosites.com/assets/other.css";
        let sheet = answered(url, &files);
        assert_eq!(sheet.answer, SheetAnswer::failed(4));
        assert_eq!(sheet.unfrozen.as_deref(), Some(url));
    }

    #[test]
    fn a_frozen_file_that_is_not_a_style_sheet_does_not_cross() {
        // A name proves nothing; the file's extension is all a frozen file
        // has for a type, and only `text/css` is a style sheet.
        for (file, sent_as) in [
            ("statement.html", "text/html"),
            ("notes.txt", "text/plain"),
            ("unknown.bin", "no type"),
        ] {
            let files = [frozen("/a.css", file, b"<secret/>")];
            let sheet = answered("https://nordwind.alosites.com/a.css", &files);
            assert_eq!(sheet.answer, SheetAnswer::failed(4), "{file}");
            let said = sheet.said.unwrap_or_default();
            assert!(said.contains(sent_as), "{file}: {said}");
            assert_eq!(sheet.unfrozen, None);
        }
    }

    #[test]
    fn an_ask_the_browser_process_refuses_is_refused_here_by_the_same_rule() {
        // Insecure content on a secure page, and a scheme no sheet comes over:
        // refused before anything frozen is looked at.
        let files = [frozen("http://cdn.example/a.css", "a.css", b"p{}")];
        for url in ["http://cdn.example/a.css", "ftp://cdn.example/a.css"] {
            let sheet = answered(url, &files);
            assert_eq!(sheet.answer, SheetAnswer::failed(4), "{url}");
            assert!(
                sheet.said.unwrap_or_default().contains("was refused"),
                "{url}"
            );
            assert_eq!(sheet.unfrozen, None);
        }
    }
}

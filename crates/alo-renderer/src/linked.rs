/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A document's linked style sheets, as its renderer keeps them: what it
//! asked for, what arrived, and what it said about a link it would not ask
//! for (ADR 0035 §§ 1, 3 and 4, queue item 348).
//!
//! # Asking
//!
//! At the end of each message's work the renderer reads the document's
//! `<link rel=stylesheet>`s as `alo-dom`'s `asked_for` finds them, resolves
//! each `href` against the document's base URL as a link's is resolved (ADR
//! 0020 § 2), and asks for **each URL it has not asked for before in this
//! document** ([`Linked::asks`]): two links to one sheet are one request, and
//! a link a script adds is asked for in the answer to the task that added it.
//!
//! Before it asks, three refusals are the renderer's, each said by name and
//! once:
//!
//! - **a `data:` sheet**, which is the page's own bytes and the renderer's to
//!   read when a page needs one — never a request;
//! - **a link with `integrity`**, until Subresource Integrity is built:
//!   applying unchecked a sheet whose author asked for it to be checked would
//!   be quietly weaker than the page asked for;
//! - **a link the page's `<meta>` policy refuses**, since only the renderer
//!   has seen that policy. A header policy is the browser process's to apply
//!   from its own copy ([`crate::sheet_decide`]), so it is not asked here,
//!   and its refusal is recorded there.
//!
//! At most [`MOST_SHEETS`] are asked for in a document's life; a link past
//! that is counted and said, as a change in the count.
//!
//! # What arrives
//!
//! A sheet's bytes, which crossed only because the browser process found them
//! to be a style sheet (ADR 0035 § 3), are decoded **as UTF-8** — a byte order
//! mark removed, an invalid sequence replaced — and kept by URL for the life of
//! the document ([`Linked::arrived`]). An `@charset` naming another encoding is
//! said and ignored: legacy encodings are stage 3's (queue item 138). A sheet
//! that did not arrive is said in the same words whatever happened.
//!
//! Every draw applies what arrived to whichever links the document has *then*
//! ([`Linked::for_draw`]), so a `<link>` removed since is not applied and a
//! second link to the same URL is.

use alo_bindings::navigating;
use alo_dom::Document;
use alo_dom::sheets::{Sheet, asked_for};
use alo_net::cors::{Credentials, Mode};
use alo_net::csp::Policies;
use alo_net::referrer::Policy;
use alo_net::request::Purpose;
use alo_url::{Origin, Url};

use crate::said;
use crate::sheet::{MOST_SHEETS, SheetAnswer, SheetAsk};

/// The longest encoding name an `@charset` is read for, in characters. The
/// longest name the Encoding Standard lists is under forty; anything longer
/// is not a name, and is said cut short.
const LONGEST_CHARSET: usize = 40;

/// What one document's renderer knows of its linked sheets.
#[derive(Debug, Clone, Default)]
pub struct Linked {
    /// Every URL asked for, with the number it was asked under, in order.
    asked: Vec<(u64, String)>,
    /// Every sheet that arrived, by URL, decoded.
    arrived: Vec<(String, String)>,
    /// What has been said about a link not asked for, so it is said once:
    /// at most [`MOST_SHEETS`] of them, the rest counted.
    said: Vec<String>,
    /// How many links were last said to be past a bound, so the count is said
    /// when it changes rather than at every answer.
    over: usize,
    /// The number the next ask is given.
    next: u64,
}

/// Why a link is not asked for, which is said once per link and reason.
enum NotAsked {
    /// A `data:` URL.
    Data,
    /// It carries `integrity`.
    Integrity,
    /// Its `href` does not resolve.
    Unresolved(String),
    /// The page's `<meta>` policy refused it.
    Meta(String),
}

impl Linked {
    /// A new document's: nothing asked, nothing arrived.
    pub fn new() -> Self {
        Self::default()
    }

    /// The sheets `document`, at `address` under its `<meta>` policies
    /// `metas`, links and has not asked for — each given its number and taken
    /// as asked. What a link not asked for makes the renderer say is added to
    /// `issues`.
    pub fn asks(
        &mut self,
        document: &Document,
        address: &Url,
        metas: &Policies,
        issues: &mut Vec<String>,
    ) -> Vec<SheetAsk> {
        let base = navigating::base(document, address);
        let page = Origin::of(address);
        let mut asks = Vec::new();
        let mut over = 0_usize;
        for sheet in asked_for(document) {
            let Sheet::Linked { href, element } = sheet else {
                continue;
            };
            let Some(link) = document.element(element) else {
                continue;
            };
            let url = match alo_url::join(&base, &href) {
                Ok(url) => url,
                Err(why) => {
                    self.say(&href, &NotAsked::Unresolved(why.why), issues, &mut over);
                    continue;
                }
            };
            if self.asked.iter().any(|(_, at)| *at == url.serialised) {
                continue;
            }
            let nonce = alo_dom::nonce::presented(link);
            let refused = if url.scheme == "data" {
                Some(NotAsked::Data)
            } else if link.attr("integrity").is_some() {
                Some(NotAsked::Integrity)
            } else {
                metas
                    .allows_load(&Purpose::Style, &url, &page, nonce.as_deref())
                    .err()
                    .map(|refusal| NotAsked::Meta(refusal.to_string()))
            };
            if let Some(refused) = refused {
                self.say(&url.serialised, &refused, issues, &mut over);
                continue;
            }
            if self.asked.len() >= MOST_SHEETS {
                over += 1;
                continue;
            }
            let (mode, credentials) = crossing(link.attr("crossorigin"));
            let number = self.next;
            self.next = self.next.saturating_add(1);
            self.asked.push((number, url.serialised.clone()));
            asks.push(SheetAsk {
                number,
                url: url.serialised,
                mode,
                credentials,
                referrer: link.attr("referrerpolicy").and_then(Policy::named),
                nonce,
            });
        }
        if over != self.over {
            self.over = over;
            if over > 0 {
                issues.push(said::line(&format_args!(
                    "{over} of the page's linked style sheets were not asked for: a page may ask \
                     for at most {MOST_SHEETS}, and say why it did not ask for at most \
                     {MOST_SHEETS} more"
                )));
            }
        }
        asks
    }

    /// Say why the link to `what` is not asked for, if it has not been said;
    /// past [`MOST_SHEETS`] such lines, count it in `over` instead.
    fn say(&mut self, what: &str, why: &NotAsked, issues: &mut Vec<String>, over: &mut usize) {
        let key = format!("{}{what}", why.tag());
        if self.said.contains(&key) {
            return;
        }
        if self.said.len() >= MOST_SHEETS {
            *over += 1;
            return;
        }
        let shown: String = what.chars().take(said::LONGEST_LINE).collect();
        issues.push(said::line(&format_args!(
            "the style sheet at {shown:?} was not asked for: {}",
            why.said()
        )));
        self.said.push(key);
    }

    /// Take the answer to one ask: whether a sheet arrived that a draw should
    /// now apply. What the page is told — that it did not arrive, or that it
    /// named an encoding it was not read in — is added to `issues`. An answer
    /// to a number never asked is ignored.
    pub fn arrived(&mut self, answer: &SheetAnswer, issues: &mut Vec<String>) -> bool {
        let Some((_, url)) = self
            .asked
            .iter()
            .find(|(number, _)| *number == answer.number)
        else {
            return false;
        };
        let Some(bytes) = &answer.bytes else {
            issues.push(said::line(&format_args!(
                "the style sheet at {url:?} did not arrive"
            )));
            return false;
        };
        let (text, named) = decoded(bytes);
        if let Some(named) = named {
            issues.push(said::line(&format_args!(
                "the style sheet at {url:?} says it is in {named:?}, and was read as UTF-8, the \
                 only encoding this browser reads a style sheet in"
            )));
        }
        let url = url.clone();
        self.arrived.retain(|(at, _)| *at != url);
        self.arrived.push((url, text));
        true
    }

    /// The sheets that arrived for the links `document`, at `address`, has
    /// now — each `href` as the page wrote it, as a draw looks a link's sheet
    /// up, and the sheet's text.
    pub fn for_draw(&self, document: &Document, address: &Url) -> Vec<(String, String)> {
        if self.arrived.is_empty() {
            return Vec::new();
        }
        let base = navigating::base(document, address);
        asked_for(document)
            .into_iter()
            .filter_map(|sheet| match sheet {
                Sheet::Linked { href, .. } => {
                    let url = alo_url::join(&base, &href).ok()?;
                    let (_, text) = self.arrived.iter().find(|(at, _)| *at == url.serialised)?;
                    Some((href, text.clone()))
                }
                Sheet::Written { .. } => None,
            })
            .collect()
    }
}

impl NotAsked {
    /// A short tag that keeps two reasons for one link apart.
    const fn tag(&self) -> &'static str {
        match self {
            NotAsked::Data => "d ",
            NotAsked::Integrity => "i ",
            NotAsked::Unresolved(_) => "u ",
            NotAsked::Meta(_) => "m ",
        }
    }

    /// Why, in words.
    fn said(&self) -> String {
        match self {
            NotAsked::Data => "a data: style sheet is the page's own bytes, and this browser \
                               does not read one as a sheet yet"
                .to_owned(),
            NotAsked::Integrity => "its link asks for its integrity to be checked, which this \
                                    browser cannot do yet, and applying it unchecked would be \
                                    weaker than the page asked for"
                .to_owned(),
            NotAsked::Unresolved(why) => format!("it is not a URL: {why}"),
            NotAsked::Meta(refusal) => {
                format!("the page's own <meta> policy refused it: {refusal}")
            }
        }
    }
}

/// A `<link>`'s `crossorigin`, as `alo-net`'s mode and credentials: absent is
/// `no-cors` carrying credentials; `use-credentials` is `cors` carrying them;
/// anything else — `anonymous`, empty, or a value HTML does not know, which
/// it reads as `anonymous` — is `cors` carrying them to the same origin only.
fn crossing(crossorigin: Option<&str>) -> (Mode, Credentials) {
    match crossorigin {
        None => (Mode::NoCors, Credentials::Include),
        Some(value) if value.trim().eq_ignore_ascii_case("use-credentials") => {
            (Mode::Cors, Credentials::Include)
        }
        Some(_) => (Mode::Cors, Credentials::SameOrigin),
    }
}

/// A sheet's bytes as text, read as UTF-8 with a byte order mark removed and
/// an invalid sequence replaced; and the encoding an `@charset` at its start
/// names, when that is not UTF-8.
fn decoded(bytes: &[u8]) -> (String, Option<String>) {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes).into_owned();
    // CSS Syntax: `@charset "` exactly, then the name up to `";`.
    let named = text
        .strip_prefix("@charset \"")
        .and_then(|rest| rest.split_once("\";"))
        .map(|(name, _)| name.chars().take(LONGEST_CHARSET).collect::<String>())
        .filter(|name| !name.eq_ignore_ascii_case("utf-8") && !name.eq_ignore_ascii_case("utf8"));
    (text, named)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    const PAGE: &str = "https://shop.example/a/page";

    fn asked(markup: &str, linked: &mut Linked, metas: &Policies) -> (Vec<SheetAsk>, Vec<String>) {
        let document = parse_document(markup);
        let mut issues = Vec::new();
        let asks = linked.asks(&document, &url(PAGE), metas, &mut issues);
        (asks, issues)
    }

    fn meta(policy: &str) -> Policies {
        crate::Page::policies_of(&[policy.to_owned()])
    }

    #[test]
    fn each_url_is_asked_for_once_resolved_against_the_base_with_how_its_link_crosses() {
        let mut linked = Linked::new();
        let (asks, issues) = asked(
            "<base href=/b/>\
             <link rel=stylesheet href=one.css>\
             <link rel=stylesheet href=/b/one.css>\
             <link rel=stylesheet href=https://cdn.example/two.css crossorigin \
                   referrerpolicy=no-referrer nonce=n>\
             <link rel=stylesheet href=three.css crossorigin=use-credentials>",
            &mut linked,
            &Policies::none(),
        );
        assert!(issues.is_empty(), "{issues:?}");
        let said: Vec<_> = asks
            .iter()
            .map(|ask| {
                (
                    ask.number,
                    ask.url.as_str(),
                    ask.mode,
                    ask.credentials,
                    ask.referrer,
                    ask.nonce.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            said,
            [
                (
                    0,
                    "https://shop.example/b/one.css",
                    Mode::NoCors,
                    Credentials::Include,
                    None,
                    None
                ),
                (
                    1,
                    "https://cdn.example/two.css",
                    Mode::Cors,
                    Credentials::SameOrigin,
                    Some(Policy::NoReferrer),
                    Some("n")
                ),
                (
                    2,
                    "https://shop.example/b/three.css",
                    Mode::Cors,
                    Credentials::Include,
                    None,
                    None
                ),
            ]
        );
        // The same document again asks for nothing; a link added since is
        // the only new ask.
        let (again, _) = asked(
            "<base href=/b/><link rel=stylesheet href=one.css>\
             <link rel=stylesheet href=four.css>",
            &mut linked,
            &Policies::none(),
        );
        assert_eq!(
            again
                .iter()
                .map(|ask| (ask.number, ask.url.as_str()))
                .collect::<Vec<_>>(),
            [(3, "https://shop.example/b/four.css")]
        );
    }

    #[test]
    fn data_integrity_and_a_meta_refusal_are_said_once_and_never_asked() {
        let mut linked = Linked::new();
        let markup = "<link rel=stylesheet href='data:text/css,p{}'>\
                      <link rel=stylesheet href=/checked.css integrity=sha256-abc>\
                      <link rel=stylesheet href=https://cdn.example/x.css>\
                      <link rel=stylesheet href=https://cdn.example/y.css nonce=ok>\
                      <link rel=stylesheet href=/own.css>";
        let metas = meta("style-src 'self' 'nonce-ok'");
        let (asks, issues) = asked(markup, &mut linked, &metas);
        assert_eq!(
            asks.iter().map(|ask| ask.url.as_str()).collect::<Vec<_>>(),
            ["https://cdn.example/y.css", "https://shop.example/own.css"]
        );
        assert_eq!(issues.len(), 3, "{issues:?}");
        assert!(issues[0].contains("data:") && issues[0].contains("page's own bytes"));
        assert!(issues[1].contains("/checked.css") && issues[1].contains("integrity"));
        assert!(issues[2].contains("cdn.example/x.css") && issues[2].contains("style-src"));
        let (again, said_again) = asked(markup, &mut linked, &metas);
        assert!(again.is_empty() && said_again.is_empty(), "{said_again:?}");
    }

    #[test]
    fn past_the_bound_links_are_counted_and_the_count_said_when_it_changes() {
        let mut linked = Linked::new();
        let links = |many: usize| -> String {
            (0..many)
                .map(|at| format!("<link rel=stylesheet href=/{at}.css>"))
                .collect::<Vec<_>>()
                .concat()
        };
        let (asks, issues) = asked(&links(MOST_SHEETS + 2), &mut linked, &Policies::none());
        assert_eq!(asks.len(), MOST_SHEETS);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].starts_with("2 of the page's linked style sheets"));
        let (asks, issues) = asked(&links(MOST_SHEETS + 2), &mut linked, &Policies::none());
        assert!(
            asks.is_empty() && issues.is_empty(),
            "said once: {issues:?}"
        );
        let (_, issues) = asked(&links(MOST_SHEETS + 5), &mut linked, &Policies::none());
        assert!(issues[0].starts_with("5 of"), "{issues:?}");
    }

    #[test]
    fn what_arrived_is_applied_to_the_links_the_document_has_now() {
        let mut linked = Linked::new();
        let (asks, _) = asked(
            "<link rel=stylesheet href=/a.css><link rel=stylesheet href=/b.css>",
            &mut linked,
            &Policies::none(),
        );
        let mut issues = Vec::new();
        let a = SheetAnswer {
            number: asks[0].number,
            bytes: Some(b"\xEF\xBB\xBFp { color: red }".to_vec()),
        };
        assert!(linked.arrived(&a, &mut issues));
        assert!(!linked.arrived(&SheetAnswer::failed(asks[1].number), &mut issues));
        assert!(
            !linked.arrived(&SheetAnswer::failed(99), &mut issues),
            "never asked"
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].contains("/b.css") && issues[0].contains("did not arrive"));

        let now = parse_document(
            "<link rel=stylesheet href=a.css><p>x</p><link rel=stylesheet href=/a.css>",
        );
        assert_eq!(
            linked.for_draw(&now, &url("https://shop.example/")),
            [
                ("a.css".to_owned(), "p { color: red }".to_owned()),
                ("/a.css".to_owned(), "p { color: red }".to_owned()),
            ]
        );
        let removed = parse_document("<p>x</p>");
        assert!(linked.for_draw(&removed, &url(PAGE)).is_empty());
    }

    #[test]
    fn hostile_bytes_are_read_as_utf_8_and_another_charset_said() {
        assert_eq!(decoded(b"p{}\xFF"), ("p{}\u{FFFD}".to_owned(), None));
        assert_eq!(decoded(b"@charset \"utf-8\"; p{}").1, None);
        assert_eq!(
            decoded(b"@charset \"windows-1252\"; p{}").1,
            Some("windows-1252".to_owned())
        );
        let long = format!("@charset \"{}\";", "x".repeat(10_000));
        assert_eq!(
            decoded(long.as_bytes()).1.map(|name| name.chars().count()),
            Some(LONGEST_CHARSET)
        );
        assert_eq!(decoded(b"@charset \"unterminated").1, None);
        assert_eq!(decoded(b"").0, "");
    }

    #[test]
    fn crossorigin_reads_as_html_reads_it() {
        assert_eq!(crossing(None), (Mode::NoCors, Credentials::Include));
        assert_eq!(crossing(Some("")), (Mode::Cors, Credentials::SameOrigin));
        assert_eq!(
            crossing(Some("nonsense")),
            (Mode::Cors, Credentials::SameOrigin)
        );
        assert_eq!(
            crossing(Some(" USE-CREDENTIALS ")),
            (Mode::Cors, Credentials::Include)
        );
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A form's entries written as a URL and a body: the browser process's
//! encoders (ADR 0051 § 5, queue item 470).
//!
//! A renderer's ask names a form's entries as names and values and never as
//! bytes, so the encoding happens here, once, in the process that writes the
//! request — and when files arrive (item 475), their bytes go from this
//! process's disk into the body without a renderer ever holding them.
//!
//! What is done with the entries is HTML's table, by the action's scheme and
//! the method:
//!
//! | | `GET` | `POST` |
//! |---|---|---|
//! | `http`, `https` | the action's query **replaced** by the entries | the entries are the body |
//! | anything else | refused, by name | refused, by name |
//!
//! A `GET`'s enctype is ignored, as HTML says. A `POST` is written as
//! `application/x-www-form-urlencoded`, with no charset parameter, as Chrome
//! 154 sent it; `multipart/form-data` and `text/plain` are refused by name
//! until item 474. HTML leaves a form sent to a scheme outside its table to
//! each browser, and the scheme table in [`crate::navigate`] already refuses
//! every one but `about:blank` and a `file:` page's `file:`, so those two are
//! refused by name until a page needs one.
//!
//! Before anything is written, each line break in a name or a value becomes
//! CR LF — a lone CR or a lone LF alike — which is where HTML normalises them
//! (*converting an entry list to a list of name-value pairs*). A file entry
//! with no file chosen writes its name and nothing after it, as measured.

use alo_bindings::navigating::Submission;
use alo_dom::submission::{Enctype, Entry, Method, Value};
use alo_url::Url;
use core::fmt;

/// What a navigation sends as its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    /// Its `Content-Type`.
    pub content_type: &'static str,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// A form's submission, written: where to and how, and what is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// Where to: the action, with the entries as its query for a `GET`.
    pub url: Url,
    /// How.
    pub method: Method,
    /// The body, for a `POST`.
    pub body: Option<Body>,
}

/// Why a form's entries were not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unwritten {
    /// A `POST` in an enctype not built.
    Enctype(Enctype),
    /// An action whose scheme HTML's table does not give a form.
    Scheme(String),
    /// The action with its new query was not a URL.
    Unparsed(String),
}

impl fmt::Display for Unwritten {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unwritten::Enctype(enctype) => write!(
                f,
                "it is a form sent as {}, and writing one is queue item 474",
                enctype.media_type()
            ),
            Unwritten::Scheme(scheme) => write!(
                f,
                "it is a form sent to a {scheme}: URL, which HTML gives no meaning and no \
                 page here has needed"
            ),
            Unwritten::Unparsed(why) => {
                write!(
                    f,
                    "with the form's entries as its query it is not a URL: {why}"
                )
            }
        }
    }
}

/// Write `submission`'s entries for its `action`.
///
/// # Errors
///
/// [`Unwritten`], naming what is not built or not given a meaning.
pub fn write(action: &Url, submission: &Submission) -> Result<Written, Unwritten> {
    if !matches!(action.scheme.as_str(), "http" | "https") {
        return Err(Unwritten::Scheme(action.scheme.clone()));
    }
    let pairs = pairs(&submission.entries);
    let encoded = alo_url::form::urlencoded(
        pairs
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    );
    match submission.method {
        Method::Get => Ok(Written {
            url: alo_url::with_query(action, &encoded)
                .map_err(|why| Unwritten::Unparsed(why.why))?,
            method: Method::Get,
            body: None,
        }),
        Method::Post => match submission.enctype {
            Enctype::UrlEncoded => Ok(Written {
                url: action.clone(),
                method: Method::Post,
                body: Some(Body {
                    content_type: Enctype::UrlEncoded.media_type(),
                    bytes: encoded.into_bytes(),
                }),
            }),
            other => Err(Unwritten::Enctype(other)),
        },
    }
}

/// The entries as names and values, each line break CR LF, and a file entry
/// with no file chosen as its name and nothing.
fn pairs(entries: &[Entry]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|entry| {
            let value = match &entry.value {
                Value::Text(text) => crlf(text),
                Value::NoFile => String::new(),
            };
            (crlf(&entry.name), value)
        })
        .collect()
}

/// `text` with every line break CR LF: a CR not before an LF, and an LF not
/// after a CR, each become the pair.
fn crlf(text: &str) -> String {
    let mut written = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                written.push_str("\r\n");
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
            }
            '\n' => written.push_str("\r\n"),
            other => written.push(other),
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    fn entry(name: &str, value: &str) -> Entry {
        Entry {
            name: name.to_owned(),
            value: Value::Text(value.to_owned()),
        }
    }

    fn sending(method: Method, enctype: Enctype, entries: Vec<Entry>) -> Submission {
        Submission {
            method,
            enctype,
            entries,
        }
    }

    #[test]
    fn a_post_is_chromes_body_with_no_charset() {
        let written = write(
            &url("https://httpbin.org/post"),
            &sending(
                Method::Post,
                Enctype::UrlEncoded,
                vec![
                    entry("custname", "Ada Lovelace"),
                    entry("custtel", ""),
                    entry("custemail", ""),
                    entry("delivery", ""),
                    entry("comments", ""),
                ],
            ),
        )
        .unwrap();
        assert_eq!(written.url, url("https://httpbin.org/post"));
        assert_eq!(written.method, Method::Post);
        let body = written.body.unwrap();
        assert_eq!(body.content_type, "application/x-www-form-urlencoded");
        assert_eq!(
            body.bytes,
            b"custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments="
        );
    }

    #[test]
    fn a_get_replaces_the_actions_query_and_ignores_its_enctype() {
        for enctype in [Enctype::UrlEncoded, Enctype::Multipart, Enctype::TextPlain] {
            let written = write(
                &url("https://example.test/get?q=9#here"),
                &sending(Method::Get, enctype, vec![entry("a", "hello world&=é")]),
            )
            .unwrap();
            assert_eq!(
                written.url.serialised,
                "https://example.test/get?a=hello+world%26%3D%C3%A9#here"
            );
            assert_eq!(written.method, Method::Get);
            assert_eq!(written.body, None);
        }
        let nothing = write(
            &url("https://example.test/get?q=9"),
            &sending(Method::Get, Enctype::UrlEncoded, Vec::new()),
        )
        .unwrap();
        assert_eq!(nothing.url.serialised, "https://example.test/get?");
    }

    #[test]
    fn every_line_break_is_cr_lf_and_an_empty_file_field_is_its_name() {
        let written = write(
            &url("https://example.test/"),
            &sending(
                Method::Post,
                Enctype::UrlEncoded,
                vec![
                    entry("a\nb", "one\rtwo\nthree\r\nfour\n\r"),
                    Entry {
                        name: "file".to_owned(),
                        value: Value::NoFile,
                    },
                ],
            ),
        )
        .unwrap();
        assert_eq!(
            written.body.unwrap().bytes,
            b"a%0D%0Ab=one%0D%0Atwo%0D%0Athree%0D%0Afour%0D%0A%0D%0A&file="
        );
    }

    #[test]
    fn what_is_not_built_or_not_given_a_meaning_is_refused_by_name() {
        for enctype in [Enctype::Multipart, Enctype::TextPlain] {
            let refused = write(
                &url("https://example.test/"),
                &sending(Method::Post, enctype, vec![entry("a", "1")]),
            );
            assert_eq!(refused, Err(Unwritten::Enctype(enctype)));
            assert!(refused.unwrap_err().to_string().contains("queue item 474"));
        }
        for action in ["about:blank", "file:///home/me/form.html"] {
            for method in [Method::Get, Method::Post] {
                let refused = write(
                    &url(action),
                    &sending(method, Enctype::UrlEncoded, Vec::new()),
                );
                assert!(
                    matches!(refused, Err(Unwritten::Scheme(_))),
                    "{action}: {refused:?}"
                );
            }
        }
    }

    #[test]
    fn hostile_names_and_values_are_written_rather_than_trusted() {
        let long = "\r".repeat(10_000);
        let written = write(
            &url("https://example.test/"),
            &sending(
                Method::Get,
                Enctype::UrlEncoded,
                vec![
                    entry("\u{0}#?&=", "\u{feff}\u{10ffff}"),
                    entry(&long, "%zz"),
                ],
            ),
        )
        .unwrap();
        let query = written.url.query.unwrap_or_default();
        assert!(
            query.starts_with("%00%23%3F%26%3D=%EF%BB%BF%F4%8F%BF%BF&"),
            "{query}"
        );
        assert!(query.ends_with("=%25zz"), "{query}");
        assert_eq!(query.matches("%0D%0A").count(), 10_000);
        assert_eq!(
            written.url.fragment, None,
            "a # in an entry is not a fragment"
        );
    }
}

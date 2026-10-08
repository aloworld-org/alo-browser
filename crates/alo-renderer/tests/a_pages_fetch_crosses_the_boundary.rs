/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 334: a page's fetch crosses the boundary (ADR 0032).
//!
//! *Malformed, truncated and adversarial bytes in either direction refused,
//! never a panic.* The asks come from a renderer, which parsed a stranger's
//! page; the answers go to one, and an answer of an opaque kind that carries
//! anything is refused, because carrying nothing is the whole of what one
//! promises.

use alo_agent::verb::Outcome;
use alo_box::tree::BoxId;
use alo_layout::Size;
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;
use alo_net::referrer::Policy;
use alo_renderer::fetch::{Answer, FetchAsk, Fetched, Kind, Readable};
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::wire::{
    fetched_size, read_from_renderer, read_to_renderer, write_from_renderer, write_to_renderer,
};
use alo_renderer::{Page, Renderer};
use alo_text::FontDatabase;

/// One ask of every mode, credentials mode, redirect mode and referrer
/// policy, with headers and a body.
fn asks() -> Vec<FetchAsk> {
    let modes = [Mode::Cors, Mode::NoCors, Mode::SameOrigin, Mode::Navigate];
    let credentials = [
        Credentials::Omit,
        Credentials::SameOrigin,
        Credentials::Include,
    ];
    let redirects = [
        redirect::Mode::Follow,
        redirect::Mode::Error,
        redirect::Mode::Manual,
    ];
    let policies = [
        None,
        Some(Policy::NoReferrer),
        Some(Policy::NoReferrerWhenDowngrade),
        Some(Policy::Origin),
        Some(Policy::OriginWhenCrossOrigin),
        Some(Policy::SameOrigin),
        Some(Policy::StrictOrigin),
        Some(Policy::StrictOriginWhenCrossOrigin),
        Some(Policy::UnsafeUrl),
    ];
    let mut modes = modes.iter().cycle();
    let mut credentials = credentials.iter().cycle();
    let mut redirects = redirects.iter().cycle();
    let mut policies = policies.iter().cycle();
    (0..12u8)
        .filter_map(|at| {
            Some(FetchAsk {
                number: u64::MAX - u64::from(at),
                url: format!("https://example.com/{at}"),
                method: if at % 2 == 0 { "GET" } else { "POST" }.to_owned(),
                headers: vec![
                    ("Accept".to_owned(), "text/plain".to_owned()),
                    ("X-Count".to_owned(), at.to_string()),
                ],
                body: if at % 2 == 0 {
                    Vec::new()
                } else {
                    vec![0, 255, at]
                },
                mode: *modes.next()?,
                credentials: *credentials.next()?,
                redirect: *redirects.next()?,
                referrer: *policies.next()?,
            })
        })
        .collect()
}

fn answers() -> Vec<FromRenderer> {
    vec![
        FromRenderer::Loaded {
            issues: vec!["a line".to_owned()],
            wanted: Vec::new(),
            objections: Vec::new(),
            navigation: None,
            fetches: asks(),
        },
        FromRenderer::Acted {
            outcome: Outcome::Activated {
                node: BoxId::from_wire(4),
                name: Some("Download".to_owned()),
            },
            issues: Vec::new(),
            navigation: None,
            fetches: asks(),
        },
        FromRenderer::Delivered {
            issues: vec!["then: uncaught: TypeError".to_owned()],
            navigation: None,
            fetches: asks(),
        },
        FromRenderer::Delivered {
            issues: Vec::new(),
            navigation: None,
            fetches: Vec::new(),
        },
    ]
}

fn readable(kind: Kind) -> Readable {
    if kind.is_opaque() {
        return Readable {
            kind,
            status: 0,
            status_text: String::new(),
            url: None,
            redirected: false,
            headers: Vec::new(),
            body: Vec::new(),
        };
    }
    Readable {
        kind,
        status: 206,
        status_text: "Partial Content".to_owned(),
        url: Some("https://example.com/a".to_owned()),
        redirected: true,
        headers: vec![("Content-Type".to_owned(), "text/plain".to_owned())],
        body: b"the bytes".to_vec(),
    }
}

fn deliveries() -> Vec<ToRenderer> {
    let mut sent = vec![ToRenderer::Fetched(Box::new(Fetched::failed(0)))];
    for kind in [Kind::Basic, Kind::Cors, Kind::Opaque, Kind::OpaqueRedirect] {
        sent.push(ToRenderer::Fetched(Box::new(Fetched {
            number: 77,
            answer: Answer::Response(Box::new(readable(kind))),
        })));
    }
    sent
}

#[test]
fn every_ask_and_every_answer_survives_the_crossing() {
    for original in answers() {
        let back = read_from_renderer(&write_from_renderer(&original));
        assert_eq!(back.as_ref(), Ok(&original));
    }
    for original in deliveries() {
        let back = read_to_renderer(&write_to_renderer(&original));
        assert_eq!(back.as_ref(), Ok(&original));
    }
}

/// The filter asks how large an answer is before sending a body, so the
/// arithmetic has to be the encoding's exactly.
#[test]
fn an_answers_size_is_what_it_writes() {
    for sent in deliveries() {
        let ToRenderer::Fetched(fetched) = &sent else {
            continue;
        };
        assert_eq!(
            fetched_size(fetched),
            write_to_renderer(&sent).len(),
            "{fetched}"
        );
    }
}

/// Every prefix of every message is a message that did not arrive.
#[test]
fn a_message_that_stops_part_way_through_is_refused_in_either_direction() {
    for original in answers() {
        let whole = write_from_renderer(&original);
        for cut in 1..whole.len() {
            assert!(
                read_from_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
                "{cut} bytes of {original:?} were read as a whole one"
            );
        }
    }
    for original in deliveries() {
        let whole = write_to_renderer(&original);
        for cut in 1..whole.len() {
            assert!(
                read_to_renderer(whole.get(..cut).unwrap_or_default()).is_err(),
                "{cut} bytes of {original} were read as a whole one"
            );
        }
    }
}

/// Every byte of every message changed to every value, read again: whatever
/// comes back, nothing panics — and a tag nobody has is refused rather than
/// read as something.
#[test]
fn every_byte_of_every_message_changed_is_read_or_refused_never_a_panic() {
    for original in answers() {
        let whole = write_from_renderer(&original);
        for at in 0..whole.len() {
            for value in [0u8, 1, 3, 4, 9, 0x7f, 0xff] {
                let mut changed = whole.clone();
                if let Some(byte) = changed.get_mut(at) {
                    *byte = value;
                }
                let _ = read_from_renderer(&changed);
            }
        }
    }
    for original in deliveries() {
        let whole = write_to_renderer(&original);
        for at in 0..whole.len() {
            for value in [0u8, 1, 3, 4, 9, 0x7f, 0xff] {
                let mut changed = whole.clone();
                if let Some(byte) = changed.get_mut(at) {
                    *byte = value;
                }
                let _ = read_to_renderer(&changed);
            }
        }
    }
}

/// The last ask's four tags, from the end of a `Delivered` carrying one:
/// referrer, redirect, credentials, mode.
#[test]
fn an_ask_tagged_with_something_nobody_has_is_refused_by_name() {
    let mut one = asks().remove(0);
    one.referrer = None;
    let whole = write_from_renderer(&FromRenderer::Delivered {
        issues: Vec::new(),
        navigation: None,
        fetches: vec![one],
    });
    let end = whole.len();
    for (from_end, value, said) in [
        (1, 10u8, "referrer policy tagged 9"),
        (2, 3, "redirect mode tagged 3"),
        (3, 3, "credentials tagged 3"),
        (4, 4, "mode tagged 4"),
    ] {
        let mut strange = whole.clone();
        if let Some(byte) = strange.get_mut(end - from_end) {
            *byte = value;
        }
        let refused = read_from_renderer(&strange);
        assert!(
            refused.as_ref().is_err_and(|why| why.why.contains(said)),
            "{said}: {refused:?}"
        );
    }
}

/// A count of asks no message could hold is refused before anything is made
/// room for.
#[test]
fn a_count_of_asks_no_message_could_hold_is_refused() {
    let mut bytes = vec![8u8];
    bytes.extend_from_slice(&0u64.to_be_bytes()); // no issues
    bytes.push(0); // no navigation
    bytes.extend_from_slice(&u64::MAX.to_be_bytes()); // asks
    assert!(read_from_renderer(&bytes).is_err());
}

/// An opaque answer that carries anything is not one: the renderer refuses
/// it rather than hand the page a body it was promised it would not have.
#[test]
fn an_opaque_answer_carrying_anything_is_refused() {
    let carrying = [
        Readable {
            status: 200,
            ..readable(Kind::Opaque)
        },
        Readable {
            body: b"secret".to_vec(),
            ..readable(Kind::Opaque)
        },
        Readable {
            headers: vec![("X-A".to_owned(), "b".to_owned())],
            ..readable(Kind::OpaqueRedirect)
        },
        Readable {
            url: Some("https://bank.example/".to_owned()),
            ..readable(Kind::OpaqueRedirect)
        },
        Readable {
            redirected: true,
            ..readable(Kind::Opaque)
        },
    ];
    for readable in carrying {
        let sent = ToRenderer::Fetched(Box::new(Fetched {
            number: 1,
            answer: Answer::Response(Box::new(readable)),
        }));
        let refused = read_to_renderer(&write_to_renderer(&sent));
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.why.contains("may not read")),
            "{refused:?}"
        );
    }
}

/// A status is at most sixteen bits, and an answer tagged with neither a
/// response nor a failure is not an answer.
#[test]
fn an_answer_that_is_not_one_is_refused() {
    let mut bytes = vec![7u8];
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.push(2);
    assert!(
        read_to_renderer(&bytes)
            .as_ref()
            .is_err_and(|why| why.why.contains("answer tagged 2"))
    );
    let mut bytes = vec![7u8];
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.push(1); // a response
    bytes.push(0); // basic
    bytes.extend_from_slice(&70_000u64.to_be_bytes());
    assert!(
        read_to_renderer(&bytes)
            .as_ref()
            .is_err_and(|why| why.why.contains("status"))
    );
}

/// Until a page records an ask (item 335) nothing waits on any number, and an
/// answer nothing waits for is answered by nobody: said, and the page left as
/// it was.
#[test]
fn an_answer_nothing_on_the_page_waits_for_is_said_and_changes_nothing() {
    let mut renderer = Renderer::new(FontDatabase::new());
    assert_eq!(
        renderer.handle(ToRenderer::Fetched(Box::new(Fetched::failed(3)))),
        FromRenderer::Failed(Failure::NothingLoaded)
    );
    let page = Page::new("<p>hi</p>", Size::new(100.0, 50.0));
    assert!(matches!(
        renderer.handle(ToRenderer::Load(Box::new(page))),
        FromRenderer::Loaded { .. }
    ));
    let changes = renderer.document().map(alo_dom::Document::change_count);
    let FromRenderer::Delivered {
        issues,
        navigation,
        fetches,
    } = renderer.handle(ToRenderer::Fetched(Box::new(Fetched::failed(3))))
    else {
        panic!("a delivery was not answered as one");
    };
    assert_eq!(navigation, None);
    assert!(fetches.is_empty());
    assert_eq!(issues.len(), 1);
    assert!(issues[0].contains("waiting for fetch 3"), "{issues:?}");
    assert_eq!(
        renderer.document().map(alo_dom::Document::change_count),
        changes
    );
}

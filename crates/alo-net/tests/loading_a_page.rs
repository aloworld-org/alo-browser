//! Loading a page from somewhere, rather than from a string handed over.
//!
//! Queue item 51's closing condition, and the shape queue item 53 will pour
//! HTTP into. Nothing here touches the network, which is what lets it be a
//! test at all.

use alo_net::encoding::Source;
use alo_net::{Purpose, Request, fetch};
use alo_url::Origin;

/// A URL, or one that names nothing when the text is not one — which no case
/// here is. Built by hand rather than panicking in a helper, so the assertion
/// that asked the question is the thing that fails.
fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

fn get(text: &str) -> Result<alo_net::Response, alo_net::FetchError> {
    fetch(&Request::get(url(text)))
}

#[test]
fn a_data_url_carries_its_own_bytes() {
    let response = get("data:text/html,%3Cp%3Ehello%3C/p%3E").expect("a response");
    assert!(response.status.is_ok());
    assert_eq!(response.text().text, "<p>hello</p>");
    assert!(response.media_type().is_some_and(|held| held.is_html()));
}

#[test]
fn a_data_url_can_be_base64_and_can_be_wrapped() {
    let plain = get("data:text/plain;base64,aGVsbG8gd29ybGQ=").expect("a response");
    assert_eq!(plain.text().text, "hello world");

    // Real `data:` URLs arrive wrapped across lines when they have been
    // written into markup, and the whitespace is not part of the data.
    let wrapped = get("data:text/plain;base64,aGVsbG8g\n   d29ybGQ=").expect("a response");
    assert_eq!(wrapped.text().text, "hello world");
}

#[test]
fn a_data_url_with_no_media_type_is_what_the_standard_says_it_is() {
    let response = get("data:,hello").expect("a response");
    let media = response.media_type().expect("the default");
    assert_eq!(media.essence(), "text/plain");
    assert_eq!(media.charset(), Some("US-ASCII"));
}

#[test]
fn a_file_url_reads_what_is_there_and_says_what_it_is() {
    // alo's own sign-in screen, loaded the way a browser would rather than
    // read into a string by the test.
    let case = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/cases/alo-sign-in/page.html")
        .canonicalize()
        .expect("the case is checked in");
    let as_url = format!("file://{}", case.display());

    let response = get(&as_url).expect("a response");
    assert!(response.status.is_ok());
    assert!(
        response.media_type().is_some_and(|held| held.is_html()),
        "a .html file is markup",
    );
    let text = response.text();
    assert!(text.text.contains("Your workspace."), "it is alo's screen");
    assert!(!text.had_errors);
    assert_eq!(
        text.encoding.source,
        Source::Header,
        "a .html file's name is all there is to go on, and it is enough",
    );
}

#[test]
fn a_file_that_is_not_there_is_an_error_with_the_path_in_it() {
    let answer = get("file:///no/such/file/anywhere.html");
    match answer {
        Err(alo_net::FetchError::Failed { why, .. }) => {
            assert!(why.contains("anywhere.html"), "{why}");
        }
        other => panic!("expected a failure with a path in it, got {other:?}"),
    }
}

#[test]
fn a_scheme_this_browser_does_not_fetch_says_which_one() {
    // Distinct from "the server did not answer", because they are different
    // things to tell a person. `http` and `https` stopped being on this list
    // with queue item 53.
    for text in ["ftp://example.com/", "gopher://example.com/"] {
        match get(text) {
            Err(alo_net::FetchError::UnsupportedScheme { scheme }) => {
                assert!(text.starts_with(&scheme), "{text}: {scheme}");
            }
            other => panic!("expected an unsupported scheme for {text}, got {other:?}"),
        }
    }
}

#[test]
fn a_request_says_who_asked_and_what_for() {
    let page = Origin::of(&url("https://example.com/"));
    let request = Request::get(url("data:text/css,p{color:red}"))
        .for_purpose(Purpose::Style)
        .asked_by(page.clone());
    assert_eq!(request.initiator, Some(page));

    let response = fetch(&request).expect("a response");
    assert_eq!(response.text().text, "p{color:red}");
}

#[test]
fn nothing_a_stranger_can_put_in_a_url_makes_this_panic() {
    // `docs/autonomy/LOOP.md`, stage 2. A `data:` URL is entirely under
    // somebody else's control, which makes it the most hostile input this
    // crate has.
    let nasty = [
        "data:",
        "data:,",
        "data:;base64,",
        "data:text/html;base64,!!!!not base64!!!!",
        "data:text/html;base64,aGVsbG8",
        "data:%",
        "data:%zz,x",
        "data:text/html,%",
        "data:text/html,%e0%a4",
        "file://",
        "file:///",
    ];
    for text in nasty {
        let Ok(parsed) = alo_url::parse(text) else {
            continue;
        };
        // Either answer is fine; not returning one is not.
        if let Ok(response) = fetch(&Request::get(parsed)) {
            let _ = response.text();
            let _ = response.media_type();
            let _ = response.to_string();
        }
    }
}

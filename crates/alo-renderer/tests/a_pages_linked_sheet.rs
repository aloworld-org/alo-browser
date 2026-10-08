/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 348 (ADR 0035 §§ 1, 3 and 4): a renderer asks for the style
//! sheets its page links, and draws the page with what arrives.
//!
//! Each test loads a page into a renderer in this process — the browser
//! process's half is `a_linked_sheet_is_made.rs`'s — and reads what crossed:
//! the asks in each answer, and the computed colour of an element after a
//! sheet's answer was delivered as a task of its own.

use alo_agent::{Target, Verb};
use alo_layout::Size;
use alo_net::cors::{Credentials, Mode};
use alo_renderer::sheet::{SheetAnswer, SheetAsk};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// Where every page here is.
const HERE: &str = "https://shop.example/a/page";

/// The colour of a `<p>` nothing styled: the user-agent sheet's.
const UNSTYLED: &str = "black";

/// A page at [`HERE`] of `markup` — at `about:blank` if [`HERE`] stopped
/// parsing, which every assertion on what it asked for then says.
fn page(markup: &str) -> Page {
    let page = Page::new(markup, WINDOW);
    match alo_url::parse(HERE) {
        Ok(url) => page.at(url),
        Err(_) => page,
    }
}

/// What one answer carried: its issues and its sheet asks.
struct Answer {
    issues: Vec<String>,
    sheets: Vec<SheetAsk>,
}

/// The issues and sheets of a `Loaded`, `Acted` or `Delivered`, or what came
/// instead, as an issue.
fn answer(answered: FromRenderer) -> Answer {
    match answered {
        FromRenderer::Loaded { issues, sheets, .. }
        | FromRenderer::Acted { issues, sheets, .. }
        | FromRenderer::Delivered { issues, sheets, .. } => Answer { issues, sheets },
        other => Answer {
            issues: vec![format!("not an answer that asks: {other:?}")],
            sheets: Vec::new(),
        },
    }
}

/// Load `page` into a new renderer.
fn load(page: Page) -> (Renderer, Answer) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let loaded = answer(renderer.handle(ToRenderer::Load(Box::new(page))));
    (renderer, loaded)
}

/// Deliver the answer to sheet `number`.
fn deliver(renderer: &mut Renderer, number: u64, bytes: Option<&[u8]>) -> Answer {
    answer(renderer.handle(ToRenderer::Sheet(Box::new(SheetAnswer {
        number,
        bytes: bytes.map(<[u8]>::to_vec),
    }))))
}

/// The colour drawn for the element whose `id` is `wanted`, as computed —
/// or what stood in the way of reading it.
fn colour(renderer: &mut Renderer, wanted: &str) -> String {
    renderer.handle(ToRenderer::Paint);
    let Some(document) = renderer.document() else {
        return "no document".to_owned();
    };
    let Some(id) = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    }) else {
        return format!("no #{wanted}");
    };
    let Some(drawing) = renderer.rendered() else {
        return "not drawn".to_owned();
    };
    drawing
        .styles
        .get(id)
        .and_then(|style| style.get("color"))
        .unwrap_or("none")
        .to_owned()
}

/// The URLs asked for, in order.
fn urls(sheets: &[SheetAsk]) -> Vec<&str> {
    sheets.iter().map(|ask| ask.url.as_str()).collect()
}

/// The closing condition's first clause: a page with a linked sheet is drawn
/// with it once its answer is delivered — and two links to one URL are one
/// ask, and a sheet arriving draws the page once.
#[test]
fn a_linked_sheet_is_asked_for_once_and_the_page_drawn_with_it_when_it_arrives() {
    let (mut renderer, loaded) = load(page(
        "<link rel=stylesheet href=site.css>\
         <link rel=stylesheet href=/a/site.css>\
         <p id=p>styled</p><p id=q>also</p>\
         <script>var ran = true;</script>",
    ));
    assert_eq!(urls(&loaded.sheets), ["https://shop.example/a/site.css"]);
    let ask = &loaded.sheets[0];
    assert_eq!(
        (
            ask.mode,
            ask.credentials,
            ask.referrer,
            ask.nonce.as_deref()
        ),
        (Mode::NoCors, Credentials::Include, None, None)
    );
    assert!(
        loaded
            .issues
            .iter()
            .any(|line| line.contains("no style sheet was loaded")),
        "the page is drawn without it until it arrives: {:?}",
        loaded.issues
    );
    assert_eq!(colour(&mut renderer, "p"), UNSTYLED);

    let draws = renderer.draws();
    let delivered = deliver(
        &mut renderer,
        ask.number,
        Some(b"\xEF\xBB\xBF#p { color: red } #q { color: blue }"),
    );
    assert!(delivered.issues.is_empty(), "{:?}", delivered.issues);
    assert!(
        delivered.sheets.is_empty(),
        "a sheet's task asks for nothing"
    );
    assert_eq!(
        renderer.draws(),
        draws + 1,
        "drawn once, in the sheet's task"
    );
    assert_eq!(colour(&mut renderer, "p"), "red");
    assert_eq!(colour(&mut renderer, "q"), "blue");
    assert_eq!(renderer.draws(), draws + 1, "and not again to be read");
}

/// A sheet that did not arrive is said, in the same words whatever happened,
/// and the page is left as it was.
#[test]
fn a_sheet_that_did_not_arrive_is_said_and_nothing_is_applied() {
    let (mut renderer, loaded) = load(page("<link rel=stylesheet href=/gone.css><p id=p>x</p>"));
    let delivered = deliver(&mut renderer, loaded.sheets[0].number, None);
    assert_eq!(delivered.issues.len(), 1, "{:?}", delivered.issues);
    assert!(
        delivered.issues[0].contains("https://shop.example/gone.css")
            && delivered.issues[0].contains("did not arrive"),
        "{:?}",
        delivered.issues
    );
    assert_eq!(colour(&mut renderer, "p"), UNSTYLED);
    // An answer to a number never asked for changes and says nothing.
    let stray = deliver(&mut renderer, 900, Some(b"#p { color: red }"));
    assert!(stray.issues.is_empty(), "{:?}", stray.issues);
    assert_eq!(colour(&mut renderer, "p"), UNSTYLED);
}

/// The closing condition's last renderer clause: a link a script adds is
/// asked for in the answer to the task that added it — and one it adds to a
/// URL already asked for is not asked for again.
#[test]
fn a_link_a_script_adds_is_asked_for_in_that_tasks_answer() {
    let (mut renderer, loaded) = load(page(
        "<link rel=stylesheet href=/first.css>\
         <button id=add>Add</button><p id=p>x</p><div id=links></div>\
         <script>\
           document.getElementById('add').addEventListener('click', function () {\
             ['/first.css', '/added.css'].forEach(function (href) {\
               var link = document.createElement('link');\
               link.setAttribute('rel', 'stylesheet');\
               link.setAttribute('href', href);\
               document.getElementById('links').appendChild(link);\
             });\
           });\
         </script>",
    ));
    assert_eq!(urls(&loaded.sheets), ["https://shop.example/first.css"]);
    let acted = answer(renderer.handle(ToRenderer::Act {
        target: Target::Named("Add".to_owned()),
        verb: Verb::Activate,
    }));
    assert_eq!(
        urls(&acted.sheets),
        ["https://shop.example/added.css"],
        "{:?}",
        acted.issues
    );
    assert_ne!(
        acted.sheets[0].number, loaded.sheets[0].number,
        "a number is the document's, once"
    );
    let delivered = deliver(
        &mut renderer,
        acted.sheets[0].number,
        Some(b"p { color: green }"),
    );
    assert!(delivered.issues.is_empty(), "{:?}", delivered.issues);
    assert_eq!(colour(&mut renderer, "p"), "green");
}

/// A `<meta>` policy is the renderer's to apply before it asks; a header
/// policy is the browser process's, so its sheet is asked for and decided
/// there.
#[test]
fn a_meta_policy_refuses_before_asking_and_a_header_policy_is_left_to_the_browser() {
    let (_, metas) = load(page(
        "<meta http-equiv=Content-Security-Policy content=\"style-src 'self'\">\
         <link rel=stylesheet href=https://cdn.example/x.css>\
         <link rel=stylesheet href=/own.css>",
    ));
    assert_eq!(urls(&metas.sheets), ["https://shop.example/own.css"]);
    assert!(
        metas
            .issues
            .iter()
            .any(|line| line.contains("cdn.example/x.css")
                && line.contains("<meta> policy")
                && line.contains("style-src")),
        "{:?}",
        metas.issues
    );

    let (_, headers) = load(
        page("<link rel=stylesheet href=https://cdn.example/x.css>")
            .with_policy("style-src 'self'"),
    );
    assert_eq!(urls(&headers.sheets), ["https://cdn.example/x.css"]);
}

/// A `data:` sheet and a link asking for its integrity to be checked are
/// refused by name, and a `crossorigin` link asks as CORS does; a new load
/// forgets what the last one asked for.
#[test]
fn data_and_integrity_are_refused_by_name_and_a_new_load_asks_again() {
    let markup = "<link rel=stylesheet href='data:text/css,p{color:red}'>\
                  <link rel=stylesheet href=/checked.css integrity=sha384-x>\
                  <link rel=stylesheet href=https://cdn.example/c.css crossorigin=anonymous \
                        referrerpolicy=origin>";
    let (mut renderer, loaded) = load(page(markup));
    assert_eq!(urls(&loaded.sheets), ["https://cdn.example/c.css"]);
    assert_eq!(
        (loaded.sheets[0].mode, loaded.sheets[0].credentials),
        (Mode::Cors, Credentials::SameOrigin)
    );
    let said = loaded.issues.join("\n");
    assert!(
        said.contains("data:") && said.contains("integrity"),
        "{said}"
    );

    let again = answer(renderer.handle(ToRenderer::Load(Box::new(page(markup)))));
    assert_eq!(urls(&again.sheets), ["https://cdn.example/c.css"]);
    assert!(
        again.issues.join("\n").contains("integrity"),
        "said again for a new document"
    );
}

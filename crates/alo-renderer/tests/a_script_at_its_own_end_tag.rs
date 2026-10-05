/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 247, cut from 80 (ADR 0017 § 7): a parser-inserted script runs
//! when the parser reaches its end tag, against the document parsed so far.
//!
//! *An inline script in the middle of `<body>` reads the body's last child as
//! its own `<script>`; a script before a `<p>` cannot find it and one after
//! can; scripts still run in document order under the same policies.* The
//! reference render is the corpus case `a-script-beside-itself`.
//!
//! The scripts here use item 80's members and no others, so `document.body`
//! is `document.documentElement.lastChild` — the body is the root element's
//! last child while it is being parsed — and what a script saw is written
//! onto the root element as an attribute for the test to read.

use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const WINDOW: Size = Size {
    width: 300.0,
    height: 200.0,
};

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 14px } \
                     div, li { height: 20px } ul { margin: 0; padding: 0 }";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

/// A renderer with `html` loaded, and the issues its load answered.
fn loaded(html: &str) -> (Renderer, Vec<String>) {
    loaded_under(html, &[])
}

/// The same, under the response's own `policies`.
fn loaded_under(html: &str, policies: &[&str]) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(fonts());
    let mut page = Page::new(html, WINDOW).with_sheet(SHEET);
    page.policies = policies.iter().map(|policy| (*policy).to_owned()).collect();
    let issues = match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

/// The value of the root element's attribute `name`, where a script wrote
/// what it saw.
fn saw(renderer: &Renderer, name: &str) -> Option<String> {
    let document = renderer.document()?;
    let html = document.element(document.children(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html("html"))
    })?)?;
    html.attr(name).map(ToOwned::to_owned)
}

/// The first element in tree order with `id`.
fn with_id(document: &Document, id: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))
}

fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

/// Write `what` onto the root element as `name` — the script half of
/// [`saw`].
fn record(name: &str, what: &str) -> String {
    format!("document.documentElement.setAttribute('{name}', {what});")
}

#[test]
fn a_script_in_the_middle_of_body_is_its_bodys_last_child() {
    let (renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><body><div id=before>before</div><script id=me>\
         const body = document.documentElement.lastChild;\
         {}{}</script><div id=after>after</div></body></html>",
        record("data-last", "body.lastChild.getAttribute('id')"),
        record(
            "data-before-me",
            "body.lastChild.previousSibling.getAttribute('id')"
        ),
    ));
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(saw(&renderer, "data-last").as_deref(), Some("me"));
    assert_eq!(saw(&renderer, "data-before-me").as_deref(), Some("before"));
}

/// A script that says whether the body has a child with id `later` yet.
fn looks_for_later(name: &str) -> String {
    format!(
        "<script>{{\
         let found = 'no';\
         let node = document.documentElement.lastChild.firstChild;\
         while (node !== null) {{\
           if (node.getAttribute !== undefined && node.getAttribute('id') === 'later') \
             {{ found = 'yes'; }}\
           node = node.nextSibling;\
         }}\
         {}}}</script>",
        record(name, "found")
    )
}

#[test]
fn a_script_before_a_paragraph_cannot_find_it_and_one_after_can() {
    let (renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><body>{}<p id=later>later</p>{}</body></html>",
        looks_for_later("data-early"),
        looks_for_later("data-late"),
    ));
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(saw(&renderer, "data-early").as_deref(), Some("no"));
    assert_eq!(saw(&renderer, "data-late").as_deref(), Some("yes"));
}

/// A script that appends `mark` to the root element's `data-order`, and the
/// number of the body's children it can see.
fn marks(mark: &str) -> String {
    format!(
        "<script>{{\
         const html = document.documentElement;\
         const was = html.getAttribute('data-order');\
         let seen = 0;\
         let node = html.lastChild.firstChild;\
         while (node !== null) {{ seen += 1; node = node.nextSibling; }}\
         html.setAttribute('data-order', `${{was === null ? '' : was}}{mark}${{seen}} `);\
         }}</script>"
    )
}

#[test]
fn scripts_run_in_document_order_each_seeing_what_was_parsed_before_it() {
    let (renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><head></head><body>{}<div>1</div>{}<div>2</div><div>3</div>{}\
         </body></html>",
        marks("a"),
        marks("b"),
        marks("c"),
    ));
    assert!(issues.is_empty(), "{issues:?}");
    // Each counts itself: one child, then a div and two scripts, then three
    // divs and three scripts.
    assert_eq!(saw(&renderer, "data-order").as_deref(), Some("a1 b3 c6 "));
}

#[test]
fn a_meta_policy_a_script_removes_still_governs_the_scripts_after_it() {
    let (renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><head>\
         <meta http-equiv=Content-Security-Policy content=\"script-src 'nonce-n'\">\
         <script nonce=n>document.documentElement.firstChild.firstChild.remove();\
         {}</script>\
         <script>{}</script>\
         <script nonce=n>{}</script></head></html>",
        record("data-first", "'ran'"),
        record("data-second", "'ran'"),
        record("data-third", "'ran'"),
    ));
    assert_eq!(saw(&renderer, "data-first").as_deref(), Some("ran"));
    assert_eq!(saw(&renderer, "data-second"), None, "{issues:?}");
    assert_eq!(saw(&renderer, "data-third").as_deref(), Some("ran"));
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(
        issues
            .first()
            .is_some_and(|issue| issue.starts_with("script 2: refused")),
        "{issues:?}"
    );
    let document = renderer.document();
    assert!(
        document
            .is_some_and(|document| !document.serialize_node(document.root()).contains("<meta")),
        "the `<meta>` is gone, and its policy is not"
    );
}

#[test]
fn a_meta_policy_after_a_script_does_not_reach_back_to_it() {
    let (renderer, issues) = loaded_under(
        &format!(
            "<!DOCTYPE html><html><head><script>{}</script>\
             <meta http-equiv=Content-Security-Policy content=\"script-src 'none'\">\
             <script>{}</script></head></html>",
            record("data-first", "'ran'"),
            record("data-second", "'ran'"),
        ),
        &["script-src 'unsafe-inline'"],
    );
    assert_eq!(saw(&renderer, "data-first").as_deref(), Some("ran"));
    assert_eq!(saw(&renderer, "data-second"), None);
    assert_eq!(issues.len(), 1, "{issues:?}");
}

/// A list a script grows beside itself — the corpus case's markup, smaller.
const GROWS_BESIDE_ITSELF: &str = "<!DOCTYPE html><html><body><ul><li>One</li><script>\
const list = document.documentElement.lastChild.lastChild;\
const row = document.createElement('li');\
row.appendChild(document.createTextNode('Two'));\
list.appendChild(row);\
</script><li>Three</li></ul></body></html>";

#[test]
fn what_a_script_inserts_beside_itself_is_laid_out_there_in_numbers() {
    let (renderer, issues) = loaded(GROWS_BESIDE_ITSELF);
    assert!(issues.is_empty(), "{issues:?}");
    let Some(drawing) = renderer.rendered() else {
        panic!("nothing was rendered");
    };
    assert_eq!(
        drawing.layout.to_outline(&drawing.boxes),
        "block flow · document → 300×60 at (0, 0)\n\
         \x20 block flow · generic → 300×60 at (0, 0)\n\
         \x20   block flow · list → 300×60 at (0, 0)\n\
         \x20     block flow list-item · listitem → 300×20 at (0, 0)\n\
         \x20       text \"One\" → 28.50586×16.296875 at (0, 0)\n\
         \x20     block flow list-item · listitem → 300×20 at (0, 20)\n\
         \x20       text \"Two\" → 26.25×16.296875 at (0, 20)\n\
         \x20     block flow list-item · listitem → 300×20 at (0, 40)\n\
         \x20       text \"Three\" → 40.09961×16.296875 at (0, 40)\n",
        "the row the script made is between the row before it and the row the \
         parser read after it",
    );
}

#[test]
fn a_node_a_script_makes_is_numbered_before_what_the_parser_reads_after_it() {
    let (mut renderer, _) = loaded(GROWS_BESIDE_ITSELF);
    let Some(document) = renderer.document() else {
        panic!("no document");
    };
    let rows: Vec<(usize, String)> = document
        .descendants(document.root())
        .filter(|id| document.element(*id).is_some_and(|e| e.name.is_html("li")))
        .map(|id| (id.as_usize(), document.text_content(id)))
        .collect();
    let numbers: Vec<usize> = rows.iter().map(|(id, _)| *id).collect();
    let mut ascending = numbers.clone();
    ascending.sort_unstable();
    assert_eq!(
        numbers, ascending,
        "one counter (ADR 0003): the script's row was made before the parser \
         read the third: {rows:?}"
    );
    let tree = read(&mut renderer).to_outline();
    assert!(tree.contains("\"Two\""), "{tree}");
}

#[test]
fn a_script_whose_end_tag_never_comes_does_not_run() {
    let (renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><body><div>here</div><script>{}",
        record("data-ran", "'yes'"),
    ));
    // HTML: a `<script>` still open at the end of the markup is marked
    // already started and never prepared.
    assert_eq!(saw(&renderer, "data-ran"), None);
    assert!(
        issues.iter().all(|issue| !issue.contains("script 1")),
        "{issues:?}"
    );
    assert!(renderer.document().is_some_and(|d| !d.is_being_parsed()));
}

// --- Hostile pages -----------------------------------------------------------

#[test]
fn a_script_that_takes_out_the_body_the_parser_is_in_and_then_collects() {
    // The first script takes the body out of the document and holds no part
    // of it — inside a function whose frame is gone before the collection,
    // so not even a register of the script's has the body's wrapper — then
    // allocates until the heap collects: the body is a detached tree nothing
    // wraps, and the parser still has it open.
    let (mut renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><body><div id=gone>gone</div><script>\
         (function () {{ document.documentElement.lastChild.remove(); }})();\
         let grown = 'x';\
         for (let i = 0; i < 23; i++) {{ grown = grown + grown; }}\
         {}</script><div id=nobody>nobody sees this</div><script>{}</script></body></html>\
         <div>nor this</div>",
        // Item 80's members and the language as far as it goes: a
        // string's `length` needs a wrapper object (item 73).
        record("data-grown", "'eight mebibytes of it'"),
        record("data-detached-ran", "'yes'"),
    ));
    assert_eq!(
        saw(&renderer, "data-grown").as_deref(),
        Some("eight mebibytes of it"),
        "{issues:?}"
    );
    let collections = renderer
        .event_loop()
        .map(|looping| looping.objects().heap().collections());
    assert!(
        collections.is_some_and(|collections| collections > 0),
        "the test collected nothing: {collections:?}"
    );
    // The parser carried on into the body it had open — never asking a
    // released node its name — and a script it put there is not connected,
    // so it did not run.
    assert!(
        issues.iter().all(|issue| !issue.contains("tree builder")),
        "{issues:?}"
    );
    assert_eq!(saw(&renderer, "data-detached-ran"), None);
    assert!(
        issues.iter().all(|issue| !issue.contains("script 2")),
        "{issues:?}"
    );
    let Some(document) = renderer.document() else {
        panic!("no document");
    };
    assert!(!document.is_being_parsed());
    let nobody = with_id(document, "nobody");
    assert_eq!(nobody, None, "not in the document's tree");
    let tree = read(&mut renderer).to_outline();
    assert!(!tree.contains("gone") && !tree.contains("nobody"), "{tree}");
    assert!(matches!(
        renderer.handle(ToRenderer::Paint),
        FromRenderer::Painted(_)
    ));
}

#[test]
fn a_script_that_removes_its_own_element_and_the_rest_still_parses() {
    let (mut renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><body><div>one</div><script>\
         document.documentElement.lastChild.lastChild.remove();\
         {}</script><div>two</div></body></html>",
        record("data-ran", "'yes'"),
    ));
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(saw(&renderer, "data-ran").as_deref(), Some("yes"));
    let tree = read(&mut renderer).to_outline();
    assert!(
        tree.contains("\"one\"") && tree.contains("\"two\""),
        "{tree}"
    );
    let document = renderer.document();
    assert!(
        document
            .is_some_and(|document| !document.serialize_node(document.root()).contains("<script")),
        "its element is gone"
    );
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 253: `document.body` in a page's load.
//!
//! Item 247 was written as *an inline script in the middle of `<body>` reads
//! `document.body.lastChild` as its own `<script>`*, and reached the body
//! another way because `body` did not exist; this is that sentence, as
//! written. Then a script before the body reading `null`, a body a script
//! assigns laid out in numbers and read by the agent, and a refused
//! assignment said by its name in the load's issues. The reference render is
//! the corpus case `a-script-gives-a-new-body`.

use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const WINDOW: Size = Size {
    width: 300.0,
    height: 200.0,
};

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 14px } \
                     div { height: 20px }";

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
    let mut renderer = Renderer::new(fonts());
    let page = Page::new(html, WINDOW).with_sheet(SHEET);
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

/// Write `what` onto the root element as `name` — the script half of
/// [`saw`].
fn record(name: &str, what: &str) -> String {
    format!("document.documentElement.setAttribute('{name}', {what});")
}

fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

#[test]
fn a_script_in_the_middle_of_body_reads_document_body_last_child_as_itself() {
    let (renderer, issues) = loaded(&format!(
        "<!DOCTYPE html><html><head><script>{}</script></head>\
         <body><div id=before>before</div><script id=me>{}{}</script>\
         <div id=after>after</div></body></html>",
        record("data-in-head", "'' + document.body"),
        record("data-last", "document.body.lastChild.getAttribute('id')"),
        record(
            "data-same",
            "'' + (document.body === document.documentElement.lastChild)"
        ),
    ));
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        saw(&renderer, "data-in-head").as_deref(),
        Some("null"),
        "before the parser makes a body there is none"
    );
    assert_eq!(saw(&renderer, "data-last").as_deref(), Some("me"));
    assert_eq!(saw(&renderer, "data-same").as_deref(), Some("true"));
}

/// A page whose last script makes a new body holding the old body's first
/// row and a row of its own, and assigns it.
const GIVES_A_NEW_BODY: &str = "<!DOCTYPE html><html><body><div>Kept</div><div>Dropped</div>\
     <script>\
     const made = document.createElement('body');\
     made.appendChild(document.body.firstChild);\
     const row = document.createElement('div');\
     row.appendChild(document.createTextNode('Made'));\
     made.appendChild(row);\
     document.body = made;\
     </script></body></html>";

#[test]
fn a_body_a_script_assigns_is_laid_out_in_numbers_and_read_by_the_agent() {
    let (mut renderer, issues) = loaded(GIVES_A_NEW_BODY);
    assert!(issues.is_empty(), "{issues:?}");
    let Some(drawing) = renderer.rendered() else {
        panic!("nothing was rendered");
    };
    assert_eq!(
        drawing.layout.to_outline(&drawing.boxes),
        "block flow · document → 300×40 at (0, 0)\n\
         \x20 block flow · generic → 300×40 at (0, 0)\n\
         \x20   block flow · generic → 300×20 at (0, 0)\n\
         \x20     text \"Kept\" → 31.472656×16.296875 at (0, 0)\n\
         \x20   block flow · generic → 300×20 at (0, 20)\n\
         \x20     text \"Made\" → 38.158203×16.296875 at (0, 20)\n",
        "the new body's two rows, and nothing of the old body's",
    );
    let tree = read(&mut renderer).to_outline();
    assert!(
        tree.contains("\"Made\"") && !tree.contains("Dropped"),
        "{tree}"
    );
    let Some(document) = renderer.document() else {
        panic!("no document");
    };
    assert_eq!(
        document.serialize_node(document.root()),
        "<!DOCTYPE html><html><head></head><body><div>Kept</div><div>Made</div></body></html>"
    );
}

#[test]
fn a_refused_assignment_is_said_by_its_name_and_the_page_still_draws() {
    let (renderer, issues) = loaded(
        "<!DOCTYPE html><html><body><div>Here</div>\
         <script>document.body = document.createElement('div');</script></body></html>",
    );
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(
        issues.first().is_some_and(|issue| issue.starts_with(
            "script 1: uncaught: HierarchyRequestError: the body must be a body or a \
             frameset element"
        )),
        "{issues:?}"
    );
    let Some(document) = renderer.document() else {
        panic!("no document");
    };
    assert_eq!(document.change_count(), 0, "the refusal changed nothing");
    assert!(renderer.rendered().is_some(), "and the page is drawn");
}

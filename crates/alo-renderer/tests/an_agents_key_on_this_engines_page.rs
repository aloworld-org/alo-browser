/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 938: a key pressed by name is refused on a page this engine
//! drew, which takes no key from an agent yet — said by name, never taken
//! for another verb — while every refusal `perform` reaches first is still
//! reached first.
//!
//! The page has a field whose own `keydown` listener writes what it heard
//! into a paragraph; after the press the paragraph still says nothing was
//! heard, so the refusal is not a key carried out and then denied.

use alo_agent::{Key, Refusal, Target, Verb};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

fn pressed(renderer: &mut Renderer, name: &str, key: Key) -> FromRenderer {
    renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb: Verb::Press(key),
    })
}

/// The words of the paragraph the page writes what it heard into.
fn heard(renderer: &mut Renderer) -> Option<String> {
    let FromRenderer::Tree(snapshot) = renderer.handle(ToRenderer::ReadTree) else {
        return None;
    };
    let mut waiting: Vec<_> = snapshot.root.iter().collect();
    while let Some(node) = waiting.pop() {
        if let Some(name) = node.name.as_deref()
            && name.starts_with("Heard")
        {
            return Some(name.to_owned());
        }
        waiting.extend(node.children.iter());
    }
    None
}

#[test]
fn a_key_is_refused_by_name_on_this_engines_page_and_the_page_hears_nothing() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        "<!doctype html><body><p>Heard nothing</p><input aria-label='Name box'>\
         <script>const out = document.body.firstChild; const field = out.nextSibling;\
         field.addEventListener('keydown', (e) => { out.textContent = 'Heard ' + e.key; });\
         </script></body>"
            .to_owned(),
        Size::new(400.0, 300.0),
    ))));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "the page loads and says nothing: {answer:?}"
    );
    let refused = pressed(&mut renderer, "Name box", Key::Enter);
    assert!(
        matches!(
            refused,
            FromRenderer::Refused(Refusal::KeyUnbuilt {
                key: Key::Enter,
                ..
            })
        ),
        "{refused:?}"
    );
    assert_eq!(heard(&mut renderer).as_deref(), Some("Heard nothing"));
    // What `perform` refuses first is still refused first: a thing that is
    // not there, and words that take no keyboard.
    assert!(matches!(
        pressed(&mut renderer, "Nowhere", Key::Escape),
        FromRenderer::Refused(Refusal::NotFound { .. })
    ));
    assert!(matches!(
        pressed(&mut renderer, "Heard nothing", Key::Escape),
        FromRenderer::Refused(Refusal::NotOperable { .. })
    ));
}

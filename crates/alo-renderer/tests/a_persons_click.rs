/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 535, cut from 298: a person's main button reaches the element
//! it fell on (ADR 0024 § 5, ADR 0018 § 3) — and, for queue item 543, a
//! submit button's click sends its form (ADR 0051 §§ 4 and 6).
//!
//! Every page here is the page put to Chrome 154 over CDP — a 400 × 300
//! viewport, no margin, a box `b` at (50, 40) holding a span `s`, a box `o`
//! at (200, 40) — and every sequence a page's listeners record is the one
//! Chrome's recorded for the same presses, with the same members. The boxes
//! the points depend on are asserted in numbers beside them.
//!
//! Every page is pressed twice where a collection could lose something —
//! once ordinarily and once with the collector at every allocation — and
//! the two must say the same.

use alo_agent::{Outcome, Target, Verb};
use alo_dom::{Document, NodeId};
use alo_layout::{Rect, Size};
use alo_renderer::pointer::Pointer;
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

/// The probe page's boxes, and a `<p>` below them where listeners write.
const BOXES: &str = "<style>body{margin:0;font:16px sans-serif}\
     #b{position:absolute;left:50px;top:40px;width:100px;height:60px}\
     #o{position:absolute;left:200px;top:40px;width:100px;height:60px}\
     #out{position:absolute;left:0;top:250px;margin:0}</style>\
     <div id=b><span id=s>press</span></div><div id=o>other</div>";

/// What a listener writes into the `<p>`, and a listener on the document,
/// capturing, for each of a person's events, writing what it carries and
/// writing again when its microtasks run.
const RECORD: &str = "const out = document.getElementById('out'); \
     function say(what) { out.textContent = out.textContent + '|' + what; } \
     function who(node) { return node.getAttribute('id'); } \
     const kinds = ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']; \
     for (let i = 0; i < kinds.length; i++) { \
       document.addEventListener(kinds[i], e => { \
         say(e.type + ' ' + who(e.target) + ' ' + e.isTrusted + ' ' + e.bubbles + ' ' + \
             e.cancelable + ' ' + e.composed + ' ' + e.clientX + ' ' + e.clientY + ' ' + \
             e.screenX + ' ' + e.screenY + ' ' + e.button + ' ' + e.buttons + ' ' + \
             e.detail + ' ' + e.pointerId + ' ' + e.pointerType); \
         queueMicrotask(() => say('then')); \
       }, true); \
     } ";

/// `markup` after the probe page's boxes, and a script running [`RECORD`]
/// and then `script`, loaded at 400 × 300 and at an address, with the
/// collector at every allocation from then on if `stress`.
fn loaded(markup: &str, script: &str, stress: bool) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(
            format!(
                "<!doctype html><html id=html><body id=body>{BOXES}{markup}<p id=out></p>\
                 <script>{RECORD}{script}</script></body></html>"
            ),
            Size::new(400.0, 300.0),
        )
        .at(alo_url::parse("https://example.com/").unwrap_or_else(|_| alo_url::Url::about_blank())),
    )));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "the page loads and says nothing: {answer:?}"
    );
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    renderer
}

/// Send `pointer`, answering what the page said and where it asked to go —
/// or, if it was not answered as a delivery, what it was answered with as
/// the one thing said, which no test expects.
fn point(renderer: &mut Renderer, pointer: Pointer) -> (Vec<String>, Option<String>) {
    match renderer.handle(ToRenderer::Pointer(pointer)) {
        FromRenderer::Delivered {
            issues, navigation, ..
        } => (issues, navigation.map(|asked| asked.url)),
        other => (vec![format!("not a delivery: {other:?}")], None),
    }
}

/// The element whose `id` is `id`.
fn by_id(document: &Document, id: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))
}

/// What the listeners wrote, one entry each, the first `|` dropped.
fn heard(renderer: &Renderer) -> Vec<String> {
    renderer
        .document()
        .and_then(|document| Some(document.text_content(by_id(document, "out")?)))
        .unwrap_or_default()
        .split('|')
        .skip(1)
        .map(str::to_owned)
        .collect()
}

/// Where each piece of the element `id` was laid out.
fn pieces(renderer: &Renderer, id: &str) -> Vec<Rect> {
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        return Vec::new();
    };
    let Some(node) = by_id(document, id) else {
        return Vec::new();
    };
    let Some(first) = drawing
        .boxes
        .ids()
        .find(|box_id| drawing.boxes.get(*box_id).and_then(|b| b.kind.node()) == Some(node))
    else {
        return Vec::new();
    };
    match drawing.layout.fragments(first) {
        [] => drawing
            .layout
            .get(first)
            .map(|geometry| vec![geometry.border_box])
            .unwrap_or_default(),
        some => some.iter().map(|fragment| fragment.rect).collect(),
    }
}

/// Whether `rect` is at (`x`, `y`), `width` × `height`, to within a 64th
/// of a pixel.
fn at(rect: Option<&Rect>, x: f32, y: f32, width: f32, height: f32) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1.0 / 64.0;
    rect.is_some_and(|rect| {
        near(rect.left(), x)
            && near(rect.top(), y)
            && near(rect.size.width, width)
            && near(rect.size.height, height)
    })
}

/// The probe page's boxes are where Chrome put them, and the span holds
/// the point pressed.
fn assert_the_boxes(renderer: &Renderer) {
    assert!(at(pieces(renderer, "b").first(), 50.0, 40.0, 100.0, 60.0));
    assert!(at(pieces(renderer, "o").first(), 200.0, 40.0, 100.0, 60.0));
    let span = pieces(renderer, "s");
    assert!(
        span.first().is_some_and(|rect| {
            (rect.left()..rect.right()).contains(&70.6)
                && (rect.top()..rect.bottom()).contains(&50.3)
        }),
        "the span holds the point pressed: {span:?}"
    );
}

/// Chrome's line for each event of a press at (70.6, 50.3) on `target`:
/// type, target, `isTrusted`, `bubbles`, `cancelable`, `composed`, the
/// coordinates twice, `button`, `buttons`, `detail`, `pointerId` and
/// `pointerType`.
fn chromes(kind: &str, target: &str) -> String {
    let rest = match kind {
        "pointerdown" => {
            "70.5999984741211 50.29999923706055 70.5999984741211 50.29999923706055 0 1 0 1 mouse"
        }
        "mousedown" => "70 50 70 50 0 1 1 undefined undefined",
        "pointerup" => {
            "70.5999984741211 50.29999923706055 70.5999984741211 50.29999923706055 0 0 0 1 mouse"
        }
        "mouseup" => "70 50 70 50 0 0 1 undefined undefined",
        "click" => "70 50 70 50 0 0 1 1 mouse",
        _ => "",
    };
    format!("{kind} {target} true true true true {rest}")
}

#[test]
fn a_click_fires_what_chrome_fires_in_its_order_each_with_its_checkpoint() {
    for stress in [false, true] {
        let mut renderer = loaded("", "", stress);
        assert_the_boxes(&renderer);

        let (said, went) = point(&mut renderer, Pointer::down(70.6, 50.3));
        assert!(said.is_empty() && went.is_none(), "{said:?} {went:?}");
        assert_eq!(
            heard(&renderer),
            [
                chromes("pointerdown", "s"),
                "then".to_owned(),
                chromes("mousedown", "s"),
                "then".to_owned(),
            ],
            "a press is one task: pointerdown, then mousedown (stress {stress})"
        );

        let (said, went) = point(&mut renderer, Pointer::up(70.6, 50.3));
        assert!(said.is_empty() && went.is_none(), "{said:?} {went:?}");
        assert_eq!(
            heard(&renderer).get(4..).map(<[String]>::to_vec),
            Some(vec![
                chromes("pointerup", "s"),
                "then".to_owned(),
                chromes("mouseup", "s"),
                "then".to_owned(),
                chromes("click", "s"),
                "then".to_owned(),
            ]),
            "a release is the next: pointerup, mouseup, click (stress {stress})"
        );
    }
}

/// Only the type and target of each event a listener heard.
fn kinds(renderer: &Renderer) -> Vec<String> {
    heard(renderer)
        .into_iter()
        .filter(|line| line != "then")
        .map(|line| line.split(' ').take(2).collect::<Vec<_>>().join(" "))
        .collect()
}

#[test]
fn a_cancelled_pointerdown_keeps_the_mouse_events_from_firing_and_not_the_click() {
    for stress in [false, true] {
        let mut renderer = loaded(
            "",
            "document.addEventListener('pointerdown', e => e.preventDefault());",
            stress,
        );
        point(&mut renderer, Pointer::down(70.6, 50.3));
        point(&mut renderer, Pointer::up(70.6, 50.3));
        assert_eq!(
            kinds(&renderer),
            ["pointerdown s", "pointerup s", "click s"],
            "stress {stress}"
        );
    }
}

#[test]
fn a_cancelled_mousedown_pointerup_or_mouseup_changes_nothing() {
    for kind in ["mousedown", "pointerup", "mouseup"] {
        let mut renderer = loaded(
            "",
            &format!("document.addEventListener('{kind}', e => e.preventDefault());"),
            false,
        );
        point(&mut renderer, Pointer::down(70.6, 50.3));
        point(&mut renderer, Pointer::up(70.6, 50.3));
        assert_eq!(
            kinds(&renderer),
            [
                "pointerdown s",
                "mousedown s",
                "pointerup s",
                "mouseup s",
                "click s"
            ],
            "{kind} cancelled"
        );
    }
}

#[test]
fn a_release_somewhere_else_clicks_the_nearest_element_both_are_in() {
    let mut renderer = loaded("", "", false);
    point(&mut renderer, Pointer::down(70.0, 50.0));
    point(&mut renderer, Pointer::up(250.0, 50.0));
    assert_eq!(
        kinds(&renderer),
        [
            "pointerdown s",
            "mousedown s",
            "pointerup o",
            "mouseup o",
            "click body"
        ]
    );
}

#[test]
fn a_release_outside_the_page_is_the_root_elements() {
    let mut renderer = loaded("", "", false);
    point(&mut renderer, Pointer::down(70.0, 50.0));
    point(&mut renderer, Pointer::up(500.0, 50.0));
    assert_eq!(
        kinds(&renderer),
        [
            "pointerdown s",
            "mousedown s",
            "pointerup html",
            "mouseup html",
            "click html"
        ]
    );
}

#[test]
fn no_click_when_the_pressed_element_has_left_the_page() {
    for (removed, released) in [("s", "b"), ("b", "html")] {
        let mut renderer = loaded(
            "",
            &format!(
                "document.addEventListener('mousedown', \
                 () => document.getElementById('{removed}').remove());"
            ),
            false,
        );
        point(&mut renderer, Pointer::down(70.6, 50.3));
        point(&mut renderer, Pointer::up(70.6, 50.3));
        assert_eq!(
            kinds(&renderer),
            [
                "pointerdown s".to_owned(),
                "mousedown s".to_owned(),
                format!("pointerup {released}"),
                format!("mouseup {released}"),
            ],
            "{removed} removed"
        );
    }
}

#[test]
fn a_press_outside_the_page_fires_nothing_and_its_release_no_click() {
    let mut renderer = loaded("", "", false);
    let (said, went) = point(&mut renderer, Pointer::down(-1.0, 50.0));
    assert!(said.is_empty() && went.is_none());
    assert!(heard(&renderer).is_empty(), "{:?}", heard(&renderer));
    point(&mut renderer, Pointer::up(70.0, 50.0));
    assert_eq!(kinds(&renderer), ["pointerup s", "mouseup s"]);
}

#[test]
fn a_persons_click_on_a_link_follows_it_unless_a_listener_cancels_it() {
    let link = "<a id=l href=/next style=position:absolute;left:50px;top:150px>link</a>";
    for stress in [false, true] {
        let mut renderer = loaded(link, "", stress);
        let rect = pieces(&renderer, "l");
        assert!(
            rect.first().is_some_and(|rect| at(
                Some(rect),
                50.0,
                150.0,
                rect.size.width,
                rect.size.height
            )),
            "the link is where it was placed: {rect:?}"
        );
        point(&mut renderer, Pointer::down(60.0, 155.0));
        let (said, went) = point(&mut renderer, Pointer::up(60.0, 155.0));
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(went.as_deref(), Some("https://example.com/next"));
        assert_eq!(kinds(&renderer).last().map(String::as_str), Some("click l"));
    }

    let mut renderer = loaded(
        link,
        "document.addEventListener('click', e => e.preventDefault());",
        false,
    );
    point(&mut renderer, Pointer::down(60.0, 155.0));
    let (_, went) = point(&mut renderer, Pointer::up(60.0, 155.0));
    assert_eq!(went, None, "a cancelled click follows nothing");
}

#[test]
fn a_click_on_a_link_asks_for_what_the_agents_activate_asks_for() {
    let link = "<a id=l href=/next style=position:absolute;left:50px;top:150px>link</a>";
    let mut person = loaded(link, "", false);
    point(&mut person, Pointer::down(60.0, 155.0));
    let (_, went) = point(&mut person, Pointer::up(60.0, 155.0));

    let mut agent = loaded(link, "", false);
    let FromRenderer::Acted {
        outcome,
        navigation,
        ..
    } = agent.handle(ToRenderer::Act {
        target: Target::Named("link".to_owned()),
        verb: Verb::Activate,
    })
    else {
        panic!("the link can be activated");
    };
    assert!(matches!(outcome, Outcome::Followed { .. }), "{outcome:?}");
    assert_eq!(went, navigation.map(|asked| asked.url));
    assert_eq!(
        kinds(&person).last(),
        kinds(&agent).last(),
        "both are a click at the link"
    );
}

/// A page with no script at all: `body` at an address, 400 × 300.
fn scriptless(body: &str) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(
            format!("<!doctype html><body style=margin:0>{body}</body>"),
            Size::new(400.0, 300.0),
        )
        .at(alo_url::parse("https://example.com/").unwrap_or_else(|_| alo_url::Url::about_blank())),
    )));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");
    assert!(renderer.event_loop().is_none(), "nothing ran a script");
    renderer
}

#[test]
fn on_a_page_that_never_ran_script_a_click_still_follows_a_link_and_ticks_a_box() {
    let mut renderer = scriptless(
        "<a href=/next style=position:absolute;left:10px;top:10px>link</a>\
         <input type=checkbox aria-checked=false \
          style=position:absolute;left:10px;top:60px;width:20px;height:20px;margin:0>",
    );
    point(&mut renderer, Pointer::down(15.0, 15.0));
    let (said, went) = point(&mut renderer, Pointer::up(15.0, 15.0));
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(went.as_deref(), Some("https://example.com/next"));

    point(&mut renderer, Pointer::down(20.0, 70.0));
    let (said, went) = point(&mut renderer, Pointer::up(20.0, 70.0));
    assert!(said.is_empty() && went.is_none(), "{said:?} {went:?}");
    let document = renderer.document().expect("a page");
    let checkbox = document
        .descendants(document.root())
        .find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.name.is_html("input"))
        })
        .and_then(|node| document.element(node))
        .expect("a checkbox");
    assert!(checkbox.attr("checked").is_some(), "the box is ticked");
    assert_eq!(
        checkbox.attr("aria-checked"),
        Some("false"),
        "a person's click is not an agent's verb, and changes no ARIA state"
    );
    assert!(
        renderer.event_loop().is_none(),
        "no heap was made to find that nobody listens"
    );
}

/// A form sent by `GET` to `/send` with one field, and its button at
/// (10, 150), 100 × 30 with its padding and border inside.
const FORM: &str = "<form action=/send><input name=a value=1 style=position:absolute;left:300px>\
     <button id=go style=position:absolute;left:10px;top:150px;width:100px;height:30px;margin:0;box-sizing:border-box>\
     Send</button></form>";

/// Press and let go at (20, 160), inside [`FORM`]'s button: what the release
/// said, and where it asked to go.
fn sent(renderer: &mut Renderer) -> (Vec<String>, Option<String>) {
    point(renderer, Pointer::down(20.0, 160.0));
    point(renderer, Pointer::up(20.0, 160.0))
}

#[test]
fn a_click_on_a_submit_button_sends_its_form_with_script_or_without() {
    for stress in [false, true] {
        let mut renderer = loaded(
            FORM,
            "document.addEventListener('submit', e => \
               say('submit ' + who(e.target) + ' ' + e.isTrusted + ' ' + who(e.submitter)));",
            stress,
        );
        assert!(
            at(pieces(&renderer, "go").first(), 10.0, 150.0, 100.0, 30.0),
            "the button is where it was placed: {:?}",
            pieces(&renderer, "go")
        );
        let (said, went) = sent(&mut renderer);
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(went.as_deref(), Some("https://example.com/send"));
        assert_eq!(
            heard(&renderer).last().map(String::as_str),
            Some("submit null true go"),
            "the form heard a trusted submit naming its button, as Chrome's does"
        );
    }

    let mut renderer = scriptless(FORM);
    let (said, went) = sent(&mut renderer);
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(went.as_deref(), Some("https://example.com/send"));
    assert!(
        renderer.event_loop().is_none(),
        "no heap was made to find that nobody listens"
    );
}

#[test]
fn a_click_on_a_person_and_an_agents_press_ask_for_the_same_form() {
    let mut person = loaded(FORM, "", false);
    let (_, went) = sent(&mut person);

    let mut agent = loaded(FORM, "", false);
    let FromRenderer::Acted {
        outcome,
        navigation,
        ..
    } = agent.handle(ToRenderer::Act {
        target: Target::Named("Send".to_owned()),
        verb: Verb::Activate,
    })
    else {
        panic!("the button can be pressed");
    };
    assert!(matches!(outcome, Outcome::Submitted { .. }), "{outcome:?}");
    let Some(asked) = navigation else {
        panic!("the agent's press asked for nothing");
    };
    assert_eq!(went.as_deref(), Some(asked.url.as_str()));
    let FromRenderer::Delivered {
        navigation: Some(again),
        ..
    } = ({
        let mut person = loaded(FORM, "", false);
        point(&mut person, Pointer::down(20.0, 160.0));
        person.handle(ToRenderer::Pointer(Pointer::up(20.0, 160.0)))
    })
    else {
        panic!("the person's click asked for nothing");
    };
    assert_eq!(
        again.submission, asked.submission,
        "the same entries, method and enctype"
    );
}

#[test]
fn a_form_whose_fields_fail_is_not_sent_and_says_which_after_invalid_is_fired() {
    let form = "<form action=/send><input name=day required>\
                <button style=position:absolute;left:10px;top:150px;width:100px;height:30px;\
                margin:0>Send</button></form>";
    for stress in [false, true] {
        let mut renderer = loaded(
            form,
            "document.addEventListener('invalid', e => say('invalid ' + e.target.getAttribute('name')), true);",
            stress,
        );
        let (said, went) = sent(&mut renderer);
        assert_eq!(went, None);
        assert_eq!(
            said,
            [
                "the person's click: its form was not sent: input name=\"day\" valueMissing: \
                 \"Please fill out this field.\""
            ]
        );
        assert!(
            heard(&renderer).contains(&"invalid day".to_owned()),
            "{:?}",
            heard(&renderer)
        );
    }
    let (said, went) = sent(&mut scriptless(form));
    assert_eq!(went, None);
    assert_eq!(said.len(), 1, "{said:?}");
}

#[test]
fn a_form_this_engine_cannot_check_is_heard_and_said_rather_than_refused() {
    // An agent's press on this form is refused before the page hears it
    // (`a_form_sent_on_a_page_that_runs_script.rs`). A person pressed.
    let form = "<form action=/send><input name=a pattern=x value=y>\
                <button style=position:absolute;left:10px;top:150px;width:100px;height:30px;\
                margin:0>Send</button></form>";
    let mut renderer = loaded(form, "", false);
    let (said, went) = sent(&mut renderer);
    assert_eq!(went, None);
    assert_eq!(
        said,
        [
            "the person's click: its form was not sent: input name=\"a\" could fail \
             patternMismatch, which this engine does not check yet"
        ]
    );
    assert_eq!(
        kinds(&renderer).last().map(String::as_str),
        Some("click null"),
        "the page heard the click"
    );

    let dialog = "<form method=dialog><button style=position:absolute;left:10px;top:150px;\
                  width:100px;height:30px;margin:0;box-sizing:border-box>Send</button></form>";
    for mut renderer in [loaded(dialog, "", false), scriptless(dialog)] {
        let (said, went) = sent(&mut renderer);
        assert_eq!(went, None);
        assert_eq!(
            said,
            [
                "the person's click: its form was not sent: the form closes a dialog, and dialogs \
              are not built"
            ]
        );
    }
}

#[test]
fn a_page_whose_script_stopped_sends_its_form_as_a_page_nobody_listens_on() {
    let mut renderer = loaded(
        FORM,
        "document.addEventListener('submit', e => { e.preventDefault(); say('submit'); });",
        false,
    );
    renderer
        .event_loop()
        .expect("a scripted page")
        .stop_switch()
        .ask();
    // The press's task is where the page stops; from then on nobody can
    // cancel anything, so the form's own rules decide.
    let (said, went) = sent(&mut renderer);
    assert_eq!(
        went.as_deref(),
        Some("https://example.com/send"),
        "{said:?}"
    );
    assert!(
        said.iter().any(|line| line.contains("nobody heard it")),
        "{said:?}"
    );
    assert!(
        !heard(&renderer).contains(&"submit".to_owned()),
        "no listener ran: {:?}",
        heard(&renderer)
    );
}

#[test]
fn a_cancelled_submit_sends_nothing_and_says_the_page_took_it() {
    let mut renderer = loaded(
        FORM,
        "document.addEventListener('submit', e => e.preventDefault());",
        false,
    );
    let (said, went) = sent(&mut renderer);
    assert_eq!(went, None);
    assert_eq!(
        said,
        [
            "the person's click: its form was not sent: the page cancelled its submit event, and \
          took the click"
        ]
    );

    let mut renderer = loaded(
        FORM,
        "document.addEventListener('click', e => e.preventDefault());",
        false,
    );
    let (said, went) = sent(&mut renderer);
    assert_eq!(
        (said, went),
        (Vec::new(), None),
        "a cancelled click submits nothing"
    );
}

#[test]
fn a_pointer_before_anything_is_loaded_is_answered_that_nothing_is() {
    let mut renderer = Renderer::new(fonts());
    assert_eq!(
        renderer.handle(ToRenderer::Pointer(Pointer::down(1.0, 1.0))),
        FromRenderer::Failed(alo_renderer::Failure::NothingLoaded)
    );
}

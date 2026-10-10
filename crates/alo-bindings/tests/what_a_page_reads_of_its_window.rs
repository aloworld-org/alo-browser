/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 366, as ADR 0038 designs it: **a page reads the size of its
//! window, and nothing beyond it**.
//!
//! - `innerWidth` and `innerHeight` answer the [`View`]'s viewport, asked at
//!   every read, rounded to whole pixels, a half up, and clamped as a `long`,
//!   with a size that is not finite answering `0` (§ 2).
//! - `scrollX`, `scrollY`, `pageXOffset` and `pageYOffset` answer its scroll
//!   position (§ 3).
//! - Each is the window's own, enumerable and configurable, and
//!   `[Replaceable]`.
//! - A page shown no view refuses each by name (§ 5), and a page cannot be
//!   shown twice.
//! - The screen, the window's outer size and place, and the scale factor are
//!   absent (§ 1).
//!
//! And queue item 370: **an element's `scrollWidth` and `scrollHeight`**
//! answer what the view measures of it at the moment of the read (§ 4),
//! rounded and clamped as `innerWidth` is; `0` for an element the view finds
//! no box for, and for one in a document no window shows; and a page shown
//! no view, or a view that cannot measure, refuses by name.
//!
//! The view is this test's own, so it can be given sizes no window has.
//! Every script runs twice — once with the collector at every allocation —
//! and the two must agree.

use core::cell::{Cell, RefCell};
use std::rc::Rc;

use alo_bindings::{Extent, Scrolled, Unmeasured, View, adopt, install, show};
use alo_dom::{Document, NodeId, parse_document};
use alo_js::heap::Root;
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// A view whose answers the test sets.
#[derive(Debug)]
struct Lent {
    viewport: Cell<Extent>,
    scrolled: Cell<Scrolled>,
    /// The scrolling area of each element measured, by its `id`: an element
    /// not here has no box.
    areas: RefCell<Vec<(String, Extent)>>,
    /// Whether it refuses to measure, as a view whose easel is in use does.
    busy: Cell<bool>,
    /// The `id` of each element it was asked to measure, in order.
    asked: RefCell<Vec<String>>,
}

impl Lent {
    fn at(width: f64, height: f64) -> Rc<Self> {
        Rc::new(Self {
            viewport: Cell::new(Extent { width, height }),
            scrolled: Cell::new(Scrolled::default()),
            areas: RefCell::new(Vec::new()),
            busy: Cell::new(false),
            asked: RefCell::new(Vec::new()),
        })
    }

    /// The same, measuring the element with `id` as `width` × `height`.
    fn measuring(self: Rc<Self>, id: &str, width: f64, height: f64) -> Rc<Self> {
        self.areas
            .borrow_mut()
            .push((id.to_owned(), Extent { width, height }));
        self
    }
}

impl View for Lent {
    fn viewport(&self) -> Extent {
        self.viewport.get()
    }

    fn scrolled(&self) -> Scrolled {
        self.scrolled.get()
    }

    fn scrolling_area(
        &self,
        document: &Document,
        node: NodeId,
    ) -> Result<Option<Extent>, Unmeasured> {
        if self.busy.get() {
            return Err(Unmeasured);
        }
        let id = document
            .element(node)
            .and_then(|element| element.attr("id"))
            .unwrap_or_default()
            .to_owned();
        self.asked.borrow_mut().push(id.clone());
        Ok(self
            .areas
            .borrow()
            .iter()
            .find(|(of, _)| *of == id)
            .map(|(_, area)| *area))
    }
}

/// An engine with a page's document installed, shown by `view` if given one.
struct Page {
    engine: Engine,
    _root: Root,
}

impl Page {
    fn new(view: Option<Rc<Lent>>, stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
        let cell = adopt(
            engine.objects(),
            parse_document(
                "<!DOCTYPE html><html id=root><body id=body><p id=p>hi</p></body></html>",
            ),
        )
        .map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        if let Some(view) = view {
            show(&mut engine, view).map_err(|why| why.to_string())?;
        }
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
        })
    }

    /// What `source` evaluates to, after [`JOIN`].
    fn run(&mut self, source: &str) -> String {
        let program = match script(&format!("{JOIN}\n{source}")) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        match self.engine.evaluate(&program) {
            Ok(Value::Number(number)) => numeric::text_of(number),
            Ok(Value::Text(held)) => self
                .engine
                .objects()
                .units(held)
                .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
            Ok(Value::Bool(answer)) => answer.to_string(),
            Ok(Value::Undefined) => "undefined".to_owned(),
            Ok(_) => "something else".to_owned(),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }
}

/// `j`, joining an array's elements with commas — `Array.prototype.join` is
/// not built yet (item 73).
const JOIN: &str = "function j(all) { var out = ''; \
                    for (var at = 0; at < all.length; at++) { out += (at ? ',' : '') + all[at]; } \
                    return out; }";

/// Run each of `sources`, in order, in one page shown at `width` ×
/// `height`, both ways; and answer what each answered.
fn shown_at(width: f64, height: f64, sources: &[&str]) -> Vec<String> {
    let answers: Vec<Vec<String>> = [false, true]
        .into_iter()
        .map(
            |stress| match Page::new(Some(Lent::at(width, height)), stress) {
                Ok(mut page) => sources.iter().map(|source| page.run(source)).collect(),
                Err(why) => vec![why],
            },
        )
        .collect();
    assert_eq!(
        answers.first(),
        answers.get(1),
        "{sources:?} answered differently when the collector ran at every allocation"
    );
    answers.first().cloned().unwrap_or_default()
}

/// What `source` answers in a page shown at 800 × 600.
fn at_800(source: &str) -> String {
    shown_at(800.0, 600.0, &[source]).concat()
}

/// Every member, read by its bare name and through `window` and `self`.
const ALL: &str = "j([innerWidth, innerHeight, scrollX, scrollY, pageXOffset, pageYOffset, \
                   window.innerWidth, self.innerHeight])";

#[test]
fn the_viewport_is_the_views_and_the_scroll_position_is_its_top_left() {
    assert_eq!(at_800(ALL), "800,600,0,0,0,0,800,600");
    assert_eq!(
        at_800("typeof innerWidth + typeof scrollY + typeof pageXOffset"),
        "numbernumbernumber"
    );
}

#[test]
fn the_view_is_asked_at_every_read() {
    for stress in [false, true] {
        let view = Lent::at(800.0, 600.0);
        let mut page =
            Page::new(Some(Rc::clone(&view)), stress).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(
            page.run("var w = innerWidth; w + 'x' + innerHeight"),
            "800x600"
        );
        // As a `Resize` leaves it, between two tasks.
        view.viewport.set(Extent {
            width: 640.0,
            height: 480.0,
        });
        assert_eq!(page.run("innerWidth + 'x' + innerHeight"), "640x480");
        // What a script kept is what it read then: nothing is a live number.
        assert_eq!(page.run("w"), "800");
        // A position the view is given is the position a script reads.
        view.scrolled.set(Scrolled { x: 12.25, y: 300.0 });
        assert_eq!(
            page.run("j([scrollX, scrollY, pageXOffset, pageYOffset])"),
            "12.25,300,12.25,300"
        );
    }
}

#[test]
fn a_fractional_size_is_rounded_a_half_up() {
    let answers = shown_at(800.5, 599.4, &["innerWidth + 'x' + innerHeight"]);
    assert_eq!(answers, ["801x599"]);
    let answers = shown_at(0.5, 0.49, &["innerWidth + 'x' + innerHeight"]);
    assert_eq!(answers, ["1x0"]);
}

#[test]
fn a_negative_an_infinite_and_a_huge_size_are_clamped_as_a_long() {
    let read = &["innerWidth + 'x' + innerHeight", "1 / innerWidth"];
    assert_eq!(shown_at(-5.0, -0.0, read), ["0x0", "Infinity"]);
    assert_eq!(shown_at(f64::INFINITY, f64::NAN, read), ["0x0", "Infinity"]);
    assert_eq!(
        shown_at(1.0e12, f64::NEG_INFINITY, read),
        ["2147483647x0", "4.656612875245797e-10"]
    );
}

#[test]
fn a_scroll_position_that_is_not_finite_answers_zero() {
    for stress in [false, true] {
        let view = Lent::at(800.0, 600.0);
        view.scrolled.set(Scrolled {
            x: f64::NAN,
            y: f64::INFINITY,
        });
        let mut page = Page::new(Some(view), stress).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(page.run("scrollX + ',' + pageYOffset"), "0,0");
    }
}

#[test]
fn each_member_is_the_windows_own_enumerable_configurable_and_replaceable() {
    for name in [
        "innerWidth",
        "innerHeight",
        "scrollX",
        "scrollY",
        "pageXOffset",
        "pageYOffset",
    ] {
        assert_eq!(
            at_800(&format!(
                "j([window.hasOwnProperty('{name}'), window.__proto__.hasOwnProperty('{name}'), \
                  window.propertyIsEnumerable('{name}')])"
            )),
            "true,false,true",
            "{name}"
        );
        // `[Replaceable]`: assigning puts an own data property in its place,
        // which the view is no longer asked for.
        assert_eq!(
            at_800(&format!("{name} = 'mine'; {name}")),
            "mine",
            "{name}"
        );
        assert_eq!(
            at_800(&format!(
                "(function () {{ 'use strict'; window.{name} = 5; }})(); {name}"
            )),
            "5",
            "{name}"
        );
        // Configurable: deleting it leaves nothing.
        assert_eq!(
            at_800(&format!("delete window.{name}; typeof {name}")),
            "undefined",
            "{name}"
        );
    }
    // Replacing one leaves the others asking.
    assert_eq!(at_800("scrollX = 9; j([scrollX, pageXOffset])"), "9,0");
}

#[test]
fn a_page_shown_no_view_refuses_by_name() {
    for stress in [false, true] {
        let mut page = Page::new(None, stress).unwrap_or_else(|why| panic!("{why}"));
        for name in [
            "innerWidth",
            "innerHeight",
            "scrollX",
            "scrollY",
            "pageXOffset",
            "pageYOffset",
        ] {
            assert_eq!(
                page.run(&format!(
                    "try {{ {name}; 'answered' }} catch (e) {{ e.name + ': ' + e.message }}"
                )),
                format!("TypeError: this page was given no view, so it cannot say its '{name}'"),
            );
        }
        // Replacing one needs no view.
        assert_eq!(page.run("innerWidth = 3; innerWidth"), "3");
    }
}

#[test]
fn a_page_is_shown_once() {
    let mut page =
        Page::new(Some(Lent::at(800.0, 600.0)), false).unwrap_or_else(|why| panic!("{why}"));
    let again = show(&mut page.engine, Lent::at(1.0, 1.0)).map_err(|why| why.to_string());
    assert_eq!(
        again,
        Err("TypeError: this page already has a view (at byte 0)".to_owned())
    );
    assert_eq!(page.run("innerWidth"), "800", "the first view stands");
}

#[test]
fn nothing_beyond_the_window_is_answered() {
    assert_eq!(
        at_800(
            "j([typeof screen, typeof outerWidth, typeof outerHeight, typeof screenX, \
              typeof screenY, typeof screenLeft, typeof screenTop, \
              typeof devicePixelRatio])"
        ),
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined"
    );
}

/// Run `source` in a page shown by `view`, both ways; and answer what it
/// answered, the same both ways.
fn measured_by(view: impl Fn() -> Rc<Lent>, source: &str) -> String {
    let answers: Vec<String> = [false, true]
        .into_iter()
        .map(|stress| match Page::new(Some(view()), stress) {
            Ok(mut page) => page.run(source),
            Err(why) => why,
        })
        .collect();
    assert_eq!(answers.first(), answers.get(1), "{source:?}");
    answers.first().cloned().unwrap_or_default()
}

#[test]
fn an_elements_scrolling_area_is_what_the_view_measures_of_it_now() {
    let view = || {
        Lent::at(800.0, 600.0)
            .measuring("root", 800.0, 600.0)
            .measuring("body", 800.0, 253.2)
            .measuring("p", 1200.5, 18.4)
    };
    assert_eq!(
        measured_by(
            view,
            "var root = document.documentElement, body = document.body, \
               p = document.querySelectorAll('p')[0]; \
             j([root.scrollWidth, root.scrollHeight, body.scrollWidth, body.scrollHeight, \
               p.scrollWidth, p.scrollHeight, typeof p.scrollWidth])"
        ),
        "800,600,800,253,1201,18,number"
    );
    // Asked at every read, of the element read, never kept.
    for stress in [false, true] {
        let lent = view();
        let mut page =
            Page::new(Some(Rc::clone(&lent)), stress).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(page.run("document.body.scrollHeight"), "253");
        // As a script's change leaves it, measured again.
        lent.areas.borrow_mut().retain(|(id, _)| id != "body");
        lent.areas.borrow_mut().push((
            "body".to_owned(),
            Extent {
                width: 800.0,
                height: 2253.0,
            },
        ));
        assert_eq!(page.run("document.body.scrollHeight"), "2253");
        assert_eq!(*lent.asked.borrow(), ["body", "body"]);
    }
}

#[test]
fn an_element_with_no_box_measures_zero() {
    // `p` has no box in this view, and neither has an element made and never
    // put in the document.
    let view = || Lent::at(800.0, 600.0).measuring("root", 800.0, 600.0);
    assert_eq!(
        measured_by(
            view,
            "var made = document.createElement('div'); \
             j([document.querySelectorAll('p')[0].scrollWidth, \
               document.querySelectorAll('p')[0].scrollHeight, made.scrollWidth, \
               made.scrollHeight])"
        ),
        "0,0,0,0"
    );
}

#[test]
fn a_measure_is_rounded_and_clamped_as_a_long() {
    let view = || {
        Lent::at(800.0, 600.0)
            .measuring("root", 0.5, 0.49)
            .measuring("body", f64::INFINITY, -3.0)
            .measuring("p", 1.0e12, f64::NAN)
    };
    assert_eq!(
        measured_by(
            view,
            "var root = document.documentElement, body = document.body, \
               p = document.querySelectorAll('p')[0]; \
             j([root.scrollWidth, root.scrollHeight, body.scrollWidth, body.scrollHeight, \
               p.scrollWidth, p.scrollHeight])"
        ),
        "1,0,0,0,2147483647,0"
    );
}

#[test]
fn the_members_are_read_only_accessors_on_element_prototype() {
    let view = || Lent::at(800.0, 600.0).measuring("body", 800.0, 253.0);
    assert_eq!(
        measured_by(
            view,
            "var body = document.body, element = body.__proto__.__proto__; \
             j([element.hasOwnProperty('scrollWidth'), \
               element.hasOwnProperty('scrollHeight'), \
               body.hasOwnProperty('scrollHeight'), \
               body.__proto__.hasOwnProperty('scrollHeight'), \
               element.propertyIsEnumerable('scrollHeight')])"
        ),
        "true,true,false,false,true"
    );
    // No setter: sloppy code's assignment is dropped, strict code's throws.
    assert_eq!(
        measured_by(
            view,
            "document.body.scrollHeight = 5; document.body.scrollHeight"
        ),
        "253"
    );
    assert_eq!(
        measured_by(
            view,
            "try { (function () { 'use strict'; document.body.scrollHeight = 5; })(); 'set' } \
             catch (e) { e.name }"
        ),
        "TypeError"
    );
    // Not an element: refused by the brand check, as every member is.
    assert_eq!(
        measured_by(
            view,
            "var fake = {}; fake.__proto__ = document.body.__proto__.__proto__; \
             try { fake.scrollWidth; 'answered' } catch (e) { e.name }"
        ),
        "TypeError"
    );
}

#[test]
fn a_page_shown_no_view_or_a_view_that_cannot_measure_refuses_by_name() {
    for stress in [false, true] {
        let mut page = Page::new(None, stress).unwrap_or_else(|why| panic!("{why}"));
        for name in ["scrollWidth", "scrollHeight"] {
            assert_eq!(
                page.run(&format!(
                    "try {{ document.body.{name}; 'answered' }} \
                     catch (e) {{ e.name + ': ' + e.message }}"
                )),
                format!("TypeError: this page was given no view, so it cannot say its '{name}'"),
            );
        }
        let busy = Lent::at(800.0, 600.0).measuring("body", 800.0, 253.0);
        busy.busy.set(true);
        let mut page = Page::new(Some(busy), stress).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(
            page.run(
                "try { document.body.scrollHeight; 'answered' } \
                 catch (e) { e.name + ': ' + e.message }"
            ),
            "TypeError: this page could not be measured for its 'scrollHeight': its renderer \
             was busy"
        );
    }
}

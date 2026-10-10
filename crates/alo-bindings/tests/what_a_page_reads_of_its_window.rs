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
//! The view is this test's own, so it can be given sizes no window has.
//! Every script runs twice — once with the collector at every allocation —
//! and the two must agree.

use core::cell::Cell;
use std::rc::Rc;

use alo_bindings::{Extent, Scrolled, View, adopt, install, show};
use alo_dom::parse_document;
use alo_js::heap::Root;
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// A view whose answers the test sets.
#[derive(Debug)]
struct Lent {
    viewport: Cell<Extent>,
    scrolled: Cell<Scrolled>,
}

impl Lent {
    fn at(width: f64, height: f64) -> Rc<Self> {
        Rc::new(Self {
            viewport: Cell::new(Extent { width, height }),
            scrolled: Cell::new(Scrolled::default()),
        })
    }
}

impl View for Lent {
    fn viewport(&self) -> Extent {
        self.viewport.get()
    }

    fn scrolled(&self) -> Scrolled {
        self.scrolled.get()
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
            parse_document("<!DOCTYPE html><html><body></body></html>"),
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

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' call-to-action section, in numbers: the page that opened
//! queue item 347, frozen by item 349.
//!
//! `cases/alo-sites-cta` pins the whole page as files. This says out loud
//! what item 349 is closed by. The page carries an inline script, so it is
//! loaded by a renderer, and it links its whole style sheet,
//! `/assets/site.css`. The renderer asks for that sheet by the URL it
//! resolves against the page's address, and the corpus answers it from the
//! `site.css` frozen beside the case, through the browser process's own
//! decision and check (ADR 0035 § 6). So the section is drawn as alo Sites
//! styles it: a band of the theme's primary blue, its heading at
//! `clamp(1.5rem, 3.5vw, 2.125rem)` — 28 px at 800 wide — and two buttons
//! side by side, centred, `0.75rem` apart.
//!
//! The skip link is `position: absolute; left: -999rem`, which takes it out
//! of flow and off the page until it is focused. Until queue item 352 it
//! was drawn in a line at the top left, pushing the section down by that
//! line; this says it no longer is. And it pins that the page's analytics
//! script, which changes nothing drawn, now runs to its end: past
//! `Date.now()` on its third line since queue item 356 gave the corpus a
//! clock stopped at [`alo_corpus::INSTANT`], past `encodeURIComponent` and
//! `location` on its eighth since queue items 359 and 360 built them, and
//! past `window.addEventListener("pagehide", …)` on its thirty-second since
//! queue item 362 made the global object a `Window`. Where its `pagehide`
//! listener stops when one is dispatched is `alo-bindings`'
//! `what_a_window_is.rs`: nothing in a load fires it (queue item 364).

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// Where alo Sites serves the page: the site's root, on its own host.
const ADDRESS: &str = "https://nordwind.alosites.com/";

/// Where its one sheet is, as the page's `href` resolves against
/// [`ADDRESS`].
const SHEET: &str = "https://nordwind.alosites.com/assets/site.css";

/// `body`'s `font-size: 1.0625rem` at `line-height: 1.6`: one line of the
/// page's text.
const LINE: f32 = 17.0 * 1.6;

/// The case, read and rendered.
fn cta() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-cta"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border box of the first element whose `class` attribute is `class`,
/// and its computed `property` if the cascade set one.
fn found(rendering: &Rendering, class: &str, property: &str) -> Option<(Rect, Option<String>)> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("class") == Some(class))
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    let value = drawing
        .styles
        .get(node)
        .and_then(|style| style.get(property))
        .map(ToOwned::to_owned);
    Some((drawing.layout.get(id)?.border_box, value))
}

/// The colour drawn at `(x, y)`, as RGBA bytes.
fn drawn_at(rendering: &Rendering, x: u32, y: u32) -> Option<[u8; 4]> {
    let rgba = rendering.drawing()?.canvas.at(x, y)?.to_rgba8();
    Some([rgba.0, rgba.1, rgba.2, rgba.3])
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = cta() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(case.address.as_deref(), Some(ADDRESS));
    assert!(case.responses.is_empty());
    assert_eq!(answered.delivered, 1);
    assert_eq!(answered.sheets, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
    // Frozen as `site.css`, so typed `text/css; charset=utf-8`, as alo Sites
    // serves it: nothing to say about it.
    assert!(answered.said.is_empty(), "{:?}", answered.said);
    // The sheet's task said nothing: it arrived.
    assert!(answered.issues.is_empty(), "{:?}", answered.issues);
    // What the load said: its first draw was before the sheet's answer and
    // said so truly. Nothing else: the analytics script runs past
    // `Date.now()` on its third line (queue item 356), past
    // `encodeURIComponent(location.pathname)` on its eighth (queue items 359
    // and 360), past `window.addEventListener` on its thirty-second (queue
    // item 362) and on to its end.
    assert_eq!(
        answered.loaded,
        ["no style sheet was loaded for \"/assets/site.css\""]
    );
    let origin = std::fs::read_to_string(case.expectation("origin.txt")).unwrap_or_default();
    assert!(origin.contains(SHEET), "origin.txt does not say {SHEET}");
}

#[test]
fn the_section_is_drawn_as_the_sheet_styles_it() {
    let Some((_, rendering)) = cta() else {
        panic!("the case renders");
    };
    // The band: the theme's `--primary`, below its last line of content.
    let Some((band, _)) = found(&rendering, "s-cta cta-two-actions", "color") else {
        panic!("the section is drawn");
    };
    assert_eq!(drawn_at(&rendering, 40, 250), Some([29, 78, 216, 255]));
    // `max-width: 70rem` is wider than the page, so the band is the page's
    // width, and its height is its padding and what is in it.
    assert_eq!((band.origin.x, band.size.width), (0.0, 800.0));
    let heading = 28.0 * 1.2 + 14.0;
    let paragraph = LINE + 17.0;
    let buttons = LINE + 2.0 * 9.6 + 2.0 + 17.0;
    assert!(
        (band.size.height - (48.0 + heading + paragraph + buttons + 48.0)).abs() < 0.01,
        "{band:?}"
    );

    // Two buttons, `0.75rem` apart and centred between the band's padding:
    // a white one in blue text, and a clear one in white text.
    let Some((order, colour)) = found(&rendering, "button", "color") else {
        panic!("the first button is drawn");
    };
    // As `var(--primary)` wrote it.
    assert_eq!(colour.as_deref(), Some("#1d4ed8"));
    let Some((subscriptions, colour)) = found(&rendering, "button secondary", "color") else {
        panic!("the second button is drawn");
    };
    assert_eq!(colour.as_deref(), Some("#ffffff"));
    // Inside each, clear of its text and its rounded corners: six pixels in
    // from its left edge, and halfway down a row that starts at 139.8.
    assert_eq!(drawn_at(&rendering, 224, 164), Some([255, 255, 255, 255]));
    assert_eq!(drawn_at(&rendering, 377, 164), Some([29, 78, 216, 255]));
    assert!((order.origin.y - 139.8).abs() < 0.01, "{order:?}");
    assert!((order.size.height - (LINE + 2.0 * 9.6 + 2.0)).abs() < 0.01);
    assert!((subscriptions.origin.x - (order.origin.x + order.size.width + 12.0)).abs() < 0.01);
    let left = order.origin.x - 24.0;
    let right = 800.0 - 24.0 - (subscriptions.origin.x + subscriptions.size.width);
    assert!((left - right).abs() < 0.01, "{left} and {right}");
}

/// Queue item 352: the skip link, an absolutely positioned inline, is
/// blockified and laid out at its offsets, off the page, and takes no room
/// in the flow, so the section starts at the top.
#[test]
fn the_skip_link_is_out_of_flow_and_off_the_page() {
    let Some((_, rendering)) = cta() else {
        panic!("the case renders");
    };
    let Some((link, position)) = found(&rendering, "skip-link", "position") else {
        panic!("the skip link is laid out");
    };
    assert_eq!(position.as_deref(), Some("absolute"));
    // `left: -999rem; top: 0`, against the page, with `padding: 0.5rem 1rem`
    // around one line of `body`.
    assert!((link.origin.x - -999.0 * 16.0).abs() < 0.01, "{link:?}");
    assert!(link.origin.y.abs() < 0.01, "{link:?}");
    assert!(
        (link.size.height - (LINE + 2.0 * 8.0)).abs() < 0.01,
        "{link:?}"
    );
    // The page's own white at the top left, not the link's `--surface`.
    assert_eq!(drawn_at(&rendering, 2, 2), Some([255, 255, 255, 255]));
    let Some((band, _)) = found(&rendering, "s-cta cta-two-actions", "color") else {
        panic!("the section is drawn");
    };
    assert!(band.origin.y.abs() < 0.01, "{band:?}");
    // The band's own blue, where the link's line used to be.
    assert_eq!(drawn_at(&rendering, 40, 10), Some([29, 78, 216, 255]));
}

/// The root element's attribute `name`, as a script in the page left it.
fn left(rendering: &Rendering, name: &str) -> Option<String> {
    let document = rendering.document()?;
    let html = document
        .descendants(document.root())
        .find(|id| document.element(*id).is_some())?;
    document.element(html)?.attr(name).map(ToOwned::to_owned)
}

/// Queue item 366 (ADR 0038): the analytics script reads the window it is
/// drawn in, 800 × 600 at its top, and `shape()` reports that width.
///
/// Its own two ways to `shape()` each need something not built. `record`,
/// on `pagehide`, calls it only once the page's height is known, which is
/// `scrollHeight` (queue item 370); the click listener calls it only for an
/// event with a numeric `pageX`, which no event has yet. So a later task
/// lends what it needs and nothing else, as a timer or an event will run
/// one: a beacon in `navigator.sendBeacon`'s place (queue item 369), and a
/// click with `pageX` and `pageY` of `0`. Run ordinarily and with the
/// collector at every allocation.
#[test]
fn its_script_reads_the_window_it_is_drawn_in_and_reports_its_width() {
    for stress in [false, true] {
        let Some((_, mut rendering)) = cta() else {
            panic!("the case renders");
        };
        let Rendering::Loaded(renderer, _) = &mut rendering else {
            panic!("the case is loaded by a renderer");
        };
        let looping = renderer
            .event_loop()
            .unwrap_or_else(|| panic!("the page's script ran"));
        looping.engine().objects().heap_mut().stress(stress);
        let queued = looping.queue_script(
            "later",
            "var root = document.documentElement; \
             root.setAttribute('data-read', innerWidth + 'x' + innerHeight + ' at ' + \
               scrollX + ',' + scrollY); \
             var sent = ''; \
             navigator.sendBeacon = function (to, body) { sent += body + ';'; return true; }; \
             window.dispatchEvent(new Event('pagehide')); \
             var click = new Event('click', { bubbles: true }); \
             click.pageX = 0; click.pageY = 0; \
             document.body.dispatchEvent(click); \
             root.setAttribute('data-sent', sent);",
        );
        assert!(queued.is_ok(), "{queued:?}");
        let turn = looping.run_next();
        assert!(
            turn.as_ref()
                .is_some_and(|turn| turn.reports.is_empty() && turn.stopped.is_none()),
            "{turn:?}"
        );
        assert_eq!(
            left(&rendering, "data-read").as_deref(),
            Some("800x600 at 0,0")
        );
        // `pagehide`: it reaches 600 pixels down, but over a height that is
        // `NaN` until item 370 that is no share, so no depth; then the
        // seconds read, none on the corpus's fixed clock. The click: no
        // share of a width or a height not known yet, and `shape()`: the
        // page's path, and the window's width.
        assert_eq!(
            left(&rendering, "data-sent").as_deref(),
            Some("t=0;x=0&y=0&p=%2F&w=800;"),
            "stress: {stress}"
        );
    }
}

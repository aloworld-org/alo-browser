/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What every drawing of a page is made with, and the last one made.
//!
//! ADR 0038 § 4: an element's `scrollWidth` is measured **from the layout
//! the page would be drawn with if it were drawn now** — the same sheets in
//! the same order, the linked sheets that have arrived, the same judgment of
//! inline style under the page's policies, the same fonts and the same
//! viewport — and that layout is **kept as the next drawing's**. A
//! measurement is asked in the middle of a script, through the page's view
//! ([`crate::view`]), where the renderer cannot be reached. So what a drawing
//! is made with is here, shared by the renderer and the view, and there is
//! one [`Easel::draw`] for both: two pipelines would be two answers the day
//! they disagreed.
//!
//! # Who holds it, and when
//!
//! The renderer holds it for its whole life, as an `Rc<RefCell<Easel>>`, and
//! each page's view holds the same one. The renderer borrows it only for as
//! long as one of its own steps lasts, and **never across a step that runs
//! the page's script**; the view borrows it only for one measurement, which
//! runs no script, since nothing reachable from style, boxes, text or layout
//! is JavaScript. So the two borrows never meet. A view that found it in use
//! anyway refuses to measure rather than wait or guess
//! ([`alo_bindings::Unmeasured`]).
//!
//! # When the page is drawn again
//!
//! [`Easel::fresh`] draws again when the document's change count has moved
//! since the last drawing, or something it is drawn with has: a sheet
//! arrived, or the page's policies changed as its `<meta>`s were parsed. A
//! measurement asks the same question, so a page that changes its document,
//! reads `scrollHeight` and is then painted is laid out once, not twice.
//!
//! A measurement draws the page whole, paint included, and keeps all of it,
//! so the next `Paint` is the drawing the measurement made. Stopping before
//! paint would save the paint of a drawing the script changes again before
//! anything shows it; that is a speed, and is queue item 372's.

use std::rc::Rc;

use alo_dom::Document;
use alo_layout::Size;
use alo_net::Policies;
use alo_text::FontDatabase;
use alo_url::Url;

use crate::inline_style::Judged;
use crate::linked::Linked;
use crate::objected::Objected;
use crate::page::Page;
use crate::pipeline::{Drawing, draw};

/// What a page is drawn with, and what it was last drawn as.
pub struct Easel {
    /// The renderer's fonts, every one the browser process handed over.
    pub(crate) fonts: FontDatabase,
    /// The page's own sheets, in order, as one.
    sheets: String,
    /// Where the page is, which its linked sheets resolve against.
    url: Url,
    /// The page's linked style sheets: asked for, and arrived.
    pub(crate) linked: Linked,
    /// Every enforced policy the page holds — its headers' and each
    /// `<meta>`'s the parser made so far — asked of its inline style at
    /// every draw (ADR 0034 § 2).
    under: Policies,
    /// The policies its headers stated, report-only ones included: those an
    /// objection is named against ([`Page::stated`]).
    stated: Policies,
    /// What those policies have objected to in the page's inline style, for
    /// the page's life, and what of it waits to cross (ADR 0034 § 4).
    pub(crate) objected: Objected,
    /// Whether what the page is drawn with changed since it was last drawn,
    /// which changes its rendering without changing its document.
    pub(crate) restyle: bool,
    /// What the last drawing produced, and the document's change count when
    /// it was made.
    drawn: Option<(Rc<Drawing>, u64)>,
    /// How many times a page has been drawn, for a test that asks how often.
    draws: u64,
}

impl core::fmt::Debug for Easel {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // A drawing is every tree of a page and its pixels: what is said of it
        // is whether there is one, and of which change.
        out.debug_struct("Easel")
            .field("url", &self.url)
            .field("restyle", &self.restyle)
            .field("drawn_at", &self.drawn.as_ref().map(|(_, count)| *count))
            .field("draws", &self.draws)
            .finish_non_exhaustive()
    }
}

impl Easel {
    /// An easel with these fonts and no page on it.
    pub fn new(fonts: FontDatabase) -> Self {
        Self {
            fonts,
            sheets: String::new(),
            url: Url::about_blank(),
            linked: Linked::new(),
            under: Policies::none(),
            stated: Policies::none(),
            objected: Objected::new(),
            restyle: false,
            drawn: None,
            draws: 0,
        }
    }

    /// A new page on it, drawn with `page`'s sheets at its address under its
    /// headers' policies: nothing drawn, nothing linked, nothing objected
    /// to. The fonts stay.
    pub(crate) fn begin(&mut self, page: &Page) {
        self.sheets = page.sheets.join("\n");
        self.url = page.url.clone();
        self.linked = Linked::new();
        self.under = Page::policies_of(&page.policies);
        self.stated = page.stated();
        self.objected = Objected::new();
        self.restyle = false;
        self.drawn = None;
    }

    /// No page on it: the one it had was left (ADR 0039 § 2), and nothing
    /// is drawn until the next is loaded. The fonts stay.
    pub(crate) fn clear(&mut self) {
        self.sheets = String::new();
        self.url = Url::about_blank();
        self.linked = Linked::new();
        self.under = Policies::none();
        self.stated = Policies::none();
        self.objected = Objected::new();
        self.restyle = false;
        self.drawn = None;
    }

    /// The page holds `under` from now on: its headers' policies and every
    /// `<meta>`'s parsed so far. Its inline style is judged by them at the
    /// next drawing, whether or not its document changed.
    pub(crate) fn govern(&mut self, under: Policies) {
        self.under = under;
        self.restyle = true;
    }

    /// Draw `document` again, whole, at `viewport`, its inline style judged
    /// by the policies the page holds ([`crate::inline_style`]).
    ///
    /// What the judgment objected to for the first time waits in
    /// [`Objected`] for the next answer that carries objections — this
    /// drawing's own, or a later one's (queue item 346).
    pub(crate) fn draw(&mut self, document: &Document, viewport: Size) {
        let judged = Judged::of(document, &self.under, &self.stated, &mut self.objected);
        let (found, more) = judged.objections();
        self.objected.owe(found, more);
        let linked = self.linked.for_draw(document, &self.url);
        let drawing = draw(
            document,
            &self.sheets,
            viewport,
            &self.fonts,
            &linked,
            &[],
            &judged,
        );
        self.drawn = Some((Rc::new(drawing), document.change_count()));
        self.draws = self.draws.saturating_add(1);
        self.restyle = false;
    }

    /// Draw `document` again if it has changed since it was last drawn, or
    /// what it is drawn with has — the one question every reader of a
    /// rendering asks first, a measurement among them.
    pub(crate) fn fresh(&mut self, document: &Document, viewport: Size) {
        let then = self.drawn.as_ref().map(|(_, count)| *count);
        if then != Some(document.change_count()) || self.restyle {
            self.draw(document, viewport);
        }
    }

    /// What the last drawing produced.
    pub fn drawn(&self) -> Option<&Rc<Drawing>> {
        self.drawn.as_ref().map(|(drawing, _)| drawing)
    }

    /// How many times a page has been drawn on it.
    pub const fn draws(&self) -> u64 {
        self.draws
    }
}

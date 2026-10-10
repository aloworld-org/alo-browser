/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The renderer: one page, and the answers to questions about it.
//!
//! ADR 0005's boundary, as a type. [`Renderer::handle`] takes work and returns
//! a result, and that is the whole of its surface — no callback, no handle to
//! call back through, nowhere to wait. A renderer in another process would
//! have exactly this shape, which is the point of building it before there is
//! one.
//!
//! # Nothing ambient
//!
//! Everything a renderer needs arrives in a message or in [`Renderer::new`],
//! but for one thing it reads itself: the time a page's `Date` reads, which
//! is the machine's wall clock in whole milliseconds ([`crate::clock`], ADR
//! 0036 § 2). A renderer that must draw the same on every day — a corpus
//! case's — is handed a fixed clock ([`Renderer::told_the_time_by`]).
//! Fonts are the interesting case: a sandboxed renderer cannot open a font
//! file, so in the split they are handed to it by the browser process. They
//! are a constructor argument here for that reason rather than for tidiness.
//!
//! # A page's script
//!
//! Each page loaded gets its own [`EventLoop`], and so its own engine and
//! realm: a renderer serves one site (ADR 0005), and the globals one of its
//! pages left behind are not the next page's to find. A `Load` is one task
//! per script the page carries, each run as the parser reaches its end tag
//! (ADR 0016 § 2, ADR 0017 § 7, [`crate::scripts`]), and its answer comes
//! after the whole parse, every one of them and their jobs.
//!
//! # The page's document, and when it is drawn again
//!
//! ADR 0017 §§ 2 and 6. The document is the renderer's until the page's
//! first script is about to run, and then the page's heap's ([`Held`]);
//! every render **borrows** it, wherever it is. What was drawn is kept with
//! the document's change count at the time, and the page is **drawn again
//! whole, from the same document**, whenever something reads its rendering
//! and the count has moved: a `Paint`, a `ReadTree`, an `Act`'s decision.
//! A `Load` draws once, after its last script, so what it reports is about
//! the page its scripts left; a `Resize` draws the document the page has,
//! never its markup again. **Never inside a task**: a page that changes its
//! document ten thousand times in one script is drawn once.
//!
//! # An agent's verb on a page that runs script
//!
//! `Activate` there is a `click` (ADR 0018 § 5): one task on the page's
//! loop, its listeners called and its activation steps run, and the `Act`
//! answered after it with what the page's script said ([`crate::press`]).
//!
//! # Where a page asks to go
//!
//! A navigation is a claim in the answer to the message whose work made it
//! (ADR 0020 § 1): a `Load`'s scripts' clicks in its `Loaded`, an `Act`'s
//! link — the agent's own, or a listener's `click()` — in its `Acted`. The
//! page's ongoing navigation is taken when the work is done ([`crate::ask`])
//! and the renderer never learns what became of it.
//!
//! # What a page reads of its window
//!
//! Its viewport and its scroll position, through a [`PageView`] made for
//! each page loaded and handed to its realm (ADR 0038 § 5,
//! [`crate::view`]). A `Resize` sets it with the page's size, so a script
//! reads the size the page is drawn at.
//!
//! And its elements' scrolling areas, measured **in the middle of a script**
//! (§ 4). So what a page is drawn with and its last drawing are not the
//! renderer's alone: they are on an [`Easel`] the renderer and the page's
//! view share, and a measurement draws on it as the renderer would and
//! leaves its drawing there for the renderer's next look. The renderer
//! borrows the easel only for as long as one of its own steps lasts, never
//! across one that runs the page's script ([`crate::easel`]).
//!
//! # What a page asks to fetch
//!
//! Likewise a claim in the answer to the message whose work made it (ADR
//! 0032 § 1) — every ask, in order — and the answer to each comes back as a
//! message of its own, [`ToRenderer::Fetched`], whose handling is a task
//! that settles the page's promise ([`crate::deliver`]). Its answer carries
//! what the reactions asked for in turn.
//!
//! # What a page links
//!
//! Its style sheets are asked for the same way (ADR 0035): at the end of every
//! message whose work could have changed the document, each linked sheet not
//! asked for before is a claim in the answer ([`crate::linked`]), and its
//! answer is a message of its own, [`ToRenderer::Sheet`], whose task keeps the
//! bytes and draws the page with them.
//!
//! What is not here yet is the loop running between messages — a task a
//! page queues for itself has no idle moment to run in (queue item 233).

use core::cell::RefCell;
use std::rc::Rc;

use crate::ask;
use crate::clock::WallClock;
use crate::deliver::deliver;
use crate::easel::Easel;
use crate::event_loop::EventLoop;
use crate::face::Face;
use crate::fetch::{FetchAsk, Fetched};
use crate::frame::Frame;
use crate::generic::Generics;
use crate::held::Held;
use crate::message::{Failure, FromRenderer, ToRenderer};
use crate::page::Page;
use crate::pipeline::Drawing;
use crate::press::press;
use crate::put::put;
use crate::said;
use crate::scripts;
use crate::sheet::{SheetAnswer, SheetAsk};
use crate::snapshot::Snapshot;
use crate::view::PageView;
use alo_agent::{AgentTree, apply, perform};
use alo_agent::{Outcome, Target, Verb};
use alo_bindings::navigating::{self, By};
use alo_dom::{Document, Parsing};
use alo_layout::Size;
use alo_text::Font;
use alo_text::FontDatabase;

/// Everything that touches a page.
pub struct Renderer {
    /// What the page is drawn with — the fonts, its sheets, its policies,
    /// its linked sheets and what they objected to — and its last drawing,
    /// shared with the page's view so that a script can be measured on it
    /// ([`crate::easel`]).
    easel: Rc<RefCell<Easel>>,
    /// The page as it was sent: its sheets and its size, which every drawing
    /// of it is made with.
    page: Option<Page>,
    /// The page's document, and its script once it has run.
    held: Option<Held>,
    /// The policies each `<meta>` the parser made stated — enforced, and
    /// only those, since a header policy is the browser process's to apply to
    /// a linked sheet (ADR 0035 § 1).
    metas: alo_net::Policies,
    /// What every page's realm is told the time by: the machine's wall clock
    /// (ADR 0036 § 2), unless the renderer was made with another.
    clock: Rc<dyn alo_js::Clock>,
    /// How the page is shown, as its script reads it: its viewport, kept
    /// with [`Page::viewport`] at load and at every resize, and its scroll
    /// position (ADR 0038 §§ 2–3). One for each page loaded.
    view: Rc<PageView>,
}

impl Renderer {
    /// A renderer that draws with these fonts and holds no page yet.
    pub fn new(fonts: FontDatabase) -> Self {
        let easel = Rc::new(RefCell::new(Easel::new(fonts)));
        Self {
            view: Rc::new(PageView::at(Size::default(), Rc::clone(&easel))),
            easel,
            page: None,
            held: None,
            metas: alo_net::Policies::none(),
            clock: Rc::new(WallClock),
        }
    }

    /// The same renderer, whose pages read the time from `clock` rather than
    /// the machine's wall clock: a fixed instant, so that a corpus case or a
    /// test draws the same pixels on every day it is run (ADR 0036 § 5).
    #[must_use]
    pub fn told_the_time_by(mut self, clock: Rc<dyn alo_js::Clock>) -> Self {
        self.clock = clock;
        self
    }
    /// Take a font the browser process handed over.
    ///
    /// A confined renderer cannot go and find one (ADR 0010), so this is the
    /// only way it gets any. Bytes that do not parse are **refused here**
    /// rather than kept: a font that fails at the moment text is shaped fails a
    /// long way from the moment somebody could have been told.
    fn use_font(&mut self, face: &Face) -> FromRenderer {
        match Font::load(&face.family, face.weight(), face.slant, face.bytes.clone()) {
            Some(font) => {
                let family = font.family().to_owned();
                self.easel.borrow_mut().fonts.add(font);
                FromRenderer::UsingFont { family }
            }
            None => FromRenderer::Failed(Failure::NotAFont {
                family: face.family.clone(),
            }),
        }
    }

    /// Take what the browser process says the generic families mean.
    ///
    /// A renderer cannot work this out for itself — `sans-serif` is a fact about
    /// the machine, and the machine is what ADR 0010 confines it away from. What
    /// it *can* do is say which of them it can now answer, and that is what
    /// comes back: a generic whose family is not among the faces this renderer
    /// holds resolves to nothing, and reporting it as understood would tell the
    /// browser process every page here has a `sans-serif` while text kept coming
    /// out in whatever was to hand.
    fn use_generics(&mut self, generics: &Generics) -> FromRenderer {
        let mut easel = self.easel.borrow_mut();
        for (generic, family) in generics.pairs() {
            easel.fonts.map_generic(generic, family);
        }
        let answering = generics
            .named()
            .into_iter()
            .filter(|generic| easel.fonts.holds(generic))
            .map(ToOwned::to_owned)
            .collect();
        FromRenderer::UsingGenerics { answering }
    }

    /// Do one piece of work, and answer.
    ///
    /// The only way in. Every request is answered — with a result, with a
    /// refusal, or with a [`Failure`] that leaves the renderer usable.
    pub fn handle(&mut self, work: ToRenderer) -> FromRenderer {
        match work {
            ToRenderer::UseFont(face) => self.use_font(&face),
            ToRenderer::UseGenerics(generics) => self.use_generics(&generics),
            ToRenderer::Load(page) => self.load(*page),
            ToRenderer::Resize(viewport) => self.resize(viewport),
            ToRenderer::Paint => self.paint(),
            ToRenderer::ReadTree => self.read_tree(),
            ToRenderer::Act { target, verb } => self.act(&target, &verb),
            ToRenderer::Fetched(fetched) => self.delivered(&fetched),
            ToRenderer::Sheet(answer) => self.styled(&answer),
        }
    }

    /// What the last drawing produced, for a test that asserts on the
    /// engine's insides.
    ///
    /// Not part of the boundary and never sent anywhere: a display list is not
    /// something a browser process asks for. The corpus reaches in because it
    /// is a test of the engine rather than of the browser, and ADR 0005 says
    /// tests stay single-process.
    pub fn rendered(&self) -> Option<Rc<Drawing>> {
        self.easel.borrow().drawn().cloned()
    }

    /// The loaded page's document, wherever it is — for a test, for the
    /// reason [`Renderer::rendered`] is one.
    pub fn document(&self) -> Option<&Document> {
        self.held.as_ref().and_then(Held::document)
    }

    /// The loaded page's event loop, if any script of its ran, for a test that
    /// reads what the script left behind.
    ///
    /// Not part of the boundary, for the reason [`Renderer::rendered`] is not.
    pub fn event_loop(&mut self) -> Option<&mut EventLoop> {
        self.held.as_mut().and_then(Held::event_loop)
    }

    /// How many times a page has been drawn since this renderer was made —
    /// for a test that asks whether reading a rendering drew it again, and a
    /// change inside a task did not (ADR 0017 § 6).
    ///
    /// Not part of the boundary, for the reason [`Renderer::rendered`] is not.
    pub fn draws(&self) -> u64 {
        self.easel.borrow().draws()
    }

    /// Draw the page again, whole, from the document it has, its inline
    /// style judged by the policies it holds ([`crate::inline_style`]).
    ///
    /// What the judgment objected to for the first time waits in
    /// [`Objected`] for the next answer that carries objections — this
    /// draw's own, or a later one's if a `Paint` or a `ReadTree` asked for
    /// it (queue item 346).
    fn draw(&mut self) {
        let (Some(page), Some(document)) =
            (&self.page, self.held.as_ref().and_then(Held::document))
        else {
            return;
        };
        self.easel.borrow_mut().draw(document, page.viewport);
    }

    /// Draw the page again if its document has changed since it was last
    /// drawn, or a sheet has arrived — the one question every reader of a
    /// rendering asks first. A drawing a script's measurement made since is
    /// the last drawing, and is not made again ([`crate::easel`]).
    fn fresh(&mut self) {
        let (Some(page), Some(document)) =
            (&self.page, self.held.as_ref().and_then(Held::document))
        else {
            return;
        };
        self.easel.borrow_mut().fresh(document, page.viewport);
    }

    /// Decide what a verb does, carry it into the document, and render again.
    ///
    /// Three steps, in that order, and they cannot be fewer. The **decision**
    /// is made against the tree the agent read — drawn again first if the
    /// page's script changed the document since — the **change** is made to
    /// the document, wherever it lives, and the page is **drawn again** if the
    /// change count moved, because a document that changed and a layout that
    /// did not are two structures that disagree.
    ///
    /// On a page that runs script, `Activate` is a `click` the page's
    /// listeners hear, as one task run to its end before this answers
    /// ([`crate::press`], ADR 0018 §§ 3–7): what the document becomes is the
    /// page's, and a link is followed only if nobody cancelled the click.
    /// `PutText` is likewise one task ([`crate::put`], § 5) — a
    /// `beforeinput` the page may cancel, which answers
    /// [`Outcome::TextCanceled`], or else the text, `input` and `change`. On
    /// a page that never ran script, `alo-agent`'s `apply` carries the
    /// decision in, as it did in stage 1 — and a link it follows is asked
    /// for here, since such a page has no cell to ask in.
    ///
    /// A link answers [`Outcome::Followed`] only when following it started
    /// a navigation; one that asks for a download, names another window or
    /// goes nowhere was activated, and the answer says why.
    fn act(&mut self, target: &Target, verb: &Verb) -> FromRenderer {
        self.fresh();
        // Its own handle on the drawing, not a borrow of the easel: the
        // page's script runs below, and may measure on the easel, leaving a
        // drawing of its own there. The decision is against this one, the
        // tree the agent read.
        let (Some(page), Some(held), Some(drawing)) = (
            &self.page,
            &mut self.held,
            self.easel.borrow().drawn().cloned(),
        ) else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let Some(document) = held.document() else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let tree = AgentTree::new(document, &drawing.boxes, &drawing.layout);
        let outcome = match perform(&tree, target, verb) {
            Ok(outcome) => outcome,
            Err(refusal) => return FromRenderer::Refused(refusal),
        };
        // What it is called, in case a cancelled link answers as activated.
        let name = tree
            .nodes()
            .into_iter()
            .find(|node| node.id() == outcome.node())
            .and_then(|node| node.name());
        let node = drawing
            .boxes
            .get(outcome.node())
            .and_then(|held| held.kind.node());
        let ran = match (&outcome, node) {
            (Outcome::Activated { .. } | Outcome::Followed { .. }, Some(node)) => press(held, node)
                .map(|pressed| {
                    let outcome = match pressed.follow {
                        Some(to) => Outcome::Followed {
                            node: outcome.node(),
                            to,
                        },
                        None => Outcome::Activated {
                            node: outcome.node(),
                            name: name.clone(),
                        },
                    };
                    (outcome, pressed.issues)
                }),
            (Outcome::TextPut { node: id, text }, Some(node)) => put(held, node, text).map(|put| {
                let (node, text) = (*id, text.clone());
                let outcome = if put.canceled {
                    Outcome::TextCanceled { node, text }
                } else {
                    Outcome::TextPut { node, text }
                };
                (outcome, put.issues)
            }),
            _ => None,
        };
        let mut ongoing = held.take_navigation();
        let (outcome, mut issues) = if let Some(ran) = ran {
            ran
        } else {
            // From the **same document**: working out what a changed
            // attribute could possibly have affected is a cache, and a wrong
            // cache is a wrong pixel nobody can find. Re-parsing would be
            // worse than slow — it would mint new node ids and break every
            // snapshot anybody was holding.
            held.change(|document| apply(document, &drawing.boxes, &outcome));
            let outcome = match (outcome, node, held.document()) {
                (Outcome::Followed { node: id, to }, Some(link), Some(document)) => {
                    let followed = navigating::follow(document, &page.url, link, By::Browser);
                    if ongoing.start(followed) {
                        Outcome::Followed { node: id, to }
                    } else {
                        Outcome::Activated { node: id, name }
                    }
                }
                (outcome, ..) => outcome,
            };
            (outcome, Vec::new())
        };
        let (navigation, mut said) = ask::answer(&ongoing);
        issues.append(&mut said);
        let fetches = asks(held);
        let sheets = self.sheet_asks(&mut issues);
        self.fresh();
        let mut objections = Vec::new();
        self.easel
            .borrow_mut()
            .objected
            .take(&mut objections, &mut issues);
        FromRenderer::Acted {
            outcome,
            issues,
            objections,
            navigation,
            fetches,
            sheets,
        }
    }

    /// The answer to one of the page's fetches, as a task of its own
    /// (ADR 0032 § 1): the promise waiting under its number settled, every
    /// reaction run in the checkpoint after it ([`crate::deliver`]), and the
    /// page drawn again if they changed it. What they asked for — where to
    /// go, what to fetch next — is this message's answer.
    fn delivered(&mut self, arrived: &Fetched) -> FromRenderer {
        let Some(held) = &mut self.held else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let mut issues = deliver(held, arrived);
        let ongoing = held.take_navigation();
        let (navigation, mut said) = ask::answer(&ongoing);
        issues.append(&mut said);
        let fetches = asks(held);
        let sheets = self.sheet_asks(&mut issues);
        self.fresh();
        let mut objections = Vec::new();
        self.easel
            .borrow_mut()
            .objected
            .take(&mut objections, &mut issues);
        FromRenderer::Delivered {
            issues,
            objections,
            navigation,
            fetches,
            sheets,
        }
    }

    /// The answer to one of the page's linked style sheets, as a task of its
    /// own (ADR 0035 § 4): what arrived is kept, and the page drawn again
    /// with it. No script runs, so the task asks for nothing; it says a sheet
    /// that did not arrive.
    fn styled(&mut self, answer: &SheetAnswer) -> FromRenderer {
        if self.held.is_none() {
            return FromRenderer::Failed(Failure::NothingLoaded);
        }
        let mut issues = Vec::new();
        {
            let mut easel = self.easel.borrow_mut();
            if easel.linked.arrived(answer, &mut issues) {
                easel.restyle = true;
            }
        }
        self.fresh();
        let mut objections = Vec::new();
        self.easel
            .borrow_mut()
            .objected
            .take(&mut objections, &mut issues);
        FromRenderer::Delivered {
            issues,
            objections,
            navigation: None,
            fetches: Vec::new(),
            sheets: Vec::new(),
        }
    }

    /// The linked style sheets the page has and has not asked for, taken as
    /// asked; what a link not asked for makes it say goes into `issues`.
    fn sheet_asks(&mut self, issues: &mut Vec<String>) -> Vec<SheetAsk> {
        let (Some(page), Some(document)) =
            (&self.page, self.held.as_ref().and_then(Held::document))
        else {
            return Vec::new();
        };
        self.easel
            .borrow_mut()
            .linked
            .asks(document, &page.url, &self.metas, issues)
    }

    /// A new page: parsed, each of its scripts run as a task when the parser
    /// reaches its end tag, and then drawn once.
    ///
    /// The loop the last page ran in goes with it, whatever this page turns
    /// out to carry.
    fn load(&mut self, page: Page) -> FromRenderer {
        self.held = None;
        // A new page has objected to nothing, and is owed nothing the last
        // one's draws found; it has asked for no sheet, and none has arrived.
        // What a script measures as it loads is drawn with its sheets at its
        // address, under its headers' policies and then its `<meta>`s'.
        self.easel.borrow_mut().begin(&page);
        let (mut parsing, document) = Parsing::start(&page.html);
        let mut held = Held::Parsed(document);
        let mut said = Vec::new();
        let mut objections = Vec::new();
        // A new page is shown at its own size, scrolled to its top.
        self.view = Rc::new(PageView::at(page.viewport, Rc::clone(&self.easel)));
        let policies = scripts::at_load(
            &mut held,
            &mut parsing,
            &page,
            &self.clock,
            &self.view,
            &mut said,
            &mut objections,
        );
        self.easel.borrow_mut().govern(Page::policies_of(&policies));
        // The header policies come first, and every policy after them is a
        // `<meta>`'s ([`scripts::at_load`]).
        self.metas = Page::policies_of(policies.get(page.policies.len()..).unwrap_or_default());
        self.held = Some(held);
        self.page = Some(page);
        // After the scripts, so what the load says — its issues, the fonts it
        // wants, what a policy objected to in its style — is about the page
        // they left (ADR 0017 § 6).
        self.draw();
        self.easel
            .borrow_mut()
            .objected
            .take(&mut objections, &mut said);
        let ongoing = self
            .held
            .as_mut()
            .map(Held::take_navigation)
            .unwrap_or_default();
        let (navigation, mut asked) = ask::answer(&ongoing);
        said.append(&mut asked);
        let fetches = self.held.as_mut().map(asks).unwrap_or_default();
        let sheets = self.sheet_asks(&mut said);
        match self.loaded() {
            FromRenderer::Loaded {
                mut issues, wanted, ..
            } => {
                issues.append(&mut said);
                FromRenderer::Loaded {
                    issues,
                    wanted,
                    objections,
                    navigation,
                    fetches,
                    sheets,
                }
            }
            other => other,
        }
    }

    /// The same page at another size: the document it has, drawn again,
    /// running none of its script — never its markup parsed again, which
    /// would lose everything its script and the agent changed. What its
    /// script reads of its viewport is the new size from here on.
    fn resize(&mut self, viewport: Size) -> FromRenderer {
        let Some(page) = &mut self.page else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        page.viewport = viewport;
        self.view.resized(viewport);
        self.draw();
        self.loaded()
    }

    /// What the page's markup and its drawing say, as a load answers it.
    fn loaded(&self) -> FromRenderer {
        let easel = self.easel.borrow();
        let (Some(document), Some(drawing)) = (self.document(), easel.drawn()) else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        // What the markup made the engine say, as much of it as one load
        // says (queue item 243).
        let issues = said::of_markup(document, drawing);
        // A renderer may not go and find a font (ADR 0010), so saying which
        // families it wanted and did not have is the whole of what it can do
        // about one — and the browser process, which may look, is exactly who
        // is listening.
        let wanted = drawing.wanted.families.clone();
        FromRenderer::Loaded {
            issues,
            wanted,
            // Nothing runs in a drawing, so nothing can have been objected
            // to, or asked for; a load adds what its scripts did and what its
            // document links.
            objections: Vec::new(),
            navigation: None,
            fetches: Vec::new(),
            sheets: Vec::new(),
        }
    }

    fn paint(&mut self) -> FromRenderer {
        self.fresh();
        let easel = self.easel.borrow();
        let Some(drawing) = easel.drawn() else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        if drawing.canvas.is_empty() {
            return FromRenderer::Failed(Failure::Unpaintable {
                why: "the window has no size".to_owned(),
            });
        }
        FromRenderer::Painted(Frame::from_canvas(&drawing.canvas))
    }

    fn read_tree(&mut self) -> FromRenderer {
        self.fresh();
        let easel = self.easel.borrow();
        let (Some(document), Some(drawing)) = (self.document(), easel.drawn()) else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        let tree = AgentTree::new(document, &drawing.boxes, &drawing.layout);
        FromRenderer::Tree(Box::new(Snapshot::of(&tree)))
    }

    /// How big the page it holds is, if it holds one.
    pub fn viewport(&self) -> Option<Size> {
        self.page.as_ref().map(|page| page.viewport)
    }
}

/// Every fetch the page has asked for since its asks were last taken, as
/// they cross.
fn asks(held: &mut Held) -> Vec<FetchAsk> {
    held.take_fetches()
        .into_iter()
        .map(FetchAsk::from)
        .collect()
}

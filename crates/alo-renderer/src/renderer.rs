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
//! Everything a renderer needs arrives in a message or in [`Renderer::new`].
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
//! # What a page asks to fetch
//!
//! Likewise a claim in the answer to the message whose work made it (ADR
//! 0032 § 1) — every ask, in order — and the answer to each comes back as a
//! message of its own, [`ToRenderer::Fetched`], whose handling is a task
//! that settles the page's promise ([`crate::deliver`]). Its answer carries
//! what the reactions asked for in turn.
//!
//! What is not here yet is the loop running between messages — a task a
//! page queues for itself has no idle moment to run in (queue item 233).

use crate::ask;
use crate::deliver::deliver;
use crate::event_loop::EventLoop;
use crate::face::Face;
use crate::fetch::{FetchAsk, Fetched};
use crate::frame::Frame;
use crate::generic::Generics;
use crate::held::Held;
use crate::inline_style::Judged;
use crate::message::{Failure, FromRenderer, ToRenderer};
use crate::objected::Objected;
use crate::page::Page;
use crate::pipeline::{Drawing, draw};
use crate::press::press;
use crate::put::put;
use crate::said;
use crate::scripts;
use crate::snapshot::Snapshot;
use alo_agent::{AgentTree, apply, perform};
use alo_agent::{Outcome, Target, Verb};
use alo_bindings::navigating::{self, By};
use alo_dom::{Document, Parsing};
use alo_layout::Size;
use alo_text::Font;
use alo_text::FontDatabase;

/// Everything that touches a page.
pub struct Renderer {
    fonts: FontDatabase,
    /// The page as it was sent: its sheets and its size, which every drawing
    /// of it is made with.
    page: Option<Page>,
    /// The page's document, and its script once it has run.
    held: Option<Held>,
    /// What the last drawing produced, and the document's change count when
    /// it was made.
    drawn: Option<(Drawing, u64)>,
    /// How many times a page has been drawn, for a test that asks how often.
    draws: u64,
    /// Every enforced policy the page holds — its headers' and each
    /// `<meta>`'s the parser made — kept for the page's life and asked of
    /// its inline style at every draw (ADR 0034 § 2).
    under: alo_net::Policies,
    /// The policies its headers stated, report-only ones included: those an
    /// objection is named against ([`Page::stated`]).
    stated: alo_net::Policies,
    /// What those policies have objected to in the page's inline style, for
    /// the page's life, and what of it waits to cross (ADR 0034 § 4).
    objected: Objected,
}

impl Renderer {
    /// A renderer that draws with these fonts and holds no page yet.
    pub fn new(fonts: FontDatabase) -> Self {
        Self {
            fonts,
            page: None,
            held: None,
            drawn: None,
            draws: 0,
            under: alo_net::Policies::none(),
            stated: alo_net::Policies::none(),
            objected: Objected::new(),
        }
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
                self.fonts.add(font);
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
        for (generic, family) in generics.pairs() {
            self.fonts.map_generic(generic, family);
        }
        let answering = generics
            .named()
            .into_iter()
            .filter(|generic| self.fonts.holds(generic))
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
        }
    }

    /// What the last drawing produced, for a test that asserts on the
    /// engine's insides.
    ///
    /// Not part of the boundary and never sent anywhere: a display list is not
    /// something a browser process asks for. The corpus reaches in because it
    /// is a test of the engine rather than of the browser, and ADR 0005 says
    /// tests stay single-process.
    pub fn rendered(&self) -> Option<&Drawing> {
        self.drawn.as_ref().map(|(drawing, _)| drawing)
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
    pub const fn draws(&self) -> u64 {
        self.draws
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
        let judged = Judged::of(document, &self.under, &self.stated, &mut self.objected);
        let (found, more) = judged.objections();
        self.objected.owe(found, more);
        let sheets = page.sheets.join("\n");
        let drawing = draw(
            document,
            &sheets,
            page.viewport,
            &self.fonts,
            &[],
            &[],
            &judged,
        );
        self.drawn = Some((drawing, document.change_count()));
        self.draws = self.draws.saturating_add(1);
    }

    /// Draw the page again if its document has changed since it was last
    /// drawn — the one question every reader of a rendering asks first.
    fn fresh(&mut self) {
        let now = self
            .held
            .as_ref()
            .and_then(Held::document)
            .map(Document::change_count);
        let then = self.drawn.as_ref().map(|(_, count)| *count);
        if now.is_some() && now != then {
            self.draw();
        }
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
        let (Some(page), Some(held), Some((drawing, _))) =
            (&self.page, &mut self.held, &self.drawn)
        else {
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
        self.fresh();
        let mut objections = Vec::new();
        self.objected.take(&mut objections, &mut issues);
        FromRenderer::Acted {
            outcome,
            issues,
            objections,
            navigation,
            fetches,
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
        self.fresh();
        let mut objections = Vec::new();
        self.objected.take(&mut objections, &mut issues);
        FromRenderer::Delivered {
            issues,
            objections,
            navigation,
            fetches,
        }
    }

    /// A new page: parsed, each of its scripts run as a task when the parser
    /// reaches its end tag, and then drawn once.
    ///
    /// The loop the last page ran in goes with it, whatever this page turns
    /// out to carry.
    fn load(&mut self, page: Page) -> FromRenderer {
        self.held = None;
        self.drawn = None;
        // A new page has objected to nothing, and is owed nothing the last
        // one's draws found.
        self.objected = Objected::new();
        let (mut parsing, document) = Parsing::start(&page.html);
        let mut held = Held::Parsed(document);
        let mut said = Vec::new();
        let mut objections = Vec::new();
        let policies = scripts::at_load(&mut held, &mut parsing, &page, &mut said, &mut objections);
        self.under = Page::policies_of(&policies);
        self.stated = page.stated();
        self.held = Some(held);
        self.page = Some(page);
        // After the scripts, so what the load says — its issues, the fonts it
        // wants, what a policy objected to in its style — is about the page
        // they left (ADR 0017 § 6).
        self.draw();
        self.objected.take(&mut objections, &mut said);
        let ongoing = self
            .held
            .as_mut()
            .map(Held::take_navigation)
            .unwrap_or_default();
        let (navigation, mut asked) = ask::answer(&ongoing);
        said.append(&mut asked);
        let fetches = self.held.as_mut().map(asks).unwrap_or_default();
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
                }
            }
            other => other,
        }
    }

    /// The same page at another size: the document it has, drawn again,
    /// running none of its script — never its markup parsed again, which
    /// would lose everything its script and the agent changed.
    fn resize(&mut self, viewport: Size) -> FromRenderer {
        let Some(page) = &mut self.page else {
            return FromRenderer::Failed(Failure::NothingLoaded);
        };
        page.viewport = viewport;
        self.draw();
        self.loaded()
    }

    /// What the page's markup and its drawing say, as a load answers it.
    fn loaded(&self) -> FromRenderer {
        let (Some(document), Some((drawing, _))) = (self.document(), &self.drawn) else {
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
            // to, or asked for; a load adds what its scripts did.
            objections: Vec::new(),
            navigation: None,
            fetches: Vec::new(),
        }
    }

    fn paint(&mut self) -> FromRenderer {
        self.fresh();
        let Some((drawing, _)) = &self.drawn else {
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
        let (Some(document), Some((drawing, _))) = (self.document(), &self.drawn) else {
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

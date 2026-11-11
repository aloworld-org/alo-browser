/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The frames of a page in Chrome, read into the tree an agent reads (ADR
//! 0109 § 1, queue item 937).
//!
//! `Accessibility.getFullAXTree` reads one document. A page's `<iframe>` is
//! a node of it — role `Iframe`, with no children — and the document the
//! frame holds is not in it (measured against Chrome 157 on Google Sheets'
//! `/htmlview`, whose grid is a frame of its own, and on a page of a test
//! server's with a frame from each kind of site). So each frame's document
//! is read here and put under its `<iframe>` node, as the one child it
//! holds, read by the same rules as the page's own ([`crate::chrome_tree`]).
//!
//! # Which frame, and through what
//!
//! Chrome names the frame an `<iframe>` holds when its element is described
//! (`DOM.describeNode`'s `frameId`). Then:
//!
//! - **A frame of the page's own process** — of its own site — is read
//!   through the session that holds its parent, with its `frameId`. Its
//!   nodes' ids are Chrome's `backendNodeId`s, which Chrome allocates once
//!   for the whole process, so they never meet the page's own (measured:
//!   the page's nodes and its frame's are numbered from one count). Where
//!   Chrome drew them is already where they are in the window. So a verb
//!   on one is carried out as on any other node of the page, by name.
//! - **A frame of another site** is a target of its own, held as the tab
//!   holds every frame ([`crate::chrome_hold`]), and is read through its own
//!   session. Its process numbers its nodes from its own count, which
//!   meets the page's, so its ids are made apart: the frame's number,
//!   given it the first time it is read and kept for the tab's life, times
//!   [`APART`], plus Chrome's id. Where Chrome drew a node is told within
//!   the frame's own window, so the content box of its `<iframe>` is added.
//!   **A verb on one is refused** ([`crate::chrome_act::Unacted::InAnotherSitesFrame`]),
//!   before anything is sent: a press would be aimed through another
//!   session, and checked against a hit test the page's own process cannot
//!   answer for (it finds the `<iframe>`). Acting there is queue item 943.
//!
//! # What is left out
//!
//! A frame Chrome declines to describe or read — gone between the page's
//! read and its own, or another site's frame not held yet — is read as
//! empty, its `<iframe>` node still there. Past [`MOST_FRAMES`] frames, or
//! frames nested deeper than [`MOST_NESTED`], the rest are left empty too,
//! because a page could otherwise make every read of it as long as it
//! liked.

use crate::chrome::Failed;
use crate::chrome_protocol::{self, AxNode, Command};
use crate::chrome_tab::Tab;
use crate::chrome_tree::{self, Unread};
use alo_box::{BoxId, Role};
use alo_layout::Rect;
use alo_renderer::{Snapshot, SnapshotNode};
use std::collections::HashMap;

/// The most frames read of one page; the rest are left empty.
pub const MOST_FRAMES: usize = 32;

/// The deepest frames are read inside frames; deeper ones are left empty.
pub const MOST_NESTED: usize = 4;

/// What a frame of another site's number is multiplied by to make its
/// nodes' ids apart from the page's: Chrome's ids are below it, measured on
/// every page read (a page of a million nodes stays far below it). And the
/// largest id it makes — a frame numbered below [`MOST_NUMBERED`] — stays
/// below 2⁵³, so an agent reading the socket's JSON in a language whose
/// numbers are doubles reads it exactly.
pub const APART: u64 = 1 << 32;

/// The most frames of other sites one tab numbers in its life.
pub const MOST_NUMBERED: u64 = 1 << 20;

/// Whether `node` was read from a frame of another site, whose nodes are
/// read and not yet acted on.
pub fn in_another_sites_frame(node: BoxId) -> bool {
    u64::try_from(node.as_usize()).map_or(true, |id| id >= APART)
}

/// `dom`, a node of the frame of another site numbered `number`, as the id
/// it is read with — or [`None`] when it cannot be made apart.
fn apart(number: u64, dom: u64) -> Option<u64> {
    (number > 0 && number < MOST_NUMBERED && dom < APART).then(|| number * APART + dom)
}

/// The numbers a tab gives the frames of other sites it reads, by their
/// target, each given once.
#[derive(Debug, Default)]
pub struct FrameNumbers {
    numbers: HashMap<String, u64>,
}

impl FrameNumbers {
    /// The number of the frame `target`, given now if it has none — or
    /// [`None`], once [`MOST_NUMBERED`] have been given.
    fn of(&mut self, target: &str) -> Option<u64> {
        if let Some(number) = self.numbers.get(target) {
            return Some(*number);
        }
        let next = u64::try_from(self.numbers.len()).ok()?.checked_add(1)?;
        if next >= MOST_NUMBERED {
            return None;
        }
        self.numbers.insert(target.to_owned(), next);
        Some(next)
    }
}

/// Where a document being read is: the session it is asked through
/// ([`None`] for the tab's own), the number its nodes are made apart by
/// (nought for the tab's own process), and where its window sits in the
/// tab's.
#[derive(Debug, Clone)]
struct Within {
    session: Option<String>,
    number: u64,
    offset: (f32, f32),
}

/// How far the read has gone: frames read, and how deep.
#[derive(Debug, Clone, Copy)]
struct Count {
    read: usize,
}

impl Tab<'_> {
    /// Every frame `snapshot` holds — the page's own document, read from
    /// `nodes` in a window `window` wide and high — read into it under its
    /// `<iframe>` node, and the nodes of each added to `nodes`, their ids
    /// in Chrome's tree made apart by the frame's.
    ///
    /// # Errors
    ///
    /// [`Unread`], when Chrome does not answer, or a frame's tree is not a
    /// tree.
    pub(crate) fn read_frames(
        &mut self,
        snapshot: &mut Snapshot,
        nodes: &mut Vec<AxNode>,
        window: (f64, f64),
    ) -> Result<(), Unread> {
        let Some(root) = snapshot.root.as_mut() else {
            return Ok(());
        };
        let within = Within {
            session: None,
            number: 0,
            offset: (0.0, 0.0),
        };
        let mut count = Count { read: 0 };
        self.fill(root, &within, 0, &mut count, nodes, window)
    }

    /// Every frame under `node`, a node of the document `within`, read into
    /// it.
    fn fill(
        &mut self,
        node: &mut SnapshotNode,
        within: &Within,
        nested: usize,
        count: &mut Count,
        nodes: &mut Vec<AxNode>,
        window: (f64, f64),
    ) -> Result<(), Unread> {
        if is_a_frame(node) && node.children.is_empty() {
            if count.read >= MOST_FRAMES || nested >= MOST_NESTED {
                return Ok(());
            }
            count.read += 1;
            if let Some((mut root, inner)) = self.frame_of(node.id, within, nodes, window)? {
                self.fill(&mut root, &inner, nested + 1, count, nodes, window)?;
                node.children.push(root);
            }
            return Ok(());
        }
        for child in &mut node.children {
            self.fill(child, within, nested, count, nodes, window)?;
        }
        Ok(())
    }

    /// The document the `<iframe>` `node` of the document `within` holds,
    /// read, and where it is — or [`None`] when Chrome names no frame, or
    /// declines to read it.
    fn frame_of(
        &mut self,
        node: BoxId,
        within: &Within,
        nodes: &mut Vec<AxNode>,
        window: (f64, f64),
    ) -> Result<Option<(SnapshotNode, Within)>, Unread> {
        let dom = local(node);
        let Some(described) = self.ask_within(within, &Command::Describe { node: dom })? else {
            return Ok(None);
        };
        let Some(frame) = chrome_protocol::frame_of(&described).map_err(Failed::Malformed)? else {
            return Ok(None);
        };
        let (inner, read) = if let Some(session) = self.frame_session(&frame) {
            // Another site's: its own target, its own count, its own window.
            let Some(boxed) = self.ask_within(within, &Command::BoxModel { node: dom })? else {
                return Ok(None);
            };
            let content = chrome_protocol::content_box(&boxed).map_err(Failed::Malformed)?;
            let Some(at) = chrome_tree::rect_of(&content) else {
                return Ok(None);
            };
            let Some(number) = self.frame_numbers().of(&frame) else {
                return Ok(None);
            };
            let inner = Within {
                session: Some(session),
                number,
                offset: (within.offset.0 + at.left(), within.offset.1 + at.top()),
            };
            let read = self.ask_within(&inner, &Command::ReadTree)?;
            (inner, read)
        } else {
            // The page's own process: asked through the session holding its
            // parent, its ids and places the page's.
            let read = self.ask_within(
                within,
                &Command::ReadFrameTree {
                    frame: frame.clone(),
                },
            )?;
            (within.clone(), read)
        };
        let Some(read) = read else {
            return Ok(None);
        };
        let frame_nodes = chrome_protocol::ax_nodes(&read).map_err(Failed::Malformed)?;
        let mut drawn = |dom: u64| self.drawn_within(&inner, dom);
        let snapshot = chrome_tree::snapshot(&frame_nodes, window, &mut drawn)?;
        let Some(mut root) = snapshot.root else {
            return Ok(None);
        };
        if inner.number > 0 && !made_apart(&mut root, inner.number) {
            return Ok(None);
        }
        nodes.extend(kept_apart(frame_nodes, &frame, inner.number));
        Ok(Some((root, inner)))
    }

    /// `command`, asked of the document `within`: [`None`] when Chrome
    /// declines it, or the frame's target has gone.
    fn ask_within(
        &mut self,
        within: &Within,
        command: &Command,
    ) -> Result<Option<Vec<u8>>, Failed> {
        let asked = match &within.session {
            None => self.ask(command),
            Some(session) => match self.ask_frame(session, command)? {
                Some(result) => result,
                None => return Ok(None),
            },
        };
        match asked {
            Ok(result) => Ok(Some(result)),
            Err(Failed::Declined(_)) => Ok(None),
            Err(why) => Err(why),
        }
    }

    /// Where Chrome drew the node `dom` of the document `within`, in the
    /// tab's window: nowhere, when it declines to say.
    fn drawn_within(&mut self, within: &Within, dom: u64) -> Result<Vec<Rect>, Failed> {
        if within.session.is_none() {
            return self.drawn(dom);
        }
        let Some(result) = self.ask_within(within, &Command::WhereDrawn { node: dom })? else {
            return Ok(Vec::new());
        };
        let (x, y) = within.offset;
        chrome_protocol::quads(&result)
            .map_err(Failed::Malformed)?
            .iter()
            .map(|quad| {
                chrome_tree::rect_of(quad)
                    .map(|rect| {
                        Rect::new(
                            rect.left() + x,
                            rect.top() + y,
                            rect.size.width,
                            rect.size.height,
                        )
                    })
                    .ok_or(Failed::Malformed(chrome_protocol::Malformed::BadField {
                        field: "quads",
                    }))
            })
            .collect()
    }
}

/// Whether `node` is a frame's element, as Chrome's tree names one.
fn is_a_frame(node: &SnapshotNode) -> bool {
    matches!(&node.role, Role::Declared(role) if &**role == "Iframe")
}

/// Chrome's own id of `node`, within the process it was read from.
fn local(node: BoxId) -> u64 {
    u64::try_from(node.as_usize()).map_or(u64::MAX, |id| id % APART)
}

/// Every id under and of `node`, read from the frame of another site
/// numbered `number`, made apart; `false`, and the frame left unread, for
/// an id that cannot be.
fn made_apart(node: &mut SnapshotNode, number: u64) -> bool {
    let Some(id) = u64::try_from(node.id.as_usize())
        .ok()
        .and_then(|dom| apart(number, dom))
        .and_then(|id| usize::try_from(id).ok())
    else {
        return false;
    };
    node.id = BoxId::from_wire(id);
    node.children
        .iter_mut()
        .all(|child| made_apart(child, number))
}

/// The nodes of Chrome's tree of the frame `frame`, numbered `number`, kept
/// beside the page's: their ids in that tree named by the frame, which
/// Chrome's trees of two documents may share, and their DOM nodes made
/// apart when the frame is another site's.
fn kept_apart(nodes: Vec<AxNode>, frame: &str, number: u64) -> Vec<AxNode> {
    nodes
        .into_iter()
        .map(|mut node| {
            node.id = format!("{frame}:{}", node.id);
            node.children = node
                .children
                .iter()
                .map(|child| format!("{frame}:{child}"))
                .collect();
            if number > 0 {
                node.node = node.node.and_then(|dom| apart(number, dom));
            }
            node
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::{KnownRole, States};

    fn node(id: usize, role: Role, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role,
            name: None,
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::ZERO,
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    #[test]
    fn another_sites_frame_is_numbered_once_and_its_ids_kept_apart_from_the_pages() {
        let mut numbers = FrameNumbers::default();
        assert_eq!(numbers.of("A"), Some(1));
        assert_eq!(numbers.of("B"), Some(2));
        assert_eq!(
            numbers.of("A"),
            Some(1),
            "a frame read again keeps its number"
        );
        let mut root = node(
            1,
            Role::Known(KnownRole::Document),
            vec![node(4, Role::Known(KnownRole::Button), Vec::new())],
        );
        assert!(made_apart(&mut root, 2));
        let ids: Vec<usize> = [&root, &root.children[0]]
            .iter()
            .map(|node| node.id.as_usize())
            .collect();
        assert_eq!(ids, vec![2 * (1 << 32) + 1, 2 * (1 << 32) + 4]);
        assert!(
            root.children
                .iter()
                .all(|child| in_another_sites_frame(child.id))
        );
        assert!(!in_another_sites_frame(BoxId::from_wire(4)));
        assert_eq!(local(root.children[0].id), 4);
    }

    #[test]
    fn an_id_that_cannot_be_kept_apart_leaves_the_frame_unread() {
        let mut big = node(usize::try_from(APART).unwrap(), Role::Generic, Vec::new());
        assert!(!made_apart(&mut big, 1));
        assert_eq!(apart(0, 4), None, "nought is the page's own process");
        assert_eq!(apart(MOST_NUMBERED, 4), None);
        assert!(apart(MOST_NUMBERED - 1, APART - 1).is_some_and(|id| id < 1 << 53));
    }

    #[test]
    fn a_frames_nodes_are_named_by_it_beside_the_pages() {
        let ax = AxNode {
            id: "7".to_owned(),
            ignored: false,
            role: "button".to_owned(),
            name: None,
            value: None,
            properties: Vec::new(),
            children: vec!["8".to_owned()],
            node: Some(5),
        };
        let kept = kept_apart(vec![ax.clone()], "F", 0);
        assert_eq!(kept[0].id, "F:7");
        assert_eq!(kept[0].children, vec!["F:8".to_owned()]);
        assert_eq!(
            kept[0].node,
            Some(5),
            "the page's own process keeps its ids"
        );
        let apart = kept_apart(vec![ax], "G", 3);
        assert_eq!(apart[0].node, Some(3 * APART + 5));
    }

    #[test]
    fn only_an_iframe_is_a_frame() {
        assert!(is_a_frame(&node(
            1,
            Role::Declared("Iframe".into()),
            Vec::new()
        )));
        assert!(!is_a_frame(&node(1, Role::Generic, Vec::new())));
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What alo's own interface becomes on the platform's accessibility bus
//! (ADR 0115 § 2, queue item 987): a [`Published`] window in, AccessKit's
//! [`TreeUpdate`]s out — and an action the bus asked, in, as this
//! browser's [`Asked`] or refused. Pure functions, tested on numbers.
//!
//! One of the two files that name AccessKit; [`crate::accessible`], the
//! adapter, is the other.
//!
//! # There is no second tree
//!
//! ADR 0002: the window's tree on the bus is the agent's tree. Each part —
//! the chats panel (or the rail it folds to), and the first screen with
//! its card's frame — answers `ReadTree` with a [`alo_renderer::Snapshot`], and each is
//! published as an AccessKit **subtree** of its own ([`tree_id`]) under a
//! graft node where the window composed it, below a root `Window` node. Each
//! renderer numbers its own boxes (ADR 0003), so each subtree is its own id
//! space and no part can name another's node.
//!
//! | `SnapshotNode` | AccessKit |
//! |---|---|
//! | `id` | `NodeId`, in its part's `TreeId` |
//! | a known role | the same role by name; `TextBox` → `TextInput`, `SearchBox` → `SearchInput`, `Radio` → `RadioButton`, `Option` → `ListBoxOption`, `Date` → `DateInput`, `ProgressBar` → `ProgressIndicator`, `Separator` → `Splitter`, `Summary` → `DisclosureTriangle`, `Text` → `Label` (its words its value) |
//! | `Generic`, `Presentational` | `GenericContainer` — or `PasswordInput`, for a field (ARIA gives a password none) |
//! | `Declared` | only from [`DECLARED`]; otherwise `GenericContainer`, never the nearest guess |
//! | `name`, `placeholder` | `label`, `placeholder` |
//! | `states` | `disabled`, `toggled` (checked, or pressed), `selected`, `expanded`, `required`, `read_only`, `busy`, `invalid`, `level`, `aria_current`, `hidden` |
//! | the runs of text inside a field | `value`, unless ADR 0115 § 4 withholds it; the field is a leaf |
//! | `rect` | `bounds`: the part's place in the window plus the rect times the scale, in device pixels |
//! | `children` | `children`, in the order a person meets them |
//!
//! A node is offered only what [`crate::assisted::offers`] says — `Click`,
//! `SetValue`, `Focus` — and every other action is refused by [`asked`]:
//! the two that carry a point always (ADR 0002), the rest until each is a
//! verb of ours with tests of its own.
//!
//! # Never a tree the adapter would refuse
//!
//! AccessKit's consumer panics on a tree that is not one — a child named
//! twice, a focus on a graft whose subtree it does not hold — and it runs
//! on the window's own thread. So a node whose id was already met is left
//! out with everything under it, and the root's focus is put on a part's
//! graft only once that part's subtree has been sent ([`updates`]); the
//! tests apply every update made here to the same consumer the macOS
//! adapter reads.
//!
//! # Bounded
//!
//! A part of more than [`MOST_NODES`] nodes is published as its root alone,
//! said to be too large to read — never half a tree said to be whole.
//!
//! # A card's page (ADR 0115 § 3, phase (b), queue item 988)
//!
//! The page a card shows is a subtree of its own ([`PAGE`]), grafted at the
//! card's picture of it — the first screen's image named
//! [`alo_renderer::card_document::PICTURED`], published as a group of that
//! name standing at the card's page box — and only in a sending whose first
//! screen holds that picture, so the adapter is never sent a subtree with
//! no graft. Its nodes are Chrome's, by `backendDOMNodeId` (ADR 0003), read
//! as the agent socket reads them, mapped by the same table, offered the
//! same actions and withheld the same secrets; their rectangles, CSS pixels
//! of the page as it sits, are placed from the page box's corner at the
//! window's scale. A page past [`MOST_NODES`], or one the socket refuses as
//! too deep, is its graft's subtree of one node, [`PAGE_TOO_LARGE`]; a page
//! on a site kept from agents is one node, [`assisted::KEPT`] (queue item
//! 989).
//!
//! # Off
//!
//! *Settings › Screen readers › Off* (queue item 989) is a [`Published`]
//! holding no part and no page: the root tree, the window alone, with no
//! graft — what ADR 0115's guard names *the window's root node alone*.

use crate::assisted::{
    self, Asked, Assist, FromTheBus, PageRead, Published, PublishedPage, PublishedPart, Secret,
    Unasked,
};
use crate::conversation_drawing::Part;
use accesskit::{
    Action, ActionData, ActionRequest, AriaCurrent, Invalid, Node, NodeId, Rect, Role, Toggled,
    TreeId, TreeInfo, TreeUpdate, Uuid,
};
use alo_box::{BoxId, Checked, Current, KnownRole, Role as Ours};
use alo_renderer::SnapshotNode;
use alo_renderer::card_document::PICTURED;
use std::collections::HashSet;

/// The most nodes of one part published — and of a card's page, the most
/// the socket reads of one (`chrome_protocol::MOST_NODES`).
pub const MOST_NODES: usize = 100_000;

/// What a part too large to publish is said to be.
pub const TOO_LARGE: &str = "this part of alo is too large to read";

/// What a card's page too large to publish is said to be.
pub const PAGE_TOO_LARGE: &str = "this page is too large to read";

/// The subtree a card's page is published as.
pub const PAGE: TreeId = TreeId(Uuid::from_u128(0xa10_0115_0003));

/// The roles declared that are published as a role of their own, by the
/// name Chrome's tree gives them; every other declared role is a generic
/// container.
pub const DECLARED: [(&str, Role); 1] = [("InputTime", Role::TimeInput)];

/// The window's own node in the root tree.
const WINDOW: NodeId = NodeId(0);

/// Which parts' subtrees the adapter has been sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Grafted {
    /// The chats panel's.
    pub panel: bool,
    /// The first screen's.
    pub start: bool,
}

impl Grafted {
    fn holds(self, part: Part) -> bool {
        match part {
            Part::Panel => self.panel,
            Part::Start => self.start,
        }
    }

    fn with(self, part: Part) -> Self {
        match part {
            Part::Panel => Self {
                panel: true,
                ..self
            },
            Part::Start => Self {
                start: true,
                ..self
            },
        }
    }
}

/// The subtree `part` is published as.
pub fn tree_id(part: Part) -> TreeId {
    TreeId(Uuid::from_u128(match part {
        Part::Panel => 0xa10_0115_0001,
        Part::Start => 0xa10_0115_0002,
    }))
}

/// The part whose subtree `tree` is, if it is one.
pub fn part_of(tree: TreeId) -> Option<Part> {
    [Part::Panel, Part::Start]
        .into_iter()
        .find(|part| tree_id(*part) == tree)
}

/// The node in the root tree each part is grafted at.
fn graft(part: Part) -> NodeId {
    match part {
        Part::Panel => NodeId(1),
        Part::Start => NodeId(2),
    }
}

/// The parts of `published`, in the order a person meets them, each with
/// what it publishes.
fn parts(published: &Published) -> Vec<(Part, &PublishedPart)> {
    [
        (Part::Panel, published.panel.as_ref()),
        (Part::Start, published.start.as_ref()),
    ]
    .into_iter()
    .filter_map(|(part, at)| at.filter(|at| at.tree.root.is_some()).map(|at| (part, at)))
    .collect()
}

/// What the adapter is to be sent, in order, for `published`, the adapter
/// having been sent the subtrees `grafted` says — and which it has been
/// sent once these are.
///
/// The root tree first, its focus on the window — or on the part whose
/// field has the keyboard when that part was grafted before — then each
/// part's subtree, then the root again when the focus could only be put on
/// a part grafted by this very sending.
pub fn updates(published: &Published, grafted: Grafted) -> (Vec<TreeUpdate>, Grafted) {
    let before = if published.fresh {
        Grafted::default()
    } else {
        grafted
    };
    let parts = parts(published);
    let wanted = published
        .keyboard
        .filter(|keyed| parts.iter().any(|(part, _)| part == keyed));
    let first = wanted.filter(|part| before.holds(*part));
    let mut sent = vec![root(&parts, first)];
    let mut now = Grafted::default();
    let mut page_grafted = false;
    for (part, at) in &parts {
        let page = published.page.as_ref().filter(|_| *part == Part::Start);
        let (update, grafted) = subtree(
            *part,
            at,
            published.scale,
            published.keyboard == Some(*part),
            page,
        );
        sent.push(update);
        page_grafted |= grafted;
        now = now.with(*part);
    }
    // The page only where the first screen just sent holds its graft.
    if page_grafted && let Some(page) = &published.page {
        sent.push(page_subtree(page, published.scale));
    }
    if wanted != first {
        sent.push(root(&parts, wanted));
    }
    (sent, now)
}

/// The root tree: the window, and a graft for each part where it stands.
fn root(parts: &[(Part, &PublishedPart)], focus: Option<Part>) -> TreeUpdate {
    let mut window = Node::new(Role::Window);
    window.set_label("alo");
    window.set_children(
        parts
            .iter()
            .map(|(part, _)| graft(*part))
            .collect::<Vec<_>>(),
    );
    let mut nodes = vec![(WINDOW, window)];
    for (part, at) in parts {
        let mut grafted = Node::new(Role::GenericContainer);
        grafted.set_tree_id(tree_id(*part));
        grafted.set_bounds(Rect {
            x0: f64::from(at.at.x),
            y0: f64::from(at.at.y),
            x1: f64::from(at.at.x) + f64::from(at.at.width),
            y1: f64::from(at.at.y) + f64::from(at.at.height),
        });
        nodes.push((graft(*part), grafted));
    }
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo {
            root: WINDOW,
            toolkit_name: Some("alo".to_owned()),
            toolkit_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        tree_id: TreeId::ROOT,
        focus: focus.map_or(WINDOW, graft),
    }
}

/// `id` as AccessKit's.
fn node_id(id: BoxId) -> NodeId {
    NodeId(u64::try_from(id.as_usize()).unwrap_or(u64::MAX))
}

/// How many nodes the tree under `root` holds, counted to one past `most`.
fn counted(root: &SnapshotNode, most: usize) -> usize {
    let mut count = 0;
    let mut waiting = vec![root];
    while let Some(node) = waiting.pop() {
        count += 1;
        if count > most {
            break;
        }
        waiting.extend(node.children.iter());
    }
    count
}

/// `part`'s subtree, its nodes placed by `at` at `scale`, its focus on its
/// field when `keyed`, and on its root otherwise — the first screen's
/// picture of the card's page, when `page` is to be published, its graft:
/// and whether it was.
fn subtree(
    part: Part,
    at: &PublishedPart,
    scale: f32,
    keyed: bool,
    page: Option<&PublishedPage>,
) -> (TreeUpdate, bool) {
    let origin = (f64::from(at.at.x), f64::from(at.at.y));
    let mut placing = Placing {
        origin,
        scale,
        keyed,
        page,
        grafted: false,
    };
    let root_id = at
        .tree
        .root
        .as_ref()
        .map_or(NodeId(0), |root| node_id(root.id));
    let (nodes, focus) = match &at.tree.root {
        Some(root) => placing.nodes(root, TOO_LARGE),
        None => (Vec::new(), None),
    };
    (
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(root_id)),
            tree_id: tree_id(part),
            focus: focus.unwrap_or(root_id),
        },
        placing.grafted,
    )
}

/// The card's page's subtree, its nodes placed from its page box's corner
/// at `scale`, its focus on its root.
fn page_subtree(page: &PublishedPage, scale: f32) -> TreeUpdate {
    let mut placing = Placing {
        origin: (page.at.0, page.at.1),
        scale,
        keyed: false,
        page: None,
        grafted: false,
    };
    let (root_id, nodes) = match &page.read {
        PageRead::Tree(tree) => match &tree.root {
            Some(root) => (node_id(root.id), placing.nodes(root, PAGE_TOO_LARGE).0),
            None => (NodeId(0), vec![(NodeId(0), Node::new(Role::Document))]),
        },
        // One node, saying why, standing where the page box is (queue item
        // 989: without bounds, macOS placed it at nothing outside the
        // window).
        PageRead::TooLarge => (NodeId(0), vec![(NodeId(0), alone(page, PAGE_TOO_LARGE))]),
        // A site kept from agents (queue item 989).
        PageRead::Kept => (NodeId(0), vec![(NodeId(0), alone(page, assisted::KEPT))]),
    };
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(root_id)),
        tree_id: PAGE,
        focus: root_id,
    }
}

/// The one node a card's page is published as when it is not read, saying
/// `said`, at its page box.
fn alone(page: &PublishedPage, said: &str) -> Node {
    let mut alone = Node::new(Role::Document);
    alone.set_label(said);
    let (left, top, width, height) = page.at;
    alone.set_bounds(Rect {
        x0: left,
        y0: top,
        x1: left + width,
        y1: top + height,
    });
    alone
}

/// How one subtree's nodes are published: placed from `origin` at `scale`,
/// the keyboard on its first typed field when `keyed`, and the card's page
/// grafted at its picture when there is a `page` to publish.
struct Placing<'p> {
    origin: (f64, f64),
    scale: f32,
    keyed: bool,
    page: Option<&'p PublishedPage>,
    /// Whether the page's graft was published.
    grafted: bool,
}

impl Placing<'_> {
    /// Every node under `root`, outermost first, and the one the keyboard
    /// is on — or `root` alone, said to be `too_large`, past
    /// [`MOST_NODES`].
    fn nodes(
        &mut self,
        root: &SnapshotNode,
        too_large: &str,
    ) -> (Vec<(NodeId, Node)>, Option<NodeId>) {
        let mut nodes = Vec::new();
        let mut focus = None;
        if counted(root, MOST_NODES) > MOST_NODES {
            let mut alone = Node::new(Role::GenericContainer);
            alone.set_label(too_large);
            nodes.push((node_id(root.id), alone));
            return (nodes, focus);
        }
        let mut seen = HashSet::from([root.id]);
        let mut waiting = vec![root];
        while let Some(each) = waiting.pop() {
            if let Some(graft) = self.graft(each) {
                nodes.push((node_id(each.id), graft));
                continue;
            }
            let mut published = published_node(each, self.origin, self.scale);
            if self.keyed && focus.is_none() && is_a_typed_field(each) {
                focus = Some(node_id(each.id));
            }
            // A field is a leaf: what it holds is its value.
            let children: Vec<&SnapshotNode> = if each.states.takes_text {
                Vec::new()
            } else {
                each.children
                    .iter()
                    .filter(|child| seen.insert(child.id))
                    .collect()
            };
            published.set_children(
                children
                    .iter()
                    .map(|child| node_id(child.id))
                    .collect::<Vec<_>>(),
            );
            nodes.push((node_id(each.id), published));
            waiting.extend(children.into_iter().rev());
        }
        (nodes, focus)
    }

    /// `node` as the card's page's graft, if it is the card's picture of
    /// the page and the page is to be published: a group of the picture's
    /// name at the page box, holding nothing of its own — AccessKit's graft
    /// holds only its subtree — and only the first such picture met.
    fn graft(&mut self, node: &SnapshotNode) -> Option<Node> {
        let page = self.page?;
        if self.grafted
            || node.role != Ours::Known(KnownRole::Image)
            || node.name.as_deref() != Some(PICTURED)
        {
            return None;
        }
        self.grafted = true;
        let mut graft = Node::new(Role::Group);
        graft.set_label(PICTURED);
        graft.set_tree_id(PAGE);
        let (left, top, width, height) = page.at;
        graft.set_bounds(Rect {
            x0: left,
            y0: top,
            x1: left + width,
            y1: top + height,
        });
        Some(graft)
    }
}

/// Whether `node` is a field a person types words into: what the keyboard
/// of a part is on, when its field has it.
fn is_a_typed_field(node: &SnapshotNode) -> bool {
    node.states.takes_text
        && matches!(
            node.role,
            Ours::Known(KnownRole::TextBox | KnownRole::SearchBox)
        )
}

/// `node` as AccessKit's, without its children, placed from `origin` at
/// `scale`.
fn published_node(node: &SnapshotNode, origin: (f64, f64), scale: f32) -> Node {
    let secret = assisted::secret(node);
    let role = role(node, secret);
    let mut published = Node::new(role);
    if let Some(name) = node.name.as_deref() {
        if role == Role::Label {
            published.set_value(name);
        } else {
            published.set_label(name);
        }
    }
    if let Some(placeholder) = node.placeholder.as_deref() {
        published.set_placeholder(placeholder);
    }
    if let Some(value) = assisted::value(node) {
        published.set_value(value);
    }
    if !node.rects.is_empty() {
        published.set_bounds(bounds(node, origin, scale));
    }
    states(node, &mut published);
    let offers = assisted::offers(node);
    for (offered, action) in [
        (offers.click, Action::Click),
        (offers.set_value, Action::SetValue),
        (offers.focus, Action::Focus),
    ] {
        if offered {
            published.add_action(action);
        }
    }
    published
}

/// Where `node` is in the window, in device pixels: its part's place — or
/// its page box's corner — plus its rect at `scale`.
fn bounds(node: &SnapshotNode, (left, top): (f64, f64), scale: f32) -> Rect {
    let scale = f64::from(scale);
    let x0 = left + f64::from(node.rect.left()) * scale;
    let y0 = top + f64::from(node.rect.top()) * scale;
    Rect {
        x0,
        y0,
        x1: x0 + f64::from(node.rect.size.width) * scale,
        y1: y0 + f64::from(node.rect.size.height) * scale,
    }
}

/// What is true of `node`, set on `published`.
fn states(node: &SnapshotNode, published: &mut Node) {
    let states = node.states;
    if states.disabled {
        published.set_disabled();
    }
    let toggled = states
        .checked
        .map(|checked| match checked {
            Checked::No => Toggled::False,
            Checked::Yes => Toggled::True,
            Checked::Mixed => Toggled::Mixed,
        })
        .or_else(|| {
            states.pressed.map(|pressed| {
                if pressed {
                    Toggled::True
                } else {
                    Toggled::False
                }
            })
        });
    if let Some(toggled) = toggled {
        published.set_toggled(toggled);
    }
    if let Some(selected) = states.selected {
        published.set_selected(selected);
    }
    if let Some(expanded) = states.expanded {
        published.set_expanded(expanded);
    }
    if states.required {
        published.set_required();
    }
    if states.read_only {
        published.set_read_only();
    }
    if states.busy {
        published.set_busy();
    }
    if states.invalid == Some(alo_box::invalid::Invalid::Yes) {
        published.set_invalid(Invalid::True);
    }
    if states.hidden {
        published.set_hidden();
    }
    if let Some(level) = states.level {
        published.set_level(usize::from(level));
    }
    if let Some(current) = states.current {
        published.set_aria_current(match current {
            Current::Yes => AriaCurrent::True,
            Current::Page => AriaCurrent::Page,
            Current::Step => AriaCurrent::Step,
            Current::Location => AriaCurrent::Location,
            Current::Date => AriaCurrent::Date,
            Current::Time => AriaCurrent::Time,
        });
    }
}

/// The role `node` is published as, its value withheld as `secret` says.
fn role(node: &SnapshotNode, secret: Option<Secret>) -> Role {
    if secret == Some(Secret::Password) {
        return Role::PasswordInput;
    }
    match &node.role {
        Ours::Generic | Ours::Presentational => Role::GenericContainer,
        Ours::Declared(name) => DECLARED
            .iter()
            .find(|(declared, _)| *declared == &**name)
            .map_or(Role::GenericContainer, |(_, role)| *role),
        Ours::Known(known) => known_role(*known),
    }
}

/// The AccessKit role of `known`.
fn known_role(known: KnownRole) -> Role {
    match known {
        KnownRole::Document => Role::Document,
        KnownRole::Banner => Role::Banner,
        KnownRole::Navigation => Role::Navigation,
        KnownRole::Main => Role::Main,
        KnownRole::Complementary => Role::Complementary,
        KnownRole::ContentInfo => Role::ContentInfo,
        KnownRole::Region => Role::Region,
        KnownRole::Form => Role::Form,
        KnownRole::Search => Role::Search,
        KnownRole::Article => Role::Article,
        KnownRole::Heading => Role::Heading,
        KnownRole::Paragraph => Role::Paragraph,
        KnownRole::List => Role::List,
        KnownRole::ListItem => Role::ListItem,
        KnownRole::Group => Role::Group,
        KnownRole::Separator => Role::Splitter,
        KnownRole::Blockquote => Role::Blockquote,
        KnownRole::Figure => Role::Figure,
        KnownRole::Image => Role::Image,
        KnownRole::Button => Role::Button,
        KnownRole::Link => Role::Link,
        KnownRole::TextBox => Role::TextInput,
        KnownRole::SearchBox => Role::SearchInput,
        KnownRole::CheckBox => Role::CheckBox,
        KnownRole::Radio => Role::RadioButton,
        KnownRole::RadioGroup => Role::RadioGroup,
        KnownRole::Switch => Role::Switch,
        KnownRole::ComboBox => Role::ComboBox,
        KnownRole::ListBox => Role::ListBox,
        KnownRole::Option => Role::ListBoxOption,
        KnownRole::Slider => Role::Slider,
        KnownRole::SpinButton => Role::SpinButton,
        KnownRole::Date => Role::DateInput,
        KnownRole::ProgressBar => Role::ProgressIndicator,
        KnownRole::Meter => Role::Meter,
        KnownRole::Dialog => Role::Dialog,
        KnownRole::Status => Role::Status,
        KnownRole::Grid => Role::Grid,
        KnownRole::Row => Role::Row,
        KnownRole::Cell => Role::Cell,
        KnownRole::RowGroup => Role::RowGroup,
        KnownRole::ColumnHeader => Role::ColumnHeader,
        KnownRole::RowHeader => Role::RowHeader,
        KnownRole::Table => Role::Table,
        KnownRole::Tab => Role::Tab,
        KnownRole::TabList => Role::TabList,
        KnownRole::TabPanel => Role::TabPanel,
        KnownRole::Menu => Role::Menu,
        KnownRole::MenuItem => Role::MenuItem,
        KnownRole::Summary => Role::DisclosureTriangle,
        KnownRole::Details => Role::Details,
        KnownRole::Text => Role::Label,
    }
}

/// What `request`, an action asked on the bus, is, in this browser's words
/// — or why it is refused. Only `Click`, `SetValue` with text and `Focus`
/// are taken, and only on a node of a part's subtree; anything carrying a
/// point is refused whatever it names (ADR 0002).
///
/// # Errors
///
/// [`Unasked`]: what was asked and why it is refused.
pub fn asked(request: &ActionRequest) -> Result<Asked, Unasked> {
    let refused = |why| Unasked {
        action: format!("{:?}", request.action),
        why,
    };
    let assist = assist_of(request)?;
    let part = part_of(request.target_tree).ok_or_else(|| refused("it names no node of a part"))?;
    let node = node_of(request).ok_or_else(|| refused("it names a node no tree of alo's holds"))?;
    Ok(Asked { part, node, assist })
}

/// What `request` tells the window: an action on a node of a part
/// ([`asked`]), on a node of the card's page, or refused — by the same
/// rule, so a point is refused on the page as on alo's own interface.
pub fn heard(request: &ActionRequest) -> FromTheBus {
    if request.target_tree != PAGE {
        return match asked(request) {
            Ok(asked) => FromTheBus::Asked(asked),
            Err(refused) => FromTheBus::Refused(refused),
        };
    }
    let page = assist_of(request).and_then(|assist| {
        node_of(request)
            .map(|node| FromTheBus::Page { node, assist })
            .ok_or_else(|| Unasked {
                action: format!("{:?}", request.action),
                why: "it names a node no tree of alo's holds",
            })
    });
    page.unwrap_or_else(FromTheBus::Refused)
}

/// The node `request` names, by its id in the tree it names.
fn node_of(request: &ActionRequest) -> Option<BoxId> {
    usize::try_from(request.target_node.0)
        .ok()
        .map(BoxId::from_wire)
}

/// What `request` asks be done, in this browser's words, or why it is
/// refused: anything carrying a point first (ADR 0002).
fn assist_of(request: &ActionRequest) -> Result<Assist, Unasked> {
    let refused = |why| Unasked {
        action: format!("{:?}", request.action),
        why,
    };
    if matches!(
        request.data,
        Some(ActionData::ScrollToPoint(_) | ActionData::SetScrollOffset(_))
    ) || matches!(
        request.action,
        Action::ScrollToPoint | Action::SetScrollOffset
    ) {
        return Err(refused(
            "it carries a point, and no verb of alo's takes a coordinate (ADR 0002)",
        ));
    }
    let assist = match (request.action, request.data.clone()) {
        (Action::Click, _) => Assist::Click,
        (Action::Focus, _) => Assist::Focus,
        (Action::SetValue, Some(ActionData::Value(text))) => Assist::SetValue(text.into()),
        (Action::SetValue, _) => return Err(refused("it puts no text in")),
        _ => {
            return Err(refused(
                "alo takes only a press, text put in and the keyboard moved, until each \
                 other is a verb of its own",
            ));
        }
    };
    Ok(assist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::PixelRect;
    use accesskit::Point;
    use accesskit_consumer::{NodeRef, Tree, TreeChangeHandler};
    use alo_box::States;
    use alo_layout::Rect as Laid;
    use alo_renderer::Snapshot;

    /// A handler that hears nothing: the tests read the tree itself.
    struct Unheard;

    impl TreeChangeHandler for Unheard {
        fn node_added(&mut self, _: &NodeRef) {}
        fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
        fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {}
        fn node_removed(&mut self, _: &NodeRef) {}
    }

    fn node(id: usize, role: Ours, name: Option<&str>, rect: Laid) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role,
            name: name.map(str::to_owned),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect,
            rects: vec![rect],
            offscreen: false,
            scrolls: false,
            children: Vec::new(),
        }
    }

    fn known(id: usize, role: KnownRole, name: &str, rect: Laid) -> SnapshotNode {
        node(id, Ours::Known(role), Some(name), rect)
    }

    fn with(mut parent: SnapshotNode, children: Vec<SnapshotNode>) -> SnapshotNode {
        parent.children = children;
        parent
    }

    fn field(id: usize, role: Ours, name: &str, holds: &str, rect: Laid) -> SnapshotNode {
        let mut field = with(
            node(id, role, Some(name), rect),
            vec![node(
                id + 1,
                Ours::Known(KnownRole::Text),
                Some(holds),
                rect,
            )],
        );
        field.states.takes_text = true;
        field
    }

    /// The panel as `tests/the_chats_panel.rs`'s five draw it, cut to one
    /// chat each kind of node: *New chat*, *Search chats* holding words,
    /// the current chat, a heading.
    fn panel() -> SnapshotNode {
        let mut current = known(
            20,
            KnownRole::Link,
            "Charging my EV",
            Laid::new(10.0, 178.0, 220.0, 38.0),
        );
        current.states.current = Some(Current::Page);
        let mut heading = known(
            18,
            KnownRole::Heading,
            "Open",
            Laid::new(10.0, 158.0, 220.0, 14.0),
        );
        heading.states.level = Some(2);
        let mut search = field(
            14,
            Ours::Known(KnownRole::TextBox),
            "Search chats",
            "inv",
            Laid::new(46.0, 111.0, 174.0, 20.0),
        );
        search.placeholder = Some("Search chats".to_owned());
        with(
            known(
                1,
                KnownRole::Navigation,
                "Chats",
                Laid::new(0.0, 0.0, 240.0, 820.0),
            ),
            vec![
                known(
                    12,
                    KnownRole::Link,
                    "New chat",
                    Laid::new(10.0, 56.0, 220.0, 40.0),
                ),
                search,
                heading,
                current,
            ],
        )
    }

    /// The first screen cut to its box, a line of text, a password field
    /// and a card's number.
    fn start() -> SnapshotNode {
        with(
            node(
                1,
                Ours::Known(KnownRole::Main),
                None,
                Laid::new(0.0, 0.0, 1040.0, 820.0),
            ),
            vec![
                known(
                    30,
                    KnownRole::Text,
                    "Ask anything, search, or type a website.",
                    Laid::new(384.0625, 205.0, 271.859_38, 21.0),
                ),
                field(
                    40,
                    Ours::Known(KnownRole::TextBox),
                    "Ask anything, search, or type a website",
                    "lisbon",
                    Laid::new(183.0, 266.0, 632.0, 40.0),
                ),
                field(
                    50,
                    Ours::Generic,
                    "Password",
                    "hunter2",
                    Laid::new(183.0, 330.0, 632.0, 40.0),
                ),
                field(
                    60,
                    Ours::Known(KnownRole::TextBox),
                    "Card number",
                    "4242 4242 4242 4242",
                    Laid::new(183.0, 390.0, 632.0, 40.0),
                ),
            ],
        )
    }

    /// The design's window at two: the panel 480 wide from the left, the
    /// first screen in the 2080 beside it.
    fn published(keyboard: Option<Part>, fresh: bool) -> Published {
        Published {
            scale: 2.0,
            panel: Some(PublishedPart {
                at: PixelRect {
                    x: 0,
                    y: 0,
                    width: 480,
                    height: 1640,
                },
                tree: Snapshot {
                    root: Some(panel()),
                },
            }),
            start: Some(PublishedPart {
                at: PixelRect {
                    x: 480,
                    y: 0,
                    width: 2080,
                    height: 1640,
                },
                tree: Snapshot {
                    root: Some(start()),
                },
            }),
            page: None,
            keyboard,
            fresh,
        }
    }

    /// The tree the macOS adapter would hold after `updates`, applied in
    /// order as it applies them — panicking, as it would, on one that is
    /// not a tree.
    fn applied(updates: Vec<TreeUpdate>) -> Tree {
        let mut updates = updates.into_iter();
        let mut tree = Tree::new(updates.next().expect("a first update"), true);
        for update in updates {
            tree.update_and_process_changes(update, &mut Unheard);
        }
        tree
    }

    /// Every node under `node`, through each graft, outermost first.
    fn every<'a>(node: NodeRef<'a>, out: &mut Vec<NodeRef<'a>>) {
        out.push(node);
        for child in node.children() {
            every(child, out);
        }
    }

    fn all(tree: &Tree) -> Vec<NodeRef<'_>> {
        let mut out = Vec::new();
        every(tree.state().root(), &mut out);
        out
    }

    fn labelled<'a>(nodes: &[NodeRef<'a>], label: &str) -> NodeRef<'a> {
        *nodes
            .iter()
            .find(|node| node.data().label() == Some(label))
            .unwrap_or_else(|| panic!("no node labelled {label}"))
    }

    fn actions(node: NodeRef<'_>) -> Vec<Action> {
        [
            Action::Click,
            Action::Focus,
            Action::SetValue,
            Action::ScrollToPoint,
            Action::SetScrollOffset,
            Action::Expand,
            Action::ScrollDown,
        ]
        .into_iter()
        .filter(|action| node.data().supports_action(*action))
        .collect()
    }

    #[test]
    fn each_kind_of_node_is_published_with_its_role_name_value_and_actions() {
        let (updates, _) = updates(&published(None, true), Grafted::default());
        let tree = applied(updates);
        let nodes = all(&tree);
        let root = tree.state().root();
        assert_eq!(root.role(), Role::Window);
        // The window, its two grafts, and every node of both parts.
        assert_eq!(nodes.len(), 1 + 2 + 5 + 5);
        let new_chat = labelled(&nodes, "New chat");
        assert_eq!(new_chat.role(), Role::Link);
        assert_eq!(actions(new_chat), [Action::Click]);
        let current = labelled(&nodes, "Charging my EV");
        assert_eq!(current.aria_current(), Some(AriaCurrent::Page));
        let heading = labelled(&nodes, "Open");
        assert_eq!((heading.role(), heading.level()), (Role::Heading, Some(2)));
        assert_eq!(actions(heading), []);
        let search = labelled(&nodes, "Search chats");
        assert_eq!(search.role(), Role::TextInput);
        assert_eq!(search.value().as_deref(), Some("inv"));
        assert_eq!(search.data().placeholder(), Some("Search chats"));
        assert_eq!(
            actions(search),
            [Action::Click, Action::Focus, Action::SetValue]
        );
        // A field is a leaf: the run of text inside it is its value.
        assert_eq!(search.children().count(), 0);
        let the_box = labelled(&nodes, "Ask anything, search, or type a website");
        assert_eq!(the_box.value().as_deref(), Some("lisbon"));
        // A run of text is a label whose words are its value.
        let text = nodes
            .iter()
            .find(|node| node.role() == Role::Label)
            .expect("the line of text");
        assert_eq!(
            text.value().as_deref(),
            Some("Ask anything, search, or type a website.")
        );
        assert_eq!(actions(*text), []);
    }

    #[test]
    fn a_password_and_a_cards_number_are_published_without_their_values() {
        let (updates, _) = updates(&published(None, true), Grafted::default());
        let tree = applied(updates);
        let nodes = all(&tree);
        let password = labelled(&nodes, "Password");
        assert_eq!(password.role(), Role::PasswordInput);
        assert_eq!(password.data().value(), None, "not even its dots");
        assert_eq!(actions(password), [Action::Click, Action::Focus]);
        let card = labelled(&nodes, "Card number");
        assert_eq!(card.role(), Role::TextInput);
        assert_eq!(card.data().value(), None);
        assert_eq!(actions(card), [Action::Click, Action::Focus]);
        // And nothing inside either is a node at all.
        assert_eq!(password.children().count() + card.children().count(), 0);
        assert!(
            nodes.iter().all(|node| {
                node.value().is_none_or(|value| !value.contains("hunter2"))
                    && node.value().is_none_or(|value| !value.contains("4242"))
            }),
            "no secret is anywhere in the tree"
        );
    }

    /// The bounds published, in the window's device pixels at two: each
    /// part's place plus its rect, doubled.
    #[test]
    fn bounds_are_the_parts_place_plus_the_rect_at_the_scale() {
        let (updates, _) = updates(&published(None, true), Grafted::default());
        let tree = applied(updates);
        let nodes = all(&tree);
        let of = |label| labelled(&nodes, label).raw_bounds().expect("bounds");
        // *New chat* at (10, 56) 220 × 40 in the panel, which stands at the
        // window's corner.
        assert_eq!(
            of("New chat"),
            Rect {
                x0: 20.0,
                y0: 112.0,
                x1: 460.0,
                y1: 192.0
            }
        );
        // The box at (183, 266) 632 × 40 in the first screen, 480 in.
        assert_eq!(
            of("Ask anything, search, or type a website"),
            Rect {
                x0: 846.0,
                y0: 532.0,
                x1: 2110.0,
                y1: 612.0
            }
        );
        // A part's graft is where the window composed it.
        let grafts: Vec<Rect> = tree
            .state()
            .root()
            .children()
            .filter_map(|graft| graft.raw_bounds())
            .collect();
        assert_eq!(
            grafts,
            [
                Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 480.0,
                    y1: 1640.0
                },
                Rect {
                    x0: 480.0,
                    y0: 0.0,
                    x1: 2560.0,
                    y1: 1640.0
                }
            ]
        );
    }

    #[test]
    fn the_keyboard_is_on_the_field_that_has_it_once_its_part_is_grafted() {
        // First: the root with the window focused, both parts, then the
        // root again with its focus on the first screen's graft.
        let (first, grafted) = updates(&published(Some(Part::Start), true), Grafted::default());
        assert_eq!(first.len(), 4);
        assert_eq!(
            grafted,
            Grafted {
                panel: true,
                start: true
            }
        );
        let mut tree = applied(first);
        let focused = tree.state().focus().expect("a focus");
        assert_eq!(
            focused.data().label(),
            Some("Ask anything, search, or type a website")
        );
        // Then: grafted already, the root focuses it at once, and the panel's
        // *Search chats* takes the keyboard.
        let (then, _) = updates(&published(Some(Part::Panel), false), grafted);
        assert_eq!(then.len(), 3);
        for update in then {
            tree.update_and_process_changes(update, &mut Unheard);
        }
        let focused = tree.state().focus().expect("a focus");
        assert_eq!(focused.data().label(), Some("Search chats"));
        // No field with the keyboard: the window itself.
        let (none, _) = updates(&published(None, false), grafted);
        for update in none {
            tree.update_and_process_changes(update, &mut Unheard);
        }
        assert_eq!(
            tree.state().focus().map(|node| node.role()),
            Some(Role::Window)
        );
    }

    #[test]
    fn a_node_met_twice_is_left_out_and_a_part_not_yet_read_is_not_grafted() {
        let mut twice = published(None, true);
        if let Some(PublishedPart { tree, .. }) = twice.panel.as_mut()
            && let Some(root) = tree.root.as_mut()
        {
            let again = known(
                12,
                KnownRole::Button,
                "Not New chat",
                Laid::new(0.0, 0.0, 1.0, 1.0),
            );
            root.children.push(again);
        }
        twice.start = None;
        let (updates, grafted) = updates(&twice, Grafted::default());
        assert_eq!(
            grafted,
            Grafted {
                panel: true,
                start: false
            }
        );
        let tree = applied(updates);
        let nodes = all(&tree);
        assert!(
            nodes
                .iter()
                .all(|node| node.data().label() != Some("Not New chat"))
        );
        assert_eq!(tree.state().root().children().count(), 1);
    }

    #[test]
    fn a_part_too_large_is_its_root_alone_said_so() {
        let mut large = published(None, true);
        if let Some(PublishedPart { tree, .. }) = large.start.as_mut()
            && let Some(root) = tree.root.as_mut()
        {
            root.children = (0..MOST_NODES)
                .map(|id| {
                    node(
                        1000 + id,
                        Ours::Generic,
                        None,
                        Laid::new(0.0, 0.0, 1.0, 1.0),
                    )
                })
                .collect();
        }
        let (updates, _) = updates(&large, Grafted::default());
        let tree = applied(updates);
        let nodes = all(&tree);
        assert_eq!(labelled(&nodes, TOO_LARGE).children().count(), 0);
        assert_eq!(nodes.len(), 1 + 2 + 5 + 1);
    }

    #[test]
    fn every_role_this_engine_knows_is_published_as_a_role_of_its_own_name() {
        for role in KnownRole::ALL {
            assert_ne!(known_role(role), Role::Unknown, "{role:?}");
        }
        let declared = |name: &str| {
            role(
                &node(
                    1,
                    Ours::Declared(name.into()),
                    None,
                    Laid::new(0.0, 0.0, 1.0, 1.0),
                ),
                None,
            )
        };
        assert_eq!(declared("InputTime"), Role::TimeInput);
        assert_eq!(declared("toolbar"), Role::GenericContainer);
    }

    fn request(action: Action, part: Part, node: u64, data: Option<ActionData>) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: tree_id(part),
            target_node: NodeId(node),
            data,
        }
    }

    #[test]
    fn a_press_text_put_and_the_keyboard_are_taken_by_node() {
        assert_eq!(
            asked(&request(Action::Click, Part::Panel, 12, None)),
            Ok(Asked {
                part: Part::Panel,
                node: BoxId::from_wire(12),
                assist: Assist::Click
            })
        );
        assert_eq!(
            asked(&request(
                Action::SetValue,
                Part::Start,
                40,
                Some(ActionData::Value("lisbon".into()))
            ))
            .map(|asked| asked.assist),
            Ok(Assist::SetValue("lisbon".to_owned()))
        );
        assert_eq!(
            asked(&request(Action::Focus, Part::Start, 40, None)).map(|asked| asked.assist),
            Ok(Assist::Focus)
        );
        assert!(asked(&request(Action::SetValue, Part::Start, 40, None)).is_err());
    }

    #[test]
    fn every_action_that_carries_a_point_is_refused_whatever_it_names() {
        let at = Point::new(10.0, 20.0);
        for refused in [
            request(
                Action::ScrollToPoint,
                Part::Panel,
                12,
                Some(ActionData::ScrollToPoint(at)),
            ),
            request(
                Action::SetScrollOffset,
                Part::Panel,
                12,
                Some(ActionData::SetScrollOffset(at)),
            ),
            // Without its point, and a press carrying one.
            request(Action::ScrollToPoint, Part::Panel, 12, None),
            request(
                Action::Click,
                Part::Panel,
                12,
                Some(ActionData::ScrollToPoint(at)),
            ),
        ] {
            let why = asked(&refused).map(|_| ()).unwrap_err();
            assert!(why.why.contains("coordinate"), "{refused:?}: {why:?}");
        }
    }

    #[test]
    fn every_other_action_and_a_node_of_no_part_are_refused() {
        for action in [
            Action::Blur,
            Action::Collapse,
            Action::Expand,
            Action::CustomAction,
            Action::Decrement,
            Action::Increment,
            Action::HideTooltip,
            Action::ShowTooltip,
            Action::ReplaceSelectedText,
            Action::ScrollDown,
            Action::ScrollLeft,
            Action::ScrollRight,
            Action::ScrollUp,
            Action::ScrollIntoView,
            Action::SetTextSelection,
            Action::SetSequentialFocusNavigationStartingPoint,
            Action::ShowContextMenu,
        ] {
            assert!(
                asked(&request(action, Part::Panel, 12, None)).is_err(),
                "{action:?}"
            );
        }
        // The window's own node, in the root tree.
        let window = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: WINDOW,
            data: None,
        };
        assert_eq!(
            asked(&window).map(|_| ()).unwrap_err().why,
            "it names no node of a part"
        );
    }
    /// The card's page box at two in the design's window: the first
    /// screen 480 in, the page box 81 in and 145 down in it, 878 × 520.
    const PAGE_BOX: (f64, f64, f64, f64) = (642.0, 290.0, 1756.0, 1040.0);

    /// A checkout as Chrome's tree reads it, by `backendDOMNodeId`: a
    /// heading, *Pay UGX 45,000*, a name holding words, a password holding
    /// its dots, a card's number holding digits, and a link.
    fn checkout() -> SnapshotNode {
        let mut heading = known(
            3,
            KnownRole::Heading,
            "Checkout",
            Laid::new(40.0, 10.0, 300.0, 32.0),
        );
        heading.states.level = Some(1);
        with(
            known(
                1,
                KnownRole::Document,
                "Kampala Coffee",
                Laid::new(0.0, 0.0, 878.0, 520.0),
            ),
            vec![
                heading,
                known(
                    7,
                    KnownRole::Button,
                    "Pay UGX 45,000",
                    Laid::new(40.0, 60.0, 200.0, 40.0),
                ),
                field(
                    9,
                    Ours::Known(KnownRole::TextBox),
                    "Your name",
                    "Ada",
                    Laid::new(40.0, 120.0, 300.0, 30.0),
                ),
                field(
                    11,
                    Ours::Known(KnownRole::TextBox),
                    "Secret phrase",
                    "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}",
                    Laid::new(40.0, 170.0, 300.0, 30.0),
                ),
                field(
                    13,
                    Ours::Known(KnownRole::TextBox),
                    "Card number",
                    "4242 4242 4242 4242",
                    Laid::new(40.0, 220.0, 300.0, 30.0),
                ),
                known(
                    15,
                    KnownRole::Link,
                    "Terms",
                    Laid::new(40.0, 270.0, 60.0, 20.0),
                ),
            ],
        )
    }

    /// [`published`], the first screen holding the card's picture of its
    /// page and the page read as `read`.
    fn with_the_page(read: Option<PageRead>) -> Published {
        let mut window = published(None, true);
        if let Some(PublishedPart { tree, .. }) = window.start.as_mut()
            && let Some(root) = tree.root.as_mut()
        {
            root.children.push(known(
                70,
                KnownRole::Image,
                PICTURED,
                Laid::new(81.0, 145.0, 878.0, 520.0),
            ));
        }
        window.page = read.map(|read| PublishedPage { at: PAGE_BOX, read });
        window
    }

    fn page_read() -> PageRead {
        PageRead::Tree(Box::new(Snapshot {
            root: Some(checkout()),
        }))
    }

    #[test]
    fn a_cards_page_is_grafted_under_its_picture_with_its_roles_names_and_values() {
        let (updates, _) = updates(&with_the_page(Some(page_read())), Grafted::default());
        assert_eq!(updates.len(), 4, "the root, two parts and the page");
        let tree = applied(updates);
        let nodes = all(&tree);
        // The window, two grafts, the panel's five, the first screen's five
        // and its picture, and the page's seven.
        assert_eq!(nodes.len(), 1 + 2 + 5 + 6 + 7);
        let graft = labelled(&nodes, PICTURED);
        assert_eq!(graft.role(), Role::Group);
        let page = graft.children().next().expect("the page under its picture");
        assert_eq!(
            (page.role(), page.data().label()),
            (Role::Document, Some("Kampala Coffee"))
        );
        let heading = labelled(&nodes, "Checkout");
        assert_eq!((heading.role(), heading.level()), (Role::Heading, Some(1)));
        let pay = labelled(&nodes, "Pay UGX 45,000");
        assert_eq!(pay.role(), Role::Button);
        assert_eq!(actions(pay), [Action::Click]);
        let name = labelled(&nodes, "Your name");
        assert_eq!(name.role(), Role::TextInput);
        assert_eq!(name.value().as_deref(), Some("Ada"));
        assert_eq!(
            actions(name),
            [Action::Click, Action::Focus, Action::SetValue]
        );
        assert_eq!(labelled(&nodes, "Terms").role(), Role::Link);
    }

    #[test]
    fn a_pages_password_and_card_number_are_published_with_no_value_and_take_no_text() {
        let tree = applied(updates(&with_the_page(Some(page_read())), Grafted::default()).0);
        let nodes = all(&tree);
        let password = labelled(&nodes, "Secret phrase");
        assert_eq!(password.role(), Role::PasswordInput);
        assert_eq!(password.data().value(), None, "not even its dots");
        assert_eq!(actions(password), [Action::Click, Action::Focus]);
        let card = labelled(&nodes, "Card number");
        assert_eq!(card.data().value(), None);
        assert_eq!(actions(card), [Action::Click, Action::Focus]);
        assert!(
            nodes.iter().all(|node| node
                .value()
                .is_none_or(|value| { !value.contains('\u{2022}') && !value.contains("4242") })),
            "no secret is anywhere in the tree"
        );
    }

    /// The page's nodes, in the window's device pixels at two: the page
    /// box's corner plus each rect, doubled.
    #[test]
    fn a_pages_bounds_are_its_page_box_plus_the_rect_at_the_scale() {
        let tree = applied(updates(&with_the_page(Some(page_read())), Grafted::default()).0);
        let nodes = all(&tree);
        let of = |label| labelled(&nodes, label).raw_bounds().expect("bounds");
        assert_eq!(
            of(PICTURED),
            Rect {
                x0: 642.0,
                y0: 290.0,
                x1: 2398.0,
                y1: 1330.0
            }
        );
        // *Pay* at (40, 60) 200 × 40 on the page.
        assert_eq!(
            of("Pay UGX 45,000"),
            Rect {
                x0: 722.0,
                y0: 410.0,
                x1: 1122.0,
                y1: 490.0
            }
        );
        // The page itself, the whole page box.
        assert_eq!(
            of("Kampala Coffee"),
            Rect {
                x0: 642.0,
                y0: 290.0,
                x1: 2398.0,
                y1: 1330.0
            }
        );
    }

    #[test]
    fn a_page_is_never_sent_without_its_graft_and_goes_with_its_picture() {
        // Read, but its card's picture not drawn: no subtree, no graft.
        let mut unpictured = published(None, true);
        unpictured.page = Some(PublishedPage {
            at: PAGE_BOX,
            read: page_read(),
        });
        let (sent, _) = updates(&unpictured, Grafted::default());
        assert_eq!(sent.len(), 3);
        assert!(sent.iter().all(|update| update.tree_id != PAGE));
        let mut tree = applied(sent);
        assert!(
            all(&tree)
                .iter()
                .all(|node| node.data().label() != Some("Pay UGX 45,000"))
        );
        // Grafted, then its picture gone: the page goes with it, and a
        // page published again is grafted again — never a subtree the
        // adapter would refuse.
        for published in [
            with_the_page(Some(page_read())),
            published(None, false),
            with_the_page(Some(page_read())),
        ] {
            let (sent, _) = updates(&published, Grafted::default());
            for update in sent {
                tree.update_and_process_changes(update, &mut Unheard);
            }
        }
        assert!(
            all(&tree)
                .iter()
                .any(|node| node.data().label() == Some("Pay UGX 45,000"))
        );
    }

    #[test]
    fn a_page_too_large_is_its_graft_holding_one_node_said_so() {
        let mut huge = checkout();
        huge.children = (0..MOST_NODES)
            .map(|id| {
                node(
                    1000 + id,
                    Ours::Generic,
                    None,
                    Laid::new(0.0, 0.0, 1.0, 1.0),
                )
            })
            .collect();
        for read in [
            PageRead::Tree(Box::new(Snapshot { root: Some(huge) })),
            PageRead::TooLarge,
        ] {
            let tree = applied(updates(&with_the_page(Some(read)), Grafted::default()).0);
            let nodes = all(&tree);
            let graft = labelled(&nodes, PICTURED);
            let alone = graft.children().next().expect("one node");
            assert_eq!(alone.data().label(), Some(PAGE_TOO_LARGE));
            assert_eq!(alone.children().count(), 0);
            assert_eq!(nodes.len(), 1 + 2 + 5 + 6 + 1);
        }
    }

    /// Queue item 989: a card on a site kept from agents is its graft
    /// holding one node, *this page is kept from agents*, and nothing of
    /// the page under it.
    #[test]
    fn a_kept_page_is_its_graft_holding_one_node_said_so() {
        let tree = applied(updates(&with_the_page(Some(PageRead::Kept)), Grafted::default()).0);
        let nodes = all(&tree);
        let graft = labelled(&nodes, PICTURED);
        let alone = graft.children().next().expect("one node");
        assert_eq!(
            (alone.role(), alone.data().label()),
            (Role::Document, Some(assisted::KEPT))
        );
        assert_eq!(alone.children().count(), 0);
        // Where the page box is, as the graft.
        assert_eq!(alone.raw_bounds(), graft.raw_bounds());
        assert!(alone.raw_bounds().is_some());
        assert_eq!(nodes.len(), 1 + 2 + 5 + 6 + 1);
    }

    /// Queue item 989: *Off* is a window published with no part and no
    /// page — the root tree, the window alone, with no graft — after
    /// everything was published, and everything again after it.
    #[test]
    fn off_is_the_window_alone_and_back_on_everything_again() {
        let everything = with_the_page(Some(page_read()));
        let off = Published {
            panel: None,
            start: None,
            page: None,
            keyboard: None,
            ..published(None, false)
        };
        let (sent, grafted) = updates(&everything, Grafted::default());
        let mut tree = applied(sent);
        let (sent, grafted) = updates(&off, grafted);
        assert_eq!(sent.len(), 1, "the root alone");
        assert_eq!(grafted, Grafted::default());
        for update in sent {
            tree.update_and_process_changes(update, &mut Unheard);
        }
        let nodes = all(&tree);
        assert_eq!(nodes.len(), 1, "{:?}", nodes.first());
        assert_eq!(
            (nodes[0].role(), nodes[0].data().label()),
            (Role::Window, Some("alo"))
        );
        assert_eq!(nodes[0].children().count(), 0);
        let (sent, _) = updates(&everything, grafted);
        for update in sent {
            tree.update_and_process_changes(update, &mut Unheard);
        }
        assert_eq!(all(&tree).len(), 1 + 2 + 5 + 6 + 7);
    }

    #[test]
    fn an_action_on_the_page_is_taken_by_node_and_a_point_refused() {
        let on_the_page = |action, data| ActionRequest {
            action,
            target_tree: PAGE,
            target_node: NodeId(7),
            data,
        };
        assert_eq!(
            heard(&on_the_page(Action::Click, None)),
            FromTheBus::Page {
                node: BoxId::from_wire(7),
                assist: Assist::Click
            }
        );
        assert_eq!(
            heard(&on_the_page(
                Action::SetValue,
                Some(ActionData::Value("porto".into()))
            )),
            FromTheBus::Page {
                node: BoxId::from_wire(7),
                assist: Assist::SetValue("porto".to_owned())
            }
        );
        let at = Point::new(10.0, 20.0);
        for refused in [
            on_the_page(Action::ScrollToPoint, Some(ActionData::ScrollToPoint(at))),
            on_the_page(
                Action::SetScrollOffset,
                Some(ActionData::SetScrollOffset(at)),
            ),
            on_the_page(Action::Click, Some(ActionData::ScrollToPoint(at))),
        ] {
            assert!(
                matches!(heard(&refused), FromTheBus::Refused(why) if why.why.contains("coordinate")),
                "{refused:?}"
            );
        }
        assert!(matches!(
            heard(&on_the_page(Action::ScrollDown, None)),
            FromTheBus::Refused(_)
        ));
        // A part's node is still a part's, and the page is no part.
        assert!(matches!(
            heard(&request(Action::Click, Part::Panel, 12, None)),
            FromTheBus::Asked(_)
        ));
        assert!(asked(&on_the_page(Action::Click, None)).is_err());
    }
}

//! ★ The agent surface: the interface, read as what it *is*.
//!
//! `docs/decisions/0002` calls this the reason this engine exists rather than
//! a faster fork of somebody else's. Every AI browser shipping today reads a
//! page by scraping a DOM built for a different purpose or by photographing
//! the screen and guessing. Both are unreliable, both break silently, and
//! neither can say afterwards what it actually operated on. They do it because
//! they do not own the engine.
//!
//! # One tree, two readers
//!
//! This is a **view** over the boxes and the layout, not a structure built
//! beside them. If the two could disagree, an agent would eventually act on
//! something that is not on screen — so there is nothing to disagree with. An
//! [`AgentNode`] is a box's id and a borrow; every question is answered from
//! the trees that draw the page, when it is asked.
//!
//! The same view is the **accessibility tree**. A screen reader and an agent
//! want identical facts, and two implementations would guarantee one is wrong.
//! That is also why the work counts twice: EN 301 549 conformance and agent
//! capability are one piece of work rather than two competing budgets.
//!
//! # Roles are declared, never inferred
//!
//! What a thing *is* comes from what the author wrote — a `role` attribute, or
//! the element HTML says it is. Nothing here guesses from appearance, because
//! guessing from appearance is what screen-scraping already does badly.
//!
//! # Reading is never watching
//!
//! There is no subscription and no stream. A caller asks and is answered;
//! `alo-os`'s capability model decides who may ask. An engine that
//! continuously streamed its tree would have made that model impossible to
//! keep.
//!
//! # What is here
//!
//! ```text
//! document at (0, 0) 200×140
//!   main at (0, 0) 200×140
//!     heading "Invoices" [level=1] at (8, 8) 184×23
//!     list at (8, 39) 184×76
//!       listitem "Invoice 11" at (8, 39) 184×25
//!       listitem "Invoice 12" [selected=true] at (8, 64) 184×25
//!       listitem "Invoice 13" at (8, 89) 184×25
//! ```
//!
//! That is ADR 0002's opening sentence, produced from HTML and CSS: *"invoice
//! list, twelve rows, row three selected."*

pub mod apply;
pub mod name;
pub mod tree;
pub mod verb;

pub use apply::{Change, apply};
pub use name::accessible_name;
pub use tree::{AgentNode, AgentTree};
pub use verb::{Outcome, Refusal, ScrollBy, Target, Verb, perform};

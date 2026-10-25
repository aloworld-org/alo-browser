/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The alo browser's window: the browser process a person starts.
//!
//! ADR 0024. Its binary is `alo`. It holds the window and nothing of a page
//! (ADR 0005): it owns the tabs, which own the renderers, and it shows what
//! they paint.
//!
//! # How it is put together
//!
//! - [`window`] is the event loop, the one file that names `winit`, and
//!   [`present`] puts pixels in the window, the one file that names
//!   `softbuffer` (ADR 0024 § 1).
//! - [`conductor`] is the thread that owns the tabs and is the only thing
//!   that ever waits on a renderer; it and the event loop speak only in
//!   [`message`]s (§ 2). It waits on its [`inbox`] and never on a server:
//!   [`network`] is the thread that makes every request (ADR 0041), and
//!   [`hold`] keeps a load's first frame back for its style sheets.
//! - [`showing`] is what the window was last sent, and [`compose`] makes the
//!   window's pixels from it — a function, tested by reference render, placed
//!   by [`place`] at the window's [`scale`] and saying what happened to a tab
//!   with [`notice`]. [`mark`] rings the box an agent's verb acted on, on
//!   the frame painted after it (item 624).
//! - [`keys`] is what a person's key asks of the tabs, when it is one of the
//!   browser's own (item 529), read by the event loop before anything else.
//! - [`opening`] is what a person named on the command line, [`fonts`] the
//!   fonts renderers start with, and [`beside`] where the renderer program is.
//! - [`agents`] is the socket an agent outside the browser speaks to (ADR
//!   0055), at a place [`socket_place`] makes private; [`protocol`] reads and
//!   writes its lines, one of the two files that name `serde_json`, into what
//!   [`asking`] says they mean; [`serving`] answers them from the tabs on the
//!   conductor's thread, kept to the sites a person named in
//!   [`agent_sites`], and [`agent_record`] is what it records. The socket
//!   is a Unix one, and Windows is undecided (ADR 0055 § 1). What a verb
//!   that ran was answered with is shown to the person watching, in a line
//!   of the window's own ([`message::News::AgentSaid`], item 623), and so is
//!   why one was refused ([`message::News::AgentRefused`], item 625), for as
//!   long as [`agent_lines`] says.
//! - [`chrome`] is the other renderer, the machine's own Chrome, started and
//!   spoken to over a pipe (ADR 0058, item 619): [`chrome_wire`] finds where
//!   each of its messages ends and [`chrome_protocol`] reads them, the one
//!   other file that names `serde_json`, with [`json_depth`] refusing any
//!   JSON from outside that nests too deep before either reader sees it;
//!   what is read and not yet taken is held by [`chrome_held`], bounded in
//!   messages and in bytes (item 653). Each Chrome starts on a profile of its own that [`chrome_profile`]
//!   makes, keeping it on the machine's resolver (ADR 0008, item 635). A
//!   page in it is a [`chrome_tab`], read as the tree an agent reads by
//!   [`chrome_tree`] (item 620), and acted on by [`chrome_act`]: each verb
//!   decided by `alo-agent` against a [`chrome_reading`] of it, and carried
//!   out as a real click or real typing (item 621), a date a segment at a
//!   time as [`chrome_date`] reads them (item 632); a form Chrome would not
//!   send is known by the bubble [`chrome_bubble`] reads (item 631) and by
//!   each control Chrome logs it could not focus, which
//!   [`chrome_unfocusable`] reads (item 646), and
//!   what the page's script said while a verb ran is put into the engine's
//!   words by [`chrome_said`] (item 639). A page from the network
//!   is carried by Chrome's own network (ADR 0060): every target of its tab
//!   held by [`chrome_hold`] until its requests are paused, and each request
//!   judged by [`chrome_gate`] before Chrome is let send it (item 633), and
//!   written into the record with the cause this process gave it by
//!   [`chrome_record`] (item 634) — a request paused as its frame goes to a
//!   new document owed by [`chrome_owed`] until Chrome names the document
//!   that asked (item 647). A Chrome started shown draws its one
//!   page in a window of its own, which [`chrome_window`] sizes around the
//!   page and places on the screen (ADR 0061, item 627).
//! - [`pages`] is what [`serving`] asks of the tabs, whichever renderer
//!   draws them: [`engine_pages`] answers it from this engine's, and
//!   [`chrome_pages`] from a tab in Chrome, which [`chrome_serving`] holds
//!   on a thread of its own and answers the agent socket about, through the
//!   same serving and the same grants (item 630).
//!
//! # What it is not yet
//!
//! One window. The tab strip is drawn by a renderer of its own (`alo-renderer`'s
//! `strip`, item 297) and shown across the top of the window, the selected
//! tab below it (item 528), and a person opens, closes and chooses tabs with
//! the browser's own keys (item 529), and presses the page with the main
//! mouse button and moves over it ([`aim`] decides what a point is over,
//! [`cursor`] what the page is sent, items 535 and 542) or the strip to
//! choose a tab (item 539). The page and the strip are painted at the
//! window's [`scale`] (item 562, ADR 0057 § 3), 1.5 among them (item 599).
//! Not yet: an agent reading the strip (item 531), a scale factor nobody
//! has measured, such as 1.25 — painted at the largest measured scale below
//! it — and a link opening a tab (item 300).

pub mod agent_lines;
pub mod agent_record;
pub mod agent_sites;
#[cfg(unix)]
pub mod agents;
pub mod aim;
pub mod asking;
pub mod beside;
#[cfg(unix)]
pub mod chrome;
pub mod chrome_act;
pub mod chrome_bubble;
pub mod chrome_date;
pub mod chrome_gate;
pub mod chrome_held;
pub mod chrome_hold;
pub mod chrome_owed;
pub mod chrome_pages;
pub mod chrome_profile;
pub mod chrome_protocol;
pub mod chrome_reading;
pub mod chrome_record;
pub mod chrome_said;
pub mod chrome_serving;
pub mod chrome_tab;
pub mod chrome_tree;
pub mod chrome_unfocusable;
pub mod chrome_window;
pub mod chrome_wire;
pub mod colours;
pub mod compose;
pub mod conductor;
pub mod cursor;
pub mod engine_pages;
pub mod fonts;
pub mod hold;
pub mod inbox;
pub mod json_depth;
pub mod keys;
pub mod mark;
pub mod message;
pub mod network;
pub mod notice;
pub mod opening;
pub mod pages;
pub mod place;
pub mod present;
pub mod protocol;
pub mod scale;
pub mod serving;
pub mod showing;
#[cfg(unix)]
pub mod socket_place;
pub mod window;

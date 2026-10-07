/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page asks to keep, and the browser process keeping it.
//!
//! ADR 0025 is the decision this crate implements: **one bucket per storage
//! key** — the document's origin and the top-level site it is under — held by
//! the browser process; **one fixed quota** over each bucket; the profile's
//! own bound over all of them, kept by evicting **whole buckets**, least
//! recently used first; and a bucket that fails its check **set aside whole**,
//! never served in part and never deleted.
//!
//! Today it holds `localStorage` areas, the first of the four parts a bucket
//! will have (queue item 301). `sessionStorage`, `navigator.storage`,
//! `IndexedDB` and the Cache API are items 302 to 305, and each adds to the same
//! bucket under the same quota rather than to a store of its own — which is
//! what *one quota policy over all of them* means.
//!
//! It is a crate of its own rather than part of `alo-net` because keeping what
//! a page asked for is not loading anything. What it shares with `alo-net` —
//! the hostile-input reader, the private directory, the partition — it uses
//! from there rather than copying, so a promise about a person's disk is made
//! once.
//!
//! - [`key`]: whose bucket, and why an opaque origin has none.
//! - [`area`]: a `localStorage` area, counted in UTF-16 bytes.
//! - [`limits`]: the ADR's numbers.
//! - [`volume`]: how much of the disk is free, asked of the operating system.
//! - [`ledger`]: counts, use order, and what eviction chooses.
//! - [`record`]: the bytes on a disk, read as a stranger's.
//! - [`directory`]: where each bucket's files are, and setting one aside.
//! - [`store`]: all of it, in the order that makes a refusal change nothing.

pub mod area;
pub mod directory;
pub mod key;
pub mod ledger;
pub mod limits;
pub mod record;
pub mod store;
pub mod volume;

pub use area::Area;
pub use directory::{SetAside, where_the_system_keeps_storage};
pub use key::StorageKey;
pub use limits::Limits;
pub use store::{Refused, Store};

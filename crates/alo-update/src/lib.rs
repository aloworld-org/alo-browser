/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How alo updates itself (ADR 0116, queue item 721): a new alo — and with
//! it a new Chromium — reaching a person within days of a security release,
//! checked and signed by alo, with the person told, without trusting the
//! server it came from, sending anything about the person, or ending an
//! agent's work.
//!
//! # One thing is updated: the whole application
//!
//! alo's programs, the CEF host and its helpers, CEF's framework and the
//! licences are one bundle with one version, built, sealed and tested
//! together, so they are shipped together. No delta updates at first.
//!
//! # The road
//!
//! - **What a build was given** ([`given`]): an address and two public
//!   keys, put in the application by `alo-bundle` — or none, and then it
//!   does not update itself, as every build from this repository today
//!   does not. Inside a Flatpak, Flatpak updates alo.
//! - **The check** ([`updater`]): the manifest ([`manifest`]) and its
//!   minisign signature ([`signature`]) fetched through `alo-net`
//!   ([`feed`]), checked in order ([`check`]) against the highest serial
//!   believed ([`kept`]), the installed version ([`version`]) and the
//!   clock ([`instant`]).
//! - **The package** ([`package`]): downloaded in ranges to a `.partial`,
//!   exactly its size and its SHA-256.
//! - **Unpacked beside the application** ([`stage`]) and checked again:
//!   `codesign --verify --strict`, its version.
//! - **Swapped in at quit** ([`quitting`]), atomically ([`swap`]), and
//!   never while alo runs: alo never restarts by itself.
//! - **The person told** ([`found`]).
//!
//! # No `unsafe` of alo's
//!
//! The signature is `minisign-verify`'s to check, in one file; the swap and
//! the disk's free space are `rustix`'s safe functions. Nothing here asks
//! for more rights than the person has.
//!
//! # The agent socket has no part in it
//!
//! Nothing here is reachable from an agent's ask: no verb starts, delays,
//! refuses or reads an update.

pub mod address;
pub mod check;
pub mod feed;
pub mod found;
pub mod given;
pub mod instant;
pub mod kept;
pub mod manifest;
pub mod package;
pub mod platform;
pub mod quitting;
pub mod signature;
pub mod stage;
pub mod swap;
pub mod test_key;
pub mod updater;
pub mod version;

/// The worthless test keys' secret halves, for this crate's unit tests:
/// the same file its integration tests use.
#[cfg(test)]
#[path = "../tests/worthless_test_key/mod.rs"]
mod worthless_test_key;

pub use found::Found;
pub use given::Updating;
pub use instant::Instant;
pub use updater::Updater;
pub use version::Version;

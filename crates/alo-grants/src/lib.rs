/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person has allowed a site to do, and for how long.
//!
//! ADR 0026 is the decision this crate implements: a permission is a
//! **grant** in one table the browser process holds — one capability from a
//! **closed list**, to one **storage key**, made by **a person** answering an
//! ask a page made **while somebody was using it**. It **always ends**, it is
//! **revoked** in one act that stops every use at once, and it is **recorded**
//! without what it carried. **No agent can make, answer or revoke one.**
//!
//! It holds no device and no interface (queue item 307). Its question is *may
//! this key use this capability now*, and it answers in a value. The prompt and
//! the indicator are item 308's, and each capability's API arrives with its
//! own item.
//!
//! - [`capability`]: the closed list, and the names a page may use for it.
//! - [`ask`]: the facts of an ask, and what can come of one.
//! - [`grant`]: the three answers and when each ends.
//! - [`history`]: the record, sixty-four entries per key, without content.
//! - [`file`]: the bytes on a disk, read as a stranger's.
//! - [`disk`]: where they are, and setting a table aside.
//! - [`table`]: all of it, in the order that makes each refusal a rule.

pub mod ask;
pub mod capability;
pub mod disk;
pub mod file;
pub mod grant;
pub mod history;
pub mod table;

pub use ask::{Ask, Decision, Prompted, Refusal};
pub use capability::{Capability, NotOnTheList};
pub use disk::where_the_system_keeps_grants;
pub use grant::{Answer, Ending, Kept, Remembered, THIRTY_DAYS};
pub use history::{Entry, Happened, History};
pub use table::{NotAPerson, NotGranted, Table};

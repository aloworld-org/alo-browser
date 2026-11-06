/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A chat deleted from the panel, held for its *Undo* (queue item 789,
//! ADR 0092's amendment of 6 November 2026).
//!
//! *Delete* on a chat's row takes it out of the panel at once and holds it
//! here — its row, its turns and where it stood — for [`UNDO_FOR`], while
//! the panel's foot says *Deleted “…”* with *Undo*. *Undo* puts it back
//! where it stood, its turns as they were. Once the time has passed, or
//! another chat is deleted, or the window closes, it is **gone**: its file
//! is removed then, and only then ([`crate::chat_store::ChatStore::remove`]),
//! so a chat brought back never needed writing again, and one deleted by
//! mistake is never already off the disk.
//!
//! Five seconds is the time alo's own *Undo send* holds a mail
//! (`alo-workplace`'s mail module), which a person of alo's already knows.

use crate::conversation::ChatId;
use crate::conversation_thread::Thread;
use alo_renderer::panel::Chat;
use std::time::{Duration, Instant};

/// How long a chat deleted can be brought back.
pub const UNDO_FOR: Duration = Duration::from_secs(5);

/// A chat deleted and not yet gone.
#[derive(Debug, Clone, PartialEq)]
pub struct Deleting {
    /// Which chat.
    pub id: ChatId,
    /// Its row, as it was.
    pub chat: Chat,
    /// Its turns, a live card put away as its address.
    pub thread: Thread,
    /// Where its row stood among the chats.
    pub place: usize,
    /// When it is gone, unless brought back first.
    pub until: Instant,
}

impl Deleting {
    /// `chat`, deleted `now` from `place`, with its `thread`: gone at
    /// [`UNDO_FOR`] after.
    pub fn at(id: ChatId, chat: Chat, thread: Thread, place: usize, now: Instant) -> Self {
        Self {
            id,
            chat,
            thread,
            place,
            until: now + UNDO_FOR,
        }
    }

    /// Whether it is gone by `now`.
    pub fn passed(&self, now: Instant) -> bool {
        now >= self.until
    }

    /// What the panel's foot calls it: the name its row showed.
    pub fn name(&self) -> String {
        self.chat.label().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_deleted_can_be_brought_back_for_five_seconds_and_no_longer() {
        let now = Instant::now();
        let deleting = Deleting::at(
            crate::conversation::ChatId::for_a_test(3),
            Chat::new("Weekend in Lisbon", ""),
            Thread::default(),
            1,
            now,
        );
        assert!(!deleting.passed(now));
        assert!(!deleting.passed(now + Duration::from_millis(4_999)));
        assert!(deleting.passed(now + Duration::from_secs(5)));
        assert_eq!(deleting.name(), "Weekend in Lisbon");
        let unnamed = Deleting::at(
            crate::conversation::ChatId::for_a_test(4),
            Chat::new("", ""),
            Thread::default(),
            0,
            now,
        );
        assert_eq!(unnamed.name(), "New chat");
    }
}

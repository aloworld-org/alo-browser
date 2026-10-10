/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A document's visibility state (ADR 0039 § 1, queue item 364).
//!
//! **Whether a page can be seen is the browser process's to know**: only it
//! knows which tab is selected and whether the window is covered. The
//! renderer is told, and states it here, in the document cell: first when
//! the page's heap is made, then by the task that updates it, which is the
//! only thing that changes it ([`update`]). A page reads it with
//! `document.visibilityState` and `document.hidden`
//! ([`crate::interface::document_visibility`]) and cannot write it.
//!
//! **A document starts `hidden`**, as HTML says every document does: only
//! a browsing context's visibility ever changes it, so a document no
//! window was associated with — a second one, made beside the page's — is
//! `hidden` for its whole life.
//!
//! HTML's third state, `prerender`, belongs to prerendering, which does not
//! exist here.

use alo_js::heap::Ref;
use alo_js::object::Objects;

use crate::document_cell::DocumentCell;

/// Whether a document can be seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// The selected tab of a window that is neither covered nor minimised —
    /// or a tab no window shows, which whoever drives it is reading (ADR
    /// 0039 § 1).
    Visible,
    /// Anything else: and every document, until it is told otherwise.
    #[default]
    Hidden,
}

impl Visibility {
    /// What `document.visibilityState` answers.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
        }
    }
}

/// The visibility state of the document `cell` holds, or [`None`] if it is
/// not a document cell.
pub fn of(objects: &Objects, cell: Ref) -> Option<Visibility> {
    objects
        .embedded::<DocumentCell>(cell)
        .map(DocumentCell::visibility)
}

/// Set the visibility state of the document `cell` holds to `to`: whether
/// that changed it, or [`None`] if `cell` is not a document cell.
///
/// HTML's *update the visibility state*, its first half: a state the
/// document already has changes nothing, and its caller fires nothing. The
/// second half — firing `visibilitychange` when it did change — is the
/// event loop's task, since it runs the page's listeners.
pub fn update(objects: &mut Objects, cell: Ref, to: Visibility) -> Option<bool> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| {
        let changed = held.visibility != to;
        held.visibility = to;
        changed
    })
}

#[cfg(test)]
mod tests {
    use super::Visibility;

    #[test]
    fn a_document_is_hidden_until_it_is_told_otherwise() {
        assert_eq!(Visibility::default(), Visibility::Hidden);
        assert_eq!(Visibility::Visible.as_str(), "visible");
        assert_eq!(Visibility::Hidden.as_str(), "hidden");
    }
}

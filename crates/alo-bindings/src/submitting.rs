/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The forms firing their submission events now (ADR 0051 § 4, queue item
//! 485).
//!
//! HTML gives every form a **firing submission events** flag: set while its
//! `invalid` and `submit` events are dispatched, so a listener that submits
//! the same form again — by pressing its button, say — does nothing rather
//! than recursing for ever. It is the first of HTML's two guards. The second,
//! *constructing entry list*, guards the `formdata` event, which arrives with
//! `FormData` (queue item 472) and is not here.
//!
//! The flag is a fact about the document, and the document cell is the one
//! place both drivers of a submission reach: the renderer's event loop, which
//! runs an agent's press, and a script's `click()`, a native in the middle of
//! the page's script (ADR 0018 § 3). So the forms whose flag is set are kept
//! there, by node. A submission nests at most as deep as the page's own
//! listeners, so the list is a handful long at most.

use alo_dom::NodeId;
use alo_js::heap::Ref;
use alo_js::object::Objects;

use crate::document_cell::DocumentCell;

/// Set `form`'s flag in the document `cell` holds: `Some(true)` if it was
/// clear and is now set, `Some(false)` if it was already set — the guard,
/// and the submission does nothing — and [`None`] if `cell` is not a
/// document cell.
pub fn begin(objects: &mut Objects, cell: Ref, form: NodeId) -> Option<bool> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| {
        if held.submitting.contains(&form) {
            return false;
        }
        held.submitting.push(form);
        true
    })
}

/// Unset `form`'s flag: its submission events have been dispatched.
/// Whether `cell` is a document cell.
pub fn end(objects: &mut Objects, cell: Ref, form: NodeId) -> bool {
    objects
        .write_embedded::<DocumentCell, _>(cell, |held, _| {
            held.submitting.retain(|firing| *firing != form);
        })
        .is_some()
}

/// Whether `form` is firing its submission events now, in the document
/// `cell` holds — `false` if `cell` is not a document cell.
pub fn is_firing(objects: &Objects, cell: Ref, form: NodeId) -> bool {
    objects
        .embedded::<DocumentCell>(cell)
        .is_some_and(|held| held.submitting.contains(&form))
}

#[cfg(test)]
mod tests {
    use alo_dom::parse_document;
    use alo_js::Engine;

    use super::*;

    #[test]
    fn a_form_is_flagged_once_until_its_events_have_been_fired() {
        let Ok(mut engine) = Engine::new() else {
            panic!("an empty heap holds an engine");
        };
        let objects = engine.objects();
        let document = parse_document("<form></form><form></form>");
        let forms: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html("form"))
            })
            .collect();
        let (Some(&first), Some(&second)) = (forms.first(), forms.get(1)) else {
            panic!("two forms parse");
        };
        let Ok(cell) = objects.foreign(Box::new(DocumentCell::new(document))) else {
            panic!("an empty heap holds a document");
        };
        assert!(!is_firing(objects, cell, first));
        assert_eq!(begin(objects, cell, first), Some(true));
        assert!(is_firing(objects, cell, first));
        assert_eq!(begin(objects, cell, first), Some(false), "the guard");
        assert!(!is_firing(objects, cell, second), "a flag is one form's");
        assert_eq!(begin(objects, cell, second), Some(true));
        assert!(end(objects, cell, first));
        assert!(!is_firing(objects, cell, first));
        assert!(is_firing(objects, cell, second));
        assert_eq!(
            begin(objects, cell, first),
            Some(true),
            "set again once clear"
        );
    }

    #[test]
    fn what_is_not_a_document_cell_has_no_flags() {
        let Ok(mut engine) = Engine::new() else {
            panic!("an empty heap holds an engine");
        };
        let objects = engine.objects();
        let Ok(list) = objects.slots() else {
            panic!("an empty heap holds a list");
        };
        let form = parse_document("<form></form>").root();
        assert_eq!(begin(objects, list, form), None);
        assert!(!end(objects, list, form));
        assert!(!is_firing(objects, list, form));
    }
}

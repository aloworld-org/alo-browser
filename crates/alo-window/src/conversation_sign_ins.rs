/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What *Settings › Privacy*'s row *Sign-ins* says, and *Forget all
//! sign-ins* done (ADR 0111 § 5, queue item 935): the window's side of the
//! jar every card keeps ([`crate::sign_ins`]).
//!
//! The row's words are fixed ([`SignInsShown::said`]); why sign-ins are
//! not kept, in full, goes to the terminal, as every other reason does.

use crate::sign_ins::{Kept, SignIns};
use alo_renderer::settings_privacy::SignInsShown;

/// What the row says of a jar kept as `kept` says.
pub fn shown(kept: &Kept) -> SignInsShown {
    match kept {
        Kept::OnThisMac(_) => SignInsShown::Kept,
        Kept::ByAnother => SignInsShown::ByAnother,
        Kept::Unopened => SignInsShown::Unopened,
        Kept::InMemory | Kept::NoKey(_) | Kept::Nowhere(_) => SignInsShown::NotKept,
    }
}

/// What the terminal says of a jar kept as `kept` says, as alo opens.
pub fn told(kept: &Kept) -> String {
    match kept {
        Kept::OnThisMac(directory) => {
            format!("alo: sign-ins are kept in {}", directory.display())
        }
        Kept::InMemory => "alo: sign-ins are kept only until alo quits".to_owned(),
        Kept::ByAnother => {
            "alo: another alo that is open keeps the sign-ins; this one keeps its own until it \
             quits"
                .to_owned()
        }
        Kept::NoKey(why) => {
            format!("alo: sign-ins are kept only until alo quits: there is no key ({why})")
        }
        Kept::Unopened => "alo: the sign-ins kept do not open with the key in this keychain, \
                           and are left as they are until they are forgotten in Settings"
            .to_owned(),
        Kept::Nowhere(why) => {
            format!("alo: sign-ins are kept only until alo quits: there is nowhere ({why})")
        }
    }
}

/// Forget every sign-in `sign_ins` holds: what the row says after, and
/// the line said of it.
pub fn forget(sign_ins: Option<&SignIns>) -> (SignInsShown, String) {
    let Some(sign_ins) = sign_ins else {
        return (
            SignInsShown::NotKept,
            "Privacy: no sign-in is kept, so none was forgotten".to_owned(),
        );
    };
    match sign_ins.forget_all() {
        Ok(()) => (
            match sign_ins.kept_as() {
                Kept::OnThisMac(_) | Kept::InMemory => SignInsShown::Forgotten,
                kept => shown(&kept),
            },
            "Privacy: every sign-in was forgotten, and the open card's (Settings)".to_owned(),
        ),
        Err(why) => (
            shown(&sign_ins.kept_as()),
            format!("Privacy: the sign-ins were forgotten here, but {why}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kept_cookie::{Changes, KeptCookie, Priority, Scheme};

    #[test]
    fn each_kind_of_kept_is_shown_and_told() {
        let at = std::path::PathBuf::from("/Users/someone/Library/Application Support/x");
        assert_eq!(shown(&Kept::OnThisMac(at.clone())), SignInsShown::Kept);
        assert_eq!(shown(&Kept::ByAnother), SignInsShown::ByAnother);
        assert_eq!(shown(&Kept::Unopened), SignInsShown::Unopened);
        for not in [
            Kept::InMemory,
            Kept::NoKey("locked".to_owned()),
            Kept::Nowhere("no HOME".to_owned()),
        ] {
            assert_eq!(shown(&not), SignInsShown::NotKept);
        }
        assert!(told(&Kept::OnThisMac(at)).contains("Application Support"));
        assert!(told(&Kept::NoKey("locked".to_owned())).contains("locked"));
    }

    #[test]
    fn forgetting_empties_the_jar_and_is_shown() {
        let jar = SignIns::in_memory();
        let cookie = KeptCookie {
            name: "first".to_owned(),
            value: "1".to_owned(),
            domain: "127.0.0.1".to_owned(),
            path: "/".to_owned(),
            expires: Some(4_000_000_000.0),
            http_only: false,
            secure: false,
            same_site: None,
            priority: Priority::Medium,
            source_scheme: Scheme::NonSecure,
            source_port: 80,
        };
        let made = jar.change(&Changes {
            gone: Vec::new(),
            made: vec![cookie],
        });
        assert_eq!(made, Ok(()));
        let (shown, line) = forget(Some(&jar));
        assert_eq!(shown, SignInsShown::Forgotten);
        assert!(line.contains("every sign-in was forgotten"), "{line}");
        assert_eq!(jar.cookies(), Vec::new());
        assert_eq!(jar.forgotten(), 1);
        assert_eq!(forget(None).0, SignInsShown::NotKept);
    }
}

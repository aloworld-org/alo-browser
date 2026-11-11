/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' fourth section, **Privacy**, as data (ADR 0108, queue item
//! 920): what the Chromium alo drives keeps of sites' cookies
//! ([`Cookies`]), and whether alo answers a site's cookie banner with its
//! most private choice ([`Banners`]) — and whether the sign-ins are kept
//! on this Mac, with **Forget all sign-ins** ([`SignInsShown`], ADR 0111,
//! queue item 935) — drawn by [`crate::settings_privacy_document`],
//! decided by the browser process.
//!
//! # What a press is on
//!
//! Everything of the section a person presses, in the order it is drawn
//! ([`Privacy::pressables`]): the three of *Cookies*, then the two of
//! *Cookie banners*, then *Forget all sign-ins*. The browser process
//! decides a press by its place among them, against the same list it
//! painted, as *alo at work*'s are ([`crate::conversation_claim`]).

/// What the Chromium alo drives keeps of sites' cookies (ADR 0108 § 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cookies {
    /// A site's own cookies kept, and another site's — one a page of a
    /// different site holds, an advert's or a tracker's — refused: the
    /// default.
    #[default]
    RefuseOthers,
    /// Every site's refused: signing in then fails, which is said.
    RefuseAll,
    /// Every site's kept, as a Chromium nobody set keeps them.
    KeepAll,
}

impl Cookies {
    /// Each, in the order they are drawn.
    pub const ALL: [Cookies; 3] = [Cookies::RefuseOthers, Cookies::RefuseAll, Cookies::KeepAll];

    /// What its choice says, and is named.
    pub fn said(self) -> &'static str {
        match self {
            Self::RefuseOthers => "Refuse other sites\u{2019} cookies",
            Self::RefuseAll => "Refuse all cookies",
            Self::KeepAll => "Keep all cookies",
        }
    }

    /// The quiet line under its choice, if it has one: what it costs.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::RefuseAll => Some("Signing in to sites will fail: a site cannot remember you."),
            Self::RefuseOthers | Self::KeepAll => None,
        }
    }

    /// Its word in the settings file.
    pub fn word(self) -> &'static str {
        match self {
            Self::RefuseOthers => "others",
            Self::RefuseAll => "all",
            Self::KeepAll => "keep",
        }
    }

    /// The choice `word` names in the settings file, if any.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|each| each.word() == word)
    }

    /// The next, after it when `on`, or the one before, round.
    #[must_use]
    pub fn stepped(self, on: bool) -> Self {
        stepped(&Self::ALL, self, on)
    }
}

/// Whether alo answers a site's cookie banner (ADR 0108 § 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Banners {
    /// With its most private choice — never *Accept*: the default.
    #[default]
    Answer,
    /// Left to the person, always.
    Leave,
}

impl Banners {
    /// Each, in the order they are drawn.
    pub const ALL: [Banners; 2] = [Banners::Answer, Banners::Leave];

    /// What its choice says, and is named.
    pub fn said(self) -> &'static str {
        match self {
            Self::Answer => "Answer with the most private choice",
            Self::Leave => "Leave them to me",
        }
    }

    /// Its word in the settings file.
    pub fn word(self) -> &'static str {
        match self {
            Self::Answer => "answer",
            Self::Leave => "leave",
        }
    }

    /// The choice `word` names in the settings file, if any.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|each| each.word() == word)
    }

    /// The other, either way: two choices step round to each other.
    #[must_use]
    pub fn stepped(self, on: bool) -> Self {
        stepped(&Self::ALL, self, on)
    }
}

/// The one after `now` in `all` when `on`, or the one before, round.
fn stepped<T: Copy + PartialEq>(all: &[T], now: T, on: bool) -> T {
    let at = all.iter().position(|each| *each == now).unwrap_or_default();
    let next = if on {
        (at + 1) % all.len()
    } else {
        (at + all.len() - 1) % all.len()
    };
    all.get(next).copied().unwrap_or(now)
}

/// What the row *Sign-ins* says of the sign-ins kept (ADR 0111 § 5): the
/// browser process's to say, never a renderer's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SignInsShown {
    /// Kept on this Mac, sealed: the default.
    #[default]
    Kept,
    /// Every one forgotten, just now.
    Forgotten,
    /// Kept by another alo that is open: this one's until it quits.
    ByAnother,
    /// Not kept on this Mac: kept until alo quits.
    NotKept,
    /// What is kept does not open with the key in the keychain.
    Unopened,
}

impl SignInsShown {
    /// Each, in no order a person sees.
    pub const ALL: [SignInsShown; 5] = [
        SignInsShown::Kept,
        SignInsShown::Forgotten,
        SignInsShown::ByAnother,
        SignInsShown::NotKept,
        SignInsShown::Unopened,
    ];

    /// What the row says.
    pub fn said(self) -> &'static str {
        match self {
            Self::Kept => {
                "Signed-in sites are kept on this Mac, sealed under a key in your keychain, \
                 until you forget them."
            }
            Self::Forgotten => {
                "Every sign-in on this Mac was forgotten. Sites you sign in to from now on are \
                 kept again."
            }
            Self::ByAnother => {
                "Another alo that is open keeps the sign-ins. This one keeps its own until it \
                 quits."
            }
            Self::NotKept => {
                "Sign-ins are kept only until alo quits: they could not be kept on this Mac."
            }
            Self::Unopened => {
                "The sign-ins kept on this Mac do not open with the key in your keychain. \
                 Forget all sign-ins to keep them again."
            }
        }
    }
}

/// What the button that forgets every sign-in says, and is named.
pub const FORGET_ALL: &str = "Forget all sign-ins";

/// Something of the section a person presses: a choice, or the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrivacyPress {
    /// This choice of cookies.
    Cookies(Cookies),
    /// This choice of banners.
    Banners(Banners),
    /// *Forget all sign-ins* (ADR 0111 § 5): not a choice, a deed the
    /// browser process does.
    ForgetSignIns,
}

/// The section's two choices, as they are made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Privacy {
    /// What is kept of sites' cookies.
    pub cookies: Cookies,
    /// Whether a site's cookie banner is answered.
    pub banners: Banners,
}

impl Privacy {
    /// Everything a person may press, in the order it is drawn.
    pub fn pressables() -> Vec<PrivacyPress> {
        Cookies::ALL
            .into_iter()
            .map(PrivacyPress::Cookies)
            .chain(Banners::ALL.into_iter().map(PrivacyPress::Banners))
            .chain([PrivacyPress::ForgetSignIns])
            .collect()
    }

    /// The same, with `pressed` chosen — the same when it is not a choice.
    #[must_use]
    pub fn choosing(self, pressed: PrivacyPress) -> Self {
        match pressed {
            PrivacyPress::Cookies(cookies) => Self { cookies, ..self },
            PrivacyPress::Banners(banners) => Self { banners, ..self },
            PrivacyPress::ForgetSignIns => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pressables_are_the_three_then_the_two_and_a_choice_is_made_alone() {
        assert_eq!(
            Privacy::pressables(),
            vec![
                PrivacyPress::Cookies(Cookies::RefuseOthers),
                PrivacyPress::Cookies(Cookies::RefuseAll),
                PrivacyPress::Cookies(Cookies::KeepAll),
                PrivacyPress::Banners(Banners::Answer),
                PrivacyPress::Banners(Banners::Leave),
                PrivacyPress::ForgetSignIns,
            ]
        );
        // Forgetting is not a choice: nothing is made.
        assert_eq!(
            Privacy::default().choosing(PrivacyPress::ForgetSignIns),
            Privacy::default()
        );
        let made = Privacy::default().choosing(PrivacyPress::Cookies(Cookies::KeepAll));
        assert_eq!(made.cookies, Cookies::KeepAll);
        assert_eq!(made.banners, Banners::Answer);
        let made = made.choosing(PrivacyPress::Banners(Banners::Leave));
        assert_eq!(
            (made.cookies, made.banners),
            (Cookies::KeepAll, Banners::Leave)
        );
    }

    #[test]
    fn the_defaults_are_the_private_ones_and_each_word_reads_back() {
        assert_eq!(Cookies::default(), Cookies::RefuseOthers);
        assert_eq!(Banners::default(), Banners::Answer);
        for cookies in Cookies::ALL {
            assert_eq!(Cookies::of_word(cookies.word()), Some(cookies));
        }
        for banners in Banners::ALL {
            assert_eq!(Banners::of_word(banners.word()), Some(banners));
        }
        assert_eq!(Cookies::of_word("none"), None);
        // Refusing every site's says what it costs; nothing else does.
        assert!(
            Cookies::RefuseAll
                .note()
                .is_some_and(|note| note.contains("Signing in"))
        );
        assert_eq!(Cookies::RefuseOthers.note(), None);
    }

    #[test]
    fn each_sign_ins_line_is_its_own_and_says_where_they_are() {
        let said: std::collections::HashSet<_> =
            SignInsShown::ALL.iter().map(|each| each.said()).collect();
        assert_eq!(said.len(), SignInsShown::ALL.len());
        assert!(said.iter().all(|each| !each.contains("  ")), "{said:?}");
        assert!(
            SignInsShown::default()
                .said()
                .starts_with("Signed-in sites are kept on this Mac")
        );
        assert!(SignInsShown::Unopened.said().contains(FORGET_ALL));
    }

    #[test]
    fn an_arrow_steps_round_either_way() {
        assert_eq!(Cookies::RefuseOthers.stepped(true), Cookies::RefuseAll);
        assert_eq!(Cookies::KeepAll.stepped(true), Cookies::RefuseOthers);
        assert_eq!(Cookies::RefuseOthers.stepped(false), Cookies::KeepAll);
        assert_eq!(Banners::Answer.stepped(true), Banners::Leave);
        assert_eq!(Banners::Answer.stepped(false), Banners::Leave);
    }
}

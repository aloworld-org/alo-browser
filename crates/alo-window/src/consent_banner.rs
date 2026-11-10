/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A site's cookie banner, and its most private choice, found **by name**
//! in the page's tree (ADR 0108 § 2, queue item 920) — never by where
//! anything is drawn (ADR 0002), and never by a class or an id a site
//! wrote, which no tree an agent reads holds.
//!
//! # What a choice is
//!
//! A button or a link, not disabled, whose accessible name, written
//! plainly ([`plain`]: in small letters, its spaces made one, curly
//! apostrophes straight, `&` as *and*, a full stop or an exclamation mark
//! at its end dropped), is **exactly** one of:
//!
//! - [`REFUSALS`] — *Reject all*, *Only necessary*, *Decline*, *Deny*,
//!   *Necessary only*, *Refuse*, *Reject optional*, *Continue without
//!   accepting*, … and the standard labels of the common consent
//!   platforms in German, French, Spanish, Italian, Dutch and Portuguese:
//!   the most private choice, which alo may press;
//! - [`ACCEPTS`] — *Accept*, *Accept all*, *Allow all*, *Agree*, *OK*, *Got
//!   it*, …: a consent, which alo never presses.
//!
//! A name is matched whole, never by a word in it: *Accept only necessary*
//! (a refusal, as one platform labels its) and *Accept all* (a consent) differ
//! only past their first word, and no rule that read words could be
//! trusted to keep them apart. A name in neither list — *Adjust my
//! preferences*, *More options*, *Close* — is neither, and is never
//! pressed: a banner's second layer, and what closing one means, are not
//! read (ADR 0108 § 2).
//!
//! # What a banner is
//!
//! The nearest box around a choice, at most [`MOST_UP`] boxes up and never
//! the page itself or its `main`, whose words — its name and its
//! descendants' — speak of cookies or consent ([`SPEAKS`]), and which
//! holds at most [`MOST_PRESSABLE`] buttons and links. A choice with no
//! such box is a page's own *Accept* or *Decline* (an invitation, a form),
//! and is not a banner's.
//!
//! # What is found
//!
//! The first refusal of a banner, in the order a person meets them
//! ([`Found::Refusal`]); else, a banner holding a consent and no refusal
//! ([`Found::NoRefusal`]), which is left to the person; else nothing.

use alo_box::{BoxId, KnownRole, Role};
use alo_renderer::{Snapshot, SnapshotNode};

/// How many boxes up from a choice its banner is looked for.
pub const MOST_UP: usize = 8;

/// The names, written plainly, of a banner's most private choice.
pub const REFUSALS: &[&str] = &[
    // English.
    "reject all",
    "reject all cookies",
    "reject",
    "reject cookies",
    "reject optional",
    "reject optional cookies",
    "reject non-essential",
    "reject non-essential cookies",
    "reject additional cookies",
    "reject unnecessary cookies",
    "reject all and close",
    "decline",
    "decline all",
    "decline cookies",
    "decline all cookies",
    "decline optional cookies",
    "deny",
    "deny all",
    "deny cookies",
    "refuse",
    "refuse all",
    "refuse cookies",
    "refuse all cookies",
    "disagree",
    "i disagree",
    "disagree and close",
    "do not accept",
    "don't accept",
    "only necessary",
    "only necessary cookies",
    "necessary only",
    "necessary cookies only",
    "only essential",
    "only essential cookies",
    "essential only",
    "essential cookies only",
    "strictly necessary only",
    "strictly necessary cookies only",
    "use necessary cookies only",
    "use essential cookies only",
    "allow necessary only",
    "allow necessary cookies only",
    "allow only necessary cookies",
    "allow essential cookies only",
    "accept only necessary",
    "accept only necessary cookies",
    "accept necessary only",
    "accept necessary cookies only",
    "accept only essential cookies",
    "accept essential cookies only",
    "continue without accepting",
    // German.
    "alle ablehnen",
    "ablehnen",
    "nur notwendige",
    "nur notwendige cookies",
    "nur erforderliche cookies",
    "nur essenzielle cookies",
    "weiter ohne einwilligung",
    // French.
    "tout refuser",
    "refuser tout",
    "refuser",
    "continuer sans accepter",
    "refuser les cookies",
    "uniquement les cookies nécessaires",
    // Spanish.
    "rechazar todo",
    "rechazar todas",
    "rechazar",
    "rechazar cookies",
    "solo necesarias",
    "solo cookies necesarias",
    // Italian.
    "rifiuta tutto",
    "rifiuta",
    "rifiuta tutti",
    "solo necessari",
    "continua senza accettare",
    // Dutch.
    "alles weigeren",
    "weigeren",
    "alleen noodzakelijke cookies",
    "alleen noodzakelijk",
    // Portuguese.
    "rejeitar tudo",
    "rejeitar todos",
    "rejeitar",
    "recusar",
    "recusar tudo",
    "apenas necessários",
];

/// The names, written plainly, of a consent: what alo never presses.
pub const ACCEPTS: &[&str] = &[
    "accept",
    "accept all",
    "accept all cookies",
    "accept cookies",
    "accept and close",
    "accept and continue",
    "i accept",
    "allow",
    "allow all",
    "allow all cookies",
    "allow cookies",
    "allow selection",
    "agree",
    "i agree",
    "agree and close",
    "agree and proceed",
    "ok",
    "okay",
    "got it",
    "yes, i agree",
    "accept recommended settings",
    "alle akzeptieren",
    "akzeptieren",
    "zustimmen",
    "alle zulassen",
    "tout accepter",
    "accepter",
    "j'accepte",
    "aceptar todo",
    "aceptar",
    "accetta tutto",
    "accetta",
    "alles accepteren",
    "accepteren",
    "aceitar tudo",
    "aceitar",
];

/// The most buttons and links a banner holds: a box holding more is a
/// page, or most of one, which happens to speak of cookies somewhere.
pub const MOST_PRESSABLE: usize = 16;

/// Words a banner's own words hold, one at least, written plainly.
pub const SPEAKS: &[&str] = &[
    "cookie",
    "consent",
    "tracking technolog",
    "personal data",
    "your privacy",
    "value your privacy",
    "einwilligung",
    "datenschutz",
    "consentement",
    "traceurs",
    "consentimiento",
    "consenso",
    "toestemming",
    "consentimento",
];

/// What a page's tree holds of a cookie banner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// No banner: nothing is done.
    Nothing,
    /// A banner, and its most private choice to press.
    Refusal {
        /// The choice, by the id it came back with.
        node: BoxId,
        /// Its name, as the page wrote it.
        name: String,
    },
    /// A banner offering a consent and no refusal alo would press: left
    /// to the person.
    NoRefusal {
        /// The consent it offers, as the page wrote its name.
        offered: String,
    },
}

/// What a choice is, by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The most private one.
    Refusal,
    /// A consent.
    Consent,
    /// Neither.
    Other,
}

/// `name` written plainly, as the lists are.
pub fn plain(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let mut said = words
        .join(" ")
        .to_lowercase()
        .replace(['\u{2019}', '\u{2018}'], "'")
        .replace(" & ", " and ");
    while said.ends_with(['.', '!']) {
        said.pop();
    }
    said.trim().to_owned()
}

/// What the choice named `name` is.
pub fn choice(name: &str) -> Choice {
    let said = plain(name);
    if REFUSALS.contains(&said.as_str()) {
        Choice::Refusal
    } else if ACCEPTS.contains(&said.as_str()) {
        Choice::Consent
    } else {
        Choice::Other
    }
}

/// What `snapshot`, a page's tree, holds of a cookie banner.
pub fn find(snapshot: &Snapshot) -> Found {
    let Some(root) = &snapshot.root else {
        return Found::Nothing;
    };
    let mut choices = Vec::new();
    let mut above = Vec::new();
    gather(root, &mut above, &mut choices);
    let mut offered = None;
    for (node, kind, ancestors) in &choices {
        if !in_a_banner(ancestors) {
            continue;
        }
        let name = node.name.clone().unwrap_or_default();
        match kind {
            Choice::Refusal => {
                return Found::Refusal {
                    node: node.id,
                    name,
                };
            }
            Choice::Consent if offered.is_none() => offered = Some(name),
            Choice::Consent | Choice::Other => {}
        }
    }
    offered.map_or(Found::Nothing, |offered| Found::NoRefusal { offered })
}

/// A choice found, what it is, and the boxes above it, nearest last.
type Gathered<'a> = (&'a SnapshotNode, Choice, Vec<&'a SnapshotNode>);

/// Every button and link under `node` that is a refusal or a consent and
/// can be pressed, in the order a person meets them, with the boxes above
/// each.
fn gather<'a>(
    node: &'a SnapshotNode,
    above: &mut Vec<&'a SnapshotNode>,
    found: &mut Vec<Gathered<'a>>,
) {
    let pressable = matches!(node.role, Role::Known(KnownRole::Button | KnownRole::Link))
        && !node.states.disabled;
    if pressable && let Some(name) = &node.name {
        let kind = choice(name);
        if kind != Choice::Other {
            found.push((node, kind, above.clone()));
        }
    }
    above.push(node);
    for child in &node.children {
        gather(child, above, found);
    }
    above.pop();
}

/// Whether a choice with `ancestors` above it, nearest last, stands in a
/// banner: one of the nearest [`MOST_UP`] of them — never the page itself,
/// the first, nor its `main` — speaks of cookies or consent.
fn in_a_banner(ancestors: &[&SnapshotNode]) -> bool {
    ancestors
        .iter()
        .skip(1)
        .rev()
        .take(MOST_UP)
        .take_while(|node| !matches!(node.role, Role::Known(KnownRole::Main)))
        .find(|node| speaks(node))
        .is_some_and(|banner| pressables_under(banner) <= MOST_PRESSABLE)
}

/// How many buttons and links are in `node` and under it.
fn pressables_under(node: &SnapshotNode) -> usize {
    let own = usize::from(matches!(
        node.role,
        Role::Known(KnownRole::Button | KnownRole::Link)
    ));
    node.children
        .iter()
        .map(pressables_under)
        .fold(own, usize::saturating_add)
}

/// Whether `node`'s words — its own name and every name under it — speak
/// of cookies or consent.
fn speaks(node: &SnapshotNode) -> bool {
    let mut words = String::new();
    said_under(node, &mut words);
    let words = plain(&words);
    SPEAKS.iter().any(|word| words.contains(word))
}

/// Every name in `node` and under it, onto `words`.
fn said_under(node: &SnapshotNode, words: &mut String) {
    if let Some(name) = &node.name {
        words.push(' ');
        words.push_str(name);
    }
    for child in &node.children {
        said_under(child, words);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::States;
    use alo_layout::Rect;

    fn node(id: u64, role: Role, name: Option<&str>, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(usize::try_from(id).unwrap_or(usize::MAX)),
            role,
            name: name.map(str::to_owned),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    fn button(id: u64, name: &str) -> SnapshotNode {
        node(id, Role::Known(KnownRole::Button), Some(name), Vec::new())
    }

    fn text(id: u64, said: &str) -> SnapshotNode {
        node(id, Role::Generic, Some(said), Vec::new())
    }

    fn page(children: Vec<SnapshotNode>) -> Snapshot {
        Snapshot {
            root: Some(node(1, Role::Known(KnownRole::Document), None, children)),
        }
    }

    fn banner(buttons: Vec<SnapshotNode>) -> SnapshotNode {
        let mut children = vec![text(10, "We use cookies to improve your visit.")];
        children.extend(buttons);
        node(9, Role::Known(KnownRole::Dialog), None, children)
    }

    #[test]
    fn the_lists_never_share_a_name_and_no_refusal_is_a_consent_to_everything() {
        for refusal in REFUSALS {
            assert!(!ACCEPTS.contains(refusal), "{refusal}");
            assert_eq!(plain(refusal), *refusal, "written plainly");
            for consent in ["accept all", "allow all", "agree", "accept cookies"] {
                assert!(!refusal.starts_with(consent), "{refusal}");
            }
        }
        for consent in ACCEPTS {
            assert_eq!(plain(consent), *consent, "written plainly");
        }
    }

    #[test]
    fn a_name_is_matched_whole_written_plainly() {
        assert_eq!(choice("Reject All"), Choice::Refusal);
        assert_eq!(choice("  Accept   only necessary "), Choice::Refusal);
        assert_eq!(choice("DENY"), Choice::Refusal);
        assert_eq!(choice("Don\u{2019}t accept"), Choice::Refusal);
        assert_eq!(choice("Reject all & close"), Choice::Refusal);
        assert_eq!(choice("Tout refuser"), Choice::Refusal);
        assert_eq!(choice("Alle ablehnen"), Choice::Refusal);
        assert_eq!(choice("Accept All Cookies"), Choice::Consent);
        assert_eq!(choice("AGREE"), Choice::Consent);
        assert_eq!(choice("OK!"), Choice::Consent);
        // A word of a refusal in a longer name is not one.
        assert_eq!(choice("Reject all, then read our policy"), Choice::Other);
        assert_eq!(choice("Accept all necessary and marketing"), Choice::Other);
        assert_eq!(choice("More options"), Choice::Other);
        assert_eq!(choice("Close"), Choice::Other);
    }

    #[test]
    fn a_banners_refusal_is_found_before_its_consent_whatever_their_order() {
        let found = find(&page(vec![banner(vec![
            button(11, "Allow All"),
            button(12, "Accept only necessary"),
            button(13, "Adjust my preferences"),
        ])]));
        assert_eq!(
            found,
            Found::Refusal {
                node: BoxId::from_wire(12),
                name: "Accept only necessary".into()
            }
        );
    }

    #[test]
    fn a_banner_with_only_a_consent_is_left_and_a_page_without_one_has_none() {
        let found = find(&page(vec![banner(vec![
            button(11, "MORE OPTIONS"),
            button(12, "AGREE"),
        ])]));
        assert_eq!(
            found,
            Found::NoRefusal {
                offered: "AGREE".into()
            }
        );
        // An invitation's *Accept* and *Decline*, in the page's main, with
        // the word *cookies* nowhere near them.
        let invitation = page(vec![
            node(
                2,
                Role::Known(KnownRole::Main),
                None,
                vec![node(
                    3,
                    Role::Known(KnownRole::Paragraph),
                    None,
                    vec![button(4, "Accept"), button(5, "Decline")],
                )],
            ),
            text(6, "Privacy policy. We use no cookies."),
        ]);
        assert_eq!(find(&invitation), Found::Nothing);
        assert_eq!(find(&Snapshot::default()), Found::Nothing);
    }

    #[test]
    fn a_box_holding_most_of_a_page_is_not_a_banner() {
        let mut many: Vec<SnapshotNode> = (20..40)
            .map(|id| {
                node(
                    id,
                    Role::Known(KnownRole::Link),
                    Some("A story"),
                    Vec::new(),
                )
            })
            .collect();
        many.push(button(41, "Decline"));
        many.push(text(42, "Our cookie policy"));
        let wrapper = node(9, Role::Generic, None, many);
        assert_eq!(find(&page(vec![wrapper])), Found::Nothing);
    }

    #[test]
    fn a_disabled_refusal_is_not_pressed() {
        let mut off = button(12, "Reject all");
        off.states.disabled = true;
        assert_eq!(
            find(&page(vec![banner(vec![off, button(11, "Accept all")])])),
            Found::NoRefusal {
                offered: "Accept all".into()
            }
        );
    }

    #[test]
    fn a_banner_further_up_than_it_may_be_looked_for_is_not_one() {
        let mut deep = button(50, "Reject all");
        for id in 0..u64::try_from(MOST_UP).unwrap_or(8) {
            deep = node(60 + id, Role::Generic, None, vec![deep]);
        }
        let far = node(
            9,
            Role::Known(KnownRole::Dialog),
            None,
            vec![text(10, "Cookies."), deep],
        );
        assert_eq!(find(&page(vec![far])), Found::Nothing);
    }
}

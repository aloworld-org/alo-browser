/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an agent outside this browser may ask, and what it can be answered
//! (ADR 0055).
//!
//! Typed, and holding no JSON: [`crate::protocol`] is the one file that reads
//! and writes the bytes, and everything here is what those bytes mean. The
//! contract a caller builds against is `docs/contracts/agent-socket.md`.
//!
//! # Every answer is a kind
//!
//! ADR 0055 § 4. A refusal crosses as a tag and its own fields, never as a
//! sentence a caller has to parse: `verb.rs`'s twelve as they are, and the
//! ones only this interface can meet — a line too long, a tab nobody has
//! open — as [`Refused`]'s own. The English goes beside the kind, for a
//! person reading, and a caller is never asked to read it.
//!
//! # Every bound is ours
//!
//! ADR 0055 § 3, and ADR 0013's rule that *a limit somebody else chooses is
//! not a limit*. A line's length, a name's, an address's, how deeply a
//! request may nest and how deeply a tree may be answered are all here, and
//! each is refused rather than truncated.

use crate::agent_find::FoundNode;
use crate::pages::RenderedBy;
use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_renderer::Snapshot;
use alo_renderer::site::Site;
use core::fmt;

/// The version of the protocol this browser speaks. A request names the one
/// it was written for, and is refused if this browser does not speak it.
pub const VERSION: u64 = 1;

/// The longest line a caller may send, in bytes, its newline included.
pub const MOST_LINE: usize = 64 * 1024;

/// The deepest a request may nest. The deepest a valid one does is two — a
/// target inside the request — so this is room, not a guess.
pub const MOST_NESTING: usize = 4;

/// The longest name a target may carry, and the longest echo a caller may
/// ask to have handed back, in characters.
pub const MOST_NAME: usize = 1024;

/// The longest address a tab may be named by, in characters.
pub const MOST_ADDRESS: usize = 8 * 1024;

/// The deepest tree answered. A page deeper than this is refused rather than
/// cut off: an agent handed half a tree would act on what it believes is the
/// whole of it.
pub const MOST_DEPTH: usize = 256;

/// How many agents may be connected at once.
pub const MOST_CALLERS: usize = 8;

/// What a caller asked to have handed back with its answer, so it can tell
/// its answers apart. Never read by this browser, and never recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Echo {
    /// A whole number.
    Number(u64),
    /// Some text, at most [`MOST_NAME`] characters.
    Text(String),
}

/// One request, as a caller made it.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// What to hand back with the answer, if anything.
    pub echo: Option<Echo>,
    /// What is asked — or why what was sent is not a request.
    pub ask: Result<Ask, Refused>,
}

/// What an agent may ask.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// The address of every open tab, in the order they were opened.
    Tabs,
    /// The tree of the page in the tab at `tab`.
    Read {
        /// The tab's address, as the caller wrote it.
        tab: String,
    },
    /// Which renderer draws the tab at `tab` (ADR 0058 § 6, queue item
    /// 622): a person's question as much as an agent's, asked with
    /// `alo-ask`.
    Renderer {
        /// The tab's address, as the caller wrote it.
        tab: String,
    },
    /// A verb on something in the page in the tab at `tab`, named rather
    /// than pointed at (ADR 0002).
    Act {
        /// The tab's address, as the caller wrote it.
        tab: String,
        /// What to act on.
        target: Target,
        /// What to do.
        verb: Verb,
    },
    /// Open the page at `address` (queue item 956, ADR 0112 § 4): in the
    /// tab at `tab`, which then goes there, or — with no tab named — in a
    /// new one, which in the conversation is a new chat whose card shows
    /// it.
    Open {
        /// The tab it opens in, as the caller wrote it, or none for a new
        /// one.
        tab: Option<String>,
        /// Where to, as the caller wrote it: an `http` or `https` address.
        address: String,
    },
    /// A new tab with nothing in it: in the conversation, a new chat on the
    /// first screen, as *New chat* is (queue item 956).
    NewTab,
    /// The nodes of the page in the tab at `tab` whose names hold `text`
    /// (queue item 956): a person's *Find on page*, answered by name and
    /// never by place ([`crate::agent_find`]).
    Find {
        /// The tab's address, as the caller wrote it.
        tab: String,
        /// The words, at most [`MOST_NAME`] characters, not none.
        text: String,
    },
    /// The page in the tab at `tab` saved as a PDF at `path`, a new file in
    /// a directory the person allowed ([`crate::agent_saves`], queue item
    /// 956).
    PrintPdf {
        /// The tab's address, as the caller wrote it.
        tab: String,
        /// The file to make, as the caller wrote it.
        path: String,
    },
}

impl Ask {
    /// The tab it names, if it names one.
    pub fn tab(&self) -> Option<&str> {
        match self {
            Ask::Tabs | Ask::NewTab | Ask::Open { tab: None, .. } => None,
            Ask::Read { tab }
            | Ask::Renderer { tab }
            | Ask::Act { tab, .. }
            | Ask::Open { tab: Some(tab), .. }
            | Ask::Find { tab, .. }
            | Ask::PrintPdf { tab, .. } => Some(tab),
        }
    }
}

/// What a caller is answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The address of every open tab.
    Tabs(Vec<String>),
    /// The tree, as it was when it was read.
    Tree {
        /// The tab, as the caller named it.
        tab: String,
        /// The tree.
        snapshot: Box<Snapshot>,
    },
    /// The renderer that draws a tab, as the browser process knows it —
    /// never as the page says.
    Renderer {
        /// The tab, as the caller named it.
        tab: String,
        /// Which renderer draws it.
        rendered_by: RenderedBy,
    },
    /// A verb ran, and this is what it did.
    Done {
        /// The tab, as the caller named it.
        tab: String,
        /// What it did, as the page left it.
        outcome: Outcome,
        /// What the page's script said while it ran, as the renderer said it
        /// (`FromRenderer::Acted`).
        issues: Vec<String>,
    },
    /// A page was opened, and this is the address its tab is named by now
    /// — where it went, which a redirect may have changed.
    Opened {
        /// The tab's address now.
        tab: String,
    },
    /// A new tab with nothing in it is open: in the conversation, a new
    /// chat on the first screen, which no tab is named by until a page is
    /// opened in it.
    NewTab,
    /// What a `find` found ([`crate::agent_find`]).
    Found {
        /// The tab, as the caller named it.
        tab: String,
        /// The words, as the caller wrote them.
        text: String,
        /// The nodes whose names hold them, at most
        /// [`crate::agent_find::MOST_FOUND`], in reading order.
        found: Vec<FoundNode>,
        /// How many more there were.
        more: usize,
    },
    /// The page was saved as a PDF.
    Printed {
        /// The tab, as the caller named it.
        tab: String,
        /// The file made, with every link in its directory followed.
        path: String,
        /// How many bytes it holds.
        bytes: usize,
    },
    /// It was refused, and this is why.
    Refused(Refused),
}

/// Why a request was refused, as a kind with its own fields.
///
/// Refusing is a result (ADR 0002), here as in `verb.rs`.
#[derive(Debug, Clone, PartialEq)]
pub enum Refused {
    /// The line was longer than [`MOST_LINE`]. The connection is closed
    /// after it, because where the next line begins cannot be found without
    /// reading as much as the caller cares to send.
    LineTooLong {
        /// The longest line taken.
        most: usize,
    },
    /// The line is not JSON.
    NotJson,
    /// The line nests deeper than [`MOST_NESTING`].
    TooDeep {
        /// The deepest a line may nest.
        most: usize,
    },
    /// The line is JSON, and not an object.
    NotAnObject,
    /// The request names a version this browser does not speak, or none.
    UnknownVersion {
        /// The version it speaks.
        speaks: u64,
    },
    /// A field the request needs is not there.
    MissingField {
        /// Which.
        field: &'static str,
    },
    /// A field this version does not have. Refused rather than ignored, so
    /// that a misspelt field is said rather than silently doing nothing.
    UnknownField {
        /// Which, as the caller wrote it, at most [`MOST_NAME`] characters.
        field: String,
    },
    /// A field holds what it cannot: a number where text goes, an ask or a
    /// verb this version does not have.
    BadField {
        /// Which.
        field: &'static str,
    },
    /// A field holds more than it may.
    FieldTooLong {
        /// Which.
        field: &'static str,
        /// The most it may hold, in characters.
        most: usize,
    },
    /// The tab is named by something that is not an address.
    NotAnAddress {
        /// What it was named by.
        tab: String,
    },
    /// The address is on a site the person who started this browser did not
    /// name for agents ([`crate::agent_sites`]). Judged before any tab is
    /// looked at, so it says nothing about what is open there.
    NotGranted {
        /// The address asked for.
        tab: String,
        /// Its site.
        site: Site,
    },
    /// No open tab is at that address.
    NoSuchTab {
        /// The address asked for.
        tab: String,
    },
    /// More than one tab is at that address (ADR 0055 § 5), and one is not
    /// picked: acting in the wrong tab is worse than acting in none.
    Ambiguous {
        /// The address asked for.
        tab: String,
        /// How many tabs are there.
        count: usize,
    },
    /// The tab is open and nothing is loaded in it.
    NothingLoaded {
        /// The tab.
        tab: String,
    },
    /// The tab's renderer is gone, or could not answer.
    TabGone {
        /// The tab.
        tab: String,
        /// What happened to it, in words, for a person.
        said: String,
    },
    /// The verb was decided on the page and its renderer did not carry it
    /// out, so nothing was done: through Chrome, a node drawn nowhere, or
    /// one whose middle is under something else (queue item 630).
    NotCarriedOut {
        /// The tab.
        tab: String,
        /// Why, in words, for a person.
        said: String,
    },
    /// Text was to be put into a password field of a page shown in the
    /// conversation, where signing in is the person's step (queue item
    /// 704, ADR 0096): whichever agent asked, nothing was typed.
    Password {
        /// The tab.
        tab: String,
        /// The field.
        node: alo_box::BoxId,
    },
    /// A card's number, security code or expiry was to be typed or chosen
    /// on a page shown in the conversation, where paying details are the
    /// person's (queue item 826, ADR 0099): whichever agent asked, and
    /// whatever the page names the field, nothing was done.
    CardDetails {
        /// The tab.
        tab: String,
        /// The field.
        node: alo_box::BoxId,
    },
    /// The page's tree is deeper than [`MOST_DEPTH`].
    TreeTooDeep {
        /// The tab.
        tab: String,
        /// The deepest tree answered.
        most: usize,
    },
    /// [`MOST_CALLERS`] agents are connected already. Said once, and the
    /// connection closed.
    TooManyCallers {
        /// How many may be.
        most: usize,
    },
    /// The address to open is not one: not a URL, or not an `http` or
    /// `https` one, which is all an agent opens.
    NotOpenable {
        /// What it was named by.
        address: String,
    },
    /// What answers this socket does not do this: a tab of this engine's
    /// own is not saved as a PDF, and only the conversation opens a new
    /// tab (queue item 956).
    NotOffered {
        /// The ask, by its name on the wire.
        ask: &'static str,
    },
    /// A page asked to open in a new tab did not open, or another opened
    /// in its place first.
    NotOpened {
        /// The address asked for.
        address: String,
        /// Why, in words, for a person.
        said: String,
    },
    /// The path is not an absolute one of a `.pdf` file with no `.` or
    /// `..` in it.
    NotAPath {
        /// What it was named by.
        path: String,
    },
    /// The path is not in a directory the person allowed a page saved in
    /// (`alo --agent-saves DIR`), or in none that is there. Judged before
    /// any tab is looked at.
    NotAllowedThere {
        /// What it was named by.
        path: String,
    },
    /// Something is at the path already, and nothing is written over it.
    Exists {
        /// What it was named by.
        path: String,
    },
    /// The file could not be written; nothing of it is left.
    NotWritten {
        /// What it was named by.
        path: String,
        /// Why, in words, for a person.
        said: String,
    },
    /// The page refused the verb, with one of `verb.rs`'s refusals as it is.
    Verb {
        /// The tab.
        tab: String,
        /// Why.
        refusal: Refusal,
    },
}

impl Answer {
    /// What it says in English, for a person reading — the line's `said` —
    /// if it says anything: a list of tabs and a tree do not.
    ///
    /// The one wording of it there is. The socket writes it beside the
    /// kind, and the window shows a verb's to the person watching (queue
    /// item 623), so the two can never disagree.
    pub fn said(&self) -> Option<String> {
        match self {
            Answer::Tabs(_) | Answer::Tree { .. } => None,
            Answer::Renderer { rendered_by, .. } => Some(rendered_by.to_string()),
            Answer::Opened { tab } => Some(format!("opened {tab}")),
            Answer::NewTab => Some("a new chat is open, with no page in it".to_owned()),
            Answer::Found {
                text, found, more, ..
            } => Some(match (found.len() + more, found.first()) {
                (0, _) | (_, None) => format!("nothing on the page is named with {text:?}"),
                (1, Some(one)) => format!("{text:?} is in {} {:?}", one.role.as_str(), one.name),
                (count, Some(_)) => format!("{text:?} is in {count} things on the page"),
            }),
            Answer::Printed { tab, path, bytes } => Some(format!(
                "the page at {tab} was saved as a PDF of {bytes} bytes at {path}"
            )),
            Answer::Done { outcome, .. } => Some(outcome.to_string()),
            Answer::Refused(refused) => Some(refused.to_string()),
        }
    }
}

impl Refused {
    /// Its kind, as it crosses: `verb.rs`'s own name for a refusal of the
    /// page's, and this interface's for its own.
    pub fn kind(&self) -> &'static str {
        match self {
            Refused::LineTooLong { .. } => "LineTooLong",
            Refused::NotJson => "NotJson",
            Refused::TooDeep { .. } => "TooDeep",
            Refused::NotAnObject => "NotAnObject",
            Refused::UnknownVersion { .. } => "UnknownVersion",
            Refused::MissingField { .. } => "MissingField",
            Refused::UnknownField { .. } => "UnknownField",
            Refused::BadField { .. } => "BadField",
            Refused::FieldTooLong { .. } => "FieldTooLong",
            Refused::NotAnAddress { .. } => "NotAnAddress",
            Refused::NotGranted { .. } => "NotGranted",
            Refused::NoSuchTab { .. } => "NoSuchTab",
            Refused::Ambiguous { .. } => "Ambiguous",
            Refused::NothingLoaded { .. } => "NothingLoaded",
            Refused::TabGone { .. } => "TabGone",
            Refused::NotCarriedOut { .. } => "NotCarriedOut",
            Refused::Password { .. } => "Password",
            Refused::CardDetails { .. } => "CardDetails",
            Refused::TreeTooDeep { .. } => "TreeTooDeep",
            Refused::TooManyCallers { .. } => "TooManyCallers",
            Refused::NotOpenable { .. } => "NotOpenable",
            Refused::NotOffered { .. } => "NotOffered",
            Refused::NotOpened { .. } => "NotOpened",
            Refused::NotAPath { .. } => "NotAPath",
            Refused::NotAllowedThere { .. } => "NotAllowedThere",
            Refused::Exists { .. } => "Exists",
            Refused::NotWritten { .. } => "NotWritten",
            Refused::Verb { refusal, .. } => refusal_kind(refusal),
        }
    }
}

/// The kind of one of `verb.rs`'s refusals: its own name there.
pub fn refusal_kind(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::NotFound { .. } => "NotFound",
        Refusal::Ambiguous { .. } => "Ambiguous",
        Refusal::NotOperable { .. } => "NotOperable",
        Refusal::Disabled { .. } => "Disabled",
        Refusal::NotAField { .. } => "NotAField",
        Refusal::ReadOnly { .. } => "ReadOnly",
        Refusal::NotADate { .. } => "NotADate",
        Refusal::Unbuilt { .. } => "Unbuilt",
        Refusal::Chooses { .. } => "Chooses",
        Refusal::DoesNotScroll { .. } => "DoesNotScroll",
        Refusal::KeyUnbuilt { .. } => "KeyUnbuilt",
        Refusal::TooLong { .. } => "TooLong",
        Refusal::Unchecked { .. } => "Unchecked",
        Refusal::NotSubmittable { .. } => "NotSubmittable",
    }
}

/// The kind of an outcome, as it crosses: its own name in `verb.rs`.
pub fn outcome_kind(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Activated { .. } => "Activated",
        Outcome::Followed { .. } => "Followed",
        Outcome::TextPut { .. } => "TextPut",
        Outcome::TextCanceled { .. } => "TextCanceled",
        Outcome::Chosen { .. } => "Chosen",
        Outcome::Scrolled { .. } => "Scrolled",
        Outcome::KeyPressed { .. } => "KeyPressed",
        Outcome::Submitted { .. } => "Submitted",
        Outcome::NotSubmitted { .. } => "NotSubmitted",
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::LineTooLong { most } => {
                write!(
                    f,
                    "a line may be at most {most} bytes, and this one was longer"
                )
            }
            Refused::NotJson => f.write_str("the line is not JSON"),
            Refused::TooDeep { most } => {
                write!(
                    f,
                    "a request may nest at most {most} deep, and this one nests deeper"
                )
            }
            Refused::NotAnObject => f.write_str("a request is a JSON object"),
            Refused::UnknownVersion { speaks } => {
                write!(
                    f,
                    "this browser speaks version {speaks} and the request names another, or none"
                )
            }
            Refused::MissingField { field } => write!(f, "the request has no {field:?}"),
            Refused::UnknownField { field } => {
                write!(f, "{field:?} is not a field of this version")
            }
            Refused::BadField { field } => {
                write!(f, "{field:?} holds something it cannot")
            }
            Refused::FieldTooLong { field, most } => {
                write!(f, "{field:?} may hold at most {most} characters")
            }
            Refused::NotAnAddress { tab } => write!(f, "{tab:?} is not an address"),
            Refused::NotGranted { tab, site } => write!(
                f,
                "agents may not reach {site}, where {tab} is: this browser keeps them to the sites its person named"
            ),
            Refused::NoSuchTab { tab } => write!(f, "no tab is open at {tab}"),
            Refused::Ambiguous { tab, count } => {
                write!(
                    f,
                    "{count} tabs are open at {tab}, and one is not picked for you"
                )
            }
            Refused::NothingLoaded { tab } => write!(f, "nothing is loaded in the tab at {tab}"),
            Refused::TabGone { tab, said } => write!(f, "the tab at {tab} cannot answer: {said}"),
            Refused::NotCarriedOut { tab, said } => {
                write!(f, "nothing was done in the tab at {tab}: {said}")
            }
            Refused::Password { tab, node } => write!(
                f,
                "{node} in the tab at {tab} is a password field, and no agent types a password \
                 on a page in the conversation: signing in is the person's step"
            ),
            Refused::CardDetails { tab, node } => write!(
                f,
                "{node} in the tab at {tab} holds a card's details, and no agent types or \
                 chooses a card's number, security code or expiry on a page in the \
                 conversation: paying details are the person's"
            ),
            Refused::TreeTooDeep { tab, most } => write!(
                f,
                "the page at {tab} is more than {most} boxes deep, and half a tree is not answered"
            ),
            Refused::TooManyCallers { most } => {
                write!(f, "{most} agents are connected already")
            }
            Refused::NotOpenable { address } => write!(
                f,
                "{address:?} is not an http or https address, which is all an agent opens"
            ),
            Refused::NotOffered { ask } => {
                write!(f, "this browser does not answer {ask:?} here")
            }
            Refused::NotOpened { address, said } => {
                write!(f, "{address} was not opened: {said}")
            }
            Refused::NotAPath { path } => write!(
                f,
                "{path:?} is not an absolute path of a .pdf file with no . or .. in it"
            ),
            Refused::NotAllowedThere { path } => write!(
                f,
                "{path:?} is not in a directory this browser's person allowed a page saved in"
            ),
            Refused::Exists { path } => {
                write!(
                    f,
                    "something is at {path:?} already, and nothing is written over it"
                )
            }
            Refused::NotWritten { path, said } => {
                write!(f, "{path:?} could not be written: {said}")
            }
            Refused::Verb { refusal, .. } => write!(f, "{refusal}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::BoxId;

    #[test]
    fn a_refusal_of_the_pages_crosses_under_its_own_name() {
        let refused = Refused::Verb {
            tab: "https://example.com/".to_owned(),
            refusal: Refusal::Disabled {
                node: BoxId::from_index_for_tests(3),
            },
        };
        assert_eq!(refused.kind(), "Disabled");
        assert_eq!(refused.to_string(), "box#3 says it is disabled");
    }

    #[test]
    fn two_ambiguities_share_a_kind_and_not_their_fields() {
        let tabs = Refused::Ambiguous {
            tab: "https://example.com/".to_owned(),
            count: 2,
        };
        let names = Refused::Verb {
            tab: "https://example.com/".to_owned(),
            refusal: Refusal::Ambiguous {
                target: Target::Named("Save".to_owned()),
                candidates: Vec::new(),
            },
        };
        assert_eq!(tabs.kind(), names.kind());
        assert_ne!(tabs, names);
    }

    #[test]
    fn only_an_ask_about_a_page_names_a_tab() {
        assert_eq!(Ask::Tabs.tab(), None);
        assert_eq!(
            Ask::Read {
                tab: "about:blank".to_owned()
            }
            .tab(),
            Some("about:blank")
        );
        assert_eq!(
            Ask::Renderer {
                tab: "about:blank".to_owned()
            }
            .tab(),
            Some("about:blank")
        );
        assert_eq!(Ask::NewTab.tab(), None);
        let open = |tab: Option<&str>| Ask::Open {
            tab: tab.map(str::to_owned),
            address: "https://example.com/".to_owned(),
        };
        assert_eq!(open(None).tab(), None);
        assert_eq!(open(Some("about:blank")).tab(), Some("about:blank"));
    }

    #[test]
    fn what_was_found_is_said_by_how_many_and_where() {
        use alo_box::{KnownRole, Role};
        let found = |found: Vec<FoundNode>, more| Answer::Found {
            tab: "https://example.com/".to_owned(),
            text: "next".to_owned(),
            found,
            more,
        };
        let link = FoundNode {
            node: BoxId::from_index_for_tests(4),
            role: Role::Known(KnownRole::Link),
            name: "Next page".to_owned(),
        };
        assert_eq!(
            found(Vec::new(), 0).said().as_deref(),
            Some("nothing on the page is named with \"next\"")
        );
        assert_eq!(
            found(vec![link.clone()], 0).said().as_deref(),
            Some("\"next\" is in link \"Next page\"")
        );
        assert_eq!(
            found(vec![link], 2).said().as_deref(),
            Some("\"next\" is in 3 things on the page")
        );
    }
}

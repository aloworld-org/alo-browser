/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 307's closing conditions, each asserted on the table's own
//! answers, its record, and the files on disk.
//!
//! A restart here is a real one: the [`Table`] is dropped and a second one is
//! opened on the same directory, which reads everything back from the file
//! the first one left.

#![cfg(unix)]

use alo_grants::{
    Answer, Ask, Capability, Decision, Ending, Happened, Kept, Prompted, Refusal, THIRTY_DAYS,
    Table,
};
use alo_net::Partition;
use alo_net::cause::{DocumentId, Identities};
use alo_net::{Cause, deed::Link};
use alo_storage::StorageKey;
use alo_url::Origin;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A directory of this test's own. Named after the caller so two tests never
/// share one.
fn somewhere(called: &str) -> PathBuf {
    let place = std::env::temp_dir().join(format!(
        "alo-grants-{}-{called}",
        std::process::id().wrapping_mul(2_654_435_761)
    ));
    let _ = fs::remove_dir_all(&place);
    place
}

fn files_in(place: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(place)
        .map(|listing| {
            listing
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn key(document: &str, top_level: &str) -> Option<StorageKey> {
    let document = alo_url::parse(document).ok()?;
    let top_level = alo_url::parse(top_level).ok()?;
    StorageKey::of(&Origin::of(&document), &Partition::of(&top_level))
}

fn alone(host: &str) -> Option<StorageKey> {
    let at = format!("https://{host}/");
    key(&at, &at)
}

fn day(n: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_790_000_000) + Duration::from_secs(n * 86_400)
}

/// What a test needs to make asks: identities minted the way the browser
/// process mints them.
struct Browser {
    minting: Identities,
}

impl Browser {
    fn new() -> Self {
        Self {
            minting: Identities::default(),
        }
    }

    fn document(&mut self) -> DocumentId {
        self.minting.a_document()
    }

    fn person(&mut self) -> Cause {
        Cause::Person {
            tab: self.minting.a_tab(),
        }
    }

    fn agent_in(&mut self, document: DocumentId) -> Cause {
        Cause::Agent {
            action: self.minting.an_action(),
            document,
        }
    }

    /// A secure, top-level page asking with a gesture: the ask that prompts.
    fn ask(key: &StorageKey, capability: Capability, document: DocumentId) -> Ask {
        Ask {
            key: Some(key.clone()),
            capability,
            document,
            secure: true,
            delegated: true,
            activated: true,
            cause: Cause::Document { document },
        }
    }
}

fn prompted(decision: Decision) -> Option<Prompted> {
    match decision {
        Decision::Prompt(prompted) => Some(prompted),
        Decision::Granted | Decision::Refused(_) => None,
    }
}

/// Grant on this site, by a person, for a fresh document, and return it.
fn granted_on_the_site(
    table: &mut Table,
    browser: &mut Browser,
    key: &StorageKey,
    capability: Capability,
    now: SystemTime,
) -> Option<DocumentId> {
    let document = browser.document();
    let asked = prompted(table.ask(Browser::ask(key, capability, document), now))?;
    let person = browser.person();
    table.answer(asked, Answer::OnThisSite, &person, now).ok()?;
    Some(document)
}

// --- § 2 and § 3: refused without a prompt ------------------------------------

fn refusal_of(ask: Ask) -> Option<Refusal> {
    let mut table = Table::for_the_session();
    match table.ask(ask, day(0)) {
        Decision::Refused(refusal) => Some(refusal),
        Decision::Granted | Decision::Prompt(_) => None,
    }
}

#[test]
fn an_opaque_origin_is_refused_without_a_prompt() {
    let mut browser = Browser::new();
    let document = browser.document();
    for opaque in [
        "file:///Users/someone/page.html",
        "data:text/html,<p>hi",
        "about:blank",
    ] {
        let ask = Ask {
            key: key(opaque, opaque),
            ..Browser::ask(
                &alone("meet.example").expect("a key"),
                Capability::Camera,
                document,
            )
        };
        assert_eq!(ask.key, None, "{opaque} had a key");
        assert_eq!(refusal_of(ask), Some(Refusal::OpaqueOrigin));
    }
}

#[test]
fn an_insecure_context_is_refused_without_a_prompt() {
    let mut browser = Browser::new();
    let document = browser.document();
    let insecure = key("http://meet.example/", "http://meet.example/").expect("a key");
    let ask = Ask {
        secure: false,
        ..Browser::ask(&insecure, Capability::Location, document)
    };
    assert_eq!(refusal_of(ask), Some(Refusal::InsecureContext));
}

#[test]
fn a_frame_its_embedder_did_not_allow_is_refused_without_a_prompt() {
    let mut browser = Browser::new();
    let document = browser.document();
    let widget = key("https://meet.example/", "https://news.example/").expect("a key");
    let ask = Ask {
        delegated: false,
        ..Browser::ask(&widget, Capability::Camera, document)
    };
    assert_eq!(refusal_of(ask), Some(Refusal::NotDelegated));
}

#[test]
fn an_ask_nobody_was_using_the_page_for_is_refused_without_a_prompt_and_looks_unasked() {
    let mut browser = Browser::new();
    let document = browser.document();
    let ask = Ask {
        activated: false,
        ..Browser::ask(
            &alone("news.example").expect("a key"),
            Capability::Notifications,
            document,
        )
    };
    let refusal = refusal_of(ask).expect("a refusal without a prompt");
    assert_eq!(refusal, Refusal::NoGesture);
    assert!(refusal.looks_unasked());
}

#[test]
fn a_document_that_dismissed_a_prompt_is_refused_without_one_until_it_is_gone() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let key = alone("news.example").expect("a key");
    let document = browser.document();
    let ask = || Browser::ask(&key, Capability::Notifications, document);
    let person = browser.person();
    let asked = prompted(table.ask(ask(), day(0))).expect("a prompt");
    table
        .dismiss(asked, &person, day(0))
        .expect("a person dismissed it");
    assert_eq!(
        table.ask(ask(), day(0)),
        Decision::Refused(Refusal::RefusedThisDocument)
    );
    assert_eq!(table.rows().count(), 0, "a dismissal was remembered");
    // Another document of the same key may ask; this one, once it is gone,
    // is gone.
    let another = browser.document();
    assert!(matches!(
        table.ask(
            Browser::ask(&key, Capability::Notifications, another),
            day(0)
        ),
        Decision::Prompt(_)
    ));
    table.document_gone(document, day(0));
    assert!(matches!(table.ask(ask(), day(0)), Decision::Prompt(_)));
}

#[test]
fn a_remembered_refusal_and_a_refusal_everywhere_are_refused_without_a_prompt() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let news = alone("news.example").expect("a key");
    let first = browser.document();
    let person = browser.person();
    let asked = prompted(table.ask(
        Browser::ask(&news, Capability::Notifications, first),
        day(0),
    ))
    .expect("a prompt");
    table
        .answer(asked, Answer::DoNotAllow, &person, day(0))
        .expect("answered");
    let later = browser.document();
    assert_eq!(
        table.ask(
            Browser::ask(&news, Capability::Notifications, later),
            day(1)
        ),
        Decision::Refused(Refusal::RememberedRefusal)
    );

    table
        .refuse_everywhere(Capability::Location, &person, day(1))
        .expect("a person refused it");
    assert_eq!(
        table.ask(
            Browser::ask(
                &alone("maps.example").expect("a key"),
                Capability::Location,
                later
            ),
            day(1)
        ),
        Decision::Refused(Refusal::RefusedEverywhere)
    );
    table
        .ask_again_everywhere(Capability::Location, &person)
        .expect("a person took it back");
    assert!(matches!(
        table.ask(
            Browser::ask(
                &alone("maps.example").expect("a key"),
                Capability::Location,
                later
            ),
            day(1)
        ),
        Decision::Prompt(_)
    ));
}

/// § 2: a grant to the top-level site does not reach a frame it embeds, and a
/// grant to an origin alone does not reach it embedded elsewhere.
#[test]
fn a_grant_belongs_to_one_origin_under_one_top_level_site() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let news = alone("news.example").expect("a key");
    let meet = alone("meet.example").expect("a key");
    let meet_in_news = key("https://meet.example/", "https://news.example/").expect("a key");
    granted_on_the_site(&mut table, &mut browser, &news, Capability::Camera, day(0))
        .expect("granted");
    granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
        .expect("granted");
    let frame = browser.document();
    assert!(!table.allows(&meet_in_news, Capability::Camera, frame, day(0)));
    assert!(matches!(
        table.ask(
            Browser::ask(&meet_in_news, Capability::Camera, frame),
            day(0)
        ),
        Decision::Prompt(_)
    ));
}

// --- § 4: only a person ---------------------------------------------------------

#[test]
fn an_agent_cannot_answer_dismiss_or_revoke_and_the_prompt_says_it_was_acting() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let document = browser.document();
    let agent = browser.agent_in(document);
    let ask = Ask {
        cause: agent.clone(),
        ..Browser::ask(&meet, Capability::Camera, document)
    };
    let asked = prompted(table.ask(ask, day(0))).expect("a prompt");
    assert!(asked.agent_was_acting());

    let page = Cause::Document { document };
    let refused = table
        .answer(asked, Answer::OnThisSite, &agent, day(0))
        .expect_err("an agent answered");
    let asked = *refused.still_waiting.expect("the prompt is handed back");
    let refused = table
        .dismiss(asked, &page, day(0))
        .expect_err("a page dismissed its own prompt");
    let asked = *refused.still_waiting.expect("the prompt is handed back");
    assert!(!table.allows(&meet, Capability::Camera, document, day(0)));

    let person = browser.person();
    table
        .answer(asked, Answer::OnThisSite, &person, day(0))
        .expect("a person answered");
    assert!(
        table
            .revoke(&meet, Capability::Camera, &agent, day(0))
            .is_err()
    );
    assert!(
        table
            .refuse_everywhere(Capability::Camera, &agent, day(0))
            .is_err()
    );
    assert!(table.allows(&meet, Capability::Camera, document, day(0)));
}

// --- § 5: every grant ends ------------------------------------------------------

#[test]
fn allow_while_this_page_is_open_ends_with_its_document() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let document = browser.document();
    let person = browser.person();
    let asked = prompted(table.ask(
        Browser::ask(&meet, Capability::Microphone, document),
        day(0),
    ))
    .expect("a prompt");
    table
        .answer(asked, Answer::WhileThisPageIsOpen, &person, day(0))
        .expect("answered");
    assert!(table.allows(&meet, Capability::Microphone, document, day(0)));
    let another = browser.document();
    assert!(
        !table.allows(&meet, Capability::Microphone, another, day(0)),
        "a grant for one page reached another of the same site"
    );
    assert_eq!(
        table.rows().count(),
        0,
        "an allow while open was remembered"
    );

    table
        .begin_using(
            &meet,
            Capability::Microphone,
            document,
            &Cause::Document { document },
            day(0),
        )
        .expect("granted");
    assert_eq!(table.in_use(Capability::Microphone), vec![document]);
    assert_eq!(
        table.document_gone(document, day(0)),
        vec![Capability::Microphone]
    );
    assert!(!table.allows(&meet, Capability::Microphone, document, day(0)));
    assert!(table.in_use(Capability::Microphone).is_empty());
    let last = table
        .history(&meet)
        .and_then(|history| history.entries().last().cloned())
        .expect("a history");
    assert_eq!(
        last.happened,
        Happened::Ended {
            ending: Ending::PageClosed
        }
    );
}

#[test]
fn allow_on_this_site_ends_thirty_days_after_the_persons_last_visit_and_use_does_not_extend_it() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let document = granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
        .expect("granted");

    // The site uses it every day, and a page and an agent load it: none of
    // that is the person opening it.
    for n in 1..30 {
        table
            .begin_using(
                &meet,
                Capability::Camera,
                document,
                &Cause::Document { document },
                day(n),
            )
            .expect("granted");
        table.stop_using(document, Capability::Camera, day(n));
        table.person_opened(&meet, &Cause::Document { document }, day(n));
        let agent = browser.agent_in(document);
        table.person_opened(&meet, &agent, day(n));
    }
    let row = table.rows().next().expect("a row").clone();
    assert_eq!((row.visited, row.last_used), (day(0), Some(day(29))));
    assert!(table.allows(&meet, Capability::Camera, document, day(29)));
    assert!(table.allows(
        &meet,
        Capability::Camera,
        document,
        day(30) - Duration::from_secs(1)
    ));
    assert!(!table.allows(&meet, Capability::Camera, document, day(30)));
    assert_eq!(day(0) + THIRTY_DAYS, day(30));
}

#[test]
fn the_persons_own_visit_counts_the_thirty_days_again() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let document = granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
        .expect("granted");
    let person = browser.person();
    table.person_opened(&meet, &person, day(20));
    assert!(table.allows(&meet, Capability::Camera, document, day(49)));
    assert!(!table.allows(&meet, Capability::Camera, document, day(50)));

    assert_eq!(table.expire(day(50)), Vec::new());
    assert_eq!(table.rows().count(), 0);
    let last = table
        .history(&meet)
        .and_then(|history| history.entries().last().cloned())
        .expect("a history");
    assert_eq!(
        (last.at, last.happened),
        (
            day(50),
            Happened::Ended {
                ending: Ending::ThirtyDays
            }
        )
    );
    // Ended, so the site asks again.
    let later = browser.document();
    assert!(matches!(
        table.ask(Browser::ask(&meet, Capability::Camera, later), day(50)),
        Decision::Prompt(_)
    ));
}

#[test]
fn a_clock_set_backwards_expires_a_grant() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let document =
        granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(10))
            .expect("granted");
    assert!(table.allows(&meet, Capability::Camera, document, day(10)));
    assert!(!table.allows(&meet, Capability::Camera, document, day(9)));
    // A person's visit with the clock behind does not rescue it either.
    let person = browser.person();
    table.person_opened(&meet, &person, day(9));
    assert!(!table.allows(&meet, Capability::Camera, document, day(9)));
    table.expire(day(9));
    let last = table
        .history(&meet)
        .and_then(|history| history.entries().last().cloned())
        .expect("a history");
    assert_eq!(
        last.happened,
        Happened::Ended {
            ending: Ending::ClockWentBack
        }
    );
    assert_eq!(table.rows().count(), 0);
}

// --- § 7: revoked at once ---------------------------------------------------------

#[test]
fn a_revoked_grant_answers_no_on_the_next_check_and_its_use_stops_in_the_same_act() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let document = granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
        .expect("granted");
    table
        .begin_using(
            &meet,
            Capability::Camera,
            document,
            &Cause::Document { document },
            day(0),
        )
        .expect("granted");
    let person = browser.person();
    let stopped = table
        .revoke(&meet, Capability::Camera, &person, day(1))
        .expect("a person revoked it");
    assert_eq!(stopped, vec![document]);
    assert!(!table.allows(&meet, Capability::Camera, document, day(1)));
    assert!(table.in_use(Capability::Camera).is_empty());
    assert!(
        table
            .begin_using(
                &meet,
                Capability::Camera,
                document,
                &Cause::Document { document },
                day(1)
            )
            .is_err()
    );
    let tail: Vec<Happened> = table
        .history(&meet)
        .expect("a history")
        .entries()
        .rev()
        .take(2)
        .map(|entry| entry.happened.clone())
        .collect();
    assert_eq!(
        tail,
        vec![
            Happened::UseEnded {
                by: Link::of(&Cause::Document { document })
            },
            Happened::Revoked {
                by: Link::of(&person)
            },
        ]
    );
}

#[test]
fn refusing_a_capability_everywhere_revokes_every_grant_of_it() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let one = alone("one.example").expect("a key");
    let two = alone("two.example").expect("a key");
    let a = granted_on_the_site(&mut table, &mut browser, &one, Capability::Location, day(0))
        .expect("granted");
    let b = granted_on_the_site(&mut table, &mut browser, &two, Capability::Location, day(0))
        .expect("granted");
    granted_on_the_site(
        &mut table,
        &mut browser,
        &two,
        Capability::Notifications,
        day(0),
    )
    .expect("granted");
    for (key, document) in [(&one, a), (&two, b)] {
        table
            .begin_using(
                key,
                Capability::Location,
                document,
                &Cause::Document { document },
                day(0),
            )
            .expect("granted");
    }
    let person = browser.person();
    let mut stopped = table
        .refuse_everywhere(Capability::Location, &person, day(0))
        .expect("a person refused it");
    stopped.sort();
    assert_eq!(stopped, vec![a, b]);
    let left: Vec<Capability> = table.rows().map(|row| row.capability).collect();
    assert_eq!(left, vec![Capability::Notifications]);
}

#[test]
fn clearing_a_site_clears_its_grants_refusals_and_history_under_every_top_level_site() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let meet = alone("meet.example").expect("a key");
    let meet_in_news = key("https://meet.example/", "https://news.example/").expect("a key");
    let news = alone("news.example").expect("a key");
    let document = granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
        .expect("granted");
    granted_on_the_site(
        &mut table,
        &mut browser,
        &meet_in_news,
        Capability::Camera,
        day(0),
    )
    .expect("granted");
    granted_on_the_site(&mut table, &mut browser, &news, Capability::Camera, day(0))
        .expect("granted");
    table
        .begin_using(
            &meet,
            Capability::Camera,
            document,
            &Cause::Document { document },
            day(0),
        )
        .expect("granted");

    assert_eq!(table.clear_site("meet.example"), vec![document]);
    let left: Vec<&StorageKey> = table.rows().map(|row| &row.key).collect();
    assert_eq!(left, vec![&news]);
    assert!(table.history(&meet).is_none() && table.history(&meet_in_news).is_none());
    assert!(table.history(&news).is_some());
}

// --- § 8: recorded, bounded, and on a disk ----------------------------------------------

#[test]
fn the_record_keeps_sixty_four_entries_per_key_and_counts_what_it_dropped() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    let news = alone("news.example").expect("a key");
    for _ in 0..50 {
        let document = browser.document();
        let ask = Ask {
            activated: false,
            ..Browser::ask(&news, Capability::Notifications, document)
        };
        table.ask(ask, day(0));
    }
    // Each refused ask is two entries: the ask, and the rule that refused it.
    let history = table.history(&news).expect("a history");
    assert_eq!((history.entries().len(), history.dropped()), (64, 36));
    assert!(
        history
            .entries()
            .all(|entry| entry.capability == Capability::Notifications)
    );
}

#[test]
fn an_answer_on_this_site_survives_a_restart_and_an_allow_while_open_does_not() {
    let place = somewhere("restart");
    let mut browser = Browser::new();
    let meet = alone("meet.example").expect("a key");
    let news = alone("news.example").expect("a key");
    let (document, open) = {
        let mut table = Table::at(&place).expect("a table");
        let document =
            granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
                .expect("granted");
        let open = browser.document();
        let person = browser.person();
        let asked = prompted(table.ask(Browser::ask(&news, Capability::Location, open), day(0)))
            .expect("a prompt");
        table
            .answer(asked, Answer::WhileThisPageIsOpen, &person, day(0))
            .expect("answered");
        table
            .refuse_everywhere(Capability::Notifications, &person, day(0))
            .expect("refused");
        assert_eq!(table.unwritten(), None);
        (document, open)
    };
    assert_eq!(files_in(&place), vec!["table".to_owned()]);

    let table = Table::at(&place).expect("the same table");
    assert_eq!(table.set_aside(), None);
    let rows: Vec<(&StorageKey, Capability, Kept)> = table
        .rows()
        .map(|row| (&row.key, row.capability, row.kept))
        .collect();
    assert_eq!(rows, vec![(&meet, Capability::Camera, Kept::Allowed)]);
    assert!(table.allows(&meet, Capability::Camera, document, day(1)));
    assert!(!table.allows(&news, Capability::Location, open, day(0)));
    assert!(
        table
            .refused_everywhere()
            .contains(&Capability::Notifications)
    );
    assert_eq!(
        table.history(&meet).map(|history| history.entries().len()),
        Some(2),
        "the ask and the answer"
    );
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn a_damaged_table_grants_nothing_and_is_set_aside_never_deleted() {
    let place = somewhere("damaged");
    let mut browser = Browser::new();
    let meet = alone("meet.example").expect("a key");
    let document = {
        let mut table = Table::at(&place).expect("a table");
        granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
            .expect("granted")
    };
    let whole = fs::read(place.join("table")).expect("the table");

    let mut damage: Vec<(String, Vec<u8>)> = Vec::new();
    for cut in 0..whole.len() {
        damage.push((format!("cut to {cut}"), whole[..cut].to_vec()));
    }
    for at in 0..whole.len() {
        let mut flipped = whole.clone();
        flipped[at] ^= 0xff;
        damage.push((format!("byte {at} flipped"), flipped));
    }
    for claimed in [u64::MAX, 1 << 40] {
        let mut lying = whole.clone();
        lying[10..18].copy_from_slice(&claimed.to_be_bytes());
        damage.push((format!("a count of {claimed}"), lying));
    }
    damage.push(("not a table".to_owned(), b"\x89PNG\r\n\x1a\nrest".to_vec()));
    damage.push(("an empty file".to_owned(), Vec::new()));

    for (n, (what, bytes)) in damage.iter().enumerate() {
        fs::write(place.join("table"), bytes).expect("damage");
        let table = Table::at(&place).expect("a table");
        assert!(table.set_aside().is_some(), "{what} was not set aside");
        assert!(
            !table.allows(&meet, Capability::Camera, document, day(0)),
            "{what} granted the camera"
        );
        assert_eq!(table.rows().count(), 0, "{what} was believed in part");
        assert!(!place.join("table").exists());
        assert_eq!(
            fs::read(place.join(format!("table.aside.{n}"))).expect("kept"),
            *bytes,
            "{what} was not kept as it was"
        );
    }
    let _ = fs::remove_dir_all(&place);
}

/// A private session's table is never given a directory, so there is nowhere
/// for a file to go: the same structure as `alo-storage`'s session store.
#[test]
fn a_private_session_leaves_no_file() {
    let mut browser = Browser::new();
    let mut table = Table::for_the_session();
    assert_eq!(table.directory(), None);
    let meet = alone("meet.example").expect("a key");
    let document = granted_on_the_site(&mut table, &mut browser, &meet, Capability::Camera, day(0))
        .expect("granted");
    let person = browser.person();
    table
        .refuse_everywhere(Capability::Location, &person, day(0))
        .expect("refused");
    assert!(table.allows(&meet, Capability::Camera, document, day(0)));
    assert_eq!((table.directory(), table.unwritten()), (None, None));
    drop(table);
    let fresh = Table::for_the_session();
    assert_eq!(fresh.rows().count(), 0);
    assert!(fresh.refused_everywhere().is_empty());
}

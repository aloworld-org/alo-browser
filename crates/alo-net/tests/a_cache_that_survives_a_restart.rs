//! What a disk cache keeps, and what it must never be asked to keep.
//!
//! Queue item 155 closes when *"a cache survives a restart, and a response that
//! must not outlive the session does not"*, and both halves are here. The
//! restart is real: the `Cache` and its `Disk` are dropped, and a second pair is
//! opened on the same directory the way a second run of the browser would open
//! it.
//!
//! The three things being asserted are ADR 0011's three, in its order:
//!
//! 1. **The key carries the top-level site.** An entry stored while somebody was
//!    looking at one site is not served while they are looking at another —
//!    across a restart as well as within a session, because an identifier that
//!    survives a restart is worth more to whoever set it.
//! 2. **What must not outlive the session is never written.** Not written and
//!    deleted: the file is never there to be recovered.
//! 3. **What comes back off the disk is untrusted.** Rubbish in the directory is
//!    a miss and never a failure to load, whatever the rubbish is.
//!
//! Every case names its own directory under the machine's temporary place, so
//! nothing here touches a real profile and two tests never share a disk.
#![cfg(unix)]

use alo_net::cache::{Answer, Cache};
use alo_net::disk::{self, Disk};
use alo_net::{Headers, Partition, Request, Response, Status};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A fixed moment to measure everything from: 2023-11-14 22:13:20 GMT.
const START: u64 = 1_700_000_000;

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

fn inside(site: &str) -> Partition {
    Partition::of(&url(site))
}

fn asking(target: &str) -> Request {
    Request::get(url(target))
}

/// A response that is good for an hour, plus whatever else a case needs.
fn answered(target: &str, headers: &[(&str, &str)]) -> Response {
    let mut carried = Headers::new();
    carried.add("Date", alo_net::httpdate::format(at(START)));
    carried.add("Cache-Control", "max-age=3600");
    for (name, value) in headers {
        carried.add(*name, *value);
    }
    Response {
        url: url(target),
        status: Status(200),
        headers: carried,
        body: b"the stored body".to_vec(),
    }
}

/// A directory of this run's own, emptied before it is used.
fn somewhere(called: &str) -> PathBuf {
    let place = std::env::temp_dir().join(format!("alo-restart-{}-{called}", std::process::id()));
    let _ = fs::remove_dir_all(&place);
    place
}

/// A cache with a disk at this place. What a profile that persists has.
fn cache_on(place: &Path) -> Result<Cache, String> {
    Ok(Cache::new().kept_on(Disk::at(place)?))
}

fn is_hit(answer: &Answer) -> bool {
    matches!(answer, Answer::Stored(_))
}

// --- It survives a restart -----------------------------------------------------

#[test]
fn what_was_stored_before_the_restart_is_served_after_it() {
    let place = somewhere("survives");
    let site = inside("https://example.com/");
    let request = asking("https://example.com/a");

    {
        let mut cache = cache_on(&place).expect("a cache directory");
        assert!(cache.keep(
            &request,
            &site,
            &answered("https://example.com/a", &[]),
            at(START),
            at(START)
        ));
        assert!(is_hit(&cache.answer(&request, &site, at(START + 60))));
    }

    // The browser starts again. Nothing is in memory; the directory is all
    // there is.
    let mut restarted = cache_on(&place).expect("a cache directory");
    assert_eq!(restarted.len(), 0, "nothing should be in memory yet");
    let answer = restarted.answer(&request, &site, at(START + 60));
    assert!(
        is_hit(&answer),
        "the response did not survive the restart: {answer:?}"
    );
    match answer {
        Answer::Stored(response) => {
            assert_eq!(response.body, b"the stored body");
            assert_eq!(response.url.serialised, "https://example.com/a");
        }
        other => panic!("a hit was expected and this came back: {other:?}"),
    }
    assert_eq!(restarted.counts().0, 1, "the hit was not counted as one");
    let _ = fs::remove_dir_all(&place);
}

/// A response that went to a disk is served under exactly the rules it would
/// have been served under from memory. ADR 0011 changes what is *kept*, never
/// what may be *reused*.
#[test]
fn a_restart_does_not_make_a_stale_response_fresh() {
    let place = somewhere("stale");
    let site = inside("https://example.com/");
    let request = asking("https://example.com/a");
    {
        let mut cache = cache_on(&place).expect("a cache directory");
        assert!(cache.keep(
            &request,
            &site,
            &answered("https://example.com/a", &[("ETag", "\"v1\"")]),
            at(START),
            at(START)
        ));
    }
    let mut restarted = cache_on(&place).expect("a cache directory");
    assert!(
        is_hit(&restarted.answer(&request, &site, at(START + 3599))),
        "one second before it expires"
    );

    let mut later = cache_on(&place).expect("a cache directory");
    assert!(
        matches!(
            later.answer(&request, &site, at(START + 3601)),
            Answer::Revalidate { .. }
        ),
        "an hour on a disk made a stale response fresh"
    );
    let _ = fs::remove_dir_all(&place);
}

/// The `Vary` contract survives the disk too. It is stored as the request
/// header values the response was chosen by, and a reader who would have been
/// answered differently is a miss.
#[test]
fn a_french_page_is_not_served_to_a_german_reader_after_a_restart() {
    let place = somewhere("vary");
    let site = inside("https://example.com/");
    let mut french = asking("https://example.com/a");
    french.headers.add("Accept-Language", "fr");
    {
        let mut cache = cache_on(&place).expect("a cache directory");
        assert!(cache.keep(
            &french,
            &site,
            &answered("https://example.com/a", &[("Vary", "Accept-Language")]),
            at(START),
            at(START)
        ));
    }
    let mut restarted = cache_on(&place).expect("a cache directory");
    let mut german = asking("https://example.com/a");
    german.headers.add("Accept-Language", "de");
    assert_eq!(
        restarted.answer(&german, &site, at(START + 60)),
        Answer::Fetch,
        "one reader was served another reader's page"
    );
    assert!(
        is_hit(&restarted.answer(&french, &site, at(START + 60))),
        "and the reader it was for was refused it"
    );
    let _ = fs::remove_dir_all(&place);
}

// --- The key carries the top-level site ----------------------------------------

/// ADR 0011 section 1. A shared cache answers *have you been somewhere that
/// loads this* for any site that thinks to time a load, and the entry itself is
/// an identifier that survives clearing cookies — so the site is in the key,
/// and a restart does not launder it.
#[test]
fn an_entry_stored_inside_one_site_is_not_served_inside_another() {
    let place = somewhere("partitioned");
    let news = inside("https://news.example/");
    let shop = inside("https://shop.example/");
    // The same third-party script, asked for inside two different sites.
    let request = asking("https://ads.example/tracker.js");

    {
        let mut cache = cache_on(&place).expect("a cache directory");
        assert!(cache.keep(
            &request,
            &news,
            &answered("https://ads.example/tracker.js", &[]),
            at(START),
            at(START)
        ));
        assert_eq!(
            cache.answer(&request, &shop, at(START + 60)),
            Answer::Fetch,
            "one site learned what had been loaded inside another"
        );
    }

    let mut restarted = cache_on(&place).expect("a cache directory");
    assert_eq!(
        restarted.answer(&request, &shop, at(START + 60)),
        Answer::Fetch,
        "the join survived a restart, which is worse than making it once"
    );
    assert!(
        is_hit(&restarted.answer(&request, &news, at(START + 60))),
        "the site it was stored inside was refused its own entry"
    );
    let _ = fs::remove_dir_all(&place);
}

/// Where that boundary actually is, since queue item 156: the registrable
/// domain. Two subdomains of one organisation share the cache — a shared library
/// fetched at `www.` is not fetched again at the bare name — and two
/// organisations under one public suffix do not, which is the half a comparison
/// of host strings would have got wrong.
#[test]
fn the_boundary_is_the_registrable_domain_rather_than_the_host() {
    let place = somewhere("registrable");
    let request = asking("https://cdn.example/library.js");

    let mut cache = cache_on(&place).expect("a cache directory");
    assert!(cache.keep(
        &request,
        &inside("https://www.bbc.co.uk/"),
        &answered("https://cdn.example/library.js", &[]),
        at(START),
        at(START)
    ));
    assert!(
        is_hit(&cache.answer(&request, &inside("https://bbc.co.uk/"), at(START + 60))),
        "one organisation's two subdomains were two sites"
    );
    assert_eq!(
        cache.answer(&request, &inside("https://www.gov.co.uk/"), at(START + 60)),
        Answer::Fetch,
        "a suffix two organisations share was read as one site"
    );
    drop(cache);
    let _ = fs::remove_dir_all(&place);
}

// --- What must not outlive the session -----------------------------------------

/// ADR 0011 section 2, one case per line. Each of these is reusable from memory
/// for as long as the process lives, and **never written**: after a restart it
/// is gone, and there is no file to recover because there never was one.
#[test]
fn the_table_of_what_is_never_written() {
    /// What it is, the response's extra headers, and what the request carried.
    type Row = (
        &'static str,
        &'static [(&'static str, &'static str)],
        &'static [(&'static str, &'static str)],
    );

    let table: &[Row] = &[
        (
            "a response for one person only",
            &[("Cache-Control", "private, max-age=3600")],
            &[],
        ),
        (
            "a response carrying a session token",
            &[("Set-Cookie", "session=abc; Path=/")],
            &[],
        ),
        (
            "a page behind a password",
            &[],
            &[("Authorization", "Bearer a-token")],
        ),
        (
            "a body that is not the length it was said to be",
            &[("Content-Length", "999999")],
            &[],
        ),
    ];

    for (what, extra, sent) in table {
        let place = somewhere(&format!("never-{}", what.replace(' ', "-")));
        let site = inside("https://example.com/");
        let mut request = asking("https://example.com/a");
        for (name, value) in *sent {
            request.headers.add(*name, *value);
        }
        let response = answered("https://example.com/a", extra);

        {
            let mut cache = cache_on(&place).expect("a cache directory");
            assert!(
                cache.keep(&request, &site, &response, at(START), at(START)),
                "{what} was not even kept in memory, where it costs nothing"
            );
            assert!(
                is_hit(&cache.answer(&request, &site, at(START + 60))),
                "{what} is reusable for as long as the process lives"
            );
            assert_eq!(
                cache.disk().map(Disk::len),
                Some(0),
                "{what} was written to a disk"
            );
        }

        let mut restarted = cache_on(&place).expect("a cache directory");
        assert_eq!(
            restarted.answer(&request, &site, at(START + 60)),
            Answer::Fetch,
            "{what} outlived the session"
        );
        assert_eq!(
            fs::read_dir(&place)
                .expect("the directory")
                .flatten()
                .count(),
            0,
            "{what} left a file behind"
        );
        let _ = fs::remove_dir_all(&place);
    }
}

/// `file:` is already on the disk and copying it achieves nothing but a second
/// copy; `data:` is part of the page, and a page may put a secret in one. Both
/// ends are checked, because a redirect can change the scheme.
#[test]
fn nothing_that_did_not_come_over_http_is_written_down() {
    let place = somewhere("scheme");
    let site = inside("https://example.com/");
    let request = asking("data:text/plain,a-secret-a-page-put-here");
    let mut response = answered("data:text/plain,a-secret-a-page-put-here", &[]);
    response.headers.add("ETag", "\"v1\"");

    let mut cache = cache_on(&place).expect("a cache directory");
    assert!(
        cache.keep(&request, &site, &response, at(START), at(START)),
        "it is reusable in memory like anything else"
    );
    assert!(is_hit(&cache.answer(&request, &site, at(START + 60))));
    assert_eq!(
        cache.disk().map(Disk::len),
        Some(0),
        "a data: URL was copied onto a disk"
    );
    assert_eq!(
        disk::why_it_is_never_written(&request, &response),
        Some("a response that did not come over HTTP")
    );
    let _ = fs::remove_dir_all(&place);
}

/// The one on the ADR's list that is not a check but a shape: a session-scoped
/// profile has a cache that was **never opened**, rather than one emptied at
/// the end. There is nothing to leave behind because there is nowhere to
/// leave it.
#[test]
fn a_cache_with_no_disk_writes_nothing_anywhere() {
    let place = somewhere("no-disk");
    let site = inside("https://example.com/");
    let request = asking("https://example.com/a");
    let mut cache = Cache::new();
    assert!(cache.keep(
        &request,
        &site,
        &answered("https://example.com/a", &[]),
        at(START),
        at(START)
    ));
    assert!(is_hit(&cache.answer(&request, &site, at(START + 60))));
    assert!(cache.disk().is_none());
    assert!(!place.exists(), "a cache with no disk made a directory");
}

/// A URL that was public yesterday and hands out a session token today. The
/// entry that was legitimately written is removed rather than left to be served
/// after a restart.
#[test]
fn an_entry_superseded_by_one_that_may_not_be_written_does_not_survive() {
    let place = somewhere("superseded");
    let site = inside("https://example.com/");
    let request = asking("https://example.com/a");
    {
        let mut cache = cache_on(&place).expect("a cache directory");
        assert!(cache.keep(
            &request,
            &site,
            &answered("https://example.com/a", &[]),
            at(START),
            at(START)
        ));
        assert_eq!(cache.disk().map(Disk::len), Some(1));

        assert!(cache.keep(
            &request,
            &site,
            &answered("https://example.com/a", &[("Set-Cookie", "session=abc")]),
            at(START),
            at(START)
        ));
        assert_eq!(
            cache.disk().map(Disk::len),
            Some(0),
            "the older entry was left where a restart would serve it"
        );
    }
    let mut restarted = cache_on(&place).expect("a cache directory");
    assert_eq!(
        restarted.answer(&request, &site, at(START + 60)),
        Answer::Fetch
    );
    let _ = fs::remove_dir_all(&place);
}

/// ADR 0011: *"what we owe them is that deleting it is real."*
#[test]
fn emptying_the_cache_takes_the_files_with_it() {
    let place = somewhere("emptied");
    let site = inside("https://example.com/");
    {
        let mut cache = cache_on(&place).expect("a cache directory");
        for n in 0..5 {
            let request = asking(&format!("https://example.com/{n}"));
            assert!(cache.keep(
                &request,
                &site,
                &answered("https://example.com/a", &[]),
                at(START),
                at(START)
            ));
        }
        assert_eq!(cache.disk().map(Disk::len), Some(5));
        cache.empty();
        assert_eq!(cache.disk().map(Disk::len), Some(0));
        assert_eq!(cache.len(), 0);
    }
    assert_eq!(
        fs::read_dir(&place)
            .expect("the directory")
            .flatten()
            .count(),
        0,
        "emptying the cache left the files on the disk"
    );
    let _ = fs::remove_dir_all(&place);
}

// --- What comes back off the disk is untrusted ---------------------------------

/// ADR 0011 section 4, and `LOOP.md`'s stage 2 rule: bytes from a filesystem are
/// bytes from outside. Every one of these is a **miss** — never an error that
/// reaches a page, and never a panic.
#[test]
fn a_directory_full_of_rubbish_is_a_miss_rather_than_a_failure_to_load() {
    let place = somewhere("hostile");
    let site = inside("https://example.com/");
    let request = asking("https://example.com/a");

    // One real entry first, so we know what a file of ours is called.
    let name = {
        let mut cache = cache_on(&place).expect("a cache directory");
        assert!(cache.keep(
            &request,
            &site,
            &answered("https://example.com/a", &[]),
            at(START),
            at(START)
        ));
        fs::read_dir(&place)
            .expect("the directory")
            .flatten()
            .next()
            .expect("the entry")
            .file_name()
    };
    let entry = place.join(&name);
    let whole = fs::read(&entry).expect("what was written");

    let rubbish: Vec<(&str, Vec<u8>)> = vec![
        ("nothing at all", Vec::new()),
        ("somebody else's file", b"# a note to myself".to_vec()),
        ("our magic and nothing else", b"alocache".to_vec()),
        ("every byte set", vec![0xff; 512]),
        ("every byte clear", vec![0; 512]),
        (
            "half of a real entry",
            whole.get(..whole.len() / 2).unwrap_or_default().to_vec(),
        ),
        ("a real entry with a byte flipped", {
            let mut damaged = whole.clone();
            let last = damaged.len() - 1;
            damaged[last] ^= 0xff;
            damaged
        }),
        ("a real entry with something appended", {
            let mut extended = whole.clone();
            extended.extend_from_slice(b"and then somebody else's bytes");
            extended
        }),
    ];

    for (what, bytes) in rubbish {
        fs::write(&entry, &bytes).unwrap_or_else(|why| panic!("{what}: {why}"));
        let mut cache = cache_on(&place).expect("a cache directory");
        assert_eq!(
            cache.answer(&request, &site, at(START + 60)),
            Answer::Fetch,
            "{what} was served as a response"
        );
        assert_eq!(cache.counts().2, 1, "{what} was not counted as a miss");
    }

    // And a whole entry, in the same place, still reads — so the refusals above
    // are the checks doing their work rather than nothing ever hitting.
    fs::write(&entry, &whole).expect("the entry again");
    let mut cache = cache_on(&place).expect("a cache directory");
    assert!(
        is_hit(&cache.answer(&request, &site, at(START + 60))),
        "the intact entry stopped reading too, so the cases above prove nothing"
    );
    let _ = fs::remove_dir_all(&place);
}

// --- Where it lives ------------------------------------------------------------

/// ADR 0011 section 3: one directory per profile, in the place the operating
/// system keeps caches, so a person can find and delete it. A profile name is
/// not a place to be lenient — one carrying a separator would put the cache
/// somewhere nobody chose.
#[test]
fn a_profile_name_that_is_not_a_name_has_nowhere_to_put_a_cache() {
    assert_eq!(disk::where_the_system_keeps_caches(""), None);
    assert_eq!(disk::where_the_system_keeps_caches("../../etc"), None);
    assert_eq!(disk::where_the_system_keeps_caches("a/b"), None);
    assert_eq!(disk::where_the_system_keeps_caches("a name"), None);
    let ordinary = disk::where_the_system_keeps_caches("default");
    if std::env::var_os("HOME").is_some() {
        let ordinary = ordinary.expect("somewhere to put a cache");
        assert!(
            ordinary.ends_with("alo-browser/default/http"),
            "{ordinary:?}"
        );
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 301's closing conditions, each asserted in numbers: counted
//! bytes, bucket counts, and files on disk.
//!
//! A restart here is a real one: the [`Store`] is dropped and a second one is
//! opened on the same directory, which reads everything back from the files
//! the first one left.

#![cfg(unix)]

use alo_net::Partition;
use alo_storage::{Area, Limits, Refused, StorageKey, Store};
use alo_url::Origin;
use std::fs;
use std::path::{Path, PathBuf};

/// A directory of this test's own. Named after the caller so two tests never
/// share one.
fn somewhere(called: &str) -> PathBuf {
    let place = std::env::temp_dir().join(format!(
        "alo-storage-{}-{called}",
        std::process::id().wrapping_mul(2_654_435_761)
    ));
    let _ = fs::remove_dir_all(&place);
    place
}

/// The key for a document under a top-level page, or [`None`] when either is
/// not a URL or the document's origin is opaque.
fn key(document: &str, top_level: &str) -> Option<StorageKey> {
    let document = alo_url::parse(document).ok()?;
    let top_level = alo_url::parse(top_level).ok()?;
    StorageKey::of(&Origin::of(&document), &Partition::of(&top_level))
}

/// The key for a page visited on its own.
fn alone(host: &str) -> Option<StorageKey> {
    let at = format!("https://{host}/");
    key(&at, &at)
}

/// Every file under a directory, at any depth.
fn files_under(place: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut waiting = vec![place.to_path_buf()];
    while let Some(directory) = waiting.pop() {
        let Ok(listing) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in listing.flatten() {
            let path = entry.path();
            if path.is_dir() {
                waiting.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The files named `name` in buckets that are live, not set aside.
fn live(place: &Path, name: &str) -> Vec<PathBuf> {
    files_under(place)
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|file| file == name))
        .filter(|path| !path.to_string_lossy().contains(".aside."))
        .collect()
}

/// How many bucket directories are set aside.
fn set_aside_on_disk(place: &Path) -> usize {
    let mut directories: Vec<PathBuf> = files_under(place)
        .into_iter()
        .filter_map(|file| file.parent().map(Path::to_path_buf))
        .filter(|parent| parent.to_string_lossy().contains(".aside."))
        .collect();
    directories.dedup();
    directories.len()
}

fn roomy() -> Limits {
    Limits::for_a_volume_with(100 << 30)
}

#[test]
fn an_area_survives_a_restart() {
    let place = somewhere("restart");
    let mail = alone("mail.example.com").expect("a key");
    {
        let mut store = Store::at(&place, roomy()).expect("a store");
        // What alo's `i18n/locale.ts` keeps, and a panel width.
        store.set_item(&mail, "alo.locale", "fr").expect("a write");
        store
            .set_item(&mail, "alo.panel-width", "320")
            .expect("a write");
        store
            .set_item(&mail, "alo.panel-width", "360")
            .expect("a write");
        assert_eq!(store.usage(&mail), 2 * (10 + 2 + 15 + 3));
    }

    let mut reopened = Store::at(&place, roomy()).expect("the same directory");
    assert_eq!(reopened.len(), 1, "what was written was not found again");
    assert_eq!(reopened.usage(&mail), 60);
    assert_eq!(reopened.total(), 60);
    assert_eq!(
        reopened.local(&mail),
        Area::new()
            .with("alo.locale", "fr")
            .with("alo.panel-width", "360")
    );
    assert!(reopened.set_aside().is_empty());
    // One bucket: its record, its use, and its site's name. Nothing else.
    assert_eq!(files_under(&place).len(), 3, "{:?}", files_under(&place));
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn a_write_past_a_buckets_quota_is_refused_and_changes_nothing() {
    let place = somewhere("quota");
    let limits = Limits {
        bucket: 100,
        local_area: 1000,
        profile: 10_000,
    };
    let mut store = Store::at(&place, limits).expect("a store");
    let one = alone("a.example").expect("a key");
    store
        .set_item(&one, "k", &"x".repeat(40))
        .expect("a write of 82 bytes");
    let before = fs::read(&live(&place, "bucket")[0]).expect("the bucket");

    let refused = store.set_item(&one, "l", &"y".repeat(10));
    assert_eq!(
        refused,
        Err(Refused::OverQuota {
            would_be: 82 + 22,
            quota: 100
        })
    );
    assert_eq!(store.usage(&one), 82);
    assert_eq!(store.total(), 82);
    assert_eq!(store.len(), 1);
    assert_eq!(
        fs::read(&live(&place, "bucket")[0]).expect("the bucket"),
        before,
        "a refused write changed the file"
    );
    drop(store);
    let mut reopened = Store::at(&place, limits).expect("the same directory");
    assert_eq!(reopened.local(&one).get("l"), None);
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn the_profiles_bound_evicts_whole_buckets_least_recently_used_first() {
    let place = somewhere("evict");
    // Room for four buckets of 100 counted bytes and not five.
    let limits = Limits {
        bucket: 1000,
        local_area: 1000,
        profile: 400,
    };
    let fifty = "x".repeat(49);
    let mut store = Store::at(&place, limits).expect("a store");
    let [ann, ben, cal, dee, eve] = ["ann", "ben", "cal", "dee", "eve"]
        .map(|host| alone(&format!("{host}.example")).expect("a key"));
    for bucket in [&ann, &ben, &cal, &dee] {
        // One name of one unit, a value of forty-nine: 100 counted bytes.
        store.set_item(bucket, "k", &fifty).expect("a write");
    }
    assert_eq!((store.len(), store.total()), (4, 400));

    // `ann` is read again, so `ben` is now the oldest; `cal` is open in a document.
    let _ = store.local(&ann);
    store.opened(&cal);

    // `eve` needs 100: one bucket goes, and it is `ben`.
    store.set_item(&eve, "k", &fifty).expect("room was made");
    assert_eq!((store.len(), store.total(), store.evicted()), (4, 400, 1));
    assert!(
        store.local(&ben).is_empty(),
        "the oldest bucket outlived the bound"
    );
    assert_eq!(
        live(&place, "bucket").len(),
        4,
        "an evicted bucket's file is still there"
    );

    // `dee` grows by 200 bytes, so two buckets must go. `dee` is the writer and
    // `cal` is open, which leaves `ann` (read before `eve` was written) and then `eve`.
    store
        .set_item(&dee, "l", &"y".repeat(99))
        .expect("room was made");
    assert_eq!(store.evicted(), 3);
    assert_eq!(store.len(), 2, "{:?}", live(&place, "bucket"));
    assert_eq!(store.usage(&cal), 100, "a bucket in use was evicted");
    assert_eq!(store.usage(&dee), 300, "the writer was evicted");
    assert_eq!(store.usage(&ann), 0);
    assert_eq!(store.usage(&eve), 0);
    assert_eq!(live(&place, "bucket").len(), 2);

    // Evicting everything that may go (nothing but `cal`, which is open, and
    // `dee`, which is writing) cannot make room: refused, and nothing goes.
    let refused = store.set_item(&dee, "m", &"z".repeat(49));
    assert_eq!(refused, Err(Refused::ProfileFull));
    assert_eq!((store.len(), store.total(), store.evicted()), (2, 400, 3));
    assert_eq!(live(&place, "bucket").len(), 2);

    // Closed, `cal` may go.
    store.closed(&cal);
    store
        .set_item(&dee, "m", &"z".repeat(49))
        .expect("room was made");
    assert_eq!((store.len(), store.total(), store.evicted()), (1, 400, 4));
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn the_order_of_use_survives_a_restart() {
    let place = somewhere("order");
    let limits = Limits {
        bucket: 1000,
        local_area: 1000,
        profile: 200,
    };
    let fifty = "x".repeat(49);
    let [ann, ben, cal] =
        ["ann", "ben", "cal"].map(|host| alone(&format!("{host}.example")).expect("a key"));
    {
        let mut store = Store::at(&place, limits).expect("a store");
        store.set_item(&ann, "k", &fifty).expect("a write");
        store.set_item(&ben, "k", &fifty).expect("a write");
        // Read, not written: a use all the same.
        let _ = store.local(&ann);
    }
    let mut reopened = Store::at(&place, limits).expect("the same directory");
    reopened.set_item(&cal, "k", &fifty).expect("room was made");
    assert_eq!(
        reopened.usage(&ben),
        0,
        "the order of use was forgotten at the restart"
    );
    assert_eq!(reopened.usage(&ann), 100);
    let _ = fs::remove_dir_all(&place);
}

/// Write one bucket and close the store, returning the paths of its two
/// records and their bytes.
fn one_bucket(place: &Path, key: &StorageKey) -> Result<[(PathBuf, Vec<u8>); 2], String> {
    let mut store = Store::at(place, roomy())?;
    store
        .set_item(key, "alo.locale", "fr")
        .map_err(|why| why.to_string())?;
    store
        .set_item(key, "alo.refresh", "remembered")
        .map_err(|why| why.to_string())?;
    let record = |name: &str| -> Result<(PathBuf, Vec<u8>), String> {
        let path = live(place, name)
            .into_iter()
            .next()
            .ok_or_else(|| format!("no {name} record"))?;
        let bytes = fs::read(&path).map_err(|why| why.to_string())?;
        Ok((path, bytes))
    };
    Ok([record("bucket")?, record("used")?])
}

/// Every damaged copy of a record: each truncation, then each flipped byte.
fn every_damage(whole: &[u8]) -> Vec<Vec<u8>> {
    let mut damaged: Vec<Vec<u8>> = (0..whole.len())
        .filter_map(|cut| whole.get(..cut).map(<[u8]>::to_vec))
        .collect();
    for at in 0..whole.len() {
        let mut flipped = whole.to_vec();
        if let Some(byte) = flipped.get_mut(at) {
            *byte ^= 0xff;
        }
        damaged.push(flipped);
    }
    damaged
}

#[test]
fn every_truncation_and_every_flipped_byte_sets_the_bucket_aside_and_is_recorded() {
    let place = somewhere("damage");
    let mail = alone("mail.example.com").expect("a key");
    let records = one_bucket(&place, &mail).expect("a bucket");
    // Every file as written, so each case starts from exactly this.
    let written: Vec<(PathBuf, Vec<u8>)> = files_under(&place)
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).expect("a file");
            (path, bytes)
        })
        .collect();
    let mut tried = 0;
    for (path, whole) in &records {
        for (case, damaged) in every_damage(whole).into_iter().enumerate() {
            let _ = fs::remove_dir_all(&place);
            for (file, bytes) in &written {
                fs::create_dir_all(file.parent().expect("a directory")).expect("a directory");
                fs::write(file, bytes).expect("a file as written");
            }
            fs::write(path, &damaged).expect("a damaged record");

            let mut store = Store::at(&place, roomy()).expect("a store");
            assert_eq!(store.len(), 0, "a damaged bucket was counted");
            assert_eq!(store.total(), 0);
            assert!(
                store.local(&mail).is_empty(),
                "part of a damaged bucket was served"
            );
            assert_eq!(store.set_aside().len(), 1, "{} case {case}", path.display());
            assert_eq!(store.set_aside()[0].site.as_deref(), Some("example.com"));
            assert_eq!(set_aside_on_disk(&place), 1, "a damaged bucket was deleted");
            assert!(live(&place, "bucket").is_empty());

            if case == 0 {
                // The site starts as if it had stored nothing, and writes again.
                store.set_item(&mail, "alo.locale", "en").expect("a write");
                assert_eq!(store.usage(&mail), 2 * (10 + 2));
            }
            tried += 1;
        }
    }
    assert_eq!(tried, 2 * (records[0].1.len() + records[1].1.len()));
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn a_bucket_damaged_after_start_is_set_aside_when_it_is_next_read() {
    let place = somewhere("later");
    let mail = alone("mail.example.com").expect("a key");
    let [(path, whole), _] = one_bucket(&place, &mail).expect("a bucket");
    let mut store = Store::at(&place, roomy()).expect("a store");
    assert_eq!(store.len(), 1);
    let mut damaged = whole;
    let last = damaged.len() - 1;
    damaged[last] ^= 0x01;
    fs::write(&path, &damaged).expect("damage");

    assert!(store.local(&mail).is_empty());
    assert_eq!((store.len(), store.total()), (0, 0));
    assert_eq!(store.set_aside().len(), 1);
    assert_eq!(set_aside_on_disk(&place), 1);
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn a_session_scoped_profile_leaves_no_file() {
    let mut store = Store::for_the_session(roomy());
    let mail = alone("mail.example.com").expect("a key");
    store
        .set_item(&mail, "alo.refresh", "a token")
        .expect("a write");
    assert_eq!(
        store.directory(),
        None,
        "a session-scoped store has a directory"
    );
    assert_eq!(store.usage(&mail), 2 * (11 + 7));
    assert_eq!(store.local(&mail).get("alo.refresh"), Some("a token"));
    drop(store);
    // Nothing anywhere to reopen: a second session starts empty.
    let mut next = Store::for_the_session(roomy());
    assert!(next.local(&mail).is_empty());
    assert!(next.is_empty());
}

#[test]
fn clearing_a_site_removes_every_bucket_of_it_under_every_partition_set_aside_ones_included() {
    let place = somewhere("clear");
    let widget_alone = key("https://widget.example/", "https://widget.example/").expect("a key");
    let widget_in_news = key("https://widget.example/", "https://news.example/").expect("a key");
    let widget_in_shop =
        key("https://cdn.widget.example/", "https://shop.example/").expect("a key");
    let news = alone("news.example").expect("a key");
    {
        let mut store = Store::at(&place, roomy()).expect("a store");
        for bucket in [&widget_alone, &widget_in_news, &widget_in_shop, &news] {
            store.set_item(bucket, "k", "v").expect("a write");
        }
    }
    // One of the widget's buckets damaged, so that it is set aside at start.
    let widget_files: Vec<PathBuf> = live(&place, "bucket")
        .into_iter()
        .filter(|path| {
            fs::read(path)
                .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains("shop.example"))
        })
        .collect();
    assert_eq!(widget_files.len(), 1);
    fs::write(&widget_files[0], b"rubbish").expect("damage");

    let mut store = Store::at(&place, roomy()).expect("the same directory");
    assert_eq!((store.len(), store.set_aside().len()), (3, 1));
    assert_eq!(set_aside_on_disk(&place), 1);

    store.clear_site("widget.example").expect("cleared");
    assert_eq!(store.len(), 1, "a bucket of the site survived clearing it");
    assert_eq!(store.usage(&news), 4);
    assert!(
        store.set_aside().is_empty(),
        "a set-aside bucket survived clearing its site"
    );
    assert_eq!(set_aside_on_disk(&place), 0);
    assert_eq!(live(&place, "bucket").len(), 1);

    drop(store);
    let mut reopened = Store::at(&place, roomy()).expect("the same directory");
    assert_eq!(reopened.len(), 1);
    assert!(reopened.local(&widget_in_news).is_empty());
    assert!(reopened.local(&widget_alone).is_empty());
    assert_eq!(reopened.local(&news).get("k"), Some("v"));
    // The news site's bucket, its use and its site's name.
    assert_eq!(files_under(&place).len(), 3);
    let _ = fs::remove_dir_all(&place);
}

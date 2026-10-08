/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What `alo-svg` reads from a computed style is what `alo-css`'s list of
//! supported properties says it reads, and no more (ADR 0033 § 4).
//!
//! The crate's own source is scanned for the names passed to its readers
//! (`alo_css::properties::named_in`): `ComputedStyle`'s own in every file,
//! and the wrappers each file reads through in that file.
//!
//! What it reads **only to refuse** — ADR 0022 § 7's `clip-path`, `mask`,
//! `filter` and markers, and the two it says are not applied yet — is not
//! acted on, so it is held off the list rather than on it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use alo_css::properties::{self, Reader};

/// The `ComputedStyle` methods that take a property's name, read for in
/// every file.
const EVERYWHERE: &[&str] = &["get", "length", "px", "number", "color"];

/// Files where one of those names is something else — a map's `get`, an
/// attribute's `length` — by path under `src/` and the name.
const NOT_STYLE: &[(&str, &str)] = &[("shape.rs", "length")];

/// The readers of one file's own, by its path under `src/`.
const IN_FILE: &[(&str, &[&str])] = &[
    ("stroke.rs", &["keyword", "paint_of"]),
    ("fill.rs", &["paint_of"]),
];

/// Names that reach a reader through a variable, as written in the source:
/// a `{side}` in one stands for all four sides.
const INDIRECT: &[&str] = &[];

/// Names read only to say they are not applied, as written in the source.
const REFUSED: &[&str] = &[
    "clip-path",
    "filter",
    "marker-end",
    "marker-mid",
    "marker-start",
    "mask",
    "paint-order",
    "vector-effect",
];

/// Every `.rs` file under `dir`, with its path relative to `root`.
fn sources(root: &Path, dir: &Path) -> Option<Vec<(String, String)>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).ok()? {
        let path: PathBuf = entry.ok()?.path();
        if path.is_dir() {
            found.extend(sources(root, &path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let name = path.strip_prefix(root).ok()?.to_str()?.to_owned();
            found.push((name, std::fs::read_to_string(&path).ok()?));
        }
    }
    Some(found)
}

/// A name as written, its `{side}` expanded.
fn expanded(name: &str) -> Vec<String> {
    if name.contains("{side}") {
        ["top", "right", "bottom", "left"]
            .iter()
            .map(|side| name.replace("{side}", side))
            .collect()
    } else {
        vec![name.to_owned()]
    }
}

#[test]
fn every_property_it_reads_is_listed_with_it_and_nothing_else_is() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = sources(&root, &root).expect("the crate's source is readable");
    assert!(!files.is_empty());
    let mut read = BTreeSet::new();
    for (name, text) in &files {
        let everywhere: Vec<&str> = EVERYWHERE
            .iter()
            .filter(|reader| !NOT_STYLE.contains(&(name.as_str(), **reader)))
            .copied()
            .collect();
        read.extend(properties::named_in(text, &everywhere));
        for (file, readers) in IN_FILE {
            if name == file {
                read.extend(properties::named_in(text, readers));
            }
        }
    }
    for file in IN_FILE
        .iter()
        .map(|(file, _)| file)
        .chain(NOT_STYLE.iter().map(|(file, _)| file))
    {
        assert!(
            files.iter().any(|(name, _)| name == file),
            "{file} is not in src/"
        );
    }
    for name in INDIRECT.iter().chain(REFUSED) {
        let quoted = format!("\"{name}\"");
        assert!(
            files.iter().any(|(_, text)| text.contains(&quoted)),
            "{name} is written nowhere in the source"
        );
    }
    read.extend(INDIRECT.iter().flat_map(|name| expanded(name)));
    for name in REFUSED {
        assert!(
            !properties::is_supported(name),
            "{name} is refused, not acted on"
        );
        read.remove(*name);
    }
    let listed: BTreeSet<String> = properties::read_by(Reader::Svg)
        .map(str::to_owned)
        .collect();
    assert_eq!(read, listed);
}

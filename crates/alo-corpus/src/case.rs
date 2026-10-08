/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One case: what to render, and what it should come out as.
//!
//! A case is a directory. That is deliberate — the expectations are **files**,
//! so a change to one shows up in a diff as a change to that file, and a
//! reviewer reads "row three moved four pixels" out of the diff rather than
//! out of a test failure they have to reproduce.
//!
//! ```text
//! cases/invoices/
//!   page.html      what to render
//!   style.css      how
//!   boxes.txt      the box tree it should build
//!   layout.txt     where every box should end up
//!   display.txt    what should be drawn, in order
//!   agent.txt      what an agent should read
//!   render.png     what it should look like
//! ```
//!
//! A case whose page fetches says where it was served from and what it was
//! answered with (ADR 0032 § 7):
//!
//! ```text
//!   address.txt    the URL the page was served from
//!   responses.txt  each URL it fetches, and the file its response is frozen in
//! ```
//!
//! Its `linked.txt` means the same for either kind of page: what the page
//! named, and the file beside it. A page rendered as markup is handed each
//! file by that name; a page whose script runs asks for its sheets, and is
//! answered each by the name resolved against `address.txt`.
//!
//! None of them is redundant. `boxes.txt` catches a change in what exists,
//! `layout.txt` a change in where it is, `display.txt` a change in what is
//! drawn, `agent.txt` a change in what the page *means*, and `render.png`
//! everything the others cannot describe — anti-aliasing, glyph shapes,
//! compositing. The first four say *what* changed; the picture says *that*
//! something did.

use alo_renderer::Resource;
use std::path::{Path, PathBuf};

/// How wide and tall a case is rendered, unless it says otherwise.
pub const DEFAULT_SIZE: (f32, f32) = (240.0, 160.0);

/// One file frozen beside a case, as its `linked.txt` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frozen {
    /// What the page called it: an `href` or a `src` exactly as written.
    pub name: String,
    /// The file beside the case holding it, by its name there.
    pub file: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

/// One case, read from a directory.
#[derive(Debug, Clone)]
pub struct Case {
    /// The directory it came from.
    pub directory: PathBuf,
    /// What the case is called, which is its directory's name.
    pub name: String,
    /// The markup to render.
    pub html: String,
    /// The style sheet to render it with.
    pub css: String,
    /// How large a picture to render.
    pub size: (f32, f32),
    /// The style sheets the page links to, frozen beside it.
    ///
    /// An `href` exactly as the page wrote it, and the CSS behind it. A real
    /// page keeps its style in a second file, and a case that could not hold
    /// one could only ever be a page that keeps it inline — which is a thing
    /// almost no page does.
    ///
    /// **Frozen, never fetched**, for the reason `LOOP.md` gives: a suite that
    /// went to the network would be flaky, would fail on an aeroplane, and
    /// would hand every site's owner the ability to break our build.
    pub linked: Vec<(String, String)>,
    /// The pictures the page asks for, frozen beside it.
    ///
    /// A `src` exactly as the page wrote it, and the bytes behind it. Listed in
    /// the same `linked.txt` as the style sheets, because from a case's point of
    /// view they are the same thing: something the page named and something
    /// frozen next to it.
    ///
    /// A frozen file had no response, so its type is what its own file name's
    /// extension stands in for ([`Resource::from_file`]): a `.svg` is an SVG
    /// picture, and every other picture is decided by its bytes.
    pub resources: Vec<Resource>,
    /// Every line of `linked.txt` whose file is there, as it was frozen: the
    /// name the page used, the file beside the case, and its bytes.
    ///
    /// [`Case::linked`] and [`Case::resources`] are what a page rendered as
    /// markup is handed, by the name as the page wrote it. A page whose script
    /// runs is loaded by a renderer, which is handed nothing and **asks** for
    /// its sheets by URL; the corpus answers those asks from this, each name
    /// resolved against [`Case::address`] and each file typed by its
    /// extension ([`crate::sheets`], ADR 0035 § 6).
    pub frozen: Vec<Frozen>,
    /// Where the page was served from, from its `address.txt`: what a
    /// relative URL in it means, and the origin its fetches are made from.
    /// [`None`] for a page served from nowhere — `about:blank`.
    pub address: Option<String>,
    /// The responses the page's fetches are answered with, frozen beside it:
    /// each URL as the browser process would ask for it, and the bytes the
    /// server sent ([`crate::answering`]).
    ///
    /// Frozen, never fetched, for the reason [`Case::linked`] is; a URL with
    /// none is answered as a network error.
    pub responses: Vec<(String, Vec<u8>)>,
}

impl Case {
    /// Read a case from its directory.
    ///
    /// Returns [`None`] for a directory that is not a case — one with no
    /// `page.html` — so that a stray file among the cases is skipped rather
    /// than failing the run.
    pub fn read(directory: &Path) -> Option<Self> {
        let html = std::fs::read_to_string(directory.join("page.html")).ok()?;
        let css = std::fs::read_to_string(directory.join("style.css")).unwrap_or_default();
        let name = directory.file_name()?.to_str()?.to_owned();
        let size = std::fs::read_to_string(directory.join("size.txt"))
            .ok()
            .and_then(|text| parse_size(&text))
            .unwrap_or(DEFAULT_SIZE);
        Some(Self {
            directory: directory.to_path_buf(),
            name,
            html,
            css,
            size,
            linked: linked_sheets(directory),
            resources: linked_resources(directory),
            frozen: frozen_files(directory),
            address: std::fs::read_to_string(directory.join("address.txt"))
                .ok()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty()),
            responses: frozen_responses(directory),
        })
    }

    /// Every case in a directory, in a stable order.
    ///
    /// Sorted by name so that a run reports them the same way twice, which is
    /// what makes a failure list readable.
    pub fn read_all(directory: &Path) -> Vec<Self> {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut cases: Vec<Case> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| Case::read(&entry.path()))
            .collect();
        cases.sort_by(|left, right| left.name.cmp(&right.name));
        cases
    }

    /// Where one of this case's expectations is kept.
    pub fn expectation(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }
}

/// `240x160`, as a case's `size.txt` writes it.
fn parse_size(text: &str) -> Option<(f32, f32)> {
    let (width, height) = text.trim().split_once(['x', '×'])?;
    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
}

/// The frozen responses beside a case, from its `responses.txt`.
///
/// One per line: the URL the response answers, as the browser process would
/// ask for it, a space, and the file beside the case holding the bytes the
/// server sent. Blank lines and `#` comments are skipped, and so is a line
/// naming a file that is not there — that URL is then answered as a network
/// error, which is a real state rather than a broken case.
fn frozen_responses(directory: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(list) = std::fs::read_to_string(directory.join("responses.txt")) else {
        return Vec::new();
    };
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(char::is_whitespace))
        .filter_map(|(url, file)| {
            let bytes = std::fs::read(directory.join(file.trim())).ok()?;
            Some((url.trim().to_owned(), bytes))
        })
        .collect()
}

/// Every file `linked.txt` names that is there, with the name the page used
/// for it.
///
/// A line naming a file that is not there is skipped, as for
/// [`linked_sheets`]: a page rendered as markup is then drawn without it, and
/// a loaded page's ask for it is answered as a sheet the case froze nothing
/// for.
fn frozen_files(directory: &Path) -> Vec<Frozen> {
    let Ok(list) = std::fs::read_to_string(directory.join("linked.txt")) else {
        return Vec::new();
    };
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(char::is_whitespace))
        .filter_map(|(name, file)| {
            let file = file.trim();
            let bytes = std::fs::read(directory.join(file)).ok()?;
            Some(Frozen {
                name: name.trim().to_owned(),
                file: file.to_owned(),
                bytes,
            })
        })
        .collect()
}

/// The frozen pictures beside a case, from the same `linked.txt`.
///
/// Read as **bytes** rather than text, and read by the same list, because a
/// case has one place where it says "this is what that name means" — two lists
/// would be two places to forget.
fn linked_resources(directory: &Path) -> Vec<Resource> {
    let Ok(list) = std::fs::read_to_string(directory.join("linked.txt")) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for line in list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, file)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let file = file.trim();
        let Ok(bytes) = std::fs::read(directory.join(file)) else {
            continue;
        };
        found.push(Resource::from_file(name.trim(), file, bytes));
    }
    found
}

/// The frozen sub-resources beside a case, from its `linked.txt`.
///
/// One per line: the `href` as the page wrote it, a space, and the file beside
/// the case holding what came back. Written down rather than inferred from
/// filenames, because the `href` is what the page said and a mapping somebody
/// can read is a mapping somebody can check — which is the same reason a case
/// carries an `origin.txt`.
///
/// A line naming a file that is not there is skipped, so the case renders as a
/// page whose style sheet did not arrive — which the renderer records as an
/// issue rather than treating as an error, because it is a real state.
fn linked_sheets(directory: &Path) -> Vec<(String, String)> {
    let Ok(list) = std::fs::read_to_string(directory.join("linked.txt")) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for line in list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((href, file)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(directory.join(file.trim())) else {
            continue;
        };
        found.push((href.trim().to_owned(), text));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_read_in_either_spelling() {
        assert_eq!(parse_size("240x160"), Some((240.0, 160.0)));
        assert_eq!(parse_size(" 100 × 50 \n"), Some((100.0, 50.0)));
        assert_eq!(parse_size("240"), None);
        assert_eq!(parse_size("wide x tall"), None);
    }

    #[test]
    fn a_directory_that_is_not_a_case_is_skipped_rather_than_fatal() {
        assert!(Case::read(Path::new("/definitely/not/here")).is_none());
        assert!(Case::read_all(Path::new("/definitely/not/here")).is_empty());
    }

    #[test]
    fn the_real_cases_are_all_readable_and_named() {
        let cases = Case::read_all(&crate::cases_directory());
        assert!(!cases.is_empty(), "the corpus has cases in it");
        for case in &cases {
            assert!(!case.name.is_empty());
            assert!(!case.html.is_empty(), "{} has markup", case.name);
            assert!(case.size.0 > 0.0 && case.size.1 > 0.0, "{}", case.name);
        }

        let names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "cases are reported in a stable order");
    }
}

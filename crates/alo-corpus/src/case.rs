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
//! None of them is redundant. `boxes.txt` catches a change in what exists,
//! `layout.txt` a change in where it is, `display.txt` a change in what is
//! drawn, `agent.txt` a change in what the page *means*, and `render.png`
//! everything the others cannot describe — anti-aliasing, glyph shapes,
//! compositing. The first four say *what* changed; the picture says *that*
//! something did.

use std::path::{Path, PathBuf};

/// How wide and tall a case is rendered, unless it says otherwise.
pub const DEFAULT_SIZE: (f32, f32) = (240.0, 160.0);

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

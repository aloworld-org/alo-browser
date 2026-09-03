//! Checking a case against what it should be.
//!
//! **All four differences at once.** A case that changed usually changed in
//! more than one way — a box moved, so the display list moved, so the picture
//! moved — and reporting the first and stopping means running the tests four
//! times to find out what happened. So every expectation is checked, and
//! everything that differs is reported together.

use crate::case::Case;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::Rendered;
use core::fmt;

/// One expectation that did not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// Which expectation: `boxes.txt`, `layout.txt`, `display.txt`,
    /// `render.png`.
    pub expectation: String,
    /// What is wrong, in words.
    pub detail: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.expectation, self.detail)
    }
}

/// Whether the expectations are being rewritten rather than checked.
fn updating() -> bool {
    std::env::var_os("ALO_UPDATE_REFERENCES").is_some()
}

/// Check a rendered case against its committed expectations.
///
/// Returns everything that differs. An empty list is a case that holds.
pub fn check(case: &Case, rendered: &Rendered) -> Vec<Difference> {
    let mut differences = Vec::new();
    let expectations = [
        ("boxes.txt", rendered.boxes.to_outline()),
        ("layout.txt", rendered.layout.to_outline(&rendered.boxes)),
        ("display.txt", rendered.display.to_outline()),
        // ADR 0002: *"Reference renders can assert the tree, not just
        // pixels."* This is that — what an agent reads, pinned beside what a
        // person sees, so the two cannot drift apart unnoticed.
        (
            "agent.txt",
            alo_agent::AgentTree::new(&rendered.document, &rendered.boxes, &rendered.layout)
                .to_outline(),
        ),
        (
            "issues.txt",
            format!("{}\n", rendered.issues().join("\n"))
                .trim_start()
                .to_owned(),
        ),
    ];
    for (name, produced) in expectations {
        if let Some(difference) = check_text(case, name, &produced) {
            differences.push(difference);
        }
    }
    if let Some(difference) = check_picture(case, &rendered.canvas) {
        differences.push(difference);
    }
    differences
}

/// Compare one text expectation, or write it when updating.
fn check_text(case: &Case, name: &str, produced: &str) -> Option<Difference> {
    let path = case.expectation(name);
    if updating() {
        if let Err(error) = std::fs::write(&path, produced) {
            return Some(Difference {
                expectation: name.to_owned(),
                detail: format!("could not be written: {error}"),
            });
        }
        return None;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    if expected == produced {
        return None;
    }
    Some(Difference {
        expectation: name.to_owned(),
        detail: first_differing_line(&expected, produced),
    })
}

/// The first line that differs, with its number.
///
/// A whole-file diff in a test failure is a wall; the first line that changed
/// is nearly always the thing that changed, and the file on disk is there for
/// the rest.
fn first_differing_line(expected: &str, produced: &str) -> String {
    let mut expected_lines = expected.lines();
    let mut produced_lines = produced.lines();
    let mut number = 1;
    loop {
        match (expected_lines.next(), produced_lines.next()) {
            (None, None) => {
                return "the files differ only in whitespace at the end".to_owned();
            }
            (was, now) if was == now => number += 1,
            (was, now) => {
                return format!(
                    "line {number}\n     was: {}\n     now: {}",
                    was.unwrap_or("<nothing: the file is shorter>"),
                    now.unwrap_or("<nothing: the render is shorter>"),
                );
            }
        }
    }
}

/// Compare the picture, or write it when updating.
fn check_picture(case: &Case, canvas: &Canvas) -> Option<Difference> {
    let path = case.expectation("render.png");
    let drawn = match to_png(canvas) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Some(Difference {
                expectation: "render.png".to_owned(),
                detail: error.to_string(),
            });
        }
    };
    if updating() {
        return std::fs::write(&path, &drawn).err().map(|error| Difference {
            expectation: "render.png".to_owned(),
            detail: format!("could not be written: {error}"),
        });
    }

    let Ok(committed) = std::fs::read(&path) else {
        return Some(Difference {
            expectation: "render.png".to_owned(),
            detail: "there is no committed picture yet".to_owned(),
        });
    };
    let expected = match from_png(&committed) {
        Ok(canvas) => canvas,
        Err(error) => {
            return Some(Difference {
                expectation: "render.png".to_owned(),
                detail: format!("the committed picture could not be read: {error}"),
            });
        }
    };
    if (expected.width(), expected.height()) != (canvas.width(), canvas.height()) {
        return Some(Difference {
            expectation: "render.png".to_owned(),
            detail: format!(
                "the picture changed size: was {}×{}, now {}×{}",
                expected.width(),
                expected.height(),
                canvas.width(),
                canvas.height(),
            ),
        });
    }

    let mut differing = 0usize;
    let mut first = None;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (Some(was), Some(now)) = (expected.at(x, y), canvas.at(x, y)) else {
                continue;
            };
            if was.to_rgba8() != now.to_rgba8() {
                differing += 1;
                if first.is_none() {
                    first = Some((x, y, was.to_rgba8(), now.to_rgba8()));
                }
            }
        }
    }
    if differing == 0 {
        return None;
    }
    Some(Difference {
        expectation: "render.png".to_owned(),
        detail: match first {
            Some((x, y, was, now)) => format!(
                "{differing} pixels differ; the first is at {x},{y}: was {was:?}, now {now:?}",
            ),
            None => format!("{differing} pixels differ"),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_differing_line_is_the_one_reported() {
        let report = first_differing_line("one\ntwo\nthree\n", "one\nTWO\nthree\n");
        assert!(report.contains("line 2"), "{report}");
        assert!(report.contains("was: two"), "{report}");
        assert!(report.contains("now: TWO"), "{report}");
    }

    #[test]
    fn a_shorter_render_says_so_rather_than_showing_nothing() {
        let report = first_differing_line("one\ntwo\n", "one\n");
        assert!(report.contains("the render is shorter"), "{report}");

        let other = first_differing_line("one\n", "one\ntwo\n");
        assert!(other.contains("the file is shorter"), "{other}");
    }

    #[test]
    fn a_difference_says_which_expectation_it_is() {
        let difference = Difference {
            expectation: "layout.txt".to_owned(),
            detail: "line 3".to_owned(),
        };
        assert_eq!(difference.to_string(), "layout.txt: line 3");
    }
}

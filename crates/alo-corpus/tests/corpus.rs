//! Every case in the corpus, checked.
//!
//! One test per case would be better reading, and Rust does not generate tests
//! from a directory — so this is one test that reports **every** case that
//! differs, with **every** expectation that differs in it. A run that fails
//! tells you the whole story rather than the first sentence of it.

use alo_corpus::{Case, cases_directory, check, corpus_fonts, render_with};
use alo_layout::Size;
use core::fmt::Write as _;

#[test]
fn every_case_renders_the_way_it_is_committed_to() {
    let fonts = corpus_fonts();
    let cases = Case::read_all(&cases_directory());
    assert!(
        !cases.is_empty(),
        "the corpus is empty, which is a bug in it"
    );

    let mut report = String::new();
    for case in &cases {
        let rendered = render_with(
            &case.html,
            &case.css,
            Size::new(case.size.0, case.size.1),
            &fonts,
            &case.linked,
        );
        let differences = check(case, &rendered);
        if differences.is_empty() {
            continue;
        }
        // Writing to a `String` cannot fail.
        let _ = writeln!(report, "\n  {}", case.name);
        for difference in differences {
            let _ = writeln!(report, "    {difference}");
        }
    }

    assert!(
        report.is_empty(),
        "{report}\n\
         If these changes are intended, ALO_UPDATE_REFERENCES=1 rewrites the \
         expectations — and the diff is then the review.",
    );
}

#[test]
fn every_case_is_rendered_the_same_way_twice() {
    // A corpus is only worth committing if it is deterministic. This is the
    // test that says so, and it is separate because a failure here means
    // something quite different from a failure above.
    let fonts = corpus_fonts();
    for case in Case::read_all(&cases_directory()) {
        let size = Size::new(case.size.0, case.size.1);
        let first = render_with(&case.html, &case.css, size, &fonts, &case.linked);
        let second = render_with(&case.html, &case.css, size, &fonts, &case.linked);
        assert_eq!(
            first.display.to_outline(),
            second.display.to_outline(),
            "{} drew different things twice",
            case.name,
        );
        assert_eq!(
            first.canvas.pixels().len(),
            second.canvas.pixels().len(),
            "{}",
            case.name,
        );
        for (left, right) in first.canvas.pixels().iter().zip(second.canvas.pixels()) {
            assert_eq!(
                left.to_rgba8(),
                right.to_rgba8(),
                "{} drew different pixels twice",
                case.name,
            );
        }
    }
}

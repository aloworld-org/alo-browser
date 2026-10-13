/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's click on a submit button (ADR 0051 §§ 4 and 6, queue item
//! 543).
//!
//! A person submits a form the way an agent does, by pressing its submit
//! button, and the form is sent as an agent's `Activate` would send it: the
//! same checks, the same entry list, the same ask, decided by the browser
//! process as the person's (`Tabs::point`).
//!
//! # Never refused
//!
//! An agent's press is refused before anything is heard when its form could
//! fail a constraint this engine does not check, or says something it does
//! not send ([`crate::submit`]). A person's press cannot be: the person
//! pressed, and the page hears the click whatever this engine can send. So
//! what would have been an agent's refusal is, for a person, what the form
//! came to — **not sent, and said** — and so is a form whose controls fail
//! their checks, a `submit` the page cancelled, and the rest of what
//! [`Submitted`] can be ([`said`]).
//!
//! # On a page that runs script
//!
//! The click's own task submits the form (`event_loop`'s `submitted`), with
//! `invalid` and `submit` between its steps for the page's listeners, and
//! what it came to is said here.
//!
//! # On a page nobody listens on
//!
//! A page that never ran script, or whose script has stopped: nobody can
//! cancel anything, so the steps are `alo-dom`'s alone ([`unheard`]) —
//! checked unless the form or the button says not to, planned, and asked as
//! a link the renderer followed itself is asked.
//!
//! # A control, named to a person
//!
//! An agent is told a control by its box. A person is told it as markup
//! would name it, by its element and its `name` — `input name="date"` —
//! quoted as Rust quotes a string, so a page's name cannot write a line of
//! its own. Every line goes through [`crate::said::line`], so a form of a
//! million failing fields is still one line of bounded length.

use alo_bindings::navigating::{self, By, Navigation, NotFollowed};
use alo_dom::constraint::{self, Constraint, Problem};
use alo_dom::{Document, NodeId, submission};
use alo_url::Url;
use core::fmt;

use crate::event_loop::{Failed, Submitted};

/// What a person's click on a submit button came to on a page nobody
/// listens on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unheard {
    /// The form was asked for — or, for an action that resolves to nothing,
    /// why it was not.
    Asked(Result<Navigation, NotFollowed>),
    /// The form was not sent, and this is why, in words.
    NotSent(String),
}

/// Submit `form` from `submitter`, in `document` at `url`, with nobody to
/// hear it: checked, planned and asked, by `alo-dom`'s rules alone.
pub(crate) fn unheard(
    document: &Document,
    url: &Url,
    (form, submitter): (NodeId, NodeId),
) -> Unheard {
    if !submission::novalidate(document, form, Some(submitter)) {
        let mut failed = Vec::new();
        for (control, problem) in constraint::problems(document, form) {
            match problem {
                Problem::Unchecked { constraint } => {
                    return Unheard::NotSent(unchecked(document, control, constraint));
                }
                Problem::Fails {
                    constraint,
                    message,
                } => failed.push(Failed {
                    control,
                    constraint,
                    message,
                }),
            }
        }
        if !failed.is_empty() {
            return Unheard::NotSent(invalid(document, &failed));
        }
    }
    match submission::plan(document, form, Some(submitter)) {
        Ok(plan) => Unheard::Asked(navigating::submit(document, url, plan, By::Browser)),
        Err(why) => Unheard::NotSent(not_sent(&why)),
    }
}

/// What is said of `submitted`, what a person's click came to on a page
/// that runs script, in `document` as it is now: [`None`] when the form was
/// asked for, which the ask itself says.
pub(crate) fn said(submitted: &Submitted, document: Option<&Document>) -> Option<String> {
    let line = match submitted {
        Submitted::Asked { .. } => return None,
        Submitted::Invalid(failed) => match document {
            Some(document) => invalid(document, failed),
            None => not_sent(&"its controls fail their checks"),
        },
        Submitted::Unchecked {
            control,
            constraint,
        } => match document {
            Some(document) => unchecked(document, *control, *constraint),
            None => not_sent(&format_args!(
                "a control could fail {constraint}, which this engine does not check yet"
            )),
        },
        Submitted::Canceled => not_sent(&"the page cancelled its submit event, and took the click"),
        Submitted::Withdrawn => {
            not_sent(&"the page took the form out of its document as it was submitted")
        }
        Submitted::AlreadySubmitting => {
            not_sent(&"it was already being submitted, and a form is not submitted again")
        }
        Submitted::Unsubmittable(why) => not_sent(why),
    };
    Some(line)
}

/// The line for a form not sent, because of `why`.
fn not_sent(why: &dyn fmt::Display) -> String {
    crate::said::line(&format_args!("its form was not sent: {why}"))
}

/// The line for a form whose `control` could fail `constraint`, which this
/// engine does not check.
fn unchecked(document: &Document, control: NodeId, constraint: Constraint) -> String {
    not_sent(&format_args!(
        "{} could fail {constraint}, which this engine does not check yet",
        Named { document, control }
    ))
}

/// The line for a form whose controls failed their checks: each, its
/// constraint and Chrome's words for it.
fn invalid(document: &Document, failed: &[Failed]) -> String {
    not_sent(&Failing { document, failed })
}

/// A control, as a person is told it: its element and its `name`.
struct Named<'a> {
    document: &'a Document,
    control: NodeId,
}

impl fmt::Display for Named<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(element) = self.document.element(self.control) else {
            return f.write_str("a control no longer in the page");
        };
        f.write_str(&element.name.local)?;
        match element.attr("name") {
            Some(name) => write!(f, " name={name:?}"),
            None => Ok(()),
        }
    }
}

/// Each control that failed, written as it is said, one after another.
struct Failing<'a> {
    document: &'a Document,
    failed: &'a [Failed],
}

impl fmt::Display for Failing<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (at, one) in self.failed.iter().enumerate() {
            let joint = if at == 0 { "" } else { "; " };
            let named = Named {
                document: self.document,
                control: one.control,
            };
            write!(f, "{joint}{named} {}: {:?}", one.constraint, one.message)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::activation;

    /// A document of `html` at `https://example.com/page`, and the form
    /// and the button its first `<button>` submits.
    fn pressed(html: &str) -> (Document, Url, (NodeId, NodeId)) {
        let document = alo_dom::parse_document(html);
        let url = alo_url::parse("https://example.com/page").expect("a URL");
        let button = document
            .descendants(document.root())
            .find(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html("button"))
            })
            .expect("a button");
        let submits = activation::submits(&document, button).expect("it submits a form");
        (document, url, submits)
    }

    #[test]
    fn a_form_that_passes_its_checks_is_asked_for_with_its_entries() {
        let (document, url, submits) =
            pressed("<form action=/send><input name=a value='b c'><button>Go</button></form>");
        let Unheard::Asked(Ok(navigation)) = unheard(&document, &url, submits) else {
            panic!("the form was not asked for");
        };
        assert_eq!(navigation.url.serialised, "https://example.com/send");
        assert_eq!(navigation.by, By::Browser);
        let submission = navigation.submission.expect("a submission");
        assert_eq!(submission.method, submission::Method::Get);
        assert_eq!(submission.entries.len(), 1);
    }

    #[test]
    fn a_control_that_fails_is_said_by_its_element_its_name_and_chromes_words() {
        let (document, url, submits) = pressed(
            "<form><input name=day type=date required><input required>\
             <button>Go</button></form>",
        );
        assert_eq!(
            unheard(&document, &url, submits),
            Unheard::NotSent(
                "its form was not sent: input name=\"day\" valueMissing: \"Please fill out this \
                 field.\"; input valueMissing: \"Please fill out this field.\""
                    .to_owned()
            )
        );
    }

    #[test]
    fn novalidate_sends_what_fails() {
        for html in [
            "<form novalidate><input name=a required><button>Go</button></form>",
            "<form><input name=a required><button formnovalidate>Go</button></form>",
        ] {
            let (document, url, submits) = pressed(html);
            assert!(
                matches!(unheard(&document, &url, submits), Unheard::Asked(Ok(_))),
                "{html}"
            );
        }
    }

    #[test]
    fn a_constraint_not_built_and_a_form_not_sent_are_said_rather_than_refused() {
        let (document, url, submits) =
            pressed("<form><input name=a pattern=x value=y><button>Go</button></form>");
        assert_eq!(
            unheard(&document, &url, submits),
            Unheard::NotSent(
                "its form was not sent: input name=\"a\" could fail patternMismatch, which this \
                 engine does not check yet"
                    .to_owned()
            )
        );

        let (document, url, submits) = pressed("<form method=dialog><button>Go</button></form>");
        assert!(
            matches!(
                unheard(&document, &url, submits),
                Unheard::NotSent(why) if why.contains("dialogs are not built")
            ),
            "a dialog's form is said"
        );
    }

    #[test]
    fn an_action_that_goes_nowhere_is_asked_and_not_followed() {
        let (document, url, submits) =
            pressed("<form action='http://['><button>Go</button></form>");
        assert!(matches!(
            unheard(&document, &url, submits),
            Unheard::Asked(Err(NotFollowed::ActionUnresolved { .. }))
        ));
    }

    #[test]
    fn a_name_the_page_wrote_cannot_write_a_line_of_its_own() {
        let (document, url, submits) =
            pressed("<form><input name='a&#10;b' required><button>Go</button></form>");
        let Unheard::NotSent(line) = unheard(&document, &url, submits) else {
            panic!("the form was sent");
        };
        assert!(!line.contains('\n'), "{line}");
        assert!(line.contains(r#"name="a\nb""#), "{line}");
    }

    #[test]
    fn what_a_click_on_a_page_that_runs_script_came_to_is_said() {
        let (document, _, (_, button)) =
            pressed("<form><input name=a required><button>Go</button></form>");
        assert_eq!(
            said(
                &Submitted::Asked {
                    method: submission::Method::Get,
                    navigated: true
                },
                Some(&document)
            ),
            None,
            "an ask says itself"
        );
        assert_eq!(
            said(&Submitted::Canceled, Some(&document)).as_deref(),
            Some("its form was not sent: the page cancelled its submit event, and took the click")
        );
        assert_eq!(
            said(
                &Submitted::Unchecked {
                    control: button,
                    constraint: Constraint::PatternMismatch
                },
                Some(&document)
            )
            .as_deref(),
            Some(
                "its form was not sent: button could fail patternMismatch, which this engine \
                 does not check yet"
            )
        );
        assert_eq!(
            said(&Submitted::Unsubmittable("why".to_owned()), None).as_deref(),
            Some("its form was not sent: why")
        );
        assert!(
            said(&Submitted::Withdrawn, None).is_some_and(|line| line.contains("out of its")),
            "a form taken out is said"
        );
        assert!(
            said(&Submitted::AlreadySubmitting, None).is_some_and(|line| line.contains("already")),
            "a form already being submitted is said"
        );
    }

    #[test]
    fn thousands_of_failing_fields_are_one_bounded_line() {
        let fields = "<input required>".repeat(20_000);
        let (document, url, submits) =
            pressed(&format!("<form>{fields}<button>Go</button></form>"));
        let Unheard::NotSent(line) = unheard(&document, &url, submits) else {
            panic!("the form was sent");
        };
        assert!(
            line.chars().count() < crate::said::LONGEST_LINE + 200,
            "{} characters",
            line.len()
        );
        assert!(line.contains("more characters not said"));
    }
}

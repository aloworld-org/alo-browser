/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's `style-src`, asked of its inline style each time it is drawn
//! (ADR 0034 § 2, queue item 343).
//!
//! # At every draw, of every policy the page holds
//!
//! ADR 0033 § 1 reads a `style` attribute every time the page is drawn, so
//! its policy is asked every time too, rather than once when a value
//! arrives: nothing is kept on an element that could go stale. The policies
//! asked are those the page holds **now** — its headers', and every
//! `<meta>` policy the parser has made, kept for the page's life — so a
//! `<meta>` reaches back over inline style written before it. That refuses
//! where other engines applied; the ADR records the cost.
//!
//! - **A `<style>`** presents the nonce its element carries, by the rule a
//!   script's follows ([`alo_dom::nonce`]), and a digest is of its text.
//!   Refused, it contributes no rules.
//! - **A `style` attribute** is asked through the one function
//!   `element.style` asks too ([`alo_bindings::style_policy::applied`]): what
//!   the element's declaration wrote is the page's own and is applied; any
//!   other is applied only when every enforced policy allows it, presenting
//!   no nonce, a digest counting only under `'unsafe-hashes'`. Refused, it
//!   contributes no declarations.
//!
//! Each refusal is said in the drawing's issues, naming the element and the
//! directive. A page under no policy is drawn as it always was, and asks
//! nothing.
//!
//! # Telling the policy's author
//!
//! Each piece of inline style is also asked of every policy the response's
//! **headers** stated, report-only ones included, and each that objects is
//! an [`Objection`] carrying its placement (ADR 0034 § 4) — the first time
//! its element, placement and text are met on this page, and never again
//! ([`crate::objected`]). What any draw found crosses with the next answer
//! that carries objections, and the browser process posts it (queue item
//! 346). A report-only policy refuses nothing, so what it objects to is
//! applied, and that it would have been refused is said as well. What `element.style` wrote is not inline style a policy is
//! asked about at all, so it is not reported.
//!
//! # How much one draw says
//!
//! A page can carry a million `style` attributes. At most
//! [`MOST_SAID_OF_MARKUP`] lines and [`MOST_OBJECTIONS`] objections are kept
//! from one draw, and then how many more there were: a ceiling on what is
//! said is not a ceiling on what is refused.

use std::collections::BTreeSet;

use alo_bindings::style_policy;
use alo_dom::sheets::{Sheet, asked_for};
use alo_dom::{Document, Element, Namespace, NodeId};
use alo_net::Policies;
use alo_net::csp::{Content, Inline, Placement, Refusal};

use crate::objected::Objected;
use crate::said::{self, MOST_SAID_OF_MARKUP};
use crate::violations::{MOST_OBJECTIONS, Objection};

/// What a page's policies make of its inline style, at one draw.
#[derive(Debug, Default)]
pub struct Judged {
    /// Each `<style>` element refused.
    sheets: BTreeSet<NodeId>,
    /// Each element whose `style` attribute is refused.
    attributes: BTreeSet<NodeId>,
    /// Each refusal, and each objection a watched policy made, in words.
    issues: Vec<String>,
    /// How many lines were left out of `issues`.
    unsaid: usize,
    /// The first [`MOST_OBJECTIONS`] objections the headers' policies made.
    objections: Vec<Objection>,
    /// How many were left out of `objections`.
    unobjected: usize,
}

impl Judged {
    /// Nothing refused and nothing objected to: a page under no policy.
    pub fn nothing() -> Self {
        Self::default()
    }

    /// Ask `under` — every policy the page holds, its headers' and its
    /// `<meta>`s' — about each piece of inline style in `document`, and
    /// `stated` — its headers' alone, report-only ones included — which of
    /// them it objects to. An objection is made only the first time
    /// `objected` hears of its element, placement and text, so a page drawn
    /// a thousand times objects to one refused attribute once.
    pub fn of(
        document: &Document,
        under: &Policies,
        stated: &Policies,
        objected: &mut Objected,
    ) -> Self {
        let mut judged = Self::default();
        if under.is_empty() && stated.is_empty() {
            return judged;
        }
        for sheet in asked_for(document) {
            let Sheet::Written {
                text,
                element,
                nonce,
            } = sheet
            else {
                continue;
            };
            let content = Content::element(&text);
            let nonce = nonce.as_deref();
            let refused = under.allows_inline(Inline::Style, nonce, content).err();
            let name = document.element(element).map_or("style", |e| &e.name.local);
            let what = format_args!("the <{name}> {element}");
            judged.verdict(&what, refused.as_ref(), || {
                stated.inline_violations(Inline::Style, nonce, content)
            });
            if refused.is_some() {
                judged.sheets.insert(element);
            }
            let places = stated.objecting_to_inline(Inline::Style, nonce, content);
            if !places.is_empty() && objected.first_time(element, Placement::Element, &text) {
                for place in places {
                    judged.object(place, Placement::Element);
                }
            }
        }
        for id in document.descendants(document.root()) {
            let Some(element) = document.element(id) else {
                continue;
            };
            let Some(text) = styled(element) else {
                continue;
            };
            let refused = style_policy::applied(element, under).err();
            let what = format_args!("the style attribute of <{}> {id}", element.name.local);
            let declared = element.style_is_declared();
            let content = Content::attribute(text);
            judged.verdict(&what, refused.as_ref(), || {
                if declared {
                    Vec::new()
                } else {
                    stated.inline_violations(Inline::Style, None, content)
                }
            });
            if refused.is_some() {
                judged.attributes.insert(id);
            }
            if !declared {
                let places = stated.objecting_to_inline(Inline::Style, None, content);
                if !places.is_empty() && objected.first_time(id, Placement::Attribute, text) {
                    for place in places {
                        judged.object(place, Placement::Attribute);
                    }
                }
            }
        }
        judged
    }

    /// Whether the `<style>` element `id` is applied.
    pub fn applies_sheet(&self, id: NodeId) -> bool {
        !self.sheets.contains(&id)
    }

    /// Whether the `style` attribute of element `id` is applied.
    pub fn applies_attribute(&self, id: NodeId) -> bool {
        !self.attributes.contains(&id)
    }

    /// What this draw has to say: each refusal and each watched policy's
    /// objection, and how many more there were.
    pub fn issues(&self) -> Vec<String> {
        let mut issues = self.issues.clone();
        if self.unsaid > 0 {
            issues.push(format!(
                "{} more things this page's policies said about its inline style were not \
                 said: one drawing says at most {MOST_SAID_OF_MARKUP}",
                self.unsaid
            ));
        }
        issues
    }

    /// The objections the headers' policies made, and how many more there
    /// were than one draw keeps.
    pub fn objections(&self) -> (&[Objection], usize) {
        (&self.objections, self.unobjected)
    }

    /// Say what became of one piece of inline style: refused, or applied
    /// though a watched policy objected.
    fn verdict<W: Fn() -> Vec<alo_net::csp_report::Violation>>(
        &mut self,
        what: &dyn core::fmt::Display,
        refused: Option<&Refusal>,
        watched: W,
    ) {
        if let Some(refusal) = refused {
            self.say(&format_args!("{what} is not applied: {refusal}"));
        } else {
            for violation in watched() {
                self.say(&format_args!("{what} is applied, but {violation}"));
            }
        }
    }

    /// Say one line, if there is room.
    fn say(&mut self, line: &dyn core::fmt::Display) {
        if self.issues.len() < MOST_SAID_OF_MARKUP {
            self.issues.push(said::line(line));
        } else {
            self.unsaid = self.unsaid.saturating_add(1);
        }
    }

    /// Keep one objection, if there is room.
    fn object(&mut self, policy: usize, placement: Placement) {
        if self.objections.len() < MOST_OBJECTIONS {
            self.objections.push(Objection {
                policy,
                kind: Inline::Style,
                placement,
            });
        } else {
            self.unobjected = self.unobjected.saturating_add(1);
        }
    }
}

/// The `style` attribute of an HTML or SVG element — the only elements
/// whose `style` is read ([`alo_style::attached`]).
fn styled(element: &Element) -> Option<&str> {
    if matches!(element.name.ns, Namespace::Html | Namespace::Svg) {
        element.attr("style")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;
    use alo_net::Headers;

    fn enforced(policy: &str) -> Policies {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy", policy);
        Policies::stated_by(&headers)
    }

    #[test]
    fn under_no_policy_nothing_is_asked_and_everything_applies() {
        let document = parse_document(r#"<style>p{}</style><p style="color: red">x</p>"#);
        let judged = Judged::of(
            &document,
            &Policies::none(),
            &Policies::none(),
            &mut Objected::new(),
        );
        assert!(judged.issues().is_empty());
        assert!(judged.sheets.is_empty() && judged.attributes.is_empty());
    }

    #[test]
    fn a_style_and_a_style_attribute_are_refused_and_said() {
        let document = parse_document(r#"<style>p{}</style><p style="color: red">x</p>"#);
        let policy = enforced("style-src 'self'");
        let judged = Judged::of(&document, &policy, &policy, &mut Objected::new());
        assert_eq!(judged.sheets.len(), 1);
        assert_eq!(judged.attributes.len(), 1);
        let issues = judged.issues();
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(issues[0].starts_with("the <style> #"), "{issues:?}");
        assert!(
            issues[0].contains("does not allow inline style"),
            "{issues:?}"
        );
        assert!(
            issues[1].starts_with("the style attribute of <p> #"),
            "{issues:?}"
        );
        assert!(issues[1].contains("style-src"), "{issues:?}");
        let (objections, more) = judged.objections();
        assert_eq!(more, 0);
        assert_eq!(
            objections.iter().map(|o| o.placement).collect::<Vec<_>>(),
            vec![Placement::Element, Placement::Attribute],
        );
    }

    #[test]
    fn a_style_presenting_the_policys_nonce_is_applied() {
        let document = parse_document("<style nonce=n1>p{}</style><style nonce=n2>p{}</style>");
        let policy = enforced("style-src 'nonce-n1'");
        let judged = Judged::of(&document, &policy, &policy, &mut Objected::new());
        assert_eq!(judged.sheets.len(), 1, "only the second is refused");
        assert_eq!(judged.objections().0.len(), 1);
    }

    #[test]
    fn what_the_declaration_wrote_is_applied_and_not_reported() {
        let mut document = parse_document("<p>x</p>");
        let p = document
            .descendants(document.root())
            .find(|id| document.get(*id).is_some_and(|n| n.is_html_element("p")))
            .unwrap_or_else(|| panic!("a <p>"));
        document.set_declared_style(p, "color: blue;");
        let policy = enforced("style-src 'none'");
        let judged = Judged::of(&document, &policy, &policy, &mut Objected::new());
        assert!(judged.applies_attribute(p));
        assert!(judged.issues().is_empty(), "{:?}", judged.issues());
        assert!(judged.objections().0.is_empty());
    }

    /// A page's markup and a server's policy are both a stranger's: whatever
    /// they hold, each is judged and nothing panics.
    #[test]
    fn hostile_style_and_hostile_policies_are_judged_without_trouble() {
        let huge = "a".repeat(1 << 20);
        let markup = format!(
            "<p style=\"{huge}\">x</p><p style=\"\u{0}; color: red !important\">x</p>\
             <p style=\"color: rgb(((((\">x</p><p style>x</p>\
             <style nonce=\"<style\">p{{}}</style><style>}}}}{{{{</style>\
             <svg><rect style=\"fill: red\"/></svg><math><mi style=\"color: red\">x</mi></math>"
        );
        let document = parse_document(&markup);
        for policy in [
            "style-src 'unsafe-hashes' 'sha256-'",
            "style-src 'nonce-'",
            "style-src \u{0}",
            "default-src 'none'; style-src",
            "style-src 'unsafe-inline' 'nonce-x'",
        ] {
            let policies = enforced(policy);
            let judged = Judged::of(&document, &policies, &policies, &mut Objected::new());
            assert!(
                judged.attributes.len() <= 5,
                "a MathML element's style is never read"
            );
        }
    }

    #[test]
    fn a_flood_is_bounded_and_counted() {
        let markup = r#"<p style="color: red">x</p>"#.repeat(MOST_SAID_OF_MARKUP + 10);
        let document = parse_document(&markup);
        let policy = enforced("style-src 'none'");
        let judged = Judged::of(&document, &policy, &policy, &mut Objected::new());
        assert_eq!(judged.attributes.len(), MOST_SAID_OF_MARKUP + 10);
        let issues = judged.issues();
        assert_eq!(issues.len(), MOST_SAID_OF_MARKUP + 1);
        assert!(
            issues
                .last()
                .is_some_and(|line| line.starts_with("10 more")),
            "{:?}",
            issues.last()
        );
        let (objections, more) = judged.objections();
        assert_eq!(objections.len(), MOST_OBJECTIONS);
        assert_eq!(more, MOST_SAID_OF_MARKUP + 10 - MOST_OBJECTIONS);
    }

    #[test]
    fn a_second_draw_refuses_again_and_objects_to_nothing_new() {
        let document = parse_document(r#"<style>p{}</style><p style="color: red">x</p>"#);
        let policy = enforced("style-src 'self'");
        let mut objected = Objected::new();
        let first = Judged::of(&document, &policy, &policy, &mut objected);
        assert_eq!(first.objections().0.len(), 2);
        let second = Judged::of(&document, &policy, &policy, &mut objected);
        assert_eq!(second.sheets.len(), 1, "a refusal is made at every draw");
        assert_eq!(second.attributes.len(), 1);
        assert_eq!(second.issues().len(), 2, "and said at every draw");
        assert_eq!(second.objections(), (&[][..], 0), "but objected to once");
    }
}

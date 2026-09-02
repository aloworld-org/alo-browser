//! What a style sheet asked for that we did not do, and why.
//!
//! A renderer that silently drops what it does not understand is a renderer
//! nobody can debug: the page is wrong, everything parsed, and there is
//! nothing to read. So every refusal is recorded with the text that caused it
//! and the place it was written.
//!
//! This is the same bargain `alo_dom::ParseIssue` makes for HTML, and for the
//! same reason.

use core::fmt;

/// Where in a style sheet something was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// The line, counting from one.
    pub line: u32,
    /// The column, counting from one.
    pub column: u32,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Something a style sheet asked for that this engine did not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleIssue {
    /// What happened.
    pub kind: IssueKind,
    /// The text that caused it, as written.
    pub source: String,
    /// Where it was written.
    pub at: Location,
}

/// The kinds of refusal a style sheet can meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    /// A selector this engine cannot parse. The whole rule is dropped, which
    /// is what CSS says to do with an invalid selector — a rule with a
    /// selector nobody can evaluate would match either everything or nothing,
    /// and both are worse than not being there.
    InvalidSelector,
    /// A selector that names a pseudo-element. It is kept and never matched:
    /// stage 1 produces no boxes for pseudo-elements, so a rule that targets
    /// one has nothing to apply to.
    PseudoElementNotProduced,
    /// A declaration whose value could not be tokenised at all. Dropped, per
    /// CSS's own error handling.
    InvalidDeclaration,
    /// An at-rule this engine does not implement. Kept, in full, so a later
    /// stage can implement it without re-parsing the sheet.
    UnknownAtRule,
    /// A media condition this engine cannot evaluate. The rules inside it are
    /// kept and the condition is treated as not matching, because applying
    /// rules whose condition is unknown is how a dark theme leaks into a light
    /// one.
    UnknownMediaCondition,
}

impl IssueKind {
    /// What happened, in words.
    pub fn as_str(self) -> &'static str {
        match self {
            IssueKind::InvalidSelector => "invalid selector, rule dropped",
            IssueKind::PseudoElementNotProduced => {
                "selector names a pseudo-element, which stage 1 does not produce"
            }
            IssueKind::InvalidDeclaration => "invalid declaration, dropped",
            IssueKind::UnknownAtRule => "at-rule not implemented, kept unparsed",
            IssueKind::UnknownMediaCondition => {
                "media condition not understood, treated as not matching"
            }
        }
    }
}

impl fmt::Display for StyleIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.at, self.kind.as_str(), self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_issue_says_what_where_and_from_what_text() {
        let issue = StyleIssue {
            kind: IssueKind::InvalidSelector,
            source: "p::-webkit-thing".to_owned(),
            at: Location { line: 3, column: 5 },
        };
        assert_eq!(
            issue.to_string(),
            "3:5: invalid selector, rule dropped: p::-webkit-thing",
        );
    }

    #[test]
    fn every_kind_has_something_to_say() {
        for kind in [
            IssueKind::InvalidSelector,
            IssueKind::PseudoElementNotProduced,
            IssueKind::InvalidDeclaration,
            IssueKind::UnknownAtRule,
            IssueKind::UnknownMediaCondition,
        ] {
            assert!(!kind.as_str().is_empty());
        }
    }
}

//! `text/html; charset=utf-8`, in parts.
//!
//! A media type decides what a browser does with bytes — parse them as a page,
//! decode them as a picture, or offer them as a download — and it carries the
//! declared character encoding along with it. Both halves are read here, and
//! both are read leniently, because what servers actually send is not always
//! what the grammar says.

use core::fmt;

/// What a `Content-Type` says these bytes are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaType {
    /// `text`, lowercased.
    pub kind: String,
    /// `html`, lowercased.
    pub subtype: String,
    /// The parameters, names lowercased, in the order they were written.
    pub parameters: Vec<(String, String)>,
}

impl MediaType {
    /// Read a `Content-Type` value.
    ///
    /// [`None`] for anything without a `type/subtype`, which is what a caller
    /// treats as "nobody said" rather than as an error — a missing content
    /// type and an unreadable one lead to the same sniffing either way.
    pub fn parse(text: &str) -> Option<Self> {
        let mut pieces = text.split(';');
        let essence = pieces.next()?.trim();
        let (kind, subtype) = essence.split_once('/')?;
        let kind = kind.trim();
        let subtype = subtype.trim();
        if kind.is_empty() || subtype.is_empty() {
            return None;
        }
        let mut parameters = Vec::new();
        for piece in pieces {
            let Some((name, value)) = piece.split_once('=') else {
                continue;
            };
            let value = value.trim();
            // A quoted value is common and the quotes are not part of it.
            let value = value
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or(value);
            parameters.push((name.trim().to_ascii_lowercase(), value.to_owned()));
        }
        Some(Self {
            kind: kind.to_ascii_lowercase(),
            subtype: subtype.to_ascii_lowercase(),
            parameters,
        })
    }

    /// The value of a parameter, ignoring case in its name.
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.as_str())
    }

    /// The declared character encoding, if there is one.
    pub fn charset(&self) -> Option<&str> {
        self.parameter("charset")
    }

    /// `text/html`, without the parameters.
    pub fn essence(&self) -> String {
        format!("{}/{}", self.kind, self.subtype)
    }

    /// Whether these bytes are markup this engine would parse as a page.
    pub fn is_html(&self) -> bool {
        self.essence() == "text/html"
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.kind, self.subtype)?;
        for (name, value) in &self.parameters {
            write!(f, "; {name}={value}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_and_a_subtype_are_the_whole_of_what_is_needed() {
        let read = MediaType::parse("text/html").expect("a media type");
        assert_eq!(read.kind, "text");
        assert_eq!(read.subtype, "html");
        assert!(read.is_html());
        assert_eq!(read.charset(), None);
    }

    #[test]
    fn case_and_space_are_not_part_of_what_it_says() {
        let read = MediaType::parse("  TEXT/HTML ; CharSet = UTF-8  ").expect("a media type");
        assert_eq!(read.essence(), "text/html");
        assert_eq!(read.charset(), Some("UTF-8"), "the value keeps its case");
        assert!(read.is_html());
    }

    #[test]
    fn a_quoted_parameter_is_not_quoted_when_it_is_read() {
        let read = MediaType::parse("text/html; charset=\"utf-8\"").expect("a media type");
        assert_eq!(read.charset(), Some("utf-8"));
    }

    #[test]
    fn what_is_not_a_media_type_is_nobody_having_said() {
        for text in ["", "   ", "html", "/html", "text/", ";charset=utf-8"] {
            assert_eq!(MediaType::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_parameter_nobody_can_read_is_skipped_and_the_rest_survives() {
        // Servers send things the grammar does not allow, and losing the
        // charset because of a stray semicolon would be the worst answer.
        let read = MediaType::parse("text/html; ; charset=utf-8; boundary").expect("a media type");
        assert_eq!(read.charset(), Some("utf-8"));
    }

    #[test]
    fn it_writes_back_out_as_something_that_reads_the_same() {
        let read = MediaType::parse("Text/HTML; charset=utf-8").expect("a media type");
        assert_eq!(read.to_string(), "text/html; charset=utf-8");
        assert_eq!(MediaType::parse(&read.to_string()), Some(read));
    }
}

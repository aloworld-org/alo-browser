//! The headers of a request or a response.
//!
//! Not a map. Header names are case-insensitive but header *order* is
//! observable, a name may legitimately appear more than once (`Set-Cookie`
//! most of all), and a `HashMap<String, String>` quietly loses both. What is
//! here is the list, with lookup that folds case.

use core::fmt;

/// One header, as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// The name, as it was written — not lowercased, because a person reading
    /// a log should see what came over the wire.
    pub name: String,
    /// The value, with the surrounding space taken off.
    pub value: String,
}

/// The headers of a message, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Headers {
    held: Vec<Header>,
}

impl Headers {
    /// No headers at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one, keeping any that are already there.
    ///
    /// **Adds rather than replaces**, because `Set-Cookie` appearing three
    /// times means three cookies and a replacing `insert` would silently keep
    /// one.
    pub fn add(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.held.push(Header {
            name: name.into(),
            value: value.into().trim().to_owned(),
        });
    }

    /// The first value with this name, ignoring case.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.held
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    /// Every value with this name, in order.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.held
            .iter()
            .filter(move |header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    /// Every header, in the order it arrived.
    pub fn iter(&self) -> impl Iterator<Item = &Header> {
        self.held.iter()
    }

    /// How many there are, repeats counted separately.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

impl fmt::Display for Headers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for header in &self.held {
            writeln!(f, "{}: {}", header.name, header.value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_found_however_it_was_capitalised() {
        let mut headers = Headers::new();
        headers.add("Content-Type", "text/html");
        assert_eq!(headers.get("content-type"), Some("text/html"));
        assert_eq!(headers.get("CONTENT-TYPE"), Some("text/html"));
        assert_eq!(headers.get("Content-Length"), None);
    }

    #[test]
    fn a_repeated_header_keeps_every_one_of_them() {
        // `Set-Cookie` three times means three cookies. A map would keep one.
        let mut headers = Headers::new();
        headers.add("Set-Cookie", "a=1");
        headers.add("Set-Cookie", "b=2");
        headers.add("set-cookie", "c=3");
        assert_eq!(headers.all("Set-Cookie").count(), 3);
        assert_eq!(headers.get("Set-Cookie"), Some("a=1"), "the first is first");
        assert_eq!(headers.len(), 3);
    }

    #[test]
    fn the_order_they_arrived_in_survives() {
        let mut headers = Headers::new();
        headers.add("B", "2");
        headers.add("A", "1");
        let names: Vec<&str> = headers.iter().map(|header| header.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["B", "A"],
            "not sorted, because order is observable"
        );
    }

    #[test]
    fn the_space_around_a_value_is_not_part_of_it() {
        let mut headers = Headers::new();
        headers.add("Content-Type", "  text/html  ");
        assert_eq!(headers.get("Content-Type"), Some("text/html"));
        assert!(Headers::new().is_empty());
    }
}

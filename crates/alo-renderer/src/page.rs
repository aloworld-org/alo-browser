//! A page to render: what the browser process sends.
//!
//! Owned, whole, and enough on its own. A renderer is *given* a page rather
//! than told where to find one, because ADR 0005 gives it no way to find
//! anything: no filesystem, no network, no name for anything outside itself.
//! Fetching is the browser process's, and that is a privilege boundary rather
//! than a division of labour.

use alo_css::ColorScheme;
use alo_layout::Size;

/// Everything needed to render one page.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The markup.
    pub html: String,
    /// The author's style sheets, in the order they were written.
    ///
    /// A list rather than one string: order decides the cascade where
    /// specificity ties, and joining them would lose which sheet a rule came
    /// from the moment we want to say so.
    pub sheets: Vec<String>,
    /// How big the window is.
    pub viewport: Size,
    /// Light or dark, which the browser process knows and a page does not.
    pub scheme: ColorScheme,
}

impl Page {
    /// A page of markup, at a size, in the light.
    ///
    /// The common case, and the one every test wants; anything else is set on
    /// the value afterwards.
    pub fn new(html: impl Into<String>, viewport: Size) -> Self {
        Self {
            html: html.into(),
            sheets: Vec::new(),
            viewport,
            scheme: ColorScheme::Light,
        }
    }

    /// The same page with a style sheet added.
    #[must_use]
    pub fn with_sheet(mut self, css: impl Into<String>) -> Self {
        self.sheets.push(css.into());
        self
    }

    /// The same page in the dark.
    #[must_use]
    pub fn in_the_dark(mut self) -> Self {
        self.scheme = ColorScheme::Dark;
        self
    }

    /// A page from something that was fetched.
    ///
    /// **The fetching happened elsewhere**, and that is the whole point:
    /// ADR 0005 gives a renderer no filesystem and no network, so it is handed
    /// bytes rather than a place to go and get them. The response decides the
    /// character encoding — from its own byte order mark, its `Content-Type`,
    /// or a `<meta>` in the markup — because a renderer told "here is a
    /// string" has already lost the chance to get that right.
    pub fn from_response(response: &alo_net::Response, viewport: Size) -> Self {
        Self {
            html: response.text().text,
            sheets: Vec::new(),
            viewport,
            scheme: ColorScheme::Light,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_markup_and_a_size_and_nothing_it_has_to_go_and_find() {
        let page = Page::new("<p>hello</p>", Size::new(800.0, 600.0));
        assert_eq!(page.html, "<p>hello</p>");
        assert!(page.sheets.is_empty());
        assert_eq!(page.scheme, ColorScheme::Light);
    }

    #[test]
    fn sheets_keep_the_order_they_were_added_in() {
        let page = Page::new("", Size::new(1.0, 1.0))
            .with_sheet("a { color: red }")
            .with_sheet("a { color: blue }");
        assert_eq!(page.sheets.len(), 2);
        assert!(
            page.sheets
                .first()
                .is_some_and(|sheet| sheet.contains("red"))
        );
        assert!(
            page.sheets
                .get(1)
                .is_some_and(|sheet| sheet.contains("blue"))
        );
    }

    #[test]
    fn the_dark_is_the_browser_processs_to_know() {
        assert_eq!(
            Page::new("", Size::new(1.0, 1.0)).in_the_dark().scheme,
            ColorScheme::Dark,
        );
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page to render: what the browser process sends.
//!
//! Owned, whole, and enough on its own. A renderer is *given* a page rather
//! than told where to find one, because ADR 0005 gives it no way to find
//! anything: no filesystem, no network, no name for anything outside itself.
//! Fetching is the browser process's, and that is a privilege boundary rather
//! than a division of labour.

use alo_css::ColorScheme;
use alo_layout::Size;
use alo_url::Url;

/// Everything needed to render one page.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Where the page is: the URL of the response it came from, stated by
    /// the browser process (ADR 0020 § 2).
    ///
    /// The one fact about its own address a page is entitled to, and the
    /// one a renderer needs to resolve a link against — the browser process
    /// keeps its own copy, and never takes a renderer's word for it.
    /// `about:blank` for a page nobody fetched.
    pub url: Url,
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
    /// Every `Content-Security-Policy` header the response carried, as it
    /// carried them.
    ///
    /// The renderer is where a page's inline script is found and run, so it is
    /// where the page's policy has to be asked whether it may be (queue item
    /// 236) — running a page's script without its policy would be running
    /// script its author forbade. The text crosses rather than a parsed
    /// policy, and the renderer parses it with the same `alo-net` rules the
    /// browser process uses: a policy is a stranger's sentence either way, and
    /// the rule that one this engine cannot read makes things stricter lives in
    /// one place.
    ///
    /// Only the enforced header: a report-only policy forbids nothing, and is
    /// [`Page::watching`].
    pub policies: Vec<String>,
    /// Every `Content-Security-Policy-Report-Only` header, as it was carried.
    ///
    /// It forbids nothing, so nothing here decides whether a script runs by
    /// it. It crosses because its author still wants to be told what it
    /// *would* have refused — the whole point of watching a policy for a week
    /// before enforcing it — and only the renderer sees the inline script it
    /// would have refused. The renderer names the objecting policy by its
    /// place ([`Page::stated`]); the browser process writes and posts the
    /// report (queue item 237, [`crate::violations`]).
    pub watching: Vec<String>,
}

impl Page {
    /// A page of markup, at a size, in the light.
    ///
    /// The common case, and the one every test wants; anything else is set on
    /// the value afterwards.
    pub fn new(html: impl Into<String>, viewport: Size) -> Self {
        Self {
            url: Url::about_blank(),
            html: html.into(),
            sheets: Vec::new(),
            viewport,
            scheme: ColorScheme::Light,
            policies: Vec::new(),
            watching: Vec::new(),
        }
    }

    /// The same page, at `url`.
    #[must_use]
    pub fn at(mut self, url: Url) -> Self {
        self.url = url;
        self
    }

    /// The same page with a style sheet added.
    #[must_use]
    pub fn with_sheet(mut self, css: impl Into<String>) -> Self {
        self.sheets.push(css.into());
        self
    }

    /// The same page under one more `Content-Security-Policy`.
    #[must_use]
    pub fn with_policy(mut self, policy: impl Into<String>) -> Self {
        self.policies.push(policy.into());
        self
    }

    /// The same page with one more `Content-Security-Policy-Report-Only`.
    #[must_use]
    pub fn watched_by(mut self, policy: impl Into<String>) -> Self {
        self.watching.push(policy.into());
        self
    }

    /// Every policy the response's headers stated, enforced and watched, in
    /// the order [`alo_net::Policies::stated_by`] reads them.
    ///
    /// **The list a violation is named against.** A renderer says *the policy
    /// at this place objected* and the browser process, holding the same
    /// page, reads the same place in the same list — so both must be made by
    /// this one function from the same two fields, and a `<meta>` policy,
    /// which only the renderer has seen, is never in it.
    pub fn stated(&self) -> alo_net::Policies {
        let mut headers = alo_net::Headers::new();
        for policy in &self.policies {
            headers.add("Content-Security-Policy", policy.as_str());
        }
        for policy in &self.watching {
            headers.add("Content-Security-Policy-Report-Only", policy.as_str());
        }
        alo_net::Policies::stated_by(&headers)
    }

    /// The policies this page is under, parsed.
    ///
    /// Each header is its own policy and a page is under all of them at once —
    /// an intersection, so a second header can only narrow the first.
    pub fn policies(&self) -> alo_net::Policies {
        Self::policies_of(&self.policies)
    }

    /// Policies' text, parsed as though each were a `Content-Security-Policy`
    /// header — which is also what a `<meta>` policy is read as.
    pub fn policies_of(texts: &[String]) -> alo_net::Policies {
        let mut headers = alo_net::Headers::new();
        for policy in texts {
            headers.add("Content-Security-Policy", policy.as_str());
        }
        alo_net::Policies::stated_by(&headers)
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
            url: response.url.clone(),
            html: response.text().text,
            sheets: Vec::new(),
            viewport,
            scheme: ColorScheme::Light,
            policies: response
                .headers
                .all("Content-Security-Policy")
                .map(ToOwned::to_owned)
                .collect(),
            watching: response
                .headers
                .all("Content-Security-Policy-Report-Only")
                .map(ToOwned::to_owned)
                .collect(),
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
        assert_eq!(page.url.serialised, "about:blank");
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
    fn a_page_is_under_every_policy_its_response_stated_and_no_other() {
        let url = alo_url::parse("https://example.com/").expect("a URL");
        let mut response = alo_net::Response::ok(url, b"<p>hello</p>".to_vec());
        response
            .headers
            .add("Content-Security-Policy", "script-src 'self'");
        response
            .headers
            .add("Content-Security-Policy", "default-src 'none'");
        response
            .headers
            .add("Content-Security-Policy-Report-Only", "script-src 'none'");
        let page = Page::from_response(&response, Size::new(1.0, 1.0));
        assert_eq!(page.url.serialised, "https://example.com/");
        assert_eq!(
            page.policies,
            vec![
                "script-src 'self'".to_owned(),
                "default-src 'none'".to_owned()
            ],
        );
        assert_eq!(page.policies().len(), 2);
        assert_eq!(
            page.watching,
            vec!["script-src 'none'".to_owned()],
            "a report-only policy was enforced or lost",
        );
        assert_eq!(
            page.stated().len(),
            3,
            "the list a violation is named against left one out"
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

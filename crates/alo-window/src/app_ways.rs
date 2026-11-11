/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An app's own named ways of taking words, for an app that draws its board
//! on a canvas (ADR 0109 § 1, queue items 941 and 942).
//!
//! diagrams.net and Excalidraw draw their boards where Chrome's tree does
//! not reach: a box on the board is not a thing an agent can name. Each has
//! a dialog of its own that takes the board as words, reached by menus the
//! tree names — and a model that has not met the app does not always find
//! it. Measured on 11 November 2026 with alo's own agent, told nothing of
//! the app: on excalidraw.com *Draw a flowchart of the order process* found
//! *More tools* › *Mermaid to Excalidraw* in one run and was stuck in the
//! next (`handrun60/`, ADR 0109 amended).
//!
//! So the agent is told the way **in alo's words, as a hint**: menus and
//! fields by the names the app's own tree gives them, which the agent
//! still finds and presses by name, through the same verbs and the same
//! gate as anything else. Nothing here presses anything, runs anything in
//! the page, or reads what the canvas holds. A way that is wrong, or that
//! the app has renamed, is a hint the agent does not find on the page —
//! never a press of something else.
//!
//! A way is offered only on the app's own address, over `https`.

use alo_url::Url;

/// diagrams.net's: its diagram as XML in a dialog, and a label renamed by
/// its own *Find/Replace*.
pub const DIAGRAMS: &str = "diagrams.net holds the whole diagram as XML in Extras > Edit \
Diagram..., where it is read, changed and saved with Apply; a box's label can also be renamed \
with Edit > Find/Replace. In a narrow window it hides its Extras and Edit menus.";

/// Excalidraw's: a diagram drawn from a Mermaid definition — never its
/// *Text to diagram*, which one run told only of *Mermaid to Excalidraw*
/// chose while the Mermaid field was still loading, and which sends the
/// words to the app's own AI service (`handrun60/excalidraw-hinted-1`).
pub const EXCALIDRAW: &str = "Excalidraw draws a diagram from text: More tools > Mermaid to \
Excalidraw, the definition put into its field once the field is there, then Insert. Its Text to \
diagram sends the words to Excalidraw's own AI service: do not use it unless the person asks. \
What it draws is on a canvas and is not in the page you are sent, so say what you inserted, never \
that you saw it.";

/// The app's own ways of taking words at `url`, or none.
pub fn of(url: &Url) -> Vec<&'static str> {
    if url.scheme != "https" {
        return Vec::new();
    }
    match alo_url::reading::hostname(url).as_str() {
        "app.diagrams.net" | "embed.diagrams.net" => vec![DIAGRAMS],
        "excalidraw.com" => vec![EXCALIDRAW],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ways(address: &str) -> Vec<&'static str> {
        of(&alo_url::parse(address).unwrap())
    }

    #[test]
    fn a_canvas_app_is_told_its_own_way_and_nothing_else_is() {
        assert_eq!(
            ways("https://app.diagrams.net/?splash=0#R..."),
            vec![DIAGRAMS]
        );
        assert_eq!(ways("https://excalidraw.com/"), vec![EXCALIDRAW]);
        for address in [
            "http://excalidraw.com/",
            "https://excalidraw.com.example/",
            "https://diagrams.net/",
            "https://docs.google.com/document/d/abc/edit",
            "https://example.com/",
        ] {
            assert_eq!(ways(address), Vec::<&str>::new(), "{address}");
        }
    }
}

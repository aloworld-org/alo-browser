/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 244, found while building 243: a family no font could have is
//! not asked for, and that is said.
//!
//! *A page naming a family of tens of megabytes loads and its answer crosses
//! the wire, with what was not asked for said.*
//!
//! The families a load asks for were at most sixty-four, each as long as the
//! page wrote it. No font this engine reads states a family longer than
//! [`LONGEST_FAMILY`] characters, so a longer name could only be answered *not
//! here* — and carrying it cost the answer the whole of it. Around the closing
//! clause: the ceiling is in characters and exactly at the longest a font can
//! have, a name too long does not stop the page's next choice being asked
//! for, the browser process refuses one a renderer sent anyway, and every
//! prefix of a page naming one stays sendable.

use alo_css::media::ColorScheme;
use alo_layout::Size;
use alo_renderer::families::LONGEST_FAMILY;
use alo_renderer::host::Renderers;
use alo_renderer::said::{LONGEST_LINE, MOST_SAID_OF_MARKUP};
use alo_renderer::site::Site;
use alo_renderer::wire::{
    LARGEST_MESSAGE, read_from_renderer, write_from_renderer, write_to_renderer,
};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

const RENDERER: &str = env!("CARGO_BIN_EXE_alo-render");

/// A small window; nothing here is about where anything is drawn.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// A page whose one paragraph asks for these families and nothing else.
fn asking_for(families: &str, after: &str) -> Page {
    Page::new(format!("<!doctype html><p>text</p>{after}"), WINDOW)
        .with_sheet(format!("p {{ font-family: {families} }}"))
}

/// Load a page into a fresh renderer holding no fonts at all.
fn load(page: Page) -> FromRenderer {
    Renderer::new(FontDatabase::new()).handle(ToRenderer::Load(Box::new(page)))
}

/// The families an answer asked for and the lines it said.
fn asked_and_said(answer: &FromRenderer) -> (Vec<String>, Vec<String>) {
    match answer {
        FromRenderer::Loaded { wanted, issues, .. } => (wanted.clone(), issues.clone()),
        other => (Vec::new(), vec![format!("not loaded: {other:?}")]),
    }
}

/// How a name too long to ask for is said, by its first 32 characters.
fn too_long(beginning: &str, characters: usize) -> String {
    format!("a family beginning {beginning:?} and {characters} characters long")
}

/// The line a load says about a name it did not ask for.
fn not_asked(beginning: &str, characters: usize) -> String {
    format!(
        "{} was not asked for: no font states a family of more than {LONGEST_FAMILY} characters",
        too_long(beginning, characters)
    )
}

/// The line a load holding no fonts says about text it could not draw.
fn nothing_to_draw_in(named: &str) -> String {
    format!("nothing here is {named}, and there is no font at all to draw text in")
}

/// That the answer crosses the wire: under the cap, and read back as it was
/// sent.
fn crosses(answer: &FromRenderer) {
    let bytes = write_from_renderer(answer);
    assert!(
        bytes.len() <= LARGEST_MESSAGE,
        "{} bytes, against {LARGEST_MESSAGE}",
        bytes.len()
    );
    assert_eq!(read_from_renderer(&bytes).as_ref(), Ok(answer));
}

/// The closing clause, on a page built to need it. The name is sixty-three
/// mebibytes, and 254 pictures each make the longest line a load says — 1640
/// control characters of `src`, quoted at five characters each — so the page
/// crosses into the renderer under the wire's cap and, before this item, its
/// answer did not cross back: measured, 66479993 bytes in and 68167392 out,
/// against 67108864.
#[test]
fn a_family_tens_of_megabytes_long_is_not_asked_for_and_the_answer_crosses() {
    const NAME: usize = LARGEST_MESSAGE - 1024 * 1024;
    const PICTURES: usize = MOST_SAID_OF_MARKUP - 2;
    let pictures = format!("<img src=\"{}\">", "\u{1}".repeat(1640)).repeat(PICTURES);
    let page = asking_for(&format!("\"{}\"", "a".repeat(NAME)), &pictures);
    let message = ToRenderer::Load(Box::new(page));
    assert!(
        write_to_renderer(&message).len() <= LARGEST_MESSAGE,
        "the page itself must be one that could arrive"
    );

    let answer = Renderer::new(FontDatabase::new()).handle(message);
    let (asked, said) = asked_and_said(&answer);
    assert!(asked.is_empty(), "{} families asked for", asked.len());
    assert_eq!(
        said.len(),
        MOST_SAID_OF_MARKUP,
        "nothing left out uncounted"
    );
    assert!(
        said.iter().take(PICTURES).all(|line| {
            line.starts_with("no picture was loaded for ") && line.chars().count() > LONGEST_LINE
        }),
        "every picture says a line as long as a line may be"
    );
    let beginning = "a".repeat(32);
    assert_eq!(
        &said[PICTURES..],
        &[
            not_asked(&beginning, NAME),
            nothing_to_draw_in(&too_long(&beginning, NAME)),
        ],
    );
    crosses(&answer);
}

/// The ceiling is the longest family a font can state, and it is counted in
/// characters: a `name` record of 512 bytes decodes to at most 512, and to
/// exactly that in a one-byte encoding — so 512 `é`, which are 1024 bytes
/// here, could still be a font's family and are asked for.
#[test]
fn a_name_as_long_as_a_font_s_can_be_is_asked_for_and_one_longer_is_not() {
    for longest in ["a".repeat(LONGEST_FAMILY), "é".repeat(LONGEST_FAMILY)] {
        let (asked, said) = asked_and_said(&load(asking_for(&format!("\"{longest}\""), "")));
        assert_eq!(asked, vec![longest.clone()]);
        assert_eq!(said, vec![nothing_to_draw_in(&format!("{longest:?}"))]);
    }

    let longer = "é".repeat(LONGEST_FAMILY + 1);
    let (asked, said) = asked_and_said(&load(asking_for(&format!("\"{longer}\""), "")));
    assert!(asked.is_empty(), "{asked:?}");
    let beginning = "é".repeat(32);
    assert_eq!(
        said,
        vec![
            not_asked(&beginning, LONGEST_FAMILY + 1),
            nothing_to_draw_in(&too_long(&beginning, LONGEST_FAMILY + 1)),
        ],
    );
}

/// A name too long to ask for is skipped, not the end of the list: the
/// page's next choice may be on this machine, and is asked for.
#[test]
fn a_name_too_long_does_not_stop_the_next_choice_being_asked_for() {
    let long = "b".repeat(600);
    let (asked, said) = asked_and_said(&load(asking_for(
        &format!("\"{long}\", Inter, \"{long}\""),
        "",
    )));
    assert_eq!(asked, vec!["Inter".to_owned()]);
    let beginning = "b".repeat(32);
    assert_eq!(
        said,
        vec![
            not_asked(&beginning, 600),
            nothing_to_draw_in(&format!("{} or \"Inter\"", too_long(&beginning, 600))),
        ],
        "named twice in one list, it is one family and said once"
    );
}

/// The browser process bounds what a renderer sends it, as it bounds how
/// many: a renderer parsed a hostile page, so a limit it applied to itself is
/// not one the other side may rely on. A name no font could have is answered
/// absent, as it was sent.
#[test]
fn the_browser_process_answers_a_name_no_font_could_have_absent() {
    let mut renderers = Renderers::running(RENDERER, &[]);
    let url = alo_url::parse("https://example.com/");
    assert!(url.is_ok(), "{url:?}");
    let Ok(url) = url else { return };
    let site = Site::of(&url);
    let sent = vec!["c".repeat(LONGEST_FAMILY + 1), "d".repeat(20_000_000)];
    assert_eq!(renderers.supply(&site, &sent), Ok(sent.clone()));
}

/// Every prefix of a page naming a family too long to ask for: whatever the
/// cut leaves of the sheet, the load answers, asks for nothing longer than a
/// font's family can be, and stays sendable.
#[test]
fn every_prefix_of_a_page_naming_a_family_too_long_stays_bounded() {
    let page = asking_for(&format!("\"{}\", serif", "e".repeat(600)), "");
    let sheet = page.sheets.first().cloned().unwrap_or_default();
    let mut refused = 0;
    for end in (0..=sheet.len()).filter(|&end| sheet.is_char_boundary(end)) {
        let cut = Page {
            sheets: vec![sheet[..end].to_owned()],
            scheme: ColorScheme::Light,
            ..page.clone()
        };
        let answer = load(cut);
        let (asked, said) = asked_and_said(&answer);
        assert!(
            asked
                .iter()
                .all(|family| family.chars().count() <= LONGEST_FAMILY),
            "prefix {end}"
        );
        if said.iter().any(|line| line.contains("was not asked for")) {
            refused += 1;
        }
        crosses(&answer);
    }
    assert!(refused > 0, "no prefix named a family too long to ask for");
}

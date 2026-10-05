/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 243, found while building 242: a load says at most so much
//! about a page's markup, and every line it says is at most so long.
//!
//! *A page of a few million `<img>` elements loads, says a ceiling's worth of
//! them, and says how many more, under the wire's cap.*
//!
//! Everything a load says crosses to the browser process in one message the
//! wire caps, and a page's markup can make the engine say more than the page
//! is: by how many lines (an `<img>` is five bytes and its line twenty-eight)
//! and by how long one is (a quoted control character is five). Around the
//! closing clause: a line cut at its ceiling with the rest counted, from the
//! markup's half and from the scripts', the two halves each at their own
//! ceiling, a resize said the same way, and every prefix of a page that says
//! more than the ceiling.

use alo_layout::Size;
use alo_renderer::said::{LONGEST_LINE, MOST_SAID_OF_MARKUP};
use alo_renderer::scripts::MOST_SAID;
use alo_renderer::wire::{LARGEST_MESSAGE, read_from_renderer, write_from_renderer};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is drawn.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// What one `<img>` with no `src` makes the engine say.
const NO_SRC: &str = "an <img> with no src";

/// Load this markup into a fresh renderer, and answer the renderer and its
/// whole answer.
fn load(markup: String) -> (Renderer, FromRenderer) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(markup, WINDOW))));
    (renderer, answer)
}

/// The issues of an answer, or one saying what came back instead.
fn issues(answer: &FromRenderer) -> Vec<String> {
    match answer {
        FromRenderer::Loaded { issues, .. } => issues.clone(),
        other => vec![format!("not loaded: {other:?}")],
    }
}

/// The line a load says when it left `how_many` of its markup's lines out.
fn left_out(how_many: usize) -> String {
    format!(
        "{how_many} more things about this page's markup were not said: one load says at most \
         {MOST_SAID_OF_MARKUP}"
    )
}

/// The end of a line that had `how_many` more characters than it says.
fn cut(how_many: usize) -> String {
    format!("… and {how_many} more characters not said: one line says at most {LONGEST_LINE}")
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

/// The closing clause. Two and a half million `<img>` are twelve and a half
/// megabytes of page, and each says `an <img> with no src`: twenty bytes and
/// the eight that say its length, so seventy million bytes of answer had they
/// all been said — past the wire's 67108864, which this page itself is far
/// under. This is the slowest test in the crate, because the page really is
/// two and a half million elements styled, boxed and laid out.
#[test]
fn a_few_million_pictures_say_a_ceilings_worth_and_how_many_more() {
    const PICTURES: usize = 2_500_000;
    let markup = format!("<!doctype html>{}", "<img>".repeat(PICTURES));
    let (_, answer) = load(markup);
    let said = issues(&answer);
    let mut expected = vec![NO_SRC.to_owned(); MOST_SAID_OF_MARKUP];
    expected.push(left_out(PICTURES - MOST_SAID_OF_MARKUP));
    assert_eq!(said, expected);
    assert_eq!(said.last(), Some(&left_out(2_499_744)));
    crosses(&answer);
}

/// Three hundred of them: the ceiling, without the wait.
#[test]
fn past_the_ceiling_the_rest_are_counted() {
    let (_, answer) = load(format!("<!doctype html>{}", "<img>".repeat(300)));
    let mut expected = vec![NO_SRC.to_owned(); MOST_SAID_OF_MARKUP];
    expected.push(left_out(44));
    assert_eq!(issues(&answer), expected);
    crosses(&answer);
}

#[test]
fn exactly_the_ceiling_is_said_whole_and_nothing_counted() {
    let (_, answer) = load(format!(
        "<!doctype html>{}",
        "<img>".repeat(MOST_SAID_OF_MARKUP)
    ));
    assert_eq!(
        issues(&answer),
        vec![NO_SRC.to_owned(); MOST_SAID_OF_MARKUP]
    );
}

/// One picture whose `src` is fourteen million control characters: fourteen
/// megabytes of page, and a line of seventy million characters quoted — one
/// line past the wire's cap on its own.
#[test]
fn a_line_longer_than_the_wire_carries_is_said_cut() {
    const LONG: usize = 14_000_000;
    let markup = format!("<!doctype html><img src=\"{}\">", "\u{1}".repeat(LONG));
    let (_, answer) = load(markup);
    let said = issues(&answer);
    assert_eq!(said.len(), 1, "{:?}", said.first().map(|line| &line[..80]));
    let opening = "no picture was loaded for \"";
    // Characters, counted by hand: the opening's 27, five for each `\u{1}`,
    // and the closing quote.
    let whole = opening.len() + 5 * LONG + 1;
    let kept = (LONGEST_LINE - opening.len()) / 5;
    let partly = (LONGEST_LINE - opening.len()) % 5;
    let expected = format!(
        "{opening}{}{}{}",
        "\\u{1}".repeat(kept),
        &"\\u{1}"[..partly],
        cut(whole - LONGEST_LINE)
    );
    assert_eq!(said.first(), Some(&expected));
    crosses(&answer);
}

/// The scripts' half quotes what the page wrote too: a fetched script is said
/// not to have run, with its `src`.
#[test]
fn a_line_about_a_script_is_cut_the_same_way() {
    const LONG: usize = 14_000_000;
    let markup = format!(
        "<!doctype html><script src=\"{}\"></script>",
        "\u{1}".repeat(LONG)
    );
    let (_, answer) = load(markup);
    let said = issues(&answer);
    assert_eq!(said.len(), 1, "{:?}", said.first().map(|line| &line[..80]));
    let opening = "script 1: not run: it is fetched from \"";
    let line = said.first().map_or("", String::as_str);
    assert!(line.starts_with(opening), "{}", &line[..80]);
    let closing = "\", and the browser process fetching a page's scripts is queue item 238";
    let whole = opening.len() + 5 * LONG + closing.len();
    let more = cut(whole - LONGEST_LINE);
    assert!(line.ends_with(&more), "{}", &line[8000..]);
    assert_eq!(
        line.len() - more.len(),
        LONGEST_LINE,
        "every character kept is one byte"
    );
    crosses(&answer);
}

/// Each half to its own ceiling, markup first: what a page's markup says
/// cannot crowd out what its scripts did, nor the other way about.
#[test]
fn markup_and_scripts_each_say_their_own_ceilings_worth() {
    let markup = format!(
        "<!doctype html>{}{}",
        "<img>".repeat(300),
        "<script>throw 1</script>".repeat(300)
    );
    let (_, answer) = load(markup);
    let said = issues(&answer);
    assert_eq!(said.len(), MOST_SAID_OF_MARKUP + 1 + MOST_SAID + 1);
    let (markup_half, scripts_half) = said.split_at(MOST_SAID_OF_MARKUP + 1);
    assert!(
        markup_half
            .iter()
            .take(MOST_SAID_OF_MARKUP)
            .all(|line| line == NO_SRC)
    );
    assert_eq!(markup_half.last(), Some(&left_out(44)));
    assert!(
        scripts_half
            .iter()
            .take(MOST_SAID)
            .all(|line| line.starts_with("script ") && line.contains("uncaught: 1")),
        "{:?}",
        scripts_half.first()
    );
    assert_eq!(
        scripts_half.last().map(String::as_str),
        Some(
            "44 more things about this page's scripts were not said: one load says at most \
             256"
        )
    );
    crosses(&answer);
}

/// A resize lays the page out again and says what that said, bounded the same.
#[test]
fn a_resize_says_the_same_ceilings_worth() {
    let (mut renderer, _) = load(format!("<!doctype html>{}", "<img>".repeat(300)));
    let answer = renderer.handle(ToRenderer::Resize(Size {
        width: 400.0,
        height: 300.0,
    }));
    let mut expected = vec![NO_SRC.to_owned(); MOST_SAID_OF_MARKUP];
    expected.push(left_out(44));
    assert_eq!(issues(&answer), expected);
}

/// Every prefix of a page that says more than the ceiling, through a style
/// sheet so that each prefix is cheap to lay out: never more than the ceiling
/// and its count, the count only after a full ceiling's worth, and always
/// sendable.
#[test]
fn every_prefix_of_a_page_saying_too_much_stays_within_the_ceiling() {
    let markup = format!(
        "<!doctype html><style>{}</style><img src=\"\u{1}\u{2}\">",
        "a:has(b){}".repeat(MOST_SAID_OF_MARKUP + 20)
    );
    let mut counted = 0;
    for end in (0..=markup.len()).filter(|&end| markup.is_char_boundary(end)) {
        let (_, answer) = load(markup[..end].to_owned());
        let said = issues(&answer);
        assert!(said.len() <= MOST_SAID_OF_MARKUP + 1, "prefix {end}");
        let count = said
            .iter()
            .position(|line| line.contains("about this page's markup were not said"));
        if let Some(at) = count {
            assert_eq!(at, MOST_SAID_OF_MARKUP, "prefix {end}");
            counted += 1;
        }
        assert!(
            said.iter().all(|line| line.chars().count() <= LONGEST_LINE),
            "prefix {end}"
        );
        crosses(&answer);
    }
    assert!(counted > 0, "no prefix said more than the ceiling");
}

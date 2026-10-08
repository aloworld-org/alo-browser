/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's scripts as it loads: which may run, under its own policy, and
//! running them through its event loop (queue item 236, cut from 233), each
//! at its own end tag (queue item 247).
//!
//! # Each runs where the parser reaches it
//!
//! ADR 0017 § 7. The page is parsed a step at a time
//! ([`alo_dom::Parsing`]): the parser stops at each `</script>`, the script
//! runs against **the document parsed so far** — its own element the last
//! thing in it, nothing written after it there yet — and the parser carries
//! on with exactly the markup it was going to read, since there is no
//! `document.write` (law 1). So an inline script that inserts content beside
//! itself puts it where the page meant, and a script before a `<p>` cannot
//! find it while one after can. The page is drawn once, at the end of the
//! load, by the renderer (§ 6).
//!
//! # Which run
//!
//! [`alo_dom::scripts::prepared`] says what each `<script>` the parser
//! reaches is, by the rules [`alo_dom::scripts::carried`] reads a whole page
//! with — and one more, HTML's: a script the parser puts into a tree a
//! script has taken out of the document is not connected, and never runs.
//! Of those, **a classic script written into the page** runs, if the
//! page's policy allows it. Everything else is said rather than skipped in
//! silence, because a page whose script did not run is a page that looks
//! wrong for a reason, and the reason is what somebody needs to see:
//!
//! - **a script with a `src`** is fetched, and a renderer cannot fetch (ADR
//!   0005) — the browser process fetching a page's scripts and handing them
//!   over is queue item 238;
//! - **a module or an import map** is queue item 77's.
//!
//! # The page's policy, asked before anything runs
//!
//! Running a page's inline script without its `Content-Security-Policy` would
//! be running script its author forbade, so every script is asked about first,
//! with the nonce its element presents and its text for a hash: the response's
//! policies ([`Page::policies`]) and every `<meta>` policy **the parser wrote
//! before it**. A `<meta>` governs what follows it and not what came before,
//! which is why the policies are gathered as the parse goes rather than up
//! front — each as the parser made it ([`alo_dom::scripts::stated`]), so a
//! script that removes a `<meta>` does not take its policy back. A `<meta>`
//! a script inserts states nothing here: HTML would apply it, and that is
//! the narrower rule's cost, recorded rather than approximated. A refusal is
//! said with the policy's own words for it.
//!
//! # And telling the policy's author
//!
//! Each script written into the page is also asked of every policy the
//! response's **headers** stated, report-only ones included, and each that
//! objects is an [`Objection`] in the load's answer: the browser process posts
//! the report (queue item 237, [`crate::violations`]). A report-only policy
//! refuses nothing, so a script it objects to still runs, and that it would
//! have been refused is said in the issues as well. A `<meta>` policy's
//! objections do not cross — see [`crate::violations`] for why.
//!
//! # One task per script
//!
//! HTML runs a parser-inserted script in the middle of the parsing task and
//! then *cleans up after running script*, which is a microtask checkpoint
//! because nothing else is running. Each script here is a task of the
//! page's [`EventLoop`] and each is followed by a checkpoint, which is the
//! same order — the script, its jobs, then the parser again — and is the
//! order a page can see.
//!
//! # The document goes to the script, and the parser borrows it back
//!
//! When the first script that may run is about to, the page's document —
//! as much of it as is parsed — moves into its heap and `document` appears
//! on its global object (ADR 0017 § 2, [`Held::scripted`]). Not before: a
//! page none of whose scripts may run never builds a heap. Every step of the
//! parse after that is **lent** the document out of the heap
//! ([`Held::change`]), between tasks, where no script can see it half way.
//! The page is rendered once, after the parse and its last script (§ 6) — by
//! the renderer, not here.
//!
//! # How much a load says
//!
//! A page can throw as often as it likes, and every line said here crosses to
//! the browser process in **one** message, which the wire caps at
//! [`LARGEST_MESSAGE`](crate::wire::LARGEST_MESSAGE). A page that said more
//! than that would have its whole answer refused, and the tab would see a
//! renderer that failed with every issue lost and no reason given. So a load
//! says at most [`MOST_SAID`] things about its scripts, and then **how many
//! more** there were (queue item 242) — the way it says at most
//! [`MOST_OBJECTIONS`] objections, each line at most
//! [`LONGEST_LINE`](crate::said::LONGEST_LINE) characters (243, where the
//! markup's half is bounded the same way). The scripts run the same either
//! way: a ceiling on what is said is not a ceiling on what is run.
//!
//! # A page whose script stops it
//!
//! A throw nothing caught is reported and the next script runs, as in every
//! browser. Anything else ends the page's script for good (ADR 0016 § 7) and
//! every later script is said not to have run. A script that never ends ends
//! the **renderer**: the browser process stops waiting for an answer that does
//! not come and the tab says what happened ([`crate::answers`]) — the bound
//! that already holds for a renderer that stops answering for any reason.

use alo_dom::scripts::{Kind, Script, Source, prepared, stated};
use alo_dom::{Parsing, Reached};
use alo_net::csp::{Content, Inline};

use crate::event_loop::MOST_REPORTS;
use crate::held::Held;
use crate::page::Page;
use crate::said;
use crate::violations::{MOST_OBJECTIONS, Objection};

/// The most lines one load says about its scripts (queue item 242).
///
/// Each line is at most [`LONGEST_LINE`](crate::said::LONGEST_LINE)
/// characters (queue item 243 — a fetched script's `src` is quoted, escaped,
/// and is as long as the page made it), so this many is eight megabytes at the very
/// worst, against a message cap of 64. It is the most
/// one turn of the loop keeps, for the same reason: past it a page is saying
/// the same thing in a loop, and the count says how long the loop was.
pub const MOST_SAID: usize = MOST_REPORTS;

/// What a load says if the heap's cell stops being a document part way
/// through its parse — the engine's bug, answered rather than assumed. The
/// page is drawn from what was parsed.
const LOST: &str =
    "the rest of this page was not parsed: its document could not be found in its heap";

/// What a load says about its scripts: the first [`MOST_SAID`] lines, and how
/// many more there were.
#[derive(Default)]
struct Said {
    lines: Vec<String>,
    left_out: usize,
}

impl Said {
    /// Say a line about script `number`, if there is room.
    fn script(&mut self, number: usize, what: &str) {
        if self.lines.len() < MOST_SAID {
            self.lines
                .push(said::line(&format_args!("script {number}: {what}")));
        } else {
            self.left_out = self.left_out.saturating_add(1);
        }
    }

    /// How many more lines there is room for.
    fn room(&self) -> usize {
        MOST_SAID.saturating_sub(self.lines.len())
    }

    /// The lines, then the count of those left out, if any were.
    fn into_issues(self, issues: &mut Vec<String>) {
        issues.extend(self.lines);
        if self.left_out > 0 {
            issues.push(format!(
                "{} more things about this page's scripts were not said: one load says at \
                 most {MOST_SAID}",
                self.left_out
            ));
        }
    }
}

/// Parse a page to its end through `parsing`, into the document `held` —
/// which starts as the one [`Parsing::start`] handed out — and run each of its
/// scripts as the parser reaches its end tag, moving the document into the
/// page's heap before the first of them runs; with everything that did not
/// run or did not finish added to `issues`, and every header policy's
/// objection to a script written into the page added to `objections`.
pub(crate) fn at_load(
    held: &mut Held,
    parsing: &mut Parsing,
    page: &Page,
    issues: &mut Vec<String>,
    objections: &mut Vec<Objection>,
) {
    let stated_by = page.stated();
    let mut said = Said::default();
    let mut left_out = 0_usize;
    let mut policies = page.policies.clone();
    let mut ended = false;
    let mut number = 0_usize;
    loop {
        let Some(reached) = held.change(|document| parsing.resume(document)) else {
            issues.push(LOST.to_owned());
            break;
        };
        let Reached::Script(element) = reached else {
            break;
        };
        // Every `<meta>` the parser made before this end tag, read where it
        // was put — which no script has had the chance to change since.
        let metas = parsing.take_metas();
        let Some(document) = held.document() else {
            issues.push(LOST.to_owned());
            break;
        };
        policies.extend(metas.into_iter().filter_map(|meta| stated(document, meta)));
        let Some(script) = prepared(document, element) else {
            continue;
        };
        number = number.saturating_add(1);
        if ended {
            said.script(number, "not run, because the page's script has stopped");
            continue;
        }
        if let (Kind::Classic, Source::Written(text)) = (&script.kind, &script.source) {
            let content = Content::element(text);
            let nonce = script.nonce.as_deref();
            for place in stated_by.objecting_to_inline(Inline::Script, nonce, content) {
                if objections.len() < MOST_OBJECTIONS {
                    objections.push(Objection {
                        policy: place,
                        kind: Inline::Script,
                    });
                } else {
                    left_out = left_out.saturating_add(1);
                }
            }
            if stated_by
                .allows_inline(Inline::Script, nonce, content)
                .is_ok()
            {
                // Every enforced header policy allows it, so any objection
                // was a watched policy's, and the script will run regardless
                // (unless a `<meta>` refuses it, which is said below).
                for watched in stated_by.inline_violations(Inline::Script, nonce, content) {
                    said.script(number, &format!("runs, but {watched}"));
                }
            }
        }
        let text = match allowed(&script, &policies) {
            Ok(text) => text,
            Err(why) => {
                said.script(number, &why);
                continue;
            }
        };
        let page_loop = match held.scripted(&page.url, page.identity()) {
            Ok(page_loop) => page_loop,
            Err(why) => {
                said.script(number, &format!("not run: {why}"));
                ended = true;
                continue;
            }
        };
        if let Err(stopped) = page_loop.queue_script(format!("script {number}"), text) {
            said.script(number, &format!("not run: {stopped}"));
            ended = true;
            continue;
        }
        while let Some(turn) = page_loop.run_next_within(said.room()) {
            for report in &turn.reports {
                said.script(number, &report.to_string());
            }
            said.left_out = said.left_out.saturating_add(turn.unreported);
            if let Some(stopped) = turn.stopped {
                said.script(number, &stopped.to_string());
                ended = true;
            }
        }
    }
    said.into_issues(issues);
    if left_out > 0 {
        issues.push(format!(
            "{left_out} more policy objections to this page's scripts were not passed on to be \
             reported: one load carries at most {MOST_OBJECTIONS}"
        ));
    }
}

/// The text of a script that may run here, or why it may not.
fn allowed<'a>(script: &'a Script, policies: &[String]) -> Result<&'a str, String> {
    let text = match (&script.kind, &script.source) {
        (Kind::Classic, Source::Written(text)) => text,
        (Kind::Classic, Source::Linked { src }) => {
            return Err(format!(
                "not run: it is fetched from {src:?}, and the browser process fetching a \
                 page's scripts is queue item 238"
            ));
        }
        (Kind::Module, _) => return Err("not run: modules are queue item 77".to_owned()),
        (Kind::ImportMap, _) => {
            return Err("not run: import maps are queue item 77's, with modules".to_owned());
        }
    };
    let under = Page::policies_of(policies);
    under
        .allows_inline(
            Inline::Script,
            script.nonce.as_deref(),
            Content::element(text),
        )
        .map_err(|refusal| format!("refused: {refusal}"))?;
    Ok(text)
}

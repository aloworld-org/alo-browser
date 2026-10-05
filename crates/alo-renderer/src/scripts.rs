/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's scripts as it loads: which may run, under its own policy, and
//! running them through its event loop (queue item 236, cut from 233).
//!
//! # Which run
//!
//! [`alo_dom::scripts::carried`] says what the markup carries, in document
//! order. Of that, **a classic script written into the page** runs, if the
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
//! policies ([`Page::policies`]) and every `<meta>` policy **written before
//! it**. A `<meta>` governs what follows it and not what came before, which is
//! why the policies are gathered as the walk goes rather than up front. A
//! refusal is said with the policy's own words for it.
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
//! same order — the script, its jobs, the next script — and is the order a
//! page can see. What a page can still tell apart is the document being half
//! built while its script runs: every script here sees the whole parsed
//! document, and running each at its own end tag is queue item 247.
//!
//! # The document goes to the script
//!
//! When the first script that may run is about to, the page's document moves
//! into its heap and `document` appears on its global object (ADR 0017 § 2,
//! [`Held::scripted`]). Not before: a page none of whose scripts may run
//! never builds a heap. The page is rendered once, after its last script
//! (§ 6) — by the renderer, not here.
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

use alo_dom::scripts::{Carried, Kind, Script, Source, carried};
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

/// Run a page's scripts as it loads, oldest first, against the document
/// `held` — moved into the page's heap before the first of them runs — with
/// everything that did not run or did not finish added to `issues`, and every
/// header policy's objection to a script written into the page added to
/// `objections`.
pub(crate) fn at_load(
    held: &mut Held,
    page: &Page,
    issues: &mut Vec<String>,
    objections: &mut Vec<Objection>,
) {
    let stated = page.stated();
    let mut said = Said::default();
    let mut left_out = 0_usize;
    let mut policies = page.policies.clone();
    let mut ended = false;
    let mut number = 0_usize;
    // Gathered before any of them runs: a script that moves or removes a
    // later `<script>` does not change what the page carried as it arrived.
    // Which scripts run at all as the parser reaches them is item 247's.
    let found = held.document().map(carried).unwrap_or_default();
    for found in found {
        let script = match found {
            Carried::Policy(policy) => {
                policies.push(policy);
                continue;
            }
            Carried::Script(script) => script,
        };
        number = number.saturating_add(1);
        if ended {
            said.script(number, "not run, because the page's script has stopped");
            continue;
        }
        if let (Kind::Classic, Source::Written(text)) = (&script.kind, &script.source) {
            let content = Content::element(text);
            let nonce = script.nonce.as_deref();
            for place in stated.objecting_to_inline(Inline::Script, nonce, content) {
                if objections.len() < MOST_OBJECTIONS {
                    objections.push(Objection {
                        policy: place,
                        kind: Inline::Script,
                    });
                } else {
                    left_out = left_out.saturating_add(1);
                }
            }
            if stated.allows_inline(Inline::Script, nonce, content).is_ok() {
                // Every enforced header policy allows it, so any objection
                // was a watched policy's, and the script will run regardless
                // (unless a `<meta>` refuses it, which is said below).
                for watched in stated.inline_violations(Inline::Script, nonce, content) {
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
        let page_loop = match held.scripted() {
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

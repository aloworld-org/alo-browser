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
//! # One task per script
//!
//! HTML runs a parser-inserted script in the middle of the parsing task and
//! then *cleans up after running script*, which is a microtask checkpoint
//! because nothing else is running. Each script here is a task of the
//! page's [`EventLoop`] and each is followed by a checkpoint, which is the
//! same order — the script, its jobs, the next script — and is the order a
//! page can see. What a page could tell apart is the document being half
//! built while its script runs, and no script can see the document yet (no
//! binding reaches it; queue item 80 is where one does).
//!
//! # A page whose script stops it
//!
//! A throw nothing caught is reported and the next script runs, as in every
//! browser. Anything else ends the page's script for good (ADR 0016 § 7) and
//! every later script is said not to have run. A script that never ends ends
//! the **renderer**: the browser process stops waiting for an answer that does
//! not come and the tab says what happened ([`crate::answers`]) — the bound
//! that already holds for a renderer that stops answering for any reason.

use alo_dom::Document;
use alo_dom::scripts::{Carried, Kind, Script, Source, carried};
use alo_net::csp::{Content, Inline};

use crate::event_loop::EventLoop;
use crate::page::Page;

/// Run a page's scripts as it loads, oldest first, and answer the loop they
/// ran in — [`None`] if nothing ran — with everything that did not run or
/// did not finish added to `issues`.
pub(crate) fn at_load(
    document: &Document,
    page: &Page,
    issues: &mut Vec<String>,
) -> Option<EventLoop> {
    let mut policies = page.policies.clone();
    let mut looping: Option<EventLoop> = None;
    let mut ended = false;
    let mut number = 0_usize;
    for found in carried(document) {
        let script = match found {
            Carried::Policy(policy) => {
                policies.push(policy);
                continue;
            }
            Carried::Script(script) => script,
        };
        number = number.saturating_add(1);
        let said = |what: &str| format!("script {number}: {what}");
        if ended {
            issues.push(said("not run, because the page's script has stopped"));
            continue;
        }
        let text = match allowed(&script, &policies) {
            Ok(text) => text,
            Err(why) => {
                issues.push(said(&why));
                continue;
            }
        };
        let page_loop = match &mut looping {
            Some(page_loop) => page_loop,
            None => match EventLoop::new() {
                Ok(made) => looping.insert(made),
                Err(escape) => {
                    issues.push(said(&format!("not run: this page has no engine: {escape}")));
                    ended = true;
                    continue;
                }
            },
        };
        if let Err(stopped) = page_loop.queue_script(text) {
            issues.push(said(&format!("not run: {stopped}")));
            ended = true;
            continue;
        }
        while let Some(turn) = page_loop.run_next() {
            issues.extend(turn.reports.iter().map(|report| said(&report.to_string())));
            if let Some(stopped) = turn.stopped {
                issues.push(said(&stopped.to_string()));
                ended = true;
            }
        }
    }
    looping
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

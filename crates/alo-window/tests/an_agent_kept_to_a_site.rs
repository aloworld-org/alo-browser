/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent kept to the sites a person named: queue item 497, ADR 0055 § 8.
//!
//! *An agent granted one site is refused another by name, the refusal is not
//! a string, the site appears in the record of every action, and the refusal
//! holds for a redirect and for a page that navigates itself.*
//!
//! The first two tests go through the socket from `alo-ask`, a process of its
//! own, to a conductor over the real renderer program that keeps agents to
//! `https://httpbin.org`. It holds the frozen pizza form and a second tab
//! that was opened on httpbin and redirected to another site.
//!
//! The third is a page on a named site that navigates itself to another.
//! Going where a page asks is item 85's and the conductor does not do it
//! yet, so the test loads the page into the same tab as the browser will,
//! over real [`Tabs`], and asks [`serve`] what an agent is answered.

use alo_layout::Size;
use alo_net::{Cause, Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::navigate::Decided;
use alo_renderer::site::Site;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page};
use alo_window::agent_record::{AgentRecord, Caller, Did};
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::asking::{Answer, Ask, Refused};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts, Records};
use alo_window::message::{News, Order};
use alo_window::serving::serve;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// Where the form was served from: the first line of its `origin.txt`.
const FORM: &str = "https://httpbin.org/forms/post";

/// Where the second tab was opened: httpbin, which redirects.
const OPENED: &str = "https://httpbin.org/redirect-to?url=https://example.org/";

/// Where it was redirected to, on a site nobody named.
const ELSEWHERE: &str = "https://example.org/";

/// A page there.
const SHOP: &str = "<!doctype html><body><p>A shop.</p><button>Buy</button></body>";

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url::about_blank())
}

/// The site named for agents: httpbin's.
fn kept() -> AgentSites {
    AgentSites::named(["https://httpbin.org"]).unwrap_or_default()
}

/// The renderer program, beside `alo` as cargo built it.
fn renderer() -> Result<PathBuf, String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    Ok(renderer)
}

/// A conductor keeping agents to httpbin, with the frozen form in one tab
/// and the redirected shop in another, each painted before anybody asks.
fn browsing() -> Result<Conductor, String> {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/web-a-form");
    let origin = std::fs::read_to_string(case.join("origin.txt")).map_err(|why| why.to_string())?;
    assert_eq!(origin.lines().next(), Some(FORM));
    let form = std::fs::read_to_string(case.join("page.html")).map_err(|why| why.to_string())?;
    let (tell, news) = channel();
    let conductor = Conductor::start_keeping_agents_to(
        kept(),
        Tabs::over(Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        Network::over(Pool::with_trust(Trust::of(&[])?)),
        // This test listens to the tab; the strip is `the_strip_in_the_window.rs`.
        move |said| said.is_strip() || tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    let orders = conductor.orders();
    orders
        .send(Order::Resize(Size::new(600.0, 700.0)))
        .map_err(|_| "the conductor is gone".to_owned())?;
    // The shop's page is stated at where the redirect ended, as the browser
    // process states every page it fetched; the tab is where it was opened.
    // Both tabs were opened on httpbin, so one renderer holds them, and a
    // renderer holds one page: the form is loaded last, so it is the one
    // that is there to be read.
    for (opened, html, at) in [(OPENED, SHOP.to_owned(), ELSEWHERE), (FORM, form, FORM)] {
        let page = Page::new(html, Size::ZERO).at(url(at));
        orders
            .send(Order::Open {
                url: url(opened),
                page: Some(Box::new(page)),
            })
            .map_err(|_| "the conductor is gone".to_owned())?;
        // The second is opened over the first, which stops being shown.
        loop {
            match news.recv_timeout(AT_MOST) {
                Ok(News::Painted(_)) => break,
                Ok(News::Chosen(None)) if at == FORM => {}
                other => return Err(format!("{at} was not painted: {other:?}")),
            }
        }
    }
    Ok(conductor)
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-kept-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line per answer.
fn ask(socket: &Path, lines: &[String]) -> Result<Vec<String>, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("alo-ask did not start: {why}"))?;
    let mut stdin = asking.stdin.take().ok_or("alo-ask has no input")?;
    stdin
        .write_all(lines.join("\n").as_bytes())
        .map_err(|why| format!("the lines were not sent: {why}"))?;
    drop(stdin);
    let finished = asking
        .wait_with_output()
        .map_err(|why| format!("alo-ask did not finish: {why}"))?;
    if !finished.status.success() {
        return Err(format!(
            "alo-ask: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// One agent's session: `lines` sent, what it was answered, and what the
/// conductor recorded once the window closed.
fn session(name: &str, lines: &[String]) -> Result<(Vec<String>, Records), String> {
    let conductor = browsing()?;
    let path = socket(name)?;
    let listening =
        Listening::at(&path, conductor.orders()).map_err(|why| format!("not listening: {why}"))?;
    let answers = ask(&path, lines);
    listening.stop();
    let _ = conductor.orders().send(Order::CloseEverything);
    let records = conductor.records().ok_or("the conductor did not finish")?;
    Ok((answers?, records))
}

/// The agent's requests: the tabs, the redirected shop by where it is and by
/// where it was opened, then the form, read and filled.
fn the_requests() -> Vec<String> {
    vec![
        r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
        format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{ELSEWHERE}"}}"#),
        format!(
            r#"{{"v":1,"id":3,"ask":"act","tab":"{ELSEWHERE}","target":{{"name":"Buy"}},"verb":"activate"}}"#
        ),
        format!(r#"{{"v":1,"id":4,"ask":"read","tab":"{OPENED}"}}"#),
        format!(r#"{{"v":1,"id":5,"ask":"read","tab":"{FORM}"}}"#),
        format!(
            r#"{{"v":1,"id":6,"ask":"act","tab":"{FORM}","target":{{"name":"Customer name:"}},"verb":"put_text","text":"Ada Lovelace"}}"#
        ),
    ]
}

#[test]
fn an_agent_kept_to_one_site_is_refused_another_by_name_and_a_redirect_with_it() {
    let (answers, _) = session("refused", &the_requests()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(answers.len(), 6, "one answer a request: {answers:#?}");
    let answer = |at: usize| answers.get(at).map_or("", String::as_str);

    // The redirected tab is not listed: what else is open is not said.
    assert_eq!(
        answer(0),
        format!(r#"{{"answer":"tabs","id":1,"tabs":["{FORM}"],"v":1}}"#)
    );
    // Refused as a kind with its site, read and verb alike.
    for (at, id) in [(1, 2), (2, 3)] {
        assert!(
            answer(at).starts_with(&format!(
                r#"{{"answer":"refused","id":{id},"refusal":{{"kind":"NotGranted","site":"https://example.org","tab":"{ELSEWHERE}"}},"#
            )),
            "{}",
            answer(at)
        );
    }
    // Where it was opened names nothing: a tab is named by where it is.
    assert!(
        answer(3).contains(r#""refusal":{"kind":"NoSuchTab","#),
        "{}",
        answer(3)
    );
    // The site named is reached as before.
    assert!(
        answer(4).starts_with(r#"{"answer":"tree","id":5,"#),
        "{}",
        answer(4)
    );
    assert!(
        answer(5).contains(r#""outcome":{"kind":"TextPut","#),
        "{}",
        answer(5)
    );
}

#[test]
fn the_site_is_in_the_record_of_every_action() {
    let (_, records) = session("record", &the_requests()).unwrap_or_else(|why| panic!("{why}"));
    let entries: Vec<_> = records.agents.entries().collect();
    assert_eq!(entries.len(), 6);
    let sites: Vec<Option<String>> = entries
        .iter()
        .map(|entry| entry.site.as_ref().map(ToString::to_string))
        .collect();
    let httpbin = Some("https://httpbin.org".to_owned());
    let elsewhere = Some("https://example.org".to_owned());
    assert_eq!(
        sites,
        [
            None,
            elsewhere.clone(),
            elsewhere,
            httpbin.clone(),
            httpbin.clone(),
            httpbin
        ]
    );
    assert!(matches!(
        entries.get(2).map(|entry| &entry.did),
        Some(Did::Refused(Refused::NotGranted { .. }))
    ));
    assert!(matches!(
        entries.get(5).map(|entry| &entry.did),
        Some(Did::Acted { came_to: Ok(_), .. })
    ));
    let words: Vec<String> = entries.iter().map(ToString::to_string).collect();
    assert!(
        words.iter().any(|line| line.starts_with(&format!(
            "agent#0 at {FORM} on https://httpbin.org: action#"
        ))),
        "{words:#?}"
    );
}

/// A page on httpbin whose script follows its own link off the site as it
/// loads.
const LEAVES: &str = "<!doctype html><body><a href=\"https://example.org/\">Away</a>\
                      <script>document.body.firstChild.click();</script></body>";

#[test]
fn a_page_that_navigates_itself_off_the_site_is_refused_where_it_went() {
    const HERE: &str = "https://httpbin.org/html";
    let renderer = renderer().unwrap_or_else(|why| panic!("{why}"));
    let mut tabs =
        Tabs::over(Renderers::running(renderer, &[]).with_machine(alo_window::fonts::frozen()));
    let (mut record, sites) = (AgentRecord::new(), kept());
    let tab = tabs.open(url(HERE));
    let loaded = tabs.load(
        tab,
        Page::new(LEAVES.to_owned(), Size::new(400.0, 300.0)).at(url(HERE)),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let mut asked =
        |tabs: &mut Tabs, ask: Ask| serve(tabs, &mut record, &sites, Caller(0), Ok(ask)).answer;
    let read = |tab: &str| Ask::Read {
        tab: tab.to_owned(),
    };

    // Until it has gone, the page is httpbin's and is read.
    assert!(matches!(asked(&mut tabs, read(HERE)), Answer::Tree { .. }));
    let Some(Decided::Go(navigation)) = tabs.navigation(tab) else {
        panic!("the page did not ask to go anywhere");
    };
    assert_eq!(navigation.url, url(ELSEWHERE));

    // Gone where it asked, into the same tab, as item 85 will take it.
    let went = tabs.load(
        tab,
        Page::new(SHOP.to_owned(), Size::new(400.0, 300.0)).at(navigation.url.clone()),
        navigation.cause.clone(),
    );
    assert!(matches!(went, Ok(FromRenderer::Loaded { .. })), "{went:?}");
    assert_eq!(tabs.tab(tab).and_then(Tab::address), Some(&url(ELSEWHERE)));

    assert_eq!(asked(&mut tabs, Ask::Tabs), Answer::Tabs(Vec::new()));
    assert_eq!(
        asked(&mut tabs, read(ELSEWHERE)),
        Answer::Refused(Refused::NotGranted {
            tab: ELSEWHERE.to_owned(),
            site: Site::of(&url(ELSEWHERE)),
        })
    );
    assert_eq!(
        asked(&mut tabs, read(HERE)),
        Answer::Refused(Refused::NoSuchTab {
            tab: HERE.to_owned()
        }),
        "where it was is not where it is"
    );
    assert_eq!(
        asked(
            &mut tabs,
            Ask::Act {
                tab: ELSEWHERE.to_owned(),
                target: alo_agent::Target::Named("Buy".to_owned()),
                verb: alo_agent::Verb::Activate,
            }
        ),
        Answer::Refused(Refused::NotGranted {
            tab: ELSEWHERE.to_owned(),
            site: Site::of(&url(ELSEWHERE)),
        })
    );
    let refused_on: Vec<Option<String>> = record
        .entries()
        .map(|entry| entry.site.as_ref().map(ToString::to_string))
        .collect();
    assert_eq!(
        refused_on,
        [
            Some("https://httpbin.org".to_owned()),
            None,
            Some("https://example.org".to_owned()),
            Some("https://httpbin.org".to_owned()),
            Some("https://example.org".to_owned()),
        ]
    );
    tabs.close(tab);
}

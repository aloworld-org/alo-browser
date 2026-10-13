/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 535: a person's click on a link in a frozen page follows it,
//! the same as the agent's `Activate` on it — through real [`Tabs`] over
//! the real confined `alo-render` binary (ADR 0024 § 5).
//!
//! The page is the corpus's `web-example-com`, frozen as `example.com` sent
//! it, whose one link reads "Learn more". The browser process decides where
//! each click goes and assigns its cause from which message was answered
//! (ADR 0012 § 4): the person's, naming the tab, for a pointer; the agent's
//! action for an `Activate`.

use alo_agent::{Target, Verb};
use alo_layout::Size;
use alo_net::cause::Cause;
use alo_renderer::face::Face;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::navigate::Decided;
use alo_renderer::pointer::Pointer;
use alo_renderer::tab::{TabId, Tabs};
use alo_renderer::{FromRenderer, Page};
use alo_text::{Slant, Weight};
use alo_url::Url;

/// The frozen page's bytes, and where it was taken from: its `origin.txt`'s
/// first line.
const PAGE: &str = include_str!("../../alo-corpus/cases/web-example-com/page.html");
const ORIGIN: &str = include_str!("../../alo-corpus/cases/web-example-com/origin.txt");

/// The renderer binary, confined as every renderer is, with the corpus's
/// sans-serif as `system-ui` and `sans-serif`, which the page asks for.
fn tabs() -> Tabs {
    let faces = Face::new(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    )
    .into_iter()
    .collect();
    let generics = Generics::stating(vec![
        ("system-ui".to_owned(), "DejaVu Sans".to_owned()),
        ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
    ]);
    Tabs::over(
        Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
            .with_fonts(faces)
            .with_generics(generics),
    )
}

fn url(text: &str) -> Url {
    alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
}

/// A tab showing the frozen page at its address, at its corpus size.
fn showing(tabs: &mut Tabs) -> TabId {
    let at = url(ORIGIN.lines().next().unwrap_or_default());
    let tab = tabs.open(at.clone());
    let loaded = tabs.load(
        tab,
        Page::new(PAGE, Size::new(800.0, 600.0)).at(at),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    tab
}

/// The link, as the corpus's committed `agent.txt` reads it: "Learn more"
/// at (160, 188.07813), 91.25 × 19. The point pressed is inside it.
const PRESSED: (f32, f32) = (200.0, 197.0);

#[test]
fn a_persons_click_on_the_frozen_link_goes_where_the_agents_activate_goes() {
    let agent_txt = include_str!("../../alo-corpus/cases/web-example-com/agent.txt");
    assert!(
        agent_txt.contains("link \"Learn more\" at (160, 188.07813) 91.25×19"),
        "the link is where the point is pressed"
    );

    let mut tabs = tabs();
    let tab = showing(&mut tabs);
    let pressed = tabs.point(tab, Pointer::down(PRESSED.0, PRESSED.1));
    assert!(
        matches!(&pressed, Ok(FromRenderer::Delivered { issues, navigation: None, .. }) if issues.is_empty()),
        "{pressed:?}"
    );
    assert!(tabs.navigation(tab).is_none(), "a press goes nowhere");
    let released = tabs.point(tab, Pointer::up(PRESSED.0, PRESSED.1));
    assert!(
        matches!(&released, Ok(FromRenderer::Delivered { issues, .. }) if issues.is_empty()),
        "{released:?}"
    );
    let Some(Decided::Go(by_person)) = tabs.navigation(tab) else {
        panic!("the person's click on the link was not decided as a navigation");
    };
    assert_eq!(by_person.url, url("https://iana.org/domains/example"));
    assert_eq!(
        by_person.cause,
        Cause::Person { tab },
        "a link a person clicked is the person's (ADR 0012 § 2)"
    );

    let acted = tabs.act(tab, Target::Named("Learn more".to_owned()), Verb::Activate);
    let Ok((action, FromRenderer::Acted { .. })) = acted else {
        panic!("the agent could not activate the link: {acted:?}");
    };
    let Some(Decided::Go(by_agent)) = tabs.navigation(tab) else {
        panic!("the agent's Activate on the link was not decided as a navigation");
    };
    assert_eq!(by_agent.url, by_person.url, "the same link, the same place");
    assert_eq!(by_agent.method, by_person.method);
    assert_eq!(by_agent.referrer, by_person.referrer);
    assert!(matches!(by_agent.cause, Cause::Agent { action: a, .. } if a == action));
}

#[test]
fn a_click_beside_the_link_goes_nowhere() {
    let mut tabs = tabs();
    let tab = showing(&mut tabs);
    for pointer in [Pointer::down(100.0, 197.0), Pointer::up(100.0, 197.0)] {
        let answered = tabs.point(tab, pointer);
        assert!(
            matches!(
                &answered,
                Ok(FromRenderer::Delivered {
                    navigation: None,
                    ..
                })
            ),
            "{answered:?}"
        );
    }
    assert!(tabs.navigation(tab).is_none());
}

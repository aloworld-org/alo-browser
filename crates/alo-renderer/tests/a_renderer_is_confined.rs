//! ADR 0010, watched rather than trusted.
//!
//! *"A test that watches a renderer fail to open a file, not a flag saying a
//! sandbox was applied. A policy that was installed and permits everything
//! reports success exactly like one that works."*
//!
//! So these tests run the **real renderer binary**, twice: once confined and
//! once not. The unconfined run has to *fail* — a test that passed both before
//! and after would be testing nothing at all.

use alo_css::media::ColorScheme;
use alo_layout::geometry::Size;
use alo_renderer::host::Renderers;
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::sandbox;
use alo_renderer::site::Site;
use std::process::Command;

const RENDERER: &str = env!("CARGO_BIN_EXE_alo-render");

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

/// What the renderer said when asked to try the forbidden things.
fn check(confined: bool) -> (bool, String) {
    let arguments = vec!["--check-confinement".to_owned()];
    let mut command = if confined {
        match sandbox::confined(std::path::Path::new(RENDERER), &arguments) {
            Ok(command) => command,
            Err(why) => return (false, why.why),
        }
    } else {
        let mut plain = Command::new(RENDERER);
        plain.args(&arguments);
        plain
    };
    match command.output() {
        Ok(done) => (
            done.status.success(),
            String::from_utf8_lossy(&done.stdout).into_owned(),
        ),
        Err(why) => (false, why.to_string()),
    }
}

/// The one that matters. Four things a renderer must not be able to do, each
/// attempted **inside the renderer** in the state it actually runs in, and each
/// refused by the operating system rather than merely failing.
#[test]
fn a_confined_renderer_cannot_read_a_file_write_one_or_open_a_socket() {
    if !sandbox::is_available() {
        // Not skipped quietly: on a platform with no sandbox, ADR 0010 says the
        // browser does not run at all, and `Renderers` refuses to start one.
        // There is a test for that below.
        return;
    }
    let (passed, said) = check(true);
    assert!(passed, "a confined renderer was allowed something:\n{said}");
    for what in [
        "read /etc/hosts",
        "read a file in the home directory",
        "write a file",
        "open a socket",
    ] {
        assert!(
            said.contains(&format!("refused: {what}")),
            "{what} was not refused:\n{said}"
        );
    }
    // "Operation not permitted" rather than "no such file" or "connection
    // refused": only the platform saying no counts, because the other two look
    // like failure and are not confinement.
    assert_eq!(
        said.matches("Operation not permitted").count(),
        4,
        "something failed for a reason that is not the sandbox:\n{said}"
    );
}

/// The test above would pass against a sandbox that does nothing if the
/// operations failed on their own. They do not — unconfined, all four work.
#[test]
fn the_same_renderer_unconfined_is_allowed_all_of_it() {
    let (passed, said) = check(false);
    assert!(
        !passed,
        "an unconfined renderer refused something, so the test above proves nothing:\n{said}"
    );
    assert_eq!(
        said.matches("ALLOWED:").count(),
        4,
        "the unconfined run should be allowed everything:\n{said}"
    );
}

/// Confinement is not a mode a renderer is put into afterwards — the profile is
/// applied by `exec`, so there is no instant in which the process is running
/// and unconfined. ADR 0010 rejected "apply it after start-up" for that reason.
///
/// Checked here by the thing that follows from it: a renderer that renders is
/// a renderer that was already confined.
#[test]
fn a_renderer_that_renders_was_confined_before_it_read_anything() {
    if !sandbox::is_available() {
        return;
    }
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));
    let loaded = renderers.ask(
        &site,
        &ToRenderer::Load(Box::new(Page {
            html: "<p>rendered inside a sandbox</p>".to_owned(),
            sheets: vec!["p { margin: 4px }".to_owned()],
            viewport: Size {
                width: 100.0,
                height: 50.0,
            },
            scheme: ColorScheme::Light,
        })),
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "a confined renderer could not render: {loaded:?}"
    );

    // And it is genuinely confined while doing so: the same binary, asked to
    // misbehave through the same path, is refused.
    let (passed, said) = check(true);
    assert!(passed, "{said}");
}

/// ADR 0010: *"the browser does not claim a platform it cannot sandbox."* So a
/// platform with no sandbox does not get an unconfined renderer — it gets no
/// renderer.
#[test]
fn a_platform_with_no_sandbox_gets_no_renderer_rather_than_an_unconfined_one() {
    let made = sandbox::confined(std::path::Path::new(RENDERER), &[]);
    if sandbox::is_available() {
        assert!(made.is_ok());
    } else {
        assert!(
            made.is_err(),
            "a platform with no sandbox handed back a plain command"
        );
        let why = made.err().map(|why| why.why).unwrap_or_default();
        assert!(why.contains("does not claim"), "{why:?}");
    }
}

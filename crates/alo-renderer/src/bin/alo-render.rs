//! The renderer process.
//!
//! Everything it does happens with a hostile page's bytes in memory, so it does
//! as little as possible: read work from standard input, answer on standard
//! output, and stop when the other end closes. It opens no file it was not
//! given, makes no connection, and knows nothing about the profile.
//!
//! That is not the sandbox — the sandbox is queue item 167 and needs an ADR of
//! its own. This is the shape the sandbox will be applied to, and the point of
//! keeping it this small is that when the sandbox arrives there is very little
//! here for it to have to permit.

use alo_renderer::renderer::Renderer;
use alo_renderer::serve;
use alo_text::FontDatabase;

fn main() -> std::process::ExitCode {
    // ADR 0010 asks for a check that *watches a refusal* rather than trusting a
    // flag, and asks for it of the renderer rather than of a stand-in. So the
    // renderer can be asked to try the forbidden things and say what happened,
    // in the state it actually runs in.
    if std::env::args().any(|argument| argument == "--check-confinement") {
        return check_confinement();
    }
    // No fonts, and that is the design rather than a gap. A confined renderer
    // cannot open a font file (ADR 0010), so it starts with none and is handed
    // them by the browser process before it is given a page.
    let mut renderer = Renderer::new(FontDatabase::new());
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    match serve::serve(&mut renderer, &mut input, &mut output) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(why) => {
            // To the terminal rather than to the pipe: the pipe carries
            // messages, and a diagnostic on it would be read as one.
            eprintln!("alo-render: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Try what a renderer must not be able to do, and print what happened.
///
/// One line per thing, `refused` or `ALLOWED`, so a person reading the output
/// and a test reading the output are reading the same thing.
fn check_confinement() -> std::process::ExitCode {
    use alo_renderer::sandbox::probe;
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users".to_owned());
    let attempts = [
        // A file that exists on every machine and is outside everything the
        // profile allows. Reading it unconfined works, which is what makes its
        // refusal mean something.
        ("read /etc/hosts", probe::reading_a_file("/etc/hosts")),
        (
            "read a file in the home directory",
            probe::reading_a_file(&format!("{home}/.zshrc")),
        ),
        (
            "write a file",
            probe::writing_a_file("/tmp/alo-render-should-not-exist"),
        ),
        ("open a socket", probe::opening_a_socket()),
    ];
    let mut all_refused = true;
    for (what, outcome) in attempts {
        match outcome {
            probe::Attempt::Refused { why } => println!("refused: {what} ({why})"),
            probe::Attempt::Allowed { what: how } => {
                println!("ALLOWED: {what} ({how})");
                all_refused = false;
            }
        }
    }
    if all_refused {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

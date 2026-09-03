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
use alo_text::{Font, FontDatabase, Slant, Weight};

fn main() -> std::process::ExitCode {
    let mut renderer = Renderer::new(fonts());
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

/// The fonts this renderer can draw with.
///
/// One, embedded, for now — a renderer with no filesystem cannot go and look
/// for any, which is a consequence of the design rather than a gap in it. What
/// a sandboxed renderer is *given* is queue item 167's question.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database
}

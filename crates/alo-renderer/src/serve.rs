//! The renderer side of the boundary: read work, do it, answer.
//!
//! This is what runs in the child process. It is deliberately tiny — a loop, a
//! decode, a call into [`crate::Renderer`], an encode — because everything it
//! does is done with a hostile page's bytes in memory, and a small loop is one
//! somebody can read all of.
//!
//! # What it does when it cannot understand something
//!
//! It answers and carries on. A message this process cannot read is a bug in
//! the browser process or a version mismatch, and neither is a reason to take
//! the tab down — whereas exiting on one would let a browser-process bug look
//! exactly like a page crashing the renderer, which is the thing that must
//! stay distinguishable.

use crate::message::{Failure, FromRenderer};
use crate::pipe::{self, Arrived};
use crate::renderer::Renderer;
use crate::wire;
use std::io::{Read, Write};

/// Serve until the other end closes.
///
/// # Errors
///
/// Only what the pipe itself failed with. Anything about a *message* is
/// answered rather than returned.
pub fn serve(
    renderer: &mut Renderer,
    from: &mut impl Read,
    to: &mut impl Write,
) -> std::io::Result<()> {
    loop {
        let arrived = match pipe::read(from) {
            Ok(Arrived::Message(bytes)) => bytes,
            // The browser process closed cleanly. Not an error, and not a
            // crash: this is what closing a tab looks like from in here.
            Ok(Arrived::Ended) => return Ok(()),
            Err(why) => return Err(std::io::Error::other(why.why)),
        };
        let answer = match wire::read_to_renderer(&arrived) {
            Ok(work) => renderer.handle(work),
            Err(why) => FromRenderer::Failed(Failure::Unpaintable {
                why: format!("the browser process sent something unreadable: {why}"),
            }),
        };
        pipe::write(to, &wire::write_from_renderer(&answer))?;
    }
}

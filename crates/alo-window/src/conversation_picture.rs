/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The live card's page drawn by a confined renderer of its own, on a
//! thread of its own (queue item 869, ADR 0100).
//!
//! Chrome streams the card's page as PNG frames, many a second. Each was
//! drawn as the card's picture in the first screen's document, so each
//! painted the whole first screen again — two hundred milliseconds of the
//! first screen's renderer, measured, at two — and a key's edit in the one
//! box waited behind the frame being painted and then painted the card
//! again itself. Now the first screen leaves the picture's place clear
//! ([`alo_renderer::beneath`]), and the window draws the picture beneath it
//! from here: the frame and the place's size go to a renderer that holds
//! nothing else ([`alo_renderer::card_picture`]), whose answer is the
//! picture at exactly that size. A frame of the card never reaches the
//! renderer that draws the box.
//!
//! # Never waited on, never queued
//!
//! As the parts' thread is ([`crate::conversation_drawing`]): the window
//! only sends here and hears what was drawn as an event. Of the pictures
//! waiting, only the newest is drawn, and the same frame at the same size
//! is not asked again. Each picture asked is numbered, and the number of
//! the newest drawn — or replaced before it was — is told with what came of
//! it, so a window can say whether the picture it holds is the one it last
//! asked for.
//!
//! # A renderer that went
//!
//! Said once, as the first screen's is, and not asked again: the card's
//! place keeps the last picture drawn there.

use crate::conversation_drawing::{Drawn, Part};
use alo_renderer::beneath::Beneath;
use alo_renderer::card_picture::{self, CardPicture};
use alo_renderer::host::{Gone, Renderers};
use alo_renderer::{FromRenderer, ToRenderer};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// How the thread tells the window what it drew: whether anybody heard.
pub type TellPicture = Box<dyn Fn(Drawn) -> bool + Send>;

/// A picture to draw: the frame, the size of its place, and its number.
#[derive(Debug)]
struct Wanted {
    png: Vec<u8>,
    width: u32,
    height: u32,
    number: u64,
}

/// A thread holding the card's picture's renderer.
#[derive(Debug)]
pub struct CardPictures {
    /// Where work is sent; [`None`] once finished.
    asked: Option<Sender<Wanted>>,
    /// The frame and size last sent, so the same is not sent twice.
    sent: Option<(Vec<u8>, u32, u32)>,
    /// How many pictures have been sent: the number of the newest.
    numbered: u64,
    thread: Option<JoinHandle<()>>,
}

impl CardPictures {
    /// Start a thread drawing the card's picture in a renderer of
    /// `renderers`' at a site of its own, telling the window with `tell`.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(renderers: Renderers, tell: TellPicture) -> std::io::Result<Self> {
        let (asked, asking) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-card-picture".to_owned())
            .spawn(move || hold(renderers, &asking, &tell))?;
        Ok(Self {
            asked: Some(asked),
            sent: None,
            numbered: 0,
            thread: Some(thread),
        })
    }

    /// Have `png` drawn into a place the size of `place`, unless it is the
    /// frame and size last sent. Returns at once.
    pub fn draw(&mut self, png: &[u8], place: Beneath) {
        let sent = (place.width, place.height);
        if self
            .sent
            .as_ref()
            .is_some_and(|(last, width, height)| (*width, *height) == sent && last == png)
        {
            return;
        }
        if let Some(sender) = &self.asked {
            self.numbered += 1;
            // A thread that has gone is a window that is closing.
            let _ = sender.send(Wanted {
                png: png.to_vec(),
                width: place.width,
                height: place.height,
                number: self.numbered,
            });
        }
        self.sent = Some((png.to_vec(), place.width, place.height));
    }

    /// Forget what was last sent, so the next picture is sent whatever it
    /// is.
    pub fn forget(&mut self) {
        self.sent = None;
    }

    /// The number of the newest picture sent, which [`Drawn::Pictured`]
    /// says once it is drawn; 0 before any is.
    pub fn asked_through(&self) -> u64 {
        self.numbered
    }

    /// Stop the thread and wait for it, its renderer stopped with it:
    /// whether it ended rather than panicked.
    pub fn finish(mut self) -> bool {
        self.asked = None;
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

/// Draw the newest of the pictures `asking` sends, telling the window what
/// came of each, until nobody sends any more or the window has gone.
fn hold(mut renderers: Renderers, asking: &Receiver<Wanted>, tell: &TellPicture) {
    let site = card_picture::site();
    let mut gone: Option<Gone> = None;
    while let Ok(first) = asking.recv() {
        // Of the pictures waiting, only the newest is worth drawing.
        let wanted = asking.try_iter().last().unwrap_or(first);
        let mut said = Vec::new();
        let mut frame = None;
        if gone.is_none() {
            let work = ToRenderer::CardPicture(Box::new(CardPicture {
                png: wanted.png,
                width: wanted.width,
                height: wanted.height,
            }));
            match renderers.ask(&site, &work) {
                Ok(FromRenderer::Painted(painted)) => frame = Some(painted),
                Ok(FromRenderer::Failed(failure)) => said.push(Drawn::Said(
                    Part::Start,
                    format!("the card's page could not be drawn: {failure}"),
                )),
                Ok(_) => said.push(Drawn::Said(
                    Part::Start,
                    "the card's page's renderer answered something it was not asked".to_owned(),
                )),
                Err(went) => {
                    said.push(Drawn::Said(Part::Start, went.to_string()));
                    gone = Some(went);
                }
            }
        }
        said.push(Drawn::Pictured {
            frame,
            number: wanted.number,
        });
        for drawn in said {
            if !tell(drawn) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_same_frame_at_the_same_size_is_not_asked_again_and_a_renderer_gone_is_said_once() {
        let (tell, heard) = channel();
        let mut pictures = CardPictures::start(
            Renderers::running("/nonexistent/alo-render", &[]),
            Box::new(move |drawn| tell.send(drawn).is_ok()),
        )
        .expect("a thread");
        let place = Beneath {
            left: 0,
            top: 0,
            width: 4,
            height: 2,
        };
        pictures.draw(b"\x89PNG one", place);
        pictures.draw(b"\x89PNG one", place);
        assert_eq!(pictures.asked_through(), 1, "the same, not asked again");
        let wait = Duration::from_secs(30);
        assert!(matches!(
            heard.recv_timeout(wait),
            Ok(Drawn::Said(Part::Start, words)) if words.starts_with("the renderer for alo:")
        ));
        assert_eq!(
            heard.recv_timeout(wait),
            Ok(Drawn::Pictured {
                frame: None,
                number: 1
            })
        );
        // Another size is another picture; the renderer gone, nothing said.
        pictures.draw(b"\x89PNG one", Beneath { width: 8, ..place });
        assert_eq!(pictures.asked_through(), 2);
        assert_eq!(
            heard.recv_timeout(wait),
            Ok(Drawn::Pictured {
                frame: None,
                number: 2
            })
        );
        // Forgotten — its card gone — the same is asked again.
        pictures.forget();
        pictures.draw(b"\x89PNG one", Beneath { width: 8, ..place });
        assert_eq!(pictures.asked_through(), 3);
        assert_eq!(
            heard.recv_timeout(wait),
            Ok(Drawn::Pictured {
                frame: None,
                number: 3
            })
        );
        assert!(pictures.finish());
    }
}

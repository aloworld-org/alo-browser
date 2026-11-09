/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How the conversation's window is put on screen: from pixels kept on the
//! GPU, with only what changed written to them (queue item 873, ADR 0105,
//! `crate::present_kept`), or — on a machine where that cannot be had, or
//! when asked — by `softbuffer` copying all of them ([`Presenter`]).
//!
//! Either shows the same words. The kept way is chosen when `ALO_PRESENT`
//! is not `copied`, on macOS, and the window gets a Metal surface and a GPU
//! that can draw to it; anything short of that is said once, and the window
//! is copied as before.

use crate::conversation_composed::Recomposed;
use crate::present::Presenter;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// Which way to try first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// From pixels kept on the GPU, falling back to [`How::Copied`].
    Kept,
    /// By `softbuffer`, every frame copied whole.
    Copied,
}

impl How {
    /// What `ALO_PRESENT` asks for: `copied`, or the kept way.
    pub fn asked() -> Self {
        Self::named(std::env::var("ALO_PRESENT").ok().as_deref())
    }

    /// What `name` asks for: `copied`, or the kept way.
    pub fn named(name: Option<&str>) -> Self {
        if name == Some("copied") {
            Self::Copied
        } else {
            Self::Kept
        }
    }
}

/// The window's way onto the screen.
#[derive(Debug)]
pub struct Screen<W> {
    way: Way<W>,
}

#[derive(Debug)]
enum Way<W> {
    /// From pixels kept on the GPU.
    #[cfg(target_os = "macos")]
    Kept(Box<crate::present_kept::Kept>),
    /// By `softbuffer`.
    Copied(Presenter<W>),
}

impl<W: HasDisplayHandle + HasWindowHandle + Clone + Send + Sync + 'static> Screen<W> {
    /// The window's way onto the screen, tried as `how` says, and why the
    /// kept way was not had when it was tried and not had.
    ///
    /// # Errors
    ///
    /// Neither way: what `softbuffer` said.
    pub fn over(window: W, how: How) -> Result<(Self, Option<String>), String> {
        let mut missed = None;
        #[cfg(target_os = "macos")]
        if how == How::Kept {
            match crate::present_kept::Kept::over(window.clone()) {
                Ok(kept) => {
                    return Ok((
                        Self {
                            way: Way::Kept(Box::new(kept)),
                        },
                        None,
                    ));
                }
                Err(why) => missed = Some(why),
            }
        }
        #[cfg(not(target_os = "macos"))]
        if how == How::Kept {
            missed = Some("pixels are kept on the GPU on macOS alone".to_owned());
        }
        Ok((
            Self {
                way: Way::Copied(Presenter::over(window)?),
            },
            missed,
        ))
    }

    /// Whether the pixels are kept on the GPU.
    pub fn is_kept(&self) -> bool {
        !matches!(self.way, Way::Copied(_))
    }

    /// Show `words`, a window of `size` row by row, of which `recomposed`
    /// says what changed since the last. Answers how many bytes were handed
    /// on: the regions composed again when kept, every word when copied.
    ///
    /// # Errors
    ///
    /// What the way onto the screen said.
    pub fn show(
        &mut self,
        size: (u32, u32),
        words: &[u32],
        recomposed: &Recomposed,
    ) -> Result<u64, String> {
        match &mut self.way {
            #[cfg(target_os = "macos")]
            Way::Kept(kept) => kept.show(size, words, recomposed.regions()),
            Way::Copied(presenter) => {
                presenter.show_words(size, words)?;
                Ok(u64::from(size.0) * u64::from(size.1) * 4)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_is_asked_by_name_and_the_kept_way_otherwise() {
        assert_eq!(How::named(Some("copied")), How::Copied);
        assert_eq!(How::named(None), How::Kept);
        assert_eq!(How::named(Some("kept")), How::Kept);
        assert_eq!(How::named(Some("Copied")), How::Kept, "the word exactly");
    }
}

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the renderer program is: beside the browser's own.
//!
//! `alo` and `alo-render` are built into the same directory, and are shipped
//! that way. Looking beside the running program, rather than on the search
//! path, means the renderer started is the one built with this browser — a
//! renderer from somewhere else on `PATH` would speak a message format this
//! process may not.

use std::path::{Path, PathBuf};

/// The renderer's file name.
pub const RENDERER: &str = "alo-render";

/// The renderer that belongs with the program at `program`.
pub fn renderer_beside(program: &Path) -> PathBuf {
    program.with_file_name(RENDERER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_renderer_is_in_the_same_directory() {
        assert_eq!(
            renderer_beside(Path::new("/opt/alo/bin/alo")),
            PathBuf::from("/opt/alo/bin/alo-render")
        );
    }
}

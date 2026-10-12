/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the agent's socket is, and why only its person can open it (ADR
//! 0055 § 1).
//!
//! A path under the person's own runtime directory — `XDG_RUNTIME_DIR` where
//! the system says one, as Linux does, and `TMPDIR` otherwise, which macOS
//! makes per person — in a directory of this browser's that only its owner may
//! enter, holding a socket only its owner may open. **That is the grant an
//! agent holds to reach this browser at all**, and it is the operating
//! system's to enforce: no page, no site and no other person's process can
//! open the path. Neither variable set is a machine with no private place,
//! and the socket is then not opened rather than put somewhere shared.
//!
//! # One browser per place
//!
//! A socket already there that answers is another browser, and is left
//! alone: this one opens no socket rather than taking the place over. One
//! that does not answer was left by a browser that stopped, and is removed.
//! Anything else at the path is not ours, and is refused rather than removed.

use std::fs;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// The directory's name under the runtime directory.
const DIRECTORY: &str = "alo-browser";

/// The socket's name in it.
const SOCKET: &str = "agent.sock";

/// Why no socket could be placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoPlace {
    /// Neither `XDG_RUNTIME_DIR` nor `TMPDIR` is set.
    NoRuntimeDirectory,
    /// The directory exists and is not this person's alone, or is a link.
    NotPrivate(PathBuf),
    /// Something that is not a socket is at the socket's path.
    NotASocket(PathBuf),
    /// Another browser is answering at the path.
    Taken(PathBuf),
    /// The operating system refused, in its words.
    Refused(String),
}

impl core::fmt::Display for NoPlace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NoPlace::NoRuntimeDirectory => f.write_str(
                "neither XDG_RUNTIME_DIR nor TMPDIR is set, so there is no private place for it",
            ),
            NoPlace::NotPrivate(path) => write!(
                f,
                "{} is not a directory only you can enter, so nothing is put in it",
                path.display()
            ),
            NoPlace::NotASocket(path) => {
                write!(f, "{} is something other than a socket", path.display())
            }
            NoPlace::Taken(path) => {
                write!(f, "another browser is answering at {}", path.display())
            }
            NoPlace::Refused(why) => write!(f, "the system refused: {why}"),
        }
    }
}

/// The person's runtime directory, if the system names one.
pub fn runtime_directory() -> Option<PathBuf> {
    ["XDG_RUNTIME_DIR", "TMPDIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Where the socket is under `runtime`. Nothing is made: a caller looking
/// for a browser uses this, and only the browser prepares it.
pub fn socket_under(runtime: &Path) -> PathBuf {
    runtime.join(DIRECTORY).join(SOCKET)
}

/// The socket's path under the person's runtime directory, prepared.
///
/// # Errors
///
/// As [`prepare`], and [`NoPlace::NoRuntimeDirectory`].
pub fn prepared() -> Result<PathBuf, NoPlace> {
    let runtime = runtime_directory().ok_or(NoPlace::NoRuntimeDirectory)?;
    prepare(&socket_under(&runtime))
}

/// Make `socket`'s directory, private, if it is not there; check it is
/// private if it is; and clear a socket a stopped browser left at the path.
///
/// # Errors
///
/// [`NoPlace::NotPrivate`] for a directory anybody else may enter, or a link;
/// [`NoPlace::Taken`] when a browser answers at the path;
/// [`NoPlace::NotASocket`] for anything else there; and
/// [`NoPlace::Refused`] for what the system would not do.
pub fn prepare(socket: &Path) -> Result<PathBuf, NoPlace> {
    let refused = |why: std::io::Error| NoPlace::Refused(why.to_string());
    let directory = socket
        .parent()
        .ok_or_else(|| NoPlace::NotPrivate(socket.to_path_buf()))?;
    match fs::DirBuilder::new().mode(0o700).create(directory) {
        Ok(()) => {}
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(why) => return Err(refused(why)),
    }
    // Read without following a link: a link here could point anywhere.
    let found = fs::symlink_metadata(directory).map_err(refused)?;
    if !found.is_dir() || found.permissions().mode() & 0o077 != 0 {
        return Err(NoPlace::NotPrivate(directory.to_path_buf()));
    }
    match fs::symlink_metadata(socket) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(refused(why)),
        Ok(there) if !there.file_type().is_socket() => {
            return Err(NoPlace::NotASocket(socket.to_path_buf()));
        }
        Ok(_) if UnixStream::connect(socket).is_ok() => {
            return Err(NoPlace::Taken(socket.to_path_buf()));
        }
        Ok(_) => fs::remove_file(socket).map_err(refused)?,
    }
    Ok(socket.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// A directory of this test's own, short enough for a socket's path.
    fn scratch(name: &str) -> PathBuf {
        let place = std::env::temp_dir().join(format!("alo-place-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&place);
        place
    }

    #[test]
    fn a_new_place_is_private() {
        let root = scratch("new");
        let socket = socket_under(&root);
        let _ = fs::create_dir_all(&root);
        assert_eq!(prepare(&socket), Ok(socket.clone()));
        let mode = socket
            .parent()
            .and_then(|directory| fs::metadata(directory).ok())
            .map(|found| found.permissions().mode() & 0o777);
        assert_eq!(mode, Some(0o700));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_place_others_may_enter_is_refused() {
        let root = scratch("open");
        let socket = socket_under(&root);
        let directory = socket.parent().map(Path::to_path_buf).unwrap_or_default();
        let _ = fs::create_dir_all(&directory);
        let _ = fs::set_permissions(&directory, fs::Permissions::from_mode(0o755));
        assert_eq!(prepare(&socket), Err(NoPlace::NotPrivate(directory)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_live_socket_is_left_and_a_stale_one_cleared() {
        let root = scratch("live");
        let socket = socket_under(&root);
        let _ = fs::create_dir_all(&root);
        assert!(prepare(&socket).is_ok());
        let live = UnixListener::bind(&socket);
        assert!(live.is_ok(), "bound: {live:?}");
        assert_eq!(prepare(&socket), Err(NoPlace::Taken(socket.clone())));
        drop(live);
        assert_eq!(prepare(&socket), Ok(socket.clone()), "nobody answers now");
        assert!(!socket.exists(), "and what was left is gone");
        let _ = fs::write(&socket, b"not a socket");
        assert_eq!(prepare(&socket), Err(NoPlace::NotASocket(socket.clone())));
        let _ = fs::remove_dir_all(&root);
    }
}

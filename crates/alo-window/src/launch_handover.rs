/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A second `alo` on the same profile hands its address to the one
//! already running (queue item 976, ADR 0114; ADR 0112, *Answered* 4).
//!
//! # Where the running alo is found
//!
//! **At the profile's agent socket** (ADR 0055): one alo per profile holds
//! it, and the operating system lets only its person open it. The profile
//! is the data directory a person named (`--data DIR`, its socket at
//! `DIR/alo-browser/agent.sock`) or, with none, the person's own (the
//! socket under their runtime directory) — [`socket_of_the_profile`]. A
//! launch with another `--data DIR` looks at another socket, finds nobody,
//! and is an instance of its own.
//!
//! # Nobody there, or a socket left behind
//!
//! A socket nobody answers at — nothing at the path, a socket an alo that
//! was killed left, anything the connection is refused by — is
//! [`Handover::NobodyThere`]: this alo becomes the instance, and placing its
//! own socket clears what was left ([`crate::socket_place::prepare`]). A
//! running alo is never asked whether it is one: a socket that answers is
//! the profile's alo, as `prepare` already holds it to be.
//!
//! # What is sent, and what comes back
//!
//! One line, `launched` with the address (`docs/contracts/agent-socket.md`),
//! answered as `open` with no tab is — once the card has opened, or why
//! not — and the window comes forward. The answer is waited for at most
//! [`ANSWERED_WITHIN`], a card's Chromium starting included, and written
//! for the person by [`crate::socket_plain`]: this process then ends, and
//! nothing of it stays.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The longest the running alo is waited for to answer: as long as the
/// tests wait for a card's Chromium to start and draw.
pub const ANSWERED_WITHIN: Duration = Duration::from_secs(90);

/// What came of handing an address to the running alo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handover {
    /// Nobody answers at the profile's socket: this alo is the instance.
    NobodyThere,
    /// The running alo answered: whether it opened the address, and what it
    /// said, written for a person.
    Answered {
        /// Whether the address opened.
        opened: bool,
        /// The answer, as `alo-ask --plain` writes it.
        said: String,
    },
    /// Handed over, and no answer came: why.
    Unanswered(String),
}

/// The profile's agent socket: in the data directory a person named, or —
/// with none — under the person's runtime directory, or nowhere when the
/// system names none.
pub fn socket_of_the_profile(data: Option<&crate::data_dir::DataDir>) -> Option<PathBuf> {
    match data {
        Some(data) => Some(data.socket()),
        None => crate::socket_place::runtime_directory()
            .map(|runtime| crate::socket_place::socket_under(&runtime)),
    }
}

/// Hand `address` to the alo answering at `socket`, waiting at most
/// `within` for its answer.
pub fn hand_over(socket: &Path, address: &str, within: Duration) -> Handover {
    // Nothing there, a socket left by an alo that stopped, or anything
    // else the connection is refused by: nobody answers.
    let Ok(stream) = UnixStream::connect(socket) else {
        return Handover::NobodyThere;
    };
    let unanswered = |why: std::io::Error| Handover::Unanswered(why.to_string());
    if let Err(why) = stream.set_read_timeout(Some(within)) {
        return unanswered(why);
    }
    let mut sending = &stream;
    if let Err(why) = sending.write_all(crate::protocol::launched_line(address).as_bytes()) {
        return unanswered(why);
    }
    let mut answer = String::new();
    match BufReader::new(&stream).read_line(&mut answer) {
        Ok(0) => Handover::Unanswered("the running alo closed the connection".to_owned()),
        Ok(_) => Handover::Answered {
            opened: crate::protocol::answer_of(&answer).as_deref() == Some("opened"),
            said: crate::socket_plain::written(&answer).trim_end().to_owned(),
        },
        Err(why)
            if matches!(
                why.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Handover::Unanswered(format!(
                "it did not answer within {} seconds",
                within.as_secs()
            ))
        }
        Err(why) => unanswered(why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// A directory of this test's own, short enough for a socket's path.
    fn scratch(name: &str) -> PathBuf {
        let place = std::env::temp_dir().join(format!("alo-976-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&place);
        // Private, as an alo's socket's directory is.
        let _ = std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
            .create(&place);
        place
    }

    #[test]
    fn nothing_there_and_a_socket_left_behind_are_nobody() {
        let root = scratch("nobody");
        let socket = root.join("agent.sock");
        assert_eq!(
            hand_over(&socket, "https://example.com/", ANSWERED_WITHIN),
            Handover::NobodyThere
        );
        // A socket a stopped alo left: there, and nobody listening.
        drop(UnixListener::bind(&socket));
        assert!(socket.exists(), "left behind");
        assert_eq!(
            hand_over(&socket, "https://example.com/", ANSWERED_WITHIN),
            Handover::NobodyThere
        );
        // And it is cleared as this alo takes the place.
        assert_eq!(crate::socket_place::prepare(&socket), Ok(socket.clone()));
        assert!(!socket.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A running alo of this test's own at `socket`, answering `answer` to
    /// the one line it is sent, which it hands back.
    fn answering(socket: &Path, answer: &'static str) -> std::thread::JoinHandle<String> {
        let listener = UnixListener::bind(socket).unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            if !answer.is_empty() {
                (&stream).write_all(answer.as_bytes()).unwrap();
            }
            line
        })
    }

    #[test]
    fn the_address_is_handed_over_and_the_answer_said() {
        let root = scratch("answered");
        let socket = root.join("agent.sock");
        let running = answering(
            &socket,
            "{\"answer\":\"opened\",\"id\":1,\"said\":\"opened https://example.com/\",\
             \"tab\":\"https://example.com/\",\"v\":1}\n",
        );
        assert_eq!(
            hand_over(&socket, "https://example.com/", ANSWERED_WITHIN),
            Handover::Answered {
                opened: true,
                said: "opened https://example.com/".to_owned()
            }
        );
        assert_eq!(
            running.join().unwrap(),
            "{\"address\":\"https://example.com/\",\"ask\":\"launched\",\"id\":1,\"v\":1}\n"
        );
        let _ = std::fs::remove_file(&socket);
        let refused = answering(
            &socket,
            "{\"answer\":\"refused\",\"id\":1,\"refusal\":{\"ask\":\"launched\",\
             \"kind\":\"NotOffered\"},\"said\":\"not offered\",\"v\":1}\n",
        );
        assert!(matches!(
            hand_over(&socket, "https://example.com/", ANSWERED_WITHIN),
            Handover::Answered { opened: false, .. }
        ));
        let _ = refused.join();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_running_alo_that_never_answers_is_said_to_have_not() {
        let root = scratch("silent");
        let socket = root.join("agent.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let handed = hand_over(&socket, "https://example.com/", Duration::from_millis(200));
        assert!(
            matches!(&handed, Handover::Unanswered(why) if why.contains("did not answer")),
            "{handed:?}"
        );
        drop(listener);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_profiles_socket_is_its_data_directorys() {
        let root = scratch("profile");
        let data = crate::data_dir::DataDir::at(&root.join("one")).unwrap();
        assert_eq!(
            socket_of_the_profile(Some(&data)),
            Some(root.join("one").join("alo-browser").join("agent.sock"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

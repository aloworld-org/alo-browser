//! Confining a renderer with the operating system's own sandbox.
//!
//! ADR 0010 decided this: rent the platform's mechanism, apply it before any
//! page bytes, and exit rather than render without one.
//!
//! # Why `sandbox-exec` rather than the library call
//!
//! macOS has two ways in. `sandbox_init` is a C function, which means FFI,
//! which means `unsafe` — and ADR 0010 says in a sentence of its own that it
//! **authorises no `unsafe` in this repository**. `sandbox-exec` is a program
//! that applies a profile and then execs, needing no FFI at all.
//!
//! It is also deprecated, and that is a real cost written down rather than
//! discovered later: Apple has marked it obsolete for years and it still ships.
//! If it is ever removed, the replacement is FFI to `sandbox_init`, and that
//! comes back for its own ADR naming the boundary — which is exactly the
//! arrangement ADR 0010 set up.
//!
//! There is a second advantage that is not a consolation prize: applying the
//! profile at `exec` means **the process is never unconfined**, not even for the
//! instant between starting and sealing itself. ADR 0010 rejected "apply it
//! after start-up" for that reason, and this route gets it for free.
//!
//! # What the profile allows, and why each line is there
//!
//! Deny by default, then the smallest set that lets a dynamically linked binary
//! start at all: the loader's own libraries, the caches it reads, and the
//! executable itself. Nothing under a person's home directory, nothing in
//! `/tmp`, no network, no writing anywhere, no starting anything.
//!
//! The list was arrived at by removing things until it stopped working, and the
//! test in `tests/a_renderer_is_confined.rs` watches a real refusal rather than
//! trusting that any of this took effect — which ADR 0010 asks for by name,
//! because a profile that was installed and permits everything reports success
//! exactly like one that works.

use std::path::Path;
use std::process::Command;

/// Why a renderer could not be confined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unconfined {
    /// In words.
    pub why: String,
}

impl core::fmt::Display for Unconfined {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "a renderer could not be confined: {}", self.why)
    }
}

impl std::error::Error for Unconfined {}

/// The profile, with the executable named by a parameter rather than pasted in.
///
/// A parameter because a path is text somebody else chose — a checkout under a
/// directory with a quote or a bracket in its name would otherwise end up
/// changing the meaning of the policy rather than filling in a blank. That is
/// the same class of bug as an injected quote anywhere else, and it is worse
/// here because the thing being injected into is a security policy.
#[cfg(target_os = "macos")]
const PROFILE: &str = r#"(version 1)
(deny default)
(allow process-exec*)
(allow sysctl-read)
(allow mach-lookup)
(allow file-read-metadata)
(allow file-read*
  (literal "/")
  (subpath "/usr")
  (subpath "/bin")
  (subpath "/dev")
  (subpath "/opt")
  (subpath "/System")
  (subpath "/Library")
  (subpath "/private/var/db")
  (literal (param "RENDERER")))
"#;

/// A command that runs this program confined.
///
/// # Errors
///
/// [`Unconfined`] on a platform this engine has no sandbox for. ADR 0010:
/// *"the browser does not claim a platform it cannot sandbox"* — so this is a
/// refusal rather than a fallback, and the caller must not run the program
/// itself instead.
#[cfg(target_os = "macos")]
pub fn confined(program: &Path, arguments: &[String]) -> Result<Command, Unconfined> {
    let path = program.to_str().ok_or_else(|| Unconfined {
        why: "the renderer's path is not text".to_owned(),
    })?;
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command
        .arg("-D")
        .arg(format!("RENDERER={path}"))
        .arg("-p")
        .arg(PROFILE)
        .arg(program)
        .args(arguments);
    Ok(command)
}

/// A command that runs this program confined.
///
/// # Errors
///
/// [`Unconfined`], always, on a platform this engine has no sandbox for yet.
/// Linux is queue item 169 — seccomp-bpf, a user namespace and Landlock, as
/// ADR 0010 names them. Until then this engine does not claim Linux, which is
/// what the ADR asks for instead of shipping with the protection off.
#[cfg(not(target_os = "macos"))]
pub fn confined(_program: &Path, _arguments: &[String]) -> Result<Command, Unconfined> {
    Err(Unconfined {
        why: "this engine has no sandbox for this platform, and ADR 0010 says a platform \
              without one is a platform it does not claim"
            .to_owned(),
    })
}

/// Whether this platform has a sandbox at all.
///
/// For a browser process deciding whether it can open a tab, and for a test
/// deciding whether it is testing anything.
pub fn is_available() -> bool {
    cfg!(target_os = "macos") && Path::new("/usr/bin/sandbox-exec").exists()
}

/// The things a confined renderer must not be able to do.
///
/// Run inside a renderer by `alo-render --check-confinement`, so that the check
/// is *of the renderer*, in the state it actually runs in — rather than of a
/// stand-in that shares only a profile. Each returns the error it got, or
/// `None` when it succeeded, which is the answer nobody wants.
pub mod probe {
    use std::io::ErrorKind;

    /// What an attempt to do something forbidden actually did.
    ///
    /// The distinction that makes this a test rather than a hope: **only a
    /// refusal by the operating system counts.** A connection that was refused
    /// because nothing is listening means the socket was created and the
    /// sandbox did nothing; a file that was not found means the open was
    /// allowed and the file is absent. Both look like failure and neither is
    /// confinement, and a probe that counted them would report a working
    /// sandbox on a machine with none.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Attempt {
        /// The operating system refused it. This is the answer we want.
        Refused {
            /// What it said.
            why: String,
        },
        /// It worked, or failed for a reason that is not confinement.
        Allowed {
            /// What happened, so a person can see which it was.
            what: String,
        },
    }

    impl Attempt {
        /// Whether the operating system refused it.
        pub fn was_refused(&self) -> bool {
            matches!(self, Attempt::Refused { .. })
        }

        fn of(outcome: std::io::Result<()>) -> Self {
            match outcome {
                Ok(()) => Attempt::Allowed {
                    what: "it worked".to_owned(),
                },
                Err(why) if why.kind() == ErrorKind::PermissionDenied => Attempt::Refused {
                    why: why.to_string(),
                },
                Err(why) => Attempt::Allowed {
                    what: format!("not confinement: {why}"),
                },
            }
        }
    }

    /// Try to read a file nobody has given us.
    pub fn reading_a_file(path: &str) -> Attempt {
        Attempt::of(std::fs::read(path).map(|_| ()))
    }

    /// Try to write one.
    pub fn writing_a_file(path: &str) -> Attempt {
        Attempt::of(std::fs::write(
            path,
            b"a renderer should not be able to do this",
        ))
    }

    /// Try to reach the network.
    ///
    /// Loopback on a port nothing is listening on. What is being asked is
    /// whether the *socket* may be made — so a connection refused because
    /// nobody answered is [`Attempt::Allowed`], and only the platform saying no
    /// is a refusal.
    pub fn opening_a_socket() -> Attempt {
        Attempt::of(
            std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], 9)),
                std::time::Duration::from_millis(200),
            )
            .map(|_| ()),
        )
    }
}

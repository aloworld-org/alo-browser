# ADR 0069 — A Chrome profile is held by a lock, and a start removes only the ones nobody holds

**Status:** accepted
**Date:** 2026-10-27
**Context:** queue item 690 (what a killed browser left, removed at the
next start), cut from 688; ADR 0068's amendment of 27 October (Quit runs
the shutdown, a killed browser runs nothing); ADR 0058 § 3 (each Chrome on
a new, private profile, `chrome_profile.rs`); ADR 0011 (what a session
may leave on a disk); law 4 (no `unsafe`)

## The decision in one line

Each profile holds a file, `alo.lock`, **locked by the browser that made
it for as long as that browser lives**; whenever a profile is made, the
browser removes every other `alo-chrome-*` directory of the same owner
whose `alo.lock` it can lock — which is only possible once its holder is
dead — and leaves everything else.

## Why this is a decision rather than a chore

Because the obvious answer is wrong in the direction that matters. A
profile's name carries the PID of the browser that made it
(`alo-chrome-PID-N-NANOS`), and "remove it if that PID is not running"
reads as enough. It nearly is: a PID reused only makes a dead browser's
profile look alive, which leaves it, the safe way round. But asking
whether a PID runs is `kill(pid, 0)`, a call `std` does not offer — `libc`
and `unsafe`, or a `ps` read and parsed — and a profile is in use by
**two** processes, not one: Chrome, the child, outlives a killed browser
until it reads its pipe closing (`chrome_over_a_pipe.rs` waits up to 90 s
for that and asserts only that it comes). And a start has to be sure
whose directory it is deleting at all: on Linux the temporary directory
is shared by every user.

Item 690 asks how a start knows "a profile is a dead browser's and not a
running one's — the PID in its name can be reused — without touching
another person's files". This answers all three.

## Measured first

On macOS 26.5.2, Rust 1.97.1, with a probe program in the scratchpad
using `std::fs::File::try_lock` (stable since 1.89, `flock(2)` on this
system):

- A lock taken by one process **refuses** another's `try_lock` with
  `WouldBlock`; the holder **`SIGKILL`ed**, the same `try_lock`
  **succeeds** — the kernel lets go of a dead process's lock, and nothing
  the dead process would have had to run is needed.
- A lock taken on a file under one name and kept through a **rename** to
  another is still held under the new name.
- Two opens of one file **in one process** refuse each other, so a
  browser sweeping cannot take its own live profile's lock.
- A lock's file opened by `std` is not inherited by a child: with the
  holder's handle dropped and a child spawned meanwhile, the lock was
  free. Chrome does not hold it.
- The fifteen profiles this machine held on 27 October (`TMPDIR`, 8 KB
  to 68,580 KB, fourteen of the day before) carry no `alo.lock`: they
  were made before this. Thirteen no longer hold Chrome's own
  `SingletonLock` and two still do, so Chrome's lock says nothing
  reliable about whether a profile is in use — and it is a link naming a
  host and a PID, the reused-PID question again.

## 1. The lock

`new_profile` makes the directory as it does today (0700, `Local State`
written), then creates `alo.making` in it with `create_new`, takes its
lock, and renames it to **`alo.lock`**. The handle is kept in the `Chrome`
value beside the profile's path, and dropped **after** the directory is
removed, so the lock is held for the whole life of the profile and given
up only by the browser removing it or by the browser dying.

The rename is what makes the lock safe to read: a sweep only ever opens a
file named `alo.lock`, and a file has that name only once it is locked, so
there is no moment when a live browser's profile has an `alo.lock` that
nobody holds. A directory that has no `alo.lock` — one being made, one a
start has half removed, one an older build left — is never removed by a
sweep.

## 2. The sweep

When `new_profile` has made a profile, it reads the temporary directory
and, for each entry named `alo-chrome-*` other than its own:

- **Its own entry, not followed.** `symlink_metadata`: a directory, not a
  link, and **owned by the same user as the profile just made** — whose
  owner is this user by construction, so no `getuid` and no `unsafe`. A
  directory of anybody else's is not opened.
- **`alo.lock` is a file, not a link**, again by `symlink_metadata`. The
  directory is 0700 and this user's, so nobody else can put one there
  between that look and the open.
- **Its lock is taken** with `try_lock`. `WouldBlock` means a living
  browser holds it, and the directory is left. Any other failure — gone
  already, unreadable — leaves it too.
- **Holding the lock, the directory is removed** (`remove_dir_all`, which
  does not follow links out of it), and the lock is let go after. Two
  starts sweeping at once cannot both hold it, and the second finds the
  file locked or gone.

A Chrome still ending after its browser was killed may write into a
directory being removed, and the removal may then fail. That is not an
error to answer: the lock is still nobody's, and the next start's sweep
removes it.

## 3. What is said

A profile removed is a session's data gone from a disk (ADR 0011), and
removing somebody's data is not done silently. `new_profile` answers the
profiles it removed and the ones it could not, and `alo` says on its
terminal how many it removed, once, when it removed any. Making the new
profile does not fail because a sweep did: what the sweep could not do is
reported, and Chrome starts.

## What this does not do

- **Profiles made before this are left.** With no `alo.lock`, nothing
  says whether their browser is alive, and a guess by age or by PID is the
  guess this decision exists to avoid. They do not grow. A person may
  delete them; the journal says where they are.
- **No sweep at any other time.** Not on a timer, not at Quit, not from
  `alo-bundle`. A start is the one moment a browser is sure it is alive and
  about to put another profile on the disk.
- **Nothing outside the temporary directory**, and nothing not named
  `alo-chrome-*`.
- **No claim about speed.** The sweep reads one directory per Chrome
  started; this machine's temporary directory held 7,399 entries.

## Consequences

- Queue item 690 is decided and buildable: `chrome_profile.rs` gains the
  lock and the sweep, `chrome.rs` keeps the handle, `alo.rs` says what was
  removed. It closes as written: a start after a browser was killed
  removes that browser's profile and no running browser's, in a test that
  kills one.
- `a_new_profile_is_private_and_holds_only_the_local_state` will hold
  `alo.lock` as well; the test is changed with the code, not before.
- No `unsafe`, no rented crate, no new dependency: `File::try_lock`,
  `symlink_metadata`, `MetadataExt::uid` and `remove_dir_all` are `std`.

## Amendment, 28 October 2026 — the lock file is removed last

Found building it (iteration 432). `remove_dir_all` on a profile may
take `alo.lock` out before it fails on something else in it — a
directory that may not be emptied, or a Chrome still ending that writes
a file in after the listing. What is left then has no `alo.lock`, and
§ 2 never removes a directory without one: the remainder would stay for
good, against § 2's own "the next start's sweep removes it".

So a profile — swept, or its own as its browser closes — is removed
with everything in it but `alo.lock` first (`remove_dir_all` for what
is a directory, never following a link), then `alo.lock`, then the
directory. If only that last step fails, an empty `alo.lock` is put
back, which nobody holds, so the next sweep finds it. Asserted by a
profile emptied inside a directory it may not be removed from.

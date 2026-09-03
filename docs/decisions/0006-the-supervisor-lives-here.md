# ADR 0006 — The supervisor lives here

**Status:** accepted
**Date:** 2026-09-03
**Context:** `docs/autonomy/LOOP.md` (*Running it*); `scripts/gate.sh`;
`alo-workplace/scripts/run-loop.sh`, which is what this replaces

## The decision in one line

The thing that runs the build loop is **a script in this repository**,
`scripts/loop.sh`, written for macOS and owned by the people who own the loop
it drives — not a borrowed script belonging to another repository.

## What it replaces, and why that had to change

`LOOP.md` said the supervisor lived in `alo-workplace` and took a repository
path, *"so this repository stays Rust."* Three things were wrong with that.

**The premise was already false.** `scripts/gate.sh` is six and a half thousand
bytes of bash, in this repository, and it encodes the gate from `CLAUDE.md`. The
repository does not stay Rust; its *engine* does, which is what law 4 is
actually about. A shell script that runs a command and reads a journal is not
the thing memory safety was an argument about.

**The borrowed script is shaped for a repository that is not this one.** It
takes a `--track` argument, because `alo-workplace` runs several queues in
parallel and keeps a journal per track. This repository has one queue and one
journal. The track machinery is not configuration here; it is a second concept
that has to be understood and then ignored, and it defaults to the right answer
by luck rather than by design.

**Nobody could fix it.** `LOOP.md` forbids touching another repository, and it
is right to: `alo-workplace` is read-only reference. So the command in `LOOP.md`
was wrong — it passed `--repo`, which that script has never parsed, so the flag
became the path and the checkout became a track name — and it stayed wrong,
because the only place to fix it was somewhere the loop was not allowed to go.
That is the real cost of borrowing: **a dependency that cannot be maintained by
the people depending on it.** It was found by running the command, not by
reading it, and it had been in `LOOP.md` since the file was written.

## What is kept from the borrowed one

Reading `alo-workplace` to learn what correct means is exactly what `LOOP.md`
asks for, and `run-loop.sh` carries scar tissue worth having. Four of its
comments describe incidents, and every one of those lessons is carried over:

- **The end-marker is matched anchored to the start of a line**, and tolerates
  a heading or bold prefix. An unanchored match once found `LOOP COMPLETE`
  quoted in the middle of a journal and stopped a loop with 58 items open,
  reporting success.
- **The hang guard is idle-based, not duration-based.** A hung worker goes
  silent; an honest long item keeps writing to its transcript. A duration-only
  guard once killed ninety minutes of real work.
- **One supervisor per repository, machine-wide, with a lock**, because stopped
  wrappers have survived as detached processes and spawned rival workers that
  edited the same checkout.
- **A non-zero exit backs off rather than spins.** Restarting immediately into
  a rate limit spends a model call to be told to wait.

## What is dropped, and what is new

Dropped: tracks, and the GNU `stat -c` fallback. This one is for macOS, so
`stat -f %m` is simply correct and the fallback that once poisoned the age
check on Git Bash cannot exist.

New, and the reason this is worth its own file rather than a copy:

- **It refuses to run past a boundary that belongs to a person.** `LOOP.md`
  says stage 2 ends at a judgement a loop must never certify on somebody's
  behalf. The supervisor stops on `LOOP COMPLETE` and prints what a person now
  has to decide, rather than treating the marker as a transient failure.
- **It runs the gate before it starts**, and refuses to begin on a red tree. An
  iteration that opens on somebody else's failure will either work around it or
  spend itself diagnosing it, and both are worse than not starting.
- **`--once` and `--dry-run`**, so the thing can be understood without
  committing to an unattended run. The borrowed script could only be understood
  by reading it or by letting it loose.

## Why bash and not Rust

The engine is Rust because a wrong pixel and a memory bug are what this project
is about. A supervisor spawns a process, watches a file's modification time and
greps a journal — it is the same kind of thing as `scripts/gate.sh`, it lives
next to it, and making it a workspace member would mean the loop that builds
the browser has to compile before it can build the browser.

If it ever grows past that — if it starts making decisions about *what* to
build rather than *when to stop* — that is the signal it has become part of the
engine, and this decision should be revisited rather than stretched.

## What this costs

A second implementation of something that already existed, and it will drift
from `alo-workplace`'s. That is accepted: the two repositories have different
queues, different journals and different stage boundaries, and the shared thing
was never the script — it was the four lessons above, which are now written
down in prose here rather than inherited by copying a file nobody may edit.

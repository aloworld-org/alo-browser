# The alo browser build loop

One iteration builds **one queue item**, completely, and stops. A supervisor
runs iterations until the journal says the queue is done or that something is
wrong.

The loop exists because the work in `ROADMAP.md` is long and mostly independent,
not because it is unsupervised. Every iteration ends in a commit that met the
gate in `CLAUDE.md`, or in a halt that says why it could not.

## What one iteration does

1. **Read `docs/autonomy/QUEUE.md`.** Take the first item that is not done and
   is not blocked. If every remaining item is blocked, write `LOOP COMPLETE`
   into the journal with the blocked list, and stop — a loop that keeps
   re-reading impossible work is a loop burning somebody's money.
2. **Read what the item names.** The ADR it implements, the contract it must
   satisfy, the section of `docs/features.md` that promised it. An item that
   cannot name those is not ready to build; mark it `needs design` and take the
   next one.
3. **Build it whole** — input, validation, policy, execution, record, error
   paths. Law 3: no stubs, no `todo!()`, no `unwrap()` outside tests. If it is
   turning out larger than one iteration, **cut its scope, never its depth**,
   and write the cut into the queue as a new item rather than leaving a
   half-built one.
4. **Pass the gate**, all of it. `scripts/gate.sh` runs the mechanical half —
   `cargo fmt`, `cargo clippy` with zero warnings *and* zero errors, the tests,
   no stubs, no `unsafe` opt-out, every rented crate still behind its one file,
   a `CHANGELOG.md` line — and prints the half it cannot run. That half is
   still the gate: a **layout assertion in numbers** for anything that positions
   or sizes, a **reference render** for anything visual, one responsibility per
   file, and the item's section in `docs/features.md`. A green script is not a
   passed gate on its own.
5. **Commit and push.** One item, one commit, a message that says what changed
   and why somebody would care.
6. **Update the queue and the journal.** Tick the item. Append to
   `docs/autonomy/STATE.md`: what was built, what the gate said, and anything
   the next iteration should know.
7. **Stop.** One item per iteration. Two is how a bad decision gets made twice
   before anybody reads the first one.

## What stops the loop

Write one of these as a line of its own in `STATE.md`:

- **`LOOP COMPLETE`** — every item is done or blocked, with the blocked ones
  listed and why.
- **`LOOP HALT`** — something is wrong that the loop must not work around: the
  gate fails for a reason the iteration did not cause, a decision is needed that
  is not ours, a test that used to pass has started failing, or the same item
  has failed twice.

**Halting is not failure.** An iteration that halts with a clear reason is worth
more than one that invents a way past a problem nobody has looked at.

## What the loop may never do

- **Never weaken the gate to pass it.** Not a lowered lint, not an ignored test,
  not a `#[expect]` added to silence something real. If the gate is wrong, halt
  and say so.
- **Never tick an item it did not finish.** `ROADMAP.md` says a tick means done,
  not written; the same rule applies here, and the loop is exactly where that
  rule would erode first.
- **Never add `unsafe`** without an ADR (law 4), and never a verb that takes
  a coordinate (ADR 0002).
- **Never accept "it needs hardware" as a reason something is unverified.** That
  rule was inherited from `alo-os`, where it is true and where most of a release
  really does end on a certified machine. Here it is false — and it still named
  `alo-os`'s `v0.01` rather than this repository's stages, which is how the copy
  was spotted. **Stage 1 produces files: a PNG and a box tree.** Anything in it
  can be verified on the machine the loop is already running on, so an item left
  unverified is an item left *unfinished*, and the honest word for that is halt.

  What genuinely cannot be verified here is short, and none of it is in stage 1:
  hardware acceleration, embedding into a compositor that does not exist, and
  any claim about speed. A performance claim is measured on hardware or not made.
- **Never let another repository's absence block an item.** `alo-os` not being
  checked out is a fact about this machine, not about the engine. It once held a
  finished item open and put "not met" into three documents. If an item seems
  blocked on a sibling repository, the gate is wrong — halt and say so.
- **Never touch another repository.** `alo-os` and `alo-workplace` are read-only
  reference here — read them to know what "correct" means, never edit them.

## Where things are

| | |
|---|---|
| `docs/autonomy/QUEUE.md` | The work, in order, with what each is blocked on |
| `docs/autonomy/STATE.md` | The journal: one entry per iteration, newest last |
| `CLAUDE.md` | The four laws and the gate |
| `docs/decisions/` | Why things are the way they are. Read before proposing otherwise |

## Running it

The supervisor lives in `alo-workplace`, and takes a repository path — so this
repository stays Rust:

```
powershell -ExecutionPolicy Bypass -File <alo-workplace>/scripts/run-loop.ps1 -RepoPath "<this checkout>"
```

Stop it any time. Every finished item was committed and pushed by the iteration
that built it, so nothing is lost by interrupting one.

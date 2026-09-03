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
6. **Update the queue, the roadmap and the journal.** Tick the queue item.
   Append to `docs/autonomy/STATE.md`: what was built, what the gate said, and
   anything the next iteration should know.

   **Then open `ROADMAP.md` and move the line this item served** — in the same
   commit. A queue item is usually smaller than a roadmap line, so most often
   what you write is the `· Built: … · Owed: …` clause described at the top of
   that file rather than a tick.

   If the item served no roadmap line, **say so in `STATE.md` and say why**.
   That is a real answer; silence is not. Silence is what happened: fifteen of
   twenty-eight consecutive commits left `ROADMAP.md` untouched, and stage 2's
   first line still read as unstarted after its ADR was accepted *and* its
   boundary was built.

   **Never resolve this by ticking.** A tick means the line is finished, and
   `ROADMAP.md` says a tick means done, not written. A loop that learns to tick
   in order to discharge an obligation is worse than one that never updated the
   file at all.
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
  really does end on a certified machine. Here it is mostly false. **Stage 1
  produced files: a PNG and a box tree**, and every one of its items was
  verified on the machine the loop was already running on.

  Stage 2 produces files too, nearly all of it: a parse tree, a set of headers,
  a JavaScript value, a display list. An item left unverified is an item left
  *unfinished*, and the honest word for that is halt.

  What genuinely needs hardware is a short list, and an item on it says so in
  the queue: hardware-accelerated paint, WebGL and WebGPU, media decode on a
  real device, embedding into a compositor that does not exist, and **any claim
  about speed at all**. A performance claim is measured on hardware or not made.
- **Never let another repository's absence block an item.** `alo-os` not being
  checked out is a fact about this machine, not about the engine. It once held a
  finished item open and put "not met" into three documents. If an item seems
  blocked on a sibling repository, the gate is wrong — halt and say so.
- **Never touch another repository.** `alo-os` and `alo-workplace` are read-only
  reference here — read them to know what "correct" means, never edit them.

## Stage 2, which is different in four ways

Stage 1 had one question — *does alo render?* — and one answer: a committed PNG
and a committed box tree. Stage 2 has neither, and pretending otherwise is how
a loop starts marking things done because they compile.

### 1. A real page decides, and the page is frozen

`ROADMAP.md`: *"the trigger for most items is a real page that fails, never a
specification listing a method."* So an item is opened by a page that does not
work and closed by the same page working, and the page goes in the corpus with
the change.

**A corpus case never touches the network.** It is a frozen copy — the bytes as
they were the day they were taken, with where they came from and when written
beside them. A suite that fetched would be flaky, would fail on an aeroplane,
and would hand every site's owner the ability to break our build. If a page
cannot be frozen, it cannot be a case.

**Freezing is not scraping the web at will.** Take the smallest thing that
fails, from a site whose terms allow it, and say in the case where it came
from. A case nobody could re-derive is a case nobody can check.

### 2. The bytes are hostile now

Stage 1 rendered markup we wrote. Stage 2 parses what strangers send, and
ADR 0005 is built on the assumption that some of it is trying to get out.

So the gate gains one clause for **anything that reads bytes from outside**: a
test that feeds it malformed, truncated and adversarial input, and a guarantee
that it **returns an error rather than panicking**. `unwrap`, `expect`, `panic`
and slice indexing are already denied by the lints; what those do not catch is
arithmetic that overflows on a hostile length and a rented crate that panics on
input we passed straight through. Refusing is a result. Crashing is a bug, and
in a renderer it is a denial of service.

### 3. Order is decided by dependencies, not by taste

`ROADMAP.md` names the two things that gate the rest: the process model, which
cannot be retrofitted, and JavaScript, which most of the list is unreachable
without. The queue writes each item's dependencies down. **Take the first item
whose dependencies are all done** — which is not always the first item in the
file, and the queue says so where it matters.

An item nothing depends on and which nothing makes reachable is an item to
leave alone until it is.

### 4. A decision is its own iteration

Stage 2 contains several things that are decisions rather than chores: what our
JavaScript engine is and is not, what a permission is, what a quota policy is,
which codecs we rent and on whose terms. An item marked **needs ADR** gets the
ADR *as its own iteration*, before any code depends on it, exactly as ADR 0005
came before `alo-renderer`.

`CLAUDE.md`: *"Settled decisions live in `docs/decisions/`. Read the ADR before
proposing an alternative."* A decision made inside a commit that was mostly
code is a decision nobody reviewed.

### And one thing that does not change

**This stage is years of work, and the loop does not get to be optimistic about
it.** `ROADMAP.md` says so at the top of stage 2. A loop that reports progress
by the number of items ticked will tick the small ones; the honest report is
which roadmap lines moved, which is what step 6 already asks for.

## Crossing from one stage to the next

The queue covers all four stages, so the loop can always say what is next. At
two places what is next is **a person**, and saying so is the answer rather
than a failure to find work.

### When a stage's queue empties

Do not move to the next stage. Check that stage's exit gate, in `ROADMAP.md`,
and write what you found into `STATE.md`. Only stage 1's gate was a thing a
machine could check — two screens diffed against committed files — and it was
met that way. The rest are not.

### Stage 2 ends where a loop cannot follow

Its exit gate is *"a person uses it as their browser for a week and reaches for
another one only for a site they can name"*, and *"an agent completes a real
task on a site nobody wrote for us"*. **A loop must never certify that on
somebody's behalf.** When stage 2's queue is empty, write `LOOP COMPLETE`,
name what a person now has to do, and stop.

This is not the loop giving up. It is the one honest answer to a gate that is
about somebody's experience rather than about a file.

### Stage 3 is opened by pages, not by the queue

Every stage 3 item is `blocked: no page yet`, and that is its real state.
**Take one only when a corpus case fails because of it** — never because it is
the next unticked line. `ROADMAP.md`: *let a broken render schedule the work.*

A loop that started stage 3 items to have something to do would be building the
legacy tail for its own sake — and refusing to do that is what made stages 1
and 2 survivable in the first place.

### Stage 4 does not start until a person says so

Every item is `blocked: stage 2's exit gate`. That block is lifted by a person,
in a commit, and never by an iteration deciding the gate looks met.

### So when is `LOOP COMPLETE` actually right?

When every remaining item is blocked and the blocks are real — which, after
stage 2, means: stage 3 waiting on pages nobody has hit yet, and stage 4
waiting on a judgement only a person makes. List both, say which is which, and
stop.

## Where things are

| | |
|---|---|
| `docs/autonomy/QUEUE.md` | The work, in order, with what each is blocked on and what closes it |
| `crates/alo-corpus/cases/` | The pages an item is judged against. Frozen, never fetched |
| `docs/autonomy/STATE.md` | The journal: one entry per iteration, newest last |
| `ROADMAP.md` | What somebody outside the loop reads to know where this is. Moved every iteration, per step 6 |
| `CLAUDE.md` | The four laws and the gate |
| `docs/decisions/` | Why things are the way they are. Read before proposing otherwise |

## Running it

The supervisor is `scripts/loop.sh`, in this repository, for macOS (ADR 0006).

```sh
scripts/loop.sh              # run until the journal says to stop
scripts/loop.sh --once       # one iteration, then exit
scripts/loop.sh --dry-run    # say what it would do, start nothing
scripts/loop.sh --self-test  # check the stop rule, start nothing
```

It refuses to begin on a tree where `scripts/gate.sh` does not pass, because an
iteration that opens on somebody else's failure will either work around it or
spend itself diagnosing it. It takes a lock, so a second supervisor on the same
machine is refused rather than left to edit the same files. A worker that goes
silent for twenty minutes is presumed hung and killed, and the item it was
building is redone next time — silence rather than elapsed time, because an
honest long item keeps writing and a hung one does not.

Stop it any time. Every finished item was committed and pushed by the iteration
that built it, so nothing is lost by interrupting one.

**It stops on its own at a boundary a person has to cross** — see the section
above. A supervisor that restarted the loop past a `LOOP COMPLETE` would be
answering a question that was asked of somebody else.

### Writing a marker so the supervisor can see it

Write `LOOP COMPLETE` or `LOOP HALT` **as its own line, at the end of the
journal**, optionally as a heading or in bold. Quoting either one mid-sentence
is safe: the supervisor only matches a line that starts with it.

And a marker is **only live while it is the last thing in the journal**. This
file's own journal has `LOOP COMPLETE` at line 1531 of 2500-odd — stage 1
finished, said so, and a person started stage 2 underneath it. A supervisor
that simply searched for the words would find that one, stop on its first tick,
and report the queue complete with ninety-nine items open: the failure that
looks exactly like the work being done. So an iteration entry written after a
marker retires it, which means **resuming a halted loop is done by appending,
not by editing history**.

### The one that came before

Until ADR 0006 this was `alo-workplace/scripts/run-loop.sh`, kept there so this
repository would "stay Rust" — a premise `scripts/gate.sh` had already made
false. It is left alone now, and it should be: a script belonging to a
repository this loop may not edit is a dependency nobody here can maintain,
which is how the command documented in this very file came to be wrong (it
passed `--repo`, which that script has never parsed) and stayed wrong.

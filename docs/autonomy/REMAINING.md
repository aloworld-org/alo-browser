# Remaining roadmap and execution loop

Audit date: 2026-09-07. `ROADMAP.md` owns scope and stage gates;
`QUEUE.md` owns implementation order and closing conditions. These counts are
an inventory snapshot, not a completion percentage or an effort estimate.

| Stage | Roadmap state | Queue inventory | What closes it |
|---|---|---|---|
| 1: renders alo | 11 done; exit gate met | 29 done, 0 open | Sign-in and Settings reference images and box trees, plus named agent activation |
| 2: modern web | 9 done, 73 open or partly built | 73 done, 84 open | A person uses it for a week; an agent completes a real external-site task with an audit record |
| 3: legacy tail | 8 open | 8 open | No fixed completion gate; actual failing pages schedule individual work |
| 4: adoption | 8 open | 9 open | Somebody outside alo chooses it on their own machine and stays |

Stage 1 also has three separate follow-ups outside its met gate: GPU paint,
OS compositor embedding, and measured hardware performance. They are not in
the stage 1 queue count. Stage 4 has more queue items than roadmap lines because
extensions include a separate design decision. A partly built line remains open.
There are **101 open queue items** overall; splitting broad items will change
that number without changing the amount of work owed.

## Continuation order

Iteration 119 halted at item 222's reachable named-expression binding scope:
the required frozen real-script failure has not been established. Supply that
case with provenance and a failing execution assertion before implementing this
scope. Existing lexer/parser corpus tests do not close it. No item or stage was
completed, and this halt does not mean the queue or roadmap is finished.
Iteration 120 marked item 222 blocked in the queue's own words.

Iteration 122 wrote item 207's decision, ADR 0015 (`BigInt` limbs rented from
`num-bigint`, sizes and spellings ours). Its code half is blocked the same way
222 is: no frozen real script uses a `BigInt`. The next unused ADR is 0016;
the next unused queue number remains 223.

Iteration 123 built item 212 at its first step, `[[Construct]]` and `new`,
opened by a frozen real script: alo's service worker, which now compiles past
its `new Request(…)` and stops at an array literal (item 211). Classes,
`super`, `new.target` and private names are cut to item 223 and `instanceof` to
item 224, so 102 queue items are open. The next unused queue number is 225; the
next unused ADR remains 0016. The service worker's next refusal names item 211,
whose dependencies (73, 75) are not built — a real-script trigger for 211 is
not the same as its dependencies being met.

1. **Item 205's plain-header scope is finished.** Iteration 117 rechecks
   function headers under the body's strictness and preserves the remaining
   early errors and import attributes as item 222. Item 60 is `needs design`;
   item 187 still awaits an upload caller, and item 169 needs Linux execution.
   Follow actual queue dependencies; item 207 requires an ADR-only iteration
   before implementation. The next unused queue number is 223.
2. **Item 219 is finished.** Native calls can suspend and resume; the
   recursion limit counts waiting builtins as well as script frames. Its
   previously unbounded test now passes, including collection stress. Item 221
   retains `apply` and the traced scratch storage it needs, so the total number
   of open items remains 101 despite closing 219. Continue with the queue's
   first eligible item; the JavaScript branch can now take item 220.
3. **Continue JavaScript's dependency chain.** Item 220 covers function
   metadata/source and binding; item 73 must be split into bounded builtin
   families with explicit closing conditions. Constructors (212), exceptions
   (210), parameter forms (213), proxies (217), tagged templates (215) and
   destructuring (211) depend on pieces of this work. Resolve dependencies per
   item rather than treating this paragraph as a new queue order.
4. **Complete process/network prerequisites as they become eligible.** Linux
   sandbox probes (169) require execution on Linux; this macOS checkout cannot
   certify them. Remaining font axes (197), request causality, HTTP behaviour,
   encrypted DNS settings and storage-access policy retain their queue gates.
5. **Connect script to live pages.** The event loop, DOM wrappers/mutation,
   events, forms, navigation, frames, storage and workers unlock useful web
   interactions. Split large capabilities when starting, preserve their
   remainders, and freeze the smallest allowed real-page case that demonstrates
   each missing behaviour.
6. **Finish the modern browser surface.** CSS/text/input, media, incremental
   rendering, window/tabs, accessibility, developer tools and the agent's
   permission/audit model all remain stage 2 work. Use the queue's actual
   prerequisites. Hardware and usability claims need the specified evidence.
7. **Exercise stage 2's real exit gate.** Record the week's browser use,
   fallback sites and external agent task. Queue exhaustion cannot stand in
   for that evidence. Then eligible stage 3 page failures and stage 4 product
   work can proceed under the existing stage rules. Stage 3 is not a finite
   promise that a loop can mark globally complete.

## The repeated operation

Read the current queue and journal → choose the first eligible bounded item →
read its ADR and feature contract → implement and exercise error paths → run
its tests and the full gate → update features, changelog, roadmap and journal →
commit locally → supervisor verifies the gate and recorded progress → repeat.

`scripts/loop.sh` already implemented this workflow; it is extended rather than
replaced with a competing supervisor. `LOOP.md` contains the worker contract.
Do not loosen a gate, manufacture a real-page trigger, or erase unfinished work
to keep the loop running. A blocked stage is reported with what would unblock
it. `LOOP COMPLETE` means no eligible work remains, not all stages are finished.

## Run and resume

Prerequisites: macOS, Rust toolchain, authenticated `codex` CLI, a clean
checkout, and a passing `scripts/gate.sh`. No supervisor was started by this audit or by the follow-up cleanup. The
item 219 recursion blocker is resolved. The follow-up validation and commit
are recorded in `STATE.md`; start from a clean checkout after the full gate.

```sh
scripts/loop.sh --dry-run
scripts/loop.sh --self-test
scripts/test-loop.sh
scripts/gate.sh
scripts/loop.sh                # up to 500 iterations, or a stop condition
```

`--items N` bounds a run, and `--once` verifies one iteration. Keep the session
alive; the script is not a scheduled service. Progress goes to
`docs/autonomy/loop.log`, commits and `STATE.md`. After interruption, inspect
preserved work, finish or repair it, verify and commit, then rerun. Retire a
resolved halt with a new journal iteration rather than deleting history.
Publishing changes is separate from this local build loop.

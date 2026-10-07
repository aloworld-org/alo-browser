# ADR 0029 — A regular expression is ours, and the work it does is counted

**Status:** accepted
**Date:** 2026-10-08
**Context:** ADR 0001 (our own engine in Rust) and its *rent the physics, build
the engine*; ADR 0013 § 3 (*absent beats approximate*, and the legacy tail
refused by name), § 4 (*every allocation a script can cause has a ceiling we
chose … and the work a regular expression may do (item 74)*, and the
interpreter is interruptible by the embedder), § 8 (Unicode tables, case
folding among them, are rented; the lexer, parser, compiler, interpreter,
object model and collector never are); ADR 0014 § 9 (ceilings are numbers in
`bounds.rs`, not in a decision); ADR 0015, which rented `BigInt` arithmetic
*because its size could be computed before the stranger's code was called*;
`docs/autonomy/QUEUE.md` item 74, whose closing condition is *a hostile
pattern is refused or bounded rather than running for ever — a catastrophic
backtrack in a renderer is a denial of service*; and `alo-downloads`, alo's
public download page, whose script is the first running frozen script to
reach a regular expression (iteration 198)

## The decision in one line

The regular expression engine — the pattern's parser, its compiler and the
matcher that runs it — is **ours**, written in `alo-js` as a **backtracking
machine over UTF-16 code units with a stack of its own**; **every step it
takes is counted against a budget we chose and the embedder's stop is checked
inside it**, so a pattern that would run for ever is a `RangeError` the page
can catch or a script the browser stopped, never a renderer that hangs; and
the only thing rented is the Unicode tables ADR 0013 § 8 already rents.

## Why this is a decision rather than a chore

Item 74 was written as a feature. It hides two decisions that the first line
of code would otherwise make by accident.

**Ours or rented.** A safe-Rust engine for exactly this syntax exists:
`regress`, a backtracking engine that targets JavaScript's regular expression
syntax and can match UTF-16 text directly. ADR 0015 shows renting is not
refused on principle here; it was the right call for `BigInt`. So the
refusal has to be argued, not assumed.

**What happens to a pattern that never ends.** `/(a+)+$/` against thirty
`a`s and a `b` is a few bytes of a stranger's page and, in a naive
backtracker, more steps than the machine will take before somebody closes the
tab. The language specifies no limit and no error for it. Whatever this
engine does is a choice, and an unwritten choice is "hang".

## 1. The matcher is ours, because the bound can only live inside it

ADR 0015 rented `BigInt` arithmetic on one condition: *no rented function is
ever called until our code has computed, from the sizes of its operands, how
large its result can be*. That condition is what made renting safe. The cost
of a multiplication is known before it starts.

**The cost of a regular expression is not.** Whether `/(a+)+$/` finishes in
ten steps or ten billion depends on the pattern, the input and every choice
the matcher makes on the way, and nothing outside the matcher can know which
before it runs. The only place a bound on that work can be enforced is
**inside the backtracking loop**: a counter it increments and a flag it
reads. A rented matcher that does not carry such a counter cannot be given
one from outside, and `regress`'s documentation names no bound on the work a
match may do. Its own comparison with the `regex` crate says that crate
*provides linear-time matching guarantees, while regress does not*.

So renting it would mean one of three things, and each is refused:

- **Calling it unbounded.** That is exactly what item 74's closing condition
  forbids, and what ADR 0013 § 4 lists by name.
- **Running it on a thread and abandoning it.** `alo-js` has no threads and
  no clock (ADR 0013 §§ 5, 7), and an abandoned thread still burns the
  processor it is on.
- **Forking it to add a counter.** A fork is our code that nobody here
  wrote. It has to be read line by line to place the counter correctly, and
  kept in step with an upstream that does not want the change. It is ours
  in every way except the one that would let us reason about it.

The bound is ADR 0013 § 4's, and ADR 0013 § 1 already says why: *a limit
somebody else chooses is not a limit*. A matcher is an interpreter of a small
language a stranger wrote, and § 8 already lists the interpreter as never
rented. This decision applies that to the second interpreter in the engine.

**What is rented** is what § 8 already rents: Unicode tables. Case folding
for `i` under `u` and `v`, the upper-case mapping that `i` uses without them,
and the property tables that `\p{…}` names (`General_Category`, `Script`,
`Script_Extensions`, the binary properties, and the string properties `v`
adds). They are data, and nobody's engine differs by them. Each rented crate
sits behind one file on `scripts/gate.sh`'s boundary list, as
`unicode-id-start` does. Which crate is chosen when the item that needs it is
built. The choice must reach nothing, add no `unsafe` of ours, and leave
`alo-js` depending on no I/O crate.

## 2. A backtracker, because that is what the language means

The specification defines a pattern's meaning as a backtracking search. Each
construct is a *matcher* that tries its choices in order and continues with
the rest of the pattern. Lazy and greedy quantifiers, alternation's order,
captures reset at each iteration of a quantifier, backreferences and
lookaround in both directions are all stated in terms of that search.

**A backtracker is therefore the only engine that is correct for every
pattern.** A linear-time engine (a Thompson or Pike machine) cannot do
backreferences or lookaround, and it can reproduce the search's choice of
match and captures only for a subset of patterns. It is an optimisation for
that subset. Law 3 says correct first. A second engine for the subset is
reopened only by a measurement on hardware naming a page, as ADR 0013 § 2
reopens a JIT. Even then it would sit beside the backtracker, never in place
of it.

The shape follows ADR 0013's choices for the main engine, for the same
reasons:

- **Parsed to a tree, compiled to instructions, run by a loop.** Not a tree
  walked recursively. The parse is bounded in nesting depth and in source
  length, and is refused with a `SyntaxError` naming the bound, as a script's
  is.
- **The matcher never calls itself.** A choice point is an entry on a stack
  the matcher owns, not a native call. A pattern's nesting and an input's
  length therefore cannot choose how much of the process stack is used. The
  backtrack stack has a ceiling of its own, in `bounds.rs`, and reaching it
  is the same refusal as exhausting the step budget (§ 3).
- **It reads code units.** An `alo-js` string is UTF-16 code units, as the
  language's is. Without `u` or `v` a pattern matches code units. With
  either, it matches code points and never splits a surrogate pair. Indices
  are code units in both modes, because that is what `lastIndex` and a
  match's `index` are.

## 3. The work is counted, and running out is a `RangeError`

**Every step of the matcher is counted** against a budget for one call of
the matcher: one `RegExpBuiltinExec`, which is what `exec`, `test` and each
iteration of a global `replace` or `split` reach. A step is one instruction
executed, so a backtrack's re-entry counts as well. The number is a constant
in `bounds.rs`, with the reason beside it (ADR 0014 § 9). It is chosen when
the matcher exists, so it can be set high enough that no frozen page's
pattern comes near it. A number written here would be a number nobody could
tune with evidence.

**Running out throws a `RangeError`,** with a message saying the regular
expression did too much work. The alternatives:

- **Answering "no match."** That is approximate, and § 3 of ADR 0013 refuses
  approximate. A page would take a different branch on a result the
  language never produced, and nobody could tell why.
- **Stopping the script uncatchably.** That is the embedder's decision, not
  the engine's, and it is still available (below). For the engine to take it
  would treat a validator's pattern written badly as a page that has stopped
  answering, which is a person's judgement.
- **A `RangeError`.** This is the precedent the engine already set. The
  language specifies no limit on call depth either, and here a recursion
  that will not end is a `RangeError` a page can catch (item 210). A regular expression that will not end is the same kind
  of thing: a resource the page asked for more of than we give. The page's
  own `catch` is how it survives us.

**The embedder's stop is checked inside the matcher** as well, at least
once per bounded number of steps and on every backtrack. The budget bounds
one call. It does not bound a script that calls a bounded matcher in a loop
for ever. The stop does that, as it does for every other loop (ADR 0013
§ 4), and the stop unwinds the matcher like any other frame.

Neither of these is a claim about speed. The budget is a ceiling on work,
not a time, and the engine still has no clock.

## 4. The syntax is the language's, and Annex B is refused by name

The grammar is the current specification's: the `u` and `v` modes, named
groups (and the same name in two alternatives), lookbehind, and the
`(?i:…)` modifiers. Every flag the lexer already accepts (`dgimsuvy`) means
what the specification says it means, or the pattern is refused with a
message naming the piece that is not built yet. A flag is never accepted and
then ignored.

**Annex B's extensions to the pattern grammar** are the web's legacy:
a lone `{` or `]` as a literal, `\c` with a digit or `_` inside a class,
an octal escape, a quantified lookahead, and the rest of B.1.2. They are
refused, each **by name**, as a `SyntaxError` that says it is an Annex B
form. That is ADR 0013 § 3's rule for the legacy tail, applied to the
second grammar in the engine.
Each form is opened by a frozen page that uses it. A page that hits one gets
a refusal it can act on, not a pattern that means something different here.

The patterns in this repository's frozen scripts need none of it. They are
`alo-downloads`' four (`/Mac/`, `/Mac OS X/`, `/Win/`, `/Windows/`) and
the theme generator's (`/\/\*[\s\S]*?\*\//g`, `/\s+/g`,
`/^(danger|success|warning|unread)/` and the like).

## 5. A `RegExp` is an object in the heap, and its program is held by it

A literal is compiled once, when its script is compiled. That way a pattern
that cannot be compiled is an early `SyntaxError` for the whole script, as
the specification requires. Each evaluation of the literal makes a new
`RegExp` object holding the compiled program, its source and its flags, and
a `lastIndex` own property. The program is immutable and shared by every
object made from the same literal. The collector traces the object, and the
program holds no heap references, so it needs no tracing.

The protocol a string method uses to reach a regular expression
(`Symbol.match`, `Symbol.replace`, `Symbol.search`, `Symbol.split`,
`Symbol.matchAll`) is the specification's. It is built when `String.prototype`
is, which is item 73's. A builtin is never given a private shortcut that
skips a page's override of one of those methods.

## 6. The cut

Item 74 keeps the engine and its bound. Its first cut is what
`alo-downloads` reaches: the parser for the whole grammar (with what is not
built yet refused by name), the compiler, the matcher with its budget, its
stack ceiling and the stop, a `RegExp` from a literal, `RegExp.prototype.exec`
and `test`, `lastIndex` with `g` and `y`, and `s` and `m`. Three cuts are
opened as items of their own:

- **322. `i`, and `\p{…}`: the rented Unicode tables.** Depends on 74. The
  first cut refuses `i` and `\p` by name, because both are wrong without
  their tables.
- **323. The string methods that take a regular expression.**
  `String.prototype.match`, `matchAll`, `replace`, `replaceAll`, `search`
  and `split`, and the `Symbol.*` methods of `RegExp.prototype` that they
  call. Depends on 74 and on `String.prototype` (item 73).
- **324. The `RegExp` constructor, `v`'s set operations, and `d`'s
  indices.** `new RegExp(source, flags)` compiles at run time, so a
  pattern's `SyntaxError` is thrown where the call is. It also covers the
  `source`, `flags` and per-flag accessors, `v`'s class set operations, and
  the `indices` array `d` adds. Depends on 74.

Each is opened as a queue item now, so that none of it is left half-built
inside 74.

## What this costs

- **A regular expression engine is weeks of work** that `regress` has
  already done, and the specification's matcher semantics are subtle enough
  to be wrong in ways a table of small cases has to find. test262's
  `built-ins/RegExp` sections are where those cases come from, vendored per
  feature and read as a table (ADR 0013 § 9).
- **We will be slower than engines with a linear-time path** on patterns
  that have one. Nothing here is measured, and nothing is claimed.
- **Some page's legitimate pattern will exceed the budget** that another
  engine finishes. It is a `RangeError` naming the budget, which is a bug
  report somebody can act on. Without a budget it would be a hung renderer,
  and that is not.
- **Pages that use Annex B's pattern forms are refused** until one is
  frozen. Old code and some minified libraries do use them. This is the
  same trade ADR 0013 § 3 made for octal literals.

## Alternatives rejected

**Renting `regress`.** Safe Rust, the right syntax, UTF-16 matching, and it
exists. Refused because the bound this item exists for has to live inside its
loop, and § 1 explains why none of the three ways to put it there is
acceptable. This is the closest call in the ADR. Reconsidering it is
legitimate if it ever carries a step budget the embedder chooses. Doing so
silently is not.

**The `regex` crate, or `regex-automata`.** Linear time, which answers the
bound in the best possible way. Refused as the engine because it has no backreferences and no lookaround,
which real pages use. It also searches UTF-8, not the UTF-16 code units a
JavaScript string is, and its captures do not follow JavaScript's rule of
resetting inside a quantifier. A pattern routed to it would mean something
slightly different. That is the approximate answer § 3 of ADR 0013 refuses.

**A recursive backtracker, as the specification's continuations are
written.** It is the most direct transcription, and the nesting of a
stranger's pattern and the length of their input would then decide how deep
the process stack goes. Refused, for the reason ADR 0013 § 4 refuses it for
the parser.

**Answering "no match" when the budget runs out.** Refused in § 3.

**Memoising the backtracker** so that every (instruction, position) pair is
tried once. It makes many catastrophic patterns linear. It costs memory
proportional to the pattern times the input, it does not survive
backreferences, and it is an optimisation of a matcher that does not exist
yet. It can be reopened with measurements, under § 2's rule.

## What this does not decide

- **The budget's number and the stack's ceiling.** They belong in
  `bounds.rs`, with reasons, when the matcher exists (ADR 0014 § 9).
- **Which crate supplies the Unicode tables.** That is item 322, under § 1's
  conditions.
- **The legacy static properties** (`RegExp.$1`, `RegExp.lastMatch` and the
  rest). They are legacy, are not in `docs/features.md`, and are opened only
  by a page.
- **How a regular expression appears to an agent or in a stack trace.**
  That is item 78's, as any other frame is.

## How we will know if this was wrong

**If frozen pages exceed the budget** with patterns another engine finishes
quickly, the budget is wrong or the matcher needs § 2's second engine. The
answer is a measurement and a change to `bounds.rs` or an amendment here, not
a quietly raised number.

**If frozen pages keep failing on Annex B forms**, § 4 drew the line in the
wrong place. It moves in an amendment with the pages named, exactly as ADR
0013's own *how we will know* says for the language.

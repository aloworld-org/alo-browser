# ADR 0015 — `BigInt` arithmetic is rented, and its size is ours

**Status:** accepted
**Date:** 2026-10-05
**Context:** ADR 0001 (our own engine in Rust), whose *rent the physics, build
the engine* is the whole question here; ADR 0013 § 3 (*absent beats
approximate*), which is why there is no `BigInt` today rather than a `BigInt`
held in a double; ADR 0013 § 4 (*every allocation a script can cause has a
ceiling we chose*, and the interpreter is interruptible only at points it
defines); ADR 0013 § 8, which already rents number-to-string as *the arithmetic
equivalent of renting a shaper* and lists what is never rented; ADR 0014
(one heap, one collector), whose § 9 says ceilings land in `bounds.rs` rather
than in a decision; ADR 0010's *whose `unsafe` this is*; ADR 0009, whose
licence a rented crate has to sit beside; `docs/autonomy/QUEUE.md` item 207,
which asks for this decision by name and states the tension in two sentences —
*renting it puts a stranger's allocator in the path of numbers a page chooses
the size of … writing it is a well-specified week nobody enjoys*

## The decision in one line

The limb arithmetic of a `BigInt` — add, multiply, divide, shift, the bitwise
operations and base conversion — is **rented from `num-bigint`**, behind one
file; everything the specification spells is **ours**; and **no rented
function is ever called until our code has computed, from the sizes of its
operands, how large its result can be and refused it if that is past a ceiling
we chose** — so the stranger's allocator only ever allocates what we already
agreed to, and the stranger's code is only ever handed an argument it cannot
panic on.

## Why this is a decision rather than a chore

Item 207 was cut from 206 because a `BigInt` is not a variant to add. The
lexer keeps a literal's digits as text (`token::Kind::BigInt`), the compiler
refuses one by name, and `object/value.rs` says there is no `BigInt` and why.
Every one of those was waiting for an answer to the same question, and the
answer decides three things that cannot be changed afterwards without touching
every operator: whose code does the arithmetic, where the value lives, and
which side of a call a bound is checked on.

The question is genuinely two-sided, which is why it gets an ADR rather than a
dependency line.

**For renting:** arbitrary precision is physics in exactly ADR 0001's sense.
Nobody's engine differs by how it multiplies two numbers, the specification
says nothing about how, and the hard parts — long division that is right on
every carry, multiplication that is not quadratic on large operands, and base
conversion that is not quadratic either — are where a hand-written
implementation is wrong in a way no test written by its author notices. ADR
0013 § 8 rented number-to-string for precisely this reason, and a `BigInt` is
the same argument with more surface.

**Against renting:** a page chooses the size of a `BigInt`. `2n ** 100000000n`
is fifteen bytes of somebody else's script, and a rented library asked to
evaluate it will try. Every bound in this engine is written against that
clause — *a limit somebody else chooses is not a limit* — and a call into
somebody else's arithmetic is a call that neither checks our ceiling nor
answers our interrupt.

## 1. What is rented, and why that is safe to rent

**Rented: the limbs.** Addition with carry, subtraction with borrow,
multiplication (`num-bigint` uses Karatsuba and Toom-3 above a size, so a large
product is not quadratic), division with remainder, shifts, two's-complement
bitwise operations, and the digits of a number in a radix. These are
operations on magnitudes that the specification defines only by their
mathematical value, so an engine cannot be *more* right about them than a
correct library — only wrong in a different place.

**Why the objection above does not survive.** A `BigInt` operation is the one
kind of arithmetic whose **result size is known before it is computed**, from
the operands' bit lengths alone:

- `a + b` and `a - b` are at most one bit longer than the longer operand;
- `a * b` is at most the sum of their lengths;
- `a / b` and `a % b` are at most the length of `a`;
- `a << n` is at most the length of `a` plus `n`;
- `a ** n` is at most the length of `a` times `n`;
- `&`, `|`, `^` and `~` are at most one bit longer than the longer operand;
- a literal or a `BigInt("…")` of `d` digits in radix `r` is at most
  `d × ⌈log₂ r⌉` bits.

So the bound is **ours, and checked first**. The adapter computes the largest
the result can be, in arithmetic that cannot itself overflow, compares it with
the ceiling, and refuses before the rented function is reached. The allocation
the queue item worried about is then an allocation of a size we already
accepted — which is the same position `alo-net` is in when it hands a length it
has already bounded to `flate2`.

**Why the interrupt objection does not survive either.** ADR 0013 § 4 makes
the interpreter interruptible at points *it* defines, and a single rented call
is not one of them. That is acceptable for exactly one reason, and the reason
is a constraint on the ceiling: **the ceiling bounds work as well as memory.**
The slowest operations at a given size are division and base conversion, so
the ceiling is chosen so that the worst single operation on the largest
permitted operands finishes in a time nobody can see — and that time is
*measured*, on this machine, and written beside the constant the way
`STACK_FOR_A_PARSE` writes down its measurement. A loop of ten thousand such
operations is then ten thousand interruptible instructions, which is what the
interpreter's interrupt already handles.

**Why the collector argument from ADR 0013 § 1 does not apply.** Renting an
engine was refused partly because a rented collector decides how the DOM is
stored. A `BigInt` holds no references at all — it is a leaf, like a string —
so renting its arithmetic decides nothing about the object graph, and the
heap's accounting sees it as a cell of a size the adapter states.

## 2. What is ours

Everything the specification spells, because spelling is not physics and every
engine has to agree on it to the character — the same line `numeric.rs` draws
for doubles:

- **The value and where it lives.** A `BigInt` is a primitive whose magnitude is
  a **heap cell**, immutable once made, exactly as a string is: `Value` gains a
  variant carrying a `Ref`, equality and `SameValue` compare magnitudes rather
  than slots, and the cell reports its size to the heap from its bit length so
  [`HEAP_CEILING`](../../crates/alo-js/src/bounds.rs) counts it. No type of
  `num-bigint`'s appears outside the adapter file, so `Value`, the heap and the
  interpreter never name it.
- **The specification's operations**: `BigInt::add` and the rest as the
  specification names them, the `TypeError` for mixing a `BigInt` and a
  Number in arithmetic, the `RangeError` for division by zero and for a
  negative exponent, unsigned right shift refused with the `TypeError` the
  language specifies, `typeof` answering `"bigint"`, and `ToBoolean`.
- **Comparison across types.** `1n < 1.5`, `2n ** 64n == 18446744073709551616`
  and `NaN` against a `BigInt` are specification algorithms that compare
  **exactly** — never by converting the `BigInt` to a double first, which is the
  approximate answer this engine refuses.
- **Conversion to and from a Number.** `Number(x)` rounds to nearest, ties to
  even, from the top bits the adapter hands over; `BigInt(1.5)` is the
  `RangeError` the language gives. Both are written, because they are where a
  page sees the difference between two engines.
- **Text, both ways.** The `StringToBigInt` grammar — whitespace, the `0x`,
  `0o` and `0b` prefixes, no sign beside a prefix, no separators, no `n` — and
  `toString(radix)` with its lower-case digits, its `-` and its `RangeError` for
  a radix outside 2 to 36. The adapter returns digits as numbers and this code
  spells them, so the rented crate's own string formatting is never called.

## 3. No rented function is reached with an argument it panics on

`LOOP.md`'s stage 2 clause 2: *a rented crate that panics on input we passed
straight through* is a denial of service. `num-bigint` panics on three things,
and each is refused on our side before the call:

- **Division or remainder by zero** — a `RangeError` the specification asks for
  anyway, so the check is the specification's step rather than a guard.
- **A shift by a count out of range** — the count is a `BigInt` the page chose,
  and it is reduced to a bounded size by the result-size check before it is
  ever a machine integer.
- **An exponent whose result would overflow memory** — the `**` row above,
  checked first.

The adapter's contract is therefore a **precondition list**, one per rented
function, and the test that closes the code half of item 207 feeds each
precondition's violation through the interpreter and asserts a catchable error
rather than a stopped process.

## 4. Which crate, and on whose terms

**`num-bigint`**, with default features off apart from `std` — so no random
number generator is linked, which ADR 0013 § 5 forbids this crate anyway.

- **Pure Rust.** Its `unsafe` is a carry intrinsic on x86-64 that the standard
  library has since made safe, and an unchecked UTF-8 conversion inside its own
  string formatter, which § 2 never calls. Under ADR 0010's rule that is **the
  crate's `unsafe`, not ours**: this decision authorises no `unsafe` in this
  repository, and `unsafe_code = "forbid"` stays on `alo-js` unchanged.
- **MIT or Apache-2.0**, which sits beside MPL-2.0 under ADR 0009 as every other
  rented crate here does.
- **The widely used one.** It is the arbitrary-precision crate most of the Rust
  ecosystem depends on, including the Rust JavaScript engine ADR 0013 refused,
  so its bugs are found by more people than ours would be.
- **Behind one file**, `crates/alo-js/src/bigint.rs`, added to `scripts/gate.sh`'s
  boundary list in the commit that adds the dependency — as `unicode_id_start`
  is behind `unicode.rs`.

The crate is the replaceable part. Because § 2 keeps every spelling and § 1
keeps every bound on our side, swapping it for another library — or for code of
our own — touches one file and none of the tests that say what a page sees.

## What this costs

- **A dependency in the engine's own crate.** `alo-js` has had one rented crate,
  a Unicode table. This is the second, and it is the first that runs code on a
  page's values rather than looking one up. The precondition list in § 3 is the
  price, and it is paid in tests.
- **A ceiling some page may hit.** The specification sets no maximum; every
  engine picks one. A page that legitimately needs more bits than ours gets a
  `RangeError` it can catch and a bug report somebody can act on.
- **No interrupt inside one operation.** Bounded by the ceiling, as § 1 says,
  and that is the reason the ceiling must be measured rather than guessed.

## Alternatives rejected

**Write it ourselves.** Rejected, and it is the closer call. It would keep
every line ours and allow an interrupt check inside a long division. But the
bound and the interrupt objections are both answered by checking size first,
which a rented library permits as well as our own code does, and what writing
it buys after that is a second, younger implementation of physics — the trade
ADR 0001 refuses for a shaper and ADR 0013 § 8 refused for number-to-string.
Re-opened if § 3's precondition list ever grows to something a person cannot
hold in their head, because then the library is no longer a black box we can
bound from outside.

**A double, or a 128-bit integer with a refusal above it.** Rejected by ADR
0013 § 3. A double is approximate, which is the answer this engine refuses; a
128-bit integer is exact but refuses the sizes pages use `BigInt` *for* —
cryptographic and hashing code works in hundreds and thousands of bits — and it
would be a second implementation to delete when the real one arrived.

**GMP, through `rug`.** Rejected: fastest, and C. It is the QuickJS trade in ADR
0013 in a smaller package, in the process that runs a stranger's script.

**`malachite`.** Rejected on licence: LGPL-3.0, which ADR 0009 already explains
fits a statically linked Rust binary badly.

**`ibig` or `dashu`.** Not rejected on merit — both are pure Rust and fast — but
on reach: fewer people depend on them, and speed is a claim this repository
makes only on hardware, after a measurement, which nobody has taken. Swapping to
one later is § 4's one-file change.

## What this does not decide

- **The ceiling itself.** A named constant in `crates/alo-js/src/bounds.rs`,
  with its measurement beside it, landed by the commit that builds the value —
  ADR 0014 § 9: *a ceiling written into an ADR is a number nobody can tune with
  evidence*.
- **`BigInt64Array` and `BigUint64Array`**, which belong to typed arrays, and
  **`BigInt.asIntN`/`asUintN`**, which are builtins in item 73's library. Both
  sit on this representation and neither changes it.
- **When the code is built.** `LOOP.md`'s stage 2 clause 1 still applies: the
  implementation is opened by a frozen real script that fails on a `BigInt`, and
  closed by the same script working. No frozen script in the corpus uses one
  today, so this decision is written first, as `LOOP.md` asks of a decision,
  and the code waits for its page.

## How we will know if this was wrong

**If a frozen real page hits the ceiling** doing something legitimate, the
ceiling is wrong, and it moves with that page as its evidence — never quietly.

**If a rented panic ever reaches a renderer**, § 3's precondition list was
incomplete. The fix is the missing precondition and a test that feeds it; if
the list keeps growing, writing the arithmetic ourselves is the alternative
above that this ADR names the condition for.

**And if `num-bigint` stops being maintained**, § 4 already made leaving it a
one-file change.

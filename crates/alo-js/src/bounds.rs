/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The ceilings a script cannot walk past, and why each is the number it is.
//!
//! ADR 0013 § 4: *every allocation a script can cause has a ceiling we chose*,
//! for the reason `alo-net` gives in every file — **a limit somebody else
//! chooses is not a limit**. A bound with no reason beside it is a number
//! nobody can argue with later, so each one here says what it protects and what
//! a page hitting it would look like.
//!
//! The lexer needs the first two. A lexer allocates in proportion to its source
//! and nothing else — a string's characters, a name's bytes and a token's span
//! are all cut out of the text that was already read, so bounding the text
//! bounds all of them — and it does not recurse, so a million open brackets is a
//! million tokens and no stack at all. The parser is the thing that recurses,
//! which is why the third arrived with it.
//!
//! The rest are the heap's, and ADR 0014 § 9 says explicitly that they land here
//! rather than in the decision: *a ceiling written into an ADR is a number
//! nobody can tune with evidence*. None of them is a claim about speed —
//! `LOOP.md` says such a claim is measured on hardware or not made, and nothing
//! here has been measured on any.

/// The most source text we will read, in bytes.
///
/// Sixty-four mebibytes. The largest bundles anybody ships are a few megabytes
/// and a page that concatenated all of its script into one file would still be
/// far under this; a script above it is a page trying to decide how much memory
/// this process uses. A page that hits it gets a refusal naming the size, which
/// is a bug report somebody can act on — unlike a renderer that stopped.
pub const LONGEST_SOURCE: usize = 64 * 1024 * 1024;

/// How deep the parser will **recurse**.
///
/// Two hundred and fifty-six. The parser is recursive descent, so a nesting
/// level is a handful of stack frames, and a script chooses its own nesting —
/// twenty thousand `[` is four bytes of somebody else's file and a stack
/// overflow in ours, which is an abort rather than a refusal and which
/// ADR 0013 § 4 forbids outright.
///
/// Hand-written code does not reach a tenth of it: the deepest thing a person
/// writes is a nested conditional, and a bundler's output nests wide rather
/// than deep because its input was written by people too. What does reach it is
/// a file written to reach it, which is the case this exists for.
///
/// It is paired with [`STACK_FOR_A_PARSE`], and neither number means anything
/// without the other. It is **not** the depth of the tree that comes out —
/// that is [`DEEPEST_EXPRESSION`], and reading this one as though it were both
/// is what let `a+a+a+…` build a tree sixty thousand deep.
pub const DEEPEST_NESTING: usize = 256;

/// How deep a tree the parser will **build**, along one path through it.
///
/// Four thousand and ninety-six, and it is a different question from
/// [`DEEPEST_NESTING`] rather than a larger answer to the same one.
///
/// [`DEEPEST_NESTING`] bounds how deep the parser **recurses**, which is why it
/// is small: a bracket costs thirteen stack frames, and the ceiling is measured
/// against [`STACK_FOR_A_PARSE`]. Some of this grammar builds a level of tree
/// without recursing at all — `a.b.b.b…`, `a()()()…`, `a+a+a+…`, `a||a||a…`,
/// a run of tagged templates and every `?.` link are read in a **loop** that
/// nests what it has already built inside what it reads next. A thousand of
/// those cost the parser one frame and build a tree a thousand deep.
///
/// That tree is then walked by everything downstream, one frame per level, and
/// the first walker is `Drop`: a chain of sixty thousand `+` parses in a
/// fraction of a second and aborts the process when the program is let go of.
/// A compiler (queue item 72) is the second. So the bound belongs on the
/// **tree** rather than on any one reader of it, and it is here rather than in
/// each of them.
///
/// The number is what those walkers can afford on the smallest stack any of
/// them runs on. A `cargo test` thread has two mebibytes and drops a tree
/// sixteen thousand deep without trouble in a debug build, so four thousand is
/// that with the margin a measurement taken on one machine deserves. It is
/// sixteen times [`DEEPEST_NESTING`] because a level here costs one frame
/// rather than thirteen, and it is far above what a written program reaches:
/// the longest `+` chain a bundler emits is a few hundred, and `a.b.c.d` is
/// four.
///
/// A path through the tree may also pass through [`DEEPEST_NESTING`] levels of
/// recursion, so the deepest tree this parser will build is the sum of the two.
/// Both are counted, neither is inferred from the other, and a program that
/// reaches either is refused by name.
pub const DEEPEST_EXPRESSION: usize = 4096;

/// The stack a parse is given, which is why [`DEEPEST_NESTING`] is a number we
/// chose rather than a number somebody else did.
///
/// Thirty-two mebibytes, and the reasoning is the one every bound in this
/// crate has: *a limit somebody else chooses is not a limit*. A recursive
/// descent parser's real ceiling is how much stack it was called on, and that
/// is a property of the caller — a `cargo test` thread has two mebibytes, a
/// process's first thread has eight, and a renderer will have whatever
/// somebody set. So the parser does not use the caller's stack at all: it runs
/// on a scoped thread of its own with this much, and [`DEEPEST_NESTING`] then
/// means the same thing everywhere, in a debug build and a release one alike.
///
/// The number is measured rather than guessed at. The most expensive nesting
/// this grammar has is a bracket — thirteen frames a level, since an array
/// element is a whole assignment expression — and **256 of them needs under
/// twelve mebibytes in a debug build**, which is the worst case because a
/// release build reuses stack slots and needs roughly a fifth of it. Thirty-two
/// is that with the margin a measurement taken on one machine deserves.
///
/// Reserved rather than used: an operating system commits the pages a thread
/// touches, so a script that nests four deep costs four levels of stack and
/// not this.
pub const STACK_FOR_A_PARSE: usize = 32 * 1024 * 1024;

/// The most a page's objects may hold, in bytes.
///
/// One gibibyte. ADR 0014 § 9: reaching it collects first and fails second, and
/// failing means an error the script or the embedder is told about rather than
/// an abort.
///
/// The reasoning is the process model rather than the machine. ADR 0005 gives a
/// **site its own renderer**, so this number is multiplied by however many sites
/// a person has open — and somebody with a dozen tabs must not be able to reach
/// a dozen times a machine's memory before anything says a word. A gibibyte is
/// far above what the heaviest single-page application legitimately holds and
/// far below what a page would need to make a laptop swap, which is the band
/// this belongs in.
///
/// It is a number to argue with once there is evidence: the honest report is
/// that no page has yet been measured against it, because nothing in this
/// engine runs a page.
pub const HEAP_CEILING: usize = 1024 * 1024 * 1024;

/// How many bytes a script may allocate before the collector runs.
///
/// Eight mebibytes. ADR 0014 § 9: the trigger is **bytes allocated since the
/// last collection, never a clock** — ADR 0013 § 5 gives this crate no clock and
/// ADR 0014 does not hand it one, which is also why a collection is a thing a
/// test asks for rather than a thing a test waits for.
///
/// The number bounds *growth* rather than pause: a program allocating rubbish
/// in a loop is collected every eight mebibytes of it, so a heap that holds
/// nothing stays a heap that holds nothing. Whether it is the right number for a
/// pause somebody can see is a measurement nobody has taken, and ADR 0014's own
/// section on how we will know this was wrong says what to do when somebody
/// does.
pub const COLLECT_AFTER: usize = 8 * 1024 * 1024;

/// The longest string a script may make, in UTF-16 code units.
///
/// 2²⁸−1, which is 268 million code units and half a gibibyte of memory —
/// deliberately below [`HEAP_CEILING`], so that **one string cannot be the
/// whole heap**. A page that could allocate a string of the ceiling's size
/// would have a way to make every later allocation fail while holding one
/// object, which is a denial of service written in one line of `repeat`.
///
/// The specification allows a string of 2⁵³−1 code units, which no engine has
/// ever been able to make; every engine picks a smaller number, and picking one
/// is what stops a length computation overflowing somewhere further in. It is
/// far above what a page legitimately holds as a single string — the largest
/// such thing is usually a document's own source, a few mebibytes — so a script
/// that reaches this is one building a string to see what happens.
///
/// Reaching it is a `RangeError` the script can catch (queue item 72), which is
/// what the language specifies for a string that cannot be made, rather than
/// the [`Full`](crate::heap::Full) that a heap at its ceiling produces.
pub const LONGEST_STRING: usize = (1 << 28) - 1;

/// How many references the marker's worklist holds.
///
/// Sixty-four thousand, which is half a mebibyte of them, taken once when the
/// heap is made and never grown. ADR 0014 § 8: *a collection allocates nothing
/// it has not already got*, because the moment we most need to collect is the
/// moment there is nothing spare — and the size of a worklist that grew would be
/// chosen by whoever wrote the script, which is `alo-net`'s sentence in a
/// different crate: **a limit somebody else chooses is not a limit**.
///
/// Overflowing it costs a rescan and never correctness: the mark bits are the
/// truth and the worklist is only a list of what to look at next. So this is a
/// number that trades work against memory and cannot trade away an answer, which
/// is why it can be small.
pub const MARKING_WORKLIST: usize = 64 * 1024;

/// How many ephemeron pairs the marker holds while it iterates them.
///
/// Sixteen thousand, a quarter of [`MARKING_WORKLIST`] and for the same
/// reasons. A pair is a `WeakMap` entry whose key was reached, so a page holding
/// more than this many live weak entries at once pays for a rescan rather than
/// losing one: ADR 0014 § 7 asks for a fixpoint that never drops an entry a
/// chain of maps keeps live, and a bound that could drop one would be that
/// promise broken quietly.
pub const MARKING_EPHEMERONS: usize = 16 * 1024;

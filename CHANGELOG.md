# Changelog

What changed, in words a person outside this repository can read. Newest first.

---

## Unreleased

- **Style sheets parse into rules we hold.** `cssparser` tokenises and
  `selectors` matches; what a rule is, which selectors exist, and what happens
  to what we do not implement are ours. A sheet in the shape alo's design
  system is written in — custom properties throughout, a light and a dark
  theme behind `prefers-color-scheme`, a width breakpoint — parses whole, and
  an engine can now ask which rules apply to which element of a document.
- **Nothing is dropped in silence.** An unknown property is kept with its
  value, so a later stage can implement it without re-parsing the sheet; an
  at-rule we do not implement is kept whole for the same reason; a selector we
  cannot evaluate takes its rule down, which is what CSS says, and says so.
  Every one of those is reported with the text that caused it and the line it
  was on.
- **Media queries: width and `prefers-color-scheme`.** A condition we do not
  understand is treated as not matching rather than guessed at — the
  alternative is a dark theme leaking into a light one, which looks like a
  rendering bug and is not one.
- **`:hover` and `:focus` parse and never match**, because nobody is hovering
  a document being rendered to a file, and **`:visited` never matches at all**:
  whether a link has been followed is history, and a style that depends on it
  is readable back off the page. `:disabled`, `:checked`, `:required` and
  `:read-only` do match, from the markup, including the awkward parts — a
  disabled `<fieldset>` reaches its controls but not the ones in its first
  `<legend>`.

- **There is a document tree.** `html5ever` parses HTML and alo browser holds
  the result: nodes, attributes, parents and children, in our own types rather
  than the parser's. A document round trips — parse it, write it back out, and
  the text is the same — and malformed input still produces a usable tree, with
  a record of everything the parser had to repair to get one.
- **Every node has an identity that is never reused** (ADR 0003). An id names a
  node for the life of its document, and a node that leaves the tree keeps it,
  so a stale id names something that is gone rather than something else that is
  not. This exists in the first commit of the tree because the agent surface
  will stand on it, and identity cannot be retrofitted.
- **Quirks mode is recorded and never honoured.** A document that would put
  another engine into quirks mode is laid out as standards, because law 1
  refuses quirks mode outright. What the parser observed is kept for
  diagnostics rather than thrown away.
- `unsafe` is now forbidden by the compiler rather than by review, across the
  whole workspace.
- **The gate is a command that fails**, not a paragraph to remember:
  `scripts/gate.sh`. It runs the formatter, the linter and the tests, refuses a
  stub, refuses a crate that has quietly opted out of the ban on `unsafe`, and
  checks that each rented crate is still named in only the one file that is
  allowed to name it. What it cannot check, it names, so that a green run is
  never mistaken for the whole gate.
- The scope is written down: what gets built, in which stage, and what will not
  be built at all. The engine renders the modern platform and refuses thirty
  years of legacy — no quirks mode, no floats-as-layout, no CSS-table layout —
  because refusing them is what makes a project this size survivable. Stage 1
  needs no JavaScript engine, which removes a browser's largest single component
  from the critical path.
- alo browser exists as a decision and a queue. Nothing renders yet.

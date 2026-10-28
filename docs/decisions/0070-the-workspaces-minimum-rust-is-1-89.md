# ADR 0070 — The workspace's minimum Rust is 1.89

**Status:** accepted
**Date:** 2026-10-28
**Context:** queue items 690 and 691; ADR 0069 (a Chrome profile held by
`File::try_lock`); law 4; iteration 144, which worked around the minimum
rather than raise it

## The decision in one line

`rust-version` moves from **1.85 to 1.89**, decided by the owner's
session, and the workspace is made clippy clean at it in the same change.

## Why

1.85 was never chosen for anybody. It is the first Rust that has edition
2024, which the workspace declared in its first commit, and nothing since
has said that someone builds this with an older compiler than the one
here (1.97.1). A minimum that serves nobody is not worth a rented crate or
`unsafe`, which are the only ways to a lock the kernel lets go of at a
process's death on a `std` before 1.89 (item 691) — and both cost more
than a line in `Cargo.toml`: a crate is a dependency to review and keep
behind a boundary, and `unsafe` needs a boundary and an ADR of its own.

## What it cost

At 1.89 clippy finds what 1.85 hid from it: `collapsible_if` — nested
`if let`s that let-chains (1.88) now write as one — and
`manual_is_multiple_of`, in `alo-js`, `alo-net`, `alo-dom`, `alo-paint`
and `alo-text`'s tests. Each was applied by `cargo clippy --fix` and
formatted, a rewrite of the same condition, and the whole suite gated
after. No lint was allowed and nothing was silenced.

## Consequences

- `File::try_lock` stands as ADR 0069 chose it, and 690 is gated on it.
- Anyone building this needs Rust 1.89 or later. The minimum moves again
  only for a reason written down like this one.

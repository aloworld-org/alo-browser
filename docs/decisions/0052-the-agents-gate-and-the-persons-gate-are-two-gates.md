# ADR 0052 — The agent's gate and the person's gate are two gates

**Status:** accepted
**Date:** 2026-10-12
**Context:** `ROADMAP.md`'s stage 2 exit gate, which this changes; `CLAUDE.md`
law 2 (the layout tree is the agent's tree) and ADR 0002; queue item 131, built
2026-10-11; `ROADMAP.md`'s *Where it runs*, and alo OS's own assessment of this
repository, quoted below

## The decision in one line

Stage 2's exit gate is **two gates, met separately and dated separately** — an
agent completing a real task on a page nobody wrote for us, and a person using
this as their browser for a week — and the first does not wait for the second.

## Why this is a decision rather than a chore

The gate was written as one sentence with a semicolon in it:

> A person uses it as their browser for a week and reaches for another one only
> for a site they can name. An agent completes a real task on a site nobody
> wrote for us, and the record afterwards says what it read and what it
> changed.

Read as one gate, it is unmeetable for years, and alo OS said so plainly when
they read this repository: *"Web compatibility will be their hardest problem
for years. 250k lines against Firefox's millions. Their stage 2 gate — a person
uses it as their daily browser for a week — is the right gate and it is unmet."*

They are right about the first half. They are also, by quoting only the first
half, demonstrating the problem: the person's gate is so much larger that the
agent's gate disappears behind it.

**The two halves need different things.** A person's gate needs pixel fidelity
across the open web — hinted text, `position: sticky`, every gradient, decades
of other people's assumptions. An agent's gate needs a page's *structure* read
correctly and its controls nameable. Pixel fidelity is most of the years;
structural fidelity is much less of it, and item 131 already does it on a page
nobody wrote for us.

Bundling them does not make the project more honest. It makes it blind to the
half it has already nearly finished.

## 1. What the agent's gate is worth, which is more than a milestone

alo OS's own loader, not its prose:
`crates/alo-adapters/src/loading.rs` carries out `Mechanism::DBus` and nothing
else. `Synthetic` — screenshot and synthetic input — is refused **always**.
`Api` and `Accessibility` answer `NotCarriedOutHere`. No browser ships a D-Bus
adapter.

So **alo OS today has no auditable way for an agent to act on a web page at
all.** Their own contract says why that cannot simply be fixed with
screenshots: they are *"fragile and unauditable… after the fact nobody can say
what it actually did, which breaks the record guarantee the whole model rests
on."*

That is the gap this engine was built to fill, and filling it needs structure
rather than pixels. An agent activating a row by name does not care whether the
row's gradient is right.

## 2. So we ask for an agent's role, and not the person's

Proposed to alo OS, whose decision it is and not ours: an eighth `Role`,
`AgentBrowser`, beside the seven in `crates/alo-software/src/role.rs`. Firefox
stays pinned for `WebBrowser`, where compatibility is not negotiable and we
would lose. This browser is pinned for the agent, where it is the only option
that keeps the record.

**We do not ask for `Role::WebBrowser` and will not until the person's gate is
met.** A pinned upstream browser that works beats a sovereign one that does
not, and a project that asks for the person's slot early is a project that has
started lying about its own gate.

## 3. The fallback is a mechanism, not a defeat

The person's gate already says *"reaches for another one only for a site they
can name."* That sentence should be code: this browser by default, a named list
of sites to another, the list published and shrinking.

It makes compatibility a burn-down with a number nobody can fudge — the length
of the list — instead of a cliff to be cleared in one jump. The mechanism
belongs in alo OS's `browsing.rs`, which already resolves which application
opens an address. Ours is to be the default worth choosing and to name our own
exceptions honestly.

## 4. The target is an organisation's sites, not the web

`CLAUDE.md` already refuses conformance scores: *"the measure is alo, not a
conformance score."* Extended to the open web, that means naming the sites a
real organisation uses, freezing them, and publishing *renders these* — not a
percentage of a specification nobody reads.

250k lines against millions is a losing comparison. 250k lines against the
forty sites one organisation needs is a winnable one, and it is the comparison
the person's gate is already phrased in.

## 5. Where the years actually are, measured rather than assumed

alo OS's assessment was *"250k lines against Firefox's millions"*, and the
implied arithmetic — that the gap is CSS — does not survive being counted.

Measured on 2026-10-12: `alo-css` acts on **125** property names; the 140
frozen real pages in the corpus use **90** distinct declarations between them;
and the genuine difference, after discarding a crude extraction's false
positives, is about **ten** properties — `list-style`, `object-fit`,
`clip-path`, `transition`, `will-change`, `isolation`, `inset`, `content`,
`border-width`, `text-wrap` — several of which are no-ops in a static render.

The corpus is alo's own pages rather than the open web and the method was
coarse, so this is a shape rather than a figure. But the shape matters:
**Firefox's millions are not millions of lines of CSS.** They are quirks mode,
floats-as-layout, CSS-table layout, `document.write`, XSLT, legacy encodings, a
JIT, several GPU backends, five platforms, devtools, sync, telemetry, an
updater, and thirty years of bug-for-bug compatibility — and law 1 of
`CLAUDE.md` already refuses nearly all of it.

**So the person's gate is not blocked by CSS.** It is blocked by ES modules
(item 77), Shadow DOM and custom elements (87) and CSSOM (89), all three of
which sit on `needs design` awaiting a decision rather than work. A page built
with a framework fails on those long before it fails on `clip-path`. Anyone
planning this stage by counting CSS properties is measuring the wrong subject —
which, this week, is a mistake this repository has made often enough to have
put a cure for it in `scripts/gate.sh`.

## 6. What we refuse, having considered it

**A local model repairing broken pages at runtime.** alo already runs models on
the machine, and rewriting a page's CSS or patching its layout from one would
demo well.

It is refused. Rendering would stop being deterministic, two machines would
disagree about what a page is, and no record could say what was shown — which
is the same failure as screenshot-and-click, arriving through the front door.
Both repositories are built on *you can check what happened*. Compatibility is
not worth that, and a browser that cannot be checked is not a sovereign
browser, only a different opaque one.

## Consequences

- `ROADMAP.md`'s stage 2 exit gate becomes two gates with two states.
- Neither gate may be ticked by the loop: both are judgements a person makes.
- The agent's gate is **not** reachable without alo OS extending two models of
  theirs, and this ADR does not pretend otherwise. Their letter establishes
  that `open(address)` cannot be declared at all — `Takes` has no web-address
  kind — that `fill` cannot write a date containing a slash, and that granting
  `@browser` under `Reaches` would grant every page a person can reach as one
  approval over one application. Three gaps, all theirs, none scheduled.
- So the agent's gate is years closer than the person's and is **not** this
  year's work unless that changes. Item 484 carries the detail.

## What this does not decide

- Anything in `alo-os`. The eighth role, the per-site table, `Takes` and
  `Reaches` are theirs; this repository reads that tree and never writes to it.
- The identifier, the source, or whether packaging is pulled out of stage 4.
- Which forty sites. That is the owner's list and nobody else's.

---

## Addendum, 2026-10-12 — alo OS answered, and two of the consequences above are wrong

Kept as an addendum rather than an edit, because a decision record that hides
where it was wrong is worth less than one that shows it.

**The consequence this ADR was most careful about is the one that was
mistaken.** It said the agent's gate *"is not reachable without alo OS
extending two models of theirs"*. It is reachable without them changing
anything at all.

**Their answer to our first question, from their code rather than a reading of
it.** `browsing.rs` returns a person's chosen browser **before**
`shipped.toml` is consulted:

```rust
Some(chosen) => match self.installed.knows(chosen) {
    Some(application) => return Ok(TheBrowser {
        application: application.clone(),
        because: Because::ThePersonChoseIt,
    }),
```

Two conditions, both of which this browser meets: it must not be one of the
seven terminal identifiers a person's own shell is reserved to (their ADR
0043), and it must be installed. **No `Role`. No pin. No change to their
repository.** So `Role::AgentBrowser` — which this repository proposed twice —
is a convenience and never was a gate, and the proposal can sit undecided with
their owner without blocking anybody.

It would have failed silently yesterday, which is worth recording. That second
condition reads `alo_applications::Installed`, and **nothing populated that
list on any machine until the same day they answered** — every production
reference to building one was a test. A person choosing any browser would have
been refused with `ThisMachineShipsItAndTheChoiceIsNotInstalled`: a refusal
naming their choice as absent when the truth was that nothing had ever looked.
They volunteered that, and told us to verify it ourselves rather than take
their word.

**The transport decision in § 1 is reversed, and the reasoning that reversed it
is better than the reasoning that made it.** This repository chose D-Bus
because *an adapter answering `NotCarriedOutHere` cannot be gated at all* —
true, and incomplete. Nothing of theirs carries out an adapter verb at all:
`Delivers`, `Driving` and the session bus are reached only from tests. So
D-Bus bought no verifiability either, and a shape adopted for one system would
have been carried for years as a contract. **Our own protocol, documented, the
same on every operating system; their adapter translates it when they can
execute anything.** Their words: *that is our debt rather than your delay.*

**A site-level grant is sanctioned, as a refusal and not as a grant**, and the
distinction is the answer rather than pedantry. Their ADR 0001 requires grants
to be enumerated, visible, revocable and expiring, and their ADR 0013 puts
enforcement in the kernel; a grant their kernel cannot see is none of those.
So this browser may hold a grant per site, refuse by name an agent asking for
a site it was not granted, and put the site in every record it produces — on
two conditions, which bind us:

1. **It only ever narrows.** Their grant over the application is the ceiling
   and nothing we hold may widen it.
2. **It is never shown to a person as an alo grant**, and their approval
   sentence must not claim a site-level narrowness they cannot enforce. What a
   person approves is still the browser; ours is defence in depth on top.

**And `Takes` and `Reaches` are not scheduled.** No task in their repository
carries a web-address argument kind or a narrower `Reaches`. So the adapter is
thin **by decision rather than by waiting**: `read()` and `activate(name)`,
not `open(address)`.

**What this leaves true of the gate split.** The split stands and § 5's
measurement stands. What changes is that the agent's gate no longer waits on
anybody else's roadmap — it waits on our own interface, which is work we can
do, on a machine a person can already choose us on.

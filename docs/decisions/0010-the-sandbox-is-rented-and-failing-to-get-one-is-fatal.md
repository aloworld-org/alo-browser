# ADR 0010 — The sandbox is rented, and failing to get one is fatal

**Status:** accepted
**Date:** 2026-09-03
**Context:** ADR 0001 (rent the physics, build the engine); ADR 0005, whose
consequences require this decision by name — *"the sandbox will need syscalls,
and law 4 still holds… This ADR does not pre-authorise any of it"*;
`docs/autonomy/QUEUE.md` item 167; `crates/alo-renderer/src/bin/alo-render.rs`,
which is the process this applies to

## The decision in one line

A renderer confines itself with **the operating system's own sandbox, through
rented crates**, before it reads a single byte of any page — and a renderer that
cannot get one **exits** rather than rendering without it.

## Rent it, and why that is not the usual argument

ADR 0001 rents the physics: text shaping, codecs, TLS. A sandbox is physics in
the strictest sense — it is a kernel interface, its correct use is a matter of
fact rather than of taste, and getting it subtly wrong produces something that
*looks* confined and is not.

But the usual reason for renting is effort, and that is not the reason here.
The reason is that **a sandbox we wrote would be a sandbox only we had tested.**
Seatbelt profiles and seccomp filters are load-bearing in browsers that are
attacked continuously; the bugs have been found by people attacking them, not by
people reading them. A hand-rolled filter enters the world with none of that
behind it, and law 3 — correct before fast — has no equivalent for "correct
before adversarially exercised". You cannot test your way to it alone.

## Which mechanism, on which platform

- **macOS: Seatbelt.** The profile is applied to the renderer as it starts. It
  permits the two inherited pipe descriptors and nothing else — no file open, no
  socket, no process spawn, no `fork`.
- **Linux: seccomp-bpf, a user namespace, and Landlock.** Three layers because
  they cover different things: seccomp says which system calls exist at all, the
  namespace removes what the process can even name, and Landlock removes the
  filesystem within what remains.

Both are the mechanisms the browsers that get attacked use. That is the whole
argument for them and it is enough.

**Windows is not on this list, and the browser does not claim it.** See below —
that is a deliberate consequence rather than an omission.

## Whose `unsafe` this is

Law 4 forbids `unsafe` outside a reviewed, named boundary with a written reason.
A rented crate containing `unsafe` is **the crate's `unsafe`, not ours** — the
same position ADR 0005 already takes about TLS and codecs, and the same one
`ring` sits in today.

So: **this decision authorises no `unsafe` in this repository.** If a platform
turns out to need FFI we must write ourselves, that is a different decision and
it comes back here for its own ADR, naming the boundary and the reason. It is
not covered by this one, and this sentence exists so that nobody reads
"the sandbox ADR" as having settled it.

## Failing to get one is fatal, and that is the hard part

A renderer that cannot apply its sandbox **exits**. It does not render.

This is the decision somebody will want to reverse on a bad afternoon, so the
reasoning is written out.

A renderer *is* the thing ADR 0005 exists to contain. Rendering without a
sandbox is not a degraded browser — it is a browser that has removed a
protection the person believes it has, at exactly the moment it discovered it
could not provide it. The failure is silent by nature: nothing about the page
looks different, and nobody finds out until it matters.

The counter-argument is real: a platform quirk, an unusual kernel, a container
without the right permissions, and the browser will not open a page. That is a
bad experience and it is somebody's Tuesday.

We take it, and pay for it in the previous section: **the browser does not claim
a platform it cannot sandbox.** Failing closed is only defensible if it is rare,
and it is only rare if we are honest about where we run. A platform we have not
built a sandbox for is a platform we do not ship — not one we ship with the
protection quietly off.

## What a renderer is given, now that it can ask for nothing

The consequence people underestimate. A confined renderer cannot open a font
file, so somebody has to decide where fonts come from.

**The browser process passes bytes; the renderer opens nothing.** Not "the
sandbox permits the font directory read-only", which is the tempting answer and
which puts a filesystem path into the policy for every kind of resource that
will follow. One rule that holds for fonts, images, and whatever comes next is
better than a policy that grows a hole per resource type.

`alo-render` currently embeds one font, which is what the design forces and not
a gap in it. Fonts arriving as bytes over the boundary is queue item 168.

## What this does *not* protect against

**A compromised renderer can still say anything on its pipe.** The sandbox stops
it reaching the disk and the network; it does nothing about the messages it
sends. That is why queue item 63's decoder treats everything from a renderer as
bytes a stranger chose, and the two decisions are load-bearing together — either
alone is a half-measure.

**Nor does it protect a renderer from the page it is rendering.** It confines
the damage; it does not prevent the compromise. That is what memory safety is
for, and it is why ADR 0005's four reasons survive a memory-safe engine rather
than being replaced by one.

## How we will know it works

**A test that watches a renderer fail to open a file**, not a flag saying a
sandbox was applied. A policy that was installed and permits everything reports
success exactly like one that works.

So the test asks a renderer to do something forbidden and asserts it cannot —
and it runs on every platform we claim, because a sandbox that is only checked
on the machine of whoever wrote it is a sandbox that stops working on a Tuesday
without anybody noticing.

## What was rejected

**A sandbox of our own**, for the reason at the top: it would be a sandbox only
we had tested.

**Applying it after start-up, once things are loaded.** Convenient, and it means
a window exists in which the process is unconfined. Every such window has to be
argued for individually and none of them has been.

**Failing open with a warning.** A warning nobody reads is a protection nobody
has, and it converts a decision into a notification.

**Shipping a platform with the sandbox off "for now".** That is the same
decision as failing open, made once instead of per launch, and it is worse for
being invisible.

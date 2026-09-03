# ADR 0007 — Cookies are partitioned by default

**Status:** accepted
**Date:** 2026-09-03
**Context:** ADR 0005 (one process per site); `docs/autonomy/QUEUE.md` item 57,
which asks for this decision by name — *"what a default costs and who it
protects"*; `crates/alo-net/src/redirect.rs`, which already drops `Cookie` at an
origin boundary

## The decision in one line

A cookie is stored under **two** keys — the site that set it *and* the
top-level site the person was looking at when it was set — so a cookie set by
`ads.example` inside `news.example` is a different cookie from the one it sets
inside `shop.example`, and neither can see the other.

Plus four things that follow from it: `SameSite=Lax` when a site does not say,
`Secure` required for anything that crosses a site boundary, `HttpOnly`
honoured against the scripting engine from the day there is one, and the
`__Host-` and `__Secure-` prefixes enforced rather than merely parsed.

## What this is actually about

An unpartitioned third-party cookie is a **single identifier that follows one
person across every site that embeds the same third party**. That is not a
side effect of the design; for twenty years it *was* the design, and the
tracking industry is built on it.

The mechanism is one sentence: if `ads.example` is embedded in a thousand sites,
and it sets one cookie, then every one of those thousand sites hands
`ads.example` the same identifier along with the address of the page. Nobody
consented to that in any meaningful sense, and no page can tell you it is
happening.

Partitioning does not remove the cookie. It removes the *joining*. `ads.example`
may still remember something about you on `news.example`; it simply cannot tell
that the person on `news.example` is the person on `shop.example`.

## Who it protects, and from what

**Everybody, from cross-site tracking**, which is the whole of the point above.

**Everybody, from a large class of CSRF.** A cross-site request that carries no
ambient credential cannot act as you. `SameSite=Lax` by default is what makes
that true for a form post from another site, and it is the single highest-value
line in this document measured in bugs that stop existing.

**People whose browsing is dangerous to them.** The threat model that decides
this is not an advertiser building a profile; it is somebody whose reading
history is evidence — of an illness, a pregnancy, an immigration status, a
sexuality, an intention to leave. For that person a joined cross-site identity
is not an annoyance, and a default that protects only the people who know to go
and change it protects nobody who most needs it.

That last paragraph is the actual argument. Everything else is engineering.

## What it costs, honestly

This is the half a decision document usually skips.

**Federated sign-in breaks.** "Sign in with X" that relies on a third-party
cookie on X's domain stops working silently — the person clicks, nothing
happens, and the page cannot tell them why. This is the largest single cost and
it lands on people who have done nothing wrong.

**Embedded things that expect to know you break.** A payment widget, a comment
system, a support chat that remembers your ticket, a video player that knows
your subtitle preference. Each is a small breakage and there are a great many of
them.

**"Keep me signed in" across a corporate SSO breaks**, which is the version of
the first cost that arrives at somebody's job.

**And the failure mode is bad.** A blocked cookie is not an error a site can
catch and explain. It is a login that quietly does not stick. The person
concludes the browser is broken, and they are not wrong to.

**One more cost, which is about us rather than about users:** this is not a
settled industry position. Safari and Firefox partition or block by default.
Chrome announced the end of third-party cookies, moved the date four times, and
in 2024 abandoned the plan. A browser with an advertising business could not
make this decision. We have no advertising business and no compatibility debt,
which is exactly why we can — but it means "every other browser does this" is
**not** available as a justification, and pretending otherwise would be dishonest
about where we stand.

## Why we take that cost anyway

Because a default is not a preference. It is what happens to everybody who never
opens a settings screen, which is nearly everybody, and it is the only decision
in this document that affects people who will never hear of us.

The costs above are all of the same shape: **a site that wanted a cross-site
credential does not get one.** They are real, they are visible, and they are
recoverable — a person can be asked. The cost of the other default is invisible,
falls on people who cannot see it happening, and is not recoverable, because a
profile once joined stays joined.

Between a breakage somebody can see and a harm nobody can, we take the breakage.

## The escape hatch, and its shape

A default this strict without a way through is a browser that loses arguments
with reality. So:

- **A per-site grant, made by the person, in response to a specific ask.** The
  embedded site asks for storage access; the person is told *who* is asking and
  *inside what*, in those words, and answers for that pair. Not a global switch.
- **Never a blanket "allow third-party cookies" toggle.** A global setting is a
  thing people are told to turn on by support pages and then never turn off, and
  it converts a considered default into a decision nobody made.
- **Never an allowlist we ship.** A list of sites this browser trusts with
  cross-site identity is a business we would then be in, and the pressure to be
  on that list is a pressure we should not be able to sell.

The grant is queue item 57's successor rather than part of it, and it is
explicitly *not* implemented by the parser: the parser's job is that a cookie
carries its partition, and no code path can lose it.

## The four rules that follow

- **`SameSite=Lax` when a site does not say.** The specification's default is
  `None`, historically. A cookie with no `SameSite` is a cookie whose author did
  not think about cross-site use, and the safe reading of "did not think about
  it" is not "send it everywhere".
- **`SameSite=None` requires `Secure`.** A cross-site cookie sent in the clear
  is a cross-site cookie any network can read and replay.
- **`HttpOnly` binds the scripting engine**, from the day there is one. There is
  no JavaScript in stage 1, which makes this the cheapest possible moment to put
  the rule in place and the worst possible moment to skip it — a flag honoured
  from the first commit costs nothing, and one retrofitted is a security review.
- **`__Host-` and `__Secure-` are enforced, not parsed.** A cookie named
  `__Host-session` that does not meet the prefix's conditions is **rejected**,
  because the entire value of a prefix is that a server can trust the name.

## What this does not decide

How long a cookie may live, how many a site may set, and how large they may be.
Those are bounds and belong with the code that holds them. The one thing said
here about them is that they must exist, for the reason every other bound in
`alo-net` exists: a limit somebody else chooses is not a limit.

## How we will know if this was wrong

If the escape hatch is being used constantly, the default is not protecting
people — it is annoying them into turning it off, which is worse than not having
it, because it trains people to click yes. That is the signal to come back to
this file, and it is a measurement rather than an argument.

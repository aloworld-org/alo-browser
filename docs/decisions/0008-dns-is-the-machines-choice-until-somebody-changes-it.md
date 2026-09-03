# ADR 0008 — DNS is the machine's choice until somebody changes it

**Status:** accepted
**Date:** 2026-09-03
**Context:** ADR 0007 (cookies are partitioned by default), whose reasoning
about defaults this follows; `ROADMAP.md` stage 2 — *"DNS, and encrypted DNS as
a choice somebody made rather than a default nobody was told about"*;
`docs/autonomy/QUEUE.md` item 58

## The decision in one line

This browser uses **the resolver the machine is configured to use**, and never
silently replaces it — encrypted DNS is available, named, and chosen, and the
name of the company that would see every site you visit is in the sentence where
you choose it.

## Why this is a decision and not a detail

DNS is the one place where **every site you visit is visible in a single
stream**, in order, with timestamps. Not the pages, but the names — which is
usually enough. It is the most complete record of a person's browsing that
exists anywhere, and it is produced whether or not anybody asked for it.

So "which resolver" is a question about **who gets to hold that record**, and
answering it silently is the thing this ADR exists to prevent.

## Why not default to encrypted DNS

The obvious reading is that plain DNS is bad and encrypted DNS is the fix, so we
should turn it on for everybody. That reading is wrong in a specific way worth
writing down.

Encrypted DNS does not make the record go away. **It moves it.** Plain DNS
scatters your browsing across whoever runs the network you are on — your ISP,
the café, the hotel, the airport. Encrypted DNS concentrates it at **one
resolver, globally, tied to your IP address, across every network you ever
join.** That is a smaller number of watchers and a much better record.

Which of those is safer depends entirely on who the person is and where they
are, and a browser does not know either. Firefox learned this in public in 2019:
turning on DoH by default routed a large fraction of a country's DNS to a single
company, and the objection was not that the company was untrustworthy — it was
that nobody had been asked.

**And a default resolver is a business.** ADR 0007 refused to ship an allowlist
of sites trusted with cross-site identity, because being able to sell a place on
that list is a pressure we should not have. A default DNS provider is the same
object: a slot with enormous value, which somebody would eventually offer to pay
for. The way not to be corrupted by that is not to have the slot.

## Why not override the machine, either

The system resolver is not merely a default we inherit. It is where several
things the person actually chose already live:

- a **VPN**, which stops working as intended the moment a browser resolves names
  outside it;
- a **corporate network**, whose internal names do not exist anywhere else;
- a **Pi-hole or similar**, which is somebody deliberately filtering their own
  network and which we would silently defeat;
- **`/etc/hosts`**, which is how every developer alive points a name at their own
  machine;
- and the operating system's *own* encrypted DNS, when it has one — in which
  case the person has already made this decision and we should not make it again.

A browser that resolves names its own way breaks all five, and does it
invisibly. "We know better than your machine" is a large claim, and none of the
reasons for it survive the previous section.

## So: what actually happens

- **By default, the system resolver.** Whatever the machine is configured to do,
  including the machine's own encrypted DNS.
- **Encrypted DNS is offered, not imposed.** It is a setting, and the setting
  names the resolver: not "use secure DNS" but "send every site you visit to
  *this company* instead of to your network". If we cannot write that sentence
  honestly about a provider, it does not go in the list.
- **No provider is preselected**, and the order of the list is not for sale.
  There is no arrangement — commercial or otherwise — under which a resolver
  appears here.
- **Falling back is a decision, not a silence.** If a chosen encrypted resolver
  cannot be reached, we do not quietly return to plain DNS: that is the moment
  somebody's threat model is being violated and it is exactly when they would
  want to know. It fails, and it says why.

## Two rules that hold regardless of resolver

**DNS is not trusted for anything security decides.** A resolver — any resolver
— can lie about an address. What stops that mattering is TLS: the certificate
proves the server is who the name says, and a wrong address produces a
certificate error rather than a wrong page. This is the reason plain DNS is a
*privacy* problem rather than an *authentication* one, and it is why "we use
encrypted DNS" must never be presented as a security feature. It bounds the
harm; it does not remove it.

**A public name that resolves to a private address is refused.** DNS rebinding
is the attack where a name the page controls answers first with a public address
and then with `127.0.0.1` or `192.168.1.1`, turning the browser into a way to
reach things behind the person's own firewall. The rule is simple and it is
absolute: an address in a private, loopback, link-local or unspecified range is
not a valid answer for a name that arrived from the public web.

## What this costs

**Plain DNS on a hostile network stays plain**, for everybody who never opens the
setting — which is nearly everybody. That is a real cost and it falls on exactly
the people ADR 0007 was written about: somebody on a network they do not control,
whose browsing is evidence.

We take it because the alternative is choosing their watcher for them without
telling them, and because the honest version of helping that person is to make
the choice **easy and legible** rather than to make it silently. If the setting
turns out to be one nobody finds, that is a failure of the interface and the fix
is in the interface — not a reason to start deciding on people's behalf.

## How we will know if this was wrong

If almost nobody ever changes it, the setting is not a choice, it is a
decoration. That is the signal to come back to this file — and the answer then
is a **prompt** that asks once, plainly, naming the trade, not a default that
picks a company and says nothing.

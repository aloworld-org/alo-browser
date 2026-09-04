# ADR 0012 — Every request says what caused it

**Status:** accepted
**Date:** 2026-09-04
**Context:** ADR 0002 (the layout tree is the agent's tree), whose verbs are the
actions this has to be able to name, and whose *"reading is never watching"*
clause this one is the other half of; ADR 0003 (node identity is allocated once
and never reused), whose rule an action's identity takes unchanged; ADR 0005
(one process per site), which makes a renderer the process that parsed a
stranger's page and therefore not a witness; ADR 0007 and ADR 0011, which
already priced what a durable record of somebody's browsing costs;
`alo-os` ADR 0001 (the capability model — enumerated, visible, revocable,
expiring, **recorded**), as ADR 0002 records it here; `docs/autonomy/QUEUE.md`
item 67, which asks for this decision by name — *what is recorded, for how
long, and who may read it*; `crates/alo-net/src/request.rs`, which carries a
purpose and an initiator today and says in its own comment that item 67 is
coming

## The decision in one line

Every request carries a **cause** — a person, a document, or a named agent
action — which the **browser process** assigns and which cannot be omitted; the
causes are held for the session, the ones reaching an agent action are kept
until the person deletes them, and **no page and no agent may read the record**.

## Why this is a decision rather than a chore

`ROADMAP.md` says why the question exists: *"no other engine has needed to
answer that, and an agent-driven browser that cannot is one nobody should
trust."* But answering it badly is not a smaller version of answering it — it
is two different failures, and they pull in opposite directions.

**Attribution is a claim, so who gets to make it decides what it is worth.** A
record saying *the person opened this* is evidence. If the code that parsed a
hostile page can write that line, it is not evidence, it is a sentence somebody
else composed. A forgeable record is worse than no record, because people
believe it.

**And a record of every request is a record of everywhere somebody went.** ADR
0011 spent five sections being careful about a *cache* for that reason. A file
listing every URL, in order, with times beside them, is the thing itself rather
than a by-product of it — and it would be built in the name of protecting the
person it is about. So the interesting half of this decision is not what to
record. It is what **not** to keep, and who may never read what is kept.

## 1. A cause is carried, never inferred afterwards

The cause is a field on the request, with **no default**. A request that cannot
say what caused it does not compile.

This is the shape ADR 0002 already uses for verbs — *there is no function that
accepts a point, and there is no way to write one without changing this file*.
The guarantee is structural rather than a discipline, and the reason is the
same in both places: the call site added in a hurry is exactly the one that
would have omitted it, and it is exactly the one somebody will later need to
account for.

A log written *beside* the network stack was the obvious alternative and it is
refused below, for one reason: it can only guess, and a guess in a record is
indistinguishable from a fact.

## 2. Three causes, and there is no fourth

- **A person.** A navigation somebody made — typed, a bookmark, a link they
  clicked, back. Carries the tab.
- **A document.** A page loading what it needs, or a script it ran. Carries the
  document's identity, and through that its origin and its tab.
- **An agent action.** A verb this engine performed, by the identity minted when
  the verb was accepted, and the document it acted in.

There is deliberately no `Unknown`, no `Internal` and no `Other`. The requests
this engine makes on its own behalf are attributed to whatever caused the thing
they are about: a violation report to the load that violated the policy
(`Purpose::Report` already exists and says which), a revalidation to whatever
wanted the response, a redirect to whatever asked for the first hop. An
engine-made request with no antecedent would be the one line in the record
nobody can account for, and a category like that does not stay empty — it
becomes where the awkward cases go.

## 3. It is a chain, and each document records what caused its own load

*Which page, and which agent action* is two questions, and the second one is
usually answered indirectly. An agent activates a link; a document loads; that
document fetches a script. The script's cause is the document. The document's
cause is the agent action. The action's cause is the person who asked for it.

So a cause is **a link in a chain rather than a label**, and the chain is walked
rather than assembled: a document already has an identity, and it records what
caused its load, so nothing has to keep a side table of which activity
"belongs" to which action. The walk terminates and cannot lie about which
document it reached, because ADR 0003's identities are allocated once and never
reused — an id that came back round would join two unrelated pieces of
somebody's history into one story.

**Both answers are kept, in order.** A record that had to choose between *the
page asked for this* and *the agent asked for this* would be choosing between
two true things.

## 4. The browser process assigns it; a renderer never does

A renderer may say what it **wants** — this is a script, this is a picture,
which is `Purpose` and which it is the only thing that knows. It may never say
**who wanted it**.

The browser process knows what a renderer cannot be trusted to state: which tab
asked, which document that tab holds, and whether it had just sent that renderer
an `Act`. ADR 0005 exists because a renderer parses bytes a stranger sent; a
compromised one that could state its own cause would launder its fetches into
*the person did that*, which is precisely the sentence somebody would later rely
on.

**While a verb is being applied, that tab's requests are the agent's.** The
boundary is the task, which the event loop defines (queue item 76) — and the
edge case is named here rather than discovered later: a page that fetches on a
timer minutes after an agent clicked something is **not** the agent's. Widening
the window until every consequence is captured makes the record true and
useless; it would attribute a page's whole life to whoever last touched it.
Until scripts exist a verb's consequences are immediate, so the rule can be
written now and its precise edge lands with the event loop.

## 5. What is recorded

Per request: **when**; the **cause chain** — tab, document, and the action where
there is one; the **method and the URL**; the **purpose**; and **what
happened** — the status, whether it was served from the cache, or which rule
refused it, by name, the way `alo-net` already refuses things.

Never:

- **Bodies**, in either direction. What was asked of whom is the question; what
  was in it is somebody's private data, their password, or a page.
- **Header sets wholesale.** They carry `Cookie` and `Authorization`, and a
  record holding those is a file that logs somebody into their own bank.
- **Anything a page chose to put there.** A page that could write into the
  record could write a plausible line into it.

The full URL is kept, because *what it read* is the whole question and an
origin-only record cannot answer it. That is also why section 6 is careful:
some URLs are themselves a credential.

## 6. How long, and where

**Everything, for the session, in memory, bounded.** This is the view a person
opens to see what a page is doing and what item 129's developer tools read. It
dies with the process, which is the correct lifetime for a record of a session:
the pages are gone, and so is the list of them.

**What an agent did is kept until the person deletes it.** A record that
vanishes when the browser closes cannot answer *what did it do while I was not
watching*, and that question is the entire reason `alo-os` ADR 0001 records
anything at all. So this is the one place in the browser that deliberately keeps
a durable browsing record, and what makes it affordable is that it is **small on
purpose**: only requests whose cause chain reaches an agent action, plus the
action and its outcome. A person's own browsing is not in it.

It is written under ADR 0011 section 3's rules unchanged — one directory per
profile, in the place the operating system keeps such things, private to its
owner, **no encryption of ours** because a key that lives next to the data is
not a key — and with ADR 0011's honest boundary restated rather than quietly
dropped: protected against another user account, **not** against a program
running as the person.

Two clauses of its own:

- **A session-scoped profile never opens it.** Private browsing is a decision
  the person made once, and an agent acting in a private window leaves a record
  that lives as long as the window. Not a file that is emptied afterwards — a
  file that was never created, which is ADR 0011 section 2's rule and its
  reason.
- **The bound is counted in actions, not bytes.** When it is reached the oldest
  actions go whole. An agent's most recent work is the thing somebody is most
  likely to be asking about, and a bound in bytes would let one action with
  three hundred requests in it evict a week of ordinary ones.

## 7. Who may read it

- **The person.** It is theirs, it is deletable, and deleting it is real — the
  file, not a flag on it. Where it is read from is queue item 127's, and *not
  buried* is that item's stated requirement.
- **No page, ever.** There is no API for it and there is not going to be one. A
  record readable by script is a cross-site history oracle handed out by the
  browser, which is the exact thing ADR 0007's partitioning and ADR 0011's
  per-site cache key exist to prevent. Building one here would undo both.
- **Not the agent.** The record is *about* the agent and kept *for* the person.
  An agent that could read it could read everywhere that person has been, and
  could check whether its own actions had been noticed. It is told the outcome
  of its own action, which it already is — `alo_agent::Outcome` — and nothing
  more.
- **Nowhere else.** `ROADMAP.md` refuses telemetry outright, and this is the
  single most attractive file in the browser to send somewhere. Nothing in this
  decision authorises a byte of it leaving the machine, and sync (queue item
  145) does not inherit it.

## 8. What this costs

- **Memory, for the session**, bounded, on every request. A page that fetches a
  thousand things has a thousand entries, and the bound is ours to choose for
  the reason every bound in `alo-net` is: a limit somebody else chooses is not a
  limit.
- **A durable file naming sites an agent visited**, with all of ADR 0011's
  boundary and none of its own encryption. For the person whose reading is
  evidence, an agent acting on their behalf now leaves a trace that survives a
  restart. What we owe them is that it is small, that they can delete it, and
  that a private window never writes one.
- **Friction, on purpose.** Every place that makes a request must name a cause,
  and no default rescues anybody from thinking about it. That is the cost of
  section 1 and it is being paid deliberately.

## Alternatives rejected

**A log written beside the network stack.** Rejected: attribution nobody is
*forced* to supply goes missing exactly where it matters, and a log downstream
of the decision can only guess at what caused what.

**Infer it — whatever the agent last did, within a few seconds.** Rejected
hardest. A guess recorded in the same shape as a fact is unfalsifiable
afterwards, and the record exists precisely for the case where somebody needs to
know rather than to estimate.

**Let the renderer state the cause.** Rejected: it is the process that parsed a
stranger's page, and a cause it could state is a cause it could forge.

**Keep everything durably.** Rejected: that is a surveillance file with an
accountability label on it, and ADR 0011 already refused the smaller version.

**Keep nothing durably.** Rejected: it makes the ★ promise unkeepable. An agent
that acts while nobody is watching and leaves no trace is the thing nobody
should trust, and it is what every other AI browser currently is.

**One label per request instead of a chain.** Rejected: it forces a choice
between two true answers, and whichever is chosen the other question — the one
`ROADMAP.md` asks — becomes unanswerable.

## What this does not decide

- **Who was allowed to.** This records what happened; **grants** are queue items
  93 and 133 and have their own ADRs owing. A record is not a permission, and an
  action being recorded must never become the argument that it was authorised.
- **The quota** across storage — queue item 90, its own ADR, as ADR 0011 also
  said of itself.
- **The interface** any of this is read in — queue item 127, and the same block
  items 157 and 158 sit behind.
- **Where the task boundary is** — queue item 76, as section 4 says.
- **An agent across frames** — queue item 132, which is a security boundary
  rather than a record.

## How we will know if this was wrong

Two measurements rather than an argument, which is the standard ADR 0007, 0008
and 0011 set.

**If a person opens the durable record and it does not answer the question they
actually asked** — *did it do what I asked, and nothing else?* — then it holds
the wrong fields, and the answer is to record **different** things rather than
more of them.

**And if the chain is `document` all the way down with no action ever reachable
from a real page**, attribution is not surviving the verb boundary and the whole
thing is decoration. That is checkable the day queue item 131 drives a frozen
real page: read the record afterwards, and every request that action caused
should lead back to it.

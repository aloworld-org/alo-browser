# ADR 0011 — What the cache may write to a disk

**Status:** accepted
**Date:** 2026-09-03
**Context:** ADR 0005 (one process per site), which gives a renderer no
filesystem; ADR 0007 (cookies are partitioned by default), whose argument about
*joining* this one follows; ADR 0010 (the sandbox is rented), whose consequence
that the browser process passes bytes rather than the profile permitting a
directory applies here unchanged; `docs/autonomy/QUEUE.md` item 155, which asks
for this decision by name — *"what may be written to a disk other programs can
read is a different question from what may be reused, and it has a different
answer for a page behind a password"*; `crates/alo-net/src/cache.rs`, which is
memory only and says so

## The decision in one line

The cache is written to the person's own disk, **partitioned by top-level site
exactly as cookies are**, with everything that must not outlive the session
**never written at all** rather than written and deleted — and a renderer never
touches it.

## Why this is a decision rather than a chore

A cache in memory is bytes we already had, held a while longer. Nothing about it
is new: the responses came from the network this session, and when the process
exits they are gone.

A cache on a disk is two other things.

It is **a durable record of everywhere somebody has been** — not the pages only,
but the order and the times, on a medium that outlives the session, survives a
restart, and can be read by whoever later has the machine. Queue item 56 built
something that decides *what may be reused*. This decides *what may be kept*,
and those have the same answer only for a page nobody minds being seen reading.

And it is **an input**. Everything else `alo-net` reads arrives from a socket
and is treated as hostile. A cache file arrives from a filesystem, is trusted by
habit, and is then handed to a page **under that page's own origin**. A cache we
believe uncritically is a cross-site scripting vector we deliver ourselves, with
the site's name on it.

## 1. The cache is partitioned, for the reason cookies are

One cache shared across every site is the same object ADR 0007 refused: a thing
that joins one person's activity on one site to their activity on another.

Three ways, each real:

- **Cache probing reads your history.** A page can time a load and learn whether
  the resource was already held. Held means you have been somewhere that loads
  it. A shared cache answers that question for any site that thinks to ask, about
  any resource it can name.
- **An entry is an identifier.** A resource at a URL only one visitor was ever
  given is a mark that survives clearing cookies and follows that person to every
  site that fetches it. It is a cookie, kept somewhere nobody thinks to clear,
  set by a mechanism nobody thinks of as storage.
- **And it survives the escape hatch.** ADR 0007's grant is per-site and
  deliberate. A shared cache would hand back the joining that grant exists to
  ration, without anybody being asked.

So the key includes the **top-level site**, and it is the same `Partition` the
cookie jar uses. One answer to "what is a site", in one place: when queue item
156 corrects that answer from the host to the registrable domain, it corrects it
for both, and there is never a version where cookies and the cache disagree
about where a boundary is.

**What it costs**, honestly: a font, a script or a stylesheet that a thousand
sites load from one address is fetched once per site rather than once. That is
bandwidth somebody pays for and a first load somebody waits through, and it is
worst on exactly the shared libraries that are most widely used. The browsers
that partitioned reported the cost as small; **we have not measured it and are
not going to quote theirs as though we had.** Any number this project gives for
it will come from queue item 117, on hardware, or not be given.

## 2. What is never written

Not written, rather than written and then removed. A file that was deleted is a
file that was on the disk — recoverable, and present for the whole window
between the two operations, which is precisely the window a crash or a power cut
lands in. The only way to promise something did not outlive the session is not
to write it.

- **`Cache-Control: no-store`.** Already never stored anywhere, from item 56.
- **`Cache-Control: private`.** It means *for one person*, and a disk other
  programs can read is not one person. Such a response is reusable from memory
  for as long as the process lives, and never persisted.
- **A response to a request that carried `Authorization`.** The header is the
  page being behind a password, said in the request itself.
- **A response carrying `Set-Cookie`.** It is a session token, and a session
  token in a file is a login somebody can pick up later. It stays reusable in
  memory; the disk never sees it.
- **Anything from a session that is not meant to persist** — private browsing,
  and any profile that is session-scoped (queue item 125). Not a cache that is
  emptied at the end: a cache that was never opened.
- **Anything that is not `http:` or `https:`.** `file:` is already on the disk
  and copying it achieves nothing but a second copy; `data:` is part of the page,
  and a page may put a secret in one.
- **A response that did not arrive whole.** A body that stopped early is a fact
  worth keeping for the length of a download (queue items 154 and 185) and is not
  a page. The cache is not where a truncation becomes durable.
- **`Vary: *`.** Already not stored at all, from item 56, and for the same
  reason: there is no key that would be right.

The shape of the list is the point. Every entry on it is something whose leak is
a person's account, a person's session, or a person's session being remembered
past the moment they ended it — and every one of them is still cached in
**memory**, where it costs nothing to be careful.

## 3. Where it is written, and who else can read it

- One directory per profile, in the place the operating system keeps caches, so
  that it is somewhere a person can find and delete without breaking the browser.
- The directory is created private to its owner, and so is every file in it.
  Nothing in the cache is readable by another user account by default.
- **No encryption of our own.** ADR 0001 rents the physics; disk encryption
  belongs to the operating system, and a browser that rolled its own would be
  claiming a protection it cannot make good on — the key would have to live next
  to the data, which is not a key.
- **A file name is a hash of the cache key because a URL is not a file name**,
  and for no other reason. It is not claimed as privacy: anybody who can read the
  directory can ask whether a URL they already suspect is in it, and a hash does
  not stop them.

Which names the boundary plainly: **against another user on the machine, the
cache is protected; against a program running as the person themselves, it is
not.** Neither is anything else that person owns, and pretending otherwise would
be selling a protection we do not have.

## 4. What comes back off the disk is untrusted

`LOOP.md`'s stage 2 rule — anything that reads bytes from outside gets a
malformed, truncated and adversarial input test, and returns an error rather
than panicking — applies to a cache file as fully as to a socket. A cache file
may have been written by another program, or by us and then half-overwritten by
a power cut.

- **Every entry carries a checksum over its metadata and its body**, and an entry
  that does not match is discarded rather than served. This is the clause that
  keeps section 3's honest boundary from becoming a code execution bug: a program
  running as the person can already read the cache, and this is what stops it
  *writing* a page into somebody's bank origin.
- **The index is parsed under the same rules as a response**: every length
  checked before anything is reserved, and no arithmetic that a hostile number
  can overflow.
- **An unreadable entry is a miss.** Never an error that reaches the page, never
  a failure to load. A cache is an optimisation, and an optimisation that can
  stop a page opening is a defect however correct its reasoning was.
- **The format carries a version**, and a version we do not recognise is
  discarded wholesale rather than interpreted hopefully.

## 5. It lives in the browser process, and only there

ADR 0005 gives a renderer no filesystem. ADR 0010 named the temptation that
follows — permitting a directory in the sandbox profile instead of passing bytes
— and queue item 168 refused it for fonts. The cache is the same shape and the
stakes are higher.

A sandbox profile granting a renderer the cache directory would hand any
compromised renderer **every page that person has read, across every site**,
which is the precise opposite of what one process per site exists to prevent.
So: the browser process reads and writes the cache, and a renderer receives
bytes it did not go looking for.

## 6. A bound, and whose disk it is

The cache is bounded in bytes as well as in entries, and when the bound is
reached the oldest go, by the insertion order `cache.rs` already keeps — the
clock is not involved in a decision that has nothing to do with time.

The bound is modest and it is ours to choose, for the reason every bound in
`alo-net` exists: a limit somebody else chooses is not a limit. A browser that
quietly fills somebody's disk is a browser they uninstall, and they are right
to.

**This is not the quota decision.** One policy across `localStorage`, IndexedDB
and the Cache API is queue item 90 and needs its own ADR. A bound on our own
cache must not become that policy by precedent — the questions differ, because
this is disk we take without asking and that is disk a page asks for.

## What this costs

**The disk cache is weakest exactly where it would help most.** A site somebody
is signed into and uses daily is the one whose responses most often carry
`Set-Cookie` or `private`, and those are the ones that stay in memory and are
re-fetched after every restart. That is a real, everyday cost, and it is the
direct price of section 2.

**Partitioning costs re-fetches**, as section 1 says.

**And a cache that survives a restart is a browsing record that survives a
restart** — a smaller one than the never-written list would otherwise allow, but
a record. The person for whom that matters most is the person ADR 0007 was
written about: somebody whose reading is evidence. What we owe them is that
deleting it is real and easy to reach, and that the session-scoped answer is
*never written* rather than *deleted afterwards*.

We take these costs because the alternative is a file, on a disk that outlives
the session, holding a session token or a page behind a password, on behalf of
somebody who was never asked and would not have said yes.

## What this does not decide

- **The quota across storage** — queue item 90, its own ADR, as above.
- **Whether a cache follows a person between machines.** Sync is queue item 145
  and gated behind stage 2; nothing here authorises sending a cached byte
  anywhere.
- **What private browsing is**, beyond the one clause that its cache is never
  opened. Queue item 125.
- **The revalidation and freshness rules**, which are item 56's and already
  built. This changes what is kept, never what may be reused: a response that
  went to disk is served under exactly the rules it would have been served under
  from memory.

## How we will know if this was wrong

If the disk survives a restart and almost never hits — because most of what a
real page loads is partitioned away or on the never-written list — then we are
keeping a durable record of somebody's browsing in exchange for nearly nothing,
and the honest answer is to **keep less**, not to relax the rules.

`Cache::counts` already reports hits, revalidations and misses; the measurement
is that, taken across a restart, on the corpus and on real use. It is a
measurement rather than an argument, which is the standard ADR 0007 and ADR 0008
set for coming back to a decision.

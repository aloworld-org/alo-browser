# ADR 0025 — Storage is the browser process's, and one quota covers it

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 90, *"`localStorage`, `sessionStorage`, IndexedDB, the
Cache API, and **one quota policy over all of them**"*, marked *needs ADR*
because *"a quota is a policy about somebody's disk"*. It also uses:

- ADR 0005: the browser process holds *"the disk, the profile"*, a renderer has
  no filesystem, and a renderer *"never makes a synchronous call back into the
  browser process and never waits on one"*.
- ADR 0007: cookies are keyed by the setting site *and* the top-level site, so
  that nothing joins one site's view of a person to another's.
- ADR 0011: what the cache may write to a disk. Its § 6 says in so many words
  that its own bound *"must not become"* this policy *"by precedent"*, because
  *"this is disk we take without asking and that is disk a page asks for"*.
- ADR 0012: every request says what caused it.
- ADR 0013 § 7: values cross between heaps by copying, as the structured clone.
- ADR 0016 § 2: a response is its own task and never a wait inside one.
- The code this has to fit: `alo-net`'s `Partition` (`cookie.rs`), which is
  the cookie jar's and the cache's top-level key; `alo-url`'s `Origin::of`,
  which makes every `file:`, `data:` and `about:` document an opaque origin of
  its own; and `alo-renderer`'s `Page`, which is what a `Load` carries.
- What alo's own pages do with storage, read in `alo-workplace` on 2026-10-06:
  - `web/src/auth/session.ts` keeps the sign-in refresh token in
    `sessionStorage`, *"so a page reload keeps the session without persisting
    to disk across tab-close"*, and in `localStorage` only when the person
    chose *Remember me*.
  - `web/src/i18n/locale.ts`, `ds/usePanelWidth.ts`, `mail/MailModule.tsx`,
    `drive/DocEditor.tsx` and others keep preferences in `localStorage`. The
    largest is a few hundred bytes.
  - `web/public/sw.js` uses the Cache API to precache exactly one page,
    `offline.html`.
  - `billing/quote-studio/quoteStudioPersistence.ts` opens one IndexedDB
    database, only to read an old copy once and move it to the server.

## The decision in one line

All four kinds of storage belong to one **bucket per storage key**, which is
the page's origin *and* the top-level site it is under. The **browser process**
holds every bucket, on disk or in memory. A **fixed quota** covers each bucket
whole, and it is the same number on every machine. A **whole bucket is
evicted**, least recently used first, when the profile's own bound needs the
room. A renderer gets the bytes it is entitled to and never the directory.

## Why this is a decision rather than a chore

The four APIs look like four features. Underneath they are one thing: **a page
writing to a person's disk, and reading it back later**. That one thing raises
four questions, and a browser that answers them separately per API gets each of
them wrong somewhere.

- **Whose disk, how much of it, and who decides.** Some browsers have given a
  site a share of the disk's size. Others have asked the person at a fixed
  point. Neither answer survives contact with this repository's rule that *a
  limit somebody else chooses is not a limit*.
- **Who can see it.** Stored data is the strongest identifier a page can make.
  It is a cookie with no expiry, no size limit worth the name, and no header
  anybody inspects. ADR 0007's partition is worth nothing if storage ignores it.
- **What a hostile renderer can do with it.** The disk is the browser
  process's, and a renderer may be under a page's control.
- **What the numbers tell a page.** `navigator.storage.estimate()` reports a
  quota. A quota computed from the disk tells every page the size of your disk.
  A quota that differs in private browsing tells every page you are browsing
  privately. Both have been used, in shipped browsers, exactly that way.

## 1. The storage key is the origin and the top-level site

A bucket is keyed by two things:

- **the origin of the document** using it, which the specifications already
  require;
- **the top-level site** it sits under, as ADR 0007's `Partition`. This is the
  same type the cookie jar and the cache use.

So `widget.example` embedded in `news.example` has a different bucket from
`widget.example` embedded in `shop.example`, and from `widget.example` visited
on its own. Neither can read the others.

There is one answer to *what is a site*, in one place. When that answer
changes, it changes for cookies, the cache and storage together. No version
ever has two of them disagreeing about where a boundary is.

**An opaque origin has no storage at all.** `localStorage`, `sessionStorage`
and `indexedDB` throw `SecurityError` there, as the specifications say, and
`caches` is absent. Under `Origin::of` that includes every `file:`, `data:` and
`about:` document. This is ADR 0005's own reasoning: two documents that are
each their own origin must not share anything. Note one thing, so that a later
reader does not reach for it: `Partition::of` gives every hostless top-level
URL the one partition `"opaque"`. That serves cookies and the cache, which
`file:` never reaches. **Storage must never be keyed by it** without the
origin beside it, and an opaque origin never reaches the key at all.

**The storage-access grant does not unpartition storage.** ADR 0007's escape
hatch, queue item 157, is for cookies. Whether a grant should also reach a
widget's own unpartitioned bucket is a question for the first page that needs
it. Until then the answer is no.

## 2. One bucket, and four ways into it

Each API is one part of the bucket:

- **`localStorage`**: one area of string pairs per bucket.
- **IndexedDB**: databases per bucket.
- **The Cache API**: named caches per bucket.
- **`sessionStorage`**: one area of string pairs per **tab** per storage key.
  It is the only one that is not in the durable bucket, as § 5 says.

The quota is the bucket's, not each API's. A site that keeps 200 MiB in
IndexedDB has that much less for the Cache API. That is what *one quota policy
over all of them* means, and it is the Storage Standard's own model. A quota
per API would let a page fill each in turn. It would also make
`estimate()` a sum nobody could explain.

## 3. The numbers, and why they are fixed

- **Every bucket's quota is 1 GiB.** It is the same on every machine,
  whatever the disk, whatever the free space, and in private browsing too.
  `navigator.storage.estimate()` reports that number as `quota`. It reports the
  bucket's own counted bytes as `usage`.
- **`localStorage` is capped at 5 MiB** within that quota, counted as two bytes
  for each UTF-16 code unit of every key and value. That is the specification's
  suggestion, and pages are written against it. A write past it throws
  `QuotaExceededError` synchronously, as the specification says. alo's largest
  use is a few hundred bytes.
- **Each `sessionStorage` area is capped at 5 MiB** in the same units.
- **The profile has a bound of its own.** That bound is the smaller of **8 GiB**
  and **a fifth of the volume's free space when the browser starts**. It is
  never reported to a page.

**Why fixed rather than a share of the disk.** A share of the disk leaks the
disk. A page that can read `quota` learns a number that very few machines share
and that rarely changes, which is a fingerprint handed out for free. It also
promises a site room it can only have by squeezing out everything else the
person keeps. A fixed number leaks nothing and is the same promise to
everybody. A page that is told 1 GiB and later cannot write because the
profile's bound is reached receives `QuotaExceededError`. The specification
allows exactly that, since `estimate()` is an estimate. That is honest about
what it is.

**Why 1 GiB.** It is enough for every alo page above by a factor of thousands.
It also covers an offline mailbox of ordinary size, the heaviest thing alo will
plausibly keep. It is small enough that one bad site cannot fill an ordinary
laptop. It is **ours to choose and ours to change**. A change with a
measurement behind it is a policy change in the code, not a new decision: §§
1, 2, 4 and 5 do not move with it. A change without one is a guess. Law 3 says
a guess is the wrong thing to settle first.

**Private browsing reports the same quota.** Its data lives in memory and is
held to a far smaller real bound. But a page that compares `estimate()` across
two visits must not be able to tell which one was private. A write past the
real bound is `QuotaExceededError`, as on a full disk. That leak has a long
history, and this is the end of it here.

## 4. Eviction is whole buckets, oldest used first, and never what is open

When a write would take the profile past its bound:

1. **Buckets are evicted whole.** Eviction never takes a single database, a
   single cache or a single key. An application whose storage is half deleted
   is in a state its author never tested, and it will misbehave in ways that
   look like its own bug. A bucket gone entirely looks like a first visit,
   which every application already handles.
2. **The least recently used goes first.** That is measured by a use counter
   the store keeps, not by a clock, as ADR 0011 § 6 measures its order. Use
   means a read or a write by a page.
3. **Never the bucket that is writing, never a bucket an open document is using,
   and never one a person has marked to keep.** A person keeps a site's data
   through queue item 93's grant, and § 7 covers it.
4. **If that does not free the room, the write fails** with
   `QuotaExceededError`, and nothing else is evicted to make the attempt.

**A page is not told its bucket was evicted.** No API exists to tell it, and
none should: a notice would be a signal about what else the person keeps.

## 5. Where each part lives, and the boundary it crosses

**The browser process owns every bucket.** It reads them, writes them, counts
them and evicts them. **A renderer never holds the directory.** A sandbox
profile granting it the storage directory would hand a compromised renderer
every site's storage. ADR 0011 § 5 refused the same grant for the cache, and
the stakes here are higher, because this is data a page chose to keep.

**The browser process never takes a renderer's word for whose bucket it is.**
A storage request from a renderer names a storage key. The browser process
serves it only if it has itself loaded a document with that origin under that
top-level site into that renderer, and the document is still there. A renderer
asking for any other bucket is refused and reported as one that broke the
boundary. A compromised renderer can therefore reach the buckets of the site it
already is. It can reach no others, which is ADR 0005's boundary unchanged.

**`localStorage` is synchronous, and ADR 0005 forbids a renderer to wait.** So
the renderer holds the area itself:

- When the browser process loads a document into a renderer, it sends the
  area's contents with the load if that renderer does not hold it already. The
  area is bounded at 5 MiB, so the message is bounded too. An origin that has
  stored nothing sends nothing.
- Reads and writes are served from the renderer's copy, synchronously, as the
  specification requires.
- Each write is **posted** to the browser process and never waited on. The
  browser process checks the write against the cap again and writes it down.
  A write over the cap is refused there whatever the renderer said, and it is
  reported as a broken boundary.
- **One renderer holds a given area at a time**, because every document of an
  origin's site is in that site's one renderer (ADR 0005). The `storage` event
  for other documents of the same key is therefore the renderer's to dispatch,
  as a task (ADR 0016).

The cost is that a write the renderer accepted but had not yet posted when it
died is lost. The specification permits that, and every browser built this way
has the same window.

**`sessionStorage` is never written to a disk.** It is held in the browser
process's memory, per tab and storage key. It goes to the renderer with a load
and comes back as posted writes, as `localStorage` does. A navigation to
another site and back finds it again, and so does a renderer that died. **A
closed tab, a quit or a crash ends it.** That is the promise alo's own sign-in
is written against: its refresh token is in `sessionStorage` *"without
persisting to disk across tab-close"*. Restoring tabs after a crash, when that
exists, restores their addresses and not their `sessionStorage`. A browser that
wrote it down to restore it later would have written a session token to a disk.
ADR 0011 § 2 forbids that by name.

**IndexedDB and the Cache API are asynchronous, so they are messages.** A
request is posted. Its answer arrives as its own task (ADR 0016 § 2). Nothing
waits inside a task.

- **IndexedDB values are stored as bytes the browser process never reads.** The
  renderer serialises a value by the structured clone and sends the bytes. The
  browser process stores them and returns them. Per ADR 0005 it never parses a
  page's content, and a serialised value is exactly that.
- **IndexedDB keys are the one exception, and they are designed so they need
  not be parsed.** The browser process must order keys and answer ranges. A key
  therefore crosses in an encoding of ours in which **comparing the bytes is
  comparing the keys**. Number, date, string, binary and array each have a tag
  and a layout that sorts correctly. The browser process compares bytes. It
  never decodes them, and it refuses a key that is not well formed.
- **Index keys are extracted in the renderer.** That means evaluating a key
  path over a value, and the renderer sends them beside the value. A hostile
  renderer can lie about them. It thereby corrupts only its own site's indexes,
  which it could do anyway.
- **The Cache API stores what the page put in it.** That is the page's choice
  and counts against the page's quota. ADR 0011's never-written list governs
  the HTTP cache, which is disk we take without asking. It does not govern this,
  with one exception. A session-scoped profile writes nothing at all. Every
  bucket there is memory only, for every API.
- **An opaque response counts against the quota at a fixed padded size**, never
  at its real length. A cross-origin response a page may not read could
  otherwise be measured through `usage`. A page could learn the size of
  somebody's private page on another site by caching it and asking. The amount
  is a constant in the code and never depends on the response.
- **The Cache API exists only in secure contexts**, as the specification says.

## 6. What comes back off the disk is untrusted, and losing it is said

ADR 0011 § 4 applies to every bucket file. It means a checksum per record, a
format version, every length checked before anything is reserved, and no
arithmetic that a hostile number can overflow. `LOOP.md`'s stage 2 rule
applies too: malformed, truncated and adversarial input returns an error and
never panics.

One clause differs, because storage is not a cache. **For the cache, an
unreadable entry is a miss. For storage, an unreadable bucket is data a person
or a site wanted kept.** So:

- **A bucket that fails its check is set aside whole.** It is not served at
  all, not even in part, and it is **not deleted**. The site starts as if it
  had never stored anything. Half a database served as if whole is the § 4
  problem again.
- **The browser process records that it happened**, naming the site, where a
  person can see it. A security surface (queue item 127) or settings (queue
  item 128) will show it when they exist. A set-aside bucket is cleared by the
  same act that clears the site's data, and only by that act.

**Durability is the specification's.** An IndexedDB transaction's `complete`
event fires after the browser process has handed the write to the operating
system. A transaction that asks for `durability: "strict"` waits for the write
to be flushed to the device. `localStorage` writes are flushed in batches, and
a crash can lose the last of them. That is stated in § 5 and permitted.

## 7. Persistence is a person's grant, and it comes later

`navigator.storage.persist()` asks for a bucket never to be evicted. That is a
permission. Queue item 93 decides what a permission is: *"enumerated, visible,
revocable, expiring, recorded"*. This ADR will not invent a smaller version of
it.

- **Until item 93 exists, `persist()` resolves `false`** and
  `navigator.storage.persisted()` answers `false`. Both are true answers.
- When it exists, keeping a site's data is one of its capabilities. It is
  asked for by the page, granted by the person, and revocable in the same place
  as every other. A grant raises nothing but protection from eviction. **The
  quota stays 1 GiB.** Whether a granted site may have more is a measurement's
  to decide (§ 3), not a prompt's.

## 8. Deleting is one act, and it is real

**Clearing a site's data clears all of it at once**: its cookies, its cache
entries and every bucket under every top-level site it appears in, including
anything set aside. Deleting storage while leaving cookies, or the reverse,
leaves a site with half of what it uses to recognise somebody, and that is
enough to recognise them. The `Clear-Site-Data` response header, when it is
built, does the same thing through the same function.

Deleting removes the files. As ADR 0011 § 3 says, we make no claim beyond what
the file system does with them. The storage directory is in the place the
operating system keeps **application data**, not caches, because a system that
clears caches to free space must not take a person's work with it. It is
private to its owner, and so is every file in it. Like the cache, it is
protected against another user on the machine and not against a program
running as the person.

## What this costs

- **A site that wants more than 1 GiB cannot have it.** An offline video
  library, a large design file and a local copy of a big mailbox all exist,
  and each will hit `QuotaExceededError` where another browser said yes. That
  is the largest cost, and it falls on the sites with the most ambitious
  offline features.
- **Partitioning breaks embedded state**, exactly as ADR 0007 says for
  cookies. A chat widget, a video player or a comment box embedded in two sites
  remembers nothing between them.
- **Every load of a site with `localStorage` carries its area**, up to 5 MiB,
  the first time a renderer needs it. That is bytes copied that a
  synchronous call would not have copied. We have not measured it and will not
  quote another browser's figure as ours. Any number comes from queue item
  117, on hardware, or is not given.
- **The last `localStorage` writes before a crash can be lost**, and
  `sessionStorage` never survives one.
- **A corrupt bucket loses a site's data until somebody looks.** Setting it
  aside rather than deleting it is the most we can do. It is still a site that
  appears to have forgotten somebody.
- **A key encoding of our own** is one more format to own, test against
  hostile bytes and never change without a version.

## Alternatives rejected

- **A quota as a share of the disk**, which some shipped browsers have used.
  It leaks the disk's size to every page, as § 3 says, and it promises a
  site the person's free space.
- **A prompt at a size threshold**, as early browsers did at 5 MiB or 50 MiB.
  A prompt about megabytes asks a person a question they cannot answer. It
  teaches them to click yes. Item 93 is where asking is designed properly.
- **A quota per API.** It would not be *one quota policy*, and it lets a page
  fill each part in turn.
- **A renderer holding its site's storage directory.** It is simpler and
  faster, and it is refused by § 5 and ADR 0011 § 5 for the same reason.
- **A synchronous call for `localStorage`.** It needs no copy in the renderer,
  but ADR 0005 forbids exactly this, and it is what makes a renderer
  wait on a browser process that may itself be waiting.
- **Evicting part of a bucket.** It reclaims more precisely and leaves
  applications in states nobody tested.
- **Storage keyed by origin alone.** This is the unpartitioned model ADR 0007
  refused for cookies, and storage is a more capable identifier than a
  cookie.

## What this does not decide

- **What IndexedDB's store is built on.** Ordered keys, transactions and
  crash-safe commits are a storage engine. Whether that engine is rented, and
  from whom, or written here over the record format the cache already owns, is
  a decision of its own. It is made when IndexedDB is taken, under ADR 0001's
  rule and ADR 0023's practice: rented only in Rust, under a licence that sits
  beside MPL-2.0 (ADR 0009), and with its files read as untrusted (§ 6). No C
  database is rented, for the reason ADR 0023 gave about libopus.
- **What a permission is**, item 93, beyond § 7's one use of it.
- **Storage in a worker**, item 91. A worker's storage key is its owner's, and
  its requests cross the same boundary. Its loop is ADR 0016 § 8's.
- **What private browsing is**, item 125, beyond §§ 3 and 5: memory only, the
  same reported quota.
- **Whether storage follows a person between machines.** That is item 145, and
  nothing here authorises sending a stored byte anywhere.
- **The storage-access grant's reach into storage**, as § 1 says.
- **`SharedArrayBuffer` in a stored value**, which ADR 0013 § 7 refuses. A
  value that cannot be cloned is the specification's `DataCloneError`.

## How we will know if this was wrong

- **If real pages hit 1 GiB**, the default is too small for the web we
  render. If frozen corpus cases or a person's week of use (stage 2's gate)
  show `QuotaExceededError` on sites that are working as intended, raise the
  number with that measurement beside it. The shape stays.
- **If eviction is taking buckets people expected to keep**, the profile's
  bound is too tight or the order is wrong. The store's counts of evictions,
  by reason, are the measurement.
- **If the cost of carrying `localStorage` with each load shows up in item
  117's measurements**, send the area once per renderer and keep it there.
  That is already the rule. Then look again at the cap before looking at a
  synchronous call, which stays refused.

Each of these is a measurement rather than an argument, the standard ADRs 0007,
0008 and 0011 set for coming back to a decision.

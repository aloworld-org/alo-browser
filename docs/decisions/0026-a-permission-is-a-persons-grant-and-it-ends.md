# ADR 0026 — A permission is a person's grant, and it ends

**Status:** accepted
**Date:** 2026-10-07
**Context:** queue item 93, *"permissions as capabilities — camera,
microphone, location, notifications, in the shape of `alo-os` ADR 0001:
enumerated, visible, revocable, expiring, recorded"*, marked *needs ADR*. It
also uses:

- `alo-os` ADR 0001 § 3, read on 2026-10-07: a grant comes from **a deliberate
  act**, and grants are *"enumerated (a list, not a rule), visible where the
  person can find them without hunting, revocable in one action, and expiring
  by default. There is no grant that outlives the reason it was made."* Its
  rejected alternatives refuse *"a permission prompt per action, with a
  'remember this' checkbox"*, because remembering turns one approval into an
  unbounded session.
- `alo-os` ADR 0005: an application asking for the camera is the same grant
  model as an agent asking for a path, and *"the person sees one list of what
  has been granted to what"*.
- ADR 0005: the browser process holds *"the decisions about who may do
  what"*, a renderer has no device and no filesystem, and *"a renderer must
  never be able to grant itself"* anything.
- ADR 0007: the storage-access grant is *"a per-site grant, made by the
  person, in response to a specific ask"*, naming *who* is asking and *inside
  what*. It is never a global toggle and never an allowlist we ship.
- ADR 0012: every request carries its cause (`Person`, `Document` or `Agent`),
  and *"a record is not a permission, and an action being recorded must never
  become the argument that it was authorised"*.
- ADR 0016: an answer from the browser process arrives as its own task.
- ADR 0018 § 4: an agent's event is **trusted** and unmarked, so a page cannot
  tell it from a person's. *Whether* an agent may act is a permission, left to
  items 93 and 133.
- ADR 0020 § 3: a link to `mailto:` and other schemes is refused, because
  handing a person to another program *"is a permission (item 93), not a
  link"*.
- ADR 0024 § 4: the browser's own interface is a document of ours, rendered by
  the engine in a sandboxed renderer of its own and built from data, so a
  stranger's string never becomes markup and the agent reads the interface as
  it reads a page.
- ADR 0025: the **storage key** is the origin and the top-level site, an opaque
  origin has none, and § 7 leaves `navigator.storage.persist()` answering
  `false` until this decision exists.
- The code this has to fit: `alo-net`'s `Cause` (`cause.rs`) and `Partition`
  (`cookie.rs`), and `alo-storage`'s `StorageKey` (`key.rs`).

## The decision in one line

A permission is a **grant** in one table the browser process holds: one
capability from a **closed list**, to one **storage key**, made by **a
person** answering a specific ask that a page made **while somebody was using
it**. It **always ends**, either when the page closes or thirty days after the
person last opened the site. It is **shown** in one list and while in use,
**revoked** in one act that takes effect at once, and **recorded** without what
it carried. **No agent can make, answer or revoke one.**

## Why this is a decision rather than a chore

Every browser has permissions, and the usual design is the one `alo-os`
ADR 0001 rejects by name: a prompt per capability whose answer is kept for
ever, in a list a person meets only by going looking for it. Four questions
decide whether ours is different, and an iteration that built the camera
first would answer each of them by accident.

- **To whom is it granted.** It could be granted to an origin, a site, or a
  tab, and to a page or to a page inside another page. The answer decides
  whether a grant made to a site you trust reaches a widget embedded in it.
- **For how long.** "Remember" with no end is the unbounded session that
  ADR 0001 refuses. Asking every time is a dialogue people learn to click
  through, which is worse.
- **Who can say yes.** ADR 0018 made an agent's click indistinguishable from a
  person's to a page. If an agent could also answer the prompt that click
  raised, a page that has persuaded an agent would hold the camera. Its owner
  could persuade the agent by text the agent reads.
- **What the record is.** A dialogue nobody can audit afterwards is the
  sentence in the roadmap this item exists to answer.

## 1. A capability is one of a closed list

A capability is a Rust `enum`. Adding a variant changes this table, in an
amendment to this ADR, before any code changes. A capability is never a string
a page names, and there is no "other".

| Capability | What it reaches | Asked by |
| --- | --- | --- |
| **Camera** | video frames from a camera device | `getUserMedia({ video })` |
| **Microphone** | audio frames from an input device | `getUserMedia({ audio })` |
| **Location** | the machine's position | `navigator.geolocation` |
| **Notifications** | showing a notification outside the page | `Notification.requestPermission()` |
| **Keep data** | protection of the site's bucket from eviction (ADR 0025 § 7) | `navigator.storage.persist()` |
| **Storage access** | the embedded site's unpartitioned cookies (ADR 0007) | `document.requestStorageAccess()` |
| **Open another program** | handing a person to a scheme another program owns (ADR 0020 § 3) | navigating to `mailto:` and the like |
| **Read the clipboard** | what the person last copied, from any program | `navigator.clipboard.read*()` |

Camera and microphone are two capabilities, even when one call asks for both,
because a person may want to grant one. A prompt that asks for both offers them
together and records them as two grants.

**What is not on the list is not a permission.** Some things a page does need a
person's recent gesture and nothing more: writing to the clipboard, going full
screen, and starting playback with sound (ADR 0023, item 291). They need
**transient activation**, exactly as the specifications say. They are never
stored, and so they are never a row in the table. Making one of them a
permission would add a prompt and buy nothing, because a person who clicked
*copy* has already answered.

**Anything a page asks for that is not on the list is refused**, and the
refusal names this ADR. That covers screen capture, MIDI, USB, serial,
Bluetooth, HID, idle detection, sensors and the rest. The page is told, as the
specifications say, through `NotAllowedError` or `"denied"`. A capability
joins the list when a frozen page needs it and an amendment says what it
reaches (`LOOP.md`, stage 2 § 1).

## 2. It is granted to a storage key, and the ask names both halves

A grant belongs to **one capability and one storage key**: the origin of the
document that asked and the top-level site it is under (ADR 0025 § 1). It is
the same key that holds the site's data.

- **`meet.example` opened on its own** and **`meet.example` embedded in
  `news.example`** are two keys. A grant to the first does not reach the
  second.
- **A grant to `news.example` does not reach a frame it embeds.** Permissions
  Policy's `allow="camera"` on the `<iframe>` is **necessary**, because the
  embedding page must agree. It is **not sufficient**, because the person must
  also agree, about that frame. Several browsers let an embedder's grant flow
  into a frame it delegates to. We do not, for ADR 0007's reason: the person is
  told *who is asking, inside what*, and answers for that pair.
- **An opaque origin has no key and so can hold no grant.** Its ask is refused
  without a prompt. That covers every `file:`, `data:` and `about:` document,
  and every sandboxed frame without `allow-same-origin`.
- **Only a secure context may ask**, whatever the specification for a
  particular capability once allowed. An insecure origin's ask is refused
  without a prompt. A grant to a name an attacker on the network can answer for
  is a grant to the attacker.

Frames are item 86's and not built. This section is how they arrive, so that
the rule exists before the first frame does.

## 3. A page may ask only while somebody is using it

**An ask needs transient activation in the asking document**, which means a
gesture moments before. A page that asks for the camera or for notifications
while it loads, before anybody has touched it, is refused **without a prompt**.
`"denied"` is not reported to it either. It receives the answer an ask that was
not made would receive: `"prompt"` from `permissions.query()`, and
`NotAllowedError` from the call.

That is stricter than every specification on the list except the clipboard.
It ends the prompt that arrives with the page, which is most of why people
learn to dismiss them unread.

- **An ask from a tab that is not selected waits** until the tab is selected.
  A prompt is drawn only over the tab that asked and never over another.
- **One prompt at a time per tab.** A second ask while one is showing waits
  behind it. A page cannot stack prompts to hurry a person through them.
- **A document that was refused, or whose prompt was dismissed, is refused
  without a prompt** for any further ask of the same capability, until it is
  gone. A page cannot ask again in a loop.

## 4. Only a person answers, and never an agent

A prompt is answered by a person's own input, from the keyboard or a pointer,
which the browser process reads before any renderer does (ADR 0024 § 5).

- **No agent verb reaches a prompt.** The prompt is browser interface (§ 6),
  and the agent may **read** that one is showing, what asked, and for what.
  Every verb on it is refused, by name and permanently. This is not a gap left
  for item 133 to fill. A grant is a deliberate act of a person
  (`alo-os` ADR 0001 § 3), and an agent answering an ask is the agent granting
  itself, through a page, the thing the page wanted.
- **An ask a page made in answer to an agent's action is still shown**, to the
  person, and the prompt says that an agent was acting when the page asked.
  ADR 0018 § 4 means the page cannot tell, but the browser process can. ADR
  0020 § 4 already attributes the work done in answer to an `Act` to that
  action, so the attribution comes from that task and is never from the page.
  The page is never told that an agent was involved.
- **No agent verb makes, changes or revokes a grant**, in the list or anywhere
  else. Reading the list is a reading under item 133's grants.
- **A renderer cannot answer for anybody.** The browser process decides. A
  renderer's message saying that something was granted does not exist in the
  wire format (§ 7).

## 5. Every grant ends

A prompt offers three answers, and no fourth:

- **Allow while this page is open.** The grant ends when the document that
  asked is gone: navigated away, closed, or its renderer died. This is the
  answer the prompt offers first.
- **Allow on this site.** The grant ends **thirty days after the person last
  opened the site themselves**. That means a top-level load of the key's
  origin whose cause is `Person` (ADR 0012), not one a page or an agent
  caused. A site you use every week keeps its grant. A site you visited once
  in March has lost it by May.
- **Don't allow.** This is remembered on the same terms as *allow on this
  site*, so a page refused once cannot ask again on every visit. Changing it is
  the person's act, in the list (§ 6).

There is no *always*. There is no *remember this* box. There is also no
setting that turns a grant into one without an end. Dismissing the prompt is
not *Don't allow*. It refuses that one document (§ 3) and stores nothing.

**Why thirty days, and why from the person's last visit.** It is the shortest
period after which a person who uses a site regularly is never asked twice.
Counting from the person's own visits, rather than from use, means the site
cannot keep its grant alive by using it. A notification permission renewed by
sending notifications would never end. The number is **ours to choose and ours
to change**, as ADR 0025 § 3 says of its numbers. A change with a measurement
behind it is a change of a constant, not a new decision.

**The clock is the wall clock here, and it fails closed.** An expiry is a date
a person understands, so it is kept as one, unlike ADR 0011's use counter. A
grant whose recorded time is in the future of the current clock is treated as
**expired**, so a clock set backwards cannot extend one.

**A person may refuse a capability for every site**, in the list: *never ask
me about notifications*. A blanket refusal is safe in the direction it errs.
**A blanket allow does not exist**, for ADR 0007's reason about global
toggles, and **we ship no allowlist**. alo's own pages ask like everybody
else's.

**A private session starts with no grants** and keeps the ones it makes in
memory, for that session only. Nothing it grants or refuses is written to a
disk. What a private session is beyond that is item 125's.

## 6. Visible: one list, and an indicator while in use

**One list.** Every grant and every remembered refusal is a row: the
capability, the origin, the top-level site, the answer, when it was given,
when it ends, and when it was last used. The list is reached from the page it
concerns in one step, and the whole list is reached from settings (items 127
and 128 build the surfaces). Item 133's grants to an agent will be rows of the
**same** table when they exist, because `alo-os` ADR 0005 says a person keeps
one list and not two.

**An indicator while a device is in use.** While a tab holds the camera, the
microphone or the location, its tab says so in the tab strip, and the window
says so whichever tab is selected. The indicator is a fact in the data the
browser process sends the strip's renderer (ADR 0024 § 4), so it is drawn by
our document and never by the page, and no page can hide or imitate it in the
strip.

**The prompt is a document of ours too**, rendered the way the tab strip is: in
a sandboxed renderer at an internal site no page can name, built from a typed
message with `alo-dom`'s operations, with the origin and the top-level site as
**text nodes**. No page-chosen string becomes markup, no script runs, and the
agent reads it as a tree. The prompt shows the asking origin and the top-level
site. It never shows the page's title or any other words the page chose, so a
page cannot write the question.

## 7. Revocable, and enforced where the device is

**The browser process holds the table, and checks it at every use.** A renderer
holds no grant and no device. It asks, the browser process decides, and the
answer is a task (ADR 0016). Frames from a camera, audio from a microphone and
a position all come from the browser process or a process it starts (ADR 0023's
media process). Each is checked against the table **when it is sent**, not
only when it was first asked for.

**A storage key is served only for a document the browser process put
there**, as ADR 0025 § 5 says for storage. An ask naming a key whose document
the browser process did not load into that renderer is refused and reported as
a broken boundary. A compromised renderer can therefore ask with its own key,
and the person still answers.

**Revoking is one act and it is immediate.** Removing a row stops every use
under it before the act returns. The browser process stops sending frames,
ends the stream (the page sees its track end, as the specification says), and
stops answering position watches. A revocation that waited for the page to
cooperate would not be one.

**Clearing a site clears its grants**, its refusals and its history, in the same
act that clears its cookies, cache and buckets (ADR 0025 § 8). A site whose
data is gone but whose camera grant survives is a site half forgotten. The
person can also remove grants alone, without clearing data.

## 8. Recorded, and the record holds no content

The browser process records each **ask**, each **answer** (including that a
prompt was dismissed, or that an ask was refused without one, and which rule
refused it), each **start and end of a use**, each **revocation** and each
**expiry**. Each entry names the capability, the storage key, the time, and the
cause under ADR 0012, whose chain reaches an agent's action when there was one.

- **It never holds what the capability carried**: no frame, no sound, no
  position, no notification text, no clipboard contents. A record of where
  somebody was is a tracker we would be keeping on them.
- **It is kept with the grant table**, for as long as the site's grants are,
  and bounded at **sixty-four entries per storage key**. The oldest is dropped
  first and the drop is counted, so a short history never reads as a quiet
  one. ADR 0012 keeps a person's browsing in memory only. This is narrower: it
  says what a site was allowed and did with it, and that is what a person
  opening the list wants to see next week.
- **A private session writes none of it** to a disk.
- **No page and no agent reads it.** It is the person's, through the list.

The grant table and its history are a file in the profile, under ADR 0011
§ 4's rules for anything read back off a disk: a version, a checksum per
record, every length checked before anything is reserved, no arithmetic a
hostile number can overflow, and an error returned rather than a panic. **A
table that fails its check grants nothing.** It is set aside, not deleted, and
the person is told. Every site then asks again, which is a nuisance. The
other failure would be a corrupt table read as a grant, and that is a breach.

## 9. Where it is built

- **The table, its decision function, expiry, revocation, the record and its
  file** are browser-process code in a crate of their own, **`alo-grants`**.
  It holds no device and no interface. Its question is *may this key use this
  capability now*, and it answers in a value. It can be built and tested on
  any machine today, and it is the first item cut from this one.
- **The prompt and the indicator** are documents and data, as § 6 says. They
  wait on the window (item 296) and the tab strip (item 297).
- **Each capability's API** is built when a frozen page needs it, with the
  table already in place to ask. `persist()` is item 303's, storage access is
  item 157's, and the clipboard is item 92's. Camera and microphone wait on
  the media process (item 290). Location and notifications each wait on a
  page.

## What this costs

- **Pages that ask on load stop working until somebody touches them.** A video
  call page that asks for the camera the moment it opens shows its "camera
  blocked" message first. That is the largest compatibility cost here, and it
  falls on pages that were written to be convenient for the browser that asks
  on load.
- **A widget embedded in two sites asks twice**, and an embedded call widget
  asks even when the person has granted the site around it. ADR 0007 accepted
  the same cost for cookies.
- **A site unvisited for thirty days loses its notifications**, so a person who
  wanted a rarely opened site to tell them about something will stop hearing
  from it. They are asked again on their next visit, with a gesture.
- **No *always*.** A person who wants a site to keep the camera forever cannot
  set that up. Visiting it at least once a month has the same effect.
- **Every capability on the list is a prompt we draw and a row we keep.** The
  list stays short for that reason, and capabilities outside it are refused
  until a page needs them.
- **A corrupt table loses every grant at once.** It is safe and it is
  irritating.

## Alternatives rejected

- **"Remember this decision" with no end.** `alo-os` ADR 0001 rejects it by
  name. It is the unbounded session, and it is how every browser's permission
  list has come to hold grants nobody remembers making.
- **Ask every time.** No grant lasts long enough to go stale, but a prompt that
  appears on every visit is clicked through unread within a week. That makes
  it a dialogue nobody reads as well as one nobody audits.
- **Grant to the top-level site, and let it delegate to frames.** It is fewer
  prompts. It also means a person who trusts a site with the camera has
  trusted every frame that site chooses to embed, and was never told who those
  are.
- **Keep the decision in the renderer, with the browser process enforcing only
  the device.** It is simpler for every API that answers a query. It also
  makes a compromised renderer the authority on what was granted, which
  ADR 0005 refuses in one sentence.
- **Let the agent answer prompts under a grant of its own.** It is convenient
  for an agent that is filling a form that wants the location. It is also how
  a page that can persuade an agent comes to hold the camera, and persuading
  agents by text is what pages that want things will do.
- **A blanket allow, or an allowlist we ship**, for alo's own pages above
  all. A browser that trusts its maker's pages with the camera without asking
  is in the business ADR 0007 refused to enter.

## What this does not decide

- **What an agent may do on a page**, which is item 133. This decides only
  that its grants live in the same table and that it never answers a prompt.
- **The surfaces of the list**, which are items 127 and 128. This decides what
  they show.
- **Which camera, which microphone.** Device choice is part of the prompt when
  the media process exists, and it is item 290's to shape.
- **Notifications outside the browser.** Whether a notification is the
  operating system's, and what alo OS's shell does with it, is decided when
  notifications are taken. Nothing here sends one anywhere.
- **Push messaging and background sync.** They need a service worker
  (item 91), and they are capabilities for an amendment when a page needs
  them.
- **Profiles**, item 125, beyond § 5's private session.

## How we will know if this was wrong

- **If people answer *allow on this site* almost always**, and the thirty-day
  expiry produces repeated asks for the same capability from sites they use,
  the period is too short. The record (§ 8) counts asks per key and capability,
  so this is a measurement rather than a feeling.
- **If frozen pages fail because they ask before a gesture**, and the page
  cannot reasonably be expected to wait, § 3 is too strict for that
  capability. Relax it for that capability, in an amendment, with the page
  beside it.
- **If a grant is ever honoured after its row is gone**, the enforcement in
  § 7 is in the wrong place. That is a defect in the boundary, as ADR 0005 says
  of a renderer crash that becomes a browser crash. It is not a reason to
  change this decision.

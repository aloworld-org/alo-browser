# ADR 0092 — Chats are kept on the machine, a file each, in words

**Status:** accepted — built by queue items 757 and 758 (1 November 2026)
**Date:** 2026-11-01
**Context:** ADR 0085 (the conversation keeps every turn, per chat; an
earlier card is its address, not a picture); ADR 0074 (the interface is a
conversation, drawn by confined renderers from state the browser process
sends); ADR 0087 (a search's videos and news are paid for only when their
tab is opened); ADR 0081 (the model alo thinks with — not decided); ADR
0069 and item 733 (every Chromium's profile is temporary and removed);
ROADMAP's *no telemetry*; `docs/design/shell.md`, *The chats panel*
(*Earlier — past chats … Closing a chat never loses it*); queue items 703,
757 and 758

## The question in one line

Each chat's turns live only while the window does: where are they kept so
that a chat outlives a launch, in what form, what is never written, what
becomes of what was live or under way, what happens to a file that does not
read — and what is a chat called?

## 1. Where: a directory of alo's own, on this machine, only this user's

On a Mac, `~/Library/Application Support/org.aloworld.Browser/Chats`: the
place macOS gives an application for what it makes, under the
application's identifier (`info_plist::IDENTIFIER`), so it is alo's and no
other program's by name. Elsewhere, `$XDG_DATA_HOME/alo/chats`, or
`~/.local/share/alo/chats`. It is made on first use readable by this user
alone (`0700`), and each file in it is `0600`.

`alo --conversation --chats DIR` keeps them in `DIR` instead, an absolute
path. That is how a hand run is kept apart from a person's chats, and the
terminal says, as the window opens, which directory is used. Every test
names a directory of its own under the temporary directory; nothing in the
test suite touches a person's.

**Nothing in it leaves the machine.** Only the browser process reads or
writes it; no renderer is given its path; nothing it holds is sent
anywhere, to alo or to anyone (ROADMAP: no telemetry, not *anonymised*
telemetry). Reading it asks nothing of the network either: a chat read back
is drawn from what was written, and a search is asked again only when a
person searches again.

This is the first thing alo keeps between launches. Every Chromium's
profile is still temporary and removed (ADR 0069), so what a page stored —
cookies, a sign-in — is still not kept; that is not this decision.

## 2. What: words and addresses, never a picture's bytes

A chat is **its name and its turns**, as text:

- **A search** — what the person said, the tab shown, and each of its four
  searches as it stands: the sites' titles, addresses, descriptions and
  names; the images' titles, pages and sources; the videos' titles, pages,
  lengths, sites, ages and descriptions; the news stories' titles, pages,
  sites, ages and descriptions; or why one was not answered.
- **A page** — what was said for it, and the address it was last at. **A
  live card is written as the page shown earlier it becomes** when anything
  newer comes (ADR 0085 § 2): after a relaunch no page is open, so it is a
  line with its address, one press from the page itself.
- **A page that could not be opened** — what was said, and why not.

**Never written:** a picture's bytes — a thumbnail, a site's mark, a
card's frame; anything typed into the box and not chosen; how far a chat
was scrolled; a provider's key; anything of a Chromium's profile. The
pictures were considered and refused on a number: a search's thumbnails
and marks run to a few hundred KiB where its words are a few KiB, so
writing them would make every chat's file some hundred times the size of
what it says, written again at each change, to keep what the provider's
proxy hands out again on request. Read back, an image is drawn by its title
on its tile, a site by its letter on its tint — as each is drawn already
when its picture does not come.

**Not encrypted by alo.** The files are the user's, `0600` in a `0700`
directory, on a disk FileVault encrypts when a person has turned it on.
Encrypting them under a key in the keychain would keep them from another
program running as the same person, which `0600` does not; it is the
owner's choice and its cost — a key that, lost, loses every chat — is queue
item 790.

## 3. How: a file per chat, JSON, written whole or not at all

`chat-N.json`, N the chat's number: given as the chat is made, never
reused, the newest highest, past every number any file in the directory is
named by. So the panel's order — newest first — is the files' numbers, and
no index file lists them (an index would be a second file to keep in step,
and a second thing to lose). One chat changing rewrites one file.

The format is JSON — `serde_json` is already the browser process's, for
Chrome's protocol and the provider's answers — one object:
`"format": "alo chat"`, `"version": 1`, `"name"`, `"turns"`. A chat with
no turns is never written.

**Atomic.** Each write goes to `chat-N.json.writing`, is flushed with
`fsync`, renamed over `chat-N.json`, and the directory flushed. A rename
within a directory is atomic, so the file is always the last whole write or
the one before, whatever stops alo between the two; a `.writing` left
behind is never read and is overwritten by its chat's next write. A chat is
written when what is kept of it changes — a turn, its results, its card's
address, its name — and not for a card's frames, which are never kept; the
last of it is written as the window closes.

**What was under way is read as what it came to.** A search still
searching when alo closed is read as unanswered, saying so (*This search
was not answered before alo was closed.*); a tab's results still being
asked for as never asked, so opening the tab asks again, as ADR 0087 has it,
and a tab so left is not the one shown.

## 4. A file that does not read is said, and never lost

A `chat-N.json` that does not read as a chat — not JSON, another format or
version, a field missing or of the wrong kind, more turns than a
conversation holds, an address that is not a site's where one must be — is
**not read at all**, never in part. It is **left where it is, untouched**,
and said, with the file's path and why, on the terminal and in the first
screen's line as the window opens. Its number is never given to a chat
again, so nothing is written over it. A directory that cannot be made or
listed, or a chat that cannot be written, is said the same way — *chats are
not kept: …*, *a chat is not kept: …* — and the conversation goes on, so
nobody believes a chat kept that is not.

**Nothing here deletes anything.** The panel has no way to delete a chat
yet; that is queue item 789. Until then a person deletes one by removing
its file, which this ADR names so that it can be found.

## 5. Kept chats are *Earlier*; a chat is named by its first words

A relaunch opens on **a new chat**, on screen and open, as a first launch
does; the chats read back stand under the panel's *Earlier*, newest first,
"past chats" as the design has it. Pressed, an earlier chat is on screen
again, scrolled to its newest turn's top, and **in use again**: drawn under
*Open*.

**A chat is named by the first words said in it** (item 758,
`chat_name.rs`): once it holds a turn, the first turn's words — a search's,
an address typed, or a site's title pressed — their white space run
together, as many whole words as fit in 48 characters, and `…` after them
when any were left out; a single word longer than that is cut at a
character. Named once, as the first turn is made; a later turn does not
rename it, and the name is kept with it. 48 is more than the panel's row
shows (some thirty characters of 13.5 px text), which the panel's sheet
cuts where the row ends, so *Search chats* finds a chat by more of its
words than the row draws.

No model names it. Which model alo thinks with is not decided (ADR 0081),
and an agent's name for a chat waits on the agent's record (704). A rule
over the person's own words says nothing they did not say, and is checked
today; a model's name replaces it when there is one, by decision.

**Closing a chat** — moving an open chat to *Earlier* while alo runs, as
the design says — is not built: there is no control for it, and what it
looks like is not designed. That is queue item 791.

## Consequences

- `alo-window`: `chat_record.rs` (the format, written and read),
  `chat_store.rs` (where, the atomic write, the reading and the damaged),
  `chat_name.rs` (the name), `Conversation::keeping`, `to_keep` and the
  naming, `Thread::kept`, `ConversationWindow::with_chats`,
  `keeping_nothing` and `keep`, and `--chats DIR` (`opening.rs`, `alo.rs`).
- Tests: the format round trip and every refusal (`chat_record`), the
  store's modes, numbers, damaged files and refusals in a temporary
  directory (`chat_store`), the name (`chat_name`), the conversation's
  keeping, numbering and naming (`conversation`), and two launches through
  the window's own road on one directory
  (`tests/chats_kept_across_launches.rs`).
- Cut, each queued: deleting a chat from the panel (789); chats encrypted
  at rest, the owner's choice (790); closing a chat to *Earlier* while alo
  runs (791).

# ADR 0092 — Chats are kept on the machine, a file each, in words

**Status:** accepted — built by queue items 757 and 758 (1 November 2026); amended for 814, 820, 789, 791 and 824; § 2's *not encrypted* superseded by ADR 0098 (790)
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

**Not encrypted by alo** — *superseded on 6 November 2026 by ADR 0098: the
files are now sealed under a key in the person's keychain.* The files are the user's, `0600` in a `0700`
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

**Nothing here deletes anything** but a person's *Delete* (amendment of 6
November 2026, queue item 789): a damaged file never.

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

## Amendment, 2026-11-01 — a search's answer is kept (queue item 814)

§ 2's list of what a search keeps gains **its answer, in words**: a
written answer's line, its sentences and its points, each with the
numbers of the sites it cites (`"answer": {"written": {"line", "sentences":
[{"text", "sources": [1, 3]}], "points": […]}}`); or `"not found"` — the
model said the sites do not reliably answer, *I couldn't find a reliable
answer*; or why it was not answered (`{"unanswered": WHY}`); or `"not
asked"`. Only the words drawn are kept — the same few hundred characters
the Answer tab shows — and nothing of what was sent to have it written
(ADR 0095 § 3) that the search does not already keep.

- **Read back checked as the wire checks it.** None to a search that found
  no sites; a written answer only when `Written::check` passes against the
  first eight sites read with it — a line and a sentence, at most three
  sentences and six points, none past 600 characters, every one citing at
  least one source and every number one of the sites given. A file whose
  answer fails is **not read at all** (§ 4), never read without its
  answer: a citation the person was never shown is never drawn.
- **Never asked again.** A chat read back shows what was written; nothing
  in reading it asks the model, and a search is asked again only when a
  person searches again (§ 1). An answer still being written when alo
  closed is read as not written, saying so (*This answer was not written
  before alo was closed.*), and is not asked again either.
- **The Answer tab** is read back as the tab shown when it was, as every
  other tab is; a chat written before this amendment holds no answer, is
  read as never asked, and is shown on All. `"version"` stays 1: the
  field is added, a file without it is read, and a browser before 814
  reading a file with it reads every field it knows.

Checked by `chat_record`'s tests (each kind round trip; a citation past
the sites read, past the eighth, to a search with none, more sentences
than are drawn, a number that is not one — each refused by name) and
through the window's road (`tests/a_searchs_answer_in_the_window.rs`): an
answer kept, two launches on one directory, the chat pressed under
*Earlier* painting its Answer tab's rows as they were, and no request to
the model.

## Amendment, 2026-11-01 — alo's agent's runs are kept as turns (queue item 820)

A run of alo's own agent (ADR 0096) was drawn on its live card and went
when the card was put away; the file kept the page's address and nothing of
what was done on it. § 2's list of what a page keeps gains **each run of
the agent on it**, oldest first, kept with the page it worked on
(`"visited": {"said", "url", "ran": [{"asked", "steps": [{"line",
"why"}], "ended": {"done" | "stuck" | "stopped": …}}]}`), and drawn, once
the page is shown earlier, under its line as turns of the conversation:
what the person asked as their bubble, then what alo did — one line per
step, why under it when it chose, and *Done in N steps* and the result, or
why it was stuck or stopped (`ran_document.rs`, a reference render).

**What is kept, and why each:**

- **What the person asked** — as they said it, as a search's words are:
  it is the person's own turn.
- **Each step's line and why, as the person was shown them** — the run is
  the person's record of what was done in their name, and the steps are
  what they read while it was done. They are the model's words about the
  page, and so hold **page text only as far as the steps already said
  it**: no tree, no field's value, no address but the page's own is added.
- **How it ended** — the result in full, or why it was stuck or stopped.
  A run still at work when alo closed is kept as *alo was closed before
  this was finished.*; one put away with its card as *alo stopped: the
  page was closed or replaced.*; one replaced by a newer as *You asked alo
  something else, so it stopped.*

**What is never kept:**

- **What was typed into a field**, unless the person's own words already
  said it (`agent_kept.rs`). The browser process hears what each typing
  step put into its field (never drawn, never on the terminal). A typing
  step whose text the person's request holds as whole words — *search
  Wikipedia for Kampala*, typing *Kampala* — is kept as shown; any other
  typing step is kept as *Typed into a field*, with no why; and every
  other line, why and result has each such text replaced by `…` wherever
  it stands as whole words, in any case. A value the agent read off the
  page or made up — an address, a name, a code — never reaches the file.
  **Its limit**: a model that writes a typed text otherwise than as typed
  — spelled out, split, translated — is not recognised.
- **A stuck run's choices**: buttons on a live card, no use on a line.
- **The question it asked the person**, *Allow* or *Not now*: how the run
  ended says what came of it.
- A password: never typed (ADR 0096 § 6), so never kept; a page's
  password field's dots never in a step.

**Read back checked** as the wire checks it: at most 20 runs a page
(`MOST_RUNS`; past it the oldest is not kept), each of at most 40 steps;
an ending of no known kind, a step without its line or why, refuses the
whole file (§ 4). A file without `"ran"` — kept before 820 — reads as no
runs. **Never asked again**: a run read back is words; nothing is sent to
the model, and nothing is done on any page.

A live card's runs are the browser process's: the wire carries them only
with a page shown earlier, and the live card draws its newest run as it
did (ADR 0096 § 7).

Checked by `agent_kept`'s, `conversation_work`'s, `conversation_thread`'s
and `chat_record`'s tests (each mutated: the typed text kept, a run put
away left as at work, the runs not read — each fails), the wire's bounds
(`messages_across_a_boundary.rs`), a layout assertion and a reference of a
page with two runs kept (`the_agent_at_work.rs`, `agent-runs-kept.png`),
and through the window's road on both Chromiums
(`the_agent_at_work_on_a_card_through_chrome.rs`): seven runs over two
cards read back from the chat's file — the text typed into the password
field, which the person never said, and the page's own password absent.

## Amendment, 2026-11-06 — a chat deleted, with an undo (queue item 789)

Approved by the owner on 6 November 2026 as proposed: *Delete* when
hovering over a chat in the panel, an *Undo* for a few seconds, the file
removed only once the undo has passed or on quit, never another chat's.

- **Delete is offered on the row the pointer is over.** As the pointer
  moves over the panel the browser process asks the panel's renderer what
  is under it (`ToRenderer::ConversationHover`, answered as a press is,
  with the claim under the point; only *which chat's row* is read of it),
  and draws that row with a *Delete* button at its end — the row does not
  move; its name gives way. Off the panel, or the cursor out of the
  window, no row offers it. The rail shows no chats, so it offers none.
  A press on *Delete* is decided only for the chat whose row was painted
  offering it and still does (`Shown::hovered`); anything else is refused
  by name, as every claim is.
- **Out of the panel at once; gone after five seconds.** The chat leaves
  the panel the moment *Delete* is decided — its row, its turns and where
  it stood held in the browser process (`chat_delete.rs`) — and the
  panel's foot says *Deleted “…”* with **Undo**, in a dark pill above
  *Settings*, for **five seconds**, the time alo's own *Undo send* holds a
  mail (`alo-workplace`'s mail module). *Undo* puts it back where it
  stood, its turns as they were, not on screen. When the five seconds
  pass, when another chat is deleted, or when the window closes, the chat
  is **gone**, and only then is its file removed — so a chat brought back
  never needed writing again, and one deleted by mistake was never off the
  disk. The window wakes for it (`ConversationWindow::wake_at`, the event
  loop's `WaitUntil`); nothing polls.
- **Its own file alone.** `ChatStore::remove(N)` removes `chat-N.sealed`
  and, if that chat was read from words not yet sealed, its `chat-N.json`
  — never a file of another number, never a damaged file (which is never a
  chat in the panel, and whose number is never given again). Its number is
  not given again either. Removing is said on the terminal; a file that
  cannot be removed is said as a chat not kept is.
- **The chat on screen deleted** puts its card away first (the live page
  closed, the agent at work on it stopped, as any card put away) and puts
  an open chat with nothing in it on screen, or a new one.
- **What an agent reads**: the hovered row is read with its *Delete*, as a
  finished chat is with its *Done*, and the button is named *Delete* and
  the chat's name; the foot's pill is a `status` holding *Deleted “…”* and
  a button *Undo*. An agent's verbs never reach the panel (ADR 0074); a
  person deletes.

Checked by `panel_document`'s, `conversation_claim`'s (a press on *Delete*
and *Undo*, a hover finding the row), `conversation_decide`'s (*Delete*
and *Undo* only for what was painted and is still held), `conversation`'s
(out at once, back with *Undo* where it stood, gone after five seconds, on
another delete and at closing; the chat on screen deleted) and
`chat_store`'s tests (its own files and no other), a layout assertion and
a reference of the panel offering both (`the_chats_panel.rs`,
`panel-deleting.png`), the wire's round trip, and through the window's road
on a temporary directory (`tests/chats_kept_across_launches.rs`): hovered,
deleted, brought back, deleted again and its file gone when its five
seconds pass — the other chat's file kept — then deleted and the window
closed before they pass, its file gone, and a relaunch showing neither.
Mutations — the file never removed, *Delete* decided against what is held
now rather than what was painted — each fail.

## Amendment, 2026-11-06 — a chat closed to *Earlier* (queue item 791)

The owner approved this on 6 November 2026 (*"I agree"*): **an × appears on
hover over an open chat; closing moves it from *Open* to *Earlier*; nothing
is deleted.** § 5's *closing is not built* is replaced by this.

- **Which rows offer which.** The row the pointer is over offers one thing
  at its end, never two: **an open chat its ×**, 24 square, a 12 px cross
  in the ink on the line's colour, named *Close* and the chat's name; **an
  earlier chat *Delete*** (789). Closing loses nothing, so it is what a chat
  in use offers; deleting is offered only for a chat already put away, so
  deleting an open chat takes two deliberate presses — closed, then
  deleted — and the × and *Delete* never stand where a person reaching for
  one could press the other. The rail offers neither (760).
- **Closed, a chat moves to the top of *Earlier*** — the most recently
  put away is the newest of the earlier ones — its name and turns as they
  were, a live card put away as its address (and its page closed, as any
  card put away). Nothing is written for it but what its kept form already
  changed: whether a chat is open is not kept (§ 5 reads every chat back
  under *Earlier*), so a closed chat's file is the one it had.
- **A chat with nothing in it closed goes**: no name and no turn, nothing
  would be kept of it, and it has no file (§ 3 writes a chat once it holds
  a turn).
- **The chat on screen closed** puts the open chat below it in the panel on
  screen, or else the one above, or else a new chat. **The empty chat on
  screen offers no ×**: closed it would come straight back as *New chat*.
- **Pressed again, an earlier chat is open and first under *Open*** — the
  most recent, as a new chat is first — and so first on the rail (760).
  Until 760 a chat opened again kept its place among the open ones.
- **What an agent reads**: the × is a button named *Close* and the chat's
  name. A person closes; an agent's verbs never reach the panel (ADR 0074).

Checked by `conversation`'s tests (to the top of *Earlier*, nothing gone and
everything kept; the chat on screen closed, below, above or a new one; the
empty chat on screen not closable; an empty chat closed gone, a card on
screen put away; open again first under *Open*), `conversation_decide`'s
(the × only for the open chat painted offering it and still so),
`conversation_claim`'s and `the_chats_panel.rs` (the × 24 square at (196,
225) in the 38-row from 218, its cross at (202, 231), the row unmoved, no
*Delete*; a press and a hover on it claimed as the ×; reference
`panel-closing.png`; the 789 reference `panel-deleting.png` now hovers an
earlier row), the wire, and through the window's road
(`offers_and_the_panel_pressed_in_the_window.rs`): an open row hovered and
painted offering its ×, pressed, the chat under *Earlier* — where, hovered,
the panel's renderer finds it at 254 and its row offers *Delete*.
Mutations — a closed chat put last rather than first, the painted × not
kept by the drawing thread — each fail.

## Amendment, 2026-11-07 — a run's plan is kept with it (queue item 824)

Each run of alo's agent kept with its page (§ 820's amendment) keeps its
plan as it last stood, when it wrote one: `"plan": {"steps": […], "on":
n}` beside `"asked"`, `"steps"` and `"ended"` — its steps in the model's
words, each with every text typed that the person did not say replaced
by `…` as the steps' lines are (`agent_kept.rs`), `on` the place of the
step it was on. Sealed with the rest of the file (ADR 0098). Read back
checked as the wire checks it: one to five steps, none empty or past 80
characters, `on` one of them; a run kept before plans were reads as
having none. Drawn under the page's line once it is shown earlier, as the
live column drew it: done ticked, the step it stopped on bold, the rest
greyed; every step ticked when it finished. Nothing of a payment's details
(ADR 0099) is kept.

Checked by `chat_record`'s tests (a plan written and read back the same,
a plan of six steps not read), `agent_kept`'s (a plan's typed address
replaced by `…`), `conversation_work`'s (the plan kept as it is told, and
a new run beginning with none), `the_agents_plan.rs` (a kept run's plan
drawn as it stood) and through both Chromiums
(`the_agent_at_work_on_a_card_through_chrome.rs`: the run carried on after
taking over read back from the file with its plan of two steps).


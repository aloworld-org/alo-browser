# ADR 0114 — A second launch hands its address to the running alo, over its socket

**Status:** accepted — queue item 976, 11 November 2026. The behaviour is
ADR 0112's (*Answered* 4, the coordinator on the owner's decisions); what is
decided here is how this repository carries it out. Built, not yet gated.
**Date:** 2026-11-11
**Context:** ADR 0112 (requirement 8; *Answered* 4: *"A second launch on
the **same** profile hands its address to the running instance, which opens
it and comes forward; a launch with a **different** `--data DIR` — alo's
own copy — stays a separate instance"*); ADR 0055 (the agent socket, the
person's grant, enforced by the operating system); ADR 0002 (no verb takes a
coordinate); queue items 956 (`open` with no tab is a new chat) and 957
(`--data DIR`, the socket at `DIR/alo-browser/agent.sock`).

## Decision

1. **The running alo is found at the profile's agent socket.** One alo per
   profile holds it (`socket_place::prepare` already refuses to take a
   socket that answers). The profile is `--data DIR` — its socket
   `DIR/alo-browser/agent.sock` — or, with none, the person's own, the
   socket under their runtime directory. Another `--data DIR` looks at
   another socket and finds nobody: an instance of its own. No lock file,
   no D-Bus name, no second channel: the socket is already the one place an
   alo answers, and the operating system already keeps it to its person.
2. **Nobody answering is nobody there.** Nothing at the path, a socket an
   alo that was killed left, or any connection refused: the launch becomes
   the instance, and placing its own socket clears what was left (as
   `prepare` always has). Two launches at the same instant with nobody
   running both become instances; the second finds the socket taken and
   runs with none, said on its terminal, as before this ADR.
3. **One new ask, `launched`**, additive to version 1 of
   `docs/contracts/agent-socket.md`: `{"v":1,"id":1,"ask":"launched",
   "address":…}`. It is `open` with no tab — judged the same (an `http` or
   `https` address on a site agents may reach, `--agent-site` included),
   recorded the same, opened by the same road (*Open in new chat*) and
   answered the same (`opened` once the card has opened, or why not) — and
   **the window comes forward**: brought in front at once, shown again if
   minimised, and owed the front back from the card's Chromium as a
   person's own address owes it. Only the conversation offers it; `alo
   --chrome` and this engine's window answer `NotOffered`.
4. **Why a new ask and not `open`.** An agent's `open` must not take the
   front from the person: an agent works beside them. Coming forward is
   what a person who started alo again asked for, so it is a separate ask
   that says so. It is no grant: the socket is the person's already (ADR
   0055 § 1), and any process that can open it could already open pages
   there; `launched` adds only the window brought in front, and is held to
   every rule `open` is.
5. **What is handed is the address, and nothing else.** Only
   `alo --conversation ADDRESS` hands over (the conversation is the
   instance that opens chats). What only an instance uses — `--frozen-fonts`,
   `--scale`, `--agent-site`, `--agent-saves` — stays the running alo's;
   given to a second launch, the terminal says they were not handed over,
   rather than dropping them quietly. A hand run naming `--chats` or
   `--sign-ins` never hands over: those are a hand run's own places, not a
   profile, and it runs as before.
6. **The second process waits for the answer, then ends.** It says on its
   terminal which alo it handed the address to and what that alo answered
   (as `alo-ask --plain` writes it), and exits `0` when the page opened, `1`
   otherwise. It waits at most 90 seconds — a card's Chromium starting
   included — and then says it was not told and exits `1`.
7. **A launch early in the first alo's life waits.** The first alo places
   its socket before its window exists. A `launched` that arrives before the
   window has said it answers is held by the relay, and one that arrives
   before the window is on the screen is held by the window, until it is
   (a later one replacing it, answered `NotOpened`); an agent's `open` there
   is still refused as before.
8. **The first alo opens its own address the same way.** `alo
   --conversation ADDRESS` with nobody running opens the address in a new
   chat once its window is on the screen, through the same road, and nobody
   waits on an answer.

## Not decided here, and not built

- **A second launch with no address** on the same profile still opens a
  second window with no socket, as before. Coming forward without an
  address is the natural next step; it is not in item 976.
- **OpenURI on alo OS** (`org.freedesktop.portal.OpenURI`) calls the
  default browser's desktop entry, whose `Exec` line (item 963) will be
  `alo --conversation %u` or the application's equivalent; that the entry
  says so is 963's.
- **Windows**, where the socket is undecided (ADR 0055 § 1): no hand-over.

## Alternatives rejected

- **A lock file or PID file per profile.** A second record of what the
  socket already says, which goes stale when an alo is killed, and still
  needs a channel to hand the address over.
- **`open` with a field asking to come forward.** The same capability with
  a quieter name; an ask of its own says what it does.
- **Answering at once, before the card opens.** The person's terminal (or
  the program that launched alo) would be told it worked when the page then
  failed; `open`'s answer is the true one, and the wait is seconds.

## Consequences

- `docs/contracts/agent-socket.md` gains `launched` (additive).
- ADR 0112 requirement 8's row and *Built* name item 976.
- Tests: `launch_handover.rs` (nobody there, a socket left behind cleared,
  the line sent and the answer read, an alo that never answers),
  `conversation_agents.rs` (a launch judged as `open`, waiting for the
  window, handed to it to come forward), `conversation_window/outside.rs`
  (a launch opens a new chat and asks the front; an agent's open does not;
  one before the window is on the screen waits), `protocol.rs`,
  `serving.rs`, `opening.rs`; and
  `a_second_alo_hands_its_address_to_the_first_through_chrome.rs`, three
  `alo` processes.

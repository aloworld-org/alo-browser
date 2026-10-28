# ADR 0071 — A closed list's option through a shown Chrome is chosen in the menu, by keys posted to Chrome's process

**Status:** accepted
**Date:** 2026-10-28
**Context:** ADR 0064 § 6 (a closed list's option is refused through Chrome
until a shown Chrome's menu is measured); ADR 0002 (no verb takes a
coordinate); ADR 0062 § 3 (the window server is read through `osascript`,
never `unsafe` of ours); law 4; queue items 660 and 658

## The decision in one line

Through a **shown** Chrome on macOS, an option of a closed `<select>` is
chosen as a person chooses it from the keyboard: the list's menu is opened
by the click every `Activate` makes, the highlight is moved by Down or Up
arrows **posted to Chrome's own process**, and Return chooses — each key
sent only while the window server shows Chrome's menu, and the verb
answered `NotCarriedOut` the moment it does not. A headless Chrome, which
draws no menu, keeps ADR 0064 § 6's refusal.

## Why this is a decision rather than an implementation detail

ADR 0064 § 6 refused it on a measured ground — every key a *closed* list
hears chooses an option nobody asked for — and named what would lift it:
a person's way through a shown Chrome's menu, measured with the screen in
use. That has now been measured, and the way it works sends input through
the operating system rather than through Chrome's protocol, to a menu no
part of Chrome's tree describes. Which input, to whom, and what stops it
from reaching anything else are choices nobody has made yet.

## 1. What was measured

Chrome 156.0.8078.73, macOS 26.5.2, Stage Manager on, the screen in use
(`IOConsoleLocked` No, no `CGSSessionScreenIsLocked`). A shown Chrome in
`--app` mode on a page holding one closed `<select>`, a listener recording
`mousedown`, `mouseup`, `click`, `focus`, `blur`, `keydown`, `keyup`,
`input` and `change` with the list's `selectedIndex`, and a timer writing a
line every quarter second. Driven over CDP from a scratch probe, never
committed; windows read from the window server by Chrome's process id.

- **`Input.dispatchMouseEvent` at the list's middle opens the menu.**
  Chrome's process gains a window at **layer 101**, as wide as the list,
  beside its page window at layer 0; the screen shows macOS's own menu
  with a check at the chosen option.
- **The page is not held while the menu is open.** The timer's lines go on
  every 250 ms, and `Runtime.evaluate` is answered at once.
- **Keys sent with `Input.dispatchKeyEvent` never reach the menu.** The
  page's list hears them as `keydown` and `keyup`, nothing is chosen, and
  the menu stays open — CDP's input goes to the page's renderer, not to the
  application whose menu is tracking.
- **Keys posted to Chrome's process reach the menu.** `CGEventPostToPid`
  with Down, Down, Return, from a compiled helper and equally from
  JavaScript for Automation in `osascript`: the menu closes and the page
  hears **one `input` and one `change`**, for the option two down from the
  chosen one, then a `click`. Keys pressed through System Events, which go
  to whichever application is in front, did the same while Chrome was in
  front.
- **The arrows walk the menu, not the list.** On a list of *Apples*,
  *Bananas* `disabled`, *Cherries* `hidden` and an `<optgroup>` *Late* of
  *Dates* and *Elderberries*, two Downs from *Apples* chose
  *Elderberries*: the menu skips a disabled option, has no item for a
  hidden one, and its group's label is not an item. An arrow alone chooses
  nothing; **only Return chooses.**
- **Another application in front closes the menu**, and the page hears
  `blur`. Keys posted to Chrome's process then reach the focused closed
  list: the first Down opened the menu again without choosing, the second
  moved its highlight from the chosen option, and Return chose
  ***Bananas*, the wrong option**, with one `input` and one `change`.
- **Not read:** which item the open menu highlights. System Events listed
  no menu of Chrome's process; whether the Accessibility API reads it
  another way is not measured.

## 2. What is decided

1. **Only a shown Chrome.** Headless, nothing is drawn and nothing is
   posted: ADR 0064 § 6's refusal stands there, by name, as today.
2. **Opened by the click every `Activate` makes**, at the list's middle,
   aimed and checked as every press is (item 621). The list is the
   target; no coordinate is taken from the agent (ADR 0002).
3. **The menu is witnessed by the window server**: a window of Chrome's
   process at layer 101 that was not there before the click. No key is
   posted unless it is shown, and it is read again **before every key**.
   If it is not shown — before the first arrow, between two, or before
   Return — no further key is posted, the verb answers `NotCarriedOut`
   with what happened, and an Escape is posted only if the menu is still
   shown.
4. **Keys are posted to Chrome's process and to nothing else**, through
   `CGEventPostToPid` called from `osascript` as the window server is
   read (ADR 0062 § 3), with the process id as the script's argument.
   Never System Events' keystrokes, which go to whichever application a
   person has in front.
5. **The count of arrows is the menu's**, from the option the list holds
   to the option asked, over the options the menu has items for and can
   highlight: not `hidden`, not `disabled` and not in a disabled group, in
   tree order. Down or Up by that count, then Return. An option the list
   already holds is answered `Chosen` with no key posted, as on this
   engine's page.
6. **The answer is read back from Chrome's tree** after Return, as for a
   drawn list (item 660): `Chosen` only when Chrome's tree says the asked
   option is selected. A page that heard a `change` for another option is
   answered with the option it holds, and the record says the verb was
   carried out — it was.

## 3. Why not the alternatives

- **Keep the refusal.** The most common list on the web would stay
  unchoosable on the product path, and ADR 0064's own *How we will know*
  said the route, not the verb, is reopened once a shown Chrome's menu can
  be chosen in without passing through other options. It can: arrows move
  only a highlight.
- **CDP's keys.** Measured: they never reach the menu.
- **A click in the menu.** It needs a screen coordinate of a menu no tree
  describes, moves the person's pointer, and is input to whatever window
  is under it if the menu has closed.
- **System Events keystrokes.** They go to the application in front, so a
  person switching applications mid-verb would receive the agent's
  Return.
- **Typing an option's first letters into the menu.** Copies AppKit's
  collation, which ADR 0064 already declined to copy, and chooses nothing
  without Return anyway.
- **`objc2-core-graphics` for `CGEventPostToPid`.** Its event creation and
  posting are reachable safely, but anything near the window list is not
  (ADR 0062 § 3), and `osascript` already carries both readings and costs
  no `unsafe`.

## 4. What remains a risk, named

- **A race the guard narrows and does not close.** Between the last
  window-server reading and the key, a person may bring another
  application in front; measured, a key then reaches the closed list and
  can choose an option nobody asked for. One reading per key keeps the
  window to about one `osascript` call (ADR 0062 § 3 measured 80 ms). The
  answer and the record then say what Chrome's tree holds.
- **A person's own keys or pointer in the menu** while the agent acts move
  the same highlight. A person in front of the agent's tab is acting in
  it; the answer says what the list holds.
- **The menu's highlight is assumed to start at the chosen option**, as
  measured on a list with its first option chosen; the build measures it
  with another chosen and with none, and with the pointer resting where
  the menu opens; a disabled `<optgroup>`'s options are taken as skipped
  like a disabled option's, which is measured then too.
- **Permission.** `CGEventPostToPid` worked here from a terminal that
  macOS may already trust; whether a fresh install needs the person's
  permission to post events, and what is answered without it, is the
  build's to measure and say.
- **Off macOS** this is not measured, and the refusal stands there until
  it is.

## Consequences

- `alo-window`'s `chrome_choose.rs`: a closed list on a shown Chrome takes
  the route in § 2; headless keeps `Unacted::Closed`. Keys and the menu's
  witness go in a file of their own beside `window_server.rs`.
- A test against a shown Chrome: an option of a closed list several down,
  one with a disabled and a hidden option before it, and one up, each
  heard by the page as one `input` and one `change` for the option asked;
  and the menu closed by another window brought in front before Return,
  answered `NotCarriedOut` with no `change`.
- ADR 0064 § 6 is amended by this: its refusal is headless only.
- Queue item 658 is built against this decision; it closes when that test
  passes, as the item says.

## How we will know if this was wrong

- **If the agent's Return is ever heard by an application other than
  Chrome**, posting to a process is not the bound it was taken for.
- **If an answer says another option was chosen more than rarely**, the
  race is not narrow in practice and the route is reopened.
- **If macOS asks a person's permission** in a way the browser cannot
  explain at the moment it is asked, the route costs more than it gives.

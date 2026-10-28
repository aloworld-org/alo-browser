# ADR 0072 — A closed list's menu is not opened by a browser macOS does not let post keys

**Status:** accepted — built (queue item 692, iteration 436)
**Date:** 2026-10-28
**Context:** ADR 0071 (a closed list's option through a shown Chrome is
chosen in the menu by keys posted to Chrome's process) and its § 4
*Permission* risk; ADR 0062 § 3 (the window server through `osascript`,
never `unsafe` of ours); ADR 0067 and 0068 (the application on macOS);
item 679 (a refusal drawn in the frame); queue item 692

## The decision in one line

Before the click that opens a closed list's menu, and again before every
key, the browser asks macOS whether its process **may post events** —
a question that asks the person nothing — and when it may not, it opens
no menu and posts no key, and the verb answers `NotCarriedOut` with a
sentence saying where a person allows it. The browser never puts
macOS's permission prompt in front of a person in the middle of an
agent's verb.

## Why this is a decision rather than an implementation detail

ADR 0071 § 4 named it as a risk and left it to the build: every key it
measured was posted by a process Accessibility already trusted. What a
fresh install does is now measured, and the answer involves a setting a
person has to change, which queue item 692 says is an ADR's to decide.

## 1. What was measured

Chrome 156.0.8078.73, macOS 26.5.2, Stage Manager on, the screen in use
(`IOConsoleLocked` false). A scratch probe, never committed: a compiled
helper that runs `osascript -l JavaScript` with a script posting keys by
`CGEventPostToPid` — the shape of `chrome_menu`'s posting — run two ways:
from the terminal this repository's tests run in, and as the executable
of a **new application bundle**, sealed `codesign -s -` as `alo-bundle`
seals `alo browser.app`, under an identifier macOS had never seen, and
started by `open`. The target was a shown `--app` Chrome whose closed
list's menu had been opened by CDP's click, as `Activate` opens it, the
page writing down `focus`, `keydown`, `input` and `change`.

- **A fresh application is untrusted, and so is the `osascript` it
  starts.** In the helper `AXIsProcessTrusted`,
  `CGPreflightPostEventAccess` and `CGPreflightListenEventAccess` all
  read false; in the script, `AXIsProcessTrusted` read false. From the
  terminal all of them read true. macOS judges the child by the
  application that started it.
- **An untrusted process's keys are dropped, silently.** Down, Down and
  Return posted to Chrome's process: the menu stayed shown, the page
  heard nothing beyond the click's `focus`, the list still held
  *Apples*. Escape alone, the same: the menu stayed shown.
  `CGEventPostToPid` returns nothing, so the script printed `posted`
  each time. From the terminal the same keys chose *Cherries*, the page
  hearing one `input` and one `change`.
- **Nothing was asked of the person.** No window of any process was
  shown that had not been before, in three runs; posting a key does not
  prompt.
- **The window server and the pointer are read all the same** by the
  untrusted process: the menu's window, its layer and its number, and
  where the pointer is.
- **The question can be asked from `osascript`.** `CGPreflightPostEventAccess`
  is not in the scripting bridge's own list, but `ObjC.bindFunction`
  binds it, and it reads `true` from the terminal and `false` from the
  fresh application, asking nothing.
- **Not measured:** `CGRequestPostEventAccess`, which shows macOS's
  prompt — measuring it would put a prompt in front of the owner and an
  entry into their settings — and whether a rebuilt, re-sealed bundle
  keeps a grant its earlier build was given.

So today, on a fresh install, an agent's choice in a closed list opens
the menu, posts keys nobody receives, waits out Return, posts an Escape
nobody receives either, and answers `NotCarriedOut` (*its menu was still
shown after Return*) — **with the menu left open on the person's screen**
and no word of why.

## 2. What is decided

1. **Asked before the click.** The browser reads
   `CGPreflightPostEventAccess` through `osascript`, as the window server
   is read (ADR 0062 § 3). When it reads false no click is made, so no
   menu is opened, and no key is posted; the verb answers
   `NotCarriedOut`, and the sentence says that macOS does not let this
   browser post keys to Chrome, so the list's menu was not opened, and
   that a person allows it in System Settings, under Privacy & Security,
   Accessibility, for the application the browser runs in. The frame
   draws that sentence as it draws every refusal (item 679), so the
   person reads it where the agent does.
2. **Asked again before every key**, in the same script that reads the
   menu's window and the pointer (ADR 0071's amendment): a permission
   withdrawn mid-walk posts nothing further and answers the same way.
   Its sentence says the menu may be left open, since the Escape that
   would close it cannot be posted either.
3. **Never prompted for by a verb.** The browser does not call
   `CGRequestPostEventAccess` while acting for an agent. A prompt in the
   middle of a verb is a question the person did not start, about a
   process they cannot see, at a moment chosen by an agent — ADR 0071's
   own *How we will know* names exactly that as the cost that would make
   the route not worth it. Whether the application asks at a moment a
   person chooses is not decided here.
4. **Nothing else changes.** A trusted process walks the menu as ADR
   0071 says; a headless Chrome keeps ADR 0064 § 6's refusal; off macOS
   the refusal stands. No other verb posts keys to a process, so no
   other verb is asked about.

## 3. Why not the alternatives

- **Leave it as it is.** It answers `NotCarriedOut` already, but only
  after a menu is opened that the browser cannot close, and the sentence
  names a symptom, not the cause or the remedy.
- **Prompt at the verb.** § 2.3.
- **Prompt at start.** Every person would be asked for Accessibility on
  first launch, for one verb on one kind of list on one renderer; that is
  asking for the most a person can grant in exchange for the least.
- **Read `AXIsProcessTrusted` instead.** It is in the scripting bridge
  without binding, and read the same as the preflight here. But it
  answers whether the process may use the Accessibility API, and the
  question this route depends on is whether it may post events, which
  macOS keeps as a service of its own; asking the question we mean costs
  one binding line.
- **Read it from Rust.** `objc2-core-graphics` would call it directly,
  but `osascript` already carries the window server's reading and the
  key, at no `unsafe` (ADR 0062 § 3), and the answer must be read in the
  same script as the key for § 2.2.

## 4. What remains a risk, named

- **A grant may not survive a rebuild.** `alo-bundle` seals ad hoc, and
  whether macOS ties the grant to the identifier or to the build is not
  measured. A person who allowed it may be refused after an update; the
  sentence then tells them again.
- **The application named is the one macOS judges**: `alo browser.app`
  when opened from the Finder, the terminal when `alo --chrome` is run
  from one. The sentence says "the application the browser runs in"
  because the browser does not read which that is.
- **The preflight answers for this moment.** A grant given between the
  reading and the key is not seen until the next verb; one withdrawn is
  § 2.2.

## Consequences

- `alo-window`'s `chrome_menu.rs`: a reading of whether keys may be
  posted, and the posting script reading it before each key and
  printing that it may not, read strictly as its other answers are.
- `chrome_choose.rs`: the reading before the click, and a new
  `Unwalked` for a key not allowed, both answered `NotCarriedOut`.
- Tests: a unit test of the script's new answer read strictly; and,
  since the tests' own process is trusted, an integration test run as a
  fresh application bundle that asserts no menu is shown, nothing is
  heard by the page and the sentence names the setting — the build
  decides how such a bundle is made inside a test, or says why it
  cannot be and how else it is asserted.
- ADR 0071 carries an amendment line pointing here.
- Queue item 692 is built against this decision and closes when those
  tests pass.

## How we will know if this was wrong

- **If a fresh install's keys ever reach Chrome** with the preflight
  false, the question asked is not the one macOS enforces.
- **If people allow it and are refused anyway**, § 4's first risk is the
  common case and the application has to ask, or say more, at a moment
  of the person's choosing.

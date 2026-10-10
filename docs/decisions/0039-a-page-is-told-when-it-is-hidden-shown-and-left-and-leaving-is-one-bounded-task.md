# ADR 0039 — A page is told when it is hidden, shown and left, and leaving is one bounded task

**Status:** accepted
**Date:** 2026-10-10
**Context:** queue item 364, *the page lifecycle at its window*, opened by a
page: `alo-sites-cta`, the call-to-action section alo Sites publishes for its
customers. Its analytics script sends what it measured from two listeners and
nowhere else:

```js
document.addEventListener("visibilitychange", function () {
  if (document.visibilityState === "hidden") { record(); } else { since = Date.now(); }
});
window.addEventListener("pagehide", record);
```

Since items 362, 365, 366 and 370, `record` runs to its end when a test
dispatches `pagehide` itself, and reports `d=1000&p=%2F&w=800` and `t=0`
given a beacon the test lends. Nothing in the browser ever dispatches either
event. `document.visibilityState` is `undefined`, so the first listener's
test is false every time. The item is marked **needs design**: *when the
browser process tells a renderer its page is hidden, shown or being left, and
how long the page's listeners are given before the renderer goes.*

Also read:
- ADR 0037 § 5, which made the window something the browser can dispatch at
  and left the lifecycle to this item;
- ADR 0005 (the browser process sends work, a renderer returns results, and
  nothing calls back) and ADR 0024 §§ 2 and 6 (the conductor; what a tab is,
  and that closing the window closes every tab);
- ADR 0016 §§ 2, 6 and 7 (one task queue; the window of an answer; a stopped
  task is a stopped page);
- ADR 0018 § 4 (a page cannot single out an agent) and ADR 0038 § 6 (a tab
  no window shows is told the first window's size);
- ADR 0020 (a page asks to go somewhere; the browser process decides) and
  ADR 0032 (a fetch is an ask), whose *What this does not decide* leaves
  keep-alive fetches and `sendBeacon` open;
- ADR 0012 (every request says what caused it).

And the code this is about:
- `alo-renderer`'s `renderer.rs`: `load` begins with `self.held = None`.
  A page replaced by the next `Load` into the same renderer is dropped
  without a word to its script.
- `tab.rs`: `Tabs::close` reaps a site's renderer when no tab is open on it.
  The process is stopped, and its page with it.
- `alo-window`'s `conductor.rs`: `close_everything` closes every tab that
  way when the window closes. `window.rs` handles `CloseRequested`,
  `Resized` and `RedrawRequested` and ignores the rest, `Occluded` among
  them.
- `event_loop.rs`: tasks, the checkpoint after each piece of script, and
  `Stop`, an `Arc<AtomicBool>` any thread may set.
- `answers.rs`: `LONGEST_SILENCE`, ten seconds, the most the browser process
  waits for any answer.

## The decision in one line

A page has a **visibility state**, `visible` or `hidden`, that the browser
process sets: it is sent with the page and changed by a message of its own,
and a change is **one task** that fires `visibilitychange` at the document.
A page is **left** only through its leaving steps — `pagehide`, then the
visibility becoming `hidden`, then `unload` — run as **one task with a
deadline of one second**, after which nothing the page queued runs. That
happens whether a `Load` replaces it, its tab is closed or the window
closes. There is **no back-forward cache**, so `persisted` is always
`false`. `beforeunload` is not fired, because its prompt is a decision about
a person's attention that nothing has made.

## Why this is a decision rather than a chore

`visibilitychange` and `pagehide` are events like any other, and dispatching
an event at the document or the window is built. What is not decided is
everything around them, and each part is visible to a page.

**Who knows.** Whether a tab is in front, whether its window is minimised and
whether a person closed it are facts the browser process has and a renderer
does not. ADR 0005 lets the browser process tell a renderer things. It does
not say which of these it tells, or when.

**What a renderer may do with the time.** A page being left is a page whose
person has gone. Every script that listens for `pagehide` wants to do one
more thing, and some of them want to keep doing things. If leaving waits for
a page, a page can hold a closed tab's process alive. If it does not wait,
the page's last listener runs into a process that has already stopped.

**What a page can tell from it.** A page that sees `hidden` while it is
being clicked has found something out about who is clicking. A page that is
always `visible` has been told something false.

## 1. The visibility state is the browser process's, and the page is told it

A document's visibility state is **`visible` or `hidden`**. HTML's third
value, `prerender`, belongs to prerendering, which does not exist here.

**The browser process decides it**, because only it knows what the person
can see. A tab is `visible` when it is the selected tab of a window that is
not minimised or covered. Otherwise it is `hidden`:
- **Its window is occluded or minimised.** `winit` reports both as
  `WindowEvent::Occluded(true)` on macOS, and `Occluded(false)` when the
  window shows again. `alo-window` hears that event and tells the selected
  tab.
- **It is not the selected tab** (item 297). Selecting a tab tells the one
  left `hidden` before it tells the one chosen `visible`, so no instant has
  two visible tabs in one window.
- **A tab no window shows is `visible`.** ADR 0038 § 6 tells such a tab the
  size a window opens at, so that its size does not mark every agent's
  session. Its visibility follows the same rule. It is also true: the tab is
  being read by whoever drives it. A page that pauses its work while hidden,
  as many do, would otherwise stop working for exactly the agent reading it.

**The renderer is told, in two ways.**
- **With the page.** `Page` gains the visibility state the page starts in,
  as it carries the viewport it starts at. A script that reads
  `document.visibilityState` while the page loads reads the truth. A page
  loaded into a tab opened behind the selected one starts `hidden`.
- **By a message of its own**, `ToRenderer::Visibility`, carrying the new
  state. It is a **task** on the page's event loop (ADR 0016 § 2), queued in
  order with everything else.

**The task is HTML's *update the visibility state*.** If the state is the
one the document already has, nothing happens and nothing is fired.
Otherwise the document's state is set, and then `visibilitychange` is fired
at the document, trusted, bubbling and not cancellable. Its path ends at the
window, as every path from the document does (ADR 0037 § 3).

**It is answered as a delivered response's task is**, with what the page's
script said, what its policies objected to, and the fetches, sheets and
navigation it asked for. Those are the **document's**: the person did not do
anything to the page by switching tabs, and no agent's verb was answered
(ADR 0016 § 6). A hidden page may still fetch. HTML does not forbid it, and
the record says whose it was.

**What a page reads**: `document.visibilityState`, the state as a string,
and `document.hidden`, which is `true` exactly when it is `"hidden"`. Both
are read-only accessors on `Document.prototype`, read from the document
cell at each read. A document no window was associated with answers
`"hidden"`, and is never changed. HTML gives every document the initial
state `"hidden"`, and only a browsing context's visibility ever updates it.
A document a script makes has no browsing context.

## 2. A page is left through its leaving steps, and only through them

HTML's *unload a document*, for a page that was shown:
1. **`pagehide`** is fired at the window, a `PageTransitionEvent` whose
   `persisted` is `false`, trusted and not bubbling;
2. the visibility state is updated to **`hidden`**, by § 1's steps: so
   `visibilitychange` is fired at the document if it was `visible`, and
   not if it was already `hidden`;
3. **`unload`** is fired at the window, trusted.

These steps run as **one task**, with the microtask checkpoint after each
piece of script as in every task. Then the document is discarded: every task
still waiting is dropped and its roots released, the job queue is emptied,
and no timer, delivered response or job the page queued ever runs. That is
ADR 0016 § 7's stopped page, reached on purpose.

***Page showing* is set when the load finishes.** HTML fires `pagehide`
only for a document whose *page showing* flag is true, and sets it when it
fires `pageshow`. So the same item fires **`pageshow`**, a
`PageTransitionEvent` with `persisted` `false`, at the window as the last
step of the load, after the page's own scripts have run and before `Loaded`
is answered. When the `load` event is built, it is fired immediately before
`pageshow`, in the same step. If `load` is later made to wait for a page's
linked sheets (item 351), `pageshow` waits with it, because HTML fires both
from one step.

**Every way a page stops being held goes through these steps.** There are
three today:
- **A `Load` into a renderer holding a page.** The renderer leaves the page
  it holds before it parses the next one. `renderer.rs`'s `self.held =
  None` becomes the leaving steps. This covers a tab loading its next page
  on the same site, and the second tab on a site displacing the first in
  their shared renderer (`tab.rs`' *one document per process*). The
  displaced page is gone from the renderer, so it is left. Its tab still
  shows its last frame.
- **A tab being closed.** `Tabs::close` sends `ToRenderer::Leave` to the
  tab's renderer, if it holds that tab's page, before it reaps.
- **The window closing.** `close_everything` closes every tab, and so leaves
  every page.

A tab whose next page is on another site, which item 85 makes possible,
sends `Leave` to the renderer it is leaving. The general rule is that the
browser process sends `Leave` whenever it stops showing a page other than by
sending that renderer a `Load`.

**What is not a leave.** A renderer that crashes, or that the browser process
stops for silence, runs nothing. That is ADR 0005's dead renderer, and no
page can be promised a last word by a process that has died.

## 3. Leaving is given one second

**The renderer arms its own stop for the leaving task**: `Stop` is set by a
deadline `LONGEST_LEAVING`, one second after the task begins, and cleared
when the task ends. A page whose leaving listeners are still running at the
deadline is stopped, as any stopped page is (ADR 0016 § 7). It was being
discarded anyway. The answer says it was stopped, so the record shows a page
that tried to stay.

**Why a bound, and why this one.** An honest `pagehide` listener does what
`alo-sites-cta`'s does: a little arithmetic, a string, one ask. That is
microseconds of script. A listener that runs for seconds is either broken or
trying to keep a process the person closed alive, and while it runs, the
process holds memory and, for a same-site `Load`, the next page waits. One
second is far beyond the first case and short enough that the second does not
cost a person anything they would notice. It is a policy number, not a speed
claim. It changes when a frozen page's honest listener is found to need more,
and that page is the evidence.

**The browser process does not wait on the page.** It waits for the answer
to `Leave` as it waits for any answer, at most `LONGEST_SILENCE`, and then
stops the process as `host.rs` already does for silence. The window waits on
none of it (ADR 0024 § 2): a closed tab leaves the strip and the window at
once, and the conductor finishes leaving its page behind it.

**The answer**, `FromRenderer::Left`, carries what the page's script said
and the fetches it asked for, as claims. **No navigation is carried.** HTML
ignores a navigation whose source document is being unloaded, and a page
being left has no tab to send anywhere. A `Load`'s leaving steps put what
they said at the front of `Loaded`'s issues, marked as said by the page that
was left, so that the new page is not blamed for it.

## 4. A leaving page's fetches are refused until keep-alive is decided

A fetch asked for during the leaving task is a request whose response has no
document to go to. Making it is exactly the *keep-alive fetch that outlives
its document* that ADR 0032 left undecided, and `navigator.sendBeacon` is
its other name (item 369).

So **the browser process refuses each one, by name**, as it would refuse any
other ask a rule forbids: the ask crosses as a claim, the decision is the
browser process's, and the record says what was refused and why. Item 369
changes the decision, in `fetch_decide.rs`, and nothing in the renderer. If
the renderer dropped the asks itself, item 369 would have to change both
processes, and a compromised renderer's asks would never be seen.

## 5. No back-forward cache, and no `beforeunload`

**`persisted` is always `false`.** A back-forward cache keeps a left page
alive, frozen, so that going back shows it as it was. Nothing keeps a page
after it is left here. Going back is item 85's, and whether it restores a
frozen page is a decision about memory and about what a page may do while
nobody can see it. Until that is made, a page left is a page gone. The
`freeze` and `resume` events, which exist for such a cache, are not fired.

**`beforeunload` is not fired.** Its point is the prompt: *leave this page?
Changes you made may not be saved.* Whether a page may put that question to
a person, and when, is a decision about the person's attention, as ADR 0024
§ 6 says of a popup. Firing the event with no prompt that could ever follow
would make its `preventDefault` and `returnValue` lie about what they do. It
stays unfired until a page needs it and that decision is made.

## 6. What an agent's tab looks like

ADR 0018 § 4 says a page cannot tell an agent's verb from a person's. § 1
keeps that for a tab no window shows, which is `visible` as a shown tab is.

It does not keep it for an agent acting in a **hidden tab of a person's
window**, which a person could not be clicking. A page that sees a trusted
`click` while `hidden` has learned that something other than the person at
the screen clicked. Making the tab `visible` for the length of the verb
would be telling the page a state no person saw. Whether an agent may act in
a tab the person is not looking at, and on what grant, is item 133's
(*under grants, and recorded*), and this ADR names the question for it rather
than answering it with a lie.

## 7. The cut

- **364. The visibility state.** *Depends on 362 (done).* Builds:
  - `Page`'s starting visibility state and `ToRenderer::Visibility`, its
    task and its answer, across the wire;
  - `document.visibilityState` and `document.hidden` in `alo-bindings`, kept
    in the document cell;
  - `visibilitychange` fired at the document by § 1's steps;
  - `alo-window` telling the selected tab on `Occluded`.

  *Closes when:*
  - a page loaded `visible` reads `"visible"` and `false`, and one loaded
    `hidden` reads `"hidden"` and `true`, while it loads;
  - `Visibility(Hidden)` fires one `visibilitychange`, at the document and
    then the window, and `Visibility(Hidden)` again fires none;
  - in `tests/alo_sites_cta.rs`, with a beacon the test lends,
    `Visibility(Hidden)` makes the page send `d=1000&p=%2F&w=800` and then
    `t=0`, and `Visibility(Visible)` afterwards sends nothing;
  - a fetch asked during the task is the document's;
  - `alo-window` sends `Hidden` and `Visible` when its window is occluded
    and shown, in a conductor test;
  - every script runs ordinarily and under `Heap::stress`.
- **373. A page left.** *Depends on 364.* Builds:
  - `PageTransitionEvent` with `persisted`, and *page showing*;
  - `pageshow` at the end of the load;
  - the leaving steps of § 2 as one task, run by `Leave` and before every
    `Load` into a renderer that holds a page;
  - § 3's deadline and `FromRenderer::Left`;
  - `Tabs::close` and `close_everything` sending `Leave`;
  - § 4's refusal of a leaving page's fetches, in `fetch_decide.rs`.

  *Closes when:*
  - in `tests/alo_sites_cta.rs`, with a beacon the test lends, `Leave` of a
    visible page makes it send `d=1000&p=%2F&w=800` and then `t=0`, from
    `pagehide`, and its `visibilitychange` listener sends nothing more;
  - the events arrive in the order `pagehide`, `visibilitychange`, `unload`,
    and a page already hidden hears no `visibilitychange`;
  - a page that loops in `pagehide` is stopped at the deadline and the
    answer says so;
  - a timer, a delivered response or a job queued during leaving never runs;
  - a `Load` after a page that listens for `pagehide` runs the listener
    first, and says so in `Loaded`;
  - a leaving page's fetch is refused by name, and its navigation is not
    carried;
  - every script runs ordinarily and under `Heap::stress`.
- **369**, `navigator.sendBeacon`, now depends on 373 rather than on 364.

## What this costs

- **Two more messages and one more answer on the wire**, and a field on
  `Page`. Each is coarse: one per change a person makes, not per frame.
- **A deadline thread, or its equivalent, in every renderer that leaves a
  page that runs script.** It is the only way a stop can be asked while the
  renderer's one thread is inside a page's listener.
- **A closed tab's process lives up to a second longer**, or up to
  `LONGEST_SILENCE` if it stops answering. The person does not wait for it.
- **A `Load` on the same site waits for the last page's leaving**, up to a
  second. Every browser runs the old page's `pagehide` before showing the
  next one.
- **Analytics on a page being left are refused** until item 369 decides
  keep-alive. A page's last report is lost, and the record says so.

## Alternatives rejected

- **Not telling the page anything, as today.** Every page that saves state
  or reports on `pagehide` or `visibilitychange` silently loses it. A silent
  absence of an event is the hardest kind of failure for a page's author to
  find.
- **`hidden` for a tab no window shows.** True in a narrow sense, and it
  marks every agent's session and stops every page that pauses while hidden.
- **Leaving with no bound.** A page could keep a closed tab's process alive
  for as long as its listener ran, and the next same-site page would wait for
  it.
- **The browser process running the deadline**, by stopping the process
  when it passes. It cannot stop one listener without stopping the renderer,
  and for a same-site `Load` the renderer it would stop is the one about to
  hold the next page.
- **The renderer dropping a leaving page's fetches itself.** § 4: the
  decision would then live in two processes, and the record would never see
  what a page tried to send.
- **Firing `beforeunload` without a prompt.** § 5: its only purpose is a
  question to a person, and an event that pretends to offer one is the
  approximate member ADR 0013 § 3 refuses.
- **Not firing `unload`.** It is deprecated in practice because it keeps a
  page out of a back-forward cache, which does not exist here. HTML still
  fires it in the same steps as `pagehide`, at no extra cost, and a page
  that relies on it would otherwise lose its last word without any sign.

## Correction (iteration 242)

§ 2 says `pagehide` is fired *trusted and not bubbling*. That was a slip
about what HTML says, not a decision: HTML's *fire a page transition event*
initialises `bubbles` and `cancelable` to `true` — "for historical
reasons", since at the window neither means anything — and sets the
*legacy target override flag*, so a listener reads the document as the
event's `target`. Queue item 373 fires `pageshow` and `pagehide` with
HTML's flags. `unload` is an ordinary `Event`, neither bubbling nor
cancelable. The target override is queue item 374. Nothing else in this
ADR changes.

## What this does not decide

- **Going back and forward**, session history, and whether a left page is
  ever kept (item 85).
- **Keep-alive fetches and `sendBeacon`** (item 369), which § 4 refuses
  until then.
- **The `load` and `DOMContentLoaded` events**, and when `load` waits for
  sheets and pictures (item 351). § 2 says only that `pageshow` goes with
  `load`.
- **Event handler properties**: `onpagehide`, `onvisibilitychange` and the
  rest are item 259's.
- **What a hidden page is not given**: whether its timers are slowed, its
  frames not drawn, or its renderer given a lower priority. Each is a
  measured policy, and none changes what a page reads.
- **An agent acting in a hidden tab** (item 133, § 6).
- **`beforeunload` and its prompt**, until a page needs it (§ 5).

## How we will know if this was wrong

**If a frozen page's `pagehide`, `visibilitychange` or `unload` listener
runs a different number of times, or in a different order, than another
engine runs it** for the same sequence of hiding, showing and leaving, § 1
or § 2 is wrong. The fix is in the renderer's one leaving task.

**If a frozen page's honest listener needs more than a second**, § 3's
number was too small. The page is the evidence, and the number changes with
it.

**If a page is found that changes what it does for agents by reading
`visibilityState`** in a tab no window shows, § 1's reasoning was wrong
about which answer singles out an agent, and § 6's question comes forward.

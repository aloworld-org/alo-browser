# A ninety-second demonstration, for developers

A script for recording alo for developers: a terminal beside alo's window, a
page driven by name from one line, the tree that line read, and a task given
to alo's own agent in the conversation, with its numbers. Every line to type
is here exactly. What was rehearsed, and how often, is in the journal
(`docs/autonomy/STATE.md`, queue item 915).

macOS only. Beats 1 and 2 need no AI key; beat 3 needs the application's
keys, so record it with the application made by
`alo-bundle --keys-from keychain`.

## Before recording

- Open alo from a terminal, its lines timed into a file for beat 4:

  ```sh
  "alo browser.app/Contents/MacOS/alo" --conversation 2>&1 | perl -MTime::HiRes=time -ne '$|=1; printf "%.3f %s", time, $_' > alo.log
  ```

  `alo.log` says `alo: an agent reaches this browser at
  …/alo-browser/agent.sock`.
- In a second terminal, put `alo-ask` within reach — the application holds
  it beside `alo`:

  ```sh
  export PATH="/Applications/alo browser.app/Contents/MacOS:$PATH"
  ```

  (or wherever the application is).
- Close any other alo: two cannot hold the same socket, and the second says
  so and opens none.
- Press alo's box before typing into it, each time.

## The pages

| Beat | Page | Why |
|---|---|---|
| 1, 2 | **Toolshop**, `https://practicesoftwaretesting.com` — a product page, e.g. *Combination Pliers* | A practice shop built for testing tools against: no sign-in to add to the cart, a button named **Add to cart**, and the cart's count in the header changes where the viewer sees it. It is a demonstration site by its own words, so nothing is bought |
| 1, 2, fallback | **The Internet**, `https://the-internet.herokuapp.com/add_remove_elements/` | A practice page stable for years: **Add Element** adds a **Delete** button under it |
| 3 | **A failing run of Gitea's**, `https://github.com/go-gitea/gitea/actions/runs/42792539551` | Public, no sign-in. Its *Annotations* name the error and where it is: `lint-backend: modules/web/middleware/flash.go#L72`, gocritic's `wrapperFunc` |

**GitHub shows a run's log only to someone signed in.** Signed out, a job
says *Sign in to view logs*; what a run shows everybody is its
**annotations** — the errors its tools reported, each with its file and
line. So the task asks for *the error in this run*, which the annotations
hold, and not for the log. alo never signs in, and its agent hands a
sign-in page to the person.

A run's page and its annotations are kept by GitHub for 90 days unless its
owner deletes them sooner (this run is from 10 November 2026). Before
recording, open it once; if it is gone, any public run whose *Annotations*
name a file and a line will do (a Go project linted by `golangci-lint`, or
a Python one tested by `pytest` with annotations, usually has them).

## Beat 1 — the hook (about 20 s)

In alo's box: `practicesoftwaretesting.com`, Return; press a product, such as
*Combination Pliers*. Reach it from the shop's first page each time: Toolshop
gives its products new addresses when it resets itself (seen within an hour
on 10 November 2026), so a saved product address stops working. Then, in the
terminal:

```sh
TAB=$(echo '{"v":1,"ask":"tabs"}' | alo-ask --plain)
echo '{"v":1,"ask":"act","tab":"'$TAB'","target":{"name":"Add to cart"},"verb":"activate"}' | alo-ask --plain
```

It prints `activated box#… "Add to cart"`, and the cart's count changes in
the card. Say: *no selector, no screenshot, no coordinates — a button, by its
name.*

Fallback page: `the-internet.herokuapp.com/add_remove_elements/` and
`"target":{"name":"Add Element"}`; a *Delete* button appears.

## Beat 2 — what the agent sees (about 15 s)

```sh
echo '{"v":1,"ask":"read","tab":"'$TAB'"}' | alo-ask --plain
```

One thing a line: its number, its role, its name and its states, indented
under what holds it — `heading "Combination Pliers" level=1`,
`spinbutton "Quantity" takes_text`, `button "Add to cart"`; what the card
does not show now is marked `offscreen`. Without `--plain` it is the JSON a
program reads.

## Beat 3 — a developer's task, in the conversation (about 40 s)

In alo's box: the run's address, Return. When the run's page is up in the
card, in the box:

    Find the error in this failing run and open that file at that line.

Return asks alo (with a page open as a card, Return on words is *Ask alo
about …*). alo writes a short plan, reads the run's annotations, opens
`flash.go` at line 72 in the card, and says what the error is.

To begin with no page instead, type the whole task with the run's address in
it and press **Ask alo to do it** under the box (Return there searches).

## Beat 4 — the numbers (about 15 s)

From alo's terminal, for that one task:

- **Model calls**: the lines `agent: … chose a step (… tokens in, … out)`.
- **Steps**: the lines `agent: …` that say what it does, and `agent opens …`.
- **Time per step**: between one `chose a step` line and the next, by the
  times `alo.log` puts before each line. Count afterwards rather than showing
  alo's own lines on screen — they name the AI service that chose each step:

  ```sh
  grep -c 'chose a step' alo.log
  ```
- **Screenshots: 0.** alo's agent is sent the page as text — the tree — and
  never a picture (`agent_protocol.rs`, ADR 0002).

Show the numbers measured in that take. Do not compare them with another
browser's unless that browser was measured the same way, on the same task,
the same day.

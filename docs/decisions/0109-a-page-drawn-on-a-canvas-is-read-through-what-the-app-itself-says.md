# ADR 0109 — A page drawn on a canvas is read through what the app itself says, never through its pixels

**Status:** Accepted in part (the owner, 11 November 2026): points 1, 3 and 4
of the decision are accepted; point 2, reading an app's own saved document
through CDP's storage domains, stays proposed. Point 1 built by queue items
937–942 (11 November 2026, the amendment below); point 2 not built.
**Date:** 2026-11-11
**Context:** ADR 0002 (the layout tree is the agent's tree; no verb takes a
coordinate); ADR 0058 (Chromium is a renderer we drive; no script of ours in
its page, as ADR 0094 and ADR 0108 read it); ADR 0108 (a cookie banner is
answered with its refusal, by name); `docs/contracts/agent-socket.md`.

## The question in one line

The owner asked: "Without using APIs, find ways we can use other web
applications." These are apps whose main content is drawn on a `<canvas>`:
Google Docs, Google Sheets, Figma, Excalidraw, tldraw, Canva, Miro and
diagrams.net. What can alo's agent read and do in them by name? The rules are
no official API, no screenshot and vision model, and no coordinate the agent
chooses.

## How it was measured

Google Chrome 157.0.8103.50 ran headless with a fresh profile each run, driven
over CDP from Node 26 scripts in the session scratchpad (`canvas-spike/`).
Pages were public and needed no sign-in. Nothing was signed into, accepted or
posted. Cookie banners were answered with their refusal, by name.

A step that "acts by name" did what alo's verbs do. It found a node in
`Accessibility.getFullAXTree` by role and name, asked `DOM.getContentQuads`
where the node sat, and pressed there. It also used `DOM.focus`,
`Input.insertText` and `Input.dispatchKeyEvent`, so the engine resolved every
position and the script never chose one. On macOS, select-all has to be sent
with the `selectAll` editing command, or the key does not select.

Recall is the multiset share of the reference text's words found among the
tree's `StaticText` names. The reference is the app's own export (Docs
`export?format=txt`, Sheets `export?format=csv`) or text the spike seeded.

The scratchpad is cleared by macOS, so the numbers are repeated here.

## What was found

| App | Technique | Measured | Fits ADR 0002 | Effort |
|---|---|---|---|---|
| Google Docs | tree, edit view as loaded | 6.9% of the doc's 1,182 words: menus, the outline's 12 headings and a "Turn on screen reader support" link. `--force-renderer-accessibility` changes nothing (480 nodes and 134 named, against 485 and 139) | yes | — |
| Google Docs | the app's own accessibility mode (⌘+Option+Z, which the tree itself names) | 15.5%: "Screen reader support enabled", "On page 1." and page 1's text as one node with control characters in it. 60 ArrowDowns and 12 wheel scrolls added nothing | yes | M |
| Google Docs | `/mobilebasic`, the app's own HTML view | **99.8%** recall (1,193 tree words), 396 named nodes, 22 ms to read the tree | yes (read only) | S |
| Google Docs | `/preview`, `/htmlview` | 0.1% and 0.4% | — | — |
| Google Sheets | tree, edit view | 0.15% of the sheet's 2,647 words. 127 named and 52 actionable nodes: menus, toolbar, the Name box and the formula bar | yes | — |
| Google Sheets | panels and keyboard: ⌘+J (named in the tree) → type `C29` → Enter → read the formula bar's value | **40 of 40 cells read exactly**, empty ones included, about 870 ms a cell with 800 ms of waits in it | yes | S |
| Google Sheets | `/htmlview` with its frames read (the grid is in a child frame) | **100%** recall, 2,893 named nodes, 921 actionable, 57 ms | yes (read only) | S |
| Excalidraw | tree | 0% of the board's labels. The toolbar's 13 tools are named buttons, and selecting brings up 76 actionable nodes, named, in the properties panel | yes | — |
| Excalidraw | keyboard | 6 of 6 tool keys (r, o, a, t, d, v), each confirmed by the pressed tool in the tree | yes | S |
| Excalidraw | the app's own storage, read with CDP `DOMStorage.getDOMStorageItems`, no page script | **6 of 6 labels**, 12 elements with types, ids and container bindings, in 2 ms | read: yes | S |
| Excalidraw | create by name: More tools → "Mermaid to Excalidraw" → type text into the named field → Insert | **a 4-node flowchart, 13 elements, every label present** in the app's storage, 6.7 s with waits | yes | S |
| tldraw | tree | 27%: only a live announcement of the selected shape ("Finance checks credit, Note. 4 of 4"). 50 named tools and style radios. The cookie banner was refused by name | yes | — |
| tldraw | keyboard | 6 of 6 tool keys confirmed in the tree. Walking between shapes by keyboard did not move in 8 tries (Tab and ⌘→) | yes | — |
| tldraw | the app's own IndexedDB, read with CDP `IndexedDB.requestData` plus `Runtime.getProperties` on the returned objects, no page script | **4 of 4 shapes, 100% of their words**, with id, x, y and rich text, in 13 ms | read: yes | M |
| diagrams.net | tree (SVG and HTML text) | **100%** of 7 labels, 164 named and 75 actionable nodes | yes | none — works today |
| diagrams.net | Extras → "Edit Diagram..." → the XML in a named field → change → Apply | the field held all 6 labels in 1,630 characters of XML. The changed label appeared in the drawing | yes | S |
| Canva | public `/view` link: tree, then "Next page" by name | text and images are in the tree: 11 pages, 192 words, 61 named images. There was no reference text, so recall is not measured | yes | none — works today |
| Figma | Community file, headless | **403** from its CDN to headless Chrome. Spoofing the user agent was not tried (that is getting round a block) | — | — |
| Miro | the app, signed out | goes to sign-up. No public board was found | — | — |

Measured numbers are in `canvas-spike/out/`: `docs-steps.json`,
`docs-sr2.json`, `docs-sr-scroll.json`, `docs.json`, `docs-forced.json`,
`sheets-steps.json`, `sheets-htmlview.json`, `excal-steps.json`,
`excal2.json`, `excal-mermaid.json`, `tldraw-steps.json`, `tldraw-idb.json`,
`drawio-steps.json`, `canva-pages.json` and `figma-steps.json`. Reading a
whole tree took 20–57 ms on every page.

## Proposed decision

1. **Read the app's own words first.** That means its own accessible views
   and dialogs, in the tree: Docs `/mobilebasic`, Sheets `/htmlview`, the
   Name box and formula bar, diagrams.net's *Edit Diagram*, Excalidraw's
   *Mermaid to Excalidraw*. These are the tree we already read and the verbs
   we already have. Nothing new runs in the page.
2. **Read an app's own saved document** through CDP's storage domains
   (`DOMStorage`, `IndexedDB`) as a new, read-only ask, kept to sites like
   every other ask. This runs no script in the page, so ADR 0058 still holds.
   Its answers are the app's own records, named as such and never mixed into
   the layout tree. It is *not* the tree a person sees: an element in storage
   may be off screen or deleted (Excalidraw keeps `isDeleted`), and that has
   to be said.
3. **No recording of a page's canvas calls.** Wrapping a page's 2D, WebGL or
   WebGPU calls means script of ours in a stranger's page, in its main world.
   ADR 0058 rules that out as it stands. The page could see the wrapper and
   tamper with it, and that would break the record guarantee. It was not
   measured (see below).
4. **No coordinates.** Making a shape where there is no named way to do so
   (Excalidraw or tldraw, without a dialog) stays undone. A place on a board
   is a coordinate.

## A first task for each app

- **Sheets:** "What is in C29?", answered through ⌘+J, the Name box and the
  formula bar. A whole sheet is read from `/htmlview`.
- **Docs:** "Summarise this doc", read from `/mobilebasic`.
- **diagrams.net:** "Rename the *Warehouse* box", done through *Edit Diagram*.
- **Excalidraw:** "Draw a flowchart of the order process", done through
  *Mermaid to Excalidraw*. The board is read back from its own storage.
- **tldraw:** "List the notes on this board", read from IndexedDB, read only.
- **Canva:** "What does slide 8 say?", reached with *Next page* by name.

## Not measured, and why

- **Recording canvas calls (technique 1), and Figma's WebGL.** The spike
  stopped at writing the in-page recorder. Hooking a third-party page's
  rendering calls from its main world was refused by a safety check in this
  session, and it is also against ADR 0058 as applied in ADRs 0094 and 0108.
  So there are no numbers for word recovery from drawn text, frame overhead,
  or glyph-atlas naming.
- **Figma, at all:** headless Chrome gets a 403. A shown Chrome was not run
  because the screen was locked.
- **Miro:** there is no signed-out board to open.
- **Writing into Sheets or Docs:** the public files were view-only, and the
  spike never edits a stranger's file.
- **Canva recall:** there was no reference text.
- **Detection and breakage** of the CDP storage reads: none was seen, but
  whether the apps keep these formats is not known. Excalidraw's localStorage
  key and tldraw's `TLDRAW_DOCUMENT_v2…` database are internal to the apps
  and may change without notice.

## Amendment — 11 November 2026: point 1 built (queue items 937–942)

Point 1 is built as the tree we already read and the verbs we already
have, plus one verb and two kinds of hint. Nothing new runs in a page, no
verb takes a position, and nothing reads an app's storage (point 2 stays
proposed and unbuilt).

### What was built

1. **A page's frames are in the agent's tree** (937, `chrome_iframes.rs`).
   `Accessibility.getFullAXTree` reads one document, and an `<iframe>` is an
   `Iframe` node with no children: measured on Sheets' `/htmlview`, whose
   grid is a frame of its own site. Each frame's document is read now and
   put under its `<iframe>` as its one child, by `chrome_tree`'s own rules.
   - A frame of the page's own process is read through its `frameId`, on
     the session that holds its parent. Chrome numbers every node of one
     process from one count (measured: the page's nodes and its frame's
     never meet), and draws them where they are in the window. So a verb
     acts in such a frame by name exactly as on the page: the test page's
     own-site frame took `put_text` and a press, and its listeners heard
     both.
   - A frame of another site is a target of its own, already held by ADR
     0060's rules (`chrome_hold`), and is read through its own session. Its
     process counts from one too, so its ids are made apart: the frame's
     number times 2³² plus Chrome's own, below 2⁵³ so a client reading JSON
     as doubles reads it exactly. Its rectangles are moved by the
     `<iframe>`'s content box (`DOM.getBoxModel`). **A verb on one is
     refused before anything is sent** (`Unacted::InAnotherSitesFrame`, on
     the socket `NotCarriedOut`). The page's hit test finds only the
     `<iframe>` there (measured: `DOM.getNodeForLocation` at the frame's
     button answers the `<iframe>`), so a press would have to be aimed and
     checked through the frame's own session. That is queue item 943.
   - It is bounded at 32 frames, 4 deep. A frame Chrome declines to read
     is an `Iframe` with no children.
2. **`press`, a key by name** (938, `alo-agent`'s `key.rs`). Sheets' Name
   box goes to a cell only on Return. The verb takes a node and one key of
   a closed list: `Enter`, `Escape`, `Tab`, the four arrows, `Home`, `End`,
   `PageUp`, `PageDown`. The node is given the keyboard (`DOM.focus`) and
   the key is sent as a person's press of it with nothing held. Enter goes
   down with its `"\r"`, and on macOS each key carries its editing command,
   as `card_input` sends a person's key. It takes a field or a thing that is
   operated (`NotOperable` otherwise). It is refused as `KeyUnbuilt` on
   this engine's own pages. The person's gate holds Enter wherever
   `activate` would be held, and in a form's field, which Enter sends. No
   letter, Space, Backspace, Delete or modifier is a key: text goes in by
   `put_text`, a press by `activate`, and a Delete on a board deletes what
   is selected, which no key's name says. It is on the socket
   (`"verb":"press","key":…`), on the wire to this engine's renderer, and a
   tool of alo's own agent (`press`). ADR 0096 § 2's tools are now four
   verbs and the two that begin.
3. **An app's own views, offered by name** (939, 940; `app_views.rs`). When
   the card is a Google Doc (`/document/d/ID/…`) or a Google Sheet
   (`/spreadsheets/d/ID/…`), the request to the model carries the page's
   `views`: *Google Docs' own reading view of this doc* at `…/mobilebasic`,
   or *Google Sheets' own view of the whole spreadsheet* at `…/htmlview`.
   The agent **opens one as it opens any site**: in a card the person sees,
   judged by `agent_opening`. That is never a hidden read. A view it opens
   is named in alo's words, whatever the model wrote: the step says
   *Opening Google Docs' own reading view of this doc, read only*, and the
   result ends *Read in … : ADDRESS*. Read only.
4. **An app's own ways, as a hint** (941, 942; `app_ways.rs`). On
   app.diagrams.net and excalidraw.com the request carries the page's
   `ways`. These are alo's words naming the app's own menus and dialogs
   that take the board as words: diagrams.net's *Extras › Edit Diagram...*
   and *Edit › Find/Replace*, and Excalidraw's *More tools › Mermaid to
   Excalidraw* and *Insert*. Excalidraw's way also says not to use its
   *Text to diagram*, which sends the words to Excalidraw's own AI service.
   A way is only a hint: the agent still finds every menu and field by its
   name in the tree, through the same verbs and gate. Nothing here presses
   anything or reads the canvas.

### What was measured

**How.** alo's own agent ran each first task — `conversation_agent::Agent`
with its model (`gpt-6.1-terra`, the key from this Mac's keychain, never
printed), its tools, its gate and its verbs. It ran on the live public page,
through **a stand-in for the window's relay** over one headless tab of the
machine's Chrome 157 (the scratchpad's `harness/`, outside the repository). The
stand-in answered the run's `tabs`, `read` and `act` asks from a `Tab` kept
to a card's rules (`keep_passwords`, `show_as_a_card`), and opened what the
run opened. The page box was the split view's in the design's window, 798 ×
740, unless said. The screen was locked through every run, so **no run was in
alo's window or the Chromium alo carries**. That run is owed (below).
Nothing was signed into. The doc and sheet are the public, view-only ones
above, and were only read. The boards were new: excalidraw.com's own blank
board, and a diagrams.net board made from three boxes of our own XML in its
`#R` address. Seconds run from the task asked to the run's end. They include
the stand-in's 4 s after each page opened and the run's own 2.4 s settle.

| App | Task | Told | Runs | Came to | Steps | Seconds | Tokens in/out |
|---|---|---|---|---|---|---|---|
| Google Docs | *Summarise this doc* | the view | 2 | **done, both**: the reading view opened, and an accurate summary with *Read in Google Docs' own reading view…* | 1 | 16.0, 12.2 | 8,443/264; 8,523/272 |
| Google Sheets | *What is in C29?* | — | 2 | **done, both, exactly**: *Spending for dec 2012*, which the sheet's own CSV export holds at C29. `put_text` *C29* into the unnamed field in the group *Naamvak (⌘ + J)* (Chrome's Dutch here), then `press` Enter, then the formula bar read | 2 | 6.8, 5.9 | 9,542/117; 9,629/127 |
| Google Sheets | *Which cities are listed in this whole spreadsheet?* | the view | 1 | **done**: `/htmlview` opened and its grid read through the frame. The tree's words held **100%** (2,485 of 2,485) of the CSV export's words. The answer, cut at 2,000 characters, listed the cities by country | 2 | 20.9 | 34,007/957 |
| diagrams.net | *Rename the Warehouse box to Distribution centre* | nothing | 1 at 798 | **stuck**: at 798 wide the app hides *Extras* and *Edit*, and the boxes are SVG text no verb takes | 6 | 12.5 | 25,196/450 |
| diagrams.net | same | nothing | 1 at 1,280 × 800 | **done**: *Edit › Find/Replace*, the label replaced. The board then reads *Distribution centre* | 10 | 21.3 | 41,212/624 |
| diagrams.net | same | the way | 2 at 798 | **done, both**: the *Diagram* menu's search found *Find*, and the label was replaced | 11, 10 | 23.9, 18.1 | 47,164/736; 44,483/559 |
| diagrams.net | same | the way | 4 at 1,280 × 800 | **1 done** through *Extras › Edit Diagram...*: the XML read, the label changed, *Apply*. **3 stuck**: the app was still *Starting...*, its tree a `document` and one image, 8–20 s after load (queue item 950) | 5 (done) | 13.9 (done) | 24,061/650 (done) |
| Excalidraw | *Draw a flowchart of the order process* | nothing | 2 | **1 done** through *More tools › Mermaid to Excalidraw* and *Insert*; **1 stuck** after picking the text tool, finding no way to place shapes | 10; 2 | 23.1; 6.2 | 34,017/920; 7,838/234 |
| Excalidraw | same | the way | 4 | **done, all four**: a flowchart of 10–17 shapes on the board (`final.png`). One run used *Text to diagram* (the app's AI) while the Mermaid field was still loading, so the way now says not to. The two runs after that change used Mermaid | 9, 5, 5, 5 | 20.1, 13.5, 13.7, 12.8 | 31,020/677; 19,410/578; 19,853/444; 19,955/479 |

**What a run says of a canvas.** Excalidraw draws on a canvas. After
*Insert* the dialog is gone from the tree and no word of the flowchart is
in it, so the run says what it inserted, never that it saw it. The
pictures in `handrun60/*/final.png` are the stand-in's screencast frames,
taken for this record and never sent to the model.

### Not done, and queued

- **The runs in alo's window**, with the Chromium alo carries, by hand: owed
  by 939–942, because the screen was locked. So those four stay open.
- **Acting in another site's frame**: 943.
- **A shortcut a control's own name holds** (*Name box (⌘ + J)*, and
  diagrams.net's narrow layout, which hides *Extras*): 949, an amendment
  first.
- **A page still starting**, which the agent has no step to wait for: 950.
- **A diagram longer than one `put_text`** (2,048 characters; the
  three-box board's XML was 1,072): 951.
- **Tests on frozen copies of the apps.** Sheets, Docs, diagrams.net and
  Excalidraw cannot be frozen: each is megabytes of the app's own script,
  and two are Google's pages. So the tests drive pages **written for the
  test in the shape measured**: an unnamed field in a group named *Name
  box*, whose own `keydown` listener moves a formula bar
  (`a_key_pressed_by_name_through_chrome.rs`); *Extras › Edit Diagram...*
  over an SVG board, and *More tools › Mermaid to Excalidraw* over a canvas
  (`an_apps_own_dialog_driven_by_name_through_chrome.rs`); a test server's
  page holding a frame from its own site and one from another
  (`a_pages_frames_read_into_its_tree_through_chrome.rs`). Each says it is
  written for the test. What the apps themselves do is the runs' record
  above.

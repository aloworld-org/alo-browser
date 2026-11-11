# ADR 0109 — A page drawn on a canvas is read through what the app itself says, never through its pixels

**Status:** Proposed — a measurement spike, not accepted, not built
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

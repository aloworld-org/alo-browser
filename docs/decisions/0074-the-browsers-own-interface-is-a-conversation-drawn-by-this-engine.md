# ADR 0074 — The browser's own interface is a conversation, drawn by this engine

**Status:** accepted — the chats panel (699), the first screen (700), their window (709), a press on either (710), icons (712), going to a website from the one box (701) and opening it in the machine's Chrome (713, ADR 0075) built; the rest queued
**Date:** 2026-10-30
**Context:** ADR 0024 § 4 (the tab strip is a document of ours, rendered by
the engine in a sandboxed renderer of its own); ADR 0002 (the layout tree is
the agent's tree, and no verb takes a coordinate); ADR 0052 (the agent's gate
and the person's gate); ADR 0061 and 0062 (Chrome draws the page in a window
of its own, and ours stands beside it); ADR 0066 (no window of ours goes over
the page); ADR 0068 (the application draws with the machine's Chrome); the
owner's design, settled on 30 October 2026 and written down in
`docs/design/shell.md`

## The decision in one line

The browser's own interface — everything around a page — becomes **a
conversation**: a panel of chats on the left, a conversation in the middle
that holds search results, answers, the pages a person opens and what the
agent is doing, and one box at the foot to type into. Each part of it is **a
document of ours, rendered by this engine in a sandboxed renderer**, exactly
as the tab strip is, so a person and an agent read the same tree; and it is
built part by part, **the chats panel first**.

## Why this is a decision rather than a design

Because it changes what the frame around a page *is*. Until now that frame
has been a band — a tab, which renderer draws the page, what an agent did —
beside Chrome's window (ADR 0062). The design the owner settled is not a
band: the conversation is the interface, and a page is something opened
*inside* it. Three things follow that are not taste:

1. **How it is drawn.** The interface could be a page in Chrome, written in
   HTML and JavaScript, or documents of ours drawn by this engine. ADR 0024
   § 4 already chose the second for the strip, for reasons that hold here
   twice over: a chat's title, a site's name and a search result are
   strangers' strings, and the interface that shows them must never be
   assembled from them; and the agent reads the interface as the tree it is
   (ADR 0002), which only a document of ours guarantees. Law 4 settles the
   rest: no JavaScript is written for it.
2. **What a person may be asked.** The design has the agent stop before
   paying, never type a password, and say when it is stuck. Those are ADR
   0052's person's gate, given a face; the face does not loosen the gate.
3. **What it needs that this repository does not have.** An answer needs a
   model and a results list needs a search provider. Neither is chosen
   (owner's decisions), so the parts that need them are queued **blocked**,
   not stubbed.

## 1. What is built, and in what order

Each part is a document with a template and a stylesheet compiled in, built
from typed state with `alo-dom`'s operations — never from markup put
together — and drawn by a renderer that holds nothing else, at a site no page
can name, as `strip.rs` and `strip_document.rs` do:

| Part | What it shows | Item |
|---|---|---|
| **The chats panel** | the mark, *New chat*, the open chats and the earlier ones, each with what is happening in it, and *Settings* | **699, built** |
| The first screen | a greeting, the one box, a person's sites, chats waiting on them | 700 |
| Going to a website | what is typed, offered as *Go to the site* first — no model involved | 701 |
| A page inside the conversation | Chrome's window placed in the conversation's card, following it | 702 |
| The conversation | a person's words, the agent's words and its steps | 703 |
| The agent at work | the split view, its steps, *Stop*, *Take over*, asking before paying | 704 |
| What the agent may do | the settings screen, over `alo-grants` | 705 |
| The typefaces | Instrument Sans and Newsreader, rented under the OFL | 706 |
| The answer | written by a model, every claim tied to its quoted source | 707, blocked |
| Search results | sites, images, videos and news, sites first | 708, blocked |

## 2. The colours

The design's palette, which every part's stylesheet spells (a stylesheet in a
renderer reaches no other file, as `strip.css` says of its own):

| Name | Value | Used for |
|---|---|---|
| Ink | `#1B2433` | text, the send button, what is selected |
| Paper | `#F4F1EC` | the conversation behind everything |
| Panel | `#EEEAE2` | the chats panel |
| Surface | `#FFFFFF` | cards, the box, what is chosen in the panel |
| Line | `#DDD6CA` | borders |
| Quiet text | `#5F5A52` | addresses, earlier chats, done steps |
| Acting | `#B5573A` | **only** while the agent acts: its mark, the outline, the step under way |
| Acting text | `#7A3A24` | words on the acting colour's tint |
| Done | `#2F6B5E` | a finished task's tick |
| Source highlight | `#FCE9A8` | the line of a page an answer used |

Every text colour is at least 4.5:1 against what it is drawn on. The strip
keeps its own rail colours until the strip is folded into the panel; nothing
here changes it.

## 3. The typefaces

The design is set in Instrument Sans and Newsreader. Both are under the SIL
Open Font License, and fonts are rented physics (`CLAUDE.md`), but carrying
font files in the application is its own change with its own test, so it is
item 706. Until
then the stylesheets ask for `system-ui, sans-serif` and `serif`, and the
layout assertions are made with the corpus's faces, as every interface test
here already is.

## 4. What is not decided here

**Where Chrome's window goes when a page is opened in the conversation.**
ADR 0062 measured why ours floats *beside* Chrome's under Stage Manager;
putting Chrome's window *inside* a region of ours, following the
conversation as it scrolls, is a different arrangement with costs nobody has
measured — how fast a placed window follows, what Stage Manager does to it,
what a person sees between two placements. ADR 0066 holds either way: the
page's window is over ours, never ours over the page. Item 702 measures it
and decides in an ADR of its own. Until then a page opens as it does today.

**The model and the search provider.** The owner's to choose; 707 and 708
wait on them.

## Consequences

- `alo-renderer` gains the panel's state (`panel.rs`), its document
  (`panel_document.rs`), its template and stylesheet (`interface/panel.*`),
  a message to hold it (`ToRenderer::Panel`) and its wire form
  (`wire/panel.rs`). A chat's title enters the document as a text node and
  nowhere else.
- The agent reads the panel as a `navigation` named *Chats*: a list of links,
  each named by its chat, the open one `aria-current`, one at work
  `aria-busy`, a finished one saying *Done* in words.
- The panel and the first screen are put in a window of their own by 709
  (`alo --conversation`); a press on either is 710's, by the strip's road
  (ADR 0024 § 5): the window aims it (`conversation_aim.rs`), the part's
  renderer claims it by place or kind (`conversation_claim.rs`,
  `ToRenderer::ConversationPress`, `FromRenderer::ConversationClaimed`),
  and the browser process decides it against what the part last painted
  (`conversation_decide.rs`) before acting. *Search chats* and the button
  that hides the panel are 711.
- What a person types into the one box (701) is held by the browser
  process (`alo-window`'s `conversation_box.rs`, keys read by `typing.rs`)
  and sent to the first screen's renderer alone, as data
  (`Start::typed`), shown as the field's value — an attribute set with
  `alo-dom`'s operations, never markup. What it means is decided with
  `alo-url` (`alo-renderer`'s `typed.rs`): *Go to the site* for an
  address, then *Search for* and *Ask alo about*, saying they are not set
  up yet until 708 and 707; an agent reads them as a `listbox` of
  `option`s. Return or *Go* decides where to go, and the address is opened
  in the machine's Chrome from the conversation's own process (713,
  `conversation_chrome.rs`), in Chrome's own window put over the first
  screen with no frame of ours beside it — **ADR 0075**, which says why
  this mode differs from ADR 0062's and what it costs under Stage Manager.
  An agent reaching that page is 716.
- `docs/design/shell.md` is the written form of the design and is what these
  items are checked against; the canvas it came from is the owner's.

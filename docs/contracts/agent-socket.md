# The agent socket — contract, version 1

How an agent outside alo browser reads and drives it. ADR 0055 is why it is
this shape; this file is what a caller builds against.

**Additive only.** A later version may add asks, verbs, fields and refusal
kinds. It never changes or removes what version 1 says, and a request names
its version so the browser can refuse one it does not speak. A caller should
ignore an answer field it does not know.

## Reaching it

A **Unix domain socket** held by the browser process, at

    $XDG_RUNTIME_DIR/alo-browser/agent.sock     (where XDG_RUNTIME_DIR is set)
    $TMPDIR/alo-browser/agent.sock              (otherwise; macOS sets it per person)

The directory is `0700` and the socket `0600`. Only processes running as the
person who started the browser can open it. That is the grant everything else
sits under, and the operating system enforces it. There is no port, and a page
can never reach the socket.

### Kept to sites

A person may start the browser with `alo --agent-site ADDRESS`, as many times
as there are sites. Every agent is then kept to the **sites** of those
addresses. A site is the scheme and the registrable domain, so
`https://example.com` covers `https://www.example.com` but not
`http://example.com`. An address on any other site is refused as
`NotGranted`, and so is a document with no site (`data:`, `about:blank`, a
local file). Without the option, nothing here narrows what the socket allows.

This is **this browser's own refusal, never a grant**. It only narrows: it
cannot raise what lets a program open the socket, or any grant over the
application made by the system it runs on, and it is not that system's grant.
No agent can name, widen or read the sites. They hold for every agent
connected and end when the browser does.

A person who starts alo with `--data DIR` (queue item 957, ADR 0112 § 5)
moves the socket with everything else that alo keeps, to

    DIR/alo-browser/agent.sock

so a second alo started with another directory is reached at its own
socket, and never at the first's. Each prints its path as below.

`alo` prints the path when its window opens, and `alo --chrome` once the
page is open in Chrome; either first says which renderer draws the page.
`alo --conversation` prints it as its window opens, and its socket reaches
**the page shown in the conversation's card now** (queue item 716, ADR
0075's amendment): `tabs` answers that page's address, or none while no card
is open; when a person chooses another address, the page it replaced is no
tab open (`NoSuchTab`) and the new one is answered from then on, over the
same socket; `--agent-site` keeps it to sites as for the others. When there is no private place,
or another browser is already answering there, it says so and opens no socket.
On Windows how an agent reaches the browser is not decided yet.

alo's own agent (queue item 704, ADR 0096) reaches the card through the
same relay, with the same asks and verbs, answered and refused alike and
recorded beside every other agent as *alo's own agent* — never through a
road of its own.

`alo-ask [SOCKET]` is the smallest caller. It sends each line it reads and
prints each answer. `alo-ask --plain [SOCKET]` prints each answer for a
person instead (queue item 915): a tree one node a line, indented, by
number, role, name and states, with no position; anything else as its
`said`. What a person reads there is not this contract: a program reads the
JSON. `docs/drive-alo.md` walks through both.

### Whichever renderer draws the page

A tab may be drawn by this browser's own engine or by the Chrome installed on
the machine (ADR 0058). **Nothing here differs between them**: the same asks,
verbs, answers and refusal kinds, judged in the same order. What each tree
holds is that renderer's description of the page, so a node's number and how
many nodes a tree has are its own, and a number read from one is never good
in the other. Through Chrome, `issues` is heard from Chrome while the verb
is carried out, in the engine's words where the engine has words for it
(queue item 639): a throw nobody caught is placed by the script's address
and the line in the page rather than `script 1` and the line in the script;
a call to the page's console, which this engine does not have, is said as
`console.log:` and what it was given; and something said in a message
longer than one from Chrome may be is said to have been, unread. A frame
from another site and a dedicated worker, each a process or thread of its
own, are heard as the page is (queue item 652), placed by their own
script's address, and so are a shared worker and a service worker (queue
item 654), whose every page is this tab's. Not heard: a
dedicated worker's throw while the verb holds the page's own thread, which
Chrome hands that thread first; and a promise rejected and unhandled that
Chrome tells of a task after the verb. And a tab is
named by the page it shows when the request is answered, however it got
there — a link to a part of the page, or a script's `pushState`, included,
with the part (queue item 640). A tab of this engine's goes nowhere a link
or a page asks yet (queue item 85): after `activate` answers `Followed` it is
still named where it was opened, where the same tab in Chrome is named where
its page went. That is the one way the two renderers name a tab
differently.

At most **8** agents may be connected at once. A ninth gets one
`TooManyCallers` refusal, and its connection is closed.

## Lines

Each request is **one JSON object on one line**, and so is each answer. A
connection's requests are answered in the order they were sent, one at a
time. A blank line is not a request and gets no answer.

| Bound | Value | Exceeded |
|---|---|---|
| A line, its newline included | 65,536 bytes | `LineTooLong`, then the connection is closed |
| Nesting of arrays and objects | 4 | `TooDeep` |
| A name, a role, an `id` | 1,024 characters | `FieldTooLong` |
| A tab's address | 8,192 characters | `FieldTooLong` |
| Depth of a tree answered | 256 | `TreeTooDeep` |

Nothing is ever cut to fit. Whatever is too big is refused.

## Requests

Every request has `"v": 1` and an `"ask"`. It may also carry an `"id"`: a
string or a non-negative whole number. The `id` is handed back unread with the
answer, so a caller can match answers to requests.

A field the ask does not take is refused as `UnknownField`. So is a field that
belongs to another ask: a misspelt field is reported, never silently ignored.

### `tabs`: the address of every open tab

    {"v":1,"id":1,"ask":"tabs"}
    {"answer":"tabs","id":1,"tabs":["https://httpbin.org/forms/post"],"v":1}

### `read`: a page's tree

    {"v":1,"id":2,"ask":"read","tab":"https://httpbin.org/forms/post"}

A tab is named **by its address, never by a number**. A position in the tab
strip is a coordinate (ADR 0002). The address is read as a URL, so
`https://Example.com` names the tab at `https://example.com/`. It is the
address of the page the tab shows. A tab that was redirected, or whose page
went somewhere else, is named by where it is now, not by where it was opened.

When agents are kept to sites, `tabs` lists only the tabs on those sites.

The answer is `{"answer":"tree","id":…,"tab":…,"tree":NODE,"v":1}`. `tree` is
`null` for a page with nothing in it. A `NODE` is:

| Field | Always | Meaning |
|---|---|---|
| `node` | yes | The box's id. A target may name it, but only against the tree it was read from |
| `role` | yes | ARIA's name for what it is: `button`, `textbox`, `generic`, … |
| `name` | no | What a person would read out as its name |
| `states` | no | Only the states that are not defaults: `disabled`, `required`, `readonly`, `busy`, `hidden`, `takes_text`, `broken` (each `true`); `selected`, `expanded`, `pressed` (`true`/`false`); `checked` (`"true"`/`"false"`/`"mixed"`); `invalid` (`"true"`/`"unknown"`); `level` (a number); `current` (`"true"`, `"page"`, `"step"`, `"location"`, `"date"`, `"time"`) |
| `placeholder`, `marker` | no | A field's placeholder, a list item's marker |
| `rect` | yes | `{"height","width","x","y"}` in CSS pixels, where it was drawn. **Read, never acted on**: no verb takes a position |
| `pieces` | no | One `rect` per piece, when an inline box was drawn in more than one |
| `offscreen`, `scrolls` | no | `true` when so |
| `undrawn` | no | `true` for a node that is not drawn at all — a list's option, which a closed `<select>` holds without drawing (ADR 0064 § 5). Its `rect` is all noughts and means nothing; it is `offscreen` too |
| `children` | yes | The nodes inside it |

A length is written as the shortest number that reads back as the same 32-bit
float: `23.2`, not `23.200000762939453`.

**A page's frames are in its tree** (queue item 937, ADR 0109 § 1). Through
Chrome, an `<iframe>` is a node of role `Iframe` whose one child is the
`document` its frame holds, read by the same rules as the page's own. A
frame of the page's own site is acted in by name, as the rest of the page
is. A frame of another site is read and not acted in yet: its nodes' `node`
numbers are its frame's number times 2³² plus Chrome's own, kept apart from
the page's, and below 2⁵³; a verb aimed at one is `NotCarriedOut`, before
anything is sent (queue item 943). At most 32 frames are read, 4 deep; the
rest, and a frame Chrome will not read, are an `Iframe` with no children.

### `renderer`: which renderer draws a tab

    {"v":1,"id":4,"ask":"renderer","tab":"https://httpbin.org/forms/post"}
    {"answer":"renderer","id":4,"renderer":{"kind":"Chrome","product":"Chrome/155.0.8059.86"},
     "said":"Chrome/155.0.8059.86, driven by this browser","tab":"https://httpbin.org/forms/post","v":1}

(The answer is shown on two lines here. On the wire it is one.)

ADR 0058 § 6: *somewhere a person can read, the product says what is
rendering.* This is that place while the browser runs, for a person with
`alo-ask` as much as for an agent (queue item 622). `renderer`'s `kind` is
`ThisEngine`, `Chrome` — the machine's, driven by `alo --chrome` from a
terminal — or `CarriedChromium`, the Chromium the macOS application carries
and the only one it starts (ADR 0077, queue item 719); either of the last
two carries `product`: its product and version as it gives them itself. A
Chromium names itself as Chrome does, so `product` alone never says whose
it is; `kind` does. `said` is the same in English: `Chrome/156.0.8078.76,
the Chromium alo carries, driven by this browser`.

It is answered by the browser process from what it started, never by the
page or its renderer, so a tab with nothing loaded in it is answered all the
same. The tab is named, found and kept to the sites exactly as for `read`,
with the same refusals before any tab is looked at.

### `act`: a verb on something named

    {"v":1,"id":3,"ask":"act","tab":"https://httpbin.org/forms/post",
     "target":{"name":"Customer name:"},"verb":"put_text","text":"Ada Lovelace"}

(Shown on two lines here. On the wire it is one.)

`target` is exactly one of:

- `{"name": …}`
- `{"role": …}`
- `{"role": …, "name": …}`
- `{"node": …}`

`verb` is one of:

- `"activate"`
- `"put_text"`, with `"text"`
- `"scroll"`, with `"scroll"`: a number of pixels, `"start"` or `"end"`
- `"press"`, with `"key"`: one of `Enter`, `Escape`, `Tab`, `ArrowUp`,
  `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`,
  `PageDown`, spelled exactly so (queue item 938). The node is given the
  keyboard and the key pressed on it as a person's, with nothing held;
  what the page does with it is the page's. It takes a field or a thing
  that is operated, and is refused on anything else (`NotOperable`). No
  letter, Space, Backspace, Delete or modifier: text goes in by
  `put_text`, a press by `activate`. On this engine's own pages it is
  refused as `KeyUnbuilt`

To submit a form, activate its submit button. There is no submit verb (ADR
0051 § 6).

When the verb runs, the answer is:

    {"answer":"done","id":3,"issues":[],"outcome":{"kind":"TextPut","node":10,"text":"Ada Lovelace"},
     "said":"put \"Ada Lovelace\" into box#10","tab":"https://httpbin.org/forms/post","v":1}

`issues` is what the page's script said while the verb ran — at most 256
lines, each at most 8,192 characters, and then how many more there were —
for an agent to read rather than to match: a page chooses much of it. `said` is the
outcome in English, for a person — and the person watching is shown the
same sentence, word for word, in a line of the browser's own along the
bottom of its window, written by the browser process and kept until the
next verb runs (queue item 623). A page can neither read that line nor
write it. The box `node` names is ringed in the window on the frame painted
after the verb, on the rectangles the tree gives it there (queue item 624);
nothing about the ring reaches the agent or changes what the verb did. A
verb refused leaves its refusal's `said` there too, word for word, in a
line of its own (queue item 625) — whoever refused it, the page or the
browser process — kept until the second verb that runs after it, so two
refusals in a row are two lines; a read refused, or a line that was not a
request, is not shown. In `alo --chrome` there is no window of the
browser's own to show any of it in until item 642.

| Outcome `kind` | Fields besides `node` |
|---|---|
| `Activated` | `name` (when it has one) |
| `Followed` | `to` |
| `TextPut`, `TextCanceled` | `text` |
| `Scrolled` | `by` |
| `KeyPressed` | `key` |
| `Chosen` | `list`, the option's list; `chosen`, whether the option is a choice of it after the verb, read from the page — `false` when a toggle in a `multiple` list took it away or the page's listener put the old choice back (ADR 0064 § 3). `node` is the option, which a closed list holds undrawn, so the window rings the list instead. Through Chrome a list Chrome draws (a `size`, or `multiple`) is chosen in by a click on the option, with ⌘ held on macOS in a `multiple` one, and `chosen` is read from Chrome's tree (queue item 660); a page's own `role=option` that is not a `<select>`'s is answered `Activated` |
| `Submitted` | `method` (`"GET"`/`"POST"`), `to` |
| `NotSubmitted` | `invalid`: `[{"control","constraint","message"}]`, where `control` is a node or `null` and `constraint` is the validity name (`valueMissing`, …), or `null` when the renderer gave the words and not the constraint. Through Chrome it is always `null` and `invalid` holds one control, the first, with the words of the bubble Chrome draws beside it (queue item 631) |

### `find`: words on a page, by name

    {"v":1,"id":5,"ask":"find","tab":"https://example.com/","text":"next"}
    {"answer":"found","found":[{"name":"Next page","node":14,"role":"link"}],"id":5,"more":0,
     "said":"\"next\" is in link \"Next page\"","tab":"https://example.com/","text":"next","v":1}

(Queue item 956, ADR 0112 § 4. Shown on two lines here; on the wire it is
one.) A person's *Find on page*, answered from the tree `read` answers:
every node whose name holds `text`, compared in lower case, in the order a
person reads them — never the tree's root, the document, whose name is the
page's title; never a node whose nearest ancestor found has the same name
(a link's text inside the link). Each is its `node`, `role` and `name`, and
**no position**: where it is drawn is `read`'s to say, and no verb takes
it. At most 256 are answered; `more` says how many more there were.
`text` is at most 1,024 characters and not only spaces (`BadField`). The
tab is named, found and kept to the sites exactly as for `read`.

### `print_pdf`: the page saved as a PDF

    {"v":1,"id":6,"ask":"print_pdf","tab":"https://example.com/","path":"/Users/ada/Saved/page.pdf"}
    {"answer":"printed","bytes":31812,"id":6,"path":"/Users/ada/Saved/page.pdf",
     "said":"the page at https://example.com/ was saved as a PDF of 31812 bytes at /Users/ada/Saved/page.pdf",
     "tab":"https://example.com/","v":1}

(Queue item 956.) The page as Chromium prints it, backgrounds included,
written by the browser process — never by a renderer — as a **new** file
at `path`, readable by its person alone. `path` must be absolute, with no
`.` or `..` in it, and name a file ending in `.pdf` (`NotAPath`); the
directory it is in, with every link followed, must be one a person allowed
when they started the browser with `alo --agent-saves DIR` (as often as
they like), or inside one (`NotAllowedThere`). **With none allowed, every
path is refused.** These two are judged after the tab's site and before
any tab is looked at. Nothing is ever written over: a file or a link
already there is `Exists`. The answer's `path` is the file made, its
directory's links followed. A renderer that answers with something that
is not a PDF has nothing written (`NotCarriedOut`). This browser's own
engine prints no PDF (`NotOffered`). On alo OS a file will leave through
the document portal instead (ADR 0112).

### `open`: a page, in a tab or a new one

    {"v":1,"id":7,"ask":"open","tab":"https://example.com/","address":"https://example.com/next"}
    {"answer":"opened","id":7,"said":"opened https://example.com/next","tab":"https://example.com/next","v":1}

(Queue item 956.) `address` must be an `http` or `https` address
(`NotOpenable`) on a site agents may reach (`NotGranted`, whose `tab` is
then the address) — judged after the tab's site and before any tab is
looked at. **With `tab`**, that tab's page goes to the address, as an
action of the agent's in the record, and the answer's `tab` is where it
arrived: the name it goes by from then. A page its server sends on to a
site agents may not reach is answered `NotOpened` and never named.
**Without `tab`**, a new tab: in `alo --conversation`, a new chat whose
card shows the page, opened as a person's *Open in new chat* opens one —
answered once the card has opened, with the address it opened at; a
server's redirect is then said by `tabs`. Where only one tab is held
(`alo --chrome`, and this browser's own window) a new tab is `NotOffered`.

### `new_tab`: a tab with nothing in it

    {"v":1,"id":8,"ask":"new_tab"}
    {"answer":"new_tab","id":8,"said":"a new chat is open, with no page in it","v":1}

(Queue item 956.) In `alo --conversation`, a new chat on the first screen,
as *New chat* and ⌘T make one; the card that was live is closed, so
`tabs` then answers none until a page is opened. Elsewhere `NotOffered`.

Neither `open` nor `new_tab` takes a coordinate, an index or a tab's
position: a tab is named by its address, and a new one by nothing.

## Refusals

A refusal is **a kind and its own fields**, never a sentence to parse:

    {"answer":"refused","id":…,"refusal":{"kind":"NotFound","tab":…,"target":{"name":"Nothing here"}},
     "said":"nothing on the page is \"Nothing here\"","v":1}

`said` is there for a person. Its wording may change in any version, so a
caller must never depend on it.

**From the page** — `verb.rs`'s refusals, by their own names. Each carries
`tab` as well:

| Kind | Fields |
|---|---|
| `NotFound` | `target` |
| `Ambiguous` | `target`, `candidates` (nodes) |
| `NotOperable`, `NotAField` | `node`, `role` |
| `Disabled`, `ReadOnly`, `DoesNotScroll` | `node` |
| `Chooses` | `node`: a `<select>`, which takes no text — an option of it is activated instead (ADR 0064 § 4) |
| `NotADate` | `node`, `text` |
| `Unbuilt` | `node`, `field_kind` |
| `KeyUnbuilt` | `node`, `key`: a key aimed at a page this engine drew, which takes no key from an agent yet (queue item 938) |
| `TooLong` | `node`, `most`, `length` |
| `Unchecked` | `node`, `control` (node or `null`), `constraint` |
| `NotSubmittable` | `node`, `why` |

**From this interface:**

| Kind | Fields | When |
|---|---|---|
| `LineTooLong` | `most` | The line is over the bound. The connection then closes |
| `NotJson` | | The line is not JSON |
| `TooDeep` | `most` | The line nests too deeply |
| `NotAnObject` | | The line is JSON, but not an object |
| `UnknownVersion` | `speaks` | `v` is missing, or is not a version this browser speaks |
| `MissingField` | `field` | A field the ask needs is absent |
| `UnknownField` | `field` | A field this ask does not take |
| `BadField` | `field` | A field holds what it cannot, such as an unknown ask or verb, or a target that names nothing, two things, or a position |
| `FieldTooLong` | `field`, `most` | |
| `NotAnAddress` | `tab` | `tab` is not a URL |
| `NotGranted` | `tab`, `site` | `tab` is on a site agents are kept from. `site` is `scheme://registrable-domain`, or `null` for a document with no site |
| `NoSuchTab` | `tab` | No open tab is at that address |
| `Ambiguous` | `tab`, `count` | More than one tab is at that address. None is picked for you |
| `NothingLoaded` | `tab` | |
| `TabGone` | `tab` | The tab's renderer is gone, or could not answer. `said` says why |
| `NotCarriedOut` | `tab` | The verb was decided and the renderer did not carry it out, so nothing was done: through Chrome, the node was drawn nowhere, its middle is under something else, the page went elsewhere while the verb was decided, or a date Chrome's date field would not hold as it was written — past `275760-09-13`, or a year of more than four digits that starts with `0` — was not typed (queue item 632), or an option of a closed `<select>`, which Chrome draws no box for and every key to which chooses options on the way, was not pressed (queue item 660), or the node is in a frame of another site, which is read and not acted in yet (queue item 937). `said` says which |
| `Password` | `tab`, `node` | `put_text` was aimed at a password field of a page shown as a card in the conversation (`alo --conversation`, queue item 704, ADR 0096 § 6): no agent types a password there — signing in is the person's step — and nothing was typed. Judged on the element itself, before anything is sent to the page. `alo --chrome` and this engine's tabs do not refuse it |
| `CardDetails` | `tab`, `node` | `put_text` was aimed at a field of a card's details — its number, security code or expiry — or an option chosen in such a list, on a page shown as a card in the conversation (queue item 826, ADR 0099): no agent types or chooses them there — paying details are the person's — and nothing was done. Judged on the element itself, whatever the page names it: its `autocomplete` (`cc-number`, `cc-csc`, `cc-exp`, `cc-exp-month`, `cc-exp-year`), or its `name`, `id`, `placeholder`, `aria-label` or name in the tree holding a name such a field goes by (*card number*, *cvc*, *security code*, *expiry*, …). A card's holder's name is not refused. `alo --chrome` and this engine's tabs do not refuse it |
| `TreeTooDeep` | `tab`, `most` | |
| `TooManyCallers` | `most` | |
| `NotOpenable` | `address` | `open`'s address is not an `http` or `https` one (queue item 956) |
| `NotOffered` | `ask` | What answers this socket does not do that ask: a new tab outside the conversation, a PDF of this engine's page |
| `NotOpened` | `address` | The page asked for did not open, went on to a site agents may not reach, or another page was asked for before it opened. `said` says which |
| `NotAPath` | `path` | `print_pdf`'s path is not an absolute one of a `.pdf` file with no `.` or `..` |
| `NotAllowedThere` | `path` | Not in a directory the person allowed with `--agent-saves`, or in none that is there. Judged before any tab is looked at |
| `Exists` | `path` | Something is at the path already; nothing is written over it |
| `NotWritten` | `path` | The file could not be written, and nothing of it is left. `said` says why |

Two kinds share the name `Ambiguous`, as ADR 0055 § 5 asks. The page's
carries `target`, and the interface's carries `tab` and `count`.

**Order.** A request is refused for what can be judged without a page before
any tab is looked at. That covers a malformed line, a name that is not an
address, and an address on a site agents are kept from. So `NotGranted` is
the answer whether that site has two tabs open, one, or none.

## The record

The browser process records every request: which connection asked, the tab it
named, that address's site whenever it is an address, and what came of it. A
refused request has its site recorded too. A read records how many nodes of which document it
read. A verb records the target, the verb, the action id it was given, and its
outcome or refusal. A page sent somewhere records the action and where;
a new tab asked for, where to; a `find`, how many nodes it found; a PDF
saved, its path and length. A refused request is recorded as refused. A request that
reached a tab records which renderer drew its page: this browser's engine, or
Chrome and the version it gives itself.

The record is the person's, and is kept about the agent. Nothing over this
socket can read it (ADR 0012 § 7). A renderer cannot write it.

## What version 1 does not do

- **Load where a verb leads.** `Followed` and `Submitted` say where the page
  asked to go. The browser decides that, and loading the next page is queue
  item 85's for every navigation.
- **Keep a box's id across a verb that adds a box.** Text put into a field is
  a box of its own, and the ids of the boxes after it move (queue item 500).
  Until that is fixed, an id is good for the tree it was read from, and a
  verb aimed by name is safer.
- **Watch.** There is no subscription. A caller asks and is answered (ADR
  0002: *reading is never watching*).

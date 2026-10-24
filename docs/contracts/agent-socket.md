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

`alo` prints the path when its window opens. When there is no private place,
or another browser is already answering there, it says so and opens no socket.
On Windows how an agent reaches the browser is not decided yet.

`alo-ask [SOCKET]` is the smallest caller. It sends each line it reads and
prints each answer.

### Whichever renderer draws the page

A tab may be drawn by this browser's own engine or by the Chrome installed on
the machine (ADR 0058). **Nothing here differs between them**: the same asks,
verbs, answers and refusal kinds, judged in the same order. What each tree
holds is that renderer's description of the page, so a node's number and how
many nodes a tree has are its own, and a number read from one is never good
in the other. Through Chrome, `issues` is always empty, because what the
page's script said is not asked of Chrome (queue item 639), and a tab is
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
| `children` | yes | The nodes inside it |

A length is written as the shortest number that reads back as the same 32-bit
float: `23.2`, not `23.200000762939453`.

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

To submit a form, activate its submit button. There is no submit verb (ADR
0051 § 6).

When the verb runs, the answer is:

    {"answer":"done","id":3,"issues":[],"outcome":{"kind":"TextPut","node":10,"text":"Ada Lovelace"},
     "said":"put \"Ada Lovelace\" into box#10","tab":"https://httpbin.org/forms/post","v":1}

`issues` is what the page's script said while the verb ran (empty through
Chrome, which is not asked). `said` is the
outcome in English, for a person.

| Outcome `kind` | Fields besides `node` |
|---|---|
| `Activated` | `name` (when it has one) |
| `Followed` | `to` |
| `TextPut`, `TextCanceled` | `text` |
| `Scrolled` | `by` |
| `Submitted` | `method` (`"GET"`/`"POST"`), `to` |
| `NotSubmitted` | `invalid`: `[{"control","constraint","message"}]`, where `control` is a node or `null` and `constraint` is the validity name (`valueMissing`, …) |

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
| `NotADate` | `node`, `text` |
| `Unbuilt` | `node`, `field_kind` |
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
| `NotCarriedOut` | `tab` | The verb was decided and the renderer did not carry it out, so nothing was done: through Chrome, the node was drawn nowhere, its middle is under something else, or the page went elsewhere while the verb was decided. `said` says which |
| `TreeTooDeep` | `tab`, `most` | |
| `TooManyCallers` | `most` | |

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
outcome or refusal. A refused request is recorded as refused. A request that
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

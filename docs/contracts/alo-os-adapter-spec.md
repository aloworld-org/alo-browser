# alo browser's adapter for alo OS — specification

**Status:** specification, version 1, 12 November 2026. Queue item 977, ADR
0112 § *Answered* (the adapter). Additive changes only; a break is a new
version of this document and of the socket's `"v"`.

**Who writes what.** This repository owns this specification and the wire
protocol it rests on, [`agent-socket.md`](agent-socket.md). The alo OS team
writes the `alo_adapters::Adapter` constant inside alo-os from it, under
alo-os's own licence, and builds the `api` mechanism that speaks the socket.
**alo browser depends on nothing of alo OS's.** The socket is the browser's
own automation interface, the same on every system it runs on; alo OS is one
caller of it among any.

It is written against alo OS's `docs/contracts/app-adapters.md` and
`agent-verbs.md` as of alo-os `origin/main` of 26 October 2026, and the
coordinator's answers of 12 November 2026. Where alo OS's contract has moved
since, alo OS's contract wins and this document is corrected.

## The adapter

| Field | Value |
|---|---|
| `name` | `browser` — an agent says `@browser`, and a verb is `browser.open_page` |
| `application` | `org.aloworld.Browser` |
| `releases` | `0` until the first release is numbered; each release series is added as it ships |
| `mechanism` | `api` — the agent socket |
| `grants` | the grant over the application, for every verb; `save_as_pdf` also needs one over the folder it saves into |

**Offered before alo OS's text arguments (Demo 2 stage 5):** `list_tabs`,
`read_page`, `open_page`, `go_to`, `new_chat`, `press_control` and
`press_key`. **After it:** `find_on_page` and `fill_field`. **After queue
item 967:** `save_as_pdf`.

**Written by alo OS, not here:** each sentence's and purpose's translatable
key (`words`), `reaches` (`OnlyItsApplication` for every verb but
`save_as_pdf`, which is `Over(file)`), and `carried_out` for `api` — one
JSON line per verb from a closed template, exactly the *Sent* lines below.

**No verb runs code.** No argument is a script, a command, an expression or
anything the browser or the page interprets as code. A page's script never
receives an argument as code: `fill_field`'s text is put into the field as
a person's typing would put it.

## Reaching the browser

One JSON object per line, each way, on a Unix socket held by the browser:

    $XDG_RUNTIME_DIR/alo-browser/agent.sock

or, for a browser started with `alo --data DIR` (alo's own copy, ADR 0112
§ 5), `DIR/alo-browser/agent.sock`. The directory is `0700` and the socket
`0600`: only the person's own processes open it. Every request carries
`"v":1` and may carry an `"id"`, handed back unread. Bounds, refusals and the
order requests are judged in are [`agent-socket.md`](agent-socket.md)'s;
nothing here changes them.

**A browser that is not running** has no socket: connecting fails. The
`api` mechanism then answers the verb as not carried out. It never starts
the browser on its own; a person or a separate verb of the machine's does.

**Answers.** An answer whose `"answer"` is `"refused"` carries a
`"refusal"` with a `"kind"` and its fields (`agent-socket.md`, *Refusals*);
the verb is then not carried out, and the kind is what the record keeps. Any
other answer is the verb carried out. `"said"` is an English sentence for a
person, shown as it is; **a caller never parses it**, since its wording may
change in any version.

## Argument kinds

| Kind | What it holds | Bound |
|---|---|---|
| **web address** | an `http` or `https` URL | 8,192 characters |
| **tab** | a tab's address, exactly as `list_tabs` answered it. A tab is named by its address, never by a number or a position | 8,192 characters |
| **name** | a thing's accessible name, as a person reads it on the page | 1,024 characters |
| **role** | a thing's accessible role (`button`, `link`, `textbox`, `checkbox`, …), optional beside a name | 1,024 characters |
| **key** | one of a closed list: `Enter`, `Escape`, `Tab`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown` | — |
| **text, shown in full** | text put into a field or looked for, **shown whole in the approval sentence**; alo OS's argument kind of Demo 2 stage 5 | 4,096 bytes of UTF-8 for `fill_field`, 1,024 characters for `find_on_page` |
| **file** | an absolute path of a new `.pdf` file, in a folder the person allowed | the path's own |

No argument is ever a position, a coordinate or a number of a thing on the
screen (this repository's ADR 0002).

## The verbs

Each verb gives: purpose, effect, arguments, the approval sentence, `by_hand`
(how a person does it themselves), and what is sent. In a sentence,
`{name}` stands for the validated argument, shown in full.

### `browser.list_tabs`

- **Purpose:** the address of every page open in the browser.
- **Effect:** read.
- **Arguments:** none.
- **Sentence:** *See which pages are open in alo browser.*
- **By hand:** look at the browser's chats panel.
- **Sent:** `{"v":1,"ask":"tabs"}`
- **Answer:** `{"answer":"tabs","tabs":[ADDRESS, …]}`.

### `browser.read_page`

- **Purpose:** the words of an open page, in the order a person reads them.
- **Effect:** read.
- **Arguments:** `tab` (tab).
- **Sentence:** *Read the page {tab} in alo browser.*
- **By hand:** read the page.
- **Sent:** `{"v":1,"ask":"page_text","tab":TAB}` (queue item 964).
- **Answer:** `{"answer":"page_text","text":TEXT,"cut":BOOL,"more":N,…}`:
  the page's words as plain text, a line for each heading, paragraph, list
  item (its marker first) and table row; nothing hidden or undrawn, no
  title, no picture names, and no value of a password or of a field whose
  name says it holds a secret. At most 100,000 characters, cut at a line
  break or a space; `cut` says it was, `more` how many characters were not
  answered.

### `browser.find_on_page`

- **Purpose:** the things on an open page whose names hold some words.
- **Effect:** read.
- **Arguments:** `tab` (tab); `words` (text, shown in full; 1,024
  characters, not only spaces).
- **Sentence:** *Find "{words}" on the page {tab} in alo browser.*
- **By hand:** *Find on page*, and type the words.
- **Sent:** `{"v":1,"ask":"find","tab":TAB,"text":WORDS}`
- **Answer:** `found`: each thing's `node`, `role` and `name`, at most 256,
  and `more`. A `node` is a number the page's tree gives a thing for this
  tree only; it is **not** offered as an argument to any verb here.
- **Offered** only once alo OS's *text, shown in full* argument kind exists
  (Demo 2 stage 5): `words` is text, which alo OS's loader refuses until
  then (the coordinator, 12 November 2026).

### `browser.open_page`

- **Purpose:** open a web address in a new chat.
- **Effect:** change.
- **Arguments:** `address` (web address).
- **Sentence:** *Open {address} in a new chat in alo browser.*
- **By hand:** paste the address into a new chat and open it.
- **Sent:** `{"v":1,"ask":"open","address":ADDRESS}`
- **Answer:** `opened`, with the `tab` it arrived at. Refused as
  `NotOpenable` (not http or https), `NotGranted` (a site the person kept
  agents from), `NotOpened`.

### `browser.go_to`

- **Purpose:** take an open page to another web address.
- **Effect:** change.
- **Arguments:** `tab` (tab); `address` (web address).
- **Sentence:** *Take the page {tab} in alo browser to {address}.*
- **By hand:** type the address into that chat's card.
- **Sent:** `{"v":1,"ask":"open","tab":TAB,"address":ADDRESS}`
- **Answer:** `opened`, whose `tab` is the page's name from then on.

### `browser.new_chat`

- **Purpose:** start a new, empty chat.
- **Effect:** change.
- **Arguments:** none.
- **Sentence:** *Start a new chat in alo browser.*
- **By hand:** *New chat*, or ⌘T / Ctrl+T.
- **Sent:** `{"v":1,"ask":"new_tab"}`

### `browser.fill_field`

- **Purpose:** put text into a field on an open page, as a person types it.
- **Effect:** change.
- **Arguments:** `tab` (tab); `field` (name); `role` (role, optional, when
  two things share a name); `text` (text, shown in full; at most 4,096
  bytes).
- **Sentence:** *Type "{text}" into the field "{field}" on the page {tab}
  in alo browser.*
- **By hand:** click the field and type.
- **Sent:** `{"v":1,"ask":"act","tab":TAB,"target":{"name":FIELD},"verb":"put_text","text":TEXT}`,
  or with `"target":{"role":ROLE,"name":FIELD}`.
- **Never carried out**, whatever was approved: a password field
  (`Password`) or a payment card's number, security code or expiry
  (`CardDetails`) on a page shown as a card. Signing in and paying are the
  person's.
- **Offered** only once alo OS's *text, shown in full* argument kind exists
  (Demo 2 stage 5), as the coordinator said on 12 November 2026.

### `browser.press_control`

- **Purpose:** press a button, link, checkbox or other control on an open
  page, by its name.
- **Effect:** change. What the page does on the press is the page's — it may
  submit a form, follow a link or send a message.
- **Arguments:** `tab` (tab); `control` (name); `role` (role, optional).
- **Sentence:** *Press "{control}" on the page {tab} in alo browser.*
- **By hand:** click it.
- **Sent:** `{"v":1,"ask":"act","tab":TAB,"target":{"name":CONTROL},"verb":"activate"}`

### `browser.press_key`

- **Purpose:** press one key on a field or control of an open page.
- **Effect:** change.
- **Arguments:** `tab` (tab); `on` (name); `role` (role, optional); `key`
  (key).
- **Sentence:** *Press {key} on "{on}" on the page {tab} in alo browser.*
- **By hand:** click it and press the key.
- **Sent:** `{"v":1,"ask":"act","tab":TAB,"target":{"name":ON},"verb":"press","key":KEY}`

### `browser.save_as_pdf`

- **Purpose:** save an open page as a PDF file.
- **Effect:** change (a new file).
- **Arguments:** `tab` (tab); `file` (file).
- **Reaches:** over `file`: a grant over the folder it is in.
- **Sentence:** *Save the page {tab} from alo browser as the PDF {file}.*
- **By hand:** *Print*, then *Save as PDF*.
- **Sent:** `{"v":1,"ask":"print_pdf","tab":TAB,"path":FILE}`
- **The browser's own refusal as well:** the folder must be one the person
  allowed when the browser started (`--agent-saves DIR`), and nothing is
  written over an existing file (`Exists`). On alo OS the file will leave
  through the document portal instead (ADR 0112, queue item 967), and this
  verb is amended then.
- **Offered on alo OS only after 967.** alo OS never starts the browser, so
  a browser a person started has no `--agent-saves` folder and every save
  would be refused. The browser does **not** take a folder on a caller's
  word: the socket cannot tell alo OS from any other program running as
  the person, so a folder named in a request is no grant. The document
  portal, which asks the person, is the road (decided 12 November 2026).

## Not offered

- **Scrolling** (`act`'s `scroll`). Reading a page does not need it, and a
  person's view is theirs.
- **Reading or acting on a page by position**, ever.
- **A verb that runs script, evaluates anything or takes a page's code.**
- **Signing in, paying, or choosing a card's details.** They are the
  person's, and the socket refuses them on a card.
- **`launched`** — a second launch of the browser with an address (ADR
  0114). It is the system's *open with* road, not an agent's.

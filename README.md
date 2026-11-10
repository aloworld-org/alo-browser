<p align="center">
  <img src="docs/brand/alo-browser-icon.svg" alt="alo browser's icon: a navy globe crossed by a teal swoosh, on a cream tile" width="128" height="128">
</p>

<h1 align="center">alo browser</h1>

**A browser built as a conversation, with an engine written in Rust.**

alo replaces the address bar, the tab strip and the search page with one
conversation. You type into a single box — a question, a search or an address
— and alo answers, searches or opens the site, right there in the chat. Web
pages arrive as cards inside the conversation. And when a task takes more than
reading, you ask alo to do it: it plans a few short steps, finds the sites,
and works on the pages while you watch.

Its own engine — the DOM, style, layout, painting and the interface — is
written in Rust, memory-safe by construction, and built so that an agent reads
what an interface *is* rather than a photograph of it.

**Status: in active development, on macOS.** It runs as a Mac application
today; it is not yet released, signed or notarised. Windows and Linux are
planned. See [`ROADMAP.md`](ROADMAP.md), [`docs/features.md`](docs/features.md)
and, for what renders correctly today, [`docs/conformance.md`](docs/conformance.md).

## What it does today

- **One box for everything.** Ask, search or type an address; as you type, it
  offers to search, to open the site, or to ask alo to do it.
- **Answers with their sources.** A short written answer from the top search
  results, with the sites it came from, and the results sorted into All,
  Answer, Sites, Images, Videos and News. When there is no reliable answer, it
  says so and shows the sites instead.
- **Pages as cards.** A site opens as a card in the conversation, running
  Chromium, so the everyday web works as it should.
- **Ask alo to do it.** The agent plans, searches, opens sites and acts on
  pages by name. It asks before it pays, sends, deletes or submits a form;
  never types a password or card details; hands a sign-in page to you; and
  stays within the sites and actions you allow in Settings.
- **Ads and trackers refused** by EasyList and EasyPrivacy, carried in the
  application.
- **Chats kept on your Mac**, searchable, with their pages.

## Why build this

Chromium is around 35 million lines, and most of that is not capability — it
is compatibility with thirty years of broken pages. If you do not have to
render the 2003 web, an enormous amount of it evaporates: the parser's error
recovery, quirks mode, floats and table layout, the accreted DOM surface.

What a modern interface needs is a shorter list — a box model, flexbox and
grid, custom properties, text, transforms, compositing, input, and a retained
tree. That is a serious project, not an impossible one. So alo draws its own
interface with its own engine, and rents Chromium only for the pages people
open — until its own engine can draw those too.

Two things follow that no existing engine offers:

- **Memory safety**, in a product whose other half is sold on auditability.
- **An agent that reads the interface as a tree** — "invoice list, twelve rows,
  row three selected" — and acts on it through typed verbs, never coordinates.
  Most AI browsers photograph somebody else's page and guess where to click.

## Drive it from your code

A program on your Mac reads a page in alo as a tree — what each thing *is*
and what it is called — and presses, types and chooses by name, one JSON
line at a time over a socket only your own programs can open. No selectors,
screenshots or coordinates, and no AI key. See
[Drive alo from your code](docs/drive-alo.md), and
[`docs/demo.md`](docs/demo.md) for a short demonstration to record.

## Your own services

alo does not tie anyone to one search engine or one AI. Searching, a
search's written answer and alo's agent each go through one interface, and
which service answers is configuration, not code: each service is a
connector beside the others, and adding one is two files and a line where
it is chosen (ADR 0106). A self-hosted, open source search service that
needs no key is one of them.

- **Choosing.** In *Settings › Search and answers*, or by the `search` and
  `answers` lines of `settings.txt` beside the chats, with an address line
  for a service that needs one (ADR 0106 § 3). A change is used from alo's
  next start.
- **Keys.** Your own key, typed or pasted in *Settings*, is kept only in
  this Mac's keychain — never in a file of alo's, a log or a chat — and wins
  over a key the application carries. A development build also reads the
  keychain items earlier builds used. Keys never live in this repository.
- **A release** carries the keys it is given when it is made, and may name
  the services it asks unless a person chooses others:
  `alo-bundle --keys-from keychain|DIRECTORY --services FILE`.

## Building it

alo is built and verified on macOS, with Rust 1.89 or newer.

```sh
cargo build --release
```

builds `alo`, the renderer and the other programs. To make the application,
`alo browser.app`, with the Chromium it carries, download the CEF distribution
the repository pins — the macOS arm64 minimal build of CEF 156.0.14 from
cef-builds.spotifycdn.com, checked against `crates/alo-window/src/cef_pin.rs`
— unpack it, then:

```sh
CEF_PATH=… cargo build --release --locked \
  --manifest-path crates/alo-cef-host/Cargo.toml --target-dir target/alo-cef-host
CEF_PATH=… target/release/alo-bundle
```

Without `CEF_PATH` the application is made with no Chromium, and says so. Pass
`--keys-from DIRECTORY` to carry keys from files outside the repository; with
none, the application carries no key and says so.

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) first: it is the project's constitution, and
everything in it is absolute. Settled decisions live in
[`docs/decisions/`](docs/decisions/) — read the ADR before proposing an
alternative. Nothing is done until [`scripts/gate.sh`](scripts/gate.sh)
passes: formatting, clippy with zero warnings, every test, and a reference
render for anything visual.

## What it is not

- **Not a Chromium fork**, and not a Ladybird fork. Chromium runs only the
  pages inside cards; the interface around them, and the engine that draws
  it, are ours.
- **Not a shaper, a codec or a TLS stack.** We rent the physics. Nobody writes
  their own text shaper, and not out of timidity.
- **Not a compatibility project.** No quirks mode, no floats-as-layout, no
  `document.write`. The legacy web is scheduled by real pages failing, never by
  a specification listing a property.
- **Not a dependency of alo OS, and alo OS is not a dependency of this.** The
  independence runs both ways.

## The stages

| | |
|---|---|
| **1** | Renders alo's own interface. No scripting, no hostile pages, no compatibility burden. Complete. |
| **2** | The modern web: a JavaScript engine, the network stack, and a process and sandbox model designed **before** it ever loads a hostile page. In progress; pages in cards run on the Chromium alo carries meanwhile. |
| **3** | The legacy tail, scheduled by real pages failing rather than by a specification. |
| **4** | Product adoption: extensions, sync, updates, mobile and local AI; gated on stage 2. |

## Licence

**MPL-2.0** (ADR 0009). An engine you want embedded and contributed to cannot be
copyleft-heavy, and this is not: MPL is *file-level* copyleft, so anyone may
embed this engine in a closed product. What they may not do is improve the
engine in private and ship a better version of our own work against us —
changes to these files come back.

It was Apache-2.0 until somebody asked the obvious question: that licence let
anyone take the agent tree, close it, and sell it. The permissive half of the
reasoning was right and is kept; the part that gave away the one genuinely novel
thing in here was not.

The alo workspace is AGPL-3.0 and alo OS is GPL-3.0; this is still the one meant
to be used by other people, and Servo — whose parser and selector engine we
rent — chose the same licence for the same reason.

# ADR 0106 — Search and answers are asked of a service chosen by configuration, through one interface each

**Status:** accepted — the owner decided on 9 November 2026; built by queue
items 904–906 (see *Built*, below)
**Date:** 2026-11-09
**Context:** ADR 0082 § 0 (the Brave Search API); ADR 0083 (a search is
asked by the browser process, with the words alone); ADR 0086 and ADR 0088
(a search's pictures and marks come from the search service alone, never
from a site); ADR 0087 (videos and news asked only when their tab is
opened); ADR 0081 and ADR 0095 (OpenAI writes a search's answer); ADR 0096
(alo's own agent asks the same model); ADR 0091 (the keys a release carries
inside the application); ADR 0098 (the chats' key in the person's
keychain); ADR 0005 (a renderer has no network, and is never sent a key);
ADR 0001 (rented crates behind named files); queue items 900 (no
provider's name where a person reads) and 854/878 (*Settings*, kept in
`settings.txt`)

## The question in one line

alo is open source, and somebody building it may need Google, a SearXNG of
their own, or another AI: how does the rest of alo stop depending on Brave
and OpenAI, how is a service chosen, and whose key is used?

## 0. The owner's decision

On 9 November 2026, verbatim:

> *"since it will be open source we should not be brave hardcoded because
> the others may need to use google or other apis"*

and, asked whether Brave stays in the public repository as one option:
*"yes please"*. The owner also asked for **both** ways of holding a key: a
release carries the keys it is made with, **and** a person can add their
own in *Settings*, their own winning. A private repository will later make
the shipped application with the owner's keys and do the signing and the
releases (nothing of it is made here).

## 1. Two interfaces

Everything that searches or asks a model goes through one interface each,
in the browser process (`alo-window`):

- **The search service** (`search_service.rs`): `SearchService`, a handle
  any thread may clone, over a `SearchConnector` — a web search of the
  words alone, a further page of it, images, videos and news, and the one
  origin pictures and marks may be fetched from. Each connector **declares
  what it searches** (`Searched::{Web, Images, Videos, News, Pages}`), and
  `SearchService` refuses what it does not before anything is asked, in
  one sentence a person reads: *The search service chosen in Settings does
  not search for videos.* What the threads are told — the sites, the
  images, a page, whether more are there — is the interface's own types
  (`Answered`, `ImageFound`, `VideoFound`, `StoryFound`), not a
  connector's.
- **The AI service** (`ai_service.rs`): `AiService`, over an `AiConnector`
  — a search's answer from its results' words (ADR 0095), and the agent's
  next step (ADR 0096). The answer's thread and the agent hold an
  `AiService` and know nothing of whose model it is.

The results tabs, the box, the agent's search, the answer's thread, the
chats and the renderer depend only on these. A connector is one file that
knows its service's address, its request and its statuses, and one file
that reads its answers (a protocol read from a stranger, the reason
`serde_json` is named there: `scripts/gate.sh`'s boundary list).

**Adding a connector** is: its two files, a variant in `services.rs`
(its word in `settings.txt`, its name in the chooser, its keychain service
and carried slot if it takes a key), and its arm where the service is made
(`services_made.rs`). Nothing else changes.

## 2. The connectors, and what each searches

| | Web | Further pages | Images | Videos | News | Key |
|---|---|---|---|---|---|---|
| **Brave Search** (`brave_search.rs`, `search_protocol.rs`) — the default | yes | yes, `offset` 1–9 | yes | yes | yes | yes |
| **SearXNG** (`searxng_search.rs`, `searxng_protocol.rs`) | yes | yes, `pageno` 2–10 | **no** | **no** | yes | none |
| **OpenAI** (`openai_answer.rs`, `answer_protocol.rs`, `agent_protocol.rs`) — the AI service, the default and the only one | answers and the agent's steps | | | | | yes |

**SearXNG** is open source and self-hostable, and takes no key. Its search
API, read in its own documentation (`docs.searxng.org/dev/search_api.html`,
9 November 2026): `GET /search` with `q`, `categories`, `pageno`,
`language`, `time_range`, `safesearch` and `format`; alo sends `q`,
`format=json` and, only where it is needed, `categories=news` and
`pageno` — the words alone again (ADR 0083 § 3), no language, no time, no
safe-search, nothing of the person. *"Requesting an unset format will
return a 403 Forbidden error"*, and *"many public instances have these
formats disabled"*: the JSON format must be allowed in the instance's
`settings.yml` (`search: formats: [html, json]`), and a 403 says so in
words. Its JSON (`searx/webutils.py`'s `get_json_response`, read the same
day) is `query`, `results`, `answers`, `corrections`, `infoboxes`,
`suggestions` and `unresponsive_engines`; each result's `url`, `title` and
`content` are plain text in that format (the HTML escaping and highlighting
is the HTML template's alone), `publishedDate` an ISO date.

**Why SearXNG searches no images or videos here.** Its JSON names each
picture at the site that holds it (`img_src`, `thumbnail_src`,
`thumbnail`): the image proxy SearXNG offers is applied by its HTML
template alone (`webapp.py`'s `image_proxify`), and an address of it needs
the instance's secret. ADR 0086 and ADR 0088 decided that showing results
contacts no site but the search service's, so a SearXNG Images or Videos
tab says plainly that the service chosen does not search them, and a story
is drawn without a photo and every site with its letter. *Queued* (907):
pictures through an instance's own image proxy, if one can be asked
without its secret.

**Why no Google now.** Google's Custom Search JSON API — the one it sold
for a program's web search — *"is closed to new customers"*, and
*"existing Custom Search JSON API customers have until January 1, 2027 to
transition to an alternative solution"* (Google's own overview,
`developers.google.com/custom-search/v1/overview`, read 9 November 2026). A
connector nobody can get a key for, to a service ending in seven weeks,
would be a connector nobody could test or use. The interface is the
point: a later Google (or any other) connector is its two files and its
variant, as § 1 says.

## 3. Which service, chosen by configuration

In `settings.txt`, beside the font and *alo at work* (queue items 854,
878), three lines, each optional:

```
search brave            # or: search searxng
searxng https://search.example.org
answers openai
```

- **Precedence:** the person's line, then the application's own default
  (`Contents/Resources/services.txt`, the same three lines, put in by
  `alo-bundle --services FILE` as the application is made — how a private
  repository's release chooses its service without touching this one),
  then Brave and OpenAI. So a build made as today keeps searching Brave.
- **Read strictly**, as the rest of the file is: a word alo does not know,
  a line twice, or a `searxng` address that is not an `http` or `https`
  site's (no user, password, query or fragment) is the whole file not
  read, said with why, and the defaults used — never half a file.
- **Written only as named**: a file is written whole whenever a person
  chooses anything, with only the service lines a person (or an older
  file) named, so a release's default is never frozen into a person's
  file.
- `searxng` with no address is *Searching is not set up yet: the search
  service chosen in Settings has no address.*
- **Read once, as alo starts.** A service chosen in *Settings* is used from
  alo's next start, and *Settings* says so under its choices. *Queued*
  (908): a change used at once.

## 4. Whose key, and where it is kept

A connector that takes a key (Brave, OpenAI; SearXNG takes none) has:

- **a keychain service of its own**, a generic password for the person's
  login name: `alo-search-brave` and `alo-ai-openai`. These hold **the
  person's own key**, entered in *Settings*.
- **a slot** in a release (ADR 0091), `Contents/Resources/keys/SLOT`,
  masked as before: `search` for Brave and `answer` for OpenAI — the same
  file names as before, so every application already made still reads.
- **an older keychain service**, read so nothing breaks for anyone who set
  one up before: `alo-brave-search` and `alo-openai` (ADR 0083 § 1, ADR
  0095 § 2). alo never writes it.

**Precedence, first found wins:** the person's own (the connector's
keychain service) → the application's carried key → the older keychain
item → none, said: *Searching is not set up yet: add a key for the search
service in Settings.* A carried key that cannot be read is still said and not swapped
for the older item (ADR 0091 § 1); a key of the person's own still wins
over it, since it is theirs. A key refused (401, 403) says which it was:
*the key you gave in Settings*, *the key in this Mac's keychain*, or *the
key this application carries*.

**A key typed in *Settings*** is held by the browser process alone, in
memory, in the field's editing state. The renderer is sent **only how many
characters it has**, drawn as dots, and never the key — it is a confined
process (ADR 0005). Saved, it goes to this Mac's login keychain through
`/usr/bin/security -i` on its **standard input** (never an argument, which
another program can read), **as hexadecimal** (`-X`), with `-U` to replace
the person's own earlier key; never to `settings.txt`, a log, the
terminal, the chats folder, the agent or the model. **Removed**, the item
is deleted (`delete-generic-password`), and the carried or older key is
used again from the next start. A key is checked as a request header can
carry it — visible ASCII, at most 256 characters — before it is kept. alo
never shows a key back: a kept key is drawn as *Your key is kept in this
Mac's keychain*, never as dots of its length. Tests name a keychain file of
their own under the temporary directory (`Keychain::File`, as ADR 0098's
are), never the person's.

**`alo-bundle`** carries the keys the application's services take:
`--keys-from keychain|DIRECTORY` takes each slot of the services it is
made to default to (by `--services`, else Brave and OpenAI) — from the
keychain, the person's own service first and the older one after; from a
directory, the file named as the slot — and refuses, naming which, when one
is missing. A service that takes no key carries none.

## 5. Nothing a person reads names a provider — but the chooser

Queue item 900 kept the provider's name off the screen. That now holds for
every service: what a person reads says *the search service* and *the AI
service* — a refusal, a status, a tab's sentence, the note under an answer
(*… were sent to the AI service chosen in Settings to write this answer.*),
the agent's stops, a missing key.

**The one exception is *Settings*' chooser.** Choosing a service is
choosing it by name — nobody can pick *the first search service* — so each
choice in the radio groups *Search with* and *Answers by* is labelled by
the service's own name (*Brave Search*, *SearXNG*, *OpenAI*), as its text
and its accessible name. Nowhere else: not the section's heading, its
lines, its fields, a key's state, nor anything with *Settings* closed.
`no_providers_name_on_screen.rs` holds it: it asks every drawn state for
each name in any case, and allows a name only inside a chooser's option
(`class="svc"`), and only its own.

## 6. What stays public, and what a private repository supplies later

**Public, here:** every connector, both interfaces, the configuration, the
key store, *Settings*, and `alo-bundle` with `--keys-from` and
`--services`. **Supplied later by the owner's private repository, at
bundle time only:** the keys (`--keys-from`), the services a release
defaults to (`--services`), signing and notarisation (735), and making
releases. Nothing of a release is compiled into a program; this
repository never holds a key, and makes no release.

## 7. What is not here

- Google, or any other search connector (§ 2).
- A second AI connector: OpenAI's Responses API is the one written. The
  agent's and the answer's wire formats (`agent_protocol.rs`,
  `answer_protocol.rs`) are OpenAI's and belong to its connector; another
  connector writes its own behind `AiConnector`.
- SearXNG's images and videos (907); a service changed in *Settings* used
  without a restart (908).
- A key for a SearXNG behind a password: SearXNG asks none of its own.

## Built (9 November 2026)

- **904** — the two interfaces; Brave and OpenAI behind them; SearXNG
  (`searxng_search.rs`, `searxng_protocol.rs`, added to `serde_json`'s
  files in `scripts/gate.sh`'s boundary list as every protocol read from a
  stranger is); `services.rs`, `services_made.rs`, `settings_file.rs`'s
  three lines; `service_keys.rs` and `keychain.rs`'s `find`, `keep`,
  `delete`; every answer, agent and refusal sentence naming *the AI
  service*.
- **905** — *Settings › Search and answers* (`settings_services.rs`,
  `settings_services_document.rs` in the renderer;
  `conversation_services.rs` in the browser process), the `Service` claim,
  its keys and fields.
- **906** — `alo-bundle --services FILE`; `--keys-from` per service.

Cut, and queued: 907 (a SearXNG's pictures), 908 (a change used at once),
909 (a field's caret moved, selected and placed by a press).

**Not verified against a live SearXNG**: see the journal (`STATE.md`,
batch 31) for what the hand run did and did not reach.

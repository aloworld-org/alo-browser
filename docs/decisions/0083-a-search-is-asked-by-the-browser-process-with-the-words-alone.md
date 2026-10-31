# ADR 0083 — A search is asked by the browser process, with the words alone

**Status:** accepted — built by queue item 708 (31 October 2026)
**Date:** 2026-10-31
**Context:** ADR 0082 § 0 (the owner chose the Brave Search API, and that in
release alo pays for it through a subscription); ADR 0005 (a renderer has no
network); ADR 0001 (rented crates behind named files — `serde_json` among
them); ADR 0030 (the browser says what it is and nothing about the machine);
ADR 0074 (the interface is a conversation, each part a document of ours);
`ROADMAP.md`, *Not built, and not by accident* — no search deal, no
telemetry; `docs/design/shell.md`, *What appears when a person types*; queue
item 708

## The question in one line

When a person chooses *Search for "…"*, which process asks Brave, with what
key, saying what, and what does the person see when Brave does not answer?

## 1. Where the key is held — for now

**In the macOS login keychain of the machine alo runs on**, as a generic
password: service `alo-brave-search`, account the person's login name
(`$USER`). The browser process reads it once as the conversation's window
opens, by running `/usr/bin/security find-generic-password -a "$USER" -s
alo-brave-search -w` and taking what it prints (`alo-window`'s
`search_key.rs`). No new `unsafe` and no new crate: the Security
framework's own tool is the boundary.

- The key is held in memory by a type whose `Debug` says only that a key is
  held; it is never written to a file, a log line, the terminal, the first
  screen's line or an error sentence. Nothing in the repository, its tests
  or its fixtures holds it.
- **With no key** — no item, the tool refusing, an empty answer — searching
  stays offered and says *Not set up yet* with why: *no Brave Search key in
  this Mac's keychain (alo-brave-search)*.
- **This road is temporary and for development.** It is read at run time,
  and the key is never put in the binary or the repository. An application
  without the item says it is not set up rather than pretending.

### In release — left open for the owner, decided before release

ADR 0082 § 0: in release alo pays the API through a subscription, so no
person brings a key of their own. Two ways to do that are on the table:

1. **The key shipped inside the application.** The owner's stated direction,
   31 October 2026: *"we shall need to ship the application with the keys."*
   **Its risk, plainly:** a key carried inside a shipped application can be
   read out of it by anyone who has the application — a program's bytes and
   its resources are the person's to read, and obfuscation slows this but
   does not stop it — and the key can then be used from anywhere, against
   alo's quota and on alo's bill, until it is revoked, which breaks search for
   every copy that carries it.
2. **A small server of alo's own** that holds the key and forwards each
   search to Brave, after checking the person's subscription. The key never
   leaves alo's server, a leaked subscription can be cut off alone, and it
   fits the subscription of ADR 0082 § 0 and ADR 0081's note. Its cost: a
   service to run, and a place every person's searches pass through, which
   must then keep no record of them (no telemetry) and say so.

**Not decided here.** The owner chooses before release, in an ADR of its
own; until then this item reads the key from the keychain at run time.

## 2. Which process asks

**The browser process**, on a thread of its own (`conversation_search.rs`),
never waited on by the window's event loop, through this repository's own
HTTP stack — `alo-net`'s `Pool` over `rustls`, trusting what the machine
trusts — and never through Chromium and never from a renderer. A renderer is
confined and has no network (ADR 0005); the first screen's renderer is sent
the results as data and draws them, and is never sent the key.

The address asked is fixed in the browser process
(`https://api.search.brave.com/res/v1/web/search`). Tests give the thread
another address — a server on `127.0.0.1` answering hand-written replies in
Brave's shape — through a Rust constructor (`Provider::at`); nothing a page,
a renderer, an agent, an argument or an environment variable says can change
where a search goes.

## 3. What is sent

- **The words typed, trimmed, as `q`**, percent-encoded — and nothing else
  in the address: no `count`, no `country`, no `search_lang`, no locale, no
  partner or referral code, no identifier of the person or the machine
  (ADR 0082 § 3, *no parameter but the words*).
- **Headers:** `X-Subscription-Token` (the key), `Accept:
  application/json`, and what `alo-net` sends with every request — `Host`,
  alo's own `User-Agent` (ADR 0030), and `Accept-Encoding`. No cookie (the
  pool's request carries none), no `Referer`.
- **Nothing before the person chooses.** Not while they type, not on focus,
  not to warm a connection. One search for one choice.
- **No telemetry.** alo counts nothing and records nothing of a search
  beyond the browser process's in-memory request record of the session,
  which every request has (ADR 0012) and which is never written to a disk.

What Brave keeps of a query is Brave's (its privacy policy, ADR 0082 § 2),
and is the one thing alo cannot make smaller than the words.

## 4. What is read back

Brave's JSON is a stranger's, so it is read in one file
(`search_protocol.rs`, the third file `serde_json` may be named in, beside
the agent's and Chrome's protocols — a third protocol, and so a third reason
to change). Its nesting is bounded before it is parsed, its size before
that, and every field is checked by name: the document must be an object of
`type` `search`; `web.results` an array; each result's `title` a string and
its `url` an `http`/`https` address. A result missing either is left out and
named (*the result at 3 has no url*); the document refused whole says why
(*Brave's answer is not JSON*, *its web results are not a list*). Nothing
here panics on any input.

**Brave writes a result's `title` and `description` as HTML text**: `&amp;`,
`&#x27;` and the like for characters, and `<strong>` around the words that
matched. alo shows them as plain text: `<strong>` and `</strong>` are taken
out, the named entities `&amp; &lt; &gt; &quot; &apos;` and numeric
references are read as the characters they name (a control character as
nothing), and anything else — `<b>`, a `&` that names nothing — is left as
the characters it is. The emphasis is not drawn: the design shows none. The
result is then a string like any other stranger's, and enters the first
screen's document as a text node, never as markup.

## 5. What a person sees when Brave does not answer

The All tab's row, then one sentence where the sites would be — never an
empty list that looks like *nothing was found*:

| What happened | What is said |
|---|---|
| 401 or 403 | *Brave Search refused alo's key (401). The key in this Mac's keychain may be wrong or revoked.* |
| 429 | *Brave Search says alo has searched too often, or its allowance is used up (429). Try again later.* |
| 5xx | *Brave Search could not answer just now (500).* |
| any other status | *Brave Search answered 422, which alo does not read.* |
| no connection | *Brave Search could not be reached: WHY* |
| an answer refused | *Brave Search's answer could not be read: WHY* |
| no sites | *Brave Search found no sites for "…".* |

None of them says the key. The same sentence is said in the first screen's
line at the window's foot.

## 6. What is not here

- The short answer at the top of All waits on the model (707, ADR 0081).
  Nothing stands in its place.
- The Answer, Sites, Images, Videos and News tabs are drawn, disabled, and
  claim nothing; *More sites ›* likewise. *(Since built: the row of images,
  the Images tab, the Sites tab and More sites ›, by queue items 743 and
  744 — what a search's images cost and where their pictures come from is
  ADR 0086.)* A site's favicon is not fetched —
  it would be one more request to Brave per result, of a picture — so a site
  shows its letter on one of four tints. Queued: images (743), the Sites tab
  and *More sites ›* (744), Videos (745), News (746), a site's own mark
  (747), and how the key is carried in release (748).
- How the key is carried in release — inside the application, or by a
  server of alo's own (§ 1): the owner's, undecided.

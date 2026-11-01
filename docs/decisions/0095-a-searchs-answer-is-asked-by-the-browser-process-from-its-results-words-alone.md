# ADR 0095 — A search's answer is asked by the browser process, from its results' words alone

**Status:** accepted — built by queue items 773 and 707 (1 November 2026)
**Date:** 2026-11-01
**Context:** ADR 0081 (the model alo thinks with — decided 1 November 2026:
OpenAI); ADR 0083 (a search is asked by the browser process, with the words
alone — this ADR's pattern); ADR 0091 (the providers' keys are carried
inside the application); ADR 0005 (a renderer has no network); ADR 0002 (the
layout tree is the agent's tree); ADR 0001 (rented crates behind named
files); `docs/design/shell.md` item 2 and *The answer*; `ROADMAP.md`, *Not
built, and not by accident* — no telemetry; queue items 707 and 773

## The question in one line

When a person's search has found sites, which process asks the model for an
answer, saying what, how is the answer checked before it is drawn, and what
does the person see when there is none?

## 1. Which process asks, and when

**The browser process**, on a thread of its own (`conversation_answer.rs`),
never waited on by the window's event loop, through this repository's own
HTTP stack — `alo-net`'s `Pool` over `rustls`, trusting what the machine
trusts — and never through Chromium and never from a renderer.

- **Once a search, after its sites.** The search's thread tells the window
  the sites, with the answer marked as being asked, and only then hands the
  person's words and the sites to the answer's thread. Nothing is asked
  while a person types, on focus, or to warm a connection; a search that
  found no sites asks no answer; Videos, News, Images and further pages ask
  none. A search's marks and images are never kept waiting for the model.
- **Newest wins.** Of the answers waiting when the thread is free, only the
  newest search's is asked; each older one is told, in its place, that alo
  did not ask because a newer search came first.
- **Patience** is 30 seconds; past it, the answer is said not had.

The address asked is fixed in the browser process
(`https://api.openai.com/v1/responses`), as is the model
(`openai_answer::MODEL`, the one place it is named). Tests give the thread
another address — a server on `127.0.0.1` answering hand-written replies in
OpenAI's shape — through a Rust constructor (`Provider::at`); nothing a
page, a renderer, an agent, an argument or an environment variable says can
change where an answer is asked or which model writes it.

## 2. The key

OpenAI's key is the **answer slot** among the carried keys (ADR 0091; queue
item 773): `Contents/Resources/keys/answer`, masked as the search key is,
put in by `alo-bundle --keys-from keychain|DIRECTORY` from the keychain item
`alo-openai` (or a file named `answer`) before the seal, and read by the
browser process alone (`answer_key::for_this_application`) — the
application's key first, this Mac's keychain otherwise, a damaged carried
key said and never swapped. `--keys-from` now needs both keys and refuses,
naming which, when either is missing. The key goes in one header,
`Authorization: Bearer …`; it is never in the body, a log, the terminal, a
chat's file or a sentence. With no key, the answer says *alo's answers are
not set up yet: no OpenAI key in this Mac's keychain (alo-openai).* where it
would be, and nothing is sent.

## 3. What is sent, and to whom

To **OpenAI**, at `api.openai.com`, over TLS, one `POST` per search:

- **The person's words, trimmed**, as the question.
- **The first eight sites the search found** (`MOST_SOURCES`), each as the
  number it is cited by (its place, from one), its **title**, its **site's
  name**, its **address** and the **description Brave gave** — all of it
  text Brave already returned for the person's search, and nothing else:
  no page is fetched in this cut, no extra snippets are asked of Brave (ADR
  0083 § 3: the words alone), no site's mark or picture.
- **alo's own instructions**, the model, `store: false` (OpenAI keeps no
  response for a later request), `reasoning.effort: low`,
  `max_output_tokens: 2000`, and the answer's JSON schema (strict).
- **Headers:** `Authorization`, `Content-Type: application/json`, `Accept:
  application/json`, and what `alo-net` sends with every request — `Host`,
  alo's own `User-Agent` (ADR 0030), `Accept-Encoding`. No cookie, no
  `Referer`, no `OpenAI-Organization` or `OpenAI-Project`; no `user`,
  `safety_identifier` or `metadata` in the body — nothing of the person or
  the machine.

The sites go in the person's message as JSON data, never in the
instructions, and the instructions say that a result's text may ask the
model to do something and is to be read, not obeyed (ADR 0081 § 5, *a page
is data*). What OpenAI keeps of a request is ADR 0081's § 0 (its abuse
monitoring logs, up to 30 days), and is the one thing alo cannot make
smaller than what it sends. **alo itself keeps nothing**: no count, no log
sent anywhere. The terminal's line says who answered, from how many sites,
and the tokens the response's `usage` reports — on the machine, for the
person running it.

## 4. What is read back, and how it is checked

OpenAI's JSON is a stranger's, so it is read in one file
(`answer_protocol.rs`, a fifth file `serde_json` may be named in —
`scripts/gate.sh`'s boundary list), which also writes the request: size (1
MiB) and nesting (32) bounded before it is parsed, then every field checked
by name. The response must be an object of `object` `response` and
`status` `completed` — `incomplete` says why it stopped, `failed` says so —
with a `message` in its `output` holding an `output_text`, or a `refusal`,
said as a refusal. That text is the answer to the schema: `answered`,
`line`, `sentences` and `points`, each sentence and point its `text` and the
`sources` it cites.

- **`answered: false` is *I couldn't find a reliable answer*,** whatever
  else it says: the model is told to say so when the results do not clearly
  answer, or disagree, and is allowed to.
- **Citations are checked by construction.** A written answer is refused
  whole unless it has a line and a sentence, every sentence and point
  cites at least one source, and **every number cited is one of the sources
  it was given** (`Written::check`). A refused answer is said in words —
  *it was refused: the answer cites source 9, and it was given 8* — and
  never drawn. Sentences past three and points past six are cut, not
  refused; a number cited twice in one sentence is kept once.
- The renderer refuses the same on the wire: an answer citing a number past
  the sites it was sent, an answer to a search that found no sites, more
  sentences or points than are drawn — a confined renderer never draws a
  citation of a site it was not shown.

## 5. What a person sees

Drawn by the first screen's renderer from plain structured text — the
line, the sentences, the points, each with its numbers — sent over the wire
like the sites (ADR 0005); every word enters the document as a text node.

- **All**: under the tab row, two or three sentences straight on the paper,
  each followed by its sources' numbers; *Full answer ›* (opening the
  Answer tab) and *Drew on N sites*; a line under it all; then the sites,
  each the answer drew on saying *alo used this*.
- **The Answer tab**, open once an answer is asked for: the answer in one
  line in the serif, its short points with their numbers, then *N sources*
  — counting only the sources cited — each by its number, site, address and
  title; then one quiet line saying what was sent to have it written, and
  to whom (ADR 0081 § 5): *Your question and what Brave Search said of its
  first 8 sites were sent to OpenAI to write this answer.* — said under *I
  couldn't find a reliable answer* too, which was sent the same. All says
  *Drew on N sites*, and not what was sent.
- **Where there is no answer**, one sentence in its place, on both:

| What happened | What is said |
|---|---|
| asked, not yet answered | *alo is writing an answer from these sites…* |
| `answered: false` | *I couldn't find a reliable answer. The sites Brave Search found for "…" are below.* (on the Answer tab: *are on the Sites tab.*) |
| no key | *alo's answers are not set up yet: no OpenAI key in this Mac's keychain (alo-openai).* |
| 401 or 403 | *alo could not write an answer: OpenAI refused alo's key (401). The key in this Mac's keychain may be wrong or revoked.* (or *the key this application carries may have been revoked*) |
| 404 | *alo could not write an answer: OpenAI does not offer alo's model, gpt-6.1-terra, to its key (404).* |
| 429 | *alo could not write an answer: OpenAI says alo has asked too often, or its allowance is used up (429). Try again later.* |
| 5xx | *alo could not write an answer: OpenAI could not answer just now (503).* |
| no connection, or past 30 s | *alo could not write an answer: OpenAI could not be reached: WHY* |
| an answer unreadable, refused, or citing a source not given | *alo could not write an answer: OpenAI's answer could not be read: WHY.* |
| a newer search came first | *alo did not ask OpenAI to answer "…": a newer search came first.* |

None says the key. **Never a fake answer**: nothing is drawn in an answer's
place but these sentences.

## 6. What an agent reads

ADR 0002: a region named *alo's answer* — on All, a paragraph of the
sentences with their numbers as text, *Full answer ›* a button, *Drew on N
sites*, and *alo used this* in each site's article it drew on; on the
Answer tab the line, the points as a list, and a region named *Sources*
holding each source's number, name, address and title. *Full answer ›* is
claimed by its place among those drawn and decided by the browser process
as *More sites ›* is.

## 7. What is not here

- **The exact line on hover** (812): the sources given are descriptions,
  not pages, so the line a sentence rests on is the description itself;
  showing it beside the answer is cut.
- **A source opened as a card, scrolled to the line used** (813).
- **The answer kept in a chat's file** (814): a chat read back shows its
  sites with no answer, and the Answer tab closed.
- **Streaming** (815): the answer is drawn whole when it comes.
- **Richer grounding** — Brave's extra snippets, or the pages' own text
  (816): each costs a decision of what more leaves the machine.
- **The evaluation ADR 0081 § 4 asks for** (817), and **zero data
  retention** at OpenAI (818), the owner's to ask for.
- **The agent (704)**, which is not asked of any model here.

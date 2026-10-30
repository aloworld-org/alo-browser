# ADR 0081 — The model alo thinks with

**Status:** proposed — awaiting the owner's choice. The owner has said the AI
is connected **last**, after the rest of the conversation is built; this ADR
is written now so that choice is ready when that time comes, and so nothing
built before it closes a door it needs.
**Date:** 2026-10-30
**Context:** ADR 0002 (the layout tree is the agent's tree; no verb takes a
coordinate; the agent never receives a screenshot); ADR 0052 (the agent's
gate and the person's gate); ADR 0074 (the browser's own interface is a
conversation) and `docs/design/shell.md` (*The answer*, *The agent at
work*); ADR 0077–0079 (alo carries CEF's Chromium, on macOS, Windows and
Linux); ADR 0082 (where alo searches); `docs/contracts/agent-socket.md`
(`read` and `act`); `ROADMAP.md`, *Not built, and not by accident* (no
telemetry of any kind); queue items 704 and 707

## The question in one line

Which language model writes alo's answers (707) and drives alo's agent on a
page's tree (704) — where it runs, who pays for it, and what leaves the
person's machine when it does?

## Why this is a decision rather than a dependency

Three things follow from the choice that are not taste:

1. **What leaves the machine.** An answer sends the text of the pages it
   draws on; the agent sends the tree of every page it acts on — names,
   field values, a basket's contents, an address. Whoever runs the model
   receives that. alo's promise is *no telemetry, none*; a model that keeps,
   trains on or logs what it is sent would be telemetry by another name.
2. **Who pays, and whether alo has a server.** Today alo has no server at
   all. If alo pays for the model, alo needs one — to hold the key, count use
   and bill — and that server sees every page anybody's agent reads.
3. **What the code is.** Law 4 is one language, Rust, and `unsafe` only
   behind an ADR. A hosted model is HTTPS and JSON, which alo already speaks.
   An on-device model is either a Swift framework (an FFI boundary, so
   `unsafe`, so an ADR like 0080) or a Rust inference engine carried in alo.

## What the two jobs need

**(a) An answer with exact citations (707).** The design wants *a small
number after each sentence a source supports*, and hovering it shows *the
exact line*. That rules out a model that writes citations as free text: a
model can invent a quotation. What is needed is citations **checked by
construction** — the model points at a passage alo gave it, and alo shows
the passage, not the model's retelling of it.

**(b) The agent acting on a tree (704).** The model reads `read`'s tree
(roles, names, states — a few thousand tokens for an ordinary page; the
corpus's `alo-sites-full-page/agent.txt` is 10 KB, a shop's checkout is
larger) and answers with one typed verb — `activate`, `put_text`, `scroll`
— on a target named by name or role. That is tool use: many turns, the
tree growing the context each turn, and a sentence for the person per step
(*"Choosing Saturday, 10:00 — 8:00 is taken"*). It needs a model that is
good at multi-step tool use, that does not wander, and that stops and asks.

### Where an answer's sources come from

ADR 0082 (proposed) has alo open the search engine a person chose, as its
own page, rather than buy results from a search API. So the sources an
answer cites have to come from somewhere, and there are three ways, each
the owner's to weigh:

1. **The pages the person opened.** *Ask alo about this page*, or about the
   pages open in the chat. Nothing is fetched that the person did not open.
   Narrowest, and the cleanest.
2. **The engine's results the person is looking at.** alo reads the results
   page's tree (the person searched; the page is open) and opens the first
   few results itself to read them. No fee — but it is alo, not the person,
   loading those result pages. Google's terms forbid *"using automated
   means to access content"* against `robots.txt`, and `/search` is
   disallowed on Google, Bing and Brave Search (ADR 0082 § 6). Reading the
   one results page the person asked for and is looking at is a grey zone,
   not read by any engine's lawyers; alo **issuing its own queries** and
   reading the results by program is plainly on the wrong side, and is the
   line to stay behind.
3. **The model provider's own search.** Anthropic's `web_search` server
   tool costs **$10 per 1,000 searches** plus the tokens of what it returns
   [1], and runs on Anthropic's servers: the query leaves for Anthropic, not
   for the engine the person chose. At 2–3 searches an answer, $0.02–0.03 an
   answer, more than Haiku's whole answer. It is a paid search by another
   road, and against the owner's *we are just a browser*.

Recommended: **(1) first; (2) only after the engines' terms are read for
it**, and then only on a results page the person opened, opening result
pages the person can see listed — never alo issuing its own queries to an
engine in the background. (3) is set aside.

## 1. The options (read 30 October 2026)

### A. Anthropic's Claude, through the API

Model IDs and prices from Anthropic's pricing page [1], per million tokens
(MTok):

| Model | ID | Input | 5-min cache write | Cache read | Output |
|---|---|---|---|---|---|
| Claude Fable 5.1 | `claude-fable-5-1` | $10 | $12.50 | $0.25 | $50 |
| Claude Opus 5.5 | `claude-opus-5-5` | $4 | $5 | $0.20 | $20 |
| Claude Sonnet 5.5 | `claude-sonnet-5-5` | $2 | $2.50 | $0.10 | $10 |
| Claude Haiku 5.5 (prompts ≤ 100k tokens) | `claude-haiku-5-5` | $0.10 | $0.125 | $0.01 | $0.50 |
| Claude Haiku 5.5 (prompts > 100k tokens) | | $0.50 | $0.625 | $0.05 | $2.50 |

The Batch API halves these, but an answer a person is waiting for cannot be
batched. Every model has a 1M-token context (Haiku 5.5's price rises past
100k) [1]. `inference_geo: "us"` costs 1.1× [1].

- **Citations by construction.** The Messages API accepts *search result*
  content blocks — a source, a title and the text as a list of blocks — and
  Claude's answer comes back as text blocks each carrying citations that name
  the result and the **block indices** it cited, with `cited_text` being
  exactly the text of those blocks [2]. Claude *"cites whole blocks, not
  substrings within a block"* [2], so alo gives each source **one block per
  line or sentence**, and the hover shows that block, word for word. It is
  GA, needs no beta header, works on every current model (Haiku 5.5
  included), and `cited_text` is *"not counted toward output tokens"* [2].
  This is exactly the design's *the exact line*.
- **Tool use** is the Messages API's own: alo defines `read_page`, `act`
  (`activate` / `put_text` / `scroll`, target by name or role — never a
  position) and `ask_the_person` as client tools; the model asks, alo runs
  the verb through the same `act` path the agent socket uses, and alo's own
  gates (ADR 0052, `alo-grants`) decide before anything is done. `strict:
  true` keeps the arguments to the schema. Forced tool choice is refused on
  the 5.5 models [skill], which does not matter here: the loop is `auto`.
- **Screenshots are not needed and not sent.** Claude has a computer-use
  tool; ADR 0002 refuses it. The tree is the input.
- **Prompt caching** makes the agent's long, growing context cheap: a cache
  read costs 0.05× input on Opus and Sonnet 5.5, 0.1× on Haiku 5.5 [1], and
  the minimum cacheable prefix on the 5.5 models is 512 tokens [skill].
  Context editing can clear old trees from the conversation so a twelve-step
  task does not carry eleven stale pages.
- **Data.** Anthropic's commercial policy: inputs and outputs are deleted
  *"within 30 days of receipt or generation"*, longer when flagged by trust
  and safety (*"up to 2 years"*) or required by law [3]. Its retention page:
  *"Retained data is never used for model training without your express
  permission"* [4]. **Zero data retention** is available per organization
  by request to sales, covers the Messages API, citations, search-result
  blocks and prompt caching, and **does not cover Fable 5.1**, which
  requires 30-day retention [4][2].
- **Rust.** Anthropic publishes no Rust SDK. alo calls `POST /v1/messages`
  over HTTPS with JSON and reads the server-sent events of a streamed answer
  itself — the API's raw HTTP shape, through `alo-net` and the TLS it already
  rents. No `unsafe`, no new language.
- **Platforms:** all three; it is a web API. **Offline:** no.

### B. Other hosted providers

| Provider and model | Input / MTok | Cached input | Output / MTok | Training on API data | Retention |
|---|---|---|---|---|---|
| OpenAI `gpt-6.1-sol` [5] | $2.00 | $0.10 | $10.00 | not by default (*"not used to train or improve OpenAI models"*) [6] | abuse logs *"up to 30 days"*; ZDR by OpenAI's prior approval [6] |
| OpenAI `gpt-6-luna` [5] | $0.10 | $0.01 | $0.50 | as above | as above |
| OpenAI `gpt-6-astra` [5] | $10.00 | $1.00 | $50.00 | as above | as above |
| Google `gemini-4-argon` [7] | $2.00 (→ $4.00 from 1 Dec 2026) | $0.10 (→ $0.20) + storage | $10.00 (→ $20.00) | paid tier: **no**; free tier: **yes** [7] | not read |
| Google Gemini 3.8 Flash [7] | $0.75 (→ $1.50 from 1 Jan 2027) | $0.075 + storage | $3.75 (→ $7.50) | as above | not read |
| Mistral Large [8] | $0.50 | not read | $1.50 | not read | not read |

All three offer tool use and some form of grounding. None was read for a
citation feature equal to Anthropic's search-result blocks (cited text
guaranteed to be the provided text); Gemini's search grounding is its own
Google search at $14 per 1,000 after 5,000 free a month [7], which is a
search deal by another road (ADR 0082). Prices on the two Google models
**double within three months** of today [7]. Each is a different HTTP API:
choosing two providers means two clients, two prompt sets and two sets of
tests.

### C. On the device

**Apple's Foundation Models framework (macOS).** Apple's own on-device
model, free, and private by construction. Apple's update notes [9]: a newer
model with better tool calling in February 2026 (macOS 26.4); a
`contextSize` property, with Xcode's playground estimating against **4,096
tokens**; in June 2026, a newer model again with macOS 27, a
`PrivateCloudComputeLanguageModel` for *"a larger context size"*, and a
Python SDK. Against alo's needs:

- 4,096 tokens (if that is still the size — newer figures in secondary
  sources disagree, 8K and 32K [10]) holds **one** small page's tree, not
  five sources and not a twelve-step task.
- It is a **Swift** API. Reaching it from Rust is an Objective-C/Swift
  bridge: an FFI boundary, `unsafe`, and an ADR of its own like 0080.
- **macOS only**, and only on Macs with Apple Intelligence. Windows and
  Linux have nothing like it. Apple's own Linux plans were reported, not
  read.

**Small open-weight models, run by alo.** A 4–8 B parameter model quantised
to 4 bits is about 2.5–5 GB of memory. Rust engines exist that run them on
Apple's GPU and on CPUs: `mistral.rs` (Metal, tool calling, v0.9.1 in
August 2026, per a secondary listing [11]) and Hugging Face's `candle`. So
a Rust-only path exists: the engine is a crate like any other we rent, and
the `unsafe` it holds is inside a rented dependency, not in this
repository. Against alo's needs:

- **On the owner's Mac (8 GB)** it shares memory with CEF's Chromium, the
  conversation's renderers and macOS. A 4 B model at 4 bits is feasible; an
  8 B model is not, comfortably. On a typical 16 GB Windows or Linux laptop
  both fit. Speed on a laptop GPU or CPU was **not measured**.
- **Quality.** Small models' tool-use scores in public write-ups (BFCL in
  the 60s per cent for 4 B-class models [12]) are self-reported, from
  different benchmark versions, and not alo's task. Multi-step driving of a
  real checkout is where small models are weakest. Not measured here.
- **Citations** would have to be built by alo: ask for block numbers, then
  check them. Possible, and checkable, because the blocks are alo's.
- **Privacy:** nothing leaves the machine. **Offline:** yes. **Cost:** none
  per use; a 2–5 GB download, carried or fetched once.

### D. Who pays

- **The person's own key (BYOK).** The person makes an account with the
  provider, pastes a key into *Settings*, and pays the provider directly.
  Requests go **from the person's machine straight to the provider**; alo
  has no server, sees nothing, counts nothing. The person is the provider's
  customer under its commercial terms (A's and B's training and retention
  terms above apply to them). The key is kept in the system's keychain, never
  in a file. Cost to alo: none. Cost to the person: friction — an account, a
  card, a key.
- **alo pays and passes the cost on.** alo would need a server to hold its
  key (a key in the application is a key anyone can take), accounts,
  payment, rate limits per person, and abuse handling — and that server sees
  every page sent through it. It can be built without logging, but then the
  promise *no telemetry* rests on alo's server being as it says, which a
  person cannot check. alo would also be the provider's customer for every
  person's use. This is a business, not a setting.

## 2. What an answer and a task cost

Assumptions, stated so they can be argued with (none measured; token counts
are estimates for the 5.x tokenizer):

- **An answer (707):** a fixed prompt and tool definitions of 2,000 tokens
  (cached after the first), five sources of about 3,000 tokens of text each
  (15,000), a question of 50; out: an answer of about 400 tokens plus about
  300 of thinking at low effort — **15,050 in, 700 out**.
- **A task (704):** twelve steps; a fixed prompt of 3,000 tokens; each step
  reads a pruned tree of about 6,000 tokens and writes about 550 (the
  person's sentence, the verb, brief thinking). With caching, each step reads
  everything before it from the cache and writes only its own 6,550:
  - cache writes: 12 × 6,550 = **78,600**
  - cache reads: Σ over steps k = 0…11 of (3,000 + 6,550 k) = 36,000 +
    6,550 × 66 = **468,300**
  - output: 12 × 550 = **6,600**

| Model | One answer | One task (cached) | Arithmetic for the task |
|---|---|---|---|
| Haiku 5.5 | **$0.0019** | **$0.018** | 78,600 × 0.125 + 468,300 × 0.01 + 6,600 × 0.50, ÷ 10⁶ |
| Sonnet 5.5 | **$0.037** | **$0.31** | 78,600 × 2.50 + 468,300 × 0.10 + 6,600 × 10, ÷ 10⁶ |
| Opus 5.5 | $0.074 | $0.62 | 78,600 × 5 + 468,300 × 0.20 + 6,600 × 20, ÷ 10⁶ |
| Fable 5.1 | $0.19 | $1.43 | 78,600 × 12.50 + 468,300 × 0.25 + 6,600 × 50, ÷ 10⁶ |
| `gpt-6.1-sol` (cached input $0.10, no write premium) | $0.037 | $0.27 | 78,600 × 2 + 468,300 × 0.10 + 6,600 × 10, ÷ 10⁶ |
| Gemini 3.8 Flash (to 31 Dec 2026, storage ignored) | $0.014 | $0.12 | 78,600 × 0.75 + 468,300 × 0.075 + 6,600 × 3.75, ÷ 10⁶ |
| On the device | $0 | $0 | electricity and memory |

Answer arithmetic, Sonnet 5.5: 15,050 × $2 + 700 × $10 = $0.0301 + $0.0070
= $0.037 (the cached 2,000 add $0.0002). Haiku 5.5: 15,050 × $0.10 + 700
× $0.50 = $0.0015 + $0.00035.

**Without caching** the same Sonnet task costs (78,600 + 468,300) × $2 +
6,600 × $10 = $1.09 + $0.07 = **$1.16**, nearly four times as much. Caching
is not an optimisation for alo's agent; it is the price.

**Per active person per month**, at 10 answers and 2 tasks a day for 30
days (300 answers, 60 tasks):

| Choice | Answers | Tasks | Month |
|---|---|---|---|
| Haiku 5.5 for both | 300 × $0.0019 = $0.57 | 60 × $0.018 = $1.08 | **≈ $1.65** |
| Haiku 5.5 answers, Sonnet 5.5 agent | $0.57 | 60 × $0.31 = $18.60 | **≈ $19** |
| Sonnet 5.5 for both | 300 × $0.037 = $11.10 | $18.60 | **≈ $30** |
| Opus 5.5 for both | $22.20 | $37.20 | ≈ $59 |

**Latency** was not measured for any of them; streaming shows the answer's
first words before it is finished, which is what a person sees.

## 3. The options against each other

| | Exact citations | Agent on a tree | Cost / month (above) | What leaves the machine | Offline | Platforms | Rust, no new `unsafe` |
|---|---|---|---|---|---|---|---|
| A. Claude, BYOK | **by construction** (search-result blocks) | strong; Sonnet/Opus made for multi-step tool use | $1.65–30 to the person | the pages and trees sent; deleted within 30 days; not trained on; ZDR on request | no | all | **yes** |
| B. OpenAI / Gemini, BYOK | built by alo (not read) | strong (not measured here) | similar | similar terms; Gemini free tier trains | no | all | yes |
| C1. Apple Foundation Models | built by alo | weak: 4K context | $0 | nothing | yes | **macOS only** | **no** (Swift bridge) |
| C2. Small open model, Rust engine | built by alo | weak to fair (not measured) | $0 | nothing | yes | all, memory permitting | yes (unsafe in a dependency) |
| D. alo pays | as A or B | as A or B | alo's, recovered somehow | as A or B, **plus alo's server** | no | all | yes, plus a server |

## 4. Recommended (for the owner to decide)

**Default: Claude through Anthropic's API, with the person's own key.**
**Claude Haiku 5.5 writes answers; Claude Sonnet 5.5 drives the agent;**
Opus 5.5 is offered in *Settings* for a person who wants it.

- It is the only option read here whose citations are **exact by
  construction**: the hover shows the block alo gave it, never the model's
  paraphrase. That is 707's whole promise.
- Haiku 5.5 is cheap enough (≈ $0.002 an answer) that an answer costs a
  person nothing they would notice; Sonnet 5.5 is the model to trust with a
  form, at ≈ $0.31 a twelve-step task, cached. Both numbers are estimates
  until 707 and 704 measure real ones from `usage`.
- BYOK means **alo has no server and sees nothing**, which keeps *no
  telemetry* true by construction rather than by policy. A person's data is
  under Anthropic's commercial terms: kept 30 days, not trained on, ZDR
  available. Fable 5.1 is not the default because it cannot be used under
  ZDR and costs five times Sonnet.
- It is HTTPS and JSON from Rust — no new language, no `unsafe`, all three
  systems on the day.
- The choice is behind one interface in alo (a *model* trait: send a
  conversation with tools and sources, receive text, citations and verbs),
  so B can be added as a second provider later without the conversation
  noticing.

**Fallback: a small open-weight model run on the device by a Rust engine**,
offered as *private and offline*, with its limits said plainly — answers
from fewer sources, the agent only on simple pages — and only once it has
been measured on alo's own frozen pages. It is the answer for a person with
no key, no network, or no wish to send anything. Apple's framework is not
the fallback: macOS only, a 4K context, and a Swift bridge.

**Who pays, later.** BYOK is enough while alo stays on the owner's
machines (ADR 0077). Before alo is offered widely, *alo pays* becomes a
question again — and if it is answered yes, the server it needs gets its own
ADR, with what it keeps written down.

**Before any choice is fixed:** an evaluation on alo's own material — 50
questions with known answers over frozen pages, and 20 tasks on the frozen
web cases (`web-a-form` and its kind) — run on Haiku 5.5, Sonnet 5.5 and
the best small local model, scored for correct citations, correct verbs,
stopping when it should, and cost from `usage`. The estimates above become
measurements there.

## 5. What must hold, whatever is chosen

- **Nothing about a page leaves without the person knowing.** An answer
  says which pages it sent (*Sent 5 pages to Claude*); the agent's column
  says the page it is reading is being sent. A page a person merely has open
  is never sent unless they asked about it or asked the agent to act on it.
- **No telemetry.** alo sends nothing to anyone but the model provider, and
  to it only the request. No analytics, no usage counting by alo, no crash
  reports carrying a conversation.
- **Passwords never leave.** A password field's value is never in the tree
  alo sends (its name and that it is filled, nothing else), and the agent
  never types one — signing in is the person's step (`shell.md`).
- **The gates are alo's, not the model's.** Paying is always asked;
  sending, deleting and cancelling are asked unless a person said otherwise.
  The model can propose a verb; alo's gate decides, and a model cannot talk
  its way past it.
- **A page is data, never instructions.** Page text and trees go in tool
  results and search-result blocks, never in the system prompt; the model is
  told that a page may try to instruct it, and alo's gates do not depend on
  it noticing.
- **No screenshot is ever sent** (ADR 0002). No verb takes a coordinate.
- **The key** lives in the system's keychain, is sent only to the provider,
  and is never written to a log or a file.
- **The person can see the cost**: tokens and price per answer and per task,
  from the API's own `usage`, shown on request.

## What is not verified, and is not hidden

- Every token count and cost above is an **estimate**; no request was sent
  to any model for this ADR. A real tree's size on real shops, and how many
  steps a real task takes, are unmeasured.
- Latency of every option, and the speed of a small model on the owner's
  Mac, were not measured.
- Quality: no model was run on alo's trees. Claims of strength in tool use
  are the providers' and the public benchmarks', not alo's.
- Apple's on-device context size on macOS 27 (4,096 per Apple's February
  2026 note; 8K or 32K per secondary sources [10]); whether
  `PrivateCloudComputeLanguageModel` is open to third-party apps on macOS
  and at what cost.
- OpenAI's and Google's citation features were not read; Mistral's prices
  beyond one line on its page, and its data terms, were not read; Gemini's
  paid-tier retention was not read.
- `mistral.rs`'s current release and Metal support come from a secondary
  listing [11], not its repository.
- Whether Anthropic's API terms let a desktop application ship a
  *bring-your-own-key* setting for its users — expected, since the key is
  the person's own, but the terms were not read for it.

## Sources (read 30 October 2026)

1. Anthropic, *Pricing* — https://platform.claude.com/docs/en/about-claude/pricing
2. Anthropic, *Search results* (citations from search-result blocks; GA;
   ZDR-eligible except Covered Models) —
   https://platform.claude.com/docs/en/build-with-claude/search-results
3. Anthropic Privacy Center, *How long do you store my organization's data?*
   (last updated 1 July 2026) —
   https://privacy.claude.com/en/articles/7996866-how-long-do-you-store-my-organization-s-data
4. Anthropic, *API and data retention* (ZDR scope; Covered Models) —
   https://platform.claude.com/docs/en/manage-claude/api-and-data-retention
5. OpenAI, *Pricing* — https://developers.openai.com/api/docs/pricing
6. OpenAI, *Your data* — https://developers.openai.com/api/docs/guides/your-data
7. Google, *Gemini Developer API pricing* (last updated 2026-10-28) —
   https://ai.google.dev/gemini-api/docs/pricing
8. Mistral AI, *Pricing* — https://mistral.ai/pricing
9. Apple, *Foundation Models updates* —
   https://developer.apple.com/documentation/updates/foundationmodels
10. Secondary write-ups on WWDC 2026's Foundation Models (context size) —
    https://dev.to/hariharanjagan/whats-new-in-apples-foundation-models-framework-at-wwdc-2026-5227,
    https://dev.to/owen_fox/apples-third-generation-foundation-models-a-developers-read-on-wwdc-2026-2ej1
11. `mistral.rs` (repository, and a secondary listing of v0.9.1) —
    https://github.com/EricLBuehler/mistral.rs, https://www.everydev.ai/tools/mistral-rs
12. Secondary write-ups on small models' tool calling (BFCL) —
    https://llm-stats.com/leaderboards/best-ai-for-tool-calling,
    https://localaimaster.com/blog/best-ollama-models-tool-calling

[skill]: Anthropic's `claude-api` reference as carried by Claude Code
(cached 2026-10-06): forced `tool_choice` refused on the 5.5 models; 512-token
minimum cacheable prefix on them.

# ADR 0082 — Where alo searches

**Status:** accepted — the owner chose on 30 October 2026: **open-source
search engines and search APIs together** (§ 0 below). Sections 1–6 stand as
the free road alo keeps beside them; § 7 is no longer set aside.
**Date:** 2026-10-30
**Context:** ADR 0074 (the browser's own interface is a conversation) and
`docs/design/shell.md` (*What appears when a person types*: the Sites tab
first, then Answer, Images, Videos, News); ADR 0075 and 0076 (a page chosen
in the conversation opens in Chromium's own window, drawn in the
conversation's card); ADR 0066 (no window of ours goes over the page); ADR
0073 (what Chromium sends of its own is named); ADR 0077–0079 (alo carries
CEF's Chromium); ADR 0081 (the model alo thinks with); `ROADMAP.md`, *Not
built, and not by accident* — **no search deal** (*"The address bar's
default is decided for the person using it, not sold"*) and **no
telemetry**; queue item 708

## 0. The owner's choice

After this ADR was first written (as *the engine's own page, no API*), the
owner weighed it against the design — whose Sites, Images, Videos and News
tabs are alo's own list, in alo's look, which only results *as data* can
draw — and against open-source engines, and chose **both kinds of source,
together**:

1. **alo's own results list, drawn as the design draws it, from search APIs:**
   - **Brave Search API** for everyday quality — its own independent index,
     web, images, videos and news (§ 7; about $5 per 1,000 queries when read).
   - **Open-source engines that offer their results as data**, each only
     once it is confirmed to be **maintained and running**, its API's terms
     read, and never used against them. Candidates to check: Mwmbl and
     Marginalia. **Not Stract:** its code (AGPL-3.0, Rust) was archived by its
     owner on 2 April 2026 and is read-only, and stract.com on 30 October
     2026 no longer showed a search engine or an API — read from GitHub and
     the site that day.
2. **The engine's own page stays** (§§ 1–6): every list ends in *See on …*,
   opening the person's chosen engine — Google, DuckDuckGo, any of
   § 2's — in the card, free and with no key.
3. **No search deal, unchanged.** Paying a provider for a service is not a
   deal: alo is never paid by a search company to send people to it, never
   adds partner or referral codes, and the person's choice of engine is
   theirs (§ 3).
4. **The agent never searches on its own** (§ 6) — through an API or a page.

**And no search engine of alo's own** — the owner, the same day, after
weighing what building one takes (crawling, storing and ranking billions of
pages, kept fresh, against spam): *"Let's use apis."* alo's results come
from search APIs, the Brave Search API first; an open-source engine is added
only as another API, once it is maintained and running. Private search of a
person's own pages, on their own machine, is a separate idea, not decided
here.

What is left open, and decided when this is built (with the AI, last):
who pays for the API's queries — **decided by the owner the same day: a
monthly subscription**, so alo pays the API and the person needs no key of
their own (the price, a free allowance, and how a subscription is sold and
checked are open; see ADR 0081's note on what a subscription needs);
which open-source engines are maintained and running, and their API terms; and the order and blend of results when
more than one source answers.

## The question in one line

When a person types words that are not an address, where does alo send
them, how is that place chosen, and what do the design's Sites, Images,
Videos and News tabs become?

## The owner's direction

> *"We shall use Google or other search engines — we are just a browser,
> not a search engine."*

So alo does what every browser does: it sends what was typed to **the
search engine the person chose**, at that engine's own results address, and
shows **the engine's own results page** — a real page, in Chromium, in the
conversation's card. No search API, no key, no fee per search, no index of
ours, and no money from any engine.

## Why this is still a decision

- **Which engine is first is the one thing a search deal buys.** Browsers
  are paid for their default. alo refuses that, so the first engine has to
  be chosen some other way, and *by whom* is the decision.
- **The design drew results alo lays out itself** — each result's site
  mark, name, address, a two-line description, and *alo used this* on the
  ones an answer drew on. An engine's own page is not ours to draw on (ADR
  0066). Parts of the design change.
- **What leaves the machine** differs by engine: some keep the query and
  the address it came from, some say they keep neither.

## 1. What alo does

1. A person types `best heat pump for a small flat` and chooses *Search
   with ENGINE for "…"* (the box's second offer, `shell.md` § 701).
2. alo percent-encodes the words into the engine's results template —
   `https://duckduckgo.com/?q={searchTerms}` — and opens that address as a
   page, exactly as *Go to the site* opens an address (ADR 0075): in
   Chromium's own window, drawn in the conversation's card (ADR 0076).
3. The page is the engine's. A person reads it, scrolls it, follows a
   result; the agent reads its tree like any page (ADR 0002). Following a
   result opens that site in the same card, with *back* to the results.
4. Nothing is sent until the person chooses the offer. Nothing is sent
   while they type.

Because the page is opened at the top level of Chromium's window — never in
an `<iframe>` of ours — an engine's `X-Frame-Options` (Google answers
`SAMEORIGIN`, measured below) does not stand in the way. It is simply a page.

## 2. The engines (read 30 October 2026)

Results templates are each engine's own **OpenSearch description** where it
publishes one — the machine-readable statement of *"send a search here"*
that browsers read — fetched today [1]. The image, video and news addresses
are the engines' own pages, each fetched once with `curl` from this Mac to
see that it is served (**200**) or what stood in the way. A browser gets
past what `curl` does not (a bot check, a consent page), so a non-200 is not
a refusal; it is not yet measured in alo.

| Engine | Index | Results (template) | Images | Videos | News | Account |
|---|---|---|---|---|---|---|
| **Google** | own | `https://www.google.com/search?q={q}` (no OpenSearch file served) | `&udm=2` — 200 | `&udm=7` — 200 | `&tbm=nws` — 302 to Google's EU **consent page** | no |
| **Bing** | own | `https://www.bing.com/search?q={q}` — 200 | `/images/search?q=` — 200 | `/videos/search?q=` — 200 | `/news/search?q=` — 200 | no |
| **DuckDuckGo** | mostly others' (not stated on its privacy page) | `https://duckduckgo.com/?q={q}` [1] | `&ia=images&iax=images` — 202 | `&ia=videos&iax=videos` | `&ia=news&iar=news` | no |
| **Brave Search** | **own** (*"independent of other search engines"* [4]) | `https://search.brave.com/search?q={q}` [1] | `/images?q=` — 200 | `/videos?q=` — 200 | `/news?q=` — 200 | no |
| **Ecosia** | others' (not re-read) | `https://www.ecosia.org/search?q={q}&addon=opensearch` [1] | `/images?q=` — 403 to `curl` | `/videos?q=` — 403 | `/news?q=` — 403 | no |
| **Startpage** | Google's results (not re-read) | `https://www.startpage.com/sp/search?query={q}` — 200 (no OpenSearch file found) | `&cat=images` — 200 | `&cat=video` (not fetched) | `&cat=news` (not fetched) | no |
| **Qwant** | own and others' (not re-read) | `https://www.qwant.com?q={q}&client=opensearch` [1] | `&t=images` — 200 | `&t=videos` (not fetched) | `&t=news` (not fetched) | no |
| **Mojeek** | **own** (not re-read) | `https://www.mojeek.com/search?q={q}` [1] | `&fmt=images` — 200 (that it shows images not checked) | none known | `&fmt=news` — 200 | no |
| **Kagi** | own and others' | `https://kagi.com/search?q={q}` [1] | `/images?q=` — 302 to sign-in | `/videos?q=` — 302 | `/news?q=` — 302 | **paid subscription** |

`udm=2`, `udm=7` and `tbm=nws` are Google's own addresses for its tabs, used
by every browser and by Google's page itself, but not documented by Google
as an interface; they can change. Google's `udm=14` (*Web*, plain links
without its AI summary) also answered 200 and is worth offering as the
Sites tab's Google address.

### What each says it keeps (privacy)

- **Google, Bing:** the query, the address, cookies; used for ads and
  personalisation under their policies (not re-read for this ADR). In the
  EEA, Google first shows a consent page (`curl` was sent to
  `consent.google.com … gl=BE` from this machine).
- **DuckDuckGo:** *"We don't save or share your search, chat, or browsing
  history"*; IP addresses not logged to disk in a way tied to a person;
  anonymous queries kept to improve results; ads through Microsoft's
  network (last updated 6 March 2026) [3].
- **Brave Search:** keeps *"de-identified search query data"*; IPs
  processed for integrity and *"deleted within seconds"*; results from its
  own index, with an optional *Google Fallback Mixing* [4].
- **Startpage, Ecosia, Qwant, Mojeek, Kagi:** each states a privacy-first
  position; their pages were not read today (Startpage's refused the
  fetch), so nothing about them is claimed here.

## 3. How the first engine is chosen

**Recommended: the person chooses, on first use, from an unweighted list,
and alo is paid by none of them.**

- The first time a person searches, before anything is sent, the
  conversation asks: *Which search engine should alo use?* with the engines
  above, **in an order shuffled each time it is shown**, each with one line
  saying what it is (*own index*, *Google's results without tracking*,
  *paid, no ads*…), and nothing pre-selected. Return chooses nothing; a
  choice must be made. This is how the EU's Digital Markets Act choice
  screens work in Chrome on desktop for the EEA [5], applied everywhere.
- The answer is kept; *Settings* changes it at any time; the box's offer
  says which engine it will use (*Search with Brave for "…"*).
- **alo adds nothing to the address but the words.** No `client=`, no
  partner code, no referral tag — the parameters through which engines
  count, and pay, a browser's traffic. Where an engine's own template carries
  one (`client=opensearch` at Qwant, `addon=opensearch` at Ecosia), alo
  leaves it out, so no engine can count alo and nobody can say alo has a
  deal. *No search deal* becomes a test: every engine's address is pinned in
  the repository and a test fails if a parameter other than the query
  appears.
- **No search suggestions while typing.** Each engine publishes a
  suggestions address (DuckDuckGo's `/ac/`, Brave's `/api/suggest` [1]);
  using it sends every keystroke to the engine. Off, and not offered until
  a person asks for it in *Settings*, with what it sends said beside it.

**The alternative**, if the owner prefers no question on first use: a
default the owner chooses for reasons written here (for example Brave
Search or Mojeek, for having their own index and keeping little), with the
choice one click away. That is still not a deal, but it is a choice made
for the person — the thing the ROADMAP's line is careful about.

## 4. What the design's tabs and screens become

| In `shell.md` | Becomes |
|---|---|
| **Sites**, first and the default: alo's list of results, site mark, name, address, title, two lines | **The engine's own results page**, in the conversation's card, as a real page. alo draws no list of its own: there is no list to draw without an API. |
| *alo used this* on results an answer drew on | **Moves to the Answer tab**: the answer's *N sources* lists what it drew on. Nothing is drawn over the engine's page (ADR 0066). |
| **Answer** | Unchanged in look; where its sources come from is ADR 0081's (*Where an answer's sources come from*). |
| **Images**, **Videos**, **News** | The **engine's own** images, videos and news pages for the same words — the addresses in § 2. A tab the chosen engine does not have (Mojeek's videos) is shown greyed with *ENGINE has no video search*, never sent elsewhere. |
| The tabs' order | Kept: Sites · Answer · Images · Videos · News — alo's own row above the card, choosing which of the engine's addresses the card shows. |
| *Search for "…"*, *Not set up yet* | *Search with ENGINE for "…"*, and before the first choice, *Choose a search engine*. |

The engine's page carries its own row of tabs as well. Two rows saying the
same thing is a cost; the alternative — no row of ours, the engine's tabs
only — loses *Answer* as a tab. Recommended: keep ours, because *Answer* is
alo's and must stand beside the others.

## 5. What it costs

**Nothing per search, to alo or the person.** At 30 searches a day for 30
days, 900 searches a month cost **$0**; no key, no account (Kagi aside,
which the person pays Kagi for directly). Compare § 7.

## 6. What must hold

- **The query goes from the person's Chromium straight to the engine.**
  alo has no server in between, logs nothing, counts nothing. ADR 0060's
  record of a tab's requests covers it like any other page.
- **Nothing is sent before the person chooses to search.** Not on typing,
  not on focus, not to warm a connection.
- **No parameter but the words** (§ 3), held by a test.
- **The agent does not search on its own.** The agent reads a results page
  a person opened, as any page. alo issuing its own queries to an engine and
  reading the results by program is what Google's terms forbid (*"using
  automated means to access content from any of our services in violation
  of the machine-readable instructions on our web pages"*, with
  `robots.txt` the example, effective 30 July 2026 [6]), and `/search` is
  disallowed for every robot on Google, Bing and Brave Search (`robots.txt`,
  read today [7]). Whether an agent acting for a person on a page the person
  opened is *automated means* in their sense was not read and is not
  claimed either way.
- **Consent pages and cookies.** If alo's Chromium profile is emptied when
  it closes (ADR 0058 § 3's private profile), Google and Bing ask for EU
  consent on every first search of a session. Keeping the engine's cookies
  between sessions is a decision about the profile, to be made in the item
  that builds this, not here.

## 7. A search API — set aside at first, then chosen (§ 0)

alo could buy results and draw the design's list itself. Set aside by the
owner's direction, and for these reasons as well:

| API | Price | Note |
|---|---|---|
| Brave Search API [8] | $5 per 1,000 requests; $5 free credit a month | own index; storing results needs a plan with *"storage rights"* |
| Bing Web Search API [9] | — | **retired 11 August 2025**; Microsoft points to *Grounding with Bing Search* inside Azure AI Agents (reported $35 per 1,000, raw results not returned [10]) |
| Google Custom Search JSON API [11] | $5 per 1,000, up to 10,000 a day; 100 a day free | **closed to new customers**; existing ones must leave by **1 January 2027** |
| Exa, Tavily, SerpAPI, Kagi's API, Mojeek's API | not read | AI-search APIs and scrapers of others' results |

At 900 searches a person a month, Brave's API is **$4.50 a person a month**
(900 × $5 ÷ 1,000), minus $5 of free credit once per account, not per
person. It needs a key alo holds, so a server, so a place where every
person's queries pass — the server ADR 0081 avoids by having the person
bring their key. Scrapers of Google's page are against Google's terms. And
a paid API is a contract with a search company, which is close enough to a
search deal to need its own argument.

## What is not verified, and is not hidden

- No engine's page has been opened **in alo**: whether each loads, shows,
  and reads as a tree in CEF's Chromium, and what bot checks (Ecosia's 403,
  Kagi's sign-in) do to a real browser, is unmeasured. The `curl` statuses
  above are from one machine, once.
- Startpage's, Qwant's, Ecosia's, Mojeek's and Kagi's privacy terms and
  index sources were not read; nor were Google's and Bing's privacy
  policies. Where results come from for DuckDuckGo is not stated on its
  privacy page.
- Startpage's video and news, Qwant's video and news, and DuckDuckGo's
  video and news addresses were not fetched.
- Whether Google, Bing or the others treat an agent acting for a person on
  their results page as *automated means*.
- Whether leaving out Qwant's `client=opensearch` or Ecosia's
  `addon=opensearch` changes what either shows.
- The EU choice screen's list and rules were read only from Google's page
  and press; the official EU site did not resolve.

## Sources (read 30 October 2026)

1. OpenSearch descriptions, fetched with `curl` —
   https://duckduckgo.com/opensearch.xml,
   https://search.brave.com/opensearch.xml,
   https://www.ecosia.org/opensearch.xml,
   https://www.qwant.com/opensearch.xml,
   https://www.mojeek.com/opensearch.xml, https://kagi.com/opensearch.xml
   (none served at Google's, Bing's or Startpage's usual paths)
2. Each engine's results, images, videos and news addresses as listed in
   § 2, fetched once with `curl` (a Chrome user agent) for their status
3. DuckDuckGo, *Privacy Policy* (last updated 6 March 2026) —
   https://duckduckgo.com/privacy
4. Brave Search, *Privacy Policy* — https://search.brave.com/help/privacy-policy
5. Google, *The choice screens* — https://www.google.com/chrome/choicescreen/
6. Google, *Terms of Service* (effective 30 July 2026) —
   https://policies.google.com/terms
7. `robots.txt` — https://www.google.com/robots.txt,
   https://www.bing.com/robots.txt, https://search.brave.com/robots.txt,
   https://duckduckgo.com/robots.txt
8. Brave, *Brave Search API* — https://brave.com/search/api/
9. Microsoft, *Bing Search API retirement* —
   https://learn.microsoft.com/en-us/lifecycle/announcements/bing-search-api-retirement
10. Neowin, *Microsoft pulls plug on Bing Search APIs* (secondary) —
    https://www.neowin.net/news/microsoft-pulls-plug-on-bing-search-apis/
11. Google, *Custom Search JSON API overview* —
    https://developers.google.com/custom-search/v1/overview

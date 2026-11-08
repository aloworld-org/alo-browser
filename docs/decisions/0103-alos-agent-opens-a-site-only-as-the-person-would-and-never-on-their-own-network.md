# ADR 0103 — alo's agent opens a site only as the person would, and never on their own network

**Status:** accepted — built by queue items 821 and 878 (8 November 2026);
amended by the owner's decision of 8 November 2026 (queue item 884, § 6)
**Date:** 2026-11-08
**Context:** ADR 0096 (alo's own agent runs in the browser process through
the same road as any agent; § 8 left *an agent that opens its own page* to
item 821); ADR 0002 (the layout tree is the agent's tree; no verb takes a
coordinate); ADR 0052 (the agent's gate and the person's gate); ADR 0055
(the agent socket, refusals as kinds); ADR 0008 (the machine's resolver,
and a public name that resolves to a private address refused); ADR 0005
(a site is the scheme and the registrable domain); ADR 0082, 0083 (the
search provider, and what a search sends); ADR 0092 (what a chat keeps);
`docs/design/shell.md`, *Beginning with no page* and *Settings*; the
owner's design of 8 November 2026, approved in full and written into
queue item 821; queue item 878.

## The question in one line

When a person asks alo to do something with no page open, how does its
agent find and open a site — through which road, judged by whom — and what
keeps that from becoming a way to reach anything a person did not mean it
to reach?

## Why an ADR of its own, rather than another amendment to 0096

ADR 0096 decided how alo's agent reads and acts on **a page the person
opened**. This decides something 0096 deliberately did not: the agent
**choosing where to go**. That widens what it can reach from one page to
the web, and so it carries rules 0096 never needed — which addresses are
never opened, how a name is resolved before one is, what a site is when
they are counted, and who is asked. Those are security decisions with an
attacker behind them (a page or a search result naming an address on the
person's network), and they deserve a record that can be read and
challenged on its own. ADR 0096 is amended only where a section of it
changes (§ 6, below).

## 1. Two steps that begin, and no new road to a page

The model is given two more tools (`agent_protocol.rs`), each with
`step`, `why`, `plan` and `on` as the verbs have:

- **`search`**, with a few words: asked through **the box's own search**
  (`brave_search::search`, ADR 0082 § 0, the same provider, key and
  request a person's search makes), its sites told to the model **as
  words** — numbered across the run, title, address, description, nothing
  else (`agent_found.rs`).
- **`open`**, with the number of a result **or** an address — the other
  `null` — and never a position (ADR 0002).

Neither touches a page. **Reading and acting stay where ADR 0096 § 1 put
them**: every read and every verb is an `Order::Agent` to the relay the
socket reaches the live card by, under `Caller::ALO`, kept to the sites a
person named with `--agent-site`. A site the agent opens is opened **by
the window, through the road a person's address takes** — chosen as an
address is, `ChromeOpener::open`, a card in the conversation — and once
Chrome says the card is up, the run reads it through the relay like any
page. There is no privileged path: nothing the run does reaches Chrome,
a tab or a renderer but through what a person's choice or an outside
agent's ask goes through, and an outside agent on the socket still opens
nothing (ADR 0055).

With no page open the model is sent `"page": null` and how many more
sites it may open; the person sees the run in a turn of its own — what
they asked, then the column a card's run has, its working line *alo is
finding the site to work on* with *Stop* — until it opens a page, and the
turn becomes that page's card.

## 2. Who decides whether a site is opened: the browser process

`agent_opening::judge`, in this order, on what the model proposed:

1. **An address, read as the box reads one.** A result's own address, or
   what the model wrote read by `alo_renderer::typed::address` — so only
   `http` and `https`. **A `file:` address, or any other scheme, is never
   opened, even when the person typed it**, because the box itself opens
   none: the agent opening what a person typed can be no wider than the
   box. (The owner's design allows one the person typed; this is
   stricter, and says why.)
2. **Named by the person**: an address in what they asked, read the same
   way, of the same host — or, both on the public internet, of the same
   registrable domain (`news on bbc.co.uk` names `www.bbc.co.uk/news`).
3. **Never this machine or its network unless the person named it**:
   `localhost`, a name ending `.localhost` or `.local`, an address that is
   not the public internet's (`alo-net`'s `is_public`: loopback, the
   private ranges, link-local, unique local, carrier-grade NAT, a v4
   address wearing a v6 hat), **and a public name that resolves only to
   such an address** — asked of the machine's resolver with the rebinding
   rule (`Resolver::resolve(…, Reach::PublicOnly)`, ADR 0008). A name that
   cannot be found is not opened. A search's result is judged the same:
   being found by a search makes nothing local safe.
4. **Kept to the sites named with `--agent-site`**, as every agent's ask
   is.
5. **At most the person's number of sites a run** (default 3, § 4). **A
   site is the host's registrable domain** — the public suffix list the
   codebase already carries (`alo_url::site::of`, ADR 0005), or the host
   itself where there is none (an address, `localhost`). The scheme is not
   part of it here: `http://` and `https://` of one domain are one site to
   a person counting the places alo went. A site the run is on or has been
   on is never counted again — the page it began on, one a link took it
   to, one it opened — and **a link followed is never counted**: it is the
   page's own navigation, as a person's press on it would be, and the
   person's gate already judges every press.
6. **Asked, or not** — *since § 6 below, by default not at all*: a site
   neither found by the run's search nor named by the person is asked
   first, when the person chose so in *Settings*, in the existing question box — *alo wants
   to open wikipedia.org. It is not a site a search found or one you
   named.*, **Allow** / **Not now** — and, when the person chose so in
   *Settings*, every site is. *Not now* ends the run, as it does for a
   press: nothing is opened and the model is not asked again.

A refusal is the step's outcome, told to the model in words (*Not opened:
…*); the run goes on and may try another site or finish.

**Its limit, said plainly:** the name is resolved here, and Chrome
resolves it again as it loads the page. A name whose answers change
between the two — a public address to us, a private one to Chrome a moment
later — is not caught by step 3. That is the rebinding ADR 0008 names, and
closing it needs Chrome to load the page from the address we resolved, or
to refuse a private answer for a navigation alo's agent started; neither
is built (queue item 881). Until then this check is what stops the plain
case — a result or a model naming an address on the person's network —
and says so.

## 3. A sign-in page hands the page over (ADR 0096 § 6, amended)

A page the run reaches — opened, or gone to by a link or a form — that
shows on the screen **a field for a password** (named or hinted
*password*, *passcode*, *passphrase*, *passwort*, *mot de passe*, or
holding a password's dots, `agent_signin.rs`) **hands the page to the
person**: the run is held exactly as when they take it over (ADR 0096's
amendment for 825), the column saying *This site needs you to sign in: the
page is yours. Sign in on it, then let alo continue.* with **Let alo
continue**. Carried on, the model is told — as a step of the person's —
that the page asked them to sign in and what changed. **A password the
card refuses the agent** (`Refused::Password`, unchanged, for every agent)
now hands the page over the same way, where it used to end the run stuck.
Judged only on a page just reached, so a page the person let it continue
on does not stop it again. **Its limit**: a sign-in page that names its
password field otherwise, or asks for it only after an address, is not
known to be one until the agent is refused a password there.

## 4. The limits are the person's (item 878)

How many sites a run may open (1 to 10, default 3), how many actions it
may take (5 to 100 in fives, default 20 — each search, open, press,
typing or scroll one), and whether alo asks before **no** site (the
default since § 6), only one neither found nor named, or **any** site are
set in *Settings*, *alo at
work*, kept in `settings.txt` beside the font, and **read by the window as
each run starts**, then fixed for the run. The bounds: one site is the
least that lets a run begun with no page do anything; ten is past any
task a person follows step by step; five actions searches, opens and
still acts; a hundred is five times the default, and the run's own token
limit (ADR 0096 § 4, 400,000 tokens, about $0.40) still stops a long run
on a large page first. **Not settings**: asking before paying, sending,
deleting or sending a form, and never typing a password or a card's
details (ADR 0096 §§ 5–6, ADR 0099) — those are not the person's to turn
off here.

## 5. What is kept

A run begun with no page that opens one is kept with that page, as every
run is (ADR 0092 as amended for 820). A run that opened none is kept as a
turn of its own (`"asked"`, its one run) and drawn, read back, as a
page's kept runs are. A run that moves to a second site leaves the first
page kept as its address, its run so far ended *alo went on to …*; the run
goes on on the new card.

## 6. Amended: by default alo asks before no site (item 884)

**The owner's decision, 8 November 2026:** alo's agent is to be less
strict, as other AI browsers' are (Atlas, Comet), and open any site
without asking by default. 821's hand run had shown why: asked to go
straight to the Wikipedia article on the Uganda Museum, alo asked *alo
wants to open en.wikipedia.org. It is not a site … you named* — the
person had named it, in words, and the rule of § 2.6 could not know it
without the model deciding what a name means.

**So § 2.6 is now a setting of three**, *Ask before opening* in *Settings*'
*alo at work*: **Never** (the default) — any public `http` or `https`
site opens with no question; **A site not found or named** — § 2.6 as it
was; **Any site** — every site is asked. Kept in `settings.txt` as
`asks never`, `asks not-found-or-named` or `asks every`. A file written
before this says `asks unnamed`, the old default, and every such file
wrote it whether the person chose it or not — so it cannot tell a choice
from the default, and it is read as the new default, *Never*, saying
nothing; a person who wants to be asked chooses so again.

**What stays, and is not a setting:**

- **Never this machine or its network** (§ 2.3) — `localhost` and its
  names, `.local`, loopback, the private ranges, and a public name that
  resolves only to one — nor any scheme but `http` and `https` (§ 2.1),
  unless the person wrote the address. **Why this one stays when the
  question goes:** what the agent opens is steered by what it reads — a
  page, a search's result, a stranger's words — and a page that tells it
  to open `192.168.1.1` would otherwise point it at the person's router,
  their printer or a service on their own machine, places a stranger on
  the web cannot otherwise reach and where a press does real harm. Asking
  before a public site guards the person's attention; refusing their
  network guards their network, and only the second is a security
  boundary. The rebinding gap of § 2 (queue item 881) is unchanged by
  this.
- Asking before paying, sending, deleting or sending a form; never typing
  a password or a card's details (ADR 0096 §§ 5–6, ADR 0099).
- The limits on sites and actions (§ 4), and `--agent-site`.

## What is not here

- **Opening a site in a card while another card is on screen without
  putting it away** — one page is open at a time (ADR 0075 § 4); earlier
  pages stay in the conversation as their addresses, with the run so far.
- **The agent choosing among a search's images, videos or news** — it is
  told the web results only.
- **A keyboard verb** — measured for canvas applications (item 879), not
  designed.

# ADR 0099 — A payment is shown from the page's own tree, and a card's details are never an agent's

**Status:** accepted — built by queue item 826 (7 November 2026)
**Date:** 2026-11-07
**Context:** ADR 0002 (the layout tree is the agent's tree; no verb takes
a coordinate); ADR 0052 (the agent's gate and the person's gate); ADR 0055
(the agent socket: refusals as kinds); ADR 0096 (alo's own agent: § 5 the
person's gate, § 6 never a password); ADR 0092 and 0098 (what a chat
keeps, sealed); `docs/design/shell.md`, *Paying*, the owner's design
approved 2026-11-06 (*"before any pay/confirm press, the question shows
amount, merchant and the page's own summary lines, with Pay and Not now.
The agent never enters card numbers"*); queue item 826

## The question in one line

When alo's agent is about to press something that pays, where do the
amount, the merchant and the summary the person is shown come from — and
what keeps a card's number, security code and expiry out of every agent's
hands on a page in the conversation?

## 1. What a payment pays is read off the page's tree, by the browser process

The person's gate (ADR 0096 § 5) already reads the page's tree again
before any press and judges the press on it. A press it holds because it
pays now carries what it pays (`alo-window`'s `agent_paying.rs`), read off
**that same tree** — the one an agent reads (ADR 0002): roles, names and
the words inside them.

- **No script.** Nothing is run in the page to find a price; a script's
  answer would be the page's to give, and a page that wants the person to
  pay more than it shows would write it.
- **Never the model.** The model only proposes the press. What the person
  is shown of the payment is not asked of it and not sent to it: a model
  told the amount could be told a wrong one by the page, and a model
  asked for it could invent one.
- **Named nodes only, and the conservative reading.** The **amount** is a
  price — a currency's sign or its code beside a number — in the words of
  what is pressed, else on the last line before it that says it is a
  total (*Total*, *Order total*, *Amount due*, *To pay*, *You pay*,
  *Balance due*; *Subtotal* is not one), else on the first such after it.
  A price merely nearest the button is **never** taken for the amount: on
  a checkout it is as often an item's or the delivery's. The **merchant**
  is the page's name for itself — its title — beside the site's host,
  which the browser knows and the page cannot say otherwise. The
  **summary** is the page's own: the lines under the first node named as
  a summary — *summary, your order, order, basket, cart, bag* — whatever
  its role but a field, a button, a link or a heading, or under a heading
  saying so until the next heading; a row a line, Chrome's `LayoutTableRow`
  among them (Chrome reads a table with no headers as a layout table —
  found by hand, when a checkout's summary and total read as nothing); the
  words of a button or a field never, a secret field's never; at most six.
  A line saying it is a total whose price stands in the next cell takes
  that price.
- **What cannot be found is said, and the person is still asked**: *alo
  could not find the amount on this page: read it on the page before you
  pay.* A question that guessed would be worse than one that says it did
  not know.
- **Pay allows only what was shown.** *Pay* (in place of *Allow*) is
  decided as *Allow* is — for the question the picture pressed showed and
  the window still asks — and the page is read once more and pressed only
  if it would ask exactly the same: the same words, the same amount, the
  same merchant and the same lines. A total that changed while the person
  read is not paid.
- **Which presses pay**: the gate's paying words, as before, and since
  this ADR *Confirm* or *Complete* on a page that shows a total to pay —
  a checkout's last button is as often *Confirm* as *Pay*.

**Its limit**, said: the tree is the page's, and a page can show a total
it does not charge. What this gives the person is what the page itself
says, beside who is paid, in alo's own words around it — not a promise
about what the site will take. That promise is the site's and the
payment provider's; the browser's part is to never let an agent press it
unasked, and to never let an agent type what pays.

## 2. A card's details are refused by the card, for every agent: `CardDetails`

ADR 0096 § 6 refuses text put into a password field, by the card, for
every agent, judged on the element. A card's details are refused the
same way, by a refusal of their own kind, **`CardDetails`** (`tab`,
`node`), on the socket too — a sibling of `Password`, not a widening of
it, so an agent told why knows which of the person's steps it met.

- **What is refused**: on a page shown as a card in the conversation
  (`Tab::keep_passwords`), text put into a field of a card's **number,
  security code or expiry**, and an option chosen in such a list (an
  expiry's month or year), judged by `chrome_reading::is_a_card_detail`
  before Chrome is asked to do anything.
- **Whatever the page names the field**: by its `autocomplete` token —
  HTML's autofill tokens `cc-number`, `cc-csc`, `cc-exp`, `cc-exp-month`,
  `cc-exp-year`, in any section, for billing or shipping — which a page
  sets for the browser's own autofill and so for exactly these fields; or
  by its `name`, `id`, `placeholder`, `aria-label` or name in the tree
  holding a name such a field goes by: *cardnumber*, *ccnumber*,
  *creditcard*, *securitycode*, *expirydate*, *mm/yy* anywhere in its
  letters, and *cvc*, *cvv*, *csc*, *exp*, *expiry* as a word of their
  own.
- **Not refused**: a card's holder's name (`cc-name`): it is no secret,
  and a person may ask alo to fill it in.
- **Met by alo's own agent**, the run is stuck: *This page asks for a
  card's details, and alo never types or chooses a card's number,
  security code or expiry: paying details are yours. Fill them in on the
  page, then ask alo again.*
- **Withheld from the model** as well: a field whose name or hint says
  *expiry*, *expiration*, *mm/yy* or *csc* is now written `(withheld)`
  with *card number*, *CVC* and *CVV* (ADR 0096 § 3), and what the person
  typed into one is never told the model when they let alo continue
  (queue item 825).

**Its limit**, said: a card's field inside a frame of another site — as
a payment provider's often is — was not measured for this ADR. Chrome's
tree is asked of the page with no frame named
(`Accessibility.getFullAXTree`), and whether such a field reaches it, and
so whether a verb could reach it at all, is queue item 858; until it is
measured nothing here claims it. A field a page names otherwise and marks
with no `autocomplete` is not known to be one; that is why the names are
read as broadly as they are, and erring on refusing is the side taken (a
field named `creditCardHolder` is refused).

## 3. What is not here

- Settings for what alo may do — paying always asked, the rest as a
  person says — remain item 705.
- A page's own payment sheet (the Payment Request API) is not a page alo
  renders in stage 2's sense; Chrome's own sheet on a card would be in a
  window nobody sees, and is not offered to an agent.
- Nothing here is kept: what a payment pays is shown on the live card and
  never written to the chat's file; the question's words are not kept
  either (ADR 0092 keeps how a run ended, and *Not now* says what was not
  done).

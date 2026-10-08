# ADR 0034 — A style written by script is remembered by its text, and a policy is asked when it is drawn

**Status:** accepted
**Date:** 2026-10-08
**Context:** queue item 343, *A page's `style-src`, applied to its inline
style*, cut from 339 by ADR 0033 § 2. Its *Needs* asks for an ADR first if
*who records that the attribute's current value came from the declaration*
turns out to be a decision about `alo-dom`'s element. It is one, for the
reasons below. Also read: ADR 0005 (only the browser process posts
anything), ADR 0017 §§ 2, 5 and 6 (the document is in the page's heap, every
change goes through `alo-dom` and is counted, and a changed page is drawn
again whole when its rendering is read), and ADR 0033 §§ 1–3 (the `style`
attribute is the only copy of the inline block, read on every draw). The
code this is about:
- `alo-dom`'s `node.rs`: `Element::attrs` is a public `Vec`, written by
  `set_attr`, `remove_attr`, `by_name.rs`'s two operations, the parser's
  `add_attrs_if_missing`, and anything that holds an `&mut Element`.
- `alo-dom`'s `sheets.rs` (`asked_for`) and `scripts.rs` (`stated`, a
  `<meta>` policy, and `nonce`, *is element nonceable*).
- `alo-renderer`'s `scripts.rs`: `at_load` gathers `<meta>` policies into a
  local list that is gone when the load ends.
- `alo-renderer`'s `pipeline.rs` (`draw`, which takes no policy),
  `renderer.rs` (`draw` and `fresh`, the draw at load and the lazy redraw
  when a rendering is read) and `violations.rs` (`Objection`, which names a
  policy and a kind and assumes `Placement::Element`).
- `alo-bindings`' `interface/css_style_declaration.rs` (`block` reads the
  attribute and `update` writes it) and `document_cell.rs` (what the page's
  heap knows about its document: its URL, its asks).
- `alo-net`'s `csp.rs` (`allows_inline`, `objecting_to_inline` and
  `inline_violation_of`, with `Inline::Style` and both placements, and
  `'unsafe-hashes'` from item 191).

## The decision in one line

`alo-dom`'s element remembers **the text `element.style` last wrote**, not
a flag. A `style` attribute whose value **is** that text is the
declaration's own and is applied under any policy. Any other `style`
attribute, and every `<style>` element, is applied only when **every
policy the page holds when it is drawn** allows it. The same rule, in one
function, decides both **what the renderer draws** and **what
`element.style` reads**.

## Why this is a decision about the element

CSSOM and HTML hold two copies of an inline style: the attribute, and the
element's declaration block. When a script writes through `element.style`,
the block is changed and the attribute is rewritten with an *updating flag*
set, so HTML's attribute-change steps skip the policy check. When anything
else sets the attribute, those steps ask the policy, and a refusal leaves
the block as it was. The policy is asked **once, when the value arrives**,
and the answer is kept in the block.

ADR 0033 made the attribute the only copy. So nothing here holds a block
that a refusal could leave alone, and the question *did this value come from
the declaration* has to be answered from the element itself, at any later
moment. That question has three possible answers, and they differ in what
happens when they are wrong. One of them fails open. Choosing between them
is a decision about what `alo-dom`'s element holds, and it is made here.

## 1. The record is the text the declaration wrote

`alo-dom`'s `Element` gains one field: the value `element.style` last
wrote to the element's `style` attribute, or nothing. It is set by **one
`alo-dom` operation**, which sets the attribute and the record together as
one counted change (ADR 0017 § 5). Only `alo-bindings`' `CSSStyleDeclaration`
calls it, in place of `set_attribute`. **No other operation reads or clears
it.**

An attribute is the declaration's own exactly when **its current value
equals the record**. That is the whole test.

**Why the text, and not a flag.** A flag must be cleared by every other path
that writes a `style` attribute. There are four such paths in `alo-dom`
today and a public `Vec` any crate can push to. Each missed path would be a
**bypass**: injected markup, or a page's own `setAttribute`, would keep a
flag that the next draw honours. A record compared by value cannot go stale
in that way. Any writer that changes the attribute makes it unequal, and
it is then judged by the policy. A writer added next year is covered without
being told.

**What a stale record can still admit.** Only the exact text the page's own
script wrote to that element through `element.style`. For example, a script
sets `el.style.color = "red"`, something else replaces the attribute, and
then the same `color: red` is written back by `setAttribute`. The record
admits it. The script could have written that text through `element.style`
at any moment, so admitting it gives no injection any power the page's
script did not already have. **The record never admits text the
declaration did not write.**

**A clone does not carry it.** HTML runs the attribute-change steps on a
clone's attributes, so a clone's `style` is judged by the policy. Nothing
clones an element today (`cloneNode` is not built, ADR 0017 § 8). The
operation that builds it must leave the record behind, and its item says so.

**Its cost** is a second copy of the attribute's text, for an element whose
style a script has written. This copy is not ADR 0033's rejected "two copies
of one fact": nothing is ever read from the record except whether it equals
the attribute.

## 2. A policy is asked when the page is drawn, of every policy it holds

The renderer applies `style-src` **every time it draws**, because ADR 0033
§ 1 reads the attribute every time it draws. It does not apply it once when
a value arrives. The policies asked are **all the policies the page holds at
that moment**: those in the response's headers, and every `<meta>` policy
the parser has made so far. The renderer keeps the `<meta>` policies for the
page's life rather than for the load's. A `<style>` element presents its
`nonce` attribute, nonceable by the same rule as a script's
(`alo-dom`'s `nonce`, moved where both can use it). A digest is matched
against the element's child text, which is also the sheet. A `style`
attribute presents no nonce, and its digest counts only under
`'unsafe-hashes'` (item 191).

**A refused `<style>` contributes no rules, and a refused attribute
contributes no declarations.** Each refusal is said in the drawing's issues,
naming the element and the directive. A drawing under no policy is the
drawing of today, which is what the corpus renders. Report-only policies
refuse nothing, as for scripts.

**Why at draw time.** Asking when the value arrives would mean keeping a
verdict per element. `alo-dom` would have to ask a policy on every write to
an attribute, the parser's included, and it has no policy and should not
have one. The verdict would also be a cache of a decision whose inputs (the
`<meta>` policies) change during the load. Asking when drawing needs
nothing kept, and it is the shape ADR 0033 § 1 already gave the attribute.

## 3. `element.style` reads a refused attribute as empty

CSSOM's block stays empty when the policy refuses the attribute, so
`cssText` is `""` and every getter answers `""`. A write then starts from
that empty block. **This is required, not courtesy.** A
`CSSStyleDeclaration` that read the refused text would carry it into its
write. The write would then make the record equal it, and an injected
`style` attribute would be admitted the first time the page's own script
touched `el.style.anything`.

So the page's heap must know its policies. The document's cell
(`document_cell.rs`) holds the page's policies beside its URL. The renderer
states them, as it states the URL (`navigating::locate`), when the page
loads and again whenever a `<meta>` adds one. **One function in
`alo-bindings`** answers *is this element's `style` attribute applied*: true
when the attribute is the declaration's own, or when every enforced policy
allows it. `element.style` asks it on every read. The renderer's draw asks
the same function, so the two cannot disagree. `getAttribute("style")`
still answers the attribute as written, as in every engine.

## 4. What is objected to is said once, and posted by the browser process

An `Objection` (item 237) gains **its placement**, and the wire carries it.
The browser process checks it against its own copy of the policy with
`inline_violation_of(place, Inline::Style, placement)`, which already takes
a placement. A claim no such policy could have made is still not believed.

A draw can happen many times for one style, so the renderer remembers which
**element, placement and text** it has already objected to, for the life of
the page. Each such combination is objected to once. What the load's draw
finds travels in `Loaded`, under the same bound of 64 as a script's. What a
later draw finds, after a script or an agent changed the page, is said in
that drawing's issues, and in this decision it is **not posted**. Carrying
it in `Acted` and `Delivered` is its own item (346), because those answers
carry no objections today and a draw read by `Paint` answers no message
that could.

## What this costs

**A `<meta>` policy reaches back.** A `<style>` or `style` attribute
written *before* a `<meta>` policy in the markup is refused here when that
policy refuses it. Other engines applied it, because they asked before the
`<meta>` existed. The error refuses rather than admits.

**A refused `setAttribute` takes the old style away.** In other engines,
setting a refused value leaves the declarations from before the write in
force. Here the attribute is the only copy, so the element has no inline
style until something allowed writes one. This too refuses rather than
admits.

**A report is per element, placement and text**, once per page, not once per
check. A page that writes the same refused attribute a thousand times
reports it once.

## Alternatives rejected

- **A flag on the element, cleared by every other writer.** It fails open:
  one missed writer is a bypass, and `Element::attrs` is public.
- **The policy asked when the value arrives, its verdict kept on the
  element.** This is the specification's timing. It puts a policy into
  `alo-dom` and keeps a verdict whose inputs change during the load.
- **A declaration block per element, as CSSOM keeps.** ADR 0033 rejected
  this, and nothing here reopens it.
- **Refuse in the renderer and leave `element.style` reading the text.**
  The first write through `element.style` would admit whatever was
  injected (§ 3).
- **Refuse writes through `element.style` too.** Every page with a strict
  policy that styles from script, which is every framework, would stop
  working. CSSOM and every engine exempt these writes, and ADR 0033 § 2
  requires the exemption.

## What this does not decide

- `style-src-elem` and `style-src-attr`. `csp.rs` does not act on them and
  names them as not enforced. Acting on them is a policy change of its own.
- A `<link rel=stylesheet>`. It is a fetch under `style-src`'s source list,
  and the renderer fetches no sheet today.
- A `CSSStyleSheet`'s `insertRule`, which CSSOM also exempts. It is item 89,
  and comes when a page opens it.
- A `<meta>` policy inserted by script. The renderer honours only the ones
  the parser made, as it does for scripts.

## How we will know if this was wrong

**A frozen page that writes a `<meta>` policy after inline style it
expects drawn** shows § 2's timing is wrong for real pages. The answer is
then a position per policy: a `<meta>` policy applies to inline style that
follows it in the document.

**A frozen page that replaces an allowed `style` with a refused one and
expects the old one kept** shows the cost of § 1's single copy. That is
ADR 0033's decision to revisit, not this one's.

**An element whose `style` attribute is applied under a refusing policy,
when its value is not the last text `element.style` wrote to it**, is a
bug in the record or in the one function of § 3. It is not a question about
this decision.

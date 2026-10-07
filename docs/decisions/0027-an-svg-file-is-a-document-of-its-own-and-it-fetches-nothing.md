# ADR 0027 — An SVG file is a document of its own, and it fetches nothing

**Status:** accepted
**Date:** 2026-10-07
**Context:** queue item 277, *"An SVG file as a picture"*, cut from item 107 by
ADR 0022 § 6 and marked *needs ADR*: *"which parser is rented, how it is held
to SVG documents only, and the secure static mode (no script, no animation, no
fetch of anything outside the file)"*. It also uses:

- ADR 0022 § 6: the cut items after the first four open *"only when a frozen
  page needs them"*, and a standalone SVG file is XML, which is stage 3's item
  139. *"This one case comes earlier because SVG icons ship as files on the
  modern web."*
- **The page that needs it is alo's own.** `alo-workplace`'s Meet screen
  (`web/src/meet/MeetModule.tsx`, read on 2026-10-07 at `738de614`, never
  written) draws its greeting as text followed by
  `<img className={styles.greetingIcon} src={wavingHand} alt="" />`, where
  `wavingHand` is `web/src/assets/alo-waving-hand.svg`. Its hero scene and
  `MeetRoom.tsx` use the same file. The file is 13 684 bytes: one `<svg>` with
  a `viewBox` of `0 0 395 385` and no `width` or `height`, a `<title>`, a `<g>`
  with `fill-rule` and `clip-rule`, and two `<path>`s. Everything inside it is
  something `alo-svg` already draws when it is written inline.
- ADR 0001 (rent the physics, build the engine), ADR 0004 (the algorithms are
  rented, the tree is ours) and ADR 0003 (a node's identity is allocated once).
- ADR 0005 (strangers' bytes are read in the renderer) and ADR 0010's line on
  *whose* `unsafe` it is: a rented crate's `unsafe` is the crate's, and FFI we
  would have to write is ours.
- ADR 0012 (every request says what caused it), so *"fetches nothing"* is
  checkable: an SVG picture is never the cause of a request.
- `alo-paint`'s `picture.rs`, which decides a raster format by its bytes, and
  `alo-renderer`'s `pipeline.rs` (`pictures_for`), which hands a decoded
  picture to paint by box and its size to layout.
- `alo-svg`'s `draw`, which takes a document, its computed styles, an `<svg>`
  node and a size, and returns a drawing.

## The decision in one line

An SVG file is read by **`quick-xml`'s pull reader, rented in one file of
`alo-dom`**, into **a document of our own** whose root must be an SVG `<svg>`.
That document has its own cascade and is drawn by the same `alo-svg` that
draws inline SVG, at the size layout gave the `<img>`. It is recognised by its
**type, never by sniffing**. It runs nothing, animates nothing and **fetches
nothing**, and any XML error refuses the whole picture.

## Why this is a decision rather than a chore

Three things here are not arithmetic. A parser for a new language enters the
renderer, and XML is a language stage 3 deliberately put off, so how it is
kept from becoming *"XML support"* by the back door matters more than which
crate it is. The SVG image is a document that is not the page's, so what it
can see of the page, and what it can make the browser do, is a security
boundary. And an SVG file cannot be identified the way a PNG is, so the
engine's rule that *the format comes from the bytes* needs an exception, which
has to be stated rather than slipped in.

## 1. What makes a resource an SVG picture: its type, and its root

`picture.rs` decides a raster format by what the bytes begin with, because a
PNG's signature cannot be faked without the file being a PNG. **That rule
does not extend to SVG.** An SVG file is text, and nearly any XML or HTML can
be made to look like one. The WHATWG MIME Sniffing standard deliberately has
no SVG pattern among its image signatures. Engines that render SVG as an image
require the response to say `image/svg+xml`. Sniffing it would let any page
that can get a stranger's XML or HTML served as a picture turn it into one.

So a resource is an SVG picture **only if both of these hold**:

1. **Its type is `image/svg+xml`.** That means the essence of the response's
   `Content-Type`, compared case-insensitively. For a resource that had no
   response, a frozen corpus file or a `file:` URL, the extension `.svg`
   stands in for the type, as it does for every engine reading a file from a
   disk. The pair of `src` and bytes that the renderer passes today gains the
   type beside it.
2. **Its root element is `svg` in the SVG namespace**
   (`http://www.w3.org/2000/svg`). A file of the right type with any other
   root is refused, and it is refused as *not an SVG picture*, never read as
   some other XML.

A resource whose type says SVG is never handed to the raster decoders. A
resource whose bytes are a PNG is never read as SVG, whatever its type says.
Raster formats are still decided by their bytes alone, and nothing about them
changes.

## 2. The parser: `quick-xml`, rented in one file

**`quick-xml`** (MIT) is rented, with default features off. Its default set
is already empty, and its only dependency without features is `memchr`. It is
named in **`crates/alo-dom/src/xml.rs` and nowhere else**, and that file is
added to `scripts/gate.sh`'s boundary list in the commit that builds it.

The reasons, in order:

- **It is a pull reader, not a tree.** It hands us one event at a time, and
  `xml.rs` builds an `alo_dom::Document` from those events with node ids
  allocated as ADR 0003 requires. So the tree is ours, exactly as `html5ever`
  hands `parse.rs` callbacks and we hold the result. A parser that builds its
  own tree, `roxmltree` for one, would give us a second tree to copy out of,
  and ADR 0004 drew the line at the tree.
- **The nesting depth is ours to bound.** A pull reader holds no stack of open
  elements beyond what it needs to check end tags, and `xml.rs` keeps its own
  open-element stack. So the depth bound in § 5 is enforced by our code before
  anything deeper is built.
- **It does not process DTDs.** It hands a document type declaration to us
  as one event without acting on it, and hands every entity reference it meets back to us unexpanded, as a `GeneralRef`
  event. This is exactly where the billion-laughs and external-entity attacks
  live, and with this crate they never reach anything that could act on
  them, because § 3 refuses them in our code.
- **It is `#![forbid(unsafe_code)]`** in 0.41, the version already in
  `Cargo.lock`. `memchr` contains `unsafe` for its vectorised search. By
  ADR 0010's line that is the crate's, and this repository still writes
  none.
- **It is already in the lockfile**, at 0.41.0, through `wayland-scanner` (a
  build-time dependency of `winit` on Linux). Renting it adds no new author to
  the supply chain. It does move the crate from a build tool into the
  renderer, which is a real change and is why it gets a boundary.

**It is configured strictly**, and the configuration is part of this
decision: end tag names checked (`check_end_names`), no unmatched end tags,
no bare `&`, and comments checked for `--`. Duplicate attributes are checked
on every element, and every prefix must resolve to a declared namespace
(`NsReader`). It is not a validating parser, and XML 1.0 has
well-formedness constraints it does not check. Where a real file shows one
that matters, the check is added in `xml.rs` with a test, rather than the
crate being replaced. The parser is a tool, and the rules in § 3 are ours.

## 3. Held to SVG documents only

`xml.rs` is **not an XML parser for the engine**. It has one entry point,
reading an SVG file, and its only caller is the picture path. Navigation to
an XML or SVG document, XHTML, XSLT and `DOMParser`'s XML modes all stay
where they were: stage 3's item 139, opened by a page.

What it accepts, and what it refuses with a named reason:

- **Encoding: UTF-8 only**, with or without a byte order mark. An XML
  declaration may name `UTF-8` or nothing. Any other declared encoding, or
  UTF-16, is refused. XML 1.0 requires a processor to read UTF-16 as well.
  SVG files are UTF-8 in practice, and a page that ships a UTF-16 icon
  reopens this.
- **Version 1.0.** A declaration naming another version is refused.
- **A document type declaration** is accepted and ignored **only if it has
  no internal subset**. That covers the `<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG
  1.1//EN" "…">` that older editors write. Its external identifier is never
  fetched. A declaration with an internal subset (`[ … ]`) refuses the file,
  because the internal subset is the only place an entity can be declared.
- **Entity references**: the five XML predefines (`&lt;` `&gt;` `&amp;`
  `&apos;` `&quot;`) and numeric character references to characters XML
  allows. Any other entity is undeclared, which XML makes a fatal error, and
  it refuses the file. So there is no expansion, no bomb and no external
  entity, by construction and not by a limit.
- **Processing instructions** are ignored, `<?xml-stylesheet?>` included. It
  is never fetched.
- **One root element**, with nothing but comments, processing instructions
  and white space around it.
- **`CDATA` sections** are read as text, because `<style>` blocks in exported
  icons use them.
- **Elements in other namespaces** (an editor's metadata, RDF, `sodipodi:`)
  are kept in the tree, and `alo-svg` draws nothing for them, as it already
  draws nothing for an element it does not know.

**An XML error refuses the whole picture.** XML defines a fatal error as
the end of normal processing, and the engines whose rendering a person would
compare ours with show such a file as a broken image. That differs from two
other rules in this engine, and the difference is deliberate. HTML recovers
from every error, and XML is defined not to. Path data is drawn up to its
first error (ADR 0022 § 2), but that rule belongs to SVG's attribute
grammar, not to the file. A refused picture leaves its box at the size its
style gives it and is recorded, as a refused PNG is today.

## 4. A document of its own: what it sees and what it can do

The image is **a second `Document`**, held by the pipeline for the box that
shows it. It is never put in the page's tree, never given a JavaScript
wrapper and never put in the page's heap (ADRs 0014 and 0017). No script can
reach it, from the page or from inside it.

**It sees nothing of the page.** Its cascade is the user-agent sheet and its
own `<style>` elements, and nothing else. The page's stylesheets do not
match inside it, the page's custom properties do not reach it, and
`currentColor` is the image's own `color`, whose initial value is used when
the file sets none. An icon that wants the colour of the text beside it must
be inline. That is how SVG images behave in every engine, and it is the cost
of the boundary.

**It can do nothing.** The mode SVG Integration calls *secure* is the only
mode here:

- **No script.** No realm is created for the image document. `<script>` in
  the file is kept in the tree and never run (`alo-dom`'s `scripts.rs` already
  refuses to run SVG `<script>`), and event attributes are inert.
- **No fetch, of any kind, to anywhere.** No external `href` on `<use>`, no
  `<image>` (already refused by ADR 0022 § 7), no `@import`, no `url()` in
  its stylesheet except a `#fragment` inside the same file, no `@font-face`
  source, no external identifier, no stylesheet processing instruction. A
  `data:` URL is not an exception: it is a second picture inside a picture,
  and nothing here draws one. The image document has **no way to cause a
  request**. Its pipeline gets no network handle, so this is a fact about its
  inputs and not a check that could be skipped. A test asserts that a file
  containing every one of these produces no request.
- **No animation.** SMIL is refused (ADR 0022 § 7). CSS animations and
  transitions inside the file draw their unanimated computed style, as
  everything does until item 94 gives the engine a clock. SVG Integration
  lets an `<img>` animate. Whether ours does is decided when 94 is built
  and a page shows an animated icon.
- **No interaction.** No hit testing, no links, no focus, nothing inside the
  image for the agent. That is ADR 0022 § 4's rule, applied to a document
  that is entirely image.

**Media queries inside it** take the image's own size as their viewport
width and height. Every other media feature is the page's. Whether a page's
dark scheme reaches an icon's `prefers-color-scheme` query is left until a
page shows it.

## 5. Size, drawing, and bounds

**Its natural size** comes from the root `<svg>`'s `width` and `height`
attributes when they are absolute lengths. A per cent, or no attribute,
gives no natural size on that axis. **Its ratio** comes from those two
together, or else from the `viewBox`. That is `alo_box::NaturalSize` with
`width`, `height` and `stated_ratio`, which item 270 already uses for an
inline `<svg>`. With no size, the `<img>` is sized as CSS Images sizes any
replaced element with no natural size: the ratio, if there is one, against
300 × 150. Meet's greeting icon is sized by its stylesheet, as the offline
screen's inline hand is.

**It is drawn as vectors, at the box's size, after layout.** It is never
rasterised at its natural size and scaled, which ADR 0022 rejected for
inline SVG and which is no more right here. The pipeline calls `alo-svg`'s
`draw` with the image document, that document's own computed styles, its
root and the `<img>`'s content box. It hands the drawing to paint by box
beside the pictures, through the same seam as inline drawings. The root's
`viewBox` and `preserveAspectRatio` place the drawing in that box. There is
no `object-fit` in this engine yet. When there is, it chooses the concrete
size before `preserveAspectRatio` applies.

**Bounds, before the work** (`LOOP.md` stage 2 § 2), in addition to every
bound ADR 0022 § 5 already puts on a drawing:

- bytes in the file, checked before the reader starts;
- elements in the document, counted as they are built;
- nesting depth, enforced on our own open-element stack;
- attributes per element, and the length of a name, an attribute value and a
  text run.

Past any bound, the picture is refused whole. The values are the building
commit's, each stated beside its reason and pinned by a test at its edge.
Malformed, truncated and adversarial files (every prefix of a real one, flipped
bytes, an internal subset, an undeclared entity, a depth bomb, an attribute
storm) each return a refusal, never a panic.

## 6. What the agent reads

**The `<img>` is the node, and its name is its `alt`**, exactly as for a PNG
today. Meet's `alt=""` makes it decoration, which is what it is: the greeting
text beside it says the same thing. The file's own `<title>`, `role` and
`aria-label` are **not** read into the page's tree. They belong to a
document the page embedded, and HTML takes an image's name from the element
in the page, not from inside the picture. The image adds no nodes. Law 2 is
unaffected, because what the agent reads is still the page's tree.

## 7. The cut

Item 277 is replaced by these. It closes when 309 and 310 close.

- **309. Reading an SVG file** (§§ 2, 3, 5's parse bounds): `alo-dom`'s
  `xml.rs`, `quick-xml` behind the boundary, every acceptance and refusal in
  § 3. *Depends on nothing.* Closes when Meet's `alo-waving-hand.svg`,
  frozen byte for byte, reads into a document of `svg`, `title`, `g` and two
  `path`s in the SVG namespace; each refusal has a named test; and every
  prefix and flipped byte of that file returns a document or a refusal,
  never a panic.
- **310. `<img src="…svg">`** (§§ 1, 4, 5, 6): the type carried with the
  resource, the image document's own cascade, its natural size, its drawing
  at the box's size, secure mode, and the agent's node from `alt`. *Depends
  on 309.* Closes on **Meet's greeting** frozen as an alo corpus case with
  the hand drawn, a layout assertion of the `<img>` box, and a reference
  render. A test shows that a file holding a script, an `<image>`, an
  external `<use>`, an `@import` and an external DTD draws its own shapes and
  causes no request. Another shows that the same bytes under a type other
  than `image/svg+xml` are refused.
- **311. A picture in `background-image`** (`url()`), raster or SVG.
  `background-image` today draws only gradients, so a picture behind a box
  is not built for any format. Once it is, SVG reaches it through 310's
  path with nothing new. *Opened only by a frozen page that needs it*, as
  179 is. No alo stylesheet in `alo-workplace` names an SVG `url()` today.

## What this costs

- **A parser for a second markup language in the renderer.** It is smaller
  than it sounds, because it is held to one root, one encoding, no DTD and
  one caller. But it is attack surface that did not exist yesterday, and
  § 5's bounds and hostile tests are what it costs to carry.
- **An icon in an `<img>` cannot follow the page's colours.** That is true
  in every engine, and a page that wants it inlines the SVG, which already
  works.
- **Some files other engines draw, we refuse**: UTF-16 files, files with an
  internal DTD subset (which some older editors write for entities like
  `&ns_svg;`), and files whose `href`s other engines would fetch. Each is
  a named refusal in the record, so a page that hits one says which.
- **A second document per picture.** The image's styles are computed
  separately for each `<img>`, and the same file shown twice is read twice.
  Caching it is a speed claim, and law 3 puts it after correctness and
  after a measurement.

## Alternatives rejected

- **`roxmltree`.** A good parser with a read-only tree of its own. The tree
  is the reason it is refused: we would copy out of it, and ADR 0004 says
  the tree is ours.
- **`xml5ever`**, the sibling of the `html5ever` we already rent. It parses
  XML5, which recovers from errors as HTML does. A file that every other
  engine shows as broken would render here, and the difference would be
  impossible for anyone to debug from the file alone. Recovery is the
  wrong behaviour for XML.
- **`usvg`**, which reads an SVG file as well as drawing it. ADR 0022
  already refused it for owning its own tree and cascade, and nothing about
  files changes that.
- **Writing our own XML reader.** XML's grammar is larger than path data's,
  and the parts that matter here, refusing DTDs and entities, are a
  configuration of a rented pull reader plus a few dozen lines of ours.
  ADR 0001's rent list is physics, and a tokenizer for a fixed grammar is
  closer to `cssparser` than to a stroker. We rent `cssparser` too.
- **Sniffing SVG from the bytes**, to keep `picture.rs`'s rule uniform. See
  § 1: uniformity here would make arbitrary XML a picture.
- **Rasterising the file at its natural size** and scaling the bitmap.
  Refused by ADR 0022 for inline SVG, and the reasons carry
  over: wrong at every other size and under every transform.
- **Letting the image document see the page's custom properties or
  `currentColor`.** Convenient, and precisely the leak the boundary exists
  to prevent. A picture from another origin would be able to read the
  page's styles back through its own rendering.

## What this does not decide

- **The bounds' values** (§ 5), which belong to the commits that build 309
  and 310.
- **Animation in an SVG image** (§ 4), which is item 94's question.
- **Preference media queries inside an image** (§ 4), left to a page.
- **SVG in `background-image`**, `border-image`, `list-style-image`,
  `content: url()` and a favicon, beyond saying they reach this path. 311
  is the first, and the rest wait for pages.
- **XML as a document**, which is still stage 3's item 139. This decision
  must not be cited as having opened it.
- **Caching parsed images** across boxes or pages.

## How we will know if this was wrong

- **If a frozen page's SVG icon renders as broken here and fine in both
  Firefox and Chromium**, and the reason is a strictness in § 3, that
  refusal is reopened with the file beside it. UTF-16 and the internal
  subset are the likeliest.
- **If an SVG picture ever causes a request**, § 4 failed at its boundary.
  That is a defect, not a reason to change the decision, and ADR 0012's
  record is what would show it.
- **If `quick-xml` accepts something that matters and that XML forbids**,
  more than a few times, our checks in `xml.rs` are growing into a parser
  of our own. At that point renting a stricter parser, or writing one, is
  reopened.

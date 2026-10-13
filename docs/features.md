# alo browser — features.md

Feature inventory. Four tiers, matching the stages in `ROADMAP.md`:
**[1]** = renders alo · **[2]** = renders the modern web · **[3]** = the legacy
tail · **[4]** = a browser somebody chooses. **★** marks the things no other
engine offers.

**[4] was added when the roadmap gained a fourth stage.** The product work —
extensions, sync, a mobile port — used to sit under "not scheduled", which had
begun doing the work of "not thought about". It is gated behind stage 2's exit
gate exactly as before; it is now *named*.

Rule of the file: **nothing gets built that isn't listed here, and nothing gets
listed without a tier.** Additions go through the scope gate — this file, the
current stage, and Non-goals below.

The tiers are not a schedule. Stage 1 is the whole of the work for a long time,
and an item marked [2] is a decision that it is *not* stage 1's problem.

---

## The document

- [1] A DOM of our own, built from `html5ever`'s parse events — the tree is ours even though the parser is not
- [1] A stable identity per node, from the first commit, because the agent tree will need to name one and adding identity later means rewriting everything holding a reference
- [1] Fragments and malformed input produce a usable tree rather than an error
- [2] The DOM APIs a modern page actually uses — driven by pages that fail, never by a specification listing a method. Opened so far by alo's download page: finding an element by its `id`, showing a hidden one, and adding a class through `classList` (327), and finding elements by a selector with `querySelectorAll`, answered as a static list (329), which a page walks with `forEach` — the list's `forEach`, `keys`, `values`, `entries` and `Symbol.iterator` are the very functions every array has, as the standard says (331). Walking one with `for…of` still stops by name at the list's `length`, which is a getter (231)
- [2] Mutation, so scripting has something to mutate
- [3] The legacy surface: `document.write`, live collections, the rest

## Style

- [1] Stylesheets parsed with `cssparser` into rules we hold
- [1] Selector matching with `selectors` — the modern subset only
- [1] ★ **The cascade, inheritance and `var()`.** alo's design system is custom properties throughout, so an engine that cannot resolve them renders nothing of alo at all. This is stage 1's first hard requirement, not decoration
- [1] A variable cycle is refused rather than looped
- [1] Lengths as numbers: every unit CSS has that does not need a window, `calc()` type-checked and evaluated, and `em` and `rem` against the font size actually in force. `ex` and `ch` are the height of the `x` and the advance of the `0` in the first face the element's text is set in (queue item 320), asked of the fonts through a seam the cascade leaves open (`alo_style::MeasureFace`), and half an em where there is no face or no such glyph. A percentage is carried rather than resolved, because what it is a percentage *of* is layout's to say — and `calc(100% - 2rem)` reaches layout as an expression and is resolved there, against the basis the running algorithm knows (ADR 0004)
- [1] **`clamp()`, `min()` and `max()`**, one family with `calc()` and nesting in each other, type-checked once at parse time
- [1] **Viewport units** — `vw`, `vh`, `vmin`, `vmax` — which need a window, and answer zero rather than a plausible number when there is none
- [1] Colours as channels — hex, `rgb()`, `hsl()`, the named colours. Blocks paint rather than layout. **A translucent colour is blended exactly** (queue item 436): source-over in floats, rounded to a byte once when the picture is written, which is what Chrome 154's default raster path draws on every cell measured whose blend is not exactly a half; its software path's eight-bit blend, up to a unit darker, is not copied. Corpus case `translucent-fills`. **`color-mix()` in sRGB** (queue item 377): two colours, each with a share or without, mixed premultiplied as CSS Color 5 § 2 says, with shares normalised and a total under a hundred fading the result; read wherever a colour is, nested up to sixteen deep. Opened by `alo-sites-pricing`, whose highlighted tier's shadow is one. Any other space is a conversion and is refused, as is a mix that names none (which means `oklab`); a mix of one colour or of three is refused; `currentColor` in a mix is refused until there is a page for it (378)
- [1] An unknown property is kept and ignored rather than dropped, so a later stage can implement it without re-parsing
- [1] Media queries for width, and `prefers-color-scheme` — the light and dark the workspace already ships. A width is in `px`, `em` or `rem`, and the last two are the initial 16 px rather than the page's font size, as Media Queries says; any other unit is recorded and the query never matches
- [2] Animations and transitions
- [2] Container queries, `:has()`, cascade layers, `@property`
- [2] Filters, `backdrop-filter`, blend modes, masks and `clip-path`
- [2] Paged media and print styles
- [3] Vendor prefixes, and anything that exists only for a page written before 2015

## Layout

- [1] The box model — content, padding, border, margin, and the box's *meaning* alongside its rectangle (ADR 0002)
- [1] **Every box on Chrome's grid of 1/64 px** (queue item 466, ADR 0054): a length, a percentage and a `calc()` are each truncated toward zero onto the grid where layout uses them — `0.6rem` is 9.59375, `-0.3px` is −0.296875, a `calc()` resolved whole and truncated once, a percentage handed to `taffy` as a calculation so that it is truncated after it is resolved; a run of text's width is rounded up, so a box that shrinks to its text holds it; and a box's edges are placed as it is read back, its near edge truncated toward zero from where its parent was placed and its size the distance between its two edges floored, so centring, auto margins and flexible grid tracks put each edge where Chrome does, and a box filling its parent still fills it (`alo-layout`'s `layout_unit.rs`, the one file holding the grid and its rules, which `line_height.rs` also reads). A length or text width within four of a float's own steps of a grid line is on it, because Chrome resolves a length in double where this engine uses `f32`; an edge `taffy` shares out is not, because Chrome shares in float as `taffy` does. Past the grid's range a length saturates at Chrome's 33,554,428 px. `alo-sites-booking`'s boxes are all Chrome 154's to the unit since its date field holds its line in segments (507). Not yet (ADR 0056: a hook offered to `taffy` upstream, 501, then built on, 502; flex `justify-content` needs measuring, 503): what `taffy` computes between its own passes — a grid sized twice, as the mini-month's `aspect-ratio` columns are, flexible items' exact shares, and a grid's `justify-content` after its first share, each within a unit of Chrome's (flexible items within two: Chrome's rule for them, shrinking and growing, at one and at two, is measured on 5,120 lines and written into ADR 0054's addendum of 2026-10-13, item 564); a line whose factors sum below one with a gap flexes by all of its free space, `taffy`'s fault (572); a `fit-content()` percentage, a `vertical-align` length, and where an atomic box stands inside a line are not put on the grid
- [1] **`flex` is its three longhands** (queue item 440): `flex` is split into `flex-grow`, `flex-shrink` and `flex-basis` as it is written, so `flex: 1` grows from nothing (`1 1 0%`), `none` is `0 0 auto`, `auto` is `1 1 auto`, and the basis may stand before or after the two factors — each form measured against Chrome 154, and what Chrome refuses left whole. Nothing read the shorthand before, so alo Agenda's calendar rows and alo Settings' dialog did not grow where they should: Settings' Cancel and Save stood at the left of its footer. Corpus case `alo-agenda-calendars`
- [1] **A maximum is a limit or `none`** (queue item 393): `max-width` and `max-height` read `none`, their initial value, as no limit, so a later rule undoes an earlier one's limit without anything recorded against the page, as alo Sites' banner sections do; `auto`, which a maximum does not take, is refused and recorded. Opened by `alo-sites-tickets`
- [1] **Text's narrowest is its widest word** (queue item 394): asked how narrow it can be, text in a line is laid out in no room, so it is as narrow as its widest piece between break opportunities (CSS Sizing 3 § 5.1), and `nowrap` text as wide as its line. So a `1fr` grid column holding a sentence stays its share and the sentence wraps in it, as in alo Sites' closed booking section; it was the sentence's whole line before. Opened by `alo-sites-booking-closed`
- [1] **An inline flex box stands on its text** (queue item 395): an `inline-flex` box in a line stands on its first baseline, its first item's first line's (CSS Flexbox 1 § 8.5), or that item's bottom border edge when it has no line, so a link made `inline-flex` to centre its text in a tall box stands where its text does, as alo Sites' menu links do; it stood on its bottom edge before, with the strut's descent under it. A row with an item aligned on its baseline (396), an `inline-grid` and a scroll container still stand on their bottom edges. Opened by `alo-sites-nav`
- [1] **An atomic inline box holds its blocks** (queue item 286): an `inline-block`, `inline-flex` or `inline-grid` is one box on its line however its children are displayed (CSS Display 3 § 2.4) — only an inline box, `display: inline`, is broken around a block in it — so a block in an `inline-flex` is its flex item, a block in an `inline-block` is one of its blocks with any text beside it wrapped as in any block, and the `inline-block` stands on its last block's last line. alo Sites' brand link, `inline-flex` holding a logo its sheet makes `display: block`, is one box holding the logo, as in Chrome; it was two empty links with the logo between them. Opened by `alo-sites-full-page`
- [1] **A form control holds what it shows in a box of its own** — a tall button's label in the middle of it, an empty field still one line tall. A box in the tree rather than a rule in the user-agent sheet, because a rule would also catch a control an author had made a flex container. **That line is the line its text would be** (queue item 467): the line height it set, or with `normal` its font's whole-pixel ascent, descent and line gap (ADR 0050), where it was 1.2 em — so `a-filled-form`'s fields are Chrome 154's 19 and alo Settings' every box is Chrome's. **A button's and a field's border are Chrome's 2 px** (queue item 505), `outset` and `inset`, and **drawn as Chrome draws the control** while the page leaves its border and background alone: a one-pixel `#767676` edge with corners rounded 2.5 px over a `#efefef` or white fill, translucent grey when disabled (`alo_paint::face`). A page that writes any `border` or `background` property, or `appearance: none`, gets its CSS drawn instead. So `a-script-hears-an-event`'s button is Chrome's 22, and its edge is Chrome's screenshot's within 7 levels. **A control is set in Chrome's own font** (queue item 506): 13.33 px, weight 400, upright, with a `normal` line, in Arial (the machine's `sans-serif` where it has none) and in `monospace` for a `<textarea>` and the five date and time fields, whatever the page around it says, and `font: inherit` takes the page's back, every longhand and the line included. So `web-a-form`'s fields are Chrome's 21 and everything down to its fieldsets stands where Chrome's does. **A date or a time is held in segments** (queue item 507): a `date`, `month`, `week`, `time` or `datetime-local` field has a pixel above and below its line, as Chrome's edit does, and Chrome's own padding, a pixel before it and none above, below or after. So such a field is 2 px taller than a text field with the same line, and `alo-sites-booking`'s every box is Chrome's. **A text field is as wide as its `size` says** (queue item 511): that many characters, twenty when it says nothing usable and always for a `number` field, each the face's `x` rounded to a whole pixel only when that makes it wider, plus the face's whole-pixel ascent less one of them, measured in Chrome 154 over 3,000 fields and confirmed on 288 it was not fitted to. So `web-a-form`'s fields are Chrome's 172 where they were 20 zeros, 177.66. Not yet: the rest of a control's text reset (512), the `font` shorthand beyond a keyword (513), a date or time field's floor at its calendar indicator's height (515) and its own width (517), and a text field's line never below `normal` (516). **A `<textarea>` draws a field's picture** (queue item 510): Chrome 154 computes a `1px solid` border for it and draws a field's rounded edge and fill in that pixel instead, measured pixel for pixel the same as a field of its size, live, disabled and read-only, and a page's border or background takes it away as a field's does. So `web-a-form`'s textarea is Chrome's screenshot exactly along its sides. Not yet: a `<select>`'s own picture and its chevron (521), the grip of a box a person can resize (522), a `<textarea>` as big as its `cols` and `rows` (523), a rounded edge's inner corner pixel (524), and a restyled field's white (525)
- [1] Box generation: `display: none` removes a subtree, `display: contents` removes a box and keeps its children, a container whose children are a mix of block and inline grows the anonymous boxes that make them one kind, and each run of text straight inside a flex or grid container is wrapped in an anonymous block item, so its lines take their `line-height`
- [1] A user-agent style sheet — what an element looks like before anybody says otherwise. The modern elements only; no defaults for what we do not lay out. **A list in a list has no space above or below it, and an edit looks like one** (queue item 400): HTML's rendering section's `margin-block: 0` for a `dl`, `menu`, `ol` or `ul` inside another, and `<s>` and `<del>` struck through, `<u>` and `<ins>` underlined. Opened by `alo-sites-rich-text`, alo Sites' rich text, whose nested point stood 16 pixels away and whose finished task was not struck out
- [2] **A closed `<details>` is its summary** (queue item 429, ADR 0048, built in iteration 277): a `details` is its first `<summary>` child, laid out first wherever it is written, then an anonymous content box holding everything else; without `open` that box is there — a flex or grid `details` keeps the gap before it — but takes no room and holds nothing laid out, drawn or read. The summary carries `expanded` and is the only `summary` with its role; `Activate` on it toggles `open` by HTML's activation behaviour in `alo-dom`, for an agent and a script's `click()` alike. Opened by `alo-sites-full-page`'s FAQ, whose closed `details` is 53.2 tall as Chrome's and opens to 92.4; `alo-layout`'s `tests/details.rs` pins the rest in numbers and `cases/details-closed-and-open` in pixels. A `details` with no summary has an empty slot and an issue, and a rule naming `::details-content` is kept and recorded rather than dropped. Waiting: the disclosure triangle with `inside` markers (432, opened by the same page), a generated "Details" summary (433), and `toggle`, `HTMLDetailsElement.open` and exclusive accordions (434)
- [2] **A list item's marker** (queue item 401, ADR 0044): the bullet or number a `display: list-item` box stands beside, as `::marker` styled under ADR 0043 and made by the item's own computed style (`alo-style`'s `marker.rs`; `display` is read there now, `display.rs` moved from `alo-box`) — a run of text, `• `, `◦ `, `▪ ` or `3. `, from `list-style-type` and HTML's ordinal value (`start`, `reversed`, `value`; `alo-style`'s `ordinal.rs`), a name nothing defines read as `decimal`; set outside the item's first line on its baseline and taking no room (`alo-box`'s `is_marker`, `alo-layout`'s `marker.rs`), a symbol `⌊ascent × 2 / 3⌋ + 8` before the line and a number ending at it; `color` and the font properties read on `::marker`, the rest said; `list-style` split by `alo-css`'s `list_style.rs`, so `list-style: none` still removes it; the user-agent sheet's nested `circle` and `square`; an agent reads it as the item's `marker` property, never in its name, through the renderer's snapshot too. Opened by `alo-sites-rich-text`. An unbuilt counter style, `inside` and an image draw none and are said. Waiting for pages: `inside` (404), `list-style-image` (405), other counter styles and `<ol type>` (406), CSS counters (407), `::marker`'s other properties (408), an item with no line (409)
- [1] **A block-level box inside an inline one, broken around it the way the specification says** — a piece on each side, the block a sibling of the anonymous blocks they sit in, so a background stops and starts again rather than running straight through
- [1] **An inline box's own border and padding**: horizontal ones take room on the line, vertical ones draw without changing its height, and a box that wraps is one rectangle per line with its start border on the first piece and its end border on the last
- [1] An *empty* piece of such a break keeps its border, and costs no line when it has none — CSS's zero-height line box
- [1] **Flexbox and grid**, on `taffy` behind our own boundary. One file may name it. **An `auto-fill` or `auto-fit` repetition whose track size is a calculation** (queue item 422, ADR 0047), such as alo Sites' blog grid's `minmax(min(100%, 18rem), 1fr)`, which `taffy` throws away whole: where the grid's size in that axis is definite the calculation is resolved against its content box and handed to `taffy` as the length it comes to, so the blog cards are two columns of 368 as in Chrome (`alo-layout`'s `auto_repeat.rs`, deleted the day `taffy` counts a calculation). Not yet: a grid sized by its content is measured with its template discarded and said once per grid and axis (424), and an auto repetition of a track that is not fixed, such as `repeat(auto-fit, 1fr)`, is accepted when CSS makes it invalid (425). Opened by `alo-sites-blog-index`
- [1] Absolute and relative positioning, `z-index`, stacking
- [1] **An absolutely positioned box is blockified** (CSS Display § 2.7): an inline with `position: absolute` is block-level, sits in no line and is placed at its offsets, shrunk to what it holds, as alo Sites' skip link is at `left: -999rem` (queue item 352). **Written in a line, it stays in the line and takes no room** (queue item 354): it does not break the inline box it is in, the line goes on as if it were not there, and each axis whose insets are both `auto` puts it where the line would have — the pen, moved with the line by `text-align`, and the line's top (CSS 2 § 10.3.7) — its margin going from there; an inset places that axis against its containing block. alo Sites' blog cards' `Read article<span class="sr-only">` is one link on one line with the `<span>` at its end, as in Chrome. Opened by `alo-sites-blog-index`. Not yet (423): a containing block inside the lines — the block holding them, or an inline box — is no box the engine places against; the box is placed against the page instead, and that is recorded
- [1] **An absolute box is placed against its containing block** (CSS 2 § 10.1, queue item 355): the padding box of its nearest ancestor that is positioned or transformed, or with none the initial containing block — the viewport's size at the top of the page, whatever its parent's margins did. An axis with both insets `auto` keeps the box where it would have been in its parent, and what is placed against the page counts in the page's scrolling area. Opened by alo Sites' footer, whose skip link sat 48 down with `body`. Not yet (399): the static position in a flex or grid container further in counts the box as empty, and is recorded; a box in an atomic inline box with nothing positioned between is placed against that box, and is recorded; and `filter`, `perspective`, `will-change`, `contain` and `container-type`, none of them read, make no containing block
- [1] Overflow and scrolling regions
- [1] Inline formatting: a line of text and the boxes in it, with breaking and baselines
- [1] **`text-align`** `start`, `left`, `center`, `end` and `right`: a line, text and inline-blocks together, moves as one, and a line in a box nobody wrote inherits its container's alignment
- [1] **An image or inline-block's margins on its line**: its margin box takes room across the line, stands on the baseline and makes the line taller, so the space an author writes under a picture is kept
- [1] **The strut**: every line starts as tall as its container's font, so a line of only a picture keeps the font's descent below it and small text in a large-font block makes a line as tall as the large font; a line with nothing worth a line box is still no line
- [1] **An inline-block's baseline is its last line's**: a button stands on its label's, a text field on its value's, an inline-block of text on its last line, so each reads level with the text beside it; **a one-line field with nothing typed in it stands where its text will** (queue item 384), so it does not drop when somebody types; with no line, or `overflow` other than `visible`, it stands on its bottom margin edge as a picture does. A grid container's baseline (its first row's) is refused and stands on its bottom edge; an `inline-flex` box's is its first item's (item 395, below)
- [2] **`line-height` on a line**: text, an inline box and the strut each take their `line-height` on the line, and a line is built from whole pixels of font, as Chrome 154's is (ADR 0050): the font's ascent, descent and line gap are each rounded half up, a set `line-height` is placed on a grid of 1/64 px (a number floored from the font size rounded to 1/64, a length or percentage rounded), and the top half of the leading, floored to a whole pixel, goes above the letters and the rest below, so 16 px `DejaVu Sans` at 1.5 is 24 with its 15 and 4 of font 2 down; a `line-height` smaller than the font is a negative leading, floored the same way; an inline box is aligned by that room, not by its letters; `normal` is the font's own ascent, descent and line gap; the computed value, and so `lh`, stays exact; what is drawn is still the font's height, so a link's background does not fill the gap between lines; a value that is negative, not finite or unreadable is `normal`
- [2] **`vertical-align`**: every keyword, a length and a percentage of the box's own `line-height`, on an image, an inline-block and an inline box with everything inside it. `middle` is half the parent's x-height over its baseline; `top` and `bottom` hold a box, and what is in it, against the line box once the rest has said how tall it is; whatever moved still makes the line taller
- [2] **A `<br>` ends its line**: what follows starts a new line whether or not the line was full; two in a row leave a blank line, one with nothing after it adds none, and one alone is a line of its own; it takes no room across the line, stands where the line's drawn content ends, and its own `line-height` counts towards the line it ends. A `<br>` given another `display` is the box the author wrote rather than a break
- [1] **`letter-spacing`**, applied where the text is measured — it changes what a run is worth and so where every line breaks
- [1] **`white-space`**: `normal`, `pre`, `pre-wrap`, `pre-line`, `nowrap` — runs of whitespace collapsed when the box is built, and where a line may break decided when the line is built; **an atomic box in a line that may not wrap stays on it** (queue item 447): whether a line breaks before an `inline-block` or a picture is decided by the space before it, or else by the nearest box holding both it and what came before (CSS Text 3 § 5.1, `alo-layout`'s `soft_wrap.rs`), never by the box's own `white-space`, so a `nowrap` line keeps it and overflows, and a clipped line's "…" hides it, all as Chrome 154 does (`cases/atomic-in-nowrap-line`); **text where two boxes meet follows the same rule** (queue item 449): after a space, the space's box or the box before the space decides; otherwise the box of what came before, or one it has closed into since, as Chrome 154 decides it (which lets a line break after an atomic box in a `normal` span); between two letters there is no break whichever boxes they are in; and a no-break space is not collapsed as white space and does not hang (`cases/soft-wrap-between-boxes`). Not yet: a word running across two boxes that does not fit overflows where it stands rather than moving whole to an earlier break on its line (452); a trailing space at a line's end still widens its inline box (450); an inline box opened just before a break keeps an empty piece on the line before (451); a space kept by `pre` or `pre-wrap` takes its room at the start of a line, so code keeps its indentation (queue item 410, opened by `alo-sites-rich-blocks`), and only a collapsible one is removed there. Not yet: a line of nothing but kept white space, such as a blank line in `<pre>`, is no line (411)
- [1] Layout is asserted in **numbers** — the computed box — never by eyeballing an image
- [2] Writing modes, and layout that is right-to-left rather than mirrored afterwards
- [2] Multi-column, `position: sticky`, scroll snap and overscroll behaviour
- [3] **Floats as layout, CSS-table layout, quirks mode.** Deliberately last, and possibly never: refusing these is what makes the scope survivable

## Text

- [1] Shaping with HarfBuzz — `rustybuzz`, the Rust port, so there is no C in the process — and font rasterisation. Rented, as every engine rents them
- [1] The fallback chain: a font is *asked* whether it has the character, never guessed at from a language tag
- [1] **The awkward scripts before the easy ones.** A pipeline that assumed left-to-right and one glyph per character is a pipeline that gets rewritten
- [1] Line breaking, and the fallback chain when a font lacks a glyph
- [1] **A face is set at its size cut to hundredths** (queue item 514): `13.333333px`, the size every control has, is set at 13.33, and `8.11px` at 8.1, the cut made in a 32-bit float as the size is held, as Chrome 154 sets a face. Advances, a line's room, `ch`, `ex` and a field's width are read at that size; `1em` is still the size as written (`alo_text::face_size`). Measured over 2,922 runs in three faces at 128 sizes, 2,916 of them Chrome's width and every one Chrome's height, so `web-a-form`'s button is Chrome's 103.078125. Not yet: six runs whose sum ends within 0.03 of a unit of a grid line on the other side of it from Chrome's (527)
- [1] **Code is drawn in a monospace face** (queue item 412): the reference corpus and `--frozen-fonts` carry DejaVu Sans Mono, regular and bold, and answer `monospace` with it, so the `pre`, `code`, `kbd` and `samp` the user-agent sheet makes `monospace` are drawn one advance per character, and no case says it could not find the family. At the size browsers draw it (415, ADR 0045): `font-family: monospace` alone has a `medium` of 13 px, a computed size carries whether it came from a keyword, its parent's font or a fixed length, and where an element crosses into or out of `monospace` a keyword is read again, a relative size rescaled by 13/16 and a fixed one kept — so the user-agent sheet's `pre` and `code` under an ordinary body are 13 px, `monospace, monospace` and a named family first stay 16, and `0.86rem` code is 13.76 either way. Keyword sizes as the engines' integer table rather than CSS's ratios, for every family: 416, waiting for a page
- [1] **A run is drawn in the face it was measured in** (queue item 402): `font-weight` is read once, by the style, and layout and paint both ask it, so a heading the user-agent sheet makes `bold` is drawn bold and fills the room it was laid out in. Not yet: `bolder` and `lighter`, which read as `normal` in both (403)
- [1] Web fonts
- [1] `text-transform` (queue item 441, built in iteration 283, ADR 0049): `uppercase` and `lowercase` applied when a text box is built, after white space is collapsed, so layout, paint and the agent read one text; Unicode's full case mapping, language-sensitive by the nearest `lang` (Turkish, Azeri, Lithuanian and Greek tailored), rented from ICU4X's `icu_casemap` behind `alo-box`'s `case.rs`. A field's text, its hint (by its `::placeholder` style), a broken picture's `alt` and a marker (by its `::marker` style, which the user-agent sheet makes `none`) are cased too. An agent reads drawn text and names from content and `<label>`s as drawn, and attributes and a field's value as written, as Chrome's accessibility tree does. Waiting: a name from `aria-labelledby` as drawn (446), `capitalize` (444), `full-width`, `full-size-kana` and `math-auto` (445), drawn as written and recorded until then
- [1] **`text-overflow: ellipsis`** (queue item 442, built in iteration 284): a line wider than a block container whose `overflow` is not `visible` ends in "…" inside its content box — whole clusters and whole atomic boxes hidden from the end, a space before the cut kept, the first thing on a line kept however narrow the box, the "…" straight after what is left in the block's font, colour and shadow on the line's baseline, an underline under what was kept and not under the "…" (`alo-layout`'s `ellipsis.rs`, rules measured in Chrome 154). Only what is drawn changes: every box and line keeps its numbers, and an agent reads the whole text. Only lines that overflow are ended, a wrapped box's too. A string and two values are refused and recorded, as Chrome refuses them. Corpus cases `alo-agenda-calendars` and `text-overflow-ellipsis`. Not yet: an atomic box in a `nowrap` line wraps rather than staying to be hidden (447), and a "…" after a space that is a text box of its own stands before the space (448); right to left waits for bidirectional text
- [2] **A variable font is one file and many weights**: the `wght` axis read as the range it covers rather than the one instance `OS/2` names, so one file answers a request for 400 and one for 700 — and is shaped, measured and outlined at the weight it was set to
- [2] Web fonts as pages actually ship them: WOFF2, the rest of a variable font's axes, and loading that does not flash
- [2] **Input methods.** A browser that cannot take Japanese or Chinese input is not a browser in those countries
- [2] `contenteditable`, which every rich text box on the web is built on
- [2] Bidirectional text end to end
- [2] Selection, carets, and text input
- [2] Hyphenation, `text-wrap: balance`

## Paint

- [1] Glyph rasterisation: an outline read from the font, scaled, and filled into coverage — how much of each pixel a letter covers
- [1] One shape type and one rasteriser, so a glyph and the box behind it agree along the edge they share. **A shape's edge pixels are sampled sixteen by sixteen** (queue item 459, `alo-paint`'s `refine.rs`): the rented rasteriser fills it, and every 16-pixel tile an edge passes through is filled again four times larger and averaged, so a small curve is covered as its area says (within 3 of 255 on a dot and a corner rounded 2.5) and alike on both sides, at the cost of the edge's pixels alone. Letters are sampled the same way (queue item 462): against Chrome 154's screenshots of `DejaVu Sans` at 12, 13 and 16 px (`alo-corpus`' `letters-at-three-sizes`), every line is nearer than with the rented four by four, and within a measured bound held by `letters_against_chromes.rs`. What remains is Chrome's own font rasteriser on macOS — it thickens every letter, about an eighth more ink (576), and pulls an `x`'s and a capital's top up to a whole pixel (577)
- [1] A display list from the box tree
- [1] **A software rasteriser to a PNG.** Deterministic and diffable, needing no GPU and no window — which is what makes every item testable from the first one
- [1] Rounded corners, and clipping to them — one question asked twice: what shape is this box. A corner's pixels are covered as its area says (459)
- [1] **Shadows and gradients**: `box-shadow` (with `inset`), `text-shadow`, `linear-gradient`, `radial-gradient` — a shadow is coverage blurred, so what is behind it is not blurred with it
- [2] **A background of several layers** (queue item 313): `background` and `background-image` as a comma list, the colour beneath them all and the first layer written on top. A radial gradient is a `circle` or an `ellipse`, sized by any of the four extent keywords and centred `at` a position of one or two keywords, percentages or pixel lengths. Gradient stops are mixed with their alpha premultiplied, so a fade to `transparent` keeps its colour. A `url()` layer is recorded and the others drawn (its picture is 311); a layer that also says where its picture sits, how big it is or whether it repeats is refused whole and recorded, as are a radial size written as lengths and a position of three or four parts. Opened by `alo-meet-greeting`, whose corner tint it draws
- [2] **Two-toned borders**: `inset`, `outset`, `groove` and `ridge`, in a darker and a lighter tone of the border's colour, each side mitred so that a corner splits where the tones change, and a groove's halves following a rounded corner (queue item 190), **in Chrome 154's tones** (queue item 508): 0.33 off or on the brightest channel, hue kept, and a colour no brighter than `rgb(32, 32, 32)` drawn lighter instead, so a black `outset` is 168 and 84 as in Chrome, and one brighter than `rgb(235, 235, 235)` its own lighter tone; a button whose page sets its background is Chrome's screenshot on every side to the level (not yet: a mitre's diagonal pixel, 518), and a fieldset's `groove` — the user-agent sheet's, as other browsers draw it — broken by its legend, the legend's part of the line cut out and the corners left whole (queue item 267)
- [2] **Patterned borders**: `dashed`, `dotted` and `double` — dashes and round dots spaced so a side starts and ends on one, a corner shared without a seam, `double` as two lines a third of the width each following a rounded corner, and a bound on how many pieces a page can ask for (queue item 266), and beside a fieldset's legend laid along the whole side and cut where the legend is (queue item 268)
- [2] **A border is whole device pixels wide** (queue item 430): a border width is snapped as CSS Values 4 computes one — one of a device pixel or more floored to whole device pixels, a smaller one that is not zero made one — once it is a number, so `.3rem` is 4, `0.5px` is 1 and a `calc()` is snapped after it is worked out, and layout and paint both use what is drawn. A device pixel is the screen's (item 560, ADR 0057): laid out at a scale of two, `4.8px` is 4.5 and `0.3px` is 0.5, as Chrome 154 under a real scale of two floors them; every page a window shows is laid out at one until item 562. alo Sites' editorial section, `border-left: .3rem`, starts its content at 44 as Chrome does; it was 44.8, drawn 4.8 wide. Corpus case `border-widths`. Opened by `alo-sites-full-page`
- [2] **A box is drawn on whole device pixels** (queue item 565): everything a box paints for its own area — its background colour and image, its border on both edges, a rounded corner, an inset, spread or blurred shadow, each piece of an inline box — is drawn from its border box with each edge rounded to the nearest device pixel, a half towards the right and the bottom, as Chrome 154 was measured to on 1,536 boxes at one and at two (ADR 0057). A shadow's offset is added after, and a transform moves the snapped box; a fieldset's border is snapped from the line through its legend. Layout, hit testing and the agent's numbers are unchanged: only the drawing moves. Corpus case `backgrounds-on-device-pixels`, Chrome's pixels along every box's middle at both scales; and `checkbox-beside-text`'s nine rows of backgrounds beside controls, every pixel outside the controls and text Chrome's exactly (queue item 461). Not yet: a clip, a picture's box and an `<svg>`'s viewport (575), and a spread shadow's square corners (573)
- [2] **A function in the `border` shorthand is read whole** (queue item 435): the shorthand is split into its top-level parts rather than at every space, so a colour written `rgb(29 78 216)`, `hsl()` or `color-mix()`, and a width written `calc()`, are read and drawn — they were torn into words none of which was a colour or a width, and the border was drawn in `currentColor` or not at all. alo Sites' presented cards, `border: 1px solid color-mix(in srgb, var(--section-text) 14%, transparent)`, are ringed `rgb(23 33 43 / 0.14)`, as Chrome 154 computes them. Corpus case `border-colour-functions`. Found while building 429
- [1] **Transforms and opacity**: `translate`, `scale`, `rotate`, `skew`, `matrix`, about a `transform-origin`; `opacity` as a group drawn once and composited once, never box by box
- [1] **Reference renders**: a committed corpus, each with its expected image *and* its expected box tree
- [1] Hardware acceleration — after the software path is correct, never before
- [2] Compositing layers, and scrolling that does not repaint the world

## ★ The agent surface (ADR 0002)

The reason this exists rather than a faster fork of somebody else's engine.

- [1] ★ **The layout tree read as roles, states, positions and text** — a *view* of the tree that draws the page, never a parallel structure. Two structures eventually disagree, and the agent acts on whichever is wrong
- [1] ★ **`aria-current`**, as the word the author used rather than a flag — a nav item being the current *page* is not the claim a cell being the current *date* makes
- [1] ★ Roles are **declared, not inferred**. A box says it is a list, a row, a field, a button — guessing that from appearance is what screen-scraping already does badly
- [1] ★ **Typed verbs**: activate, put text, scroll. **No verb takes a coordinate**, because a coordinate is a guess about a layout that may have moved between the reading and the acting
- [1] ★ A verb **changes the page**: text into a field, a checkbox ticked, a radio chosen and its group un-chosen — rendered again from the same document, so every id an agent is holding still names what it named
- [1] A field shows what it holds; a password shows one dot a character, and the dots are not in the agent tree
- [2] A form control draws its state — a tick in a checked box, a dot in a chosen radio, a dash in one that is neither, in `accent-color` if the page names one
- [2] A control nobody can operate says so, **and still says what state it is in** — "you cannot change this" and "this is off" are different things to be told. **In Chrome's colours** (queue item 509): Chrome 154's computed text, tint, edge and fade for every kind of control, measured, and a page's `background` or `border` on one beats the sheet's longhands, because a shorthand resets what it covers (`alo-style`'s cascade)
- [2] A focused control draws a focus ring
- [1] ★ **One element, one thing to read** — an inline box broken around a block is read as one node, named by everything the element contains and positioned everywhere it was drawn, with the block inside it rather than beside it
- [1] **A name read from what is inside a thing has its spaces where Chrome's has them** (queue item 530): a space wherever a box stands apart from the next, and a box stands apart unless it is an inline box in its parent's lines. So a flex or grid item is apart even when written `inline`, and so are an `inline-block`, `inline-flex` and `inline-grid`: `<div role=tab style=display:flex><span>Notes</span><span>Stopped</span></div>` is "Notes Stopped", while two `<span>`s side by side in a line are still one word, as Chrome 154's accessibility tree reads them (`alo-agent`'s `name.rs`, `tests/a_name_across_its_items.rs`). Not yet: a `<br>` and a picture drawn as its `alt` inside a line (534)
- [1] ★ Reading is never watching — the tree is exposed when asked, and `alo-os`'s capability model decides who may ask
- [1] ★ **The same tree is the accessibility tree.** A screen reader and an agent want identical facts, and two implementations would guarantee one is wrong — so EN 301 549 conformance and agent capability are one piece of work, not two competing budgets
- [2] ★ The same tree over ordinary web pages, which no browser can offer today because none of them owns both halves. **Read and driven by name on two frozen pages nobody wrote for us** (queue item 131). On the HTML specification's own pizza form an agent fills each field by the words of the label wrapped around it, chooses a size and toppings, and reads every one back. On the first web page it follows links by their words. Refusals are the same as on alo's screens. A field's text inside its label is read as the field's. Text put into a `<textarea>` becomes its content, which is what draws and reads, until a control's value is held apart from its markup (469). A `time`, `month`, `week` or `datetime-local` field refuses text by name (`Unbuilt`) until one draws what it holds (388)
- [2] ★ **An agent outside the browser reaches that tree** (ADR 0055, queue item 496): over a Unix socket the browser process holds, which only the person's own programs can open — never a port a page could reach. A tab is named by its address and never by a number; a page is read as a tree and acted on by name, one JSON line at a time; every answer is a kind with its own fields, `verb.rs`'s refusals under their own names, and the English is beside it rather than instead of it; every bound — a line, a name, an address, nesting, a tree's depth, how many agents at once — is refused, never cut. The browser process records what each agent read and changed, and nothing over the socket can read the record. The contract is `docs/contracts/agent-socket.md`. Proved from a separate process filling and sending the frozen pizza form. Windows is undecided
- [2] ★ **An agent kept to the sites a person names** (ADR 0055 § 8, queue item 497): `alo --agent-site ADDRESS` keeps every agent to those sites — ADR 0005's scheme and registrable domain — and any other is refused as `NotGranted` with its site, before any tab is looked at, so a refusal never says what is open; the list of tabs leaves out what an agent may not reach. A tab is judged by the page it shows, so a redirect or a page that navigated itself is refused where it ended. Every record entry names the site. Ours, a refusal and never a grant: it only narrows what reaches the socket, no agent can name or widen it, and it is never presented as another system's grant. Proved from a separate process on the frozen pizza form beside a redirected tab, and over real tabs for a page that navigates itself
- [2] Assistive technology bridges — AT-SPI on Linux — over that same tree

## Embedding

- [1] alo's sign-in screen, then Settings, rendering correctly from their own markup and `tokens.css`, matched against a committed reference render **and** an expected box tree — the exit gate of stage 1, and reachable on any laptop
- [1] A rendering surface with no operating system behind it: HTML and CSS in, a PNG and a box tree out, both files. Everything in stage 1 is verified through it, which is what makes the stage buildable by anybody who clones this
- [2] A surface alo OS's shell can render into — **retiered from [1]**: it is the one embedding item needing a compositor that does not exist, and tiering it [1] is what put an unreachable dependency inside stage 1
- [2] Several documents at once, the shape tabs need

## The network — stage 2

- [2] **URLs**: WHATWG parsing, resolution against a base, IDNA and punycode — rented, because whether two spellings are one host is a security question with a Unicode table for an answer
- [2] **The shape of a load**: a request that says who asked and what for, a response of bytes, headers that keep their order and their repeats, and a media type — with `data:` and `file:`, so that HTTP is one more arm rather than a second pipeline
- [2] **Which encoding a page is in**: byte order mark, `Content-Type`, `<meta>`, then UTF-8 — the tables rented, the algorithm ours, and a page that decoded badly saying so
- [2] **HTTP/1.1**, ours: a request out, a response in, and body framing that refuses every message saying two things about where it ends — which is what request smuggling is
- [2] A truncated body is an **error**, not a short page
- [2] **Content encodings**: gzip, brotli, zstd, and `deflate` in both the
  spelling the specification asks for and the one servers actually send
- [2] **`Transfer-Encoding` as the list it is** — `gzip, chunked` is chunks
  holding a gzip stream, and the chunks come off first. A coding after
  `chunked`, one we cannot undo, or a compressed body the connection closing is
  the only end of, is refused by name
- [2] **A page's own style sheets** — `<style>` and `<link>` together, in
  document order, with an alternate sheet left alone and a missing one recorded.
  A rule or declaration whose blocks nest more than thirty-two deep is dropped
  with an issue saying so, and the rules around it are kept; a sheet nested a
  hundred thousand deep is a refusal rather than a crashed renderer (queue item
  330). **A page a renderer loads asks for its own linked sheets** (ADR 0035,
  queue item 348). Each URL is asked for once per document, a link a script
  adds included, and the page's `<meta>` policy is applied first; a `data:`
  sheet and a link with `integrity` are refused by name. The browser process
  fetches each as a style request under the page's header policy, and sends
  it only when the answer is a 2xx `text/css`. The renderer reads the sheet
  as UTF-8, and the page is drawn again with it. At most 64 sheets are asked
  for per document. **A corpus case that runs script is answered its
  sheets** from the files its `linked.txt` froze, each name resolved against
  its `address.txt`, through the browser process's own decision and check
  and typed by its extension (queue item 349); a file it froze that its page
  never asked for — a picture, until item 350 — is refused by name. alo
  Sites' call-to-action section is frozen so (`alo-sites-cta`). **The
  window waits for a load's sheets** (item 351, ADR 0041): its first frame
  is held until every sheet the load asked for has answered, or three
  seconds pass and the page is shown with *this page is shown before its
  style arrived*. A late sheet is still applied when it comes, and a sheet
  a script adds later holds nothing back. Requests are made on a network
  thread of their own, so a resize or a close is answered while a server
  trickles. Not yet: a bound on a whole exchange (item 376)
- [2] **`<img>` lays out at the picture's own size** and keeps its ratio when
  given one dimension
- [2] **A broken picture is its text** (queue item 413, ADR 0046): an
  `<img>` whose picture is not shown and whose `alt` is not empty is an
  inline run of that `alt` in the element's own style, wrapping with its line
  and taking no `width`, or a block of it when its style says `block`; any
  other `<img>` without a picture is a replaced box of nothing, the size its
  style gives and 0 along any side left `auto`; no icon is drawn (418); an
  agent reads it as `[broken]`, named by its `alt` once. Whether a picture is
  shown is decided once, by `alo-renderer`'s `pictures.rs`, before the boxes
  are built, and handed to `alo-box` as a `Shown`; `img` is `display:
  inline`. Not yet: `<img>`'s `width` and `height` attributes as its size
  (419), and a picture still on its way (420)
- [2] **A picture under a transform is drawn transformed** — rotated, skewed
  or mirrored with its box, its outline anti-aliased like any other edge
  (queue item 178); sampled nearest-neighbour inside (item 179)
- [2] **PNG and JPEG from a page**, with the format taken from the bytes rather
  than the name, and the same bounds either way
- [2] **GIF and WebP from a page** (queue item 180): GIF87a and GIF89a with
  transparency, and lossy, lossless and extended WebP with alpha. An animated
  one shows its first frame rather than nothing. Each size a file declares is
  bounded before the decoder reserves memory for it, including a GIF frame
  larger than its screen and a lossy bitstream larger than its WebP canvas. A
  WebP frame whose picture is not the frame's own size is refused rather than
  allowed to panic the rented decoder. AVIF is refused until the decoder ADR
  0021 chose, `rav1d`, releases the safe API it is to be called through (269)
- [2] **PNG pictures from a page**, read tolerantly and bounded before anything
  is allocated — a hundred-byte file cannot ask for seventeen gigabytes
- [2] **A wrapped inline is more than one rectangle** — and offscreen only when
  every piece of it is
- [2] **`text-decoration`**: underline, overline and line-through, stopping at
  the end of the inline rather than the edge of the line
- [2] **Forms**: labels that wrap their control, fieldsets named by their
  legend, and a radio you can tell from a checkbox. **Which control a label
  labels is HTML's rule, held once** (`alo-dom`'s `label.rs`, queue item
  554): a `for` names the first element with its id when that is
  labelable, and nothing otherwise — never the control it wraps — and a
  label without one labels the first labelable element inside it, a hidden
  input passed over. The agent's names and a hovered label ask the same
  rule, and each field is named as Chrome 154's accessibility tree names
  it. An empty date field
  draws how a date is written, `yyyy-mm-dd`, in its own colour (queue item
  385, ADR 0042; opened by `alo-sites-booking`), and a field's text is drawn
  with nothing of the field's own around it. A checkbox and a radio are 13
  by 13 with their border, carry Chrome's user-agent margins and stand on
  the baseline by their bottom border edge, so a label beside one starts
  where it does in Chrome (queue item 443). Each draws Chrome's own
  picture of itself rather than a CSS border: a rounded square or a
  circle, white inside a grey edge, filled with the accent colour and
  ticked when checked, a dot in a chosen radio, and translucent greys
  when disabled — every shape and colour measured from Chrome's
  screenshots and the case compared with Chrome's pixel by pixel (queue
  item 454). A control laid out at a fraction of a pixel draws its picture
  on whole pixels, each edge of its box rounded as Chrome rounds it, while
  the box stays where layout put it (queue item 457). Not yet: the person's region
  for that order (387), the calendar button (386), and what an empty time,
  month or week field shows (388)
- [2] **A fieldset's border, broken by its legend** — the legend sits *in* the
  block-start border rather than above it, and the border is drawn in the two
  pieces it leaves. The band it sits in replaces that border rather than adding
  to it, so the fieldset is as tall as a browser draws one
- [2] **`border-radius` in per cent**, resolved against the box
- [2] **Links that look like links**, with no visited colour — `:visited` never
  matches, so history is not readable from a page
- [2] **The user-agent sheet's typographic defaults** — what a page looks like
  before anybody styles it
- [2] **Shorthands compete with longhands in the cascade**, so an author's
  `padding: 0` beats a user agent's `padding-left`
- [2] **`place-items`, `place-self` and `place-content`** set their `align-*`
  and `justify-*` pair — one value for both, two for block then inline, a
  two-word value such as `safe center` counted as one — so a grid centred
  with `place-items: center` is centred
- [2] **`padding-block` and `margin-block`** set the top and the bottom —
  one value for both, two for start then end — which is exactly what they
  mean in the one writing mode this engine lays out, `horizontal-tb`; they
  compete with `padding-top` and the rest by order, as CSS Logical says
  (queue item 379, opened by alo Sites' bento features card; corpus case
  `block-axis-spacing`). Their longhands, `padding-block-start` and the
  rest, are the side they name in the same way
- [2] **`margin-inline` and `padding-inline`, and the inline-axis
  longhands** (`margin-inline-start` and the rest) set the left and the
  right, start first, and compete with `margin-left` and the rest by order
  (queue item 380, opened by alo Sites' text-and-image section, whose
  overlapping card is `margin-inline-start: -4rem`; corpus case
  `alo-sites-text-image`). **This is left to right's answer**, written in
  where the declaration is: a page with `dir="rtl"` gets them on the wrong
  side, as it gets its text, flex rows and grid columns, since nothing reads
  `direction` yet (421)
- [2] **`gap` sets `row-gap` and `column-gap`** — one value for both, two
  for the row gap then the column gap — split where it is written, at the
  spaces between values and not those inside a function, so `gap:
  clamp(2rem, 6vw, 5rem)` is one gap of 48 on an 800-wide page rather than
  three pieces read as none, and `gap` and its longhands compete by order
  (queue item 397, opened by alo Sites' contact form, corpus case
  `alo-sites-contact-form`). A `gap` of more than two values is refused and
  recorded where layout reads it
- [2] **Fonts handed across the boundary** — the browser process opens the
  files, the renderer opens nothing
- [2] **A font a page asked for by name, fetched on demand** — a renderer says
  which families it wanted and did not have, the browser process looks for each
  on the machine by the name the **font** gives itself rather than the one on
  its file, and sends over what it finds. A family that genuinely is not on the
  machine comes back named, and the page says in words what it was drawn in
  instead — never a stable render nobody can explain. A name longer than any
  font's family can be (512 characters, the longest `name` record this engine
  reads) is not asked for, and the page says so, quoting it by its beginning
  and its length (`alo-renderer`, queue item 244): it could only ever come
  back *not here*, and carried whole it let a page near the size limit make
  its own load report too large to send. Asking for it cut short is not done,
  because that would ask for a different font
- [2] **A font's own name, read in the encodings older than Unicode** — the
  Macintosh `name` records several of the fonts macOS ships carry *instead* of a
  Unicode one, in Mac OS Roman and Mac OS Cyrillic. The other Macintosh
  encodings are read by nobody here rather than guessed at as their near
  relatives, and a Unicode name wins wherever a font has one. No family anywhere
  comes from a filename
- [2] **A font's own name, read in the language a page would ask for it in** —
  a `name` table holds the same family once per language, and macOS's system
  font holds it thirty-five times. The unlocalised record wins where a font has
  one, because that is the name the font calls itself and the platform reads it
  the same way; English wins over every translation of it; and a font that
  states no English name at all is still filed under its own, since a font in
  one language is a font somebody has
- [2] **A face's weight and slant, from the font** — the `OS/2` table states
  both, so `Helvetica-Oblique` leans and `InterDisplay-SemiBold` is 600 rather
  than everything being one of the two words a filename might carry. A weight of
  zero is a font that did not say rather than the lightest face there is, and a
  font with no such table at all is still a face: normal, upright, and drawn
- [2] **A slant the font does not have** (queue item 382) — `font-style:
  italic` or `oblique` asked of a family with no slanted face is drawn in the
  upright face leaned 14° about its baseline, which is what `font-synthesis`'
  initial value allows and CSS Fonts 4 § 3.3 makes an angle-less `oblique`.
  Its advances are the upright face's, so a line's layout is unchanged and
  only its ink leans; a family that has a slanted face is given it and never
  leaned twice. Opened by `alo-sites-testimonials`, whose featured quote is
  italic. Not yet: `font-synthesis: none`, a synthesised bold, and the angle
  of `oblique <angle>` (383)
- [2] **What `sans-serif` means on this machine** — `serif`, `sans-serif`,
  `monospace` and `system-ui` are decided by the browser process from the
  families it actually found, and handed to a renderer with the fonts, because a
  confined renderer may not look at the machine. Each keeps every candidate that
  is here, in preference order; `cursive` and `fantasy` are answered by nobody
  rather than guessed at; and a machine that has none of them still says so,
  rather than a page being drawn in whatever font sorted first
- [2] **Renderers confined by the platform's own sandbox** (macOS) — no file
  read, no write, no socket, each watched failing rather than assumed
- [2] **One renderer process per site** — two sites are two processes, and
  killing one leaves the other running
- [2] **A renderer nothing wants any more is stopped** — closing the last tab
  on a site ends that site's process, and closing one of two tabs on a site
  ends nothing, so the memory one process per site costs is paid for pages
  somebody still has open rather than held until a ceiling evicts them
- [2] **A tab that keeps the last frame it painted when its renderer dies, and
  says what happened** — every tab in the dead process and no other tab
  anywhere, the picture still on the screen rather than a blank rectangle, and
  a fresh process only when a person asks for the page again
- [2] **A renderer that stops answering without dying is given up on** — an
  exchange has a bound, so a renderer that is alive and silent costs its own tab
  rather than freezing the browser and every other tab with it, and one that is
  merely slow is still waited for
- [2] **A wire format for the renderer boundary**, where a message from a
  renderer is untrusted because a renderer is the process that parsed the page
- [2] **HSTS**, ignored over plain HTTP so it cannot be used as a weapon
- [2] **Mixed-content blocking** — a script refused outright, an image tried
  over TLS first, and `http://localhost` treated as secure because it is
- [2] **Referrer policy**, defaulting to origin-only across sites and nothing at
  all across a downgrade. The origin goes out written as a URL,
  `https://example.com/`, as every other engine sends it
- [2] **The same-origin policy, CORS and preflight** — a page may send almost
  anywhere and may read almost nowhere, and a wildcard never covers a request
  that carried credentials. A request is unsafe by the *value* of its
  `Content-Type` as well as by the name, so a JSON post is asked about with the
  header it is unsafe by named in the question
- [2] **Content Security Policy** — `default-src`, `script-src`, `style-src`,
  `img-src` and `connect-src` enforced, with `'self'`, `'none'`, schemes, hosts
  with wildcards, ports and paths, nonces and `'strict-dynamic'`. A source
  expression this engine cannot read is kept and matches nothing, and the
  directive holding it is kept whole — a policy is never widened by our not
  understanding a word of it. A repeated directive keeps the first, so appending
  to the header cannot widen a policy, and two policies are an intersection
- [2] **A policy a site is only watching blocks nothing** —
  `Content-Security-Policy-Report-Only` is read as what it is, and what it
  objected to is reported rather than acted on
- [2] **A page's author is told about inline script their policy refused** —
  or, under a policy they are only watching, would have refused: the script
  runs, and a report naming `script-src` and `inline` is posted to wherever
  the page's own headers said. Only the part of the browser that holds the
  network writes and sends a report, from the headers it fetched itself, so a
  page that has taken over the process drawing it cannot point a report
  anywhere else or make it say anything else, and cannot make the browser send
  more than 64 for one load. A policy written into the page's markup with
  `<meta>` is obeyed but not yet reported (queue item 240). Inline style is
  reported the same way, a `<style>` or a `style` attribute, naming
  `style-src`, when the page loads and whenever a later redraw finds one a
  script, an agent or a fetch's answer put there (ADR 0034, queue item 346).
  Each element, placement and text is reported once while the page is
  loaded, so a page drawn sixty times a second reports one refused attribute
  once, and no answer carries more than 64
- [2] **Inline content allowed by its hash** — `'sha256-…'`, `'sha384-…'` and
  `'sha512-…'` computed over the content and compared with what the policy
  named, in either base64 alphabet. A digest that mixes the two alphabets, sets
  bits that stand for no byte, or is the wrong length for the algorithm it names
  allows nothing: a hash source is a permission, and every laxness in reading one
  is a policy wider than its author wrote
- [2] **A `style` attribute allowed by its digest, and only where the page said
  `'unsafe-hashes'`** — content with no element of its own is the shape an
  injection most often takes, so a hash written for a `<style>` element never
  reaches one on its own. The keyword has to be in the directive that decides,
  it allows nothing by itself, and a refusal that names it says *no digest
  applies here*, which is a different thing to be told than *your digest does
  not match*
- [2] **A directive this engine does not act on is named** rather than assumed
  to be protecting the page
- [2] **A violation is reported to whoever the policy named** — `report-uri`
  and `report-to`, in the two documents collectors read, from an enforced
  policy and a watched one alike. A group name is resolved against the page's
  own `Reporting-Endpoints` header, and one nobody defined is said in words
  rather than dropped
- [2] **A report says a cross-origin URL as its origin and nothing more.** A
  report is posted to a server the page chose, so a full URL in one would be a
  way for a page to read a URL it was refused — a capability token in
  somebody else's query, where a redirect ended. A `data:` URL is reported as
  its scheme alone, and the page's own URLs lose their fragment and any
  credentials written into them
- [2] **A report that cannot be sent is not a load that fails.** A collector
  that is down, that answers an error, or that a public page pointed at this
  machine is a delivery that did not happen and is named as one — silence
  would make a policy reporting nowhere look exactly like a policy nothing
  violated
- [2] **The preflight cache** — a second request of the same shape sends no
  `OPTIONS`, and one of a different shape still does. Partitioned by top-level
  site, keyed by the asking origin as well, expiring on the caller's clock and
  capped at two hours however long a server asked for. A `*` is remembered as
  the method and headers it allowed rather than as a standing permission
- [2] **A request that sends something**, over both protocols — a body written
  after the blank line in HTTP/1.1 and in `DATA` frames in HTTP/2, cut to the
  frame size the server said it would read, with a window that closes part way
  through **waited on** rather than overrun, and the stream closed rather than
  left open when a server answers before it has finished reading. The length a
  request states is always the length of its bytes, never a header a caller
  wrote
- [2] **An interim response is not the answer** — `103 Early Hints` and its
  kind are read past on both protocols, whether or not anything asked for one,
  and a server that only ever says something first is refused. An `Expect` is
  refused by name, because an expectation is a promise to wait and nothing here
  can bound the waiting
- [2] **HTTP/2, negotiated by ALPN and spoken** — the protocol chosen during the
  handshake, so no request is ever sent twice to find out which one it is
- [2] **HTTP/2 streams and flow control**, with the CONTINUATION flood refused
  by a bound on the whole header block rather than on each frame
- [2] **HPACK**, against the specification's own worked examples — with the
  Huffman codes derived from the canonical structure rather than transcribed
- [2] **HTTP/2 framing**, with every length checked before anything is reserved
  and the padding underflow refused by name
- [2] **Names resolved by the machine's own resolver** (ADR 0008), with DNS
  rebinding refused — a page on the public web cannot be made to reach a private
  address
- [2] **Cookies, partitioned by default** (ADR 0007) — keyed by the setter *and*
  the top-level site, `SameSite=Lax` when a site says nothing, and the
  `__Host-`/`__Secure-` prefixes enforced rather than parsed
- [2] **The site is the registrable domain**, decided against the public suffix
  list — so two subdomains of one organisation are one site and share what a
  site is given, and two organisations under one suffix are two sites however
  alike their names look. One answer, used by the cookie jar, the cache and the
  process split
- [2] **The list is a snapshot, and its age is a message rather than a silence**
  — the day it was taken is recorded beside the version it was taken at, checked
  against what is actually compiled in, and the build fails once it is six
  months old. A stale list is two organisations sharing one site, which is the
  direction that costs
- [2] **A cookie for a whole public suffix is refused** — `Domain=co.uk` is a
  cookie for every school and council in the country, and nothing about the
  shape of the string says so
- [2] **An HTTP cache**: freshness, `Age`, revalidation with `ETag` and
  `Last-Modified`, and `Vary` as a contract so one reader never gets another
  reader's page
- [2] **A cache that survives a restart, partitioned by top-level site** exactly
  as cookies are (ADR 0011) — so no site can time a load to learn where else you
  have been, and an entry only you were ever given cannot follow you between
  sites
- [2] **What must not outlive the session is never written**, rather than
  written and deleted: a `private` response, a request that carried
  `Authorization`, a response carrying `Set-Cookie`, anything that did not come
  over HTTP, and a body that is not the length it was said to be. All of it
  still reusable from memory, and a session that is not meant to persist has no
  cache directory at all
- [2] **A cache file is untrusted input** — a version discarded rather than
  guessed at, a checksum over the whole entry, every length checked, and
  anything that does not read treated as a miss rather than as a failure to
  load. The directory and its files are private to their owner, and clearing it
  really removes them
- [2] **Redirects**, bounded and loop-detecting, with `Authorization` dropped
  at an origin boundary and `file:`/`data:` refused as destinations
- [2] **A download that stops asks for the rest of itself**, over HTTP/1.1 and
  over HTTP/2 — with a `206` required to begin exactly where it stopped, a
  `200` answering a range request never appended, and nothing encoded ever
  spliced
- [2] **A connection that ended is told apart from a peer that misbehaved**, in
  HTTP/2, so bytes that arrived on a stream the server gave up on are kept
  rather than thrown away with an error — and a server that closed is reported
  as the connection ending whether a read or a write is first to find it gone,
  so the reason never depends on timing (queue item 455)
- [2] **A decompression bomb is refused** — the bound is on what comes out,
  because every other bound in a loader watches what comes in
- [2] **Connections kept between requests**, with the retry that has to come with them: a reuse that fails before a byte arrives is tried again, and a request that must not happen twice never is
- [2] **TLS**, rented, with verification that cannot be turned off — no flag, no constructor, no feature
- [2] **A certificate refusal a person can act on**: what is wrong, what trusting it anyway would mean, and whether the fault has an innocent explanation — three things a caller cannot show one of without the others
- [2] **The origin as a value other code compares**, with an opaque origin that is the same as itself and nothing else — a `data:` URL, a local file, and any scheme nobody registered

## The process model — stage 2

- [2] **The process and sandbox model, designed before the first hostile page is ever loaded** (ADR 0005). One process per site, renderers with almost no privilege, the platform's own sandbox rather than one of ours, and work crossing as typed messages in one direction. Memory safety does not make this optional: Spectre is a hardware property, and the codecs we rent are not ours to make safe
- [2] A renderer that dies costs one tab and never the browser, and says so rather than leaving a blank rectangle
- [2] The transport, and the lifecycle that starts, reuses and reaps renderers
- [2] **Where one site ends and another begins — the origin, the site, and which of them gets a process.** The origin decides whether there is a site at all: where it is a scheme, a host and a port, the registrable domain widens it into a site and two tabs share a process, with the port left to the origin because two ports can already reach one another. Where it is **opaque** — a local file, a `data:` page, `about:`, a scheme nobody registered — there is no site, and the document is rendered in a process nothing else is ever put into, not even the same bytes opened a second time
- [2] What a **document inside a document** is given is still owed, because there are none yet: a sandboxed `iframe`'s opaque origin and `about:srcdoc` inheriting its parent's, and a `blob:` taking the origin of whoever created it

## The network — stage 2

- [2] **URLs, properly**: WHATWG parsing, origins, IDNA and punycode. Every security decision below is made against the origin this produces, which is why it comes first
- [2] TLS with `rustls`, and certificate errors a person can act on rather than click through
- [2] HTTP/1.1 and HTTP/2, with connection pooling and keep-alive
- [2] HTTP/3 and QUIC, after those are correct
- [2] DNS, with encrypted DNS as a choice somebody made rather than a default nobody was told about
- [2] Content encodings: gzip, brotli, zstd
- [2] Redirects, byte ranges, and downloads that resume
- [2] **The HTTP cache with real semantics** — freshness, revalidation, `Vary`. Subtly wrong here is invisible for months and then serves somebody a stale bank page. What of it may be written to a disk is a second question, and it has its own answer (ADR 0011) and now its own code: partitioned by top-level site like cookies, and a page behind a password never written down at all
- [2] **Cookies**: `SameSite`, `Secure`, `HttpOnly`, partitioned by default — the default is a product decision, not a parser detail
- [2] The same-origin policy, CORS and preflight
- [2] Content Security Policy, referrer policy, HSTS, mixed-content blocking
- [2] `fetch()` and `XMLHttpRequest`, over the same stack rather than beside it. **Decided, not built** (ADR 0032): a page never touches the network itself. Its fetch is a request the browser process decides from what it already knows about the page — where it came from, the rules its server sent with it, and whether a person, the page or an agent caused it — and makes through the same stack as every other request. What comes back is only what the page may read: a response from another site that did not agree to be read never reaches the page's process at all, not even as bytes it is told not to look at. A fetch that fails tells the page nothing about why and tells the person exactly why. A synchronous `XMLHttpRequest`, which would freeze the page while it waits, is refused by name. **The boundary and the decision are built** (queue item 334): the page's process sends every fetch its script asked for with its answer, and the browser process decides each one from its own copy of where the page is and what its server's policy said — refusing, by name, a URL that is not one, any scheme but `http` and `https`, a fetch the page's own `connect-src` forbids, an insecure fetch from a secure page, and a same-origin-only fetch to another site — and counts a fetch made after an agent's click as the agent's and one made after a page's own load or response as the page's. A page may have at most 64 fetches waiting at once; more are failures, said where the person can see them. A page's process that sends what no page could have asked for — a cookie it wrote itself, an origin it claimed — is refused as a process that broke the boundary rather than corrected. What a response may cross is decided before it leaves: a page's own site's response without its `Set-Cookie`, another site's only with the headers it agreed to show, and one it did not agree to be read with no bytes at all. **A page can call `fetch`** (queue item 335): its arguments are read as every browser reads them, a getter in them run in order, and what Fetch itself refuses — a URL that does not parse or names a user, a forbidden method, navigate mode, a body on a `GET` — rejects the promise rather than throwing; a header only the browser may set is dropped silently, as everywhere. The answer arrives later, as its own task: the promise is fulfilled with a read-only `Response` — its `type`, `status`, `ok`, `statusText`, `url`, `redirected`, `headers.get` and `headers.has`, `bodyUsed` and `text()`, the body readable once — or rejected with one `TypeError` that says nothing about why, and the page is drawn again with whatever its reactions changed. A new page lets go of every answer the last one was waiting for. A body may be a string or a `URLSearchParams`, which is sent as `application/x-www-form-urlencoded;charset=UTF-8` as Chrome sends it (queue item 472). Refused by name rather than half done: a `data:` or `blob:` URL, a `FormData` body (492) and any other body that is not a string, headers given as a list, and a `signal`, `cache`, `integrity`, `keepalive`, `priority` or `referrer` other than the default. A corpus page that fetches states where it was served from and is answered only from responses frozen beside it; anything it froze nothing for fails, as it would offline. **And the browser makes it** (queue item 338): through the same connections, cache and record as every other request, each step of a redirect decided again as Fetch decides it — who the page says it is (and `null` once a redirect has gone from one site to a third, so a server cannot be fooled into reading a page it never met), what it says it came from, and whether the person's cookies go, which they do to the page's own site unless it asked otherwise and to another only when it asked to include them. A request a form could not have sent to another site is asked about first, and the question is in the record before the request is; a server's cookies are kept only from an answer that carried the person's own, and a site inside another may set only the cookies it marks as meant for that; an answer from another site that did not agree is never read, and once a fetch has left the page's site, even its own site's answer must agree. A redirect is followed, stopped at or treated as a failure as the page asked, and refused before it is sent — and recorded by its rule — when it goes round in a circle, when a fetch that must stay on its own site would leave it, or when a secure page would be taken somewhere insecure. The page hears the server's own words for its status. A page's fetches are made one at a time between everything else the browser is asked to do, so a page that never stops fetching cannot stop its window closing; after each answer the page is drawn again, and if a fetch failed the person is told why. A fetch for a page that has since gone is not made at all. **Each step of a redirect is judged by the page's own `connect-src`** (queue item 340), as the first was: a redirect to a site the page's policy does not name is refused before it is sent and recorded by its rule, and a redirect within a site the policy names is followed even out of the path it named, because a policy that judged redirects by path would let a page learn where another site's server sent it. **A request may outlive its page** (ADR 0040; built for `navigator.sendBeacon`, queue item 369): a page may send a last report as it is left, and the browser makes it after the page has gone, by exactly the rules any fetch is made by: the same cookies, kept apart site by site, the page's own `connect-src`, and the record saying which page sent it and that it was a beacon. `sendBeacon(url, data)` sends text, or a `URLSearchParams` with its own type and without running its `toString`, as a `POST`, answers `true`, and never says what became of it; a URL that does not parse, or is not `http` or `https`, throws a `TypeError`. A page may have at most 64 KiB of such reports waiting at once — its own process answers `false` past that, and the browser counts again and refuses by name what a lying process claims past it. Anything else a page asks for as it is left is still refused by name. A beacon sent while the page is still open is made like any fetch, and its answer frees its share of the 64 KiB. Whatever is still waiting when the browser closes is not sent, and the record says so. **How `XMLHttpRequest` works is decided, not built** (ADR 0053, queue item 83): it asks for exactly what a `fetch` would, judged by the same rules and answered with only what the page may read, and the page hears its events in the order Chrome 154 was measured firing them, with Chrome's numbers — one `progress` for a body, none for an empty one. A request the page kept no reference to still reaches its listeners. A request aborted before it has left the page is never sent; one already sent is still made, and the page hears the abort as it would in Chrome. A synchronous request, which would freeze the page while it waits, throws the error Chrome throws for synchronous misuse. Still to come: `keepalive` on `fetch` (375), other kinds of body (a `Blob`, a form), and `XMLHttpRequest` when a page uses one (336), with its `timeout` (493), its other response types (494) and cancelling a request already sent (495)
- [2] WebSocket
- [2] ★ **Every request attributable** — which page, and which agent action, caused it. No other engine has needed to answer that, and an agent-driven browser that cannot is one nobody should trust. What is recorded, for how long, and who may read it is a decision rather than a detail, and it is made (ADR 0012): the cause is carried on the request rather than guessed at afterwards, a renderer never states one, the record is the session's except for what an agent did — which is kept until the person deletes it — and no page and no agent may read any of it. **The cause is carried**: three of them and no fourth, with no default, so a request that cannot say what caused it does not compile, and the requests the browser makes for itself — a redirect, a resumed download, a permission check, a policy violation report — each name whatever caused the thing they are about. **And the chain**: loading a page makes a document that records what caused its own load, so a fetch by that page walks back to the agent action that opened it and on to the person — both answers, in order, rather than one chosen. A page somebody opened themselves reaches no action at all, an action in one tab cannot reach into another, the walk stops rather than looping if it ever meets a document twice, and a chain that reaches a document too old to still be remembered says the piece is missing rather than reading like one that ended. **And the session's record**: every request the engine makes is a line in it, written where the request is made rather than by whoever remembered to — when, the cause, the method, the URL, the purpose, and what happened, and **never a body and never a header set**, because a record holding `Cookie` and `Authorization` is a file that logs somebody into their own bank. What the cache answered is a line that says so, and a hop a rule of ours refused is a line naming the rule, because *what did this page try to load, and what stopped it* is most of why anybody opens it. It dies with the process, it is bounded in lines and in bytes — a URL is as long as a page chooses — and it says how many it has dropped rather than quietly reading as a session in which less happened. Emptying it is real. No page and no agent can reach it: a renderer holds no pool, nothing crossing the process boundary carries a line, and the agent surface does not depend on the loader at all. **And what an agent did, kept until the person deletes it**: the small durable half beside the session's, holding only the requests whose chain reaches an agent action — so a person's own browsing is never written to a disk at all. Each chain is **frozen** as it is written, because a file read back next week has no documents left to walk against, and it holds numbers rather than identities because this morning's `action#0` is not last week's. A private window leaves **no file behind at all** — never written rather than written and deleted. The bound is counted in **actions**, so one action with three hundred requests in it cannot evict a week of ordinary ones, and the oldest action goes whole; a `data:` URL keeps its kind and loses its content, since a URL that *is* the content is a body wearing an address's clothes. It lives where the system keeps what it may not delete rather than beside the cache. A file that does not read is a **gap it counts** rather than an error, and it is left where it is because it is somebody's record. Deleting is real: the files, not a flag on them. **A navigation refused is a line too**: wherever a page asks to go somewhere it may not — as it loads, as it hears an answer, as an agent acts on it, as a person clicks — the refusal is written into the session's record under the method it was to go by and the cause the browser assigned, as a refused fetch is; a navigation let go writes nothing until it is made. Still to come: the action's own outcome beside the requests it caused

## JavaScript — stage 2

- [2] ★ **A JavaScript engine, ours, in Rust.** Stage 1 needs none at all, which is what removes the largest component of a browser from the critical path. What it is, what it is not, and why it is ours rather than rented is a decision rather than a detail, and it is made (ADR 0013): `alo-js`, a parser, a bytecode compiler and an interpreter in safe Rust, **correct before fast**, with no JIT until a measurement on hardware says the interpreter is why somebody reached for another browser and an ADR of its own weighs that against `unsafe` in the largest target in the browser. Renting is re-argued rather than inherited, and the close call is a Rust engine that already exists rather than V8: refused because a rented collector decides how the DOM is stored and a rented engine's limits are a stranger's idea of how much memory a script may make us allocate. Three clauses shape everything built under this heading — **absent beats approximate**, so a builtin we have not written is *not defined* rather than stubbed, because a stub is the one answer that defeats a page's own feature test; the engine **reaches nothing** — no network, no filesystem, no clock — so every capability comes from the embedder and the browser process never runs page script; and a script is hostile input that we then execute, so every allocation it can cause has a bound of ours, the interpreter is interruptible by the browser rather than by a clock of its own, and it never panics on any program
- [2] Lexer and parser to an AST — the current language, not ES5. **The lexer is built** (`alo-js`, queue item 70): source text into tokens, one token at a time, and the thing worth knowing about it is that **it does not guess**. A `/` is a division sign or the start of a regular expression and nothing in the characters says which — `a /b/ g` is three divisions or one regular expression, and which one it is decides what the program does. Every editor guesses from the token before and is wrong on real code; here the caller states which it expects, every single time, so the ambiguity is settled by the thing that knows rather than by a rule of thumb. The same goes for the `}` that ends a `${` inside a template. Everything the modern language spells is read: numbers in four bases with separators and `BigInt`, strings as the UTF-16 code units they really are — so a half of a surrogate pair survives being read — templates with both what the author wrote and what it means, private class members, names in any script Unicode has, and a word spelled with an escape remembered as one, because `if` is a name and not the keyword `if`. The three legacy forms ADR 0013 sends to the tail are refused **by name** rather than quietly mis-read: `0755`, `'\1'`, and HTML-like comments. And it never stops: every truncation and every cut of a nasty script is a refusal that says what was wrong and where, because a renderer that crashes on a script is a page that can take the tab down. The parser is queue item 204
- [2] **A parser, to a syntax tree** (`alo-js`, queue item 204). The tokens become a program: statements, declarations, functions, classes, patterns, modules, and every node saying which bytes of the source it came from — because a stack trace and `Function.prototype.toString` are both built from that and neither can be added afterwards without touching every line that makes a node. The second ambiguity the lexer named is settled here: `(a, b)` and `(a, b) => c` are the same characters until the closing parenthesis has been passed, so the parameter list is **tried and put back**, and a `(` that turned out not to be one is remembered so a page that nests them pays for the attempt once rather than at every level. Where the specification refuses to choose, so does this — `a ?? b || c` and `-a ** b` are refused rather than given a reading — and `with` is refused **by name** (ADR 0013 § 3). Automatic semicolon insertion is all three of its rules, so `return` alone on a line returns nothing. And there are two ceilings that are ours, because there are two ways a stranger's file makes something here go too deep. A script of twenty thousand open brackets is a refusal, and the parse runs on **a stack of the engine's own** rather than the caller's, because a limit somebody else chooses is not a limit. The second one was found by the work that comes next and is the less obvious of the two (queue item 208): `a.b.b.b…`, `a()()…`, `a?.b?.b…` and `a+a+a…` are read in a **loop**, so they cost the parser nothing and build a tree as deep as the file is long — and everything that reads that tree afterwards walks it one step at a time, starting with the ordinary business of throwing it away. Sixty thousand `+` signs, which is a hundred and eighty kilobytes of somebody else's page, parsed in a moment and then took the process down. Both are refusals now, each saying which of the two it is. What the bound deliberately does **not** count is width: a page holding an array of fifty thousand numbers or an object of twenty thousand entries is ordinary, and a limit that added those up would refuse most of the real web
- [2] Two frozen pages' own scripts, read: alo's own service worker and alo's own theme generator, in the corpus beside the pages, so what the lexer and the parser are judged against is real code somebody wrote for a different purpose rather than examples written to match the code. The second holds six regular expressions and no division at all, which is what makes the `/` question a real one rather than a theoretical one — a test walks the whole tree and requires that not one of them was read as arithmetic
- [2] **Function headers obey the body's strictness** (`alo-js`, queue item 205). A plain parameter or function binding named `eval`, `arguments` or a strict reserved word is refused even when the `"use strict"` that forbids it comes later, inside the body. Duplicate strict and arrow parameters are early errors. Rejection happens before any statement runs, including for a function never called. A method's property name and legal lexical shadowing remain valid, and `('use strict')` is an ordinary expression that ends the directive prologue. Complex parameter forms, private-name checks, named-expression binding separation and loader attributes remain item 222; parsing alone does not certify all early errors.
- [2] **A bytecode compiler and an interpreter: correct first.** **The machine is built, and functions run** (`alo-js`, queue items 72, 209, 212, 214 and 216). A program is turned into a flat list of instructions and then run, rather than being walked as a tree — the difference matters later rather than now: a paused function, which `await` and generators both are, is a thing you can build on a list of instructions and a rewrite on a tree. What runs today is numbers, strings, `true`, `null`, all of the operators with the conversions the language asks for in the order it asks for them, `var`, `let` and `const` with the *dead zone* that makes reading a `let` too early an error rather than `undefined`, blocks that shadow, objects and their properties, `a?.b`, backtick templates, every shape of control flow there is — `if`, three kinds of loop, `switch`, labels, `break` and `continue` — and **functions**: declaring one, calling one, `return`, arrow functions, `this`, and closures. A property can also be a **question rather than a thing** — `{ get total() { … } }` runs that line every time somebody reads `total`, and a `set` runs when somebody writes it, which is what frameworks keep two things in step with — and an object asked to be a string or a number is *asked*, so `"total: " + basket` calls the `toString` the page wrote for it. Five things about it are worth knowing. **What a script is holding lives where the memory collector can see it**: the engine's own working stack is in the same place a page's objects are, so a value half way through being computed is never quietly thrown away — and every one of the tests runs twice, the second time with the collector firing at *every* allocation, because that is the only way that kind of mistake shows up. **A closure keeps what it was written beside, and lets go of it when it is let go of**: the names a function declares live where a page's objects live rather than on a stack that disappears when the function returns, so `function counter() { var n = 0; return function () { return ++n; }; }` works the way every page expects — and a call that keeps nothing leaves nothing behind, which a test proves by counting a thousand calls' worth of objects back to zero rather than by watching how much memory the process is using. **Nesting costs no depth to run**: the thing that reads instructions never calls itself, and a *call* is a note on a list rather than the engine calling itself either — so a page cannot choose how much of this process's stack it uses by nesting brackets or by recursing for ever, and a recursion that will not end is the same error every other browser gives rather than a tab that vanishes. The thing that *compiles* does recurse, so it is given a stack of its own, measured against the deepest program the reader will accept. **A call that starts half way through something still costs no depth**: a getter, a setter and the method that turns an object into a string are calls nothing in the source spells, and each of them begins in the middle of an instruction — so the instruction hands its half-finished work over, the call joins the same list every other call is on, and the answer comes back to the place it was left. That is why a getter that reads itself for ever is the same catchable error a runaway function is, rather than the tab going away. And **the browser stops a script, not a timer**: there is no clock in the engine at all, and a page that will not finish is ended by the process that decided a tab has stopped answering. And **a loop gives every pass its own names**: `for (let i = 0; i < 10; i++)` making ten functions makes ten that answer with ten different numbers, rather than ten that all answer with the last one — the mistake every JavaScript programmer has met once, which the language fixed by giving each pass a copy and which this engine now does the same way. A function may read a name that a *block* around it declared, which is ordinary code that this browser refused until today. And **`new` makes things** (queue item 212): a function written with `function` comes with a `prototype` object, `new Basket(items)` makes an object that inherits from it, runs the function with that object as `this`, and answers with it — or with the object the function returned instead, if it returned one, which is a rule pages rely on. An arrow, a method and a builtin cannot be constructed and say so with the same error every browser gives, and that error comes only after the arguments have been worked out, because a page can see the order. What opened it was alo's own service worker, which did not get past its first `new Request(…)`. And **an array literal makes an array** (queue item 225): `[1, , 3]` is three long with nothing at all in the middle — not `undefined`, which a page can tell apart with `in` — writing past the end makes it longer, and setting `length` smaller throws the end away, as every browser does; a length that is not one, like `-1` or `1.5`, is the same `RangeError` everywhere else gives. That was the service worker's next stop, and it then compiled to the `try` on the line after it. And **`try`, `catch` and `finally` work** (queue item 210): a page's own `catch` catches what was thrown — from any depth of call, from inside a getter, or from the `toString` an error's message asked for — and an error the engine itself raises, like reading a property of `null`, is caught as a real `TypeError` with its message, which is how a page survives a browser that does something differently. A `finally` runs once whichever way its block was left: normally, by a throw, a `return`, a `break` or a `continue`, and two of them run innermost first. A recursion that catches its own "too deep" error and tries again can be stopped by the browser like any other script that will not finish. The service worker then compiled past its `try` to the `for…of` inside it. And **`for…of` works** (queue item 230): `for (const item of list)` visits every element in order — a hole as `undefined` — and an array that grows while it is being walked is walked to its new end. `let` and `const` give each pass a name of its own, so functions made in the loop remember different items, and `for (const x of x)` is the "used before it is declared" error rather than a read of some other `x`. The way it asks for each item is the way the language says, through calls a page can see and replace — which is how a library makes its own collections work in a `for…of` — and when a loop is left early, by `break`, `return` or an error, the thing being walked is told it will not be asked again, while a loop that simply finishes tells it nothing. `[].values()`, `keys()` and `entries()` exist for the same reason, and say `[object Array Iterator]` when asked what they are. With that, **alo's own service worker compiles from its first line to its last**; running it waits for `self`, the worker's global, which the browser supplies. What is not built is refused **by name**, each saying which piece of work builds it — classes, `super` and `new.target`, `instanceof`, spreading and taking a value apart (a `for…of` whose head takes an item apart included), `for…in` and `for await`, an array iterator reading an item through a getter, an object assigned to an array's `length`, `arguments`, default and rest parameters, and tagged templates — because a program that ran and was quietly wrong is worse than one that says what it could not do
- [2] A garbage collector, and the object model underneath it. **Both are built** (`alo-js`, queue items 71 and 206). What a script makes — objects, strings, functions — lives in one place the engine owns, and a reference to any of it is a **number naming a slot** rather than a machine address, which is why a mistake in this engine can be *seen* rather than being the kind of bug browsers get attacked through. Reclaiming is by **tracing**: start from what is definitely in use and keep what can be reached from it. Counting references instead would be simpler and would leak the commonest thing on the web — a button holding a function that mentions the button is a loop, and a count never reaches zero on a loop. **The page's own document is in the same graph**, which is the clause worth caring about: every browser that treated a page's objects and its elements as two separate things spent a decade finding leaks that only show up after a day of use, and here one thing decides what is alive. Three things go in before they are needed, because each is a rewrite if it arrives late: the hook that would let collection happen in slices if a person ever sees the browser pause, the loop that makes a `WeakMap` right rather than nearly right, and a collector that walks a page's objects **without recursing**, since a script can otherwise choose how deep it goes and take the tab down. Nothing that reclaims memory ever runs out of memory doing it, and nothing here is a stub. What that came to in practice: a chain of a million objects is tidied away with no risk of the walk running out of room, because the walk keeps its own list rather than calling itself; a list of things to look at that fills up costs the collector a second look rather than costing a page an object; a `WeakMap` with more live entries than that list holds keeps every one of them; and a page that asks for more memory than the browser will give one page is told so in a sentence, with the tab stopped, rather than taking the browser down with it. The tests count what survived rather than watching how much memory the process is using, because the second measures the machine. **And what a script makes is now a thing rather than whatever the caller put there**: an object is a prototype, a table of properties and one switch saying whether more may be added, and a property is a value with three switches of its own — whether it may be written, listed, or removed. The order a page gets when it asks an object for its property names is **the order every other browser gives**: the ones that look like whole numbers first and counting up, then the rest in the order they were added, then the invisible ones a page makes on purpose. That is not a detail to add later — it is invisible until the day a page enumerates and gets a different answer here than anywhere else, and by then everything is built on top of it. A name used twice is **stored once**, so ten thousand records each having a `title` hold one copy of the word; a name nothing uses any more is tidied away with everything else, which is what stops a page inventing names in a loop from growing the browser until it falls over. The way a page's own elements behave differently from ordinary objects — a list that answers to numbers, a property that refuses to be redefined — is **the same mechanism** the language itself uses for arrays and functions, rather than a second one beside it, which is what keeps a document and a script one thing that agrees with itself. Reading a property that a page computes with a function hands back the function rather than pretending to have called it, because calling one is the next piece of work and a browser that answered `undefined` instead would be quietly wrong. What a page cannot do, each with a test that has a number in it: build a chain of a hundred thousand objects and make the walk down it fall over, close that chain into a loop, or add and remove properties until the memory holding them grows without end
- [2] The ECMAScript standard library, in the order real pages need it. **The two objects the language is made of are built** (`alo-js`, queue item 218): the one every `{}` inherits from and the one every function inherits from. That is not a library, it is what an object *is* — until today `"" + {}` stopped the script, because an object had no `toString` to find, and that is the commonest thing a page does with a value it is not sure about. `toString`, `valueOf`, `hasOwnProperty`, `isPrototypeOf`, `propertyIsEnumerable` and `__proto__` all answer, and the last of them refuses to make a loop out of a prototype chain, which is what keeps a chain walkable. The mechanism underneath is the half that decides everything after it: **a builtin is a function whose body is ours rather than a script's, and it is called by exactly the machinery a page's own functions are called by** — so a page may replace one, a page's own method shadows one, and one is found where a `+` or a getter reaches for it rather than only where a page names it. A builtin can now suspend while the interpreter calls script or converts an object, then resume with the answer on the traced stack (queue item 219). `Function.prototype.call`, `Object.prototype.toLocaleString` and object arguments to property-name methods use that mechanism. Waiting builtins count toward the same recursion limit as script frames. Function source text remains refused by name (220); `apply` remains item 221. **A builtin can keep what it needs across the calls it asks for** (ADR 0031, queue item 332): it says with its function how many values it keeps — at most eight, and a builtin declaring more is refused when its function is made — and they live on the engine's own working stack above its arguments, where the collector already looks, so a list being walked can keep the length it read once and an array being built cannot be thrown away half way. Running out of stack for them is the same `RangeError` a deep recursion is. That is what `forEach`, `map`, `reduce` and `apply` each need. **`forEach` is built on them** (queue item 331): it reads a list's length once — through a getter, or by converting an object, if that is what the page gave it — skips the holes, calls the page's function for each element with the `this` it was given, and stops where that function throws; a walk over 2⁵³ − 1 holes, which calls nothing, still answers the browser's request to stop the page. An array is an array to the library too: `Object.prototype.toString` says `[object Array]`, and the object every array inherits from is itself an empty array with **no methods on it yet** — `[].push` is `undefined`, which is what a page's own check for it reads correctly, rather than a `push` that does half of what it should (queue item 225). **The errors a page catches exist** (queue item 227): `Error`, `TypeError`, `RangeError`, `ReferenceError`, `SyntaxError`, `EvalError` and `URIError` are on the global object, `new TypeError("bad")` has the message it was given and an optional `cause`, `"" + error` reads `"TypeError: bad"`, and calling one without `new` makes the same object as with it. They are the first built-in functions that can be used with `new`. An error the engine itself throws becomes one of these when a page's `catch` catches it (queue item 210); an error's `toString` whose message is a getter is refused by name until a builtin can hold a value across a call (items 228 and 221), and `AggregateError` waits for iteration (229). **An array can be walked** (queue item 230): `keys`, `values` and `entries` hand out an iterator, and the same `values` is what `for…of` asks an array for — under a name no page can spell yet, `Symbol.iterator`, which the engine makes along with `Symbol.toStringTag`, the name `Object.prototype.toString` now reads before it answers. Still absent, each named rather than stubbed: `Object` and `Function` themselves, which are constructors; the `Array` constructor and every array method but the three iterators and `forEach`, `JSON` and the wrapper objects; the `Symbol` function and the rest of the well-known symbols; the iterator helpers; and the weak collections. **`Date` is built, but not as text** (ADR 0036, queue items 353 and 356): the engine does a date's arithmetic and the renderer hands it the time — the machine's clock in whole milliseconds, the grain the language itself has, with nothing finer for a page to time a cache with — and a page's local time zone is UTC until the person chooses one, because where somebody is is theirs to tell. `Date.now()`, `new Date()` from nothing, a number, another date or its fields, `Date.UTC`, every getter and setter in local time and in UTC, `getTimezoneOffset` (zero), `toISOString` and `toJSON` all answer, to the last millisecond of the language's range either side and an Invalid Date one past it. An engine given no clock says so with a `TypeError` rather than inventing a time, and the reference corpus gives every page one stopped instant so that a page that writes the date draws the same pixels every day. `date + ''` asks for the date's text, as the language says — the engine now asks an object for its `Symbol.toPrimitive` before `valueOf` and `toString` — and **a date as text is refused by name** until a page needs it: `toString` and its family, `Date.parse`, `new Date(string)` and `Date()` (item 357; it will read the language's own forms and nothing older). The person's own zone is item 358, and Annex B's `getYear`, `setYear` and `toGMTString` are absent. **`encodeURIComponent` is built** (queue item 359), the first function on the global object rather than on a prototype: a string's characters are written as the bytes of their UTF-8, `%` and two uppercase hex digits each, except letters, digits and `-_.!~*'()`; a half of a surrogate pair on its own is the `URIError` the language gives, which a page can catch; and an encoding that would be longer than the longest string the engine makes is refused as it grows rather than after. `encodeURI`, `decodeURI` and `decodeURIComponent` are absent until a page needs one. **`Math` is built** (queue item 365): its eight values and thirty-four functions, each answering the language's values for a negative zero, `NaN`, the infinities and no arguments at all — `Math.round(-0.5)` is `-0`, `Math.max()` is `-Infinity`, and `Math.hypot` of an infinity and a `NaN` is `Infinity`. Each argument is converted once and in order, however many a page passes and even after one is `NaN`, which a page's own `valueOf` can see. `Math.random` is absent until it is decided where randomness comes from (item 367), and `Math.f16round` and `Math.sumPrecise` until what each is specified with exists (item 368). **A string has its properties** (queue item 488): `"abc".length` is `3`, `"abc"[1]` is `"b"`, and anything else is read from `String.prototype` with the string as `this` — a `String` object of the empty string, as the language makes it, holding `toString`, `valueOf` and `slice`. `slice` converts `this`, then `start`, then `end`, once each and in that order, which a page's own `toString` and `valueOf` can see, and counts a negative index from the end. A string is never written to: a write is silence in sloppy code and a `TypeError` in strict code, unless a setter along the chain takes it, and only what the string is made of refuses to be deleted. alo Sites' analytics script, which cuts `/f/` off a form's action with `slice(3)`, now runs to its end on every page. Still absent: `ToObject` of a primitive — the `String`, `Number` and `Boolean` functions and wrappers, a number's or a boolean's properties, a string as a sloppy function's `this` — which is item 489; iterating a string, refused by name rather than called not iterable (item 490); and every other string method, each until a page needs it
- [2] Regular expressions, with the syntax the language actually has. **The engine is built** (ADR 0029, queue item 74), and it is ours: a pattern is read whole — `u`, `v`, named groups, lookbehind, the `(?ms-ms:…)` modifiers — and one that is not a pattern stops the script before any of it runs, saying what is wrong. A match is run by a loop that counts its work, so a pattern written to run for ever is a `RangeError` the page can catch rather than a tab that hangs, and the browser can stop one mid-match. A literal makes a `RegExp` with `exec`, `test` and a `lastIndex` that `g` and `y` move. alo's download page is the first frozen page whose script reaches a regular expression, and its four patterns now tell a Mac from Windows. Still owed, each refused by name rather than approximated: ignoring case and `\p{…}` (322, with the rented Unicode tables), the string methods that take a pattern (323), and the `RegExp` constructor, `v`'s set operations and `d`'s indices (324). The legacy pattern forms of the specification's Annex B are refused by name until a page needs one
- [2] Promises, the microtask queue, `async`/`await`, generators and iterators. The microtask queue is built, as the engine's half of the event loop below (queue item 232). **Promises are built** (`alo-js`, queue item 333, ADR 0032 § 5): `new Promise` runs its function at once and a throw from it rejects the promise; `then`, `catch` and `finally` react to it, and `Promise.resolve` and `Promise.reject` make one. Each reaction waits in that same queue, so a chain of promises runs in exactly the order every browser runs it — a reaction to a promise that has already settled runs before a `queueMicrotask` queued after it, and an object with a `then` method of its own is adopted one step later than a plain value. A handler that throws rejects the next link rather than reaching the page, so a rejection travels down a chain to the `catch` at its end; `finally` runs its function whatever happened and passes the outcome on. A rejection nothing has handled by the time the queue is empty is reported once, as an error nothing caught is, and one handled in time is never reported. Absent rather than approximate: a promise made by a constructor other than `Promise` (a subclass) is refused by name (337); `Promise.all` and the other combinators, `async`/`await` and generators stay with item 75; and the page events that announce an unhandled rejection are not sent
- [2] Modules: ESM, dynamic `import()`, and the loader that fetches them
- [2] **The event loop** — tasks, microtasks, the rendering steps, `requestAnimationFrame`. **The engine's half is built** (`alo-js`, queue item 232, ADR 0016): the queue of small follow-up jobs — what `queueMicrotask` asks for, and what a promise's reactions will be — lives where the memory collector can see it, so a job waiting to run is never thrown away and a job nobody can run any more is. After a piece of work, the engine runs every waiting job, oldest first, **including the ones those jobs queue**, which is what lets a chain of promises finish before the page does anything else; a job that throws is reported and the next one runs, as every browser does; and a page the browser has stopped has its waiting jobs dropped rather than resumed in a state nothing on it expected. The order is the one the specification gives, down to the case libraries rely on: two click handlers a *person* triggered each see the other's jobs run between them, and the same two triggered by a script's `element.click()` do not. **And the loop itself is built** (`alo-renderer`, queue item 235): the work waiting for a page is kept in one queue and the oldest piece always runs next; after every piece of script — a script, or one handler of several the browser hands the same click to — the follow-up jobs it queued run before anything else does, so each handler a person triggered sees the one before it finish; `queueMicrotask` is on the page's global object; an error nothing caught is reported and the page carries on; anything else that ends a script — the browser stopping it, memory running out — stops the page, drops everything it was waiting to do and lets go of what that work was holding. Between two pieces of work the loop checks that nothing is still being held half way through, and stops the page rather than carry on if something is. **And a page's own scripts run** (queue item 236): a script written into the page runs as the page loads, in the order the page wrote them, each followed by the jobs it queued — but only where the page's own Content Security Policy allows it, whether that policy came in the response's headers or in a `<meta>` written before the script, and a script's nonce counts only when its markup is not the shape an injection leaves. A script that is fetched from elsewhere, a module, and a script the policy refused are each named in what the page load reports rather than skipped in silence; resizing the window does not run anything again; and each page starts with a fresh set of globals. A script that never finishes costs its own renderer, which the browser gives up on, rather than the browser. No script can see the page's document yet (80). Still to come: the loop running between the browser's messages, for work a page schedules for itself, and an agent's action answered only after the jobs it caused (233); drawing frames with `requestAnimationFrame` (234); and a page's fetched scripts (238)
- [2] Errors and stack traces good enough to debug somebody else's minified page. **An error nothing caught is reported by what it says** (`alo-renderer`, queue item 239): `TypeError: no such thing` rather than `an object`, by the error's own name — so a page that renamed its errors is reported by the new name — and its message. The words are read without running any of the page's code: a name or message only a page's function could produce is said to be one rather than asked for, and a page that replaced how errors turn themselves into text does not get to choose what its own failure says. A page's long strings are repeated only to their first 1024 characters, with the rest counted. **And where it happened** (`alo-js` and `alo-renderer`, queue item 241): the report names the script, line and column of the throw, then of each call it left on the way out — `(at script 2, line 3, column 5; called from script 1, line 1, column 9)` — so a function one script declared is found in that script when another calls it. Columns count UTF-16 code units, as every other engine and developer tool does, because in a minified page that is one line long the column is the only thing that finds anything; the renderer marks each script once as it keeps it so that finding a column never means re-reading a megabyte bundle from the start. A runaway recursion says its innermost thirty-two places and how many more it left. A rethrown error is placed where it was thrown again; `error.stack`, taken where an error is made and with function names in it, is still to come (78). **And there is a ceiling on how much is said** (`alo-renderer`, queue item 242): a load says at most 256 things about its scripts, then how many more there were, because everything a load says crosses to the browser in one message of bounded size and a page that throws in a loop would otherwise make its own report unsendable. The scripts all run either way, and past the ceiling a throw is counted rather than described, so a page that throws for ever costs a counter while it is being stopped rather than memory. **The same for its markup** (`alo-renderer`, queue item 243): what a page's markup, style sheets, pictures and fonts make the engine say is at most 256 lines a load, then how many more, and every line of a load's report — about markup or scripts — is at most 8192 characters, then how many more, because a line quoting what a page wrote quotes it escaped and a control character is five times its size there. Only the lines said are written out, so a page of millions of refusals costs a count; the page renders the same either way. **And the font names a load asks for** (queue item 244): a family longer than any font's can be is said rather than asked for, so nothing a load sends is longer than a font name except the bounded lines above
- [2] Internationalisation (`Intl`), rented rather than written
- [3] A JIT — refused until there is a measured reason and an ADR weighing it against the attack surface it adds

## The DOM as pages use it — stage 2

- [2] Mutation from script, and the invalidation that has to follow it. **The document's half is built** (`alo-dom`, queue item 245, ADR 0017 § 5): a page's tree can be changed — an element or text made, put in, moved, replaced or taken out — only in the ways the DOM standard allows, and a change it does not allow is refused by the standard's own name (`HierarchyRequestError`, `NotFoundError`, `InvalidCharacterError`) with nothing changed: no node inside itself, no second element or doctype under the document, no text directly under it, no template inside its own contents. A node made this way is numbered after every node the page already had, so an agent can name it like any other; every change is counted, so a renderer can tell what it drew is out of date; and a detached piece of the page nothing can reach any more can be let go, its numbers never handed out again. **The engine's half of keeping it is built too** (`alo-bindings`, queue item 248, ADR 0017 §§ 2–4): once a page runs script its document can be held in the page's own memory, counted at its real size so a page that builds without end is stopped like any other; the same element asked for twice is the same object, so what a page attaches to it stays; an element removed from the page is kept exactly as long as something still holds any part of the piece it is in, and freed at the next tidy-up when nothing does. **What a script calls is built too** (`alo-bindings`, queue item 249, ADR 0017 §§ 1, 4, 5, 8): a script given a document can find the page's root element and walk to any node's parent, first and last child and siblings; make an element or a piece of text; put it in, move it, swap it for another or take it out; read and set an element's attributes by the names a page writes them in; and read or replace a node's text. A change the standard forbids reaches the script as the error the standard names, which the script can catch; calling a member on something that is not the right kind of node, or handing it a node from another page, is a plain type error. Everything else a page might reach for is absent rather than half-done, so a page that checks before using a feature reads the truth. **And a page's own script reaches it** (`alo-renderer`, queue item 250, ADR 0017 §§ 2 and 6): when a page's first script that may run is about to, its document is handed to the page's script, and from then on the page is drawn from whatever the script left — the box tree, the layout, the picture and what an agent reads all show the element a script added, and an agent can name that element and act on it like any other. The page is drawn again only when something reads it — a frame, an agent reading or acting, a load ending, a window resized — so a script that changes its page ten thousand times in one go costs one drawing, and resizing a window lays out the page as it now is rather than as it arrived. A page none of whose scripts may run never has a script engine made for it. **And a script runs where it is written** (`alo-dom` and `alo-renderer`, queue item 247, ADR 0017 § 7): the page is read a piece at a time, stopping at the end of each script, so a script sees the page as far as it has been read — itself the last thing in it, nothing written after it there yet — and what it adds beside itself appears there, before the content written after it. Scripts still run in the order they are written, each under the policies stated before it; a policy a page states keeps governing the scripts after it even if a script removes the tag that stated it; and a script the page leaves unfinished at the end of its markup does not run, as every browser has it. **And a refusal a page lets escape is named** (`alo-renderer`, queue item 252): what a load reports for it is the standard's name and the rule that refused — `HierarchyRequestError: a document cannot hold text` — exactly what the page's own `catch` would have read, taken from the exception itself, so nothing the page did to it can change the words and reporting it runs none of the page's code. **And a script can find the page's body and swap it** (`alo-dom` and `alo-bindings`, queue item 253): `document.body` is the page's body — nothing before the page has one — and giving it a new body puts that one where the old one was, or adds it to the page when there was none; anything that is not a body is refused by the standard's name with nothing changed. Owed beyond this line: `document` as the standard's accessor (251)
- [2] **Events**: capture and bubble, listeners, default actions. **Events a page's own script sends are built** (`alo-js` and `alo-bindings`, queue item 254, ADR 0018 §§ 1–3 and 8): a script can listen on any node — for the trip down from the document to the target, or the trip back up — and send an event of its own (`Event`, or `CustomEvent` carrying whatever the page wants it to). Listeners hear it in the order the standard gives: down through each ancestor, at the target, then back up if it bubbles; each in the order it was added, a listener added or removed while the event is travelling counted as the standard counts it. A listener can stop the event going further, or stop even the listeners after it on the same node; one that asked to be called once is; one that promised not to cancel cannot; and the sender is told whether anyone cancelled. A listener that throws is reported, where it threw, and the next one still runs — the page's own `try` around the send does not see it. A listener goes when its node does, closure and all; a node the event is passing through is kept until it has passed. Absent rather than half-done: the time it was sent (92) and the legacy forms. **A page can listen on its window** (`alo-js` and `alo-bindings`, queue item 362, ADR 0037): the global object is a `Window` — `window`, `self`, `globalThis` and the top-level `this` are one object — that holds its own listeners by the same rules as a node, and every event sent to something in the page reaches it, first on the way down and last on the way back up, except `load`, which stops at the document. A script may send an event to the window itself. **A page is told whether it can be seen** (`alo-bindings`, `alo-renderer` and `alo-window`, queue item 364, ADR 0039 § 1): `document.visibilityState` says `"visible"` or `"hidden"` and `document.hidden` says the same as a yes or no, from the moment the page starts loading. A page is hidden while its window is covered or minimised, and visible otherwise — and a tab no window shows, which whoever drives it is reading, is visible. Each time that changes the page hears one `visibilitychange`, at the document and then the window; told the same thing twice it hears nothing. What its listener asks for — a fetch, say — is the page's own doing, not the person's. A document a page made beside its own is always hidden. Choosing among tabs, which hides the one left, is still to come (528). **A page is told when it is shown and when it is left** (`alo-bindings`, `alo-renderer` and `alo-window`, queue item 373, ADR 0039 §§ 2–5): the last thing a page hears as it loads is `pageshow`, at its window. A page being left — its tab closed, the window closed, or the next page loaded in its place — hears `pagehide`, then `visibilitychange` if it was visible, then `unload`, in one go, and is given at most a second: a page still running then is stopped, and the browser says so. Nothing it left waiting runs afterwards. What it said on the way out is reported as the left page's, never the next one's, and anything it asks to fetch on the way out is refused by name, and written in the record — except a report it sends with `navigator.sendBeacon`, which the browser makes after the page has gone, as the page's own (queue item 369, ADR 0040). `persisted` is always `false`: there is no back-forward cache, and no "leave this page?" prompt. Each of these events still names the window rather than the document as its `target` (374). Its prototype being fixed is still to come (363). **The browser's own dispatch is built too** (`alo-renderer`, queue item 255, ADR 0018 § 3): an event the browser makes is sent to a node as one task of the page's event loop, through the same rules, with whatever each listener left waiting run before the next listener starts — the difference from a script's own send that real pages rely on. A listener that throws is reported and the rest still hear it; a page told to stop mid-event stops; a page that never ran script is sent nothing and is not given a heap. **An agent's press arrives as a keyboard's click** (`alo-dom`, `alo-bindings` and `alo-renderer`, queue item 256, ADR 0018 §§ 4–7): on a page that runs script, an agent pressing something sends the page the one `click` a person pressing Enter on it would — trusted, a `PointerEvent` saying no pointer and no position — and the page's own listeners decide what it does. A checkbox or radio changes before the listeners run, so they read the new state, and goes back if one of them cancels; one nobody cancels then tells the page with `input` and `change`; a link is followed only if nobody cancelled. The agent is answered after all of it, with anything the page's script said. On such a page the agent no longer changes ARIA state itself — the page's listener does, and doing it twice would undo it; a page with no script is acted on as before. alo's Settings screen carries its nav script, and an agent pressing *Sharing* makes it the open row. **Every event says whether the browser sent it** (`alo-js` and `alo-bindings`, queue item 260, ADR 0019): `isTrusted` is `true` for an event the browser sent and `false` for one a script made or sent, and a page cannot change the answer — it is on each event itself, locked, rather than somewhere shared a page could replace. **A page's script can click** (`alo-bindings`, queue item 261, ADR 0018 § 6): every HTML element has `click()`, which sends the same click an agent's press does — except that it says a script sent it — through the same rules: a checkbox or radio changes before the listeners and goes back if one cancels, `input` and `change` follow, all before `click()` returns, and nothing a listener left waiting runs until the script that clicked is done. A disabled control ignores it, and a `click()` from inside the same element's own click does nothing. **A clicked link asks the browser to go there** (`alo-bindings` and `alo-renderer`, queue item 263, ADR 0020): a link a script clicks, or an agent presses, is followed by *asking* — the page's process works out the full address against the page's own (or its `<base>`) and says where the page wants to go in its answer, and the browser decides. It reads the address again itself, goes only to `http`, `https` and `about:blank` — a local file only from a local file — and refuses anything else by name, a `data:` page, a `javascript:` URL or another program's scheme among them; it records who caused it from what it had asked the page to do, so a link a page's listener clicks while an agent presses something is the agent's doing, with the page's click kept beside it. A task that clicks several links asks only for the last, and says how many it replaced; a link that opens a new window or asks for a download is not followed yet and says so (118, 264). Going there — loading the next page, history — is still to come (85), and each element's own interface — `HTMLInputElement` and the rest — is still to come (262). **An agent's typing reaches the page** (`alo-dom`, `alo-bindings` and `alo-renderer`, queue item 257, ADR 0018 § 5): on a page that runs script, an agent putting text into a field first asks the page with a trusted `beforeinput` — an `InputEvent` saying the field's text is being replaced and carrying the new text — and a page that cancels it keeps the field as it was, and the agent is told the page refused rather than that the text went in. Otherwise the text goes in and the page hears `input`, then `change`, so a page that echoes or checks what is typed does so for an agent as for a person; the agent is answered after all of it, with anything the page's script said. A page with no script takes the text as before. Still to come: focus (258) and `onclick` (259)
- [2] **Forms**: the controls, constraint validation, submission, file inputs. **How a form is submitted is decided, not built** (ADR 0051, queue item 82): pressing a form's submit button first checks its fields — a required field left empty, an email address without an `@` — and tells the page which failed; a page may then take the submission over itself, and otherwise the browser sends the form as Chrome sends it, deciding where it may go as it decides a link and refusing to send a secure page's form over plain `http`. A check this browser cannot make yet stops the form and says so, rather than letting it through. An agent submits a form by pressing its button, as a keyboard user does, and is told whether it went, which fields failed and why, or that the page took it. Only a person can choose a file to send, and its bytes never pass through the process that renders the page. **The renderer's half is built** (queue item 468): on a page that runs no script, an agent pressing a submit button has the form checked — a required field, a box, a radio group, a list or a file left empty, an email address or URL that is not one, each said in Chrome's own words — and is told `NotSubmitted` with every field that failed, or `Submitted` with how and where the form asked to go; the ask carries the entries Chrome sends, in its order, as names and values. A check not built yet (a `pattern`, a `minlength`, a `min` or `max`), a `dialog` form, another window's target, an image button or a field whose value is not read yet stops the form and says which. Text longer than a field's `maxlength` is refused rather than cut. **The browser process's half is built** (queue item 470): it writes what was asked as Chrome writes it — a `GET` form's entries replacing the query on its address, a `POST` form's as an `application/x-www-form-urlencoded` body with no charset, every line break as CR LF — and decides it as it decides a link, plus three checks: the page's own `form-action` policy, a secure page's form to plain `http` refused and said (sending anyway is the person's choice, and waits for a window to ask in), and a cookie marked `SameSite=Lax` kept off a form posted from another site. A form written as `multipart/form-data` or `text/plain` is refused by name until 474. Sending what was decided is loading a page, which is item 85's for every navigation. A `<meta>` policy's `form-action` is not applied yet (486). **A form on a page that runs script is sent by an agent's press** (queue item 485): the press is a click the page hears, and its task checks the form, firing a trusted `invalid` at each field that fails, or fires a trusted `submit` at the form, naming the pressed button as its `submitter`, which the page may cancel to send the form itself; only then are the fields read and the form asked for, so what the page's listeners changed is what is sent. A listener that presses the button again does not send it twice. A form this browser cannot check or send is refused before the page hears anything, and a page whose script has stopped sends its form as a page without script does. A script's own `click()` on a submit button is refused by name until 487. **A person's click on a submit button sends its form** (queue item 543): as an agent's press does, with the same checks, entries and decision, on pages with script or without, decided as the person's; Chrome 154's mouse click on the frozen booking and pizza forms sends what this sends. A person's click is never refused: a form this browser cannot check or send, one whose fields fail, or one the page took over is not sent, and says so in the click's answer, naming each field by its element and `name`. The window says a sent form as one — *this form is sent by GET to …* — rather than as a link. Telling the person in the window why a form was not sent is 557; an image button's point is 556. **A page can send its own form by `fetch`** (queue item 472): `new FormData(form, submitter)` holds the very entries a submission would send — Chrome's errors for a form that is not one or a button that is not the form's — and is walked as pairs by `for…of`; `new URLSearchParams(…)` reads a string, an object's own names, or any sequence of pairs, calling the page's own iterators, getters and `toString`s once each and in Chrome's order; and either one written out is what Chrome writes. alo Sites' contact form, which cancels the press to send itself, now posts exactly Chrome's body, and when its server answers puts *Thanks — talk soon.* where the form was, drawn where Chrome draws it. A form holding a file field, or with a `formdata` listener that would go unheard, is refused by name rather than read half way (491), and a `FormData` as a body is 492; `append`, `get` and the other list members wait for a page. **The agent reads which fields its form would refuse** (queue item 476): a field reads as `invalid` exactly when Chrome 154's accessibility tree says so — a required day, box, choice or list left empty, or an address that is not one, from the moment the page loads and whether or not anybody has pressed yet — while an empty text field is left alone until something is actually wrong with what is in it, as Chrome leaves it. An author's `aria-invalid` wins either way. A field this browser cannot check yet reads as `invalid=unknown` rather than a guess. Why a field is refused is said by the press, in Chrome's words. The rest is queue items 469, 471, 473–475, 486, 487, 491 and 492. **What an empty date field shows is built** (ADR 0042, queue item 385): how a date is written, `yyyy-mm-dd`, drawn in the field as its text in the field's own colour, because the order of a date's parts is the reader's — chosen in the browser's settings (387), never read from the machine or taken from the page's language — and until somebody chooses, the international order is the one nobody misreads and the one the field submits. An agent reads the field as a date, never mistakes the format for a value, and puts a date in as `yyyy-mm-dd`; anything else is refused by name. No calendar button is drawn until it opens a calendar (386). **A field's `placeholder` is built** (ADR 0043, queue item 389): alo's sign-in screen draws `you@company.eu` in its empty email field as a run of text in the field's line, in `::placeholder`'s colour — `#757575` unless the page says otherwise, `--text-tertiary` there — adding nothing to the field's size and hidden by any value, and an agent reads it as the field's `placeholder` property, never as its text or value, and as its name only when nothing else names it. It is the first pseudo-element this engine styles, and its style is computed as its field's child, which is how every later one will be; of what applies to it only `color` is read, and the rest is said as an issue (clipping a long hint, 390; `:placeholder-shown`, 391; its other properties, 392). A one-line field taller than its line holds its line in the middle, as browsers do **A `<button>` is no taller than what it holds** (queue item 437): its inside is a line tall only when it holds something a line is made of, so alo Agenda's arrow buttons, each a 15 pixel `<svg>` in 2 pixels of padding, are 19 × 19 as browsers draw them, while an empty field, `<textarea>` or `<input type=submit>` is still a line tall. An empty `<span>` and a kept space are its two measured edges (438).
- [2] **Navigation and session history**: `pushState`, back and forward, and what survives each. **Where a page is can be read** (queue item 360): `location` — on the window and on the document, one object — answers the page's address in every part a page asks for: the whole of it, its origin, scheme, host with and without its port, path, query and fragment, each exactly as the URL standard writes it, and always the address the page is at now rather than one copied when the page started. A page with no address is at `about:blank` and says so, with an origin of `null`, rather than inventing one. Its members are the page's own and cannot be replaced or deleted, so a script cannot make another script misread where it is. **Going somewhere through it is refused by name** — assigning to `location` or any part of it, `assign`, `replace` and `reload` each stop the script with a sentence saying it is not built — until navigation is (item 85, through ADR 0020's ask), because a navigation that silently did nothing is one a page would believe had happened
- [2] `iframe`s and the sandbox attribute
- [2] Shadow DOM and custom elements — component frameworks are not optional on the modern web
- [2] Selection and ranges
- [2] CSSOM — styles readable and writable from script. **Decided, partly built** (ADR 0033, queue item 339): an element's `style` attribute takes part in the cascade, above every rule a selector reaches and below `!important`, as CSS places it, on HTML and SVG elements, and a declaration in it that cannot be read is dropped and said (341, corpus case `style-attributes`). A script's `element.style`, on HTML and SVG elements, reads and writes that attribute and nothing else, so the two never disagree (342): `cssText` (and assigning to `style` itself), `length`, `item()`, `getPropertyValue`, `getPropertyPriority`, `setProperty`, `removeProperty`, and an accessor for each property under its camel-cased and dashed names; a write that changes nothing changes nothing in the document. It names only the properties this engine acts on, so a page that checks for one reads the truth, and a write to any other name is an ordinary property of the object, as in every engine: that list is built, in `alo-css`, each property on it naming the crates that act on it, and a test in each of those crates fails if it reads a property the list leaves out (344). What a script's edits do to the attribute is built beneath the binding too: a value set is kept when a style sheet would keep it, setting a shorthand removes its longhands, and the attribute is written back as the declarations that were written (344). A value is kept as the page wrote it, so a colour reads back in the page's own spelling. `alo-downloads` greys its unbuilt installers' buttons this way. A page's policy governs its inline style (ADR 0034, 343): a style a script wrote through `element.style` is exempt, recognised by its exact text, and any other `<style>` or `style` attribute is judged each time the page is drawn by every policy the page holds, its headers' and its `<meta>`s'; a refused one draws nothing and is said, and a refused `style` attribute reads as empty through `element.style`, so a script's first write cannot carry an injected style past the policy. Not yet: `el.style[0]` (345, `item(0)` answers it). **What a script reads of the page's size is decided and built** (ADR 0038, queue items 366 and 370): `innerWidth` and `innerHeight` are the size the page is laid out at, in whole CSS pixels, and change when the window does; `scrollX` and `scrollY` (and `pageXOffset` and `pageYOffset`, the same two by other names) are where the page is scrolled, which is the top, because nothing scrolls it yet. A page may replace any of them with its own value, as the standard allows. An element's `scrollWidth` and `scrollHeight` are how far its content reaches rightward and downward, never counting what spills out to the left or above, and the root element's is at least the window's size: each is measured from the page as it would be drawn at that moment, so a script that adds a row and asks reads the height with the row, and the drawing made to answer it is the one painted next (370). The screen, where the window sits on it, its outer size and the display's scale are not told to a page at all. A tab no window shows is to be laid out at the size a new window opens at, so a page cannot tell an agent's tab from a person's by its size; nothing opens such a tab yet (371)
- [2] Storage: `localStorage`, `sessionStorage`, IndexedDB, the Cache API, and one quota policy over all of them.
  Decided in ADR 0025. Built: the browser process's store, `alo-storage`.
  It keeps `localStorage` areas across a restart, evicts whole buckets and
  sets aside a bucket that fails its check. The profile's bound is a fifth
  of the store's volume's free space, asked of the operating system, and zero
  when the volume cannot be asked. No page reaches it yet. The
  decision: one bucket per origin and top-level site,
  kept by the browser process; one fixed 1 GiB quota per bucket, the same on
  every machine and in private browsing, with `localStorage` capped at 5 MiB;
  whole buckets evicted, least recently used first, never one in use;
  `sessionStorage` never written to a disk; a site's data cleared in one act
  (301–306)
- [2] Workers: dedicated, shared, and service workers with their fetch interception
- [2] Timers, clipboard, drag and drop
- [2] `IntersectionObserver` — what alo Sites asks before it shows a section that moves into view. Not built and not yet decided (queue item 381, needs ADR, after the rendering steps of 234); until then such a section stays at the opacity its sheet hides it at, as it would in any browser without the interface
- [2] `navigator`, and what the browser says it is. Decided in ADR 0030.
  Built: the `User-Agent` header, on every request over HTTP/1.1 and HTTP/2
  and in a CSP report's envelope (326), and `navigator` with every member
  of ADR 0030 § 5, answering what the browser process told the renderer
  (325). alo's download page now reads it, finds the visitor's system and
  marks that system's card as theirs, with `getElementById`, `hidden` and
  `classList` (327); the reference corpus renders as a Mac whatever
  machine runs it.
  The browser names itself, `alo/` and its version, and claims to
  be no other browser. Its string is the same on every machine running the
  same release on the same kind of system, with one frozen token per system
  (`Macintosh`, `Windows`, `Linux`) and nothing measured about the machine:
  no system version, no processor. The same string is sent as the
  `User-Agent` header and answered by `navigator.userAgent`, so a page and
  its server never disagree about which browser they are on, and it is the
  same whether a person or alo's agent is acting. `navigator.platform` says
  only the kind of system, which is what alo's download page reads to offer
  the visitor the right installer. `vendor` names no company (326, 325)
- [2] ★ **Permissions as capabilities** — camera, microphone, location, notifications, in the shape of `alo-os` ADR 0001: enumerated, visible, revocable, expiring, recorded. A browser is where most people meet a permission prompt, and every other one is a dialogue nobody can audit afterwards.
  Decided in ADR 0026. Built: the grant table, `alo-grants` (307). It
  decides an ask, keeps the answers and ends them, revokes at once, records
  without content, and keeps its file under ADR 0011 § 4, set aside whole when
  it does not read. No prompt, indicator or page API reaches it yet (308 and
  each capability's own item). The decision: a closed list of capabilities, each
  granted to one origin under one top-level site by a person answering an ask
  made with a gesture; *allow while this page is open* or *allow on this site*
  for thirty days from the person's last visit, and *don't allow* remembered
  the same way; one list, an indicator while a device is in use, revocation
  that stops the device at once, a record that holds no content; never
  answered, granted or revoked by an agent

## Pictures and media — stage 2

- [2] Image codecs, rented: PNG, JPEG, GIF, WebP, AVIF. **All but AVIF are
  built** (queue items 106, 177, 180), each behind one file and one size bound;
  AVIF is decided (ADR 0021) and waits on a `rav1d` release with a safe API (269)
- [2] **SVG** — a second rendering model inside the first, and far larger than one line suggests.
  Decided in ADR 0022. **The box is built** (item 270): an outermost `<svg>`
  is one replaced box sized from CSS, its attributes, its `viewBox` or
  300 × 150, with no boxes inside it, read by an agent as one image named by
  ARIA or its `<title>`. **Filled shapes are drawn** (item 271, crate
  `alo-svg`): `rect`, `circle`, `ellipse`, `polygon`, `polyline`, `<g>`, the
  `transform` attribute, `viewBox` and `preserveAspectRatio`, `fill`,
  `fill-rule`, `fill-opacity` and `opacity`, with presentation attributes in
  the cascade and `currentColor` from the text around the icon. **Path data
  is drawn** (item 272): `<path>`, every command, arcs as cubic curves, drawn
  up to its first error. **Strokes are drawn** (item 273): `stroke`, its
  width, caps, joins, miter limit, opacity and dashes, outlined by the rented
  stroker and squashed with their shape under a transform, so alo's offline
  screen draws its hand. **Relative sizes work** (item 279): `width` and
  `height` on an `<svg>` are presentation attributes, so `width="50%"` and
  `height="2em"` size its box through the cascade, and any stylesheet rule
  beats them. **The `transform` property works** (item 287): the attribute is
  its presentation attribute, so a stylesheet's `transform` replaces it, and
  `transform-box` (`view-box`, or `fill-box` on a shape) and
  `transform-origin` (`0 0` unless set) say what it turns about. A nested
  `<svg>` is 278, and `fill-box` on a `<g>` and `stroke-box` are 288.
  **An SVG file is a picture** (ADR 0027, for alo Meet's waving hand):
  recognised by its type, read by `quick-xml`
  behind `alo-dom`'s `xml.rs` into a document of its own that sees nothing
  of the page, runs nothing and fetches nothing, and refused whole on any
  XML error. **The file is read** (item 309): `alo_dom::read_svg` turns
  Meet's hand into `svg`, `title`, `g` and two `path`s, and refuses by name
  anything that is not UTF-8 XML 1.0, an internal DTD subset, an undeclared
  entity, a character XML forbids, a root that is not SVG's `svg`, an
  undeclared prefix, a repeated attribute, and anything past its bounds (a
  mebibyte, 65 536 elements, 256 deep, 256 attributes, 1 KiB names, 256 KiB
  values and text runs). **`<img src="…svg">` shows it** (item 310): only a
  resource whose type is `image/svg+xml` (or a file named `.svg`, with no
  response to say) is read as SVG, and never one whose bytes were handed to
  a raster decoder. Its natural size is its root's absolute `width` and
  `height`, its ratio theirs or its `viewBox`'s. It is drawn as vectors at
  the `<img>`'s size, clipped to the box, with a cascade of its own: the
  page's styles, custom properties and colour do not reach it, and its
  media queries see its own size. It runs no script and fetches nothing:
  an `<image>`, an external `<use>`, an `@import`, a `<link>` and a
  stylesheet instruction are each left out and recorded. The agent reads
  the `<img>` by its `alt` and nothing inside the file. Corpus case
  `alo-meet-greeting`. In `background-image` it is 311
- [2] Canvas 2D
- [2] Audio and video playback through rented decoders.
  Decided in ADR 0023 and not built: decoded in a sandboxed media process per
  site that holds none of the page; Vorbis, FLAC, PCM and MP3 through Symphonia
  first (289–292); Opus and VP8/VP9 wait for a decoder that can be rented in
  Rust, AV1 for the `rav1d` release AVIF waits on (293–295); AAC, H.264, H.265
  and DRM are never played, and the browser never says it plays what it cannot
- [2] Media Source Extensions, without which most video sites do not play at all
- [2] Web Audio
- [2] WebGL, then WebGPU — both large, both late, and neither before the software path is right

## Speed, where slow means unusable — stage 2

- [2] **Incremental style and layout** — recompute what changed, not the document. The largest single difference between an engine that renders a page and one somebody can use
- [2] Compositing layers, and scrolling that does not repaint the world
- [2] Off-main-thread scrolling and animation
- [2] A performance budget somebody can hold us to: named pages, measured, in CI

## The browser itself — stage 2

- [2] A window, tabs, and a tab strip.
  Decided in ADR 0024. **A window shows a page** (`alo-window`, queue item
  296): `alo PAGE.html STYLE.css` opens a window with one tab showing the
  page, drawn by its own locked-down renderer process; with nothing named it
  opens an empty tab. The window is drawn only from the last picture the tab
  sent, so a site that stops answering never freezes it: after a resize it
  shows the old picture at its old size until the new one arrives, and a tab
  whose renderer has stopped keeps its picture with a line along the bottom
  saying what happened, in the browser's own words. The page is laid out at
  the window's size in CSS pixels, for the scale of the screen the window
  is on (299, ADR 0057). A page laid out for a scale of two rounds every
  whole-pixel rule in device pixels and is where Chrome 154 puts it under
  a real scale of two, in 891 of the 893 corpus boxes compared (560, 563),
  and is drawn onto a canvas of device pixels, its letters outlined at the
  size they are shown (561). The window tells the page's renderer and the
  strip's its scale with its size, and places their frames one device
  pixel to each of theirs: nothing is doubled, so text on a high-density
  screen is as sharp as the screen (562). A window moved to a screen of
  another scale has its page and strip laid out and painted again there.
  A page is told the scale it is drawn at (567): `devicePixelRatio`
  answers it, and `resolution`, `min-resolution` and `max-resolution` in
  `dppx`, `x`, `dpi` and `dpcm` match as Chrome 154 matches them, so a page
  that draws a hairline or rounds a floating box for a dense screen is
  drawn as Chrome draws it there. `matchMedia` is not built (570). A
  fractional scale is rounded down to a whole one until one is measured,
  and a page's picture is at most 4,096 device pixels a side (569). Closing the
  window closes every tab and stops
  every renderer. `--frozen-fonts` draws with the reference corpus's fonts, so
  a frozen alo page in the window is the corpus's picture of it, pixel for
  pixel. The picture reaches the screen: started on a Mac and captured with
  the system's own screen capture, the window is the picture the browser
  composed, every pixel within one level once the display's colour matching
  is allowed for, except the corners macOS rounds. **The tab strip is a page
  the engine draws** (`alo-renderer`, queue item 297): the browser tells a
  renderer of its own, at an address no page can name and as locked down as
  any other, which tabs are open, in order, each one's title and address and
  whether it is selected, loading or stopped — as data, never as markup. That
  renderer makes the strip from a template it was built with and alo's own
  colours, and puts each title in as text, so a title written as `<b>`, as
  `</style>`, with a NUL in it or a right-to-left override is drawn and read
  exactly as written, and a very long one is cut to its tab with an ellipsis
  (and to its first 1,024 characters before that). An agent reads it as
  *"tab list 'Tabs', four tabs, the second selected"*, each by its title; a
  tab with no title shows its address, and a stopped one says "Stopped".
  **The strip is in the window** (queue item 528): a band across the top, the
  selected tab's page below it, laid out in what the strip leaves. Each tab is
  called by its page's own `<title>`, read as HTML reads it, and the strip is
  drawn again whenever a tab opens, closes, starts or finishes loading, stops,
  or the window is resized — a tab says it is loading while its page is being
  drawn. A strip whose renderer stops answering keeps its last picture and
  says so along the bottom, and every tab goes on working. Started on a Mac
  and captured, the window on screen is the strip and the page the browser
  composed. **The browser's own keys** (queue item 529): ⌘T opens an empty
  tab and selects it, ⌘W closes the tab in front, ⌃Tab and ⌃⇧Tab move to the
  next and previous tab, going round at the ends (Ctrl instead of ⌘ on other
  systems). Only exactly those keys count — ⌘⇧T or ⌘⇧W do nothing — and a key
  held down does its thing once. The window reads them before anything else
  could, and no key reaches a page yet. Choosing a tab tells the page left
  that it is hidden before telling the page chosen that it can be seen, and
  the window shows the chosen tab's last picture at once, then draws it again
  at the window's size; nothing of the tab left stays under it. Closing the
  last tab closes the window. **Where a pointer falls** (queue item 298):
  the page's own process can say which element any point of its picture is
  over, as Chrome 154 finds it — the box drawn last there, through what
  turns, moves or scales it, never outside what clips it and never in its
  own rounded corners. A point counts as the pixel that starts at it, so a
  box is found from just before its edge; a box nobody can see is still
  there; text is the element it is in, even one with no box of its own; and
  anywhere else in the window is the page's root. An `<svg>` is found as a
  whole, not shape by shape (540), and a list item over the whole of its
  marker rather than its bullet alone (541). **A person's click reaches the
  page** (queue item 535): pressing and letting go of the main mouse button
  over the page sends the page what Chrome 154 sends it — `pointerdown` and
  `mousedown` as the button goes down, `pointerup`, `mouseup` and `click` as
  it comes up — trusted, at the element under the pointer, each saying where
  the pointer was (a click and the mouse events in whole pixels, the pointer
  events exactly) and which button, and a page's listeners hear them in that
  order with whatever each left waiting run before the next. A page that
  cancels its `pointerdown` hears no `mousedown` or `mouseup`, and still its
  click. Let go somewhere else, the click goes to what both ends are inside;
  let go after what was pressed has been removed, there is no click. The
  click does what a click does — ticks a box, opens a `<details>`, follows a
  link — on a page with or without script, and a link a person clicks goes
  where an agent's press of it goes; the window says where, since going there
  is still to come (85). Where the window is on the screen is not told: a
  click's screen position is its position in the page. **The pointer moving
  over the page** (queue item 542): as the pointer goes from one element to
  another the page hears what Chrome 154 sends — `pointerout` and a
  `pointerleave` at each element it left, `pointerover` and a
  `pointerenter` at each it entered, then the same again as mouse events,
  each saying which element was on the other side — and then `pointermove`
  and `mousemove` where it is, all as one task. Arriving from outside it
  enters the document first; going off the page — onto the tab strip, over
  something the window is saying, out of the window — it leaves everything
  and moves nothing. A press or a release somewhere the page was not told
  the pointer was crosses there first. Dragging after a press on the page,
  every move is the page's, outside the window too, saying the button is
  held; a page that cancelled its `pointerdown` hears no `mousemove` until
  the release. An element removed from under the pointer is not told it
  was left, as in Chrome. Moves that arrive faster than a page answers are
  sent as the last of them. **`:hover`, drawn under the pointer** (queue
  item 550): the element the pointer is over and every element above it,
  `<html>` included, match `:hover` — in style, so the page is drawn again
  with its hover rules, and in a script's `querySelectorAll(':hover')` —
  from before the first event of the crossing to it, as in Chrome 154.
  Outside the page nothing is hovered; a drag hovers what it is over, and
  `<html>` outside the viewport; an element taken out of the document
  hovers the parent it left at once. On pages with and without script.
  **A `<label>` hovered hovers the control it labels** (queue item 554):
  the control alone, not its ancestors, decided when the label is hovered
  by HTML's labelled-control rule — `alo-dom`'s `label.rs`, which the
  agent's names ask too — and ended when the label or the control leaves
  the hovered chain, or the control leaves the document. Chrome 154 keeps
  such a control hovered after its label's `for` or the control's `id`
  changed, or the label was taken out; this engine does not, because that
  is a hover stuck on an element nothing is over. **A press on a tab in the
  strip chooses it** (queue item 539): the press of the main button, with
  no modifier held, goes to the strip's own renderer, which says which tab
  it was on — by its place in the strip, the only name it knows a tab by —
  anywhere on the tab, its title or the words in it, but not in its rounded
  top corners, the gaps between tabs or the strip beyond them. The browser
  chooses that tab only if it is one the strip showed when the person
  pressed and it is still open, and refuses anything else by name in the
  strip's line: a place it did not show, a tab closed since, or a claim no
  press asked for. A tab chosen by a press is chosen as a key chooses one,
  telling the page left before the page chosen; a press on the tab already
  chosen, or on no tab, does nothing, and the page hears nothing of a press
  on the strip. Started on a Mac and clicked with the system's own pointer,
  the window showed the tab clicked. Still to come:
  `pointerrawupdate` (551), what a page hears when it
  changes under a pointer that is not moving (552) and when another tab is
  chosen under it (553); the other buttons (544), a click with a modifier held (545) and
  double clicks (547); and focus moving to what was pressed (258). Still to come besides: an agent reading the open
  tabs from the strip, which first needs deciding what an agent kept to some
  sites may see of it (531); a title a page changes after it has loaded
  (532); a control in the strip to close a tab and one to open a tab
  (558); painting at the
  screen's density (299), which ADR 0057 decided means laying a page out
  for its screen's scale, every rule that rounds to whole pixels rounding
  to device pixels, whose layout and paint are built (560, 561), a
  renderer laying out a page at the scale it is told with it (563), and
  whose window is 562, with a box's own area on whole device pixels
  (565, built) and a line under text at a scale (566) still to measure; and a new tab opened by a link with
  `target="_blank"` that a person clicked (300)
- [2] The address bar: what somebody typed, what it means, and a search that phones nobody by default
- [2] History, bookmarks, downloads
- [2] Find in page, zoom, and per-site settings that stick
- [2] Context menus, and keyboard operation of every one of them
- [2] Printing, print preview, export to PDF
- [2] Viewing a PDF, or saying plainly that we hand it to something else.
  Decided in ADR 0028 and not built: this browser does not display PDFs, and
  says so. A link to a PDF leaves the page where it is and offers the file to
  be saved, with nothing written until the person chooses where; the saved
  file can be opened in the person's own PDF program when they ask, never
  automatically and never by a page or an agent. A PDF in a frame or an
  `<embed>` shows nothing of it, an `<object>` shows its fallback, and a
  script is told `navigator.pdfViewerEnabled` is false. The agent sees that a
  file was offered and cannot read inside it (317, 318)
- [2] Private browsing, and profiles that are genuinely separate
- [2] Autofill, and credentials held where the operating system holds secrets rather than in a file of ours
- [2] Security surfaces: certificate detail, permission state, what this page has stored — reachable, none of it buried
- [2] Settings
- [2] **Developer tools**: inspector, console, network, performance. A browser nobody can debug a site with is not one a developer keeps
- [2] **Accessibility on the shell**: keyboard operation of everything, focus always visible, and the EN 301 549 conformance the workspace is already held to

## The legacy tail — stage 3

Deliberately last, and possibly never finished. Refusing this list is what made
stages 1 and 2 survivable; a broken render schedules the work, not a
specification.

- [3] Quirks mode
- [3] **Floats as layout**, and CSS table layout — the two that most often turn an old page into a column of rubble
- [3] `document.write`, live `HTMLCollection`s, and the DOM as it was before it was a specification
- [3] Legacy character encodings, and detecting them
- [3] XML, XHTML and XSLT
- [3] `frameset`
- [3] Vendor prefixes, and anything that exists only for a page written before 2015
- [3] The sloppy-mode corners of JavaScript that only old code reaches

## A browser somebody chooses — stage 4

**[4]** is a tier this file did not have. It was added when the roadmap gained a
fourth stage: product work, gated behind stage 2's exit gate, listed rather than
left unnamed because "not scheduled" had begun doing the work of "not thought
about".

- [4] Extensions — and the decision, in an ADR, whether that means the WebExtensions API or something narrower we can actually secure
- [4] Sync, self-hosted: bookmarks, history, tabs and passwords, end-to-end encrypted, on the customer's own server
- [4] Updates that are signed, staged and reversible
- [4] A mobile port
- [4] Crash handling that helps us fix it without becoming telemetry
- [4] ★ **Translation on the machine** — alo already runs models locally; a page translated without sending it anywhere is the sovereign version of a feature every other browser sends to a server
- [4] ★ **Reading and summarising a page locally**, under the same grants and the same record
- [4] Enterprise: policy, managed configuration, and an update mirror an organisation hosts

## Non-goals

**No text shaper, font rasteriser, codec or TLS stack of our own** — we rent the
physics, as Chromium, Firefox, Servo and Ladybird all do, and not out of
timidity. **No fork** of Chromium or Ladybird. **No `unsafe`** outside a
reviewed, named boundary with an ADR. **No conformance-percentage target** — the
measure is whether alo renders correctly, because a Web Platform Tests score
grades us against the legacy we are deliberately refusing. **No plugin-shaped
agent** bolted on afterwards — that is what every other AI browser already is,
and ADR 0002 exists to prevent it.

And four that no later stage may quietly adopt:

- **No DRM, and no Encrypted Media Extensions.** A proprietary binary with privileges inside a sovereignty product is a contradiction. Sites requiring it will not play, and we say so rather than shipping a black box.
- **No proprietary codecs** we cannot ship freely.
- **No telemetry.** Not "anonymised telemetry". None — the rule alo OS already holds.
- **No search deal.** The address bar's default is decided for the person using it, not sold.

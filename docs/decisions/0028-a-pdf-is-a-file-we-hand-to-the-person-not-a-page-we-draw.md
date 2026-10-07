# ADR 0028 — A PDF is a file we hand to the person, not a page we draw

**Status:** accepted
**Date:** 2026-10-07
**Context:** queue item 124, *"Viewing a PDF — or saying plainly that we hand
it to something else"*, marked **needs ADR**: *"it is a decision, not an
omission"*. It also uses:

- **What alo does with its own PDFs.** `alo-workplace` (read on 2026-10-07 at
  `738de614`, never written) makes PDFs in exactly one way: its server
  renders invoices, quotes and purchase orders, and serves every one with
  `Content-Disposition: attachment`, `nosniff` and `no-store`. Its
  `billing_pdf.rs` gives the reason in its own words: *"A PDF rendered
  inline is rendered by a viewer inside our own origin, which is a document
  context we neither wrote nor control; this file exists to be saved, mailed
  and archived."* Its share links (`share.rs`) do the same for every file.
  Its web client fetches the PDF with the session's token and saves it
  through `billing/saveFile.ts`: a `blob:` URL, `a.download`, `a.click()`.
  **No alo screen displays a PDF.** It has no `<iframe>`, `<embed>` or
  `<object>` showing one, and no link that opens one in a tab.
- **HTML.** *Load a document* creates a viewer document for the PDF types,
  `application/pdf` and `text/pdf`, only if the user agent's *PDF viewer
  supported* is true. When it is false, the response is not made into a
  document. HTML also makes `navigator.pdfViewerEnabled` say which, and
  `navigator.plugins` and `navigator.mimeTypes` empty when it is false. A
  response marked as an attachment, or of a type the user agent does not
  display, is *handled as a download*.
- ADR 0001 (rent the physics, build the engine) and its four laws, law 2
  above all: the agent reads the tree, never a picture.
- ADR 0005 (strangers' bytes are read in the least privileged process that
  can do the work) and ADR 0010 (a rented crate's `unsafe` is the crate's;
  FFI we would write is ours).
- ADR 0011 (what may be written to a disk), ADR 0012 (every request says
  what caused it), ADR 0020 (a page asks to go somewhere and the browser
  process decides; § 6, a download is a different ask, cut to item 264),
  ADR 0021 and ADR 0023 (how a decoder is rented: Rust, safe by its own
  statement, a licence beside MPL-2.0, and reach and age), ADR 0026 (only a
  person answers a prompt, and no agent verb reaches one).
- `ROADMAP.md`'s stage 2 exit gate: *a person uses it as their browser for a
  week and reaches for another one only for a site they can name.*

## The decision in one line

**Stage 2 has no PDF viewer.** `PDF viewer supported` is false. A response
typed as a PDF is **never given to a renderer and never parsed by any process
of ours**: the browser process offers it to the person as a file, says in
its own words that it does not display PDFs, and writes nothing until the
person chooses where. Opening the saved file in another program is a
**person's** act in the browser's own interface, never a page's and never an
agent's. A viewer is reopened by a person's week naming PDFs as the reason
they reached for another browser, and then only on the terms in § 7.

## Why this is a decision rather than an omission

Every engine people compare ours with displays a PDF. Not doing so is the
single most visible thing this browser will not do on an ordinary day, and
it should be chosen with the reasons in front of whoever reads it, not
discovered by a person clicking a link.

Three things make it a decision:

- **A PDF viewer is a second rendering engine.** PDF has its own page
  description language, its own fonts (Type 1, TrueType, CFF, Type 3 and CID
  fonts with their own encodings and character maps), its own colour spaces,
  its own image codecs (JPEG 2000, JBIG2, CCITT fax, and the rest), its own
  forms, annotations and links, and its own JavaScript. None of it is shared
  with HTML and CSS. Building or renting all of it is a body of work the size
  of a stage, and law 3 does not let us do it shallowly.
- **It is one of the worst attack surfaces a browser carries.** PDF readers
  have been among the most attacked document software for as long as there
  have been PDF readers. ADR 0005 put media in a process of its own for the
  same reason, and a viewer would need the same.
- **It is pixels unless we make it otherwise.** A PDF is a page of positioned
  glyphs, not a tree. A viewer that only rasterises gives the agent a
  picture to interpret, which is what law 2 exists to refuse. A viewer that
  honours law 2 has to read the text out of the page description into a tree,
  in reading order, which no rentable crate does for us.

And the measure is alo. alo has chosen, in code and with its reason written
beside it, never to have a PDF displayed inside a browser. A viewer here
would serve no alo screen.

## The survey, taken 2026-10-07

| Candidate | What it is | Verdict |
|---|---|---|
| **`hayro` 0.8.0** (4 October 2026; first release 22 July 2025) | A pure-Rust PDF interpreter and rasteriser, Apache-2.0 OR MIT, with JPEG 2000, JBIG2 and CCITT decoders of its own in the same workspace; about three million downloads | **Not now** (§ 7). Its own README calls it *"an experimental, work-in-progress PDF interpreter and renderer"*, says nothing about untrusted input or `unsafe`, and reads no text into a tree. It is the candidate § 7 measures first |
| pdf.js (Apache-2.0) | Firefox's viewer, written in JavaScript | **Not now.** It needs a JavaScript engine that runs a large modern program, Canvas 2D (108), workers (91) and typed arrays. That is years away, and running a stranger's PDF through our young engine moves the attack surface rather than shrinking it |
| PDFium | Chromium's viewer, C++ | **Refused.** C++ in the process that reads a stranger's bytes, as ADR 0021 refused dav1d |
| MuPDF, Poppler | C under AGPL, and C++ under GPL | **Refused**, on language and on licence |
| `lopdf`, `pdf` and other parsers | Read a PDF's objects and draw nothing | **Not a viewer.** A parser is the smallest part of the work |
| The operating system's viewer embedded (PDFKit and the like) | What a native application would call | **Refused.** FFI we would write is `unsafe` of ours, its output differs by platform, and it would put the platform's PDF engine inside our process |
| The operating system's viewer, as a separate program | Preview, Evince, or whatever the person has chosen | **Taken, as a hand-off a person makes** (§ 3) |

## 1. What a PDF is here: its type

A response is a PDF when the **essence of its `Content-Type`** is
`application/pdf` or `text/pdf`, compared ignoring case: HTML's PDF types.
**Nothing is sniffed.** A response with no type, or typed
`application/octet-stream`, is already a type this browser does not display,
and goes the same way without anyone having to look inside it. A PDF served
as `text/html` is read as HTML, because that is what its server said it is,
and `nosniff` means nothing else when nothing sniffs.

A response marked `Content-Disposition: attachment` is a download whatever
its type, by HTML's rule. That is how alo's own PDFs arrive, and it is the
same path as this one. This decision does not add a second.

## 2. A navigation to a PDF is offered as a file

When the browser process has a navigation's response (item 85) and it is a
PDF:

- **No renderer receives a byte of it.** The body is not parsed anywhere:
  not in the browser process (ADR 0005's first rule), not in a renderer, and
  not in a utility process, because there is no viewer to start.
- **The tab keeps its page.** The page that was there stays, with its node
  ids and its script, as it does when a link leads to an attachment in every
  browser. A navigation typed into the address bar to a PDF leaves the tab
  as it was, and an empty tab stays empty.
- **The person is told, in the browser's words**, never in words the
  response chose: that the file is a PDF, that this browser does not display
  PDFs, and that it can be saved and opened in another program. The file's
  name is the cleaned name ADR 0020 § 6 describes, shown as text. Where this
  is shown is the downloads interface's (item 120).
- **Nothing is written until the person chooses where**, by ADR 0020 § 6 and
  ADR 0011. The body is not put in the cache or in a scratch file while the
  question is open. It is held in memory, bounded by the download's own
  bound, or not fetched past its headers until the person says yes. Which of
  the two is item 120's choice. A response the person declines is dropped
  whole, and a `no-store` response is not kept once the choice is made.
- **It is recorded**, by ADR 0012: the request with its cause, and a line
  saying it was offered as a file because it is a PDF. A page is told
  nothing, as with any navigation the browser decided against (ADR 0020 § 3).

## 3. Opening it elsewhere is a person's act

The saved file can be opened in the program the operating system uses for
PDFs, from the browser's own downloads interface, and **only when a person
asks for that file**:

- **Never automatically.** There is no *always open files of this type*. A
  file a stranger chose, opened by another program without anyone asking, is
  how a document becomes an attack on a different program. Every open is a
  person's click.
- **Never from a page.** A page cannot ask for it. ADR 0020 § 3 refuses a
  page's scheme hand-offs, and ADR 0026's *opening another program* is a
  capability for a page's `mailto:`-style ask. It does not cover this, and it
  does not reach a file.
- **Never from an agent.** No agent verb opens a file in another program.
  The other program is outside the record ADR 0012 and item 133 keep, so an
  agent opening it would act where nothing records what it read or did.
- **Through the operating system's own way of opening a file** with its
  chosen program, started as a separate process the browser does not wait
  on. This needs no `unsafe` of ours, and the building item says which call
  it uses.

## 4. Inside a page: frames, `<embed>` and `<object>`

None of these displays a PDF, and none starts a download nobody asked for:

- **A frame** whose navigation reaches a PDF shows the browser's sentence
  from § 2 in the frame's box, and offers nothing. A frame is the page's, and
  a page that could make the browser offer a file from inside a frame could
  dress one site's file as another's. A person who wants the file follows
  the link. This is built with frames (item 86).
- **`<object>`** with a PDF shows its **fallback content**, the markup
  between its tags, which is what HTML says an `<object>` does with a type
  the user agent cannot display. Pages that embed a PDF usually write a
  download link there.
- **`<embed>`** with a PDF shows nothing: an empty box at its size.
- **Each is recorded**, so a page that shows nothing where another browser
  shows a document says why.

`<object>` and `<embed>` are not built. This is what they do with a PDF when
they are, opened by a page that uses one.

## 5. What a script is told

The truth, by HTML's rule for a user agent without a viewer:
**`navigator.pdfViewerEnabled` is `false`**, and **`navigator.plugins` and
`navigator.mimeTypes` are empty.** A page that checks before embedding a PDF
writes a download link instead, which is the behaviour this decision wants.
ADR 0023 § 6 said the same for media: nothing claims what it does not do.

## 6. What the agent reads

**The agent reads the page**, which has not changed, and **the offered file
as the browser's own state**: a file offered, its cleaned name, its type and
the site it came from. That is browser state, like the tab list (ADR 0024),
and never the page's content. **The agent cannot read inside the PDF.**
Whether an agent may answer the save question for a person, and under what
grant, is item 133's to decide, with ADR 0026's rule that no agent answers a
person's prompt as the starting point.

## 7. When a viewer is reopened, and on what terms

**What reopens it** is the stage 2 exit gate itself. If the person's week
names PDFs as the reason they reached for another browser, that is the
evidence. A conformance list, or a page that merely links a PDF, is not.

**What a viewer must be**, written now so that the later decision starts from
it rather than from convenience:

- **Rented, and in Rust**, on the four tests ADRs 0021 and 0023 apply: Rust,
  safe by its own statement or with `unsafe` behind features we leave off, a
  licence beside MPL-2.0, and reach and age. `hayro` is the candidate to
  measure first. It passes the first and third today, the second is unstated,
  and its own README says it is experimental.
- **In a process of its own per site**, holding only the file's bytes,
  confined by the renderer's profile, as ADR 0023 § 1 does for media.
- **Static.** No PDF JavaScript, ever. No form submission, no launch action,
  no link that leaves the viewer except as a navigation ask the browser
  process decides (ADR 0020). No fetch of anything the file names.
- **Read by the agent as a tree**, by law 2: the text of each page in
  reading order, with its headings, links and form fields, as nodes the
  agent tree serves. A viewer that only draws pixels is refused, because it
  would make this browser the one place an agent has to read a screenshot.
- **Bounded and hostile-tested**, by `LOOP.md` stage 2 § 2: pages, objects,
  nesting, stream lengths and image sizes bounded before the work, and every
  malformed file a refusal, never a panic.

## 8. The cut

Item 124 records this decision and stays open until its cuts close.

- **317. A navigation to a PDF is offered as a file** (§§ 1–2). *Depends on
  85, for the browser process holding a navigation's response, and on 120,
  for where a file goes.* *Opened by* a frozen page whose link reaches a PDF
  served inline. alo's own PDFs are attachments, and reach the same path
  through 264. *Closes when:* a response typed `application/pdf` or
  `text/pdf`, in any case and with any parameters, reaches no renderer (a
  test that records every message a renderer is sent shows none carrying the
  bytes); the tab's page and its held ids are unchanged; the browser's
  sentence and the record line are produced; nothing is written to a disk
  before the person's choice and nothing after a refusal; and the same bytes
  typed `text/html` are loaded as a page, so it is the type that decides.
  Hostile names, types and lengths are refused, never a panic.
- **318. `navigator.pdfViewerEnabled`, `navigator.plugins` and
  `navigator.mimeTypes`** (§ 5). *Depends on a `Navigator` interface, which
  no item builds yet.* *Opened by* a page that reads one of them.
- **Opening a saved file in another program** (§ 3) is written into item
  120, the downloads interface, rather than cut: it is one button on that
  interface and means nothing before it exists.
- **Frames, `<object>` and `<embed>`** (§ 4) are built with item 86 and with
  those elements, each opened by a page.

## What this costs

- **A person who follows a link to a PDF gets a question, not a document.**
  This is the most likely *site they can name* in the stage 2 exit gate's
  week, and saying so here is the point of writing it down. If it is the
  reason somebody reaches for another browser, § 7 is how it is answered.
- **The agent cannot read a PDF.** A task that needs a PDF's contents
  cannot be done through this browser, and the agent is told it is a file,
  not shown nothing.
- **A site that puts a PDF in a frame looks empty where other browsers show
  a document**, with a sentence saying why.

## Alternatives rejected

- **Render PDFs with `hayro` now.** It is pure Rust and widely used, and its
  authors call it experimental. Rendering strangers' files with something its
  authors do not offer as finished is what ADR 0023 declined for Symphonia's
  video and for the young Opus decoders. It would also give the agent
  pixels.
- **pdf.js, on our own JavaScript engine.** It would turn a rented viewer
  into a test of our least mature component, and it waits on Canvas 2D,
  workers and the rest for years.
- **PDFium, MuPDF or Poppler.** C or C++ reading a stranger's bytes, and two
  of them under licences that do not sit beside MPL-2.0.
- **Open every PDF automatically in the operating system's viewer.** It
  looks like the convenient middle, and it is a stranger's file handed to
  another program without a person deciding. § 3 keeps the hand-off and puts
  the person in it.
- **Sniff `%PDF-` from the bytes.** Nothing gains from it. An untyped
  response is a download anyway, and a typed one has said what it is.
- **Say nothing, and let a PDF be whatever falls out.** That is the
  omission the queue item was written to refuse.

## What this does not decide

- **The downloads interface**, where the file goes, how its name is
  cleaned, and whether the body is held in memory or not fetched until the
  person says yes (item 120, ADR 0020 § 6).
- **Printing and export to PDF** (item 123), which writes a PDF rather than
  reading one, and is a different decision.
- **Whether an agent may answer the save question**, under which grant
  (item 133).
- **A viewer's design** beyond § 7's terms. That is its own ADR, when a
  person's week asks for it.

## How we will know if this was wrong

- **If a person's week names PDFs** as the reason they reached for another
  browser, § 7 is opened with that week's record beside it.
- **If a PDF's bytes ever reach a renderer or any parser of ours**, § 2
  failed at its boundary. That is a defect, and 317's test is what catches
  it.
- **If an alo screen starts displaying a PDF inline**, the measure has
  moved. alo's reason for refusing it would then have been reversed by
  alo, and that page reopens this decision.

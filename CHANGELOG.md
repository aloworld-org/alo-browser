# Changelog

What changed, in words a person outside this repository can read. Newest first.

---

## Unreleased

- **Decided: where a script's objects live, and what tidies them away.** No code
  this time. A program makes things — objects, text, functions — and something
  has to notice when nothing needs them any more. How that is done is the kind
  of choice that cannot be revisited once a language is built on top of it, so
  it is written down first, argued in one document, and open to being argued
  with.

  What was decided. Everything a script makes lives in one place the browser
  owns, and anything referring to it holds **a number naming a slot** rather
  than a machine address — so a mistake made here is something the browser can
  notice and report, rather than the kind of bug browsers get broken into
  through. Deciding what is still needed is done by **starting from what is
  definitely in use and keeping whatever can be reached from it**. The obvious
  alternative — count how many things refer to each object and free it at zero —
  is simpler and would leak the most ordinary thing on the web: a button holding
  a function that mentions the button refers to itself in a circle, and a count
  never reaches zero on a circle.

  The clause that matters most is that **the page's own document is part of the
  same picture**. Every browser that treated a page's script objects and its
  elements as two separate problems spent a decade chasing leaks that only
  appear after a tab has been open for a day. Here one thing decides what is
  alive, so a button that refers to a function that refers back to the button is
  reclaimed when the page lets go of it, and nothing has to be untangled by
  hand.

  Three things go in before anything needs them, because each is a rewrite if it
  arrives late: the hook that would let the tidying happen in slices, should
  somebody ever notice the browser pause; the extra care that makes a `WeakMap`
  actually right rather than nearly right; and a design that walks a page's
  objects **without calling itself**, since a script can otherwise decide how
  deep the walk goes and take the tab down with it. And the tidying never needs
  spare memory to run, because the moment it is most needed is the moment there
  is none.

- **The engine understands a program now, not only its words.** The reader
  built last month turned a script's characters into its words and symbols;
  this turns those into the shape of the program — this is a function, that is
  its name, this call belongs to that condition. Still nothing runs; what
  exists now is the thing an interpreter will read.

  Two questions in JavaScript cannot be answered by looking at the characters,
  and both are answered here. The first is the `/` the reader already refused
  to guess at, and the answer is that whatever asks for the next word says
  which it expects — so `return /a/g` and `x++ /y/z` come out as the different
  programs they are. The second only a parser can settle: `(a, b)` and
  `(a, b) => c` are the same characters until after the closing bracket, so the
  second reading is **tried and put back** when what follows is not an arrow.
  Trying costs a second look at what is inside the brackets, which on a page
  that nests them a thousand deep would be a page choosing how long we spend —
  so a bracket that turned out not to begin a function is remembered and never
  tried twice.

  Where the language forbids two readings, so does this. `a ?? b || c` is
  refused rather than given a precedence, because either answer is one half of
  every reader getting it wrong; `-a ** b` likewise. `with` is refused **by
  name**, with a sentence saying why, rather than failing somewhere else three
  lines later. A keyword smuggled in with an escape — `if` — is neither the
  keyword nor a name, which is the rule that stops a check comparing text from
  being walked past. And a semicolon nobody wrote is inserted exactly where the
  language says one goes, so `return` alone on a line returns nothing rather
  than returning what is on the next.

  A stranger's script cannot make it stop. A file of twenty thousand open
  brackets is a stack overflow in any parser that does not count, and an
  overflow is not an error a browser can report — it is the process going away.
  So the parser counts, refuses past a depth it chose, and — the part that
  makes that a real promise — **runs on a stack of its own** rather than on
  whatever the caller happened to have, because a limit that depends on who
  called you is not a limit.

  It is judged on real code as well as on examples: alo's own offline worker,
  and now alo's own theme generator too, frozen into the corpus exactly as they
  are. The second one earns its place by holding six search patterns and not
  one division, so a `/` read the wrong way would be a different program that
  still parses — which the test refuses to let happen quietly. Reading it found
  one real bug in last month's work, in a place nobody would have looked: a
  space before the closing brace of `` `${ a }` `` was refused.

- **The first piece of our own JavaScript engine: it can read a script.** Not
  run one — nothing executes anything yet — but turn the characters somebody
  wrote into the words and symbols a program is made of, which is where every
  engine starts and where a surprising amount of what a program *means* is
  decided.

  The thing worth knowing about it is that **it never guesses**. A `/` in a
  script is either a division sign or the start of a search pattern, and
  nothing about the character says which: `a /b/ g` is three divisions or one
  pattern, and those are different programs. Every code editor in the world
  guesses, from whatever came before, and every one of them is sometimes wrong
  — which is why syntax highlighting occasionally turns half a file the colour
  of a string. Here nothing guesses: whatever asks for the next word says which
  of the two it is expecting, every single time, so the question is answered by
  the part of the engine that actually knows.

  It reads what people write this decade: numbers in four bases, with the
  underscores that make a million readable and the whole-number kind that has
  no ceiling; text as the browser really stores it, so a half-character from an
  emoji survives being read rather than being quietly repaired; strings with
  code slotted into them, keeping both what the author typed and what it means;
  names in any writing system Unicode has. Three ancient forms that mean
  something different from what they look like — a number written `0755` being
  the first — are **refused by name**, with a message saying what to write
  instead, rather than read as something the person did not intend.

  And it does not fall over. A script is a stranger's bytes, sent by whoever
  owns the page; a file cut off in the middle of a piece of text, a hundred
  thousand open brackets, a broken escape at the last byte — every one of them
  is an answer saying what is wrong and on which line, never a browser that
  stops. A tab that dies on a malformed script is a page that can take itself
  down deliberately.

  It is judged against a real script as well as against examples: alo's own
  offline worker, frozen into the test corpus exactly as it is served, so what
  the reader is measured on is code somebody wrote for a different purpose
  rather than code written to match it.

- **Decided: this browser writes its own JavaScript engine** (ADR 0013). It is
  the largest single component of a browser and the thing most of the rest of
  the web needs, so what it will be — and what it will not — is written down
  before any of it is built, in a form somebody can argue with.

  It will be a parser, a bytecode compiler and an interpreter, in Rust, and it
  will be **correct before it is fast**. There is no just-in-time compiler and
  there will not be one until two things happen: a measurement, on real
  hardware, showing that our interpreter is the reason somebody reached for a
  different browser; and a second decision weighing that against what such a
  compiler costs. What it costs is not abstract — a just-in-time compiler
  writes machine code and jumps to it, which is the one part of a browser that
  memory-safe Rust stops protecting, in the part of the browser most worth
  attacking.

  The interesting argument was not against Google's engine, which was refused
  at the start of this project. It was against a JavaScript engine already
  written in Rust, which exists, is freely licensed and would save years. It is
  refused because the piece that reclaims memory decides how the page itself is
  stored, and because the limits on what a stranger's script can make this
  browser allocate would then be somebody else's to choose. Those limits are
  the difference between a page that is refused and a browser that falls over.

  Three promises come with it. A feature we have not written yet will be
  **missing rather than faked**, because pages already check for what they
  need and a convincing stub is the one answer that fools the check and then
  behaves wrongly. The engine can reach **nothing** on its own — no network, no
  files, not even a clock — so a script gets only what the page it is in was
  given, and the part of the browser holding your history and your cookies
  never runs a stranger's code at all. And a script that will not stop is
  stopped by the browser, on your behalf, rather than by a timer the engine
  keeps to itself.

- **What an agent did is now kept until the person deletes it, and nothing else
  is.** The record added last time dies with the browser, which is right for a
  session and cannot answer the question an agent-driven browser exists to be
  able to answer: *what did it do while I was not watching?*

  So there is now a small second record on the disk, and what makes it
  affordable is what is **not** in it. Only requests whose chain leads back to
  something an agent did are written down. A person's own browsing never
  reaches it — not filtered out on the way to being shown, never written at all
  — because a file listing every address somebody visited, with times beside
  them, is the thing itself rather than a by-product of it, and it would have
  been built in the name of protecting the person it is about.

  **A private window leaves nothing behind.** Not a record emptied when the
  window closes: a record that was never opened. A file that was deleted is a
  file that was on the disk, recoverable, and present for the whole window
  between the two operations — which is exactly the window a crash lands in.

  Each entry keeps the whole chain that led to it, frozen at the moment it is
  written, because next week there is nothing left to walk it against. It keeps
  the names as plain numbers rather than as live identities, since this
  morning's first action and last week's first action are two different things
  with one name, and a record that ran them together would join two unrelated
  pieces of somebody's history into one story.

  It is bounded in **actions** rather than in bytes, so one busy action cannot
  push a week of ordinary ones out; the oldest goes whole; and everything it
  shortens, it says the size of. It lives where the operating system keeps
  things it is not entitled to delete rather than beside the cache — a record of
  what an agent did while nobody was watching, quietly removed on a Tuesday to
  make room, is the failure this exists to prevent. A `data:` address keeps its
  kind and loses its content, because an address that *is* the content is a body
  in disguise, and bodies are never recorded.

  What comes back off a disk is treated as a stranger's, the way a cache file
  already is: a checksum, a version, every length checked before anything is
  reserved. A file that does not read is a **gap the record counts** rather than
  an error that stops anything — and it is left where it is rather than deleted,
  because it is somebody's record and a later version of this browser may be
  able to read it. Deleting is real: the files go, not a flag beside them.

- **The browser now keeps a record of what it asked for, and what happened.**
  Until this, a cause travelled with a request and was gone the moment the
  request was — so the browser could say what had caused any single fetch and
  could not say what it had done all session.

  Every request the engine makes is now a line: when it was asked for, what
  caused it, the method, the whole URL, what kind of thing it was, and what
  became of it. Something the cache answered is a line that says it was the
  cache, and a hop this browser refused to follow is a line naming the rule that
  refused it — *what did this page try to load, and what stopped it* is most of
  why anybody would open such a thing. The lines are written where a request is
  actually made rather than by whoever remembered to write one, so the requests
  the browser makes for itself are in it too.

  **What is not in it is the part worth reading twice.** No request body, ever,
  in either direction, and no headers: a record holding `Cookie` and
  `Authorization` is a file that logs somebody into their own bank, and one
  holding a body holds whatever they typed into a form. That is what the record
  is built out of rather than a rule somebody keeps — a line is made in one
  place, from six things, and the headers and the body are in front of that code
  and are not read.

  It lives for the session and dies with the process, which is the right
  lifetime for a list of everywhere somebody went. It is bounded twice — in
  lines, and in bytes, because what a line costs is mostly the length of a URL
  and a URL is as long as a page chooses — and it says how many lines it has
  dropped, rather than quietly reading as a session in which less happened. A
  person can empty it, and emptying it is real.

  **Nobody but the person can read it.** Not a page: there is no interface to it
  and there is not going to be one, because a record a script could read is a
  history oracle handed out by the browser. Not the agent either: the record is
  *about* the agent and kept *for* the person. That is kept by the shape rather
  than by a check — the process that renders a page holds no part of it, nothing
  crossing the boundary between the two carries a line, and the agent surface
  does not depend on the loading code at all.

  Still owed, and it is the other half: what an agent did, kept until the person
  deletes it, on a disk, which a private window never writes to at all.

- **A page's fetch now leads back to the agent action that opened the page.**
  Before this, a request could say *this page wanted it* and nothing anywhere
  said what had caused **that page** — so *which agent action* was answerable
  only for the handful of requests an agent made itself, which is the narrowest
  possible reading of the promise.

  Loading a page now makes a **document**, and a document remembers what caused
  its load: the person who navigated, or the agent action whose verb led there.
  A request made by that page names the document, so walking back one link at a
  time reaches the action and then the person who asked for it. Both answers are
  kept in order rather than one being chosen: *which page* and *which agent
  action* are two questions with two true answers.

  Three things are worth saying about what this refuses to do. A page a person
  opened themselves reaches **no action at all**, which is what makes the other
  answer worth anything — a browser whose every request looked like the agent's
  doing could not answer the question the record exists for. An agent acting in
  one tab cannot reach into another, because the page a cause names is the one
  that tab is actually showing rather than one a caller supplied. And a walk
  that meets a page it has already been through **stops and says so** instead of
  going round: nothing can create that situation, and a browser process that
  hung because something eventually did would be the one failure this design is
  built to prevent.

  It is bounded, as a session record has to be, and it is honest about the
  bound: a chain that reaches a page too old to still be remembered says the
  piece is missing rather than reading like one that ended at a person.

- **Every request now says what caused it, and there is no way to make one that
  does not.** A request carries a cause — a person, a page, or something an
  agent did — and it is not a field somebody remembers to set: it is an argument
  to the only two ways of making a request, so code that cannot say what caused
  something does not build. That is deliberate friction, and it is the whole
  reason the browser will be able to answer *what did the agent do on my
  behalf* later without guessing.

  There are three causes and there is no fourth. No "unknown", no "internal" —
  a category like that does not stay empty, it becomes where the awkward cases
  go, and the one line in a record nobody can account for is the line somebody
  will eventually need. So the four requests the browser makes for itself, which
  nobody asked for, each name whatever caused the thing they are about: a
  redirect names whoever asked for the first hop, a download resuming after it
  was interrupted names whoever started it, the permission check before a
  cross-site request names the request it is asking about, and a report about a
  page's own security policy names the load that broke it.

  A server cannot rewrite any of this. It can answer a redirect and send a
  request somewhere else, and the cause that arrives is still the one the
  browser assigned — otherwise any site could write *the person did that* into
  somebody's own browser by redirecting a fetch its page had made.

  Nothing is written down yet: this is the cause travelling with the request.
  The record it will be kept in, and the chain that connects a page's fetch back
  to the agent action that opened the page, are next.

- **Decided: what this browser records about who caused each request** — and,
  more to the point, what it refuses to keep and who may never read it
  (ADR 0012). An agent that acts on somebody's behalf has to be answerable
  afterwards, which means the browser has to be able to say which page and which
  agent action caused every request it made. Nothing here does that yet; this is
  the decision the code will be built to, written before the code so that it can
  be argued with.

  The cause travels **with** the request rather than being pieced together from
  a log afterwards, because a reconstruction is a guess and a guess written down
  in the shape of a fact cannot be checked by anybody later. The part of the
  page that reads a stranger's bytes never gets to say who caused something —
  it says what it wants, a script or a picture, and the browser itself says who
  wanted it, so that a page which has taken over the code that renders it cannot
  write *the person did that* into somebody's record.

  And the record is kept small on purpose. Everything is held for the session
  and dies with it; the only part written down is what an **agent** did, kept
  until the person deletes it, because a record that vanishes when the browser
  closes cannot answer the one question it exists for. A private window never
  writes one at all. No page can read any of it — a browser that let a site read
  that list would be handing out the browsing history it partitions cookies to
  protect — and neither can the agent, because the record is about the agent and
  kept for the person. None of it is sent anywhere.

- **A local file and a `data:` page each get a process of their own.** One
  process per site is what keeps one page's data out of another page's reach,
  and the question underneath it is what a *site* is. For an ordinary address
  the answer has been right for a while: the scheme and the registrable domain,
  so two tabs on one organisation's pages share a process and two organisations
  never do. For a page with no address at all — a local file, a `data:` page,
  anything on a scheme this browser has not been told about — the answer was the
  scheme and nothing else, so **every local file on the machine was one site**
  and shared one process, and so was every `data:` page.

  Those pages are not one another. The browser has said so since the day it
  learned about origins: each of them is its own origin, the same as itself and
  nothing else, and one local file being able to read another is the oldest
  exfiltration bug there is. The process split now agrees with that rather than
  undoing it — each such page is rendered alone, and nothing else is ever put in
  with it.

  It costs processes: twenty local files open is twenty renderers rather than
  one, up to the ceiling of sixteen, past which the least recently used one is
  ended. That is the direction to be wrong in. The other way is two pages that
  can read nothing of each other's sharing one address space, which no amount of
  careful checking inside the process can undo — the hardware side channels these
  processes exist to defend against do not care what the checks say.

- **A page that wedges its renderer no longer freezes the whole browser.** One
  process per site means a page that crashes costs one tab, and that has been
  true here for a while. A page that leaves its renderer *alive and silent* —
  stuck in a loop, or stuck on something a hostile page arranged — was a
  different story: nothing bounded how long an answer might take, so the
  browser process waited in a read for as long as the renderer lived. Every
  other tab, and everything a person could click, waited with it. The one
  failure the process split exists to prevent, arriving by the one road nobody
  had walked down.

  A renderer that says nothing for ten seconds is now given up on and stopped,
  and its tab says so in the same words as a tab whose renderer crashed —
  keeping the picture it was showing, taking no other tab with it, and coming
  back only when somebody asks for the page again. From a person's side those
  are the same event, so they read the same.

  A renderer that is merely **slow** is still waited for: the bound is on
  silence, not on being quick. Ten seconds is a choice rather than a
  measurement, and the honest version of it is a question — *this page is not
  responding; wait, or stop it?* — which is what this will become when there is
  an interface to ask it in.

- **Closing the last tab on a site ends that site's process.** One process per
  site is what keeps one page's failure away from every other page, and it is
  paid for in memory: N sites cost N processes. Nothing was ending them.
  A renderer whose last tab had closed kept running — holding a page nobody was
  looking at any more — until sixteen sites were open and the ceiling happened
  to evict the least recently used one. Evicting under a bound is what happens
  when nothing has been reaped, not a way of reaping.

  Closing a tab now gives up its site if it was the last tab on it, and the
  process behind it stops. Closing one of two tabs on a site stops nothing: the
  other tab is still showing a page out of that renderer. A tab whose renderer
  already died has nothing to stop, and closing it is not a way to start one.

- **A tab whose renderer dies keeps the page on the screen and says what
  happened.** The process split has always meant that one site crashing cannot
  take another down, and there was a test killing a renderer to prove it — but
  nothing in this browser was a *tab*. Nobody kept the picture a renderer had
  painted, so what a person would have been shown when one died is the blank
  rectangle the design exists to avoid, and the sentence explaining it was a
  value returned to a caller rather than anything on a screen.

  A tab is a thing now. It holds the last frame it painted and keeps it when
  its renderer goes; it says, in words naming the site, what happened; and one
  process dying marks **every** tab that was in it and no tab anywhere else,
  which is the claim the whole process model is for.

  Nothing brings a dead tab back by itself. Painting or reading one answers
  from what the tab already knows rather than starting a fresh process — which
  would have found it holding no page, reported that nothing was loaded, and
  made the crash disappear from view. Loading the page again is a person
  asking, and that is the only thing that starts another renderer.

- **A font that holds many weights is drawn at the one a page asked for.** A
  variable font is a single file covering a range — most of what macOS ships is
  one, its system font included — and this engine read only the single weight
  such a file states in its `OS/2` table. So `SFCompact.ttf` was filed as the
  blackest face CSS can name because it says 1000, and `SFNSMono.ttf` had no
  bold at all because it says 295.

  The `wght` axis is read now, as the range it covers. One file answers a
  request for 400 and a request for 700; the weight it is set to reaches the
  shaper, the measuring and the outlines together, so text is drawn in the face
  it was measured in. A page asking for something past either end of the axis
  gets the end rather than a refusal.

  One axis, deliberately: width, slant and optical size are separate CSS
  properties with grammars of their own, and a font is not narrower because this
  engine assumed an axis it had not looked at. And an axis that is not written
  in CSS's scale is left alone — this machine's `Skia.ttf` runs from 1 to 3,
  which predates `wght` having a shared meaning, and reading it as CSS numbers
  would draw every page of ordinary text in the blackest thing that file has.

- **A font is filed under the name a page would ask for it by, not the first
  name in its file.** A font states its family once per language: macOS's system
  font states it thirty-five times, in Catalan, Japanese, Russian and thirty-two
  more. This engine took whichever the file happened to list first, so four of
  the fonts on the machine it was built on were filed under names no stylesheet
  will ever write — `Songti.ttc` under `宋體-簡`, `STHeiti Light.ttc` under
  `黑體-繁`, `Hiragino Sans GB.ttc` under `冬青黑體簡體中文`. A page asking for
  Songti SC was drawn in something else and told so.

  A record's language decides now. A name written in **no language at all** is
  the font's own name and wins — that is how macOS itself reads its system font,
  which is why this engine still calls it `.SF NS` rather than `System Font`.
  Otherwise **English** wins, in any of the sixteen ids that spell it. A font
  that states no English name keeps its own, because a font in one language is
  still a font somebody has.

- **A font says how heavy it is and whether it leans; its filename no longer
  gets a vote.** Which face of a family a page is drawn in was decided by
  looking for the words `bold` and `italic` in the name of the file — so
  `Helvetica-Oblique` was upright, `InterDisplay-SemiBold` was ordinary weight,
  and every file named by any other convention was one of two possibilities.
  `DejaVuSans-Oblique.ttf`, which this repository has tested with since its
  first month, was among them.

  The `OS/2` table states both properly and the bytes were already in hand, so
  that is what is read. On this machine it is the difference between four
  weights and two: Apple ships its monospace face at 295 and its compact one at
  1000, and neither number is a word a filename could have carried.

  Two readings are decided rather than left to chance. A stated weight of zero
  is a font that **did not say**, not the lightest face CSS can name — so the
  bold bit is read instead, and a font that says neither is ordinary. And a font
  with no `OS/2` table at all is still a font: normal, upright, and kept, since
  a family of one unlabelled face is most of what is on a machine.

- **`sans-serif` now means a font this machine has.** Every page ever loaded
  asks for `system-ui, sans-serif` — it is in the user-agent style sheet, before
  anybody writes a line of CSS. Nothing had ever said what those are. The
  machinery to say it existed from the first month and only tests had called it,
  so a real page named two families nobody had and was drawn in whichever font
  happened to be first in the list.

  The browser process decides now, from the families it actually found while
  reading the machine's font directories, and hands the answer to each renderer
  along with the fonts themselves — a renderer is confined and may not look at
  the machine, so this is the only way it can be told. Each generic keeps every
  family this machine has for it, in preference order, so a character the first
  one lacks is still drawn by the second.

  `cursive` and `fantasy` are deliberately answered by nobody. There is no
  answer for them on any machine that is not a guess, and a guess here is a page
  drawn in a typeface nobody chose. A machine with none of the other four is in
  the same position and says so, in the words item 170 added: a page is told
  what it was drawn in instead, rather than left looking wrong for a reason
  nobody can see.

- **A font is called what it says it is called, in every encoding it may say it
  in.** A font file states its family in its own `name` table, once per platform
  that was ever expected to read it. Nearly every font carries a Windows record
  in Unicode; several of the ones macOS ships — Apple Braille among them — carry
  only the older Macintosh records, which are not Unicode. Those were unreadable
  here, so a page asking for such a family by name was told the machine did not
  have it, and drawn in something else, on a machine that had it.

  Mac OS Roman and Mac OS Cyrillic are read now. The other twenty Macintosh
  encodings are still not, and deliberately: Mac OS Japanese is close to Shift
  JIS and is not Shift JIS, and a family read wrongly is worse than one not read
  — it is a name a page can match by accident. A Unicode name still wins
  wherever a font has one, so no font that already had a readable name changed
  its answer.

  With that, **nothing anywhere takes a font's family from its filename**. The
  browser process used to, for the short list it hands each renderer at startup,
  on the argument that opening every font on the machine to ask would be slow —
  and it already reads every one of those files, so asking costs a table rather
  than an open.

- **A page that asks for a font by name is either given it or told it was not.**
  A renderer is confined and cannot open a font file, so it starts with whatever
  short list the browser process found and hands over. Until now a page asking
  for anything outside that list — `Inter`, `Helvetica Neue`, or the
  `system-ui` every document asks for by default — was quietly drawn in
  something else. The render was stable and diffable and was not what the page
  looks like in any other browser, which is the worst way for a rendering
  difference to be wrong: reproducible, and unexplained.

  A load now says which families it wanted and did not have. The browser
  process, which is the side allowed to look, searches the machine for each and
  sends over what it finds — **by the name each font gives itself**, not the one
  on its file, because "does this machine have Inter" decides whether a page is
  drawn as its author wrote it and an answer read off a filename is wrong for
  every font somebody else named. What is genuinely not on the machine comes
  back named, and the page reports in words what it was drawn in instead.

  A page whose *second* choice was found is not reported: it got the fallback
  its own author wrote, and there is nothing to tell anybody. The family is
  still asked for, because the machine may well have the first one.

- **A `style` attribute can be allowed by its digest, where the page asked for
  that in words.** A Content Security Policy that names a hash now reaches
  content with no element of its own — a `style` attribute today, an event
  handler when there are events — but only where the same directive also says
  `'unsafe-hashes'`. That is the specification's rule, and the reason for it is
  worth the extra keyword: a `<style>` element is something an author wrote
  once, and a `style` attribute is the shape an injection most often takes, so
  the permission is one to ask for rather than one to inherit from a hash
  written for something else.

  The keyword grants nothing on its own — it says that the hashes beside it may
  apply, so a policy with the keyword and no digest allows no attribute at all.
  It has to be in the **directive that decides**: one written into `default-src`
  while `style-src` decides is not this page allowing an attribute, and a second
  policy in a second header cannot add it to the first one's hash. And a refusal
  now says which of three things went wrong, the new one being *no digest allows
  this here*, which sends an author to a different place than *your digest does
  not match*.

- **A page can allow one exact stylesheet, and nothing else.** A Content
  Security Policy may permit inline content by naming its digest —
  `style-src 'sha256-…'` — and until now this engine read that sentence,
  understood it, and refused the content anyway, because nothing here computed
  a hash. It computes one now, so a policy written the way the specification
  recommends works: inline content the page shipped applies, and inline content
  an injection added does not, with no `'unsafe-inline'` anywhere.

  A hash is the stronger of the two ways past `'unsafe-inline'` for content
  that does not change, because it needs nothing minted per response — a site
  serving the same HTML from a cache to a million people cannot use a nonce and
  can use this. Both alphabets a digest may be written in are read, since a
  digest is usually copied out of a browser's console and browsers print both.

  **The reading of a digest is strict, in the direction of allowing less.** A
  hash source is a permission, so every laxness would be a policy quietly wider
  than its author wrote: a value mixing the two alphabets is refused, so is one
  whose spare bits hold anything, and so is one of the wrong length for the
  algorithm it names. And what will *not* be hashed is content with no element
  of its own — a `style` attribute, an event handler — because matching those
  by hash is what `'unsafe-hashes'` is for and this engine reads that keyword
  without acting on it. A refusal says which of the three it is, so an author
  whose page stopped working is not left comparing digests by eye.

- **A policy that was violated says so.** Content Security Policy has always
  been two halves: the sentence a site writes, and the reports that tell its
  author what the sentence would stop. Until now this engine had only the
  first, which made `Content-Security-Policy-Report-Only` a header with no
  effect at all — and that header is how a large site actually turns a policy
  on. You send it for a week, read what it *would* have blocked, find the four
  scripts your own marketing department added through a tag manager, and only
  then enforce. So both dispositions report now: an enforced policy and a
  watched one alike, through `report-uri` and through `report-to`, in the two
  documents collectors read.

  **A report says a cross-origin URL as its origin and nothing more.** That is
  the rule the whole feature is shaped around, because a report is posted to a
  server the *page* chose — so a report naming a blocked URL in full would be a
  way for a page to read a URL it was refused. Where a redirect ended, the
  capability token in somebody else's query: both are things a page cannot
  otherwise see, and a narrow policy plus its own report log would hand it
  either. A `data:` URL is reported as its scheme alone, since the body of one
  *is* the content. The page's own URLs keep their path and query and lose
  their fragment and any credentials written into them — `user:password@` in a
  URL is a password, and a log collector is not where it goes.

  **A report that cannot be sent is not a load that fails, and it is not a
  silence either.** A collector that is down, one that answers an error, and a
  `report-to` naming a group nobody defined are each said in words. The
  alternative is worse than it sounds: a policy reporting nowhere looks exactly
  like a policy nothing violated, which is the wrong way round to be wrong
  about whether a site is protected.

  A violation report is its own kind of request rather than an ordinary fetch,
  and that is a rule rather than a label — a policy governs what its page
  loads and deliberately does not govern its own reporting, so a report sent as
  a fetch would be blocked by `connect-src 'none'`, which is a sentence sites
  write. It is refused over plain HTTP from a secure page for the opposite
  reason: it carries the URLs that page was refused.

- **A group of controls looks like a group.** A `<fieldset>` draws a border
  now, and its `<legend>` sits **in** that border rather than above it: the
  line runs through the middle of the legend's words and stops either side of
  them, which is how a browser writes a group's name into the box around it.
  Until now a fieldset drew nothing at all, so three radio buttons under "Pizza
  Size" looked exactly like three radio buttons — the one thing a fieldset is
  *for* was the one thing that was invisible.

  The band the legend sits in **replaces** the block-start border rather than
  adding to it, which is why a fieldset here is exactly as tall as one a
  browser draws, and why two of them stacked line up. A legend written after
  something else is still drawn first, because that is what HTML says a
  fieldset does with its first legend.

  The border is `solid` where every other browser draws a `groove`. This engine
  draws only solid borders and says so: a style drawn as a different style is a
  wrong pixel that looks nearly right, and the two-tone ones are their own
  piece of work rather than an approximation slipped in here.

- **A page can say what it is willing to load, and be believed.** Content
  Security Policy: a site writes `script-src 'self'`, and if a script from
  anywhere else ever appears in one of its pages — because its escaping failed
  somewhere, which is what the sentence is for — this engine refuses to load it.
  `default-src`, `script-src`, `style-src`, `img-src` and `connect-src` are
  enforced, along with the source expressions pages actually use: `'self'`,
  `'none'`, whole schemes, hosts with wildcards, ports and paths, nonces, and
  `'strict-dynamic'`.

  **A word we cannot read never widens a policy.** That is the rule the whole
  feature is built around, because the alternative is a page losing a protection
  it asked for to our not understanding the sentence it asked in. A source
  expression from a version of the specification this engine has not read is
  kept and matches nothing, rather than being dropped; the directive holding it
  is kept whole, rather than falling back to something wider; and a refusal
  names the word, so an author whose page stopped working is told which one.

  Two smaller rules with the same shape. A repeated directive keeps the
  **first**, so anybody who can append to the header — a reflected value, a
  careless proxy — cannot widen a policy by restating one of its directives.
  And two policies are an **intersection**, never a union, so adding one can
  only ever narrow what the first allowed.

  A `Content-Security-Policy-Report-Only` header blocks nothing, which is how
  every policy is deployed: a site watches for a week before it enforces, and a
  browser that enforced the watching header would break the sites being most
  careful. Sending the report is not built yet, and neither is computing a
  content hash — a policy that permits inline content only by hash refuses it
  here and **says so in words** rather than reporting a bare block. A directive
  this engine does not act on is listed rather than assumed, so "this page is
  protected in four respects and not in a fifth" is something a person can read.

- **A cross-origin request is not asked about twice.** Before a page may send a
  `DELETE`, or anything else a plain HTML form could not have sent, this engine
  asks the server first with an `OPTIONS` — which is a whole round trip before
  the real one. A server may say how long its answer holds, and now that answer
  is remembered: a second request of the same shape goes straight out, and one
  of a different shape asks again.

  What is remembered is deliberately narrow, because a cache of permissions is
  a way of obtaining one nobody granted. A `*` is recorded as the method and
  the headers it actually allowed rather than as a standing offer, so a page
  that was allowed a `PUT` still has to ask about a `DELETE`. An answer given
  to a request without cookies does not cover one carrying them. Nothing is
  remembered from an answer that refused. And a server cannot ask to be
  remembered for ever: two hours is the cap, because a permission nobody can
  revoke is not one.

  It is **partitioned by top-level site**, like cookies and like the HTTP
  cache: an entry that made one site's request faster because another site had
  already asked would answer *have you been there* to anybody who timed a load,
  and would survive clearing cookies.

- **A JSON post is asked about with the right question.** `Content-Type` is on
  the list of headers a plain form could have set, but `application/json` is not
  one of the three values a form can produce. This engine knew that when
  deciding *whether* to ask the server, and forgot it when writing the question
  and when reading the answer — so a JSON post was preflighted with a question
  that never mentioned `Content-Type`, and was then allowed by a server that had
  said nothing about it. One rule now, used in all three places.

- **A request can send something now.** Until this change every request this
  engine made was a request for something and nothing more: there was nowhere to
  put a body, so there was no `POST`, no form, and no upload — over either
  protocol. A request carries its bytes now, HTTP/1.1 writes them after the
  blank line, and HTTP/2 sends them in `DATA` frames cut to the size the server
  said it would read.

  The half that only shows up on a large upload is flow control. A server states
  how much it is willing to hold, and this engine sends exactly that much and
  then **waits** — it does not overrun the window and it does not stall, which
  is a distinction a test here makes by watching a server go quiet. When a
  server answers before it has finished reading — a file too large, a redirect —
  the rest of the body is dropped and the stream is closed rather than left open
  for the life of the connection.

  Two rules go with it. The **length a request states is the length of its
  bytes**, never a header a caller wrote: a body and a header disagreeing about
  where a message ends is the request half of request smuggling. And a redirect
  that turns a `POST` into a `GET` now drops the body along with the headers
  that described it, where before there was no body to drop.

- **An interim response is no longer mistaken for the answer.** A server may say
  something before it answers — `103 Early Hints` is sent by a great many of
  them, unprompted — and this engine took the first thing it heard for the
  response. That is a blank page for every such server. Both protocols read past
  them now, and refuse a server that only ever says something first.

- **`Expect` is refused by name rather than sent and not honoured.** An
  expectation is a promise that the sender will wait for a go-ahead, and nothing
  here can bound the waiting; sending the header anyway would ask a server to
  hold a stream open for a signal we had stopped listening for. Nothing on the
  web can reach this — the header is one no page or script may set.

- **Every source file now says what licence it is under.** The engine is
  MPL-2.0, which is copyleft *per file* — so somebody who ends up holding one
  file of it, out of an archive or a search result or a vendored copy, needs to
  be able to read its terms from that file. Until now they could not: the terms
  were in the `LICENSE` at the root and nowhere else. All 198 files carry the
  notice, and the build fails on one that does not, which is the difference
  between a rule and an intention.

- **The list that decides where one site ends now says how old it is.** The
  public suffix list is compiled into the browser rather than fetched, which is
  what makes the boundary work with no network and impossible to move under a
  running program — and it means the list ages. A suffix delegated after our
  copy was taken reads as an ordinary name somebody holds, which quietly puts
  two organisations into one site: one cookie jar, one cache, one renderer
  process. Nothing prompted anybody to refresh it. The build now fails once the
  copy is six months old, and says which version is in, when it was taken, and
  the two commands that bring it up to date.

- **A site is an organisation now, rather than a host.** Until this change every
  subdomain was its own site: signing in at `example.com` left you a stranger at
  `www.example.com`, and the two could not share so much as a cached style
  sheet. The browser reads Mozilla's public suffix list now — the list of names
  anybody may register under — so the site is the name somebody actually holds.
  Two subdomains of one organisation are one site; `bbc.co.uk` and `gov.co.uk`
  are two, which no comparison of the names themselves could have told. One
  answer, and the cookie jar, the cache and the process split all use it.

  It closes a real hole on the way: `Domain=co.uk` was accepted, and the cookie
  it set was one for every school, council and company in the country. The only
  rule refusing anything before was that a domain had to contain a dot.

- **The cache is on a disk now, and it is partitioned.** What was loaded before
  a restart is served after one, under exactly the freshness and `Vary` rules it
  would have been served under from memory — and the key carries the top-level
  site, so a script a thousand sites load from one address is fetched once per
  site rather than once. That costs bandwidth and it buys this: no site can time
  a load to learn where else you have been, and an entry only you were ever
  given cannot follow you between sites.

  What must not outlive the session is **never written** rather than written and
  deleted, because a file that was deleted was still on the disk: a `private`
  response, a request that carried `Authorization`, a response carrying
  `Set-Cookie`, anything that did not come over HTTP, and a body that is not the
  length it was said to be. Every one of them is still reusable from memory for
  as long as the browser is open. A cache with no disk at all is what a session
  that is not meant to persist has — not one emptied at the end.

  A cache file is read as what it is, which is bytes from outside: a magic
  number, a version discarded rather than guessed at, a checksum over the whole
  of it, and every length checked against what is actually there. Anything that
  does not read is a **miss** — never an error that reaches the page, because a
  cache that can stop a page opening is a defect however correct its reasoning
  was. The directory and every file in it are private to their owner, and
  clearing it is one call that really removes the files.

- **Decided: what the cache may write to a disk** (ADR 0011). A cache in memory
  is bytes we already had; a cache on a disk is a durable record of everywhere
  somebody has been, and an input we later hand to a page under that page's own
  origin. So it is **partitioned by top-level site**, exactly as cookies are —
  a shared cache tells any site what you have loaded elsewhere, and an entry
  only you were ever given is an identifier that survives clearing your cookies.
  And what must not outlive the session is **never written**, rather than
  written and deleted: a `private` response, a request that carried
  `Authorization`, a response carrying `Set-Cookie`, a body that did not arrive
  whole, and anything from a session meant not to persist. A deleted file was
  still on the disk. All of it stays cached in memory, where being careful costs
  nothing. The cost is written down too, including that the disk cache is
  weakest exactly where it would help most.

- **A download that stops over HTTP/2 resumes too.** It used to start again at
  zero: the HTTP/2 client turned a stream that ended early into an error, so
  the bytes were gone before anything could ask for the rest of them. It hands
  them up with the reason beside them now, exactly as the HTTP/1.1 client has,
  and a download interrupted half way opens one range request rather than
  beginning again.
- **A connection that ended is not a peer that misbehaved**, and HTTP/2's frame
  reader can now say which it was. The two used to be one error. They are
  different facts: bytes delivered by a connection that then ended were framed
  properly and are worth keeping, and bytes delivered by a peer breaking the
  protocol are not. A reset counts as an ending, because a server hanging up
  part way through a body resets rather than closes tidily — we are still
  sending it window updates for what it just sent us.
- **Two ways an HTTP/2 stream ends early, and only two**: the connection ends,
  and the server gives up on the stream with a `RST_STREAM`. Everything else —
  a header block that will not decode, a window overrun, a frame where none may
  be — stays an error and takes the bytes with it.
- **The download loop is protocol-blind, and now says so.** It moved out of
  `Pool::download` into `alo_net::whole_of`, which takes the exchange as an
  argument and never learns what carried it — which is why resuming over
  HTTP/2 was a change to the HTTP/2 client and to nothing else. It is also what
  makes the loop testable against a server a test started, which matters here
  because this engine speaks HTTP/2 only over TLS and a test may not start one.
- Half a page is still not a page. `client::exchange` refuses a stream that
  ended early exactly as it did, and has a test of its own saying so.

- **A download that stops half way asks for the rest of itself.** A page that
  arrives half way is an error and stays one — half a page is not a page. A
  *file* that arrives half way is a hundred megabytes somebody already waited
  for, and `Pool::download` now asks for the bytes after the ones it holds
  rather than starting again.
- **Where a byte goes is decided by a pure function**, in `alo-net`'s new
  `download` module, so every rule about it is asserted without a socket. The
  four rules, and each is protecting against a file of exactly the right length
  that is not the thing: a `206` must begin **exactly** where the download
  stopped; a `200` answering a range request is byte zero onwards and is never
  appended, so the download starts again and says out loud that it did; nothing
  encoded is ever spliced, which is why a download asks for `identity` from its
  **first** request; and a resume needs an `ETag` or a `Last-Modified` to put in
  `If-Range`, because without one nothing could tell us the file changed between
  the two asks. A weak `ETag` is not taken — it says two representations are
  good enough to swap for one another, which is a different claim from "these
  are the same bytes".
- **`Content-Range` has a parser of its own**, and it is strict for a reason no
  other header is: its three numbers decide *where in a file the bytes that
  follow are written*. A unit that is not `bytes`, the `bytes */1234` form a
  `416` carries, a first byte after the last one, a last byte past the end, a
  sign, two `Content-Range` headers in one response — each refused by name.
- **A body that stops early keeps the bytes that arrived.** They used to be
  thrown away with the error. Only a download may see them, and it never shows
  them: it asks for the rest and checks, byte position by byte position, that
  what comes back belongs where it is put.
- A body that stopped early also has its **codings left on**, deliberately: half
  a gzip is not half a page, and a prefix produced by decompressing one is a
  thing nobody could tell from a whole page. The connection it arrived on is not
  kept either, because there is nothing left on it anybody can find the start of.

- **A body compressed for one hop arrives as a page.**
  `Transfer-Encoding: gzip, chunked` says the content was gzipped and then the
  gzip was cut into chunks — a legal response this engine refused outright,
  because it read the header as one word rather than as the list it is. The
  chunks come off and then the gzip does, in that order, and `br`, `zstd` and
  `deflate` come off the same way.
- **It is a different header from `Content-Encoding`**, and now has a file
  saying so. `Content-Encoding` describes the resource — it is still true in a
  cache, on the next hop and in a saved file. `Transfer-Encoding` describes
  *this connection* and does not survive it, which is why it is undone before
  anything else looks at the message.
- **What is refused is refused by name**, because framing is where being
  generous is a request-smuggling bug: `chunked` anywhere but last (which is
  also what refuses `chunked, chunked`), a coding we cannot undo, a list with an
  empty element, and a compressed body that is *not* ended by `chunked` — legal,
  and delimited by the connection closing, which cannot be told from an attacker
  cutting it short. gzip and zstd carry a checksum that would catch that;
  brotli and raw deflate carry nothing at all.

- **The supervisor's log records runs, not tests.** `--self-test` starts the
  script eight times to check what the arguments mean, and every one of those
  children was appending its own startup lines and its own deliberate `FAILED:`
  messages to the same log — so the record of a run that genuinely failed sat
  among a dozen failures that were tests passing. A log somebody has to filter
  before reading is a log they stop reading.
- The fix is an environment variable rather than a flag, because the children
  that mattered most were the ones failing **during argument parsing** — before
  any flag is known. Something read at the top of the file is the only thing
  that arrives early enough. A dry run and a self-test now write nowhere; a real
  invocation that fails still does, because that is a run.
- And the regression has a test of its own: a child told to log nowhere leaves
  the real log exactly the length it was.

- **A checked control looks checked.** A checkbox that was ticked drew exactly
  one thing — its border — so a person could not tell it from an unchecked one.
  It draws a tick now, a chosen radio draws a dot, and a checkbox that is
  neither on nor off (`aria-checked="mixed"`, the "select all" box above a
  half-selected list) draws a dash. True since controls were built, with an
  example sitting in the corpus the whole time; it took a page with radios and
  checkboxes side by side to make anybody look.
- **A control nobody can operate says so**, and still says what state it is in.
  "You cannot change this" and "this is off" are different things to be told: a
  disabled control draws its mark in grey rather than the accent colour, and its
  border pales, so all four of on/off and live/dead are four different pictures.
- **`accent-color` is read**, which is the property CSS has for what colour a
  control draws its own state in. The mark on top of it is black or white by
  whichever shows up — so a pale accent gives a black tick rather than an
  invisible white one.
- **`border-width`, `border-style` and `border-color` are read as shorthands.**
  Each is one value per side and splits by the same rule as `margin` and
  `padding`; they were not expanded before because nothing in the user-agent
  sheet set one, which stopped being true the moment a disabled control needed a
  paler border. `border` itself still is not: `red solid 1px` and `1px solid
  red` are the same border, so splitting it means parsing rather than counting.
- Corpus case `control-states`: nine controls, no two of them the same picture.

- **A page with a form**, frozen — the HTML specification's own example, which
  contains almost every part of a form at once and has no style sheet. It found
  four things, all now fixed.
- **A `<fieldset>` was laid out inline.** The user-agent sheet declared
  `fieldset` and `legend` as `display: block` and then, forty lines later, as
  `inline-block` — and a duplicate in one sheet is the later rule winning. A
  fieldset was an inline box, its contents came out as seven "pieces" of a
  broken inline, and the page was ninety-six pixels too tall. No alo screen uses
  a fieldset: the corpus had every control and not one group of them.
- **A `<fieldset>` is named by its `<legend>`** now, which is the same shape as
  a `<label>` naming the control it wraps. Without it an agent asked to tick
  "Large" under "Pizza Size" had no way to tell which group was which.
- **A radio button is round.** The tree told a radio from a checkbox from the
  first commit; a person looking at the page could not, because both drew as a
  bordered square. A radio group and a checkbox group mean different things —
  one answer or several — and somebody who cannot see which they are looking at
  is being asked a question without being told its shape.
- **`border-radius` in per cent works**, which making the radio round is how we
  found out it did not. It resolved against zero, because the code computing the
  radii was never given the box — a limitation written down in `corner.rs` and
  never revisited. A percentage radius is a percentage *of the box*:
  horizontally of its width, vertically of its height.
- Two things it found that are **not** fixed and are now queued: a checked
  checkbox looks exactly like an unchecked one (item 182 — true since controls
  were built, with an example sitting in the alo corpus the whole time), and a
  fieldset has no border (item 183).

- **The supervisor takes a number of iterations, keeps a log, and says what it
  did.** Three things that were missing, and all three are about being willing
  to start it rather than about what it does once running.
- `--items 5` runs five iterations and stops. "Run until the queue is empty" is
  a large thing to agree to on faith; it is the same loop either way and only
  the number differs, so somebody deciding whether to trust it at all can buy
  five rather than five hundred.
- Everything goes to `docs/autonomy/loop.log` as well as the terminal, because a
  terminal is the one place a record does not survive closing a window.
- And it ends by saying what **closed** and what was **committed** rather than
  how many iterations it managed — an iteration that halts honestly is worth
  more than one that invented a way past a problem, so counting iterations would
  be counting the wrong thing. A run that closed nothing and committed nothing
  says so, loudly.
- `--self-test` now covers the arguments as well as the stop rule, and the gate
  runs it: a supervisor that read `--items abc` as five hundred, or a typo as a
  request to run forever, is one nobody should trust with an unattended run.

- **JPEG**, rented and pure Rust for ADR 0010's reason — a decoder is where a
  memory bug is most directly a remote code execution, because the attacker
  chooses every byte the allocator sees.
- **The format comes from the bytes, not from the name.** A `src` ending in
  `.png` proves nothing: it is a string on a page, and the server that answered
  may have sent something else, by mistake or on purpose. The corpus case has a
  JPEG served as `/lying-name.png` and it decodes, because what a thing is
  cannot be lied about without also being true.
- **The same bounds either way**, which is why JPEG and PNG are one item rather
  than two: a second decoder with its own limits, or none, would be a second way
  in. Both refuse a picture larger than sixty-four megapixels, both refuse one
  of no size, and both check **before** the allocation. A JPEG's dimensions are
  in its frame header, so the size is knowable without decoding — and there is a
  test that rewrites that header to claim four billion pixels.
- A colour space this engine does not convert — sixteen-bit greyscale, CMYK — is
  **refused by name** rather than approximated, because a wrong conversion is a
  picture in the wrong colours and nobody would know which of the two it was.
- The test picture is twenty-four pixels square with stripes eight rows tall,
  and both of those are for JPEG's sake. The first version was three rows in a
  four-by-three picture — a single DCT block, which chroma subsampling returns
  as mud: the green stripe came back `(130, 123, 115)`. A test that asks a lossy
  format for an exact colour is a test about the format rather than the code, so
  it asks which channel is largest.

- **`<img>` lays out and draws.** A picture arrives as bytes, is decoded, and the
  box it belongs to is told how big it is — so an `<img>` with no width lays out
  at the picture's own size, and one with a width **keeps the picture's ratio**
  rather than coming out squashed.
- The work was **intrinsic sizing**, not pictures. Nothing in `alo-box` or
  `alo-layout` had a notion of a box sized by its content, so a replaced box is
  a new kind of leaf: `NodeKind::Replaced`, beside the text and container kinds
  that were already there.
- A picture that **did not arrive** keeps the box its style asked for and the
  fact is recorded — an empty box of the right shape rather than a collapsed
  page, which is what a browser shows for a broken image.
- **The ratio is a `taffy` aspect ratio rather than a measurement**, and finding
  out why took a wrong turn worth recording: computing the missing dimension
  inside the leaf's measure gave `width: 80px` a height of 3, because the
  measure is asked **before** the style width is applied. A ratio is what
  resolves one definite dimension against the other, which is what a ratio is
  for.
- An `<img>` is `inline-block`, so it goes through the **inline** path rather
  than taffy's leaf layout — which is why the first version sized only
  block-level images and the case caught it.
- Two things named rather than left to be found: the picture is drawn
  **nearest-neighbour**, which is exact at one-to-one and coarse anywhere else;
  and a **rotated** picture draws upright inside the right area rather than
  rotated, because only the rectangle's corners are transformed. Both are
  written into the code and the second is queue item 178. Drawing nothing would
  have been wrong and invisible.
- Corpus cases can hold pictures, listed in the same `linked.txt` as the style
  sheets — because from a case's point of view they are the same thing:
  something the page named, and something frozen next to it.

- **A picture from a page is untrusted bytes, and is now read as such.** The
  PNG reader that existed was written for reference renders — files this engine
  wrote moments earlier, in one format — and being strict there is a feature. A
  page's picture needs the opposite on both counts, so there are two readers and
  the difference between them is written down.
- **Tolerant about what a PNG may be**: palette, greyscale and sixteen-bit are
  normalised to eight-bit RGBA rather than refused, because a page's picture is
  usually one of those and refusing them would be refusing the web.
- **Unforgiving about every number that decides an allocation.** A PNG's
  *header* declares its dimensions, and a decoder that believed one would
  reserve four bytes per declared pixel before reading a single row. A file of a
  hundred-odd bytes that parses perfectly can ask for seventeen gigabytes; the
  bound is sixty-four megapixels and it is checked **before** anything is
  reserved. ADR 0005 names image codecs as a reason for the sandbox; this is the
  half no sandbox would have caught.
- Two tests taught me something I had assumed wrong. A **header alone** is
  refused for having no image data behind it, which says nothing about whether
  the size was checked — so the bomb is a *valid* picture whose header has been
  rewritten and whose checksum mended. And **not every truncation is refused**:
  a file missing only its end marker has all of its image data, and refusing it
  would refuse a picture whose last four bytes were lost in transit, which
  browsers show. What must never happen is a canvas of a size nobody declared,
  and that is what the test asserts.

- **A page's linked style sheets are applied**, and `<link>` and `<style>` are
  collected into **one list in document order** — because a later sheet
  overrides an earlier one and the order is the meaning. A page that links a
  sheet and then writes a `<style>` correcting it depends on exactly that, and
  gathering the two kinds separately would silently reorder every such page.
- **A corpus case can hold more than one file.** A `linked.txt` maps an `href`
  as the page wrote it to a file frozen beside the case. Written down rather
  than inferred from filenames, because the `href` is what the page said and a
  mapping somebody can read is a mapping somebody can check — the same reason a
  case carries an `origin.txt`. Frozen, never fetched.
- **A sheet that did not arrive is a state, not an error.** The page renders
  without it and the fact is recorded as an issue, because a page styled by a
  sheet that never came looks wrong for a reason nobody can see from the page.
- `rel="stylesheet alternate"` is **not** applied. An alternate sheet is one a
  person chooses, and applying it as well would be applying two.

- **A wrapped inline reports the boxes it actually occupies.** A link crossing
  two lines is in two places — the end of one line and the start of the next —
  and their union covers the text between them, which belongs to somebody else.
  The first web page made it visible: `link "Frequently Asked Questions"` came
  back 778 pixels wide starting at the left margin, which is not where it is.
- **A thing is offscreen only when *every* piece of it is.** That is the
  behaviour the union got wrong in both directions: a link whose first line has
  scrolled away is still visible if its second has not, and a union straddling
  the viewport edge looks visible when neither piece is inside it.
- The union is still there and still useful for *roughly where is this* — it is
  simply no longer the answer to *is any of this on screen*. The agent outline
  now says `in 2 pieces` where a node is more than one rectangle, so a reader is
  told the box is a union rather than being left to assume it is not.
- The rectangles cross the process boundary, because the browser process is
  where "what is visible" and "what to draw a highlight around" both happen, and
  a union is not something it could take apart again.

- **`text-decoration` is painted.** It had been in the user-agent sheet all
  along — `underline` on every link — and produced no paint operation at all,
  which the first web page made obvious by being almost nothing but links.
  `underline`, `overline` and `line-through` all work, and all in one change
  because they are the same machinery.
- **A decoration stops at the end of the inline, not at the edge of the line.**
  One line per *fragment*, and a fragment is one piece of one inline on one
  line — so that rule is the whole of the implementation rather than a special
  case in it. A link wrapping across two lines gets two underlines, each its own
  width.
- **The line goes where the face says.** An underline's distance below the
  baseline and its thickness come from the font, because how far letters descend
  decides where a line can go without cutting through them.
- **It propagates rather than inherits**, which is a real difference: an
  underlined `<a>` underlines everything inside it, and a descendant **cannot
  turn that off** — `text-decoration: none` on a child removes nothing, in every
  browser, and that is specified rather than a quirk. So the paint walks
  ancestors instead of the property being made inheritable, which would have
  been close and wrong in a way somebody would eventually hit.
- The colour comes from the element that **declared** the decoration, not from
  the text — so a black `<span>` inside a red underlined one is black text with
  a red line under it. There is a case that would look identical if this were
  wrong, and it is now written so that it does not.

- **A second page we did not write: the first web page.** CERN's restored 1991
  original, and the purest possible test of what item 171 had just finished —
  **it has no style sheet at all**, so every pixel of it comes from the
  user-agent sheet.
- **It found that links had no colour.** The sheet said
  `text-decoration: underline` and nothing else, so a page that is almost
  entirely links rendered as an undifferentiated wall of black text with no way
  to see what could be followed. `a:any-link` has one now — `:any-link` rather
  than `a`, because an `<a>` without an `href` is an anchor and 1991 pages are
  full of them.
- **There are no purple visited links, deliberately.** `:visited` never matches
  in this engine — whether a link has been visited is history, and a style that
  depends on it is readable from the page. That is a privacy decision with a
  visible cost, and it is now written where somebody would otherwise add the
  rule.
- Two more findings filed rather than fixed: `text-decoration: underline` has
  been in the sheet all along and **paints nothing** (queue item 173, and
  nothing in the alo cases underlines anything, so nothing noticed); and a
  wrapped inline reports one rectangle covering all of its lines (item 174).
- What it did *not* find is the point of running it: the `<DL>` indents by forty
  pixels, the `<H1>` is 2em with its margins, and the four things the parser
  could not make sense of are all in `issues.txt` rather than silently dropped.
  `<HEADER>` — 1991 for `<HEAD>` — is read as the HTML5 `<header>` and comes
  back as a `banner` landmark, which is the correct modern reading of markup
  written before either existed.

- **The user-agent sheet has typographic defaults now** — the HTML
  specification's own: `body { margin: 8px }`, headings sized `2em` down to
  `0.67em` with their margins, `p`, `pre`, `hr`, and lists indented 40px. Until
  the first page we had not written arrived, the sheet said what elements *are*
  and nothing about what they look like, and no case noticed because every case
  set its own spacing.
- **And it could not ship without fixing a cascade bug it exposed.** The cascade
  competed declarations **by property name**, so a `padding-left` from one sheet
  and a `padding` from another never met — they were different keys, and
  whichever the reader consulted first won regardless of origin. Invisible while
  the user-agent sheet set no box longhands; the moment it did, an author's
  `ul { padding: 0 }` was silently overridden **by the user agent**, which is the
  cascade upside down.
- The fix is to expand `margin` and `padding` into their longhands **where they
  are written**, so the two compete as the same property. The longhands go in at
  the shorthand's position, so `padding: 1em; padding-left: 0` still ends with a
  left of zero.
- **A wrong turn on the way, caught by a picture.** The first version refused to
  expand a value containing `var()`, reasoning that a custom property may hold
  several values. That made things worse: an author's
  `padding: var(--a) var(--b)` was then the only shorthand left unexpanded, so
  it lost to the user agent's longhand, and every control on every alo screen
  lost its padding. `var()` is now one part like any other function, and the
  split respects parentheses — `1px calc(2px + 3px)` is two values, not four.
- **Every existing corpus case is byte-identical**, which is the review the item
  asked for and a specific answer: alo's own screens set all their own spacing,
  so correct defaults change nothing for them. The one case that moved is the
  one that did not ask.

- **The first page in the corpus that we did not write.** `example.com`, frozen
  as the server sent it, with where it came from and when beside it. Everything
  before it was markup written to exercise something just built — a good way to
  check a thing works and a bad way to find out what is missing.
- It found three things on its first run, which is the argument for having it.
- **A page's style sheet is inside the page**, and nothing collected it. Every
  case until now kept its CSS in a file beside the markup, which is how alo's
  own screens are built — so the gap was not in anything anybody had considered
  and refused. It was in the shape of the corpus, and it was invisible for as
  long as the corpus was ours. `alo_dom::sheets` exists because of this page.
- **A font nobody has is substituted silently.** The page asks for
  `system-ui, sans-serif`; the corpus has DejaVu Sans and nothing else; the text
  was measured and drawn in a family the page did not ask for, and nothing said
  so. The render is stable and diffable and it is not what the page looks like
  anywhere else.
- **The user-agent sheet gives headings and paragraphs no margins**, so a real
  page renders visibly tighter than it should. Invisible until now because alo's
  own sheets set their own spacing.
- What it did *not* find is worth recording, because it was the part being
  tested: `width:60vw` and `margin:15vh auto` resolve exactly, `opacity:0.8`
  groups and ungroups, `a:link` gives `rgb(51 68 136)`, and `font-size:1.5em` is
  24px — none of which had ever been asked of a page we did not write.

- **A renderer draws with a font it was handed, never one it went looking for.**
  ADR 0010 confines renderers, and the consequence people underestimate is that
  a confined renderer cannot open a font file. There were two ways out and the
  harder one was taken: the browser process reads them and passes **bytes**,
  rather than the sandbox policy permitting a font directory. Permitting the
  directory would put a filesystem path in a security policy for one kind of
  resource, and the next kind arrives with the same argument and no way to
  refuse it.
- `alo-render` **no longer embeds a font**. It starts with none, which is the
  design rather than a gap, and is given them before it is given a page — a
  renderer handed a page first would lay it out with nothing to draw text in,
  and the result would be a rendering difference nobody could explain from
  outside.
- Fonts are sent **once per renderer**, not with each page. ADR 0005 asks for a
  coarse protocol, and a font resent with every load would be megabytes a page.
- **Bytes that are not a font are refused when they arrive**, not when text is
  shaped. A font that fails at shaping fails a long way from the moment somebody
  could have been told.
- A renderer answers with the family it **actually found**, rather than echoing
  back the name the browser process guessed from a filename — a renderer drawing
  with something other than what was asked for is a rendering difference nobody
  could explain from the outside.
- The browser process's font search is sorted and bounded: the same fonts in the
  same order on two runs of the same machine, which is what makes a rendering
  difference between runs mean something. `.ttc` collections are deliberately
  skipped — taking the first face of a collection and calling it the family
  would be a font that renders and is not the one anybody asked for.

- **Renderers are confined**, on macOS, by the platform's own sandbox — ADR 0010
  in code. A renderer cannot read `/etc/hosts`, cannot read anything in the home
  directory, cannot write a file and cannot open a socket, and each of those is
  **watched failing** rather than assumed.
- **The test would fail if the sandbox did nothing.** The same binary is run
  twice — confined and not — and the unconfined run has to be *allowed* all four
  things. A test that passed both before and after would be testing nothing, and
  there is now a test asserting that it does not.
- Only a refusal **by the operating system** counts. A connection refused
  because nothing is listening means the socket was created; a file not found
  means the open was allowed. Both look like failure and neither is confinement,
  and a probe that counted them would report a working sandbox on a machine with
  none.
- **`sandbox-exec` rather than the library call**, because the library call is
  FFI and ADR 0010 authorises no `unsafe` here. It is deprecated, which is
  written down as a real cost rather than discovered later — and it has an
  advantage that is not a consolation prize: the profile is applied *by* `exec`,
  so the process is never unconfined, not even for the instant between starting
  and sealing itself.
- The executable's path goes into the profile as a **parameter, not pasted in**.
  A checkout under a directory with a quote in its name would otherwise change
  the meaning of the policy rather than filling in a blank — the same class of
  bug as an injected quote anywhere else, and worse here because the thing being
  injected into is a security policy.
- **A platform with no sandbox gets no renderer**, not an unconfined one. Linux
  is queue item 169; until then this engine does not claim it, which is what ADR
  0010 asks for instead of shipping with the protection off.

- **ADR 0010: the sandbox is rented, and failing to get one is fatal.** A
  renderer confines itself with the operating system's own sandbox — Seatbelt on
  macOS; seccomp-bpf, a user namespace and Landlock on Linux — before it reads a
  byte of any page. A renderer that cannot get one **exits** rather than
  rendering without it.
- The reason for renting is not effort. **A sandbox we wrote would be a sandbox
  only we had tested**, and the bugs in these things are found by people
  attacking them rather than by people reading them.
- **The decision authorises no `unsafe` in this repository.** A rented crate's
  `unsafe` is the crate's, which is where ADR 0005 already puts TLS and codecs.
  If a platform ever needs FFI we write ourselves, that comes back for its own
  ADR — said explicitly so nobody reads "the sandbox ADR" as having settled it.
- Failing closed is only defensible if it is rare, so it comes with a promise:
  **the browser does not claim a platform it cannot sandbox.** A platform with
  no sandbox is one we do not ship, not one we ship with the protection quietly
  off.
- And the consequence people underestimate: a confined renderer cannot open a
  font file. **The browser process passes bytes; the renderer opens nothing** —
  rather than the tempting answer of permitting a font directory, which puts a
  filesystem path in the policy for every resource type that follows.

- **One renderer process per site.** ADR 0005's central claim is now processes
  that actually exist: two sites are two processes, two tabs on one site are
  one, and there is a real binary — `alo-render` — that reads work from a pipe
  and answers on another. The tests spawn it and talk to it for real.
- **Killing one renderer leaves the other running**, which is the entire reason
  for the design and is now a test that kills a process while another is
  serving. A page that finds a way to take over the process it is rendered in
  has taken over a process holding **one site's pages and nothing else** — no
  network, no disk, no profile.
- **A dead renderer is not quietly restarted.** ADR 0005: *"a browser that
  silently restarts a renderer hides a bug that somebody needs to see."* So the
  failing request fails, the entry is dropped, and the next *deliberate* load
  gets a fresh process — a distinction with a test of its own, because a silent
  restart would turn a page that crashes its renderer every time into an
  invisible loop.
- A pipe has no message boundaries of its own, so every message says how long it
  is — and that length is **checked before the read**, because the read is where
  the memory goes. A stream ending *between* messages is told apart from one
  ending *inside* one: a renderer that finished and exited is not a renderer
  that crashed, and a browser that could not tell would report a bug every time
  a tab closed.
- **What a "site" is, said out loud rather than assumed.** ADR 0005 says scheme
  plus registrable domain; the registrable domain needs the public suffix list
  that is queue item 156, so today it is scheme plus **host**. That is stricter
  — `a.example.com` and `b.example.com` get separate processes where they should
  share one — and stricter is the safe direction. It is written down where
  somebody adding the suffix list will find it, because the failure in the other
  direction is two sites sharing a process because we could not tell them apart.
- Sixteen renderers at most, evicting the least recently used: N processes cost
  N processes, and a browser with three hundred tabs cannot be three hundred
  processes.

- **The renderer boundary has a wire format.** ADR 0005 built the boundary as a
  *type* so the process split would be a change of **transport** rather than a
  redesign; this is that transport's encoding, and it had to be right before
  anything is spawned. Spawning is queue item 166 and the sandbox is 167 — which
  needs an ADR of its own, because ADR 0005 says it does not pre-authorise the
  `unsafe` a sandbox may require.
- **A message from a renderer is treated as bytes a stranger chose**, because a
  renderer is the process that parsed a hostile page — if that page found a way
  to steer it, everything it says afterwards is the page talking. Every length is
  checked against what is actually left before anything is reserved; a count of
  a billion in a message with no room for them is refused rather than allocated.
- **A tree deeper than 512 is refused rather than recursed into.** A decoder
  that recursed as far as it was told would run out of stack — a crash in the
  *browser* process, caused by the renderer, which is the one thing ADR 0005
  says must never happen.
- A frame whose size and pixels disagree is refused; so is a `NaN` or an
  infinity, because every comparison against one answers false and that turns a
  bounds check into a thing that passes. Anything left over after a message is
  refused too — trailing bytes mean the two ends disagree, and ignoring them
  lets a sender append something a later version would read.
- **An id in a message is a claim, not a fact.** `BoxId::from_wire` exists so a
  snapshot can cross with its ids intact, and it says in its own documentation
  that an id is meaningful only against the snapshot it arrived with. ADR 0003's
  "allocated once, never reused" is a promise the *allocating* process makes,
  and a process on the other side of a pipe is not obliged to keep it.
- Roles cross **by name**, so adding one never renumbers the others, and a name
  this engine does not know becomes a declared role rather than an error — which
  is how a role we have never heard of still reaches an agent. `KnownRole` gained
  `named` and `ALL`, with a round-trip test so the name and the parse cannot
  drift apart.

- **HSTS, mixed-content blocking and referrer policy** — three things a site
  says about *itself*, which is the opposite direction from CORS. CSP is queued
  separately as item 165: its grammar is a whole item on its own, and doing it
  badly quietly weakens the protection a page asked for.
- **A site visited once is never reached insecurely again.** The attack is
  twenty years old and no amount of correct TLS touches it: somebody types
  `example.com`, the browser tries `http://`, and a network in between answers
  before the real server is ever asked. Two rules make HSTS a defence rather
  than a weapon — a `Strict-Transport-Security` arriving over **plain HTTP is
  ignored** (or an attacker already rewriting your traffic could pin any domain
  for two years), and it **never applies to an IP address** (which belongs to
  whoever holds it today).
- Subdomain coverage is walked label by label, so `evil-example.com` does not
  inherit `example.com`'s pin — a suffix comparison says it does. A site can
  release itself with `max-age=0`, because one that could pin and never unpin is
  one nobody could move off TLS in an emergency. And a pin is capped at two
  years.
- **Mixed content is not one rule, because the answer differs by what the thing
  is.** A script or stylesheet replaced in transit does not look at the page —
  it *is* the page, and nothing recovers from that, so it is refused with
  nothing offered. An image replaced in transit is a wrong picture: those are
  tried over TLS first, because a great many sites have an `http://` URL in
  their markup and a perfectly good `https://` server.
- `http://localhost` is **secure**, and not by convention: there is no network
  between the two ends, so there is nothing in between to attack. Refusing it
  would break every developer on earth while protecting nobody.
- **The referrer default is `strict-origin-when-cross-origin`.** A full URL
  carries the path and the query, and a great many of those are the message —
  `/reset-password?token=…`, `/results/hiv-test?patient=…`. Your own site gets
  the whole URL; anybody else gets the origin; a downgrade to `http` gets
  nothing, because what we would be sending is exactly what an attacker on that
  connection is there to read.
- A referrer policy nobody can read **leaves the default in place rather than
  weakening it**, and the last policy in a list that this engine *understands*
  is the one that applies — which is how a site offers a strict policy to
  browsers that have it without an unknown value at the end discarding it.

- **The same-origin policy, CORS and preflight.** The policy is not that a page
  may not *send* a request elsewhere — it may, and the web depends on it. It is
  that a page may not **read the answer** without the other site agreeing. An
  image from another site draws, a form posts, a script runs, and none of them
  hand the page anything readable.
- **A wildcard does not hand over a page fetched with cookies.** `*` means
  "anyone may read this, and it contains nothing personal", and a request
  carrying credentials contradicts that by existing. Without this rule, every
  server that ever wrote `*` for a public file would be giving away its
  logged-in pages too.
- A scheme or a port is enough to make it somebody else, and **two opaque
  origins are not the same origin as each other** — a comparison on the
  serialised string would make every `file:` page and every sandboxed frame one
  origin, all reading each other.
- **A page cannot read a `Set-Cookie` it was never given.** A cross-origin
  response's headers are filtered to the ones the server exposed plus the ones a
  form could already have revealed; a wildcard exposure still does not reach
  `Set-Cookie`, which is not the page's to read under any arrangement.
- **Preflight asks a question that does nothing.** The `OPTIONS` carries no
  credentials of its own, because "may I do this" must not itself act on
  somebody's behalf. And the rule for *when* to ask is not "is it dangerous" but
  **could a plain HTML form have done this already** — if it could, asking first
  would make the web slower and protect nothing. A `DELETE` that arrived and was
  then refused is a `DELETE` that happened.
- A wildcard in `Access-Control-Allow-Headers` **never covers
  `Authorization`**. `*` is written by people who mean "my public API", and a
  credential is never that.
- A test found a real bug on the way: `Cookie` is set by the browser and never
  by the page, and counting it as an author header got two things wrong at once
  — every credentialled request would have been preflighted, and the preflight
  would have told the server the page asked for something it cannot ask for.

- **HTTP/2 is negotiated and spoken.** ALPN advertises `h2` then `http/1.1` in
  the TLS handshake, so **the protocol is known before the first byte of a
  request goes out**. That is the whole reason ALPN exists rather than a version
  header: a client that discovered the protocol afterwards would have to send
  the request again, and a `POST` sent twice is a payment made twice.
- A server that says nothing about ALPN gets HTTP/1.1 — the protocol everybody
  speaks without having to say so. A plain connection is always HTTP/1.1:
  reaching HTTP/2 without TLS needs prior knowledge (guessing) or an `Upgrade`
  (sending a request that may have to be sent again), and this engine does
  neither.
- **A request becomes four pseudo-headers and then the rest.** There is no
  request line in HTTP/2; the method, scheme, path and authority are headers
  whose names begin with a colon — a character no ordinary header name may
  contain, which is what makes them impossible to forge from an ordinary one.
  They go first, and a *response* carrying one, or an ordinary header before
  `:status`, is refused: a pseudo-header after an ordinary one is how a message
  gets smuggled past something that only reads the first few headers.
- `Connection`, `Host`, `Transfer-Encoding` and the rest of the hop-by-hop
  headers are **not sent**. HTTP/2 has its own way of saying all of it, and a
  server receiving one must treat the message as malformed — so sending one is
  not a compatibility gesture, it is a broken request. Names go out lowercase,
  which is a requirement rather than a convention.
- **A credential is marked never-indexed**, so `Authorization` and `Cookie` are
  never put in a compression table — ours or any relay's. The same rule ADR 0007
  applies to cookies, applied to compression.
- The HPACK tables and stream bookkeeping belong to the **connection**, not the
  exchange. Losing them between requests would mean the second request on a
  connection could not be decoded at all — the same class of mistake as throwing
  away a read-ahead buffer, with the same symptom: everything works once.
- `SETTINGS` and `PING` are answered as they arrive rather than at the end. A
  peer waiting for an acknowledgement stops sending, so a client that replied
  only once it had a whole response would deadlock waiting for a response the
  server is waiting to be allowed to send.

- **HTTP/2 streams, flow control and the connection state machine.** The bounds
  went in before the happy path, because every way a peer spends our memory over
  HTTP/2 is a **count** rather than one oversized thing.
- **The CONTINUATION flood is refused.** A header block may be spread across any
  number of `CONTINUATION` frames; each one is inside the frame-size limit and
  nothing limits how many there are. So the bound is on the **total across
  frames**, which is why it is counted by the session rather than the frame
  reader. A `CONTINUATION` sequence is also uninterruptible: a frame for another
  stream in the middle of one would let a peer make two header blocks into one.
- **A stream is not open or closed.** It is open in each direction separately,
  and a request fully sent while its response is still arriving — the normal
  state of every request a browser makes — is *half-closed locally*. Collapsing
  that into a boolean is how a `DATA` frame arriving after a response finished
  becomes a body silently appended to a page instead of a `STREAM_CLOSED`.
- **A window may legitimately be negative**, and that is the one that surprises
  people. Lowering `SETTINGS_INITIAL_WINDOW_SIZE` applies as a difference to
  every stream that already exists, so data already in flight can leave a window
  below zero — a peer doing that has done nothing wrong, and refusing it would
  break them. What *is* refused is a window widened past the protocol's ceiling,
  and it is refused rather than saturated: saturating leaves the two ends
  disagreeing about how much may be sent.
- Bounded: how many streams a peer may have open at once, how many closed ones
  are remembered (so opening and resetting forever leaves nothing behind), and
  what a peer allows *us* kept separate from what we allow *them* — mixing those
  up means either refusing our own requests or accepting an unbounded number of
  theirs.
- **`PUSH_PROMISE` is refused.** This engine sends `ENABLE_PUSH: 0`, so a server
  that pushes has ignored what it was told, and honouring it would be accepting
  a response to a request nobody made.
- A `WINDOW_UPDATE` for a stream that is already gone is **ignored, not
  refused**: the peer sent it before it knew, and ending a connection over a
  race nobody lost would be worse than the race.

- **HPACK**, the header compression HTTP/2 carries — integers, strings, the
  static table, the dynamic table, and Huffman. Checked against the
  specification's own worked examples: the exact bytes it prints, and the exact
  table sizes it says should exist after each block (57, 110, 164, 222). A codec
  that only agreed with itself would prove nothing, because HPACK's whole job is
  to agree with somebody else's encoder.
- **The Huffman codes are derived, not transcribed.** The specification prints
  257 rows; copying them is 257 chances at a bug that shows up on the one byte
  nobody tested — and a wrong code is not a crash, it is a header that silently
  decodes to something else. The code is canonical, so only *which symbols have
  which length* is written down and the codes follow. Two tests check the
  structure itself: that the code space is exactly filled, and that all 256
  bytes round-trip.
- **A decoding failure kills the connection, never one stream.** The table
  carries state from block to block, so a block nobody could decode leaves it in
  a condition nobody can reason about; resetting the stream and carrying on is
  the tempting, wrong answer, and there is a test asserting every failure is
  fatal.
- Refused: index zero (which means "a name follows", and reading it as an index
  is off-by-one into the static table); an integer that never ends, before it
  can overflow; a string longer than the block containing it; a table size
  update larger than was agreed, or one appearing after a header in the block;
  a Huffman string padded with anything but ones, or containing the
  end-of-string symbol.
- `never-indexed` **survives decoding**. It is how a sender says a value is a
  secret, and a relay that forgot would compress somebody's authorization token
  into a shared table. Nothing relays yet; the flag is kept so that when
  something does, the information is there rather than remembered.

- **HTTP/2 framing**: nine bytes of header, a payload, and every rule about what
  makes one unreadable. The rest of HTTP/2 — HPACK, streams and flow control,
  and negotiating the protocol at all — is queued as items 160, 161 and 162.
  Framing first, because everything else is carried inside it and because it is
  where a peer chooses how much memory we allocate.
- **A length is checked before anything is reserved.** HTTP/1.1 had two numbers
  a stranger chose — `Content-Length` and a chunk size. This has one per frame,
  several thousand times a page.
- **Padding is subtracted before it is trusted.** A frame's first byte may say
  how much of the rest is padding, and nothing stops it saying more than there
  is; subtracting without checking underflows, and in a language where that is
  not caught it reads whatever was next in memory. It is the classic HTTP/2
  parser bug and it is refused by name.
- A frame that is **entirely padding** is legal and carries nothing — servers
  send them to disguise how large a response is, and a bounds check written one
  off would refuse them. There is a test for that boundary on both sides.
- **An unknown frame type is ignored, not refused** — the protocol is extensible
  on purpose — but its length is still checked and its bytes still consumed
  exactly, because an "ignore" that lost the stream's place is worse than a
  refusal.
- The reserved top bit of a stream identifier is **ignored rather than
  rejected**; a reader that fails to mask it sees stream numbers near two
  billion.
- A `WINDOW_UPDATE` offering no more room is refused — room for nothing is not
  room, and left unchecked it is a peer that can make this end wait forever. It
  is fatal on the connection and survivable on a stream, which is the difference
  the error type carries.
- An error code this engine cannot name is **carried as sent** rather than
  flattened to "an error": a peer sending one is telling us something.

- **The engine is MPL-2.0 now, not Apache-2.0** (ADR 0009). Anyone may still
  embed alo browser in a closed product — that was the point of being permissive
  and it has not changed. What is no longer allowed is taking this engine,
  improving it privately, and shipping a better version of it against us: MPL is
  file-level copyleft, so changes to these files come back. Apache-2.0 permitted
  exactly that, including for the agent tree of ADR 0002, which is the one
  genuinely novel thing here. Servo — whose parser and selector engine we rent —
  chose the same licence for the same reason. Done now because every commit here
  has a single author, and relicensing after outside contributors arrive needs
  every one of them to agree.
- **Names become addresses through the machine's own resolver** (ADR 0008).
  Nothing speaks DNS; we call what the operating system was configured to call,
  which is where a VPN, a corporate network's internal names, a Pi-hole,
  `/etc/hosts` and the machine's own encrypted DNS already live.
- **A public page can no longer be made to reach a private address.** That is
  DNS rebinding, and the rule turns on **who asked**: a person typing an
  intranet name reaches it; a page on the web that resolves to `192.168.1.1`,
  `127.0.0.1` or `169.254.169.254` does not. Loopback, private, link-local,
  carrier-grade NAT, benchmarking, reserved and broadcast ranges are all
  refused, in v4 and v6 — **including a v4 address wearing a v6 hat**, which is
  how `::ffff:127.0.0.1` walks past a check that only knows `::1`.
- A name resolving to both a public and a private address is still reachable, at
  the public one. Refusing outright would break real hosts whose resolver hands
  back an internal address alongside an external one.
- **Connecting takes addresses, not a name.** `TcpStream::connect((host, port))`
  would resolve a second time inside the standard library, where nothing could
  refuse a private answer — and a name that answers differently the second time
  is precisely the attack.
- Every address is tried, not only the first: a host whose IPv6 address is
  unreachable from this network was a host this browser could not load.
- Answers are reused for half a minute — and that is **a guess, named as one**.
  The platform resolver does not hand back the record's TTL, so there is nothing
  truthful to use. A cached answer never carries a permission it was granted
  earlier: the lookup is shared, the rebinding rule is applied every time.

- **ADR 0008: DNS is the machine's choice until somebody changes it.** This
  browser uses the resolver the machine is configured to use and never silently
  replaces it. Encrypted DNS is offered, named, and chosen — and the name of the
  company that would see every site you visit is in the sentence where you
  choose it.
- The ADR says why "encrypted by default" is not the obvious win it looks like:
  it does not make the record go away, it **moves** it — from whoever runs the
  network you happen to be on, to one resolver, globally, tied to your IP,
  across every network you ever join. And a default resolver slot is a business
  we decline to be in, for the same reason ADR 0007 refused to ship an
  allowlist.
- Two rules that hold whichever resolver is used: **DNS is never trusted for
  anything security decides** — a wrong address produces a certificate error,
  not a wrong page — and **a public name that resolves to a private address is
  refused**, which is DNS rebinding.

- **Cookies, partitioned by default** — ADR 0007, in code. A cookie is keyed by
  the site that set it *and* the top-level site it was set inside, so one
  embedded server cannot tell that the person on `news.example` is the person on
  `shop.example`. It may still remember something about each; it cannot join
  them.
- **There is no way to ask for cookies without a partition.** Every lookup takes
  the top-level site, and no function returns the unpartitioned set — the
  promise is kept by the shape of the code rather than by everybody remembering.
- **`SameSite=Lax` when a site does not say.** The historical default was
  `None`; a cookie with no `SameSite` is one whose author did not think about
  cross-site use, and the safe reading of that is not "send it everywhere". The
  case this removes: a `Lax` cookie rides a *navigation* from another site —
  clicking a link to your bank — but not anything a page **embedded**, which is
  what an attacker's form post is.
- **The prefixes are enforced, not parsed.** A `__Host-` cookie that is not
  `Secure`, or has a `Domain`, or whose `Path` is not `/`, is **rejected** —
  never stored with the prefix quietly relaxed. The whole value of a prefix is
  that a server can trust the name.
- `SameSite=None` without `Secure` is refused; a `Domain` the page is not part
  of is refused; a single-label `Domain` is refused. Domain matching requires
  the dot, so `evil-example.com` is not a subdomain of `example.com`.
- Bounds: 4 KiB a cookie, 400 days however far off the expiry says, 180 per site
  and 10,000 in all — and one site filling its own jar cannot evict another
  site's cookies.
- **Clearing a site now means everything set inside it**, not only what it set
  itself. Unpartitioned, that second set was unreachable; this is something
  partitioning makes possible rather than merely safer.
- Written down rather than assumed: the site boundary is the **host** today,
  which is stricter than the registrable domain a public suffix list would give.
  Stricter is the safe direction to be wrong in, and the list is queue item 156.

- **ADR 0007: cookies are partitioned by default** — stored under the site that
  set them *and* the top-level site they were set inside, so one embedded third
  party cannot tell that the person on one site is the person on another. With
  `SameSite=Lax` when a site does not say, `Secure` required to cross a site
  boundary, `HttpOnly` binding the scripting engine from the day there is one,
  and the `__Host-`/`__Secure-` prefixes enforced rather than parsed.
- The ADR is written before any code, and it says what the default **costs**:
  federated sign-in, embedded widgets and corporate SSO break, and a blocked
  cookie is not an error a site can catch — it is a login that quietly does not
  stick. It also says that "every other browser does this" is not available as a
  justification, because Chrome abandoned the plan in 2024.

- **An HTTP cache, with the semantics that make one safe.** `ROADMAP.md` says of
  this: *"subtly wrong here is invisible for months and then serves somebody a
  stale bank page."* So **nothing in it reads the clock** — every function takes
  `now` — because the answers that matter are the ones that are only wrong an
  hour later, and those are the ones nobody finds by using the browser.
- **Age is not how long we have had it.** A response can arrive already old, and
  say so in an `Age` header. A cache that counted from arrival would grant it a
  second full lifetime, which is how one `max-age=3600` becomes six hours of
  staleness across a chain of caches. Time spent in transit counts too.
- **`Vary` is a contract, not a header.** What is stored is the response
  *together with the request header values it was chosen by*, so a page fetched
  with `Accept-Language: fr` is never served to a request asking for `de`. An
  absent header and an empty one are different. `Vary: *` is never stored at
  all — there is no key that would be right.
- `no-cache` and `no-store` are different things, and now they behave that way:
  the first is stored and always revalidated, the second is never written down.
- A `304` **refreshes the headers and keeps the body** — and a `Content-Length`
  on a `304` describes a body it did not send, so it is ignored rather than
  believed.
- Freshness from `max-age`, from `Expires` against the response's own `Date`,
  and **guessed** from `Last-Modified` when nothing was said — a tenth of the
  age, capped at a day, so a page nobody configured is never a week out of date.
  A date nobody can parse is expired, never permission.
- Request directives are honoured: `no-cache` on a reload, `min-fresh`,
  `max-stale` — the last of which does **not** override a server's
  `must-revalidate`.
- All three HTTP date formats are read, including the two obsolete ones, because
  refusing them would make a real `Expires` unparseable and an unparseable
  `Expires` means already stale. Being strict there makes a browser slower, not
  safer. Written dates use the one format anything may send.
- `Cache-Control` is parsed once for both ends, because the mistakes are in the
  syntax: `no-cache="Set-Cookie, X-Thing"` is one directive with a comma in it,
  and a `max-age` that is not a number is absent rather than zero — zero would
  mean "always revalidate", a decision the server did not make.
- Loads go through it. `Pool` owns the cache, so a second load of a fresh thing
  never reaches the server, and a write to a URL forgets what was stored for it.

- **Redirects are followed**, and the deciding is a pure function — a request
  and a response in, what to do next out, with no socket near it. Following is
  three lines of loop; what to *carry across* one is where every bug is, and
  every one of those bugs is a security bug.
- **`Authorization` does not cross an origin.** The site being redirected to
  did not have this site's credentials a moment ago and must not have them now.
  Same for `Cookie` and `Proxy-Authorization` — cookies do not exist yet, and
  the list is right in advance so nobody has to remember to update it.
- A **scheme is part of an origin**, so `https` → `http` on the same host is a
  crossing and a session cookie is not about to go out in the clear. So is a
  port. Both are what a hand-written host comparison gets wrong.
- **A redirected `POST` becomes a `GET`** on `301`, `302` and `303`, which is
  what every browser has done since the nineteen-nineties: silently
  re-submitting a form somewhere new is worse than being wrong about an RFC.
  `307` and `308` exist so a server can ask for the specified behaviour, and
  those are honoured exactly. `HEAD` survives all five.
- **A redirect into `file:` or `data:` is refused by name.** This engine
  fetches both when asked directly and refuses to be *sent* to either: a server
  that could redirect a load into `file:///` would be reading the disk of
  whoever opened the page.
- Twenty hops at most, and **a circle is told from a chain** — two URLs pointing
  at each other say so in words rather than becoming a tab that will not close.
  The trail keeps the order, which is what somebody debugging one wants to read.
- A `3xx` **with no `Location` is the answer**, not a failure: a redirect that
  does not say where is not a redirect, and its body is the only thing to show.
- A relative `Location` resolves against **where the response came from**, which
  after one hop is not where the request started.

- **The build loop has a supervisor of its own**, `scripts/loop.sh`, for macOS
  (ADR 0006). It used to borrow one from `alo-workplace` — a repository this
  loop may not edit, which meant the command documented for it had been wrong
  since the day it was written and could not be fixed from here.
- Two defects found by running the thing rather than reading it. The documented
  command passed `--repo`, which the borrowed script has never parsed, so the
  flag became the repository path and the checkout became a track name.
- And the one that mattered: **a stale stop marker would have halted the loop
  immediately.** The journal has `LOOP COMPLETE` at line 1531 of 2500-odd —
  stage 1 finished, said so, and stage 2 was started underneath it. Any
  supervisor that searched the file for those words would have stopped on its
  first tick and reported the queue complete with ninety-nine items open. A
  marker is now live only while no iteration entry follows it.
- The supervisor **refuses to start on a tree where the gate does not pass**,
  takes a lock so two of them cannot edit one checkout, and presumes a worker
  hung when it goes *silent* rather than when it takes a long time — an honest
  long item keeps writing to its transcript and a hung one does not.
- `--self-test` asserts the stop rule against seven journals, and
  `scripts/gate.sh` runs it. Getting that rule wrong is the failure that looks
  exactly like the work being done, so it is not left to rot.

- **Bodies that arrive compressed are undone**: gzip, brotli, zstd, and
  `deflate` — which is two formats sharing one name, so both the zlib-wrapped
  one the specification asks for and the raw one a great many servers send.
  Requests now say `Accept-Encoding: br, zstd, gzip, deflate`, and a caller who
  wants something else — a resumed download wants `identity` — keeps it.
- All three are rented (ADR 0001) and all three are **pure Rust on purpose**. A
  decompressor is the single place in a browser where a memory bug is most
  directly a remote code execution, because the attacker chooses every byte the
  allocator sees. So `flate2` on its `rust_backend` rather than the C zlib it
  defaults to, and `ruzstd` rather than the C `zstd` bindings.
- **The limit is on what comes out, and that is the whole point.** Every other
  bound in this crate watches what arrives — a `Content-Length`, a chunk header
  — and none of them help here, because compression is the art of arriving
  small. A gigabyte of zeroes is a megabyte of gzip and six hundred bytes of
  brotli. Eight kibibytes in the corpus decode to eight mebibytes, and are
  refused.
- **A zstd body whose contents do not match its own checksum is now refused.**
  `ruzstd` computes the frame's checksum, and reads the one the frame carries,
  and compares them for nobody — so a stream with a flipped byte decoded into
  rubbish and reported success. A test caught it; the comparison is ours.
- Written down rather than assumed: **raw DEFLATE and brotli carry no integrity
  check at all**, so a corruption that leaves a structurally valid stream is
  undetectable in any implementation. That is a property of the formats, and
  what protects those two on the wire is TLS.
- The fixtures were made by `gzip`, `brotli`, `zstd` and Python's `zlib` — not
  by the crates that read them. A suite that compresses with what it
  decompresses with proves one crate agrees with itself, which is not the
  question anybody is asking.

- **Connections are kept between requests.** Opening a socket costs a round
  trip and a TLS one costs two or three; a page asks for thirty things from the
  same host, so a browser that opened thirty connections would spend most of a
  page load saying hello. Three fetches of one host now use one socket.
- **The buffer moved from the exchange to the connection**, which is the change
  that made reuse possible at all. Reading a response means reading *ahead* —
  by the time one body ends, the reader may hold the beginning of the next
  response. Throwing that reader away between exchanges leaves the next one
  starting in the middle of a sentence.
- **A kept connection is a bet, and losing it is a retry rather than a
  failure.** A server can close an idle connection at any moment and there is
  no way to be told. So a request is tried again only when all three hold: the
  connection was **reused**, **not one byte** of an answer arrived, and the
  method is one where doing it twice is the same as doing it once.
- That third condition is about the **method**, not about how likely it seems.
  A `POST` that failed after the server received it is a payment that has
  happened; sending it again is a payment that has happened twice.
- The **scheme is part of which server a connection goes to**, so an `http`
  connection is never handed out for an `https` request — doing that would send
  a page's cookies in the clear.
- Bounds, because a pool without them is a file-descriptor leak: six idle
  connections per host, sixty-four in all, and twenty seconds before an idle one
  is closed rather than gambled on.
- How long to wait for a server that has gone quiet is now something a caller
  chooses. A browser wants tens of seconds; the test for a server that never
  answers wants half of one, and used to make the whole suite wait thirty.

- **HTTP/1.1**, ours rather than rented. The syntax is a few lines of ASCII and
  the difficulty is not reading it — it is **refusing the readings that are
  almost right**, because nearly every famous HTTP bug is a parser being
  generous. Four are refused by name:
  - Two `Content-Length` headers that disagree, which is this parser and the
    proxy in front of it disagreeing about where a response ends — request
    smuggling in its plainest form.
  - `Content-Length` and `Transfer-Encoding` together. The same bug, spelled
    differently.
  - A space before the colon. Some parsers accept it and some do not, and a
    chain containing both is a smuggling chain.
  - A header continued onto the next line, removed from the standard in 2014
    for exactly this reason.
- **A truncated body is an error, not a short page.** A browser that showed the
  first half of a bank statement and said nothing would be worse than one that
  showed nothing.
- **A `204` gets no body however loudly it claims one**, because a parser that
  believed the header would read the *next* response as this one's body.
- Every limit is a named constant rather than a number in a condition — the
  longest line, the most headers, the largest body, the largest chunk. Without
  them a server can make this process allocate for as long as it cares to send,
  which costs the sender nothing.
- `http:` and `https:` fetch now, over a socket, with TLS through the same
  verification queue item 52 built. **One exchange, then closed** — pooling and
  keep-alive are cut into their own item, because framing is the half where
  being wrong is a security bug and it deserved the whole iteration.

- **The queue covers all four stages**, so the loop can always say what is next.
  Stage 3 (the legacy tail) and stage 4 (the product) are written out as items
  with what each depends on — and, more usefully, with what each is *blocked
  on*, because almost all of them are.
- **Every stage 3 item is `blocked: no page yet`, and that is its real state.**
  `ROADMAP.md` says to let a broken render schedule that work, so an item is
  opened by a corpus case failing because of it and by nothing else. A loop that
  started one to have something to do would be building the legacy tail for its
  own sake, and refusing to do that is what made stages 1 and 2 survivable.
- **Every stage 4 item is `blocked: stage 2's exit gate`** — which is *"a person
  uses it as their browser for a week"*. That is a judgement a person makes, and
  `LOOP.md` now says plainly that a loop must never certify it on their behalf:
  when stage 2's queue empties, it writes `LOOP COMPLETE`, names what a person
  has to do, and stops.
- So `LOOP COMPLETE` has a precise meaning now rather than an approximate one:
  every remaining item is blocked, the blocks are real, and the two kinds are
  named separately — pages nobody has hit yet, and a judgement nobody has made.

- **TLS** (`rustls`, rented behind one file — ADR 0001 names it among the
  physics, and ADR 0005 names it first among the rented code whose `unsafe` is
  not ours to remove). `ring` rather than the default provider: both carry C,
  and `ring` is the smaller and more widely audited of the two.
- **The part that is ours is what a person is told.** Every other browser has
  arrived at the same interstitial — *your connection is not private* — and a
  button that goes on anyway. People press it, because the page says neither
  what is wrong nor what pressing it would mean. So a refusal here is a **type**
  carrying three things a caller cannot show one of without the others: what is
  wrong, in a sentence; what trusting it anyway would mean, in a sentence; and
  whether the fault has an innocent explanation at all.
- An expired certificate, a clock that is wrong, and an organisation's own
  authority **could** be trusted — all three happen constantly and none is an
  attack. A wrong host **could not**: it is what an interception looks like, and
  no amount of a person's confidence changes what the bytes say.
- **Nothing is bypassable.** There is no flag, no constructor and no feature
  that turns verification off. `rustls` makes it possible; this does not do it
  and does not expose the seam. Trusting nobody trusts *nothing* — an empty list
  of authorities refuses everything rather than accepting everything.
- Trust comes from **the operating system's own store** rather than a bundle
  compiled in: an organisation that runs its own certificate authority has
  already told the OS about it, and a bundle we shipped would go stale the day
  after we shipped it.

- **Loading** (`alo-net`): a request, a response, a status, headers, a media
  type and a body — with `data:` and `file:` as the only schemes and **no
  network in it at all**. That is on purpose: the shape is the same whether the
  bytes came from a socket, a file or the URL itself, so it is built and tested
  against the two that need nothing, and HTTP arrives as *one more arm of a
  `match`* rather than as a second pipeline beside this one.
- **It lives in the browser process.** ADR 0005 gives a renderer no filesystem,
  no network and no way to name anything outside itself, so fetching is a
  privilege boundary rather than a division of labour. A renderer is now *handed*
  a fetched response instead of a string somebody read for it.
- **Which encoding a page is in** — the byte order mark, then the
  `Content-Type`, then a `<meta>` in the first kilobyte, then UTF-8 because that
  is what the modern web is. The tables are rented (`encoding_rs`); the
  *algorithm* is ours, because it is a sequence of rules rather than a table and
  getting it wrong shows up as mojibake on somebody's news site.
- **A page that decoded badly says so.** Bytes that are not what they claim
  become replacement characters and the fact is kept, so somebody can find out
  why a page looks wrong instead of guessing.
- Headers are a **list, not a map**: names fold case, but order is observable
  and `Set-Cookie` appearing three times means three cookies.

- **URLs and origins** (`alo-url`), the first item of stage 2. Every security
  decision a browser makes — the same-origin policy, CORS, cookies, CSP, which
  site gets which process — asks *which origin is this?*, and all of them are
  wrong if the answer is. That is why it is first.
- **Parsing is rented** (`url`, behind one file, ADR 0001), and it drags IDNA in
  with it: whether `аpple.com` written in Cyrillic is the same host as
  `apple.com` is a security question whose answer is a Unicode table. The types
  are ours — a URL in parts, and an origin other code compares.
- **An opaque origin is the same as itself and nothing else.** A `data:` URL, a
  local file, a scheme nobody registered: each gets its own identity, and two of
  them are never the same origin however alike they look. Getting that backwards
  is a same-origin bypass, so it is a type with an identity in it rather than a
  convention somebody has to remember. `file:` is opaque for the same reason
  every modern browser made it so — one local file reading every other one is
  the oldest exfiltration bug there is.
- **Unknown means opaque, never "probably fine".** A scheme this engine has not
  been told about inherits nobody's privileges.
- The first item under stage 2's new hostile-input rule: a test feeds the parser
  empty strings, hundred-thousand-character hosts, a thousand colons inside
  IPv6 brackets, right-to-left overrides and null bytes, and requires an answer
  rather than a panic. In a renderer a crash is a denial of service.

- **The loop has rules for stage 2, and stage 2 has a queue.** Stage 1 asked one
  question — *does alo render?* — and answered it with a committed PNG and a
  committed box tree. Stage 2 has neither, so `docs/autonomy/LOOP.md` says what
  replaces them: **a real page decides, and the page is frozen** (a suite that
  fetched would be flaky, would fail on an aeroplane, and would let any site's
  owner break our build); **the bytes are hostile now**, so anything that parses
  them gets a malformed-input test and must return an error rather than panic;
  **order follows dependencies** rather than the file; and **a decision gets an
  ADR as its own iteration**.
- `docs/autonomy/QUEUE.md` gains stage 2 as **86 items** in ten groups, from the
  roadmap's own ninety lines — each with what it depends on and what closes it.
  Eleven are marked as needing an ADR before any code depends on them, and the
  handful that genuinely need hardware to verify say so rather than being
  discovered later.
- Numbering starts at 50. The sixteen coarse items sketched as 26 to 41, before
  the roadmap grew the real list, are **retired rather than reused**: two items
  with one number is how a reference quietly starts pointing at the wrong work.

- **An agent reads alo's Settings screen and acts on it by name** — the last
  clause of stage 1's exit gate. It finds the sections as buttons called what a
  person would call them, knows which one is open, activates one by name with no
  coordinate anywhere in the transaction, ticks the out-of-office box and types
  a date, and is refused when it asks for something the screen does not have.
- **`aria-current` is read.** ADR 0002's own example sentence is "invoice list,
  twelve rows, row three selected"; for this screen it is "which section is
  open", and an agent that could not read it would have to guess from a colour.
  It is a word rather than a flag — `page`, `step`, `date` — because a nav item
  being the current *page* is not the same claim as a cell being the current
  *date*.
- Honest about the edge: pressing a nav row runs the page's own code, and there
  is none in stage 1. The verb finds the row and reports what it pressed, and
  the screen does not change. There is a test that says exactly that, so the
  day scripting arrives it will fail and have to be rewritten on purpose.

- **alo's Settings screen renders**, from its own markup and its own rules, with
  no substitutions — the second of the two screens stage 1's exit gate names.
  Corpus case `alo-settings`.
- **A form control holds what it shows in a box of its own**, the way browsers
  do. That is why a tall button's label sits in the middle of it and why an
  empty field is still one line tall — and it replaces two approximations in the
  user-agent style sheet that the Settings screen walked straight into:
  - The sheet centred a button's label with `justify-content`. An author who
    made a button a flex container — which alo's settings nav does — could not
    override a rule they could not see, so every nav item was centred. **An
    author cannot override the user-agent sheet's flex alignment**, which is why
    this could never have been a rule.
  - The sheet gave every `<input>` a fixed `height: 1.2em`, so that an empty one
    was not a hairline. Once a field showed its value that height was too
    *short* for it, and the text hung out of the box. A minimum, from the box
    the control holds its text in, is right either way.

- **alo's sign-in screen renders from its own stylesheet, with no
  substitutions.** The last of the four was `transition`, `:hover` and
  `:focus-visible`, and removing it needed no new code — the engine already
  read all three and already made an interaction state match nothing. What was
  owed was finding that out and putting the rules back, which is why the item
  existed: a substitution nobody re-checks outlives the reason for it.
- That the rules change nothing is the **correct** answer rather than a missing
  one. A still picture of a settled page is what a transition has finished
  doing, and nothing is hovered or focused because there is no pointer and no
  keyboard. Two tests now pin it, so a later change that started dropping those
  rules would say so.

- **`letter-spacing`.** The third of the four substitutions in alo's own
  sign-in case is gone: the headline's `-0.02em` is the screen's own value now,
  and with it "Your servers." stops wrapping — four lines instead of five.
- It is applied **where the text is measured**, not where it is drawn. Spacing
  changes what a run is worth, so it changes where every line breaks; a version
  that only moved the pen at paint time would have drawn different text from the
  one the line was made of.
- Shaping is untouched. `alo_text::spaced` adds the room after every glyph
  afterwards, so the `rustybuzz` boundary stays exactly where it was — shaping
  is the rented part, and letter spacing is a CSS decision about the result.
- The test font honours it too. A fake that ignored it would have let a layout
  test pass without the spacing ever reaching the measurer.

- **`white-space`, and the whitespace processing that was never done at all.**
  Markup is written for people, so it is full of whitespace nobody meant to see
  — and until now the engine shaped whatever bytes the parser handed over, so
  `one   two` was three spaces on the screen and an indented paragraph was drawn
  with its indentation in it. Runs of whitespace now become one space.
- **`pre-line` keeps the newlines**, which removes the second of the four
  substitutions in alo's own sign-in case: the headline is one string with
  newlines in it, the way `alo-workplace` writes it, instead of three `<span>`s
  made blocks.
- `pre`, `pre-wrap` and `nowrap` too — and `<pre>` actually preserves its
  whitespace now. The user-agent sheet had said `pre { white-space: pre }` since
  it was written and nothing had ever read it.
- Collapsing happens **when the box is built**, so layout, paint and the agent
  tree all read the same text. Where a line may *break* stays the line builder's
  — a kept newline is a break that must happen, and `nowrap` forbids the ones
  that may. Two questions, and they are answered in two places on purpose.

- **`clamp()`, `min()`, `max()` and the viewport units.** One of the four
  substitutions standing in alo's own sign-in case is gone: the headline's
  `font-size: clamp(2.4rem, 4vw, 3.5rem)` is the screen's own value now instead
  of the `2.5rem` somebody worked out by hand. **The committed reference render
  did not change**, which is the best evidence the substitution was faithful and
  the implementation is right.
- The four are one family and are parsed as one, so they nest:
  `clamp(1rem, min(4vw, 30px), 5rem)` is a value. Each is type-checked once,
  when it is parsed — the smaller of a length and a number has no answer, and is
  refused rather than guessed at, exactly as `calc()` already was.
- `clamp(a, b, c)` is `max(a, min(b, c))`, so **when the bounds cross the lower
  one wins**. That is CSS's rule and not Rust's `clamp`, which refuses a
  reversed range.
- **A viewport unit needs a window, and says so when there is none.**
  `FontMetrics` carries an `Option<Viewport>`; without one, `4vw` is zero rather
  than a plausible-looking number nobody could trace. The cascade supplies it
  from the media context, including while resolving `font-size` itself — which
  is where the headline needed it.
- `MediaContext` has a height. No media query this engine evaluates asks for it
  yet; `vh` does, and a window with a width and no height is not a window.

- **A verb changes the page.** Putting text into a field puts it there; ticking
  a checkbox ticks it; choosing a radio un-chooses the rest of its group. Until
  now a verb decided, reported, and changed nothing — the half of "typed verbs"
  that makes an agent able to *drive* an interface rather than describe what it
  would do.
- **Deciding and changing are two steps, and the types say so.** The agent tree
  borrows the document, so nothing holding one can change it — which is right:
  the decision has to be made against the tree the agent read, and the change
  applied to the document afterwards. `alo_agent::apply` is the second half.
- **The page is rendered again from the same document, never re-parsed.** A
  re-parse would mint new node ids on every keystroke and silently invalidate
  every snapshot anybody was holding. ADR 0003's promise — an id names the node
  it named or nothing — is the thing that had to survive, and there is a test
  that holds an id across a change and uses it.
- **A field shows what it holds.** An `<input>`'s value becomes a text box
  nobody wrote, so a typed value is laid out and drawn rather than only being in
  the tree. A password shows one dot a character and never what it holds.
- **A `<label>`'s words are no longer read twice.** They were exposed as loose
  text *and* as the control's name, so every labelled field on every form
  answered to its own name ambiguously — and an agent's verbs then had to refuse
  it. alo's own sign-in screen read "Email" twice; now it reads it once.
- **A password field is findable.** ARIA gives `<input type=password>` no role
  on purpose, so that a screen reader does not read a password back — and a
  browser that then left it out of the tree would have an agent that cannot sign
  in to anything. Role says what a thing *is*; a new `takes_text` capability
  says what can be done to it.
- Attributes can be changed on a document (`alo_dom::Document::set_attribute`).
  Adding and removing nodes is still the parser's alone, and arrives with the
  DOM APIs.

- **The engine is behind a message boundary** (`alo-renderer`, ADR 0005). A
  renderer is *sent* work and *returns* results: one direction, no callback in
  the signature, nowhere to wait, and nothing ambient — everything it needs
  arrives in a message or in its constructor.
- Every message is **owned, `Clone` and `Send + 'static`**, asserted by a test.
  That is the property that makes a transport possible later without changing a
  caller; choosing the transport is queue item 29's, and inventing a wire format
  before there is a process to send it to would be inventing.
- What crosses is a **frame** (finished bytes, the one thing ADR 0005 lets
  processes share) and a **snapshot** of the agent tree. A borrow cannot cross a
  process, so a snapshot is a copy — and it is safe to act on a moment later
  because it carries node identity, which ADR 0003 never reuses. A test pins
  that the snapshot reads *exactly* as the tree it came from, because a
  description that drifted would be the second structure ADR 0002 forbids.
- The pipeline moved out of the corpus and into the renderer, so there is one of
  it. The corpus still reaches inside for the trees it asserts on: it is a test
  of the engine's insides, and ADR 0005 says tests stay single-process.
- **A claim was corrected.** `docs/conformance.md` said an agent can "act on"
  a page. A verb finds its target, refuses what cannot be operated, and reports
  what it decided — and then nothing happens, because the document is never
  written back to. Trying to use the boundary end to end is what found it. The
  docs now say so and queue item 42 is where it stops being true.

- **Stage 1 needs no hardware and no operating system, and the roadmap now says
  so.** The exit gate asked for alo OS's screens to render *on the certified
  machine, without stutter* — a compositor that does not exist, hardware nobody
  has, and a speed claim measurable on neither. It had already begun doing
  damage: a finished capability was held open, three documents recorded "not
  met" for a reason that was about where a repository sits on a disk, and the
  build had moved on to stage 2 with stage 1 unfinished. The gate now names
  **alo's** screens — an alo screen is alo's whichever repository it lives in,
  and alo's are checked out here — and asks only for what this repository can
  produce and check on any laptop: a reference image and an expected box tree,
  so "correctly" is a diff rather than an opinion. Hardware acceleration,
  embedding into alo OS's shell and rendering without stutter are still real
  work, moved out of the stage 1 list into a section that cannot block it.
  Stage 1's genuine remainder is back in the queue as engine work: four
  substitutions still standing in for things the engine does not implement, the
  Settings screen, and an agent reading it by name.

- **ADR 0005: one process per site, and a sandbox we rent.** Stage 2's first
  roadmap item is a decision rather than code, and this is it — what runs where,
  which way the boundary points, and what happens when a renderer dies.
- It answers the question a memory-safe engine has to answer first: **why does
  Rust not make this unnecessary?** Spectre is a hardware property that no
  language prevents and only site isolation mitigates; the codecs and the TLS we
  rent have `unsafe` in them and are not ours to make safe; the same-origin
  policy is code we write and can get wrong; and a page must not be able to end
  the session. Three of those four would apply to a perfect engine.
- The expensive half is the **shape**, not the `fork`: a renderer never calls
  back synchronously, nothing is shared but a read-only frame, and every message
  is typed. So the boundary gets built while everything is still one process
  (queue item 25) and the split is a change of transport (item 29).

- **An agent reads a broken link as one link, whole.** Layout breaks an inline
  box holding a block into a piece on each side, with the block a *sibling* of
  the pieces — that is where layout needs them and it is not what the document
  says. The agent tree now reads the pieces and the blocks between them as one
  thing: one node, named by everything the element contains, positioned
  everywhere it was drawn, with the block read *inside* it rather than beside
  it.
- Still a **view**, not a second structure (ADR 0002). The box tree records
  which boxes belong to which whole — `broke_out_of`, `continued_from` — and
  the reader follows what is already there.
- **A name gets a space where a block begins or ends.** `<a>Read the<div>the
  manual</div></a>` was called "Read thethe manual". A block-level box is a
  line of its own on the screen, and a name read out has to sound like what a
  person sees. The sign-in screen's headline changed with it, from "Your
  workspace.Your servers.Your rules." to the three sentences it is.

- **An empty piece of a broken inline is kept, and draws its border.** CSS says
  an inline box holding a block is broken into a piece on each side "even if
  either side is empty", because an empty inline with a border still draws one.
  This engine used to drop it.
- **A line box that holds nothing worth holding does not exist.** CSS's rule:
  a line with no text, no preserved space and no inline box with a margin,
  padding or border is zero-height and treated as not existing. That is what
  makes keeping the empty piece free — it costs a line only when it has
  something to draw.
- **An agent still reads one thing.** Of the pieces of a broken inline, the one
  that is read is the first with anything in it; the rest are read through. A
  border is not something to read, so an empty piece is never a second link.

- **An inline box has a box of its own.** A `<span>`'s border and padding are
  laid out and drawn: the horizontal ones take room on the line, once at its
  start and once at its end; the vertical ones draw without changing the height
  of the line, which is CSS's rule and what stops a padded `<em>` pushing a
  paragraph's lines apart.
- **A `<span>` that wraps is one rectangle per line**, not one rectangle with
  the gap between the lines painted over. Its background used to be drawn from
  the union of its pieces, which ran straight across that gap.
- **A broken box's border stops and starts.** The start border is drawn only on
  its first piece and the end border only on its last, the way a browser draws
  a wrapped `<a>` — a piece in the middle has neither.
- A piece ends at its **content**, not at the pen: a line that ends in a space
  has advanced past the last glyph, and a background painted to the pen ran out
  past the end of the text.
- Refused rather than guessed at, and recorded: a **percentage** padding on an
  inline box, which is of the containing block's width and is not known where
  the line is built.

- **`calc()` with a percentage in it works, in every layout property that takes
  one.** `width: calc(100% - 2rem)` — the thing a design system writes for a
  full-width box with a gutter — was refused and recorded. It is now a number:
  widths and heights, minimums and maximums, margins, padding, insets, gaps and
  grid tracks.
- **The layout tree is ours now; the layout algorithms are still rented.**
  ADR 0004. `taffy` asks the *tree* to resolve a `calc()` against a basis only
  the running algorithm knows, and its ready-made tree answers zero with no
  hook to replace it — so this engine keeps its own arena of nodes and
  implements `taffy`'s tree traits over it. Flexbox, grid and block sizing are
  untouched: those are the physics ADR 0001 says to rent.
- The handle `taffy` carries for an unresolved expression is an **index**, not
  a pointer, so there is no `unsafe` anywhere near it and a handle from another
  arena resolves to nothing rather than to somebody else's expression.
- Rounding is now impossible rather than switched off: `taffy`'s rounding is a
  pass over a trait this tree does not implement.
- Still refused, and still recorded: a `calc()` **inside `fit-content()`**,
  which the algorithms have no spelling for.

- **An inline box holding a block is broken around it, the way CSS says.** It
  used to be treated as a block container, which looks nearly the same and is
  not the same: the difference shows in where a background stops. A highlighted
  phrase interrupted by a block now stops before the block and starts again
  after it, because each piece is a box of its own.
- The block becomes a **sibling** of the anonymous blocks the pieces sit in —
  which is why this could not be done by rearranging children in place, and why
  breaking the inline hands several boxes back to its parent instead of one.
- **An agent still reads one link.** The pieces of a broken inline come from
  one element; the agent tree reads the first and reads the later ones through,
  so a verb is never handed two things with the same name to choose between.
- Two gaps the change found, both recorded rather than quietly left: an *empty*
  piece is dropped where CSS keeps it, and an inline box's own border and
  padding are still neither laid out nor drawn.

- **`transform` and `opacity`.** `translate`, `scale`, `rotate`, `skew` and
  `matrix`, about a `transform-origin` that defaults to the middle of the box;
  `opacity` as a number or a percentage. A rotated box carries everything
  inside it round with it, and a rotated letter is a real outline rather than a
  stretched picture of one.
- **`opacity` is a group, not a number applied to each box.** The subtree is
  drawn on a surface of its own and composited back once. Fading box by box
  would show every box in a group through every other one — two black squares
  on top of one another at half opacity would come out three quarters dark
  instead of a mid grey.
- **A gradient under a transform asks where the pixel came from.** The pixel is
  mapped back through the inverse of the transform before the gradient is asked
  what colour it is, so a turned box's gradient turns with it rather than
  staying pinned to the page.
- **Paint order follows stacking contexts now.** A positioned box is painted
  last in the stacking context it *belongs to*, which may be several ancestors
  up — so a positioned box inside a transformed one is painted inside that
  transform rather than over the whole page. A negative `z-index` goes behind
  its parent's content and in front of its background, which is the part of
  stacking that surprises people.
- **A transform moves what is drawn, not what is laid out.** That is what CSS
  says, and it is what lets an agent keep reading positions out of the layout
  tree while the page is animating.
- **Refused rather than approximated**: anything with a third dimension in it —
  `rotate3d`, `matrix3d`, `perspective`, `translateZ`. A value containing one is
  refused whole rather than half applied, because half a transform puts a box
  somewhere nobody asked for.

- **A box can cast a shadow and be filled with a gradient.** `box-shadow`
  (offset, blur, spread, colour, and `inset`), `text-shadow`,
  `linear-gradient` and `radial-gradient`. A page stops being flat colour in
  flat shapes.
- **A shadow is coverage, blurred — not a picture, blurred.** The shape is
  rasterised to a mask, the mask is softened by three box-blur passes, and the
  colour arrives afterwards, so what is *behind* the shadow is not blurred
  along with it. An inset shadow is the same blur run on the shape with a hole
  in it, which is why there is one blur rather than two.
- **A run of text casts one shadow, not one per letter.** The whole run is
  outlined into a single shape before it is blurred: two letters that touch
  would otherwise be blurred separately and composited, and the overlap would
  be visibly darker.
- **Refused rather than approximated**: `conic-gradient`, the repeating
  gradients, colour interpolation hints, and interpolation in any space but
  sRGB. Each is a different curve through colour, and drawing one as another is
  a wrong pixel that looks nearly right.
- Paint was three files' worth of work in two files. `Coverage` moved out of
  the rasteriser — more than one thing makes coverage now — and building a
  display list moved out of the display list itself, because a new CSS property
  and a new kind of drawing are different reasons to change.

- **alo's sign-in screen renders.** The real markup, the real rules and the
  real design tokens, drawn by this engine and diffed on every run: the
  charcoal brand panel, the headline, the fields, the terracotta button, the
  divider. It is `alo-workplace`'s sign-in screen rather than `alo-os`'s,
  because that repository is not checked out here — so stage 1's exit gate is
  **not** met, and `docs/conformance.md` says so.
- **A real screen found four bugs, which is what real screens are for.** Text
  that was a flex item was never drawn at all; a box rounded down to 96 pixels
  wrapped text that measured 96.16, so "Remember me" became two lines in a box
  wide enough for one; `border: 1px solid` was not read, because only the
  longhands were; and an empty `<input>` laid out at nothing by nothing.
- Layout is sub-pixel throughout now. It rounded to whole pixels and measured
  unrounded, which is a disagreement that shows up as a word on the wrong line.
- `text-align` works, and a button's label sits in the middle of it.

- **★ An agent can act on the interface, and no verb takes a coordinate.**
  Activate, put text, scroll — each aimed by a *description* rather than a
  position: "the Save button", "the row called Invoice 12". A description
  survives the page moving; a point does not.
- **Refusing is a result.** Two matter most: two things called the same name is
  refused rather than guessed at, with both of them named so the caller can
  narrow the request — acting on the wrong row is worse than acting on none —
  and a control that says it is disabled is not operated, even though nothing
  physically prevents it.
- Every outcome is a record of what was asked for and what happened, which is
  the guarantee a screenshot-and-guess agent cannot make.
- **The gate now checks the no-coordinate rule.** It was a rule a person had to
  remember; a function in the agent surface that takes a point or an `x` and a
  `y` now fails the run.

- **★ An agent can read the interface as what it is.** "Invoice list, twelve
  rows, row three selected" — the sentence `docs/decisions/0002` opens with —
  is now a question this engine answers about a page it rendered, by role and
  by name, with no screenshot anywhere in the chain and no second tree to
  disagree with the first.
- It is a **view**, not a structure. Nothing is built: a node is a box's id and
  a borrow of the trees that already draw the page, and every question is
  answered from them when it is asked. If the two could disagree, an agent
  would eventually act on something that is not on screen.
- **The same view is the accessibility tree.** A screen reader and an agent
  want identical facts, and two implementations would guarantee one is wrong.
- A `<div>` that means nothing is read through, exactly as a screen reader does
  — a page is mostly `<div>`s, and a tree that showed them would bury the rows
  an agent is looking for. What the author hid with `aria-hidden` is not read
  at all.
- **A node knows whether it is on screen.** That is the thing a DOM cannot say:
  a scrolled-away row looks identical to a visible one in it.
- Every corpus case now pins what an agent reads beside what a person sees, so
  the two cannot drift apart without a test noticing.
- The agent tree found a layout bug on its first run: a space that was its own
  text box took no room, so `All` and `Due` touched, and `small and` read
  `smalland`.

- **There is a reference corpus.** Six cases — an invoice list, a rounded card,
  wrapping prose, a flex row, a grid, and three font sizes on one line — each a
  directory holding what to render and four expectations: the box tree it
  builds, where every box ends up, what is drawn, and what it looks like. A
  fifth file records everything the engine refused, so a case that renders
  oddly says why.
- **A failure names the case, the expectation and the line.** The corpus
  reports every case that differs and every expectation inside it, all at once,
  rather than the first one and then stopping — because a change usually shows
  up in more than one of them and finding that out should not take four runs.
- The expectations are **files**, so a change is a diff a person reads rather
  than a failure they have to reproduce.
- The corpus found its first bug on the day it was written: a box with rounded
  corners and no border was pushing a clip for the border it did not have.

- **A box can be round.** `border-radius` changes what shape a box is, and
  `overflow: hidden` clips what is inside it to that same shape — one question
  asked twice. A card with rounded corners now clips its banner to them, which
  is the second reference render.
- A border of one width and colour all the way round is drawn as a **ring** —
  the box's shape with the box's shape inside it, wound the other way — so it
  follows the corners. Four rectangles would have square corners over a rounded
  background, which is exactly what the first attempt looked like.
- Radii that ask for more room than an edge has are scaled down together rather
  than clamped one at a time, which is what CSS says and what keeps a shape's
  proportions instead of making one side rounder than another.

- **The engine draws.** A laid-out document becomes a display list, the list
  becomes pixels, and the pixels become a PNG — and the first reference render
  is committed: a list of invoices, with a heading, three rows, separators and
  a selected row highlighted, drawn from HTML and CSS with real fonts.
- **A picture that changes says what changed.** The display list is compared
  first, in words — "fill box#8 rgb(228 228 231) at (8, 64) 184×25" — and then
  the pixels. A failure reads "the row's background moved four pixels" rather
  than "the image differs", which is the whole reason there is a list in the
  middle.
- **Text is now measured at the size it is set in.** It was measured at one
  size for the whole document, so a heading and a caption were laid out as
  though they were the same — caught by the first picture, which is what a
  picture is for.
- Paint order is decided once, in the display list: backgrounds and borders
  before content, parents before children, and positioned boxes after
  everything in the flow, ordered by `z-index` and then by which came first.

- **A letter has a shape.** A font's outlines are read, scaled and turned into
  coverage — how much of each pixel a letter covers, anti-aliased. An `l` is a
  vertical bar, an `H` has a gap at the top and none in the middle, and a space
  covers nothing; those are the assertions, because a mask is better checked by
  saying what shape it is than by committing a picture of it.
- Everything this engine draws is one shape type, so a glyph and the box behind
  it come out of the same rasteriser with the same anti-aliasing — which is
  what stops them disagreeing along the edge they share.
- Coverage is not colour. A mask says how much of each pixel is covered and
  nothing about what colour it is, which is why the same mask serves black text
  on white and white text on black, and can be reused for a shadow rather than
  rasterised twice.
- The Y axis turns over in one place: a font measures up from the baseline and
  a screen measures down from the top, and the single minus sign that reconciles
  them lives at the boundary so that it lives nowhere else.

- **Colours are channels.** Hex in every length CSS allows, the named colours,
  `rgb()` and `hsl()` in both the modern and the legacy form, `transparent`,
  and `currentColor` — which is carried as itself until there is an element to
  ask, because it is the initial value of every border and folding it into
  black at parse time would draw them all wrong.
- Channels are floats rather than bytes, because compositing multiplies and
  adds them and doing that in eight bits loses a little every time — which
  shows up as banding in exactly the gradients a design system uses. Bytes come
  back at the very end.
- `oklch`, `lab`, `color()` and `color-mix()` are refused rather than
  approximated: they are a different colour space, and a colour converted by
  guesswork is a wrong pixel that looks nearly right.

- **Text and boxes share a line properly.** A sentence spread over several
  `<span>`s is one sentence and wraps between any two of its words, not only
  around the spans; everything on a line sits on one baseline, so a tall image
  beside small text pushes the line down rather than the text up; and a link
  broken across two lines is **two rectangles**, not one covering the gap
  between them. None of those is expressible as a row of boxes, which is what
  the stand-in was.
- An `inline-block`, an image or a button on a line is laid out on its own and
  placed whole — the same layout code, called again, one formatting context
  down.
- Layout now reports the pieces a box was drawn in as well as where it is.
  Where it *is* is the union of the pieces; what should be **drawn** is the
  pieces, and a background painted from the union would cross the gap between
  two lines.

- **The engine works out how wide text is.** Fonts load, `rustybuzz` shapes
  them, UAX #14 says where a line may break and we decide where it does, and
  layout's measuring seam is filled — so a paragraph in a narrow window now
  takes more lines than the same paragraph in a wide one, with numbers that
  came from a font rather than from a guess.
- **The awkward scripts work, and they were done first.** Arabic joins and runs
  right to left; `e` followed by a combining acute is one glyph, not two.
  Nothing in the pipeline is indexed by character — a glyph names the *byte
  range* it came from, several glyphs can name the same range, and one glyph
  can cover several characters. A pipeline that assumed otherwise is one that
  gets rewritten, so it never assumed it.
- **A font is asked whether it has a character**, never guessed at from a
  language tag, and a character no font has takes no room rather than being
  drawn as something it is not.
- Breaking is UAX #14 rather than "split on spaces", which would have been
  wrong for most of the world's writing — Thai has no spaces and a hyphen is a
  break that is not one. A word wider than its line overflows rather than being
  cut in half.
- `rustybuzz` and `unicode-linebreak` each stay behind one file, and the gate
  checks it.

- **Boxes have positions and sizes.** Block, flexbox and grid, with the box
  model, positioning and overflow — every number a CSS pixel, asserted as a
  number. The whole layout of a small interface is written out in a test, so a
  change that moves a box says which box and by how much.
- Layout runs on `taffy`, which ADR 0001 calls a judgement call rather than
  physics: a real chunk of engine, taken because it gets us laying out sooner
  and meant to be replaceable. It is named in exactly one file, and the gate
  checks that on every run.
- **How wide a piece of text is, is asked rather than assumed.** Layout takes a
  measurer and there is deliberately no default: a built-in guess would be a
  wrong number every layout quietly depended on. Real text is the next item.
- Two limits are named rather than hidden: there is no inline formatting yet
  (a run of inline boxes is laid out as a wrapping row, which gets them side by
  side and not their baselines), and a `calc()` mixing percentages in a layout
  property is refused and recorded rather than becoming something else.
- **A margin now starts at zero.** It started at `auto` for one draft, which
  silently centred every box in every document — a layout that looks deliberate
  and is not.

- **Lengths are numbers.** `16px` was four characters; now it is sixteen. Every
  unit CSS has that does not need a window to answer, `calc()` type-checked
  once and evaluated whenever a caller has a font and a basis, and `em` and
  `rem` resolved against the font size actually in force — which is why this
  layer had to wait for the cascade rather than come before it.
- `calc(1px + 2)` is refused when it is read, rather than producing three of
  something. A length may be added to a length and multiplied by a number;
  anything else is somebody's mistake and is reported as one.
- A percentage is carried rather than resolved, because `50%` is half of
  *something* and which something depends on the property and the containing
  block. Only layout knows, so only layout is asked.
- **`font-size` and `line-height` now inherit as computed values**, which is
  what CSS says and what a first draft got wrong: a child that inherited the
  text `2em` resolved it again against its own font, so `2em` inside `2em` was
  four times the grandparent rather than twice the parent. A `line-height`
  written as a number stays a number, so a child with a larger font still gets
  a proportionally larger line.

- **There is a box tree**, and every box in it knows what it *is*. ADR 0002
  says the layout tree is the agent's tree, so a box carries its role, its
  state and what it is called from the moment it is made — "invoice list,
  twelve rows, row three selected" is a thing this engine can now answer, on a
  document, without a screenshot and without a plugin.
- **Roles are declared, never inferred.** They come from the `role` attribute
  or from what HTML says the element is; a role this engine does not know is
  kept as written rather than dropped. There is no third source, and in
  particular there is no "it has a border and some rows, so it is probably a
  table".
- Box generation follows the tree rather than the markup: `display: none`
  removes a subtree, `display: contents` removes one box and keeps its
  children, and a container whose children are a mix of block and inline grows
  the anonymous boxes that make them one kind. The whitespace between two
  paragraphs makes no box; the space between two links does, because it is the
  gap between two words.
- **The engine has its own style sheet** — what an element looks like before
  anybody says otherwise. It is CSS text that goes through exactly the same
  parser and cascade as anything an author writes, because a second path
  through the cascade would be a second cascade to be wrong.
- Not one number yet. Where a box ends up is the next item, and keeping the two
  apart is what stops a box's meaning depending on where it landed.

- **The cascade works, and so does `var()`.** For every element of a document:
  which declaration wins — origin, then `!important`, then specificity, then
  order — what a child inherits when nothing wins for it, and what a custom
  property actually reads. `docs/decisions/0001` calls this stage 1's first
  hard requirement rather than decoration, because alo's design system is
  custom properties throughout: an engine that cannot resolve them renders
  nothing of alo at all, not badly, nothing.
- **A variable cycle is refused rather than looped.** `--a: var(--b)` with
  `--b: var(--a)` makes every property in the ring invalid, which is what CSS
  says and the only answer that terminates. A `var()` naming something that is
  not set falls back if it can and is recorded if it cannot.
- The text around a substitution survives exactly as written, so
  `calc(var(--gap) * 2)` becomes `calc(8px * 2)` — a value this engine does not
  yet understand is still one it can pass along intact.
- `inherit`, `initial`, `unset` and `revert` all work, including the one that
  is easy to get wrong: `revert` steps a whole origin aside, both its ordinary
  and its important declarations, rather than taking second place.

- **Style sheets parse into rules we hold.** `cssparser` tokenises and
  `selectors` matches; what a rule is, which selectors exist, and what happens
  to what we do not implement are ours. A sheet in the shape alo's design
  system is written in — custom properties throughout, a light and a dark
  theme behind `prefers-color-scheme`, a width breakpoint — parses whole, and
  an engine can now ask which rules apply to which element of a document.
- **Nothing is dropped in silence.** An unknown property is kept with its
  value, so a later stage can implement it without re-parsing the sheet; an
  at-rule we do not implement is kept whole for the same reason; a selector we
  cannot evaluate takes its rule down, which is what CSS says, and says so.
  Every one of those is reported with the text that caused it and the line it
  was on.
- **Media queries: width and `prefers-color-scheme`.** A condition we do not
  understand is treated as not matching rather than guessed at — the
  alternative is a dark theme leaking into a light one, which looks like a
  rendering bug and is not one.
- **`:hover` and `:focus` parse and never match**, because nobody is hovering
  a document being rendered to a file, and **`:visited` never matches at all**:
  whether a link has been followed is history, and a style that depends on it
  is readable back off the page. `:disabled`, `:checked`, `:required` and
  `:read-only` do match, from the markup, including the awkward parts — a
  disabled `<fieldset>` reaches its controls but not the ones in its first
  `<legend>`.

- **There is a document tree.** `html5ever` parses HTML and alo browser holds
  the result: nodes, attributes, parents and children, in our own types rather
  than the parser's. A document round trips — parse it, write it back out, and
  the text is the same — and malformed input still produces a usable tree, with
  a record of everything the parser had to repair to get one.
- **Every node has an identity that is never reused** (ADR 0003). An id names a
  node for the life of its document, and a node that leaves the tree keeps it,
  so a stale id names something that is gone rather than something else that is
  not. This exists in the first commit of the tree because the agent surface
  will stand on it, and identity cannot be retrofitted.
- **Quirks mode is recorded and never honoured.** A document that would put
  another engine into quirks mode is laid out as standards, because law 1
  refuses quirks mode outright. What the parser observed is kept for
  diagnostics rather than thrown away.
- `unsafe` is now forbidden by the compiler rather than by review, across the
  whole workspace.
- **The gate is a command that fails**, not a paragraph to remember:
  `scripts/gate.sh`. It runs the formatter, the linter and the tests, refuses a
  stub, refuses a crate that has quietly opted out of the ban on `unsafe`, and
  checks that each rented crate is still named in only the one file that is
  allowed to name it. What it cannot check, it names, so that a green run is
  never mistaken for the whole gate.
- The scope is written down: what gets built, in which stage, and what will not
  be built at all. The engine renders the modern platform and refuses thirty
  years of legacy — no quirks mode, no floats-as-layout, no CSS-table layout —
  because refusing them is what makes a project this size survivable. Stage 1
  needs no JavaScript engine, which removes a browser's largest single component
  from the critical path.
- alo browser exists as a decision and a queue. Nothing renders yet.

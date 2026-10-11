# ADR 0051 — A form is checked before anybody is asked, and its submission is a navigation the browser process encodes

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 82, *forms: the controls, constraint validation,
submission, file inputs*. The owner moved it from `needs design` to
`needs ADR` on 2026-10-11 (commit 01596ea), on the ground that stage 2
closes on *a person using alo for a week*, which no amount of the CSS tail
reaches without forms. Other decisions it builds on:

- ADR 0002: the layout tree is the agent's tree, and no verb takes a
  coordinate.
- ADR 0005: a renderer has no filesystem and no network, and never calls
  back.
- ADR 0012 § 4: the browser process assigns the cause from which message
  was answered.
- ADR 0013 § 3: absent beats approximate.
- ADR 0017 § 5 and ADR 0018 § 6: a rule about the document lives once in
  `alo-dom`. ADR 0018 § 6 also names a submit button's activation
  behaviour as item 82's.
- ADR 0020: a page's navigation is a claim in an answer, the last one per
  answer, decided by the browser process. Its *What this does not decide*
  says *a submission is a navigation with a body, and item 82's; it will
  take this ask's shape, not a second one*.
- ADR 0026: a person's grant, which no agent can make.
- ADR 0029: the regular-expression engine is ours.
- ADR 0032: a page's fetch is an ask, and `FormData` and `URLSearchParams`
  are their own items.
- ADR 0042: a date is put in as a valid date string.

The pages are frozen corpus cases. `alo-sites-booking` is alo Sites' booking
form: `method="get"`, one `required` date field, and no submit listener.
`alo-sites-contact-form` is alo Sites' enquiry form: `method="post"`,
`required` and `maxlength` fields, `type="email"`, and a submit listener that
cancels and fetches with `new URLSearchParams(new FormData(form))`, falling
back to `form.submit()`. `web-a-form` is the HTML specification's own
example form, served by httpbin: `method="post"`, radios, checkboxes, a
`time` field with `min`, `max` and `step`, and a button with no type. alo's
own React screens in `alo-workplace` all cancel `submit` and handle it in
script. They rely on native `required` and `type="email"` checks running
first, and two of them (`NewTaskModal.tsx`, `ComposeModal.tsx`) carry
`type="file"`.

The code this is about:

- `alo-dom`: `activation.rs` (`Activation::Button` does nothing, the
  `checked` attribute is the checkedness, and a radio group ignores form
  owners) and `field.rs` (the `value` attribute is the text).
- `alo-agent`: `verb.rs` (`Activate`, `PutText`, `Scroll`, and the outcomes
  and refusals).
- `alo-renderer`: `ask.rs` (`Asked` has no method and no body) and
  `navigate.rs` (`Navigation` and `decide`, GET only).
- `alo-net`: `request.rs` (`Request` already carries a method and a body,
  documented as *`GET` until there is a form to submit*).

## Measured

Measured in Chrome 154, headless, over the DevTools protocol, on a probe page
and on the two frozen pages served at their own addresses with their frozen
bytes. Each request was intercepted and failed, so nothing reached a network.
Chrome's source was not read.

- **Pressing a submit button with invalid fields fires `invalid` at each
  invalid control, in tree order, and nothing else.** No `submit` is fired
  and nothing is requested. Each `invalid` is trusted, cancelable and does
  not bubble. On `alo-sites-booking` with no date chosen, the date field's
  `validationMessage` is `Please fill out this field.`
- **With valid fields, `submit` is fired at the form.** It is a trusted
  `SubmitEvent` that bubbles, is cancelable and names the pressed button as
  its `submitter`. It is trusted even when the press was a script's
  untrusted `click()`, because the browser's own algorithm fires it. After it
  comes `formdata`, a `FormDataEvent`, then the request.
- **`web-a-form`, untouched, posts
  `custname=&custtel=&custemail=&delivery=&comments=`** to
  `https://httpbin.org/post` as `application/x-www-form-urlencoded`, with no
  charset parameter. With `Ada Lovelace` typed into its first field, it
  posts `custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments=`.
  Unchecked boxes and radios send nothing, and its button has no `name`, so
  it sends nothing either.
- **`alo-sites-booking` with `2026-10-20` chosen requests `GET
  https://nordwind.alosites.com/b/studio-consultation?date=2026-10-20`.**
- **Encoding.** `hello world&=é` is written `hello+world%26%3D%C3%A9`, and a
  textarea's newline is written `%0D%0A`. A disabled control sends nothing,
  a hidden one sends its value, a chosen `<option>` its `value`, and a file
  field with nothing chosen sends `name=` and nothing after it. The
  submitter is sent only when it was the one pressed.
- **A GET's query replaces the action's.** An action of
  `https://example.test/get?q=9` is requested as
  `https://example.test/get?a=…`.
- **`form.submit()` checks nothing and fires no `submit`.** It posts the
  invalid values as they stand. **`requestSubmit()`** checks, as a press
  does, and fires `submit`.
- **Several submissions in one task go as the last one.** `submit()` then a
  button's `click()` requests the button's GET. The click then `submit()`
  requests the POST.
- **A `submit` listener's `preventDefault` stops the submission, and the
  script after the `click()` carries on.**
- **`novalidate` submits invalid values. `setCustomValidity('mine')` makes
  a control invalid** with `mine` as its message.
- **A `maxlength` is not checked against a value a script set**:
  `tooLong` is `false` for a six-character value under `maxLength = 3`.
- **`reset()` fires a trusted, cancelable `reset`** and puts back the
  value the markup gave.
- **Chrome's other messages:** `Please include an '@' in the email address.
  'nope' is missing an '@'.`, `Please enter a URL.`, `Please match the
  requested format.` and `Value must be greater than or equal to 5.`

## The decision in one line

A form control's **state is the document's, held apart from its
attributes**. A control's **form owner is worked out when it is asked for**.
**Constraint validation is `alo-dom`'s**, one rule per constraint, and a
constraint not built yet **refuses the submission by name** rather than
letting the value through. **Submission is HTML's algorithm, run once in
`alo-dom` and driven like a click**: the controls are checked, `invalid` is
fired at each one that fails, `submit` is fired at the form, and what
survives is **ADR 0020's ask with a method and an entry list**. The **browser
process encodes** the entry list into a query or a body, decides the
submission as it decides a link, and adds `form-action` and mixed content to
its checks. **An agent submits as a keyboard user does**, by activating the
submit button, and is told what happened. **A file is chosen only by a
person**, held by a renderer as a number rather than bytes, and put into the
body by the browser process.

## Why this is a decision rather than a chore

HTML spells out every step of form submission, and the steps are not the hard
part. Five questions are, and a page, the person or an agent can see the
answer to each:

- **Where a control's state lives.** Today the `value` and `checked`
  attributes *are* the state, and both callers write them. HTML keeps the
  value and the checkedness apart from those attributes, which only give the
  defaults. `reset()`, `defaultValue`, `[value=…]` selectors and a script
  that sets `value` after a person typed all read the difference.
- **What happens to a constraint not built yet.** The contact form says
  `maxlength`, `web-a-form` says `min`, `max` and `step`, and alo's sign-up
  screen says `minLength`. Leaving a constraint unchecked sends a server
  values its own form promised it would never get.
- **Who encodes the body.** A body holding a file holds bytes from the
  person's disk, and ADR 0005 keeps that disk from every renderer.
- **What an agent is told.** A press on a submit button now answers
  `Activated` whether the form went, failed its checks, or was taken over by
  the page's own listener. Those are three different things to an agent
  that has to decide what to do next.
- **Who may choose a file.** An agent that could put any path from the disk
  into a page's file field could send any file on the machine to any site.

## 1. A control's state is the document's, apart from its attributes

`alo-dom` holds, per control, what HTML calls its state:

- an `input` or `textarea`'s **value** and **dirty value flag**;
- a checkbox or radio's **checkedness** and **dirty checkedness flag**;
- an `option`'s **selectedness** and **dirtiness**.

It lives in a table on the `Document`, keyed by node, in `alo-dom`'s new
`control.rs`. It is the one place both callers read and write, and it
replaces `field.rs`' `TEXT` and `activation.rs`' `checked`.

The attributes become what HTML says they are: the **defaults**. Setting a
`value` attribute changes the value only while the dirty value flag is
clear. Every rule that reads a value or a checkedness asks `control.rs`, and
that includes `:checked`, the box tree's drawn text and the agent tree's
`checked`. HTML's **value sanitization** runs there too: a line break is
stripped from a one-line field, and a date that is not a valid date string
is held as nothing, which `date.rs` already does.

**An agent's `PutText` is a user edit.** It sets the dirty value flag, on a
page with script or without. This is what makes `tooLong` and `tooShort`
apply to it, as they apply to a person's typing.

Until this is built, the attributes stay the state, as `field.rs` and
`activation.rs` say now. Every later rule reads through one function each,
so the split changes where the answer comes from and not who asks.

## 2. A form owner is worked out when it is asked for

A control's form owner is the `form` that its **`form` attribute** names by
id in its own tree. If the attribute is present and names no `form`, it has
no owner. Without the attribute, its owner is its **nearest ancestor
`form`**. `alo-dom` answers this each time it is asked. Nothing remembers it
on the node.

HTML resets a form owner every time an element is inserted or moved, or its
`form` attribute or the named element's id changes. In each case the reset
lands on exactly the answer above, except one: the parser's *form element
pointer*, which ties a control to an unclosed `<form>` that is not its
ancestor when markup is misnested, such as a form opened inside a table.
That is the legacy tail. Law 1 refuses it, and a page that depends on it is a
stage 3 page.

alo's own screens use the `form` attribute: `NewTaskModal.tsx`,
`EventModal.tsx` and `UserModal.tsx` put the submit button outside the form
and point it there by id, because HTML forbids nested forms. **A radio's
group** is then what HTML says: the same tree, the same name *and the same
form owner*, and `activation.rs`' group gains the third condition.

## 3. Constraint validation is `alo-dom`'s, and an unbuilt constraint refuses

Each constraint is one rule in `alo-dom`'s new `constraint.rs`, a pure
function from the document and a control to whether it fails and why. The
rules are HTML's: **valueMissing**, **typeMismatch** (`email`, `url`),
**patternMismatch**, **tooLong**, **tooShort**, **rangeUnderflow**,
**rangeOverflow**, **stepMismatch**, **badInput** and **customError**.
**Barred from validation**, as HTML says, are a disabled control, one under
a disabled `fieldset`, a hidden input, a read-only one, a button that is not
a submit button, and anything inside a `datalist`.

**A constraint not built is not skipped.** It can fail only when its
attribute is present *and* the control holds a value it could reject. A
`min` on an empty field can never fail, so `web-a-form`'s empty `time` field
submits. If an unbuilt constraint could fail, the submission **stops,
refused by name**, and the person and the agent are told which constraint is
not checked yet. That is ADR 0013 § 3: a form that went through unchecked
would be a server receiving what its own markup said it never would.

**`badInput`** is a value the person typed that the control could not
parse. This engine never holds one: `PutText` refuses a date that is not a
valid date string (ADR 0042), and a number field will refuse a number the
same way when it is built. So `badInput` is always `false` here, and that is
the truth rather than a gap.

**Messages.** A control's `validationMessage` is the browser's words, and a
page and an agent both read it. It is written in **English, word for word as
Chrome 154 writes it**, for each constraint that is built, and each message
is quoted in the code beside the measurement that gave it. Words in the
person's own language come with the person's region (item 387), as a date's
order does.

**`PutText` of text longer than `maxlength` is refused by name**
(`TooLong`) before the page is asked. A person cannot type past `maxlength`,
and a browser that cut an agent's text short would answer `TextPut` with
text the agent never sent. Refusing keeps the agent's record true.

## 4. Submission is HTML's algorithm, in `alo-dom`, driven like a click

A submit button's activation behaviour, which ADR 0018 § 6 left to item 82,
is to **submit its form owner from itself**. A submit button is a `button`
whose `type` is `submit` or missing, or an `input` of type `submit`. The
rules live in `alo-dom`'s new `submission.rs`. The events are dispatched by
whoever is running the click, as ADR 0018 § 3 already has it: the renderer
for an agent's verb, and the native for a script's `click()`. The steps are
HTML's:

1. **Not again.** A form already firing its submission events, or already
   building its entry list, does nothing. These are HTML's two guards, and
   without them a page's listener could recurse for ever.
2. **Checked**, unless the form has `novalidate` or the submitter has
   `formnovalidate`, or the submission came from `form.submit()`. Every
   control the form owns that is not barred is checked. `invalid` is fired
   at each one that fails, in tree order, as the measurement shows. If any
   failed, **nothing is submitted**.
3. **`submit` is fired at the form**: a trusted `SubmitEvent` that bubbles,
   is cancelable and names its `submitter`. It is trusted whoever pressed
   the button, because the browser's algorithm fires it. If a listener
   cancels it, nothing is submitted, as on the contact form.
4. **The entry list** is HTML's *constructing the entry list*. Each control
   the form owns, in tree order, gives its name and value, according to
   HTML's table:
   - a checkbox or radio only when checked;
   - the submitter only when it was the one pressed;
   - a file field gives one entry per chosen file, or one empty entry;
   - a disabled control gives nothing.

   The `formdata` event that HTML fires at this point needs `FormData`, and
   it arrives with it (§ 7).
5. **Method, action, enctype and target** come from the form, or from the
   submitter's `formmethod`, `formaction`, `formenctype` and `formtarget`
   where it sets them. The action is resolved against the document's base
   URL, and an empty action is the document's own URL, as ADR 0020 § 2
   resolves a link.
   - `method="dialog"` closes a `<dialog>`, which is not built. It is
     refused by name.
   - A target naming another window is refused by name, as it is for a
     link.
   - An `accept-charset` that does not allow UTF-8 is refused by name. Every
     document here is UTF-8, and other encodings are the legacy tail.
6. **The ask.** The submission is recorded in the document cell as **the
   page's ongoing navigation**, the one a link records (ADR 0020 § 1). So a
   task that submits twice, or submits and then follows a link, asks for the
   last of them and counts the rest as replaced. The measurement shows
   Chrome doing the same.

An `input type="image"` submits the coordinates of a click. An agent has no
coordinates (ADR 0002), and a keyboard's click gives `0,0`. It is **refused by
name** until a page needs one, and that page decides the answer. **Implicit
submission**, pressing Enter in a field, is a key. It waits on item 258, the
item that decides whether an agent's input is ever keystrokes.

## 5. The ask carries entries, and the browser process encodes them

ADR 0020's ask gains a **method** (`GET` or `POST`, one of `alo-net`'s own
values), an **enctype** (`application/x-www-form-urlencoded`,
`multipart/form-data` or `text/plain`, likewise) and the **entry list** as
names and values. A value is text, or a file by the number § 8 gives it. It
does **not** carry bytes already encoded.

The browser process encodes. A file's bytes then go from the browser process's
disk into the body without ever reaching a renderer. The multipart boundary is
the browser process's to choose, and the encoders run once, in the process
that writes the request:

- **GET** replaces the action's query with the entries, urlencoded. A GET's
  enctype is ignored, as HTML says.
- **POST** puts the entries in the body under the enctype, with that enctype
  as the `Content-Type`, and `multipart/form-data` gains its boundary.
  `application/x-www-form-urlencoded` has no charset parameter, as measured.
- **Newlines** in names and values are written as CR LF at encoding, which
  is where HTML normalises them.

**The browser process decides a submission as it decides a link** (ADR
0020 § 3): the URL is parsed again, under the 2 MiB bound once the query is
added, and checked against the same scheme table. Then come three checks a
link does not need:

- **`form-action`**, from the browser process's own copy of the document's
  header policy. A policy in a `<meta>` element is the renderer's to apply
  before it asks, as for scripts and fetches (ADR 0032 § 3).
- **Mixed content.** A secure document submitting to an `http` URL is
  **refused, said to the person**, and recorded. Chrome warns the person and
  lets them send anyway. Choosing to send anyway is the person's alone,
  never an agent's, and it waits for a window that can ask.
- **The cause**, by ADR 0012 § 4: in an `Act`'s answer it is the agent's,
  and otherwise it is the document's. Cookies come from the jar under its
  own rules (ADR 0007). A cross-site POST navigation is the unsafe-method
  case in which RFC 6265bis withholds a `SameSite=Lax` cookie, and the item
  that builds this half shows the jar doing so.

The decided navigation now carries a method and a body. As ADR 0020 has it,
it is handed to whoever drives `Tabs` until item 85 loads it. What a reload
of a POST does is item 85's to decide.

**Bounds.** One message bounds a renderer's entry list, which is
`LARGEST_MESSAGE`. The body the browser process builds, files included, is
bounded by `alo-net`'s `LARGEST_BODY`. A submission past either is refused
and said. No new number is chosen here.

**A renderer that sends what no form could make** is refused as a renderer
that broke the boundary, as ADR 0032 § 2 refuses one: a method other than
`GET` or `POST`, an enctype outside the three, or a file number its document
was never given.

## 6. An agent submits as a keyboard user does, and is told what happened

There is **no `Submit` verb**. A person submits by pressing the form's submit
button, and so does an agent: `Activate` on it fires the click ADR 0018 § 5
fires, and the click's activation behaviour submits. A page that works for a
keyboard user works for an agent, and a form with no submit button waits on
implicit submission, as it does for anybody who cannot press Enter.

What the agent is told becomes specific. `Activate` on a submit button
answers:

- **`Submitted`**, with the method and the URL as resolved, when an ask was
  recorded. Where the tab then goes is the browser process's decision, and
  item 134's to report.
- **`NotSubmitted`**, with each control that failed, its constraint and its
  message, when step 2 stopped it. This is an outcome, not a refusal: the
  verb was carried out and the form said no, as `TextCanceled` is
  (ADR 0018 § 5).
- **`Activated`**, as now, when a listener cancelled the click or the
  `submit`. The page took the press, as the contact form's listener does,
  and what it did next is the page's.
- A refusal by name when an unbuilt constraint, method or target stopped it
  (§§ 3 and 4).

**On a page that never ran script** the same rules run without events, as
`apply.rs` runs `activation.rs` now. Nobody can cancel, so the checks and the
ask are all there is.

**The agent tree reads validity.** A control that fails a constraint is
`invalid`, with its `validationMessage`, beside the `aria-invalid` an author
can set. The item that builds this first measures what Chrome's
accessibility tree says and when, because a field that reads as invalid
before anybody touched it would mislead the agent and a screen reader alike.

## 7. A page's script reaches all of this through the same rules

`form.submit()` is step 6 without steps 2 and 3, as measured, with its own
guard. `requestSubmit(submitter?)` is a press without a click. `reset()` and a
reset button fire `reset`, then put back each control's default and clear its
dirty flags. `checkValidity()`, `reportValidity()`, `setCustomValidity()`,
`validity` (`ValidityState`), `validationMessage` and `willValidate` read and
write `constraint.rs`. `value`, `checked`, `defaultValue`, `defaultChecked`,
`selectedIndex` and `form` read and write `control.rs` and § 2. Each of these
sits on its element's own interface, so it waits on item 262. Each is built
when a page needs it, and absent until then (ADR 0013 § 3).

**`FormData` and `URLSearchParams`** are objects a page constructs. A
`FormData` is built from the same entry list as § 4, by the same function,
and the contact form is the page that needs them. A fetch with a `FormData`
body is a multipart body the renderer encodes, because ADR 0032 makes a
fetch body bytes. A file in it has to be a `Blob` the page can read, and
that is what the File API's own item decides.

## 8. A file is chosen by a person, and a renderer holds a number

`<input type="file">` is filled in only through **the browser's own file
picker**. A person chooses files there, in a window's dialog (ADR 0024). For
each file chosen, the browser process gives the renderer **a number,
unique within that document, with the file's name, size and type**. The
renderer never receives a path, and it receives the bytes only when a page
reads them through the File API, its own item.

Submission names the number (§ 5), and the browser process puts the bytes
in. It refuses a number the document was never given.

**No agent can choose, replace or clear a file.** `PutText` and `Activate` on
a file field are refused by name, and an agent cannot answer the picker.
This is ADR 0026's reasoning. Choosing what leaves the machine is the
person's decision, and an agent that could fill a file field from the disk
could send any file on it to any site. The agent reads that the field
exists, what it `accept`s and how many files are chosen.

Without a window there is no picker. A file field submits as empty, as it
does for a person who chose nothing, and the field says so in the agent
tree.

## What this costs

- **A table beside the attributes.** Every control's state is held twice
  over in one sense: its default in the attribute, its state in `control.rs`.
  That is HTML's model, and the attribute-only stand-in is what it replaces.
- **A submission is not a request the renderer could have made.** The
  renderer resolves the action, the browser process encodes it, and the
  record holds both. It is one more kind of entry on the boundary, and it is
  the price of keeping a person's files out of a renderer.
- **Forms with constraints not built yet do not submit**, and say so. This is
  a page that does not work rather than a server receiving bad data, and the
  refusal names the item to build.
- **English validation messages for everybody**, until the person's region
  is a setting.
- **No send anyway for mixed content**, until a window can ask the person.

## Alternatives rejected

- **The renderer encodes the body.** That would put the bytes of every file
  a person chose into the process ADR 0005 keeps away from the disk, and a
  compromised renderer could then send them anywhere it can fetch.
- **A `Submit` verb.** A page whose form works for a keyboard user already
  works for `Activate`. A verb that submits a form with no submit button
  would do something no person on that page can.
- **Skip the constraints not built yet.** A server would receive what its
  own form promised it never would (§ 3).
- **Truncate an agent's text at `maxlength`.** The agent would be told its
  text went in when part of it did not (§ 3).
- **Remember the form owner on the node, as HTML's reset does.** The answer
  is the same in every case but the misnested-markup one law 1 refuses, and
  a remembered owner is one more thing to keep in step with every mutation
  (§ 2).
- **Let an agent choose a file the person pointed it at.** That is a grant,
  and an agent cannot hold one (ADR 0026). If somebody wants an agent to
  attach files, that needs a decision of its own, saying what the person
  grants and how it ends.
- **A submission as its own kind of ask, beside navigation.** HTML makes it
  a navigation, Chrome replaces one with the next as it replaces a link, and
  ADR 0020 said it would take that ask's shape.

## What this does not decide

- **Choosing an option** in a `<select>` by an agent, and the control's
  popup. That is a verb's shape, and it gets its own decision when a page
  needs it.
- **`:valid`, `:invalid`, `:user-valid` and `:user-invalid`**: matching them
  is `alo-css`'s, against `constraint.rs`, when a page styles one.
- **Drawing the validation bubble** and focusing the first invalid control.
  These belong to a window and to focus (item 258).
- **Autofill, saved passwords and `autocomplete`.** These are the person's
  data and get a decision of their own.
- **The File API**: `File`, `Blob` and reading a chosen file from script.
- **What a reload or a back navigation does to a POST.** That is item 85's.
- **Every input type not named here**: `color` and `range`'s drawing, and
  `time`, `datetime-local`, `month` and `week` (item 388). An empty one of
  these submits, as `web-a-form`'s `time` field does.

## How we will know if this was wrong

**If a frozen page's form sends, in Chrome, a body or a URL different from
the one this engine asks for**, the entry list or the encoder is wrong, and
it is fixed in the one place both read.

**If a page we did not write depends on a control's owner being a form that
is not its ancestor and is not named by `form`**, § 2's computed owner is
too strict, and that page is the case that schedules the parser's form
element pointer.

**If an agent is ever found to have sent a file a person did not choose**, § 8
was not followed, and that is a security bug, not a design question.

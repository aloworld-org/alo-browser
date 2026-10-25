# ADR 0064 — An option is chosen by activating it, and a list takes no text

**Status:** accepted
**Date:** 2026-10-25
**Context:** ADR 0002 (the layout tree is the agent's tree; *"activate this
element, put this text in that field"*); ADR 0042 § 4 (a date is put in as
text in its value's form, and what a field would silently empty is refused
by name); ADR 0018 §§ 5 and 6 (text and activation, as a page hears them);
ADR 0055 (the agent socket, additive only); ADR 0058 § 4 (one decision,
whichever renderer drew the page); queue items 621, 632 and 648;
`alo-agent`'s `verb.rs` and `apply.rs`, `alo-dom`'s `value.rs` and
`field.rs`, `alo-window`'s `chrome_act.rs` and `chrome_reading.rs`

## The decision in one line

An agent chooses an option of a `<select>` by **`Activate` on the option**,
which in a list of one choice makes it the choice and in a list of several
toggles it; the answer says what the list holds afterwards, read from the
page; `PutText` on a `<select>` is refused by a refusal of its own; and
through Chrome, an option of a **closed** list is refused by name until a
way to choose it is measured that makes the page hear only the choice made.

## The question

No verb chooses an option, on either renderer, and each renderer is wrong
about it in its own way (item 648):

- **On this engine's page**, a `<select>` is a `combobox`, which
  `alo_agent::verb` counts a field, so `PutText` on one is answered
  `TextPut`. `alo_dom::field::put_text` writes a `value` attribute that
  `alo_dom::value::selected` never reads, so the form sends what it sent
  before. An agent is told it chose something nobody chose — the false
  record ADR 0002 exists to make impossible. And an option is
  `display: none` in the user agent's sheet, so there is no option in the
  tree to aim anything else at.
- **Through Chrome**, `PutText` on a `<select>` is refused `Unbuilt`
  (item 621), and `Activate` on an option is already a click — which
  chooses an option Chrome draws, answers `Activated` without saying so,
  and, on a list of several choices, quietly drops every other choice.

Three answers were open: `PutText` of the option's words on the list,
`Activate` on the option, or a verb of its own.

## What was measured

Chrome 155.0.8059.86 on macOS, headless, over the pipe, with probes in
scratch space only:

- **Its tree holds a closed list's options.** A `<select>` is a `combobox`
  whose `value` is the chosen option's words, `hasPopup` `menu` and
  `expanded` false; under it a `MenuListPopup` holds every option as an
  `option` with its `selected` state — not ignored, though not drawn. An
  `<optgroup>` is a `group`. A disabled option, every option of a disabled
  list and every option of a disabled `<optgroup>` say `disabled`. An option
  that is `hidden` or `display: none` is not there. A `<select>` with
  `size` over 1, or `multiple`, is a `listbox` (with `multiselectable`),
  its options its children.
- **A closed list's option has no box.** `DOM.getBoxModel` refuses it
  (*Could not compute box model*), so a click cannot be aimed at it.
- **No key chooses on a closed list without choosing something else
  first.** On a focused closed list, Home, End, Page Up, the arrows and
  Enter change nothing on macOS, where the arrows and Space open the
  list's menu (none opened headless; `expanded` stayed false). Typed
  letters choose by type-ahead, and each letter that changes the choice
  fires `input` and `change`: typing *Small* from *Medium* chose
  *Spaced out* on the way, *Avocado* chose *Apple*, *Hulking* chose
  *Huge*, at once and at 600 ms a key alike; one letter typed twice
  cycles through the options it begins; and the match folds case and
  accents (an *a* reached *Ã…*), which is Chrome's collation and not ours.
- **A list Chrome draws is chosen by a click.** On `size=3` a real click on
  an option chose it, with one `input` and one `change`, and the arrows
  move the choice. On `multiple`, a plain click chose that option alone —
  *Red* and *Blue* chosen became *Green* — and a click with Meta held
  **toggled** it, adding *Green* and then taking *Red* away; Control acted
  as a plain click on macOS, and Shift chose a range.

## 1. An option is chosen by activating it

`Activate` already says *"press it, follow it, choose it — whatever
operating it means"*, `option` is already a role it operates, and an
option is a thing in the tree — Chrome's, and every accessibility API's,
which ADR 0002 makes the same tree. So the verb is the one ADR 0002 names
(*activate this element*), aimed at the thing being chosen, and
everything `find` and `activate` already refuse holds unchanged: an option
called the same as another is `Ambiguous` and is then asked for by its
id; a disabled one is `Disabled`.

**Not `PutText` of the option's words.** ADR 0042 § 4 put a date in as
text because a date *is* text in its value's form. A choice is not: two
options may be called the same, an option's words are not its value, and
one text cannot say several choices of a `multiple` list. Through Chrome
it would be carried out as typing, which is type-ahead, which is the
route measured above to choose options nobody chose.

**Not a verb of its own.** A `Choose` would say what `Activate` on the
option says, and would be one more verb for every renderer, the socket and
every agent to learn — for no case `Activate` cannot express. If one turns
up (choosing by value, or a list too long to read), it is argued then,
with that case.

## 2. What activating an option does

- **A list of one choice** — closed, or with a `size` — makes the option
  its choice and no other, as a person's click on it does.
- **A list of several** (`multiple`) **toggles** the option and leaves the
  others as they are, as HTML's *toggle the selectedness* and Meta-click do.
  A plain click's *this one alone* would make choosing two things
  impossible one verb at a time. Several choices are several verbs, each
  its own line in the record, and clearing a choice is activating it.
- **The page hears what it hears from a person**: `input`, then `change`,
  as ADR 0018 has it for text and activation; on a page that never ran
  script, `alo-dom`'s selectedness rule is carried out by `apply`, as
  every other verb's is.
- **Refused `Disabled`** when the option, its `<optgroup>` or its list is
  disabled — which is what Chrome's tree says of the option in all three
  cases, and what this engine's states must say of it too.

## 3. The answer says what the list holds, read afterwards

A new outcome, **`Chosen`**: the option, its list, and whether the option
is chosen **after** the verb — read from the page once it has run, not
assumed from what was asked. *"chose box#43 "Huge" in box#29"*, or for a
toggle that took one away, that box#43 is no longer chosen in box#29. A
page's `change` listener may put the old choice back, and the answer then
says the option is not chosen: an outcome rather than a refusal, as
`TextCanceled` is, because the verb was carried out and the page answered.

On the socket this is a new outcome kind, which version 1 of the contract
already allows (*additive only*); nothing an agent asks changes.

## 4. A list takes no text

`PutText` on a `<select>` is refused, on both renderers, by a refusal of
its own — **`Refusal::Chooses`**: the list is a thing to choose from, and
its options are activated. It replaces the engine's false `TextPut` and
Chrome's `Unbuilt`, which said *cannot yet* of something that never will.

The rule is about the element, asked of the `Reading` as whether a field
is a date is: a `combobox` that is a text field (`<input role=combobox>`,
an autocomplete) takes text, as it does today. On the socket the refusal
is a new kind, additive again.

## 5. This engine's tree holds a list's options

An option a person could choose — one of HTML's *list of options* of a
`<select>`, not `hidden` — is a node of the agent's tree **under its
list**, with its words, its `selected` and `disabled` states, and its
`<optgroup>` as a `group`, whether or not the list is open. Chrome's tree
and every platform's accessibility tree hold them so, and an agent asked
to choose has to be able to read what there is to choose from.

A closed list's option is **not drawn and has no rectangle**, and the tree
says so rather than inventing one: it is a node an agent can name, never a
place on the screen. So 624's ring is drawn around the list, the box a
person sees, when the option chosen has none.

How it is held is the build's — but by the same pass that makes every
other node, never a second tree built for the agent (ADR 0002).

## 6. Through Chrome

- **An option Chrome draws** — a list with a `size`, or `multiple` — is
  clicked at its middle, as every `Activate` is (item 621), and on a
  `multiple` list with the platform's toggle held: **Meta on macOS**,
  measured; Control elsewhere is Chrome's rule there and is measured
  where it is first run.
- **An option of a closed list** has no box, and every key that reaches it
  passes through options nobody chose — each heard as `input` and
  `change`, so a page that goes wherever its list says (a jump menu) would
  be sent to the first of them. That is an action the agent did not ask
  for, carried out on its behalf, and it is not taken. Predicting a
  single key that would do instead means copying Chrome's collation,
  which is not ours to copy. So it is **refused by name, before Chrome is
  asked anything**: the option is in a closed list, which this browser
  cannot choose in through Chrome yet. Lifting that is its own item:
  what a person's way does in a shown Chrome — the list's menu, which
  macOS draws as a menu of its own — measured with the screen in use.
- **Never script, and never `DOM.setAttributeValue`.** Setting the
  attribute sends the page no `input` and no `change`, so its listeners
  and its checks would not run, and a page can tell (`chrome_act.rs`).

## Consequences

- `alo-agent`: an `Outcome::Chosen` with the option, the list and whether
  it is chosen; `Refusal::Chooses`; `perform` deciding `Activate` on an
  option as a choice in its list, and `PutText` on a `<select>` refused,
  through the `Reading` on either renderer.
- `alo-box` (or wherever the build puts it): a `<select>`'s options in the
  agent's tree, closed or open, with no rectangles while not drawn, and
  the disabled list's options disabled.
- `alo-agent`'s `apply` and the renderer: `alo-dom`'s selectedness rule —
  the `selected` attribute until item 469 holds selectedness apart from
  markup — then `input` and `change` on a page with script.
- `alo-window`: a click on a drawn option, with Meta on a `multiple`
  list; a closed list's option refused before any command; the choice
  read back from Chrome's tree for the answer.
- `docs/contracts/agent-socket.md`: the outcome kind and the refusal kind.
- Item 648 closes when that is built: an option chosen by `Activate` is the
  one a form sends, through the engine and through Chrome's drawn lists;
  `PutText` on a `<select>` answers `Chooses` on both; and a closed list's
  option through Chrome is refused by name. A closed list through a shown
  Chrome is a new item.
- *Built in parts (iteration 403).* Item 648 closed on § 4 alone, a list
  taking no text on both renderers; §§ 1, 2, 3 and 5 on this engine's page
  are item 659, and § 6 is item 660. The decision is unchanged.
- *Iteration 404.* Item 659 built §§ 1, 2, 3 and 5 on this engine's page.
  The socket's tree gains an additive `undrawn` field, since version 1
  promises every node a `rect` and an option has none to give.

## What this does not decide

- **A `<datalist>`'s suggestions.** They are offered to a text field, and
  the text is the field's; whether an agent reads them is its own item.
- **A list a page builds of its own** — `role=listbox` and `role=option` on
  `div`s. An option there is activated as today, by a click; what it does
  is the page's script, as it is for a person.
- **A list Chrome draws in the page** (`appearance: base-select`, its
  customizable list). Not measured; when a page uses one, its options may
  have boxes while open, and that is measured then.
- **Choosing by value.** An agent names what it reads, and a value is not
  read.

## How we will know if this was wrong

- **If an agent cannot tell which list an option belongs to** — two lists
  each with a *Yes*, and the tree not saying whose is whose — then the
  tree's nesting is not enough, and the target needs a way to say *the
  option Yes of the list Delivery*.
- **If a shown Chrome's list cannot be chosen in at all** without passing
  through other options, the most common list on the web stays refused on
  the product path, and the route — not the verb — is reopened.
- **If a list is too long to read** — thousands of options, every one a
  node an agent is handed — then reading options whole was wrong, and the
  answer is a way to ask for them, not a second verb.

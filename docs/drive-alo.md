# Drive alo from your code

alo answers a program the way it answers its own agent: by **the page's tree**
— what each thing on the page *is* and what it is called — and by **typed
verbs** aimed at a thing by its name. No CSS selector, no screenshot, no
coordinate. A program sends one JSON line and reads one JSON line back.

- **macOS only today.** The socket is a Unix domain socket; how a program
  reaches alo on Windows is not decided yet (ADR 0055 § 1).
- **No AI key is needed.** The socket is the browser's, not a model's.
  Nothing you send through it reaches any AI service.
- The contract a program builds against is
  [`docs/contracts/agent-socket.md`](contracts/agent-socket.md); why it is
  this shape is ADR 0055, and why no verb takes a position is ADR 0002.

## 1. Start alo and open a page

From a terminal, in the conversation:

```sh
alo --conversation
```

(`target/release/alo` in a build of this repository, or
`"alo browser.app/Contents/MacOS/alo"` inside the application.) It says where
the socket is as its window opens:

```text
alo: an agent reaches this browser at /var/folders/…/T/alo-browser/agent.sock
```

Type an address into alo's box and press Return. The page opens as a card in
the conversation, in the Chromium alo carries, and **the socket reaches the
page the card shows now**. Opened from the Finder, the application holds the
same socket; nothing is printed, and the place is the one below.

## 2. Find the socket

    $XDG_RUNTIME_DIR/alo-browser/agent.sock    when XDG_RUNTIME_DIR is set
    $TMPDIR/alo-browser/agent.sock             otherwise (macOS sets TMPDIR per person)

Only you can open it: its directory is `0700` and the socket `0600`. A web
page can never reach it — there is no port.

## 3. Ask with `alo-ask`

`alo-ask` is the smallest caller: it sends each line it reads and prints each
answer. It finds the socket itself; name one as its last argument to reach
another. It is `target/release/alo-ask` in a build, and
`"alo browser.app/Contents/MacOS/alo-ask"` inside the application.

Which page is open:

```sh
echo '{"v":1,"ask":"tabs"}' | alo-ask
```

```text
{"answer":"tabs","tabs":["https://the-internet.herokuapp.com/add_remove_elements/"],"v":1}
```

### What is on the page, for a person

`read` answers the tree as JSON, with each box's rectangle. `alo-ask --plain`
prints every answer for a person instead: the tree one node a line, indented
under what it is in — its number in brackets, its role, its name in quotes and
its states — and **no position**; a verb's outcome and a refusal as alo's own
sentence.

```sh
echo '{"v":1,"ask":"read","tab":"https://the-internet.herokuapp.com/add_remove_elements/"}' | alo-ask --plain
```

```text
tab https://the-internet.herokuapp.com/add_remove_elements/
[1] document "The Internet"
  [21] link "Skip to content" offscreen
  [22] link "The Internet: home"
  [24] region "Messages"
  [29] main
    [30] heading "Add/Remove Elements" level=3
    [33] button "Add Element"
    [34] separator
  [38] contentinfo
    [50] text "Powered by"
    [42] link "Elemental Selenium"
    [52] text "and"
    [43] link "Sauce Labs"
```

Generic boxes with no name of their own are left out and what is inside them
is written one level up; a list says how many items it holds
(`[12] list "Products", 3 items`). The number in brackets may be named as a
target (`{"node":33}`), but only against the tree it was read from: aim by
name where you can.

### Press a button by its name

```sh
echo '{"v":1,"ask":"act","tab":"https://the-internet.herokuapp.com/add_remove_elements/","target":{"name":"Add Element"},"verb":"activate"}' | alo-ask
```

```text
{"answer":"done","issues":[],"outcome":{"kind":"Activated","name":"Add Element","node":33},"said":"activated box#33 \"Add Element\"","tab":"https://the-internet.herokuapp.com/add_remove_elements/","v":1}
```

### Verbs, and what refuses them

There are three verbs: `activate` (press, follow or choose), `put_text` with
`"text"` (replace what a field holds), and `scroll` with `"scroll"`. On a
page in a card, which Chromium draws, `scroll` is refused as `DoesNotScroll`
for now: Chromium's tree cannot say what scrolls. `activate` scrolls what it
presses into view itself.

A target is a `name`, a `role`, both, or a `node`. A verb that cannot be done
is **refused with a kind**, never a sentence to parse:

```sh
echo '{"v":1,"ask":"act","tab":"https://the-internet.herokuapp.com/add_remove_elements/","target":{"name":"Nothing here"},"verb":"activate"}' | alo-ask --plain
```

```text
refused (NotFound): nothing on the page is "Nothing here"
```

alo never types a password or a card's details into a page shown as a card
(`Password`, `CardDetails`), and `alo --agent-site ADDRESS` keeps every agent
to the sites named (`NotGranted`). Every request is recorded by alo — what was
read, what was done, which connection asked.

## 4. From your own program

Each request is one JSON object on one line; each answer is one line back, in
the order asked. These are the whole of a working caller.

### Python

```python
import json, os, socket

place = os.environ.get("XDG_RUNTIME_DIR") or os.environ["TMPDIR"]
alo = socket.socket(socket.AF_UNIX)
alo.connect(os.path.join(place, "alo-browser", "agent.sock"))
lines = alo.makefile("rw")

def ask(**request):
    lines.write(json.dumps({"v": 1, **request}) + "\n")
    lines.flush()
    return json.loads(lines.readline())

tab = ask(ask="tabs")["tabs"][0]
pressed = ask(ask="act", tab=tab, target={"name": "Add Element"}, verb="activate")
print(pressed["said"])
tree = ask(ask="read", tab=tab)["tree"]
print([child.get("name") for child in tree["children"]])
```

```text
activated box#33 "Add Element"
['Skip to content', 'The Internet: home', 'Messages', None, None]
```

### Node

```js
const net = require("net"), os = require("os"), path = require("path");
const readline = require("readline");

const place = process.env.XDG_RUNTIME_DIR || os.tmpdir();
const alo = net.createConnection(path.join(place, "alo-browser", "agent.sock"));
const answers = readline.createInterface({ input: alo })[Symbol.asyncIterator]();

async function ask(request) {
  alo.write(JSON.stringify({ v: 1, ...request }) + "\n");
  return JSON.parse((await answers.next()).value);
}

(async () => {
  const [tab] = (await ask({ ask: "tabs" })).tabs;
  const pressed = await ask({ ask: "act", tab, target: { name: "Add Element" }, verb: "activate" });
  console.log(pressed.said);
  alo.end();
})();
```

```text
activated box#33 "Add Element"
```

## What it does not do

- **Take a position.** A rectangle is in the tree to be read; no verb takes
  one (ADR 0002).
- **Watch.** There is no subscription: ask, and you are answered.
- **Open an address.** The person opens pages; the socket reaches the one the
  card shows. alo's own agent opens sites through the browser's judgement
  ([ADR 0103](decisions/0103-alos-agent-opens-a-site-only-as-the-person-would-and-never-on-their-own-network.md)), not through
  this socket.

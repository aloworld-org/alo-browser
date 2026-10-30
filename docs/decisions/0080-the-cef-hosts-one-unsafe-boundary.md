# ADR 0080 — the CEF host's one `unsafe` boundary

**Status:** accepted — the owner's "Yes", 30 October 2026. Built the same
day (queue items 731, 732, 730; iteration 457).

## The decision in one line

alo's CEF host program (`crates/alo-cef-host`, a process of its own, not
alo's) may hold **one** reviewed, named `unsafe` boundary on macOS — the
`NSApplication` subclass CEF requires, in `src/application_mac.rs` — and
`src/window.rs` may expect the one clippy lint the `cef` crate's own
macros trip; every other file of the host, and every crate of alo's own
workspace, stays free of `unsafe`, and `scripts/gate.sh` checks that it
does.

## Why CEF requires it

Chromium on macOS asks the application object `-isHandlingSendEvent` and
tells it `-setHandlingSendEvent:` — Chromium's `CrAppProtocol` and
`CrAppControlProtocol`, which CEF gathers as `CefAppProtocol` — and
`AppKit`'s own `NSApplication` answers neither. Until now the host made no
application object of its own (ADR 0079 § 8b, queue item 725), and a
shown host crashed wherever Chromium asks, measured on this Mac with CEF
156.0.14 (queue item 727), each time with

```text
-[NSApplication isHandlingSendEvent]: unrecognized selector
```

an `NSInvalidArgumentException` ending the host with `SIGTRAP`, a report in
`~/Library/Logs/DiagnosticReports` and macOS's *quit unexpectedly* dialog:

- **a page closed from inside** — `Target.closeTarget`, which
  `chrome_window::close` sends, and a page's own `window.close()`;
- **a context menu**, even one CEF's `on_before_context_menu` emptied;
- **a closed list's menu** (`<select>`).

Chrome's own `BrowserCrApplication`, which CEF's framework carries, cannot
stand in: made the shared application before CEF starts it crashes in its
`+initialize` (727). CEF's own samples answer the protocols with a subclass
of their own (`SimpleApplication` in `cefsimple_mac.mm`,
`ClientApplication` in `cefclient`), and so does the `cef` crate's
`cefsimple` example in Rust, with `objc2::define_class!`. Defining an
Objective-C class is `unsafe` in Rust: the compiler cannot check that a
method's Rust signature is the one the Objective-C runtime calls it with,
nor that the superclass named is the real one. ADR 0079 § 3.2 foresaw this
boundary; writing the host in C++ instead would avoid Rust's `unsafe` and
break law 4's *one language*.

`AppKit`'s `-terminate:` costs something too: it ends the process at once,
so the message loop never returns, CEF never shuts down, and Chromium's
own clean-up never runs (ADR 0079 § 8b). The subclass overrides it, as
CEF's samples do.

## Exactly where, and what each `unsafe` does

| File | What is `unsafe` there | Why |
|---|---|---|
| `crates/alo-cef-host/src/application_mac.rs` | `define_class!`: `#[unsafe(super(NSApplication))]`; five `#[unsafe(method(…))]` — `-sendEvent:` and `-terminate:` overridden, `-isHandlingSendEvent` and `-setHandlingSendEvent:` answered; `unsafe impl` of `NSObjectProtocol`, `CrAppProtocol`, `CrAppControlProtocol` and `CefAppProtocol` (the last three as the `cef` crate declares them, `cef::application_mac`) | the class CEF requires |
| the same file | `msg_send![super(self), sendEvent: event]` | the overridden method, on the same object, with its event |
| the same file | `msg_send![AloCefHostApplication::class(), sharedApplication]` | `AppKit`'s class method, making this class the shared application before CEF starts |
| the same file | `NSMenuItem::initWithTitle_action_keyEquivalent` | a menu item's selector, chosen from a closed list (`Action`: `terminate:`, `performClose:`, `cut:`, `copy:`, `paste:`, `selectAll:`), so the menu bar (`menu_mac.rs`) holds none |
| `crates/alo-cef-host/src/main.rs` | `#![deny(unsafe_code)]` for the whole program, and `#[expect(unsafe_code, reason = …)]` on `mod application_mac;` alone | the lint lifted on the one module, and nowhere else |
| `crates/alo-cef-host/src/window.rs` | no `unsafe` written; `#![expect(clippy::transmute_ptr_to_ptr, reason = …)]` | the `cef` crate's `wrap_*` macros expand, in this file, to the crate's own code, which transmutes its base reference; the compiler's `unsafe_code` lint does not report another crate's macro, clippy reads it. The code is the crate's (ADR 0010) |

The state the class keeps — whether an event is being sent, and what
`-terminate:` does instead of ending the process — is in two statics of
safe types (`AtomicBool`, `OnceLock<fn()>`), not in instance variables:
`objc2` requires instance variables initialised by an `-init` of ours,
which `+sharedApplication` does not run, and a process has one application
object.

`-terminate:` calls `window::close_all`: every page the host has open —
its own and every one Chromium opens of its own, which CEF gives the
host's client through `default_client` — is closed without asking it (as
Chrome's `Browser.close` closes pages), and once the last is closed the
message loop is left, `cef::shutdown` runs and the host returns status 0.
`Browser.close`, the menu bar's *Quit alo* and a logout all reach it. ⌘Q
does not: the page's window takes it as Chrome's Exit command
(`IDC_EXIT`), which left the host neither ended nor able to end (measured),
so the host's command handler answers it with `close_all` too.

The menu bar (`crates/alo-cef-host/src/menu_mac.rs`, no `unsafe`): *alo*
with *Quit alo* (⌘Q); *Edit* with *Cut*, *Copy*, *Paste* and *Select All*
(⌘X, ⌘C, ⌘V, ⌘A); *Window* with *Close Window* (⌘W). macOS adds its own
items after them (Writing Tools, AutoFill, Start Dictation, Emoji &
Symbols; Close All), measured.

## What is checked

**By the gate** (`scripts/gate.sh`, queue item 730), every run:

- `unsafe` written — any use of the word outside comments — in no file of
  the host but `application_mac.rs`, and in that one (so the check names
  the right file);
- the lint `unsafe_code` named exactly twice in the host's code, both in
  `main.rs`: `#![deny(unsafe_code)]`, and the attribute whose item is
  `mod application_mac;`;
- `clippy::transmute_ptr_to_ptr` in `window.rs` and no other file;
- in the workspace's crates, no `unsafe` block, function, implementation,
  trait, extern block or attribute, no naming of the `unsafe_code` lint and
  no `transmute_ptr_to_ptr` — the compiler's `forbid` already refuses the
  first, and the existing check on every manifest stands unchanged;
- `cef` named only in `host.rs`, `window.rs` and `application_mac.rs`, and
  `objc2`, `objc2_app_kit` and `objc2_foundation` only in
  `application_mac.rs` and `menu_mac.rs` (the rented-crate boundaries);
- the host's formatting; and, when `CEF_PATH` names an unpacked CEF
  distribution, `cargo clippy --all-targets -- -D warnings` and
  `cargo test` on the host's own workspace. Without one the gate says, in
  so many words, that the host was **not built**.

Each refusal was seen to fire, on a copy of the sources: `unsafe` in
`host.rs`; the lint lifted in `menu_mac.rs`; `deny` weakened to `warn`;
the lift moved onto another module; the clippy lint expected in
`switches.rs`; an `unsafe fn` and a multi-line `#[allow(unsafe_code)]` in
a workspace crate.

**By tests** (`crates/alo-window/tests/alos_cef_host_shows_a_page_as_chrome_does.rs`,
queue item 731): a shown host lives through a context menu opened by a
right click and dismissed with Escape, a closed list's option chosen
through its menu by an agent (`Activate`, heard by the page), a page
closed by `chrome_window::close` and one closed by its own
`window.close()`; its menu bar reads *alo*, *Edit*, *Window* with the
host's items first; ⌘A, ⌘C, ⌘V copy and paste what a person typed; and ⌘Q
— with the page's window in front, and with no window — ends the host,
its pipe closed and Chromium's process-singleton directory removed by
Chromium's own clean-up, which the old exit skipped. No new crash report.
Without the application object installed the test fails (no context menu
is ever shown: the host crashes); with ⌘Q's Exit command left to Chrome it
fails (the host does not end).

## What alo's own process keeps

Everything ADR 0058 kept. alo's process links nothing of Chromium's and
holds no `unsafe`: the workspace's `unsafe_code = "forbid"` stands for
every crate in it, and the gate's existing manifest check is unchanged.
The host is not one of the workspace's crates; it is a separate program
alo starts and drives over a pipe, and this boundary is in it alone.

## What is not measured, or not fixed

- **A page closed while its context menu is still open** crashes the host
  with `SIGSEGV` inside CEF's framework, reading freed memory
  (`0xcdcd…`), once the page is gone — closed by alo (`Target.closeTarget`
  is not obeyed while the menu is open, in Chrome too; the
  `Browser.close` after it crashes) or by a person's close button.
  Measured three ways on this Mac with CEF 156.0.14; Chrome 156 closed by
  `Browser.close` in the same state did not crash. Whether it is CEF's or
  this host's is not known: queue item 734.
- Whether ⌘C and ⌘V reach the page through the *Edit* menu or through
  Chrome's own shortcuts, which the page's window sees first, is not
  measured; what a person gets is.
- In one run of the eleven that reached it, the closed list's option was
  not chosen (`NeverClosed`: its menu still shown after Return); the ten
  others chose it. Not reproduced since.
- Only macOS; Windows and Linux need no such class (ADR 0079 § 3.2) and
  are queue item 728.
- The `cef` and `objc2` crates' own `unsafe` is theirs (ADR 0010), not
  reviewed here.

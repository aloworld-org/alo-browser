# ADR 0111 — Sign-ins are kept as the cards' cookies, sealed in a jar of alo's own, and every Chromium still starts on a profile thrown away with it

**Status:** accepted by the owner, 11 November 2026 (queue item 935 asked
for the ADR before any build and for the owner's acceptance; it was built
behind it on the supervisor's word the same day)
**Date:** 2026-11-11
**Context:** queue item 935; ADR 0110 § 7 (*already signed in* holds only
inside one card's Chromium); ADR 0060, *What this does not decide* (a
persistent profile for a person); ADR 0011 (what may be written to a disk);
ADR 0058 § 3 and ADR 0069 (each Chromium on a new profile, removed with it,
held by `alo.lock`); ADR 0079 and the CEF host's `switches.rs`
(`--use-mock-keychain`); ADR 0098 (a key in the person's keychain, an AEAD
crate behind one file); ADR 0108 (the cookie choice written into
`Default/Preferences`); ADR 0096 and ADR 0099 (alo's agent never types a
password or a card's details).

## The question in one line

A person who signs in to a site in a card is signed out the next time:
every Chromium alo drives starts on a new profile that is deleted with it.
How are sign-ins kept between cards and between runs of alo — where, sealed
how, seen and forgotten how — without giving up what ADRs 0011, 0060 and
0069 bought?

## 1. The two ways, and why not a kept profile

**A Chromium profile kept on the disk** is what a browser does: give CEF a
fixed `root_cache_path` and `cache_path` (CEF 156's `cef_types.h`: with a
`cache_path` *"profile-specific data"* persists; with none, *"incognito
mode"*), or Chrome a fixed `--user-data-dir`. Read against CEF 156.0.14 and
Chromium's source, it costs four things alo cannot pay today:

- **One Chromium per profile at a time.** `cef_types.h`: *"Multiple
  application instances writing to the same root_cache_path directory could
  result in data corruption. A process singleton lock based on the
  root_cache_path value is therefore used to protect against this"* — a
  second CEF on it is handed to the first (`OnAlreadyRunningAppRelaunch`).
  Chrome's `SingletonLock` does the same. alo starts **a Chromium for every
  card** (`conversation_chrome.rs`, ADR 0108 § 1): a kept profile would need
  every card in one Chromium, which is ADR 0076's whole card design, or a
  profile per card, which keeps nothing shared.
- **Everything, not sign-ins.** A profile keeps the HTTP cache, history,
  site storage, service workers, the favicon and visited-link databases —
  the durable record of everywhere a person went that ADR 0011 § 2 refuses
  to write unless it must, and that ADR 0069 removes with each Chromium.
- **Chromium's own encryption at rest needs the keychain, and alo has
  switched it off.** On macOS Chromium seals its cookie database under a key
  it keeps in the login keychain (`components/os_crypt/common/keychain_password_mac.mm`,
  `main`, read 11 November 2026): service **`Chromium Safe Storage`**,
  account `Chromium` for a build that is not Chrome's (`Chrome Safe Storage`
  / `Chrome` for Chrome's), made at random when there is none. The CEF
  framework alo carries holds that name (`strings` of
  `Chromium Embedded Framework`, CEF 156.0.14), and CEF 156 adds
  `CefSettings.keychain_service_name` and `keychain_account_name` to choose
  another. The CEF host starts with **`--use-mock-keychain`** (ADR 0079 §
  8a, `switches.rs`) because the item's dialog on a rebuilt, differently
  signed host *held the network until a person answered it*: macOS asks the
  person before any program other than the one that made an item reads it,
  and an ad-hoc-signed host is a different program at every build. With the
  mock, Chromium's key lives in the process, so a kept profile's cookies
  would be sealed under nothing that outlives it — readable by nobody next
  time, alo included. Keeping a real `Chromium Safe Storage` item instead
  means that dialog, once per signed build, for the person, before any page.
- **A profile is a stranger's input.** Its files are written by the
  Chromium a page drives; ADR 0069's sweep and ADR 0108's preferences are
  written into it *before* it starts, which a kept one would no longer be.

**A jar of alo's own** — chosen — keeps only the **cookies**, outside every
profile, in the browser process, which hands them to each card's Chromium
through the DevTools protocol it already speaks. Every Chromium still
starts on a new profile removed with it (ADR 0069 unchanged); nothing of a
person's is ever in a directory a Chromium keeps.

## 2. How a card is given its sign-ins, and how they are taken back

- **As a card's Chromium starts**, before it opens any page,
  `Storage.setCookies` on the browser's own target puts the jar's cookies
  into it (`chrome_sign_ins::given`), and `Storage.getCookies` reads back
  what it took.
- **While it runs**, every 2 seconds between its other work, and **once
  more as it closes** (*Close*, the card replaced, its window gone, alo
  quitting), `Storage.getCookies` is read, and what changed since last seen
  — set, changed, or removed by a site signing the person out — is made to
  the jar (`Changes::between`, a cookie being the same by its name, domain
  and path, as Chromium's store keys one). A sign-in made in a card is kept
  within 2 s should alo be killed.
- **One jar for the browser process**, shared by every card: a card closed
  and another opened is signed in to what the first was, session cookies
  included, as two tabs of one browser are.
- What a cookie holds is the site's and is never read or changed: name,
  value, domain, path, expiry or none, `HttpOnly`, `Secure`, `SameSite`,
  priority, and the scheme and port that set it — each as Chromium's
  protocol gives it, and put back the same.
- **Partitioned cookies (CHIPS) are not kept**: their partition key is a
  second identity this does not carry yet. Queued (948).

Measured 11 November 2026 through Chrome 157 and CEF 156.0.14, headless,
by `tests/sign_ins_kept_through_chrome.rs`, and through cards by
`tests/sign_ins_kept_from_card_to_card_through_chrome.rs`.

## 3. What is written, where, sealed how

- **Where:** one file, `sign-ins.sealed`, in
  `~/Library/Application Support/org.aloworld.Browser/Sign-ins`, beside the
  chats' directory — never in the repository, never in the temporary
  directory. The directory is `0700`, the file `0600`, written whole under
  another name and renamed over it.
- **What:** only cookies that expire. **A session's cookie is never
  written** (ADR 0011 § 2's *never written rather than written and
  deleted*): kept in memory while alo runs, gone as it quits, as any
  browser's. An expired cookie is dropped as it is read. No site storage
  (`localStorage`, IndexedDB), no cache, no history, no password, no form.
- **Sealed:** XChaCha20-Poly1305 (`chat_seal.rs`, still the one file that
  names the cipher, ADR 0098 § 4), the file's own magic as associated data,
  under **a key of its own** in the person's login keychain: generic
  password `alo-sign-ins`, account the login name, labelled *alo: the key
  the sign-ins kept on this Mac are sealed under*, made by alo through
  `/usr/bin/security` when there is none, never on a command line, never
  printed (`sign_in_key.rs`). Its own key, not the chats', so *Forget all*
  can replace it (§ 5).
- **The prompt a person sees:** none is expected, by the same reasoning as
  ADR 0098 — the item is made by `/usr/bin/security` and read by it, so the
  program asking is the one that made it — and **not verified for a signed
  release**: every test and hand run here used a keychain file of its own
  (§ 6). Chromium's own `Chromium Safe Storage` item is never made: both
  Chromiums keep their profile thrown away, the CEF host with
  `--use-mock-keychain` as before. The machine's Chrome, used only outside
  the application, may read its own `Chrome Safe Storage` item as it always
  has; that is Chrome's and is unchanged by this.
- **A file that does not open** with the key in this keychain is said and
  **left as it is** — nothing is written over it — until *Forget all*
  removes it.

## 4. ADR 0108's cookie choice holds

Each Chromium is still started with *Settings › Privacy › Cookies* written
into its new profile's `Default/Preferences`, and it decides what it keeps
and sends by that, the jar's cookies included: started refusing every
site's cookies, a Chromium is given the jar and sends none of it (tested);
refusing other sites', it sends a kept cookie only as a first party. The
jar holds what Chromium held; a cookie refused by a Chromium is never
set in it, so is never kept.

## 5. Seen and forgotten: *Settings › Privacy › Sign-ins*

A third row of *Privacy*: **Sign-ins**, one quiet line saying whether they
are kept — *Signed-in sites are kept on this Mac, sealed under a key in
your keychain, until you forget them.* (or why not: another alo keeps them,
no key, a file that does not open) — and the button **Forget all
sign-ins**, reached by Tab after *Cookie banners*, pressed by Space or
Return or the pointer. It:

1. empties the jar in memory, a session's cookies too;
2. removes `sign-ins.sealed`;
3. replaces the `alo-sign-ins` key with a new one (`security
   add-generic-password -U`), so a copy of the old file — a backup, a synced
   folder — never opens again;
4. empties the open card's Chromium at its next look (`Storage.clearCookies`),
   and a look that raced it never puts back what was forgotten
   (`change_as_of`).

The row then says *Every sign-in on this Mac was forgotten.* **Per site** is
cut and queued (946): it needs the list of sites drawn, which is a second
section's worth of layout.

## 6. Tests and hand runs never touch a person's sign-ins

- A jar exists only where the `alo` program makes one for its conversation
  (`SignIns::for_this_alo`, handed to `window::conversation`). Every other
  road — `ChromeOpener::start`, `ChromeServing::start_as_a_card`, `alo
  --chrome`, every test — starts cards with **no jar**, keeping nothing
  past their own Chromium.
- Tests make a jar in a directory of their own, under a key of their own,
  and assert the person's own `sign-ins.sealed` unchanged.
- `alo --conversation --sign-ins DIR` (a hand run's) keeps the jar in
  `DIR` and its key in a keychain file `DIR/alo-sign-ins.keychain-db` made
  and unlocked there — never the person's keychain.
- **`--sign-ins` is a developer's hand run's alone, and the application
  refuses it** (amended 12 November 2026, queue item 973). The scratch
  keychain is unlocked by a password written in `sign_in_key.rs`, so it is
  no secret: fine at a developer's terminal, wrong on any product path.
  Inside `alo browser.app` — decided by where the program is
  (`Bundle::around`), as everything else the application does differently
  is — `--sign-ins` is **refused before anything is made** (no directory,
  no keychain file, no window), exit status 2, the terminal saying it is a
  hand run's and naming what to use instead: `--data DIR`, which keeps no
  secret at all (ADR 0112 § 5), or nothing, which keeps the sign-ins
  under their own key in the person's keychain (§ 3)
  (`bundle_refused.rs`; `tests/the_application_refuses_a_hand_runs_sign_ins.rs`,
  the real `alo` copied into an application shaped as `alo-bundle` makes
  it). Run from `target/` it is unchanged.
  *A cargo feature was rejected*: it would take the code out of a release
  build, but the gate builds and tests with every feature on, so the
  configuration that ships would be the one never built or tested, and
  every developer's `cargo run` would need the feature named. The written
  password is still in the shipped binary, reachable only through the
  refused option; a person who copies the program out of the application
  and runs it from a terminal is a hand run, by their own act.

## 7. A second alo at once

The browser process keeping the jar holds `sign-ins.lock` locked for its
life (`File::try_lock`, as ADR 0069 § 1 holds a profile). A second alo
opened meanwhile finds it held, keeps its own sign-ins **in memory alone**,
reads and writes nothing of the first's, says so on its terminal and in
*Settings* (*Another alo that is open keeps the sign-ins*). The kernel lets
a killed alo's lock go. A lock found held is tried again for half a
second before it is taken for another alo's: a child process holds a copy
of every descriptor between its `fork` and its `exec`, which once made a
lock just let go of refuse in a test run.

## 8. What alo's agent may do with them: nothing new

The agent's rules are unchanged. It never types a password or a card's
details (ADR 0096, ADR 0099; the card's tab refuses text in a password
field); a sign-in page is handed to the person. Kept cookies are never
read by the agent, never sent to the AI service, never shown on screen or
on the terminal (only counts and where), and never offered to a connector
(ADR 0110 § 0 keeps *never through the person's cookies*). They reach only
the Chromium a card opens, which sends them as a browser would.

## 9. What this does not do — cut, and queued

- **A forgetful card or chat** that keeps nothing (a private window): cut,
  queued as 945 — it needs a way to open one and a mark on its card.
- **Forget one site**: queued as 946.
- **Site storage** (`localStorage`, IndexedDB) — a site that keeps its
  sign-in there is signed out at the next card: queued as 947, only if a
  benchmark site (ADR 0110's 934) fails because of it.
- **Partitioned cookies**: queued as 948.
- **The open card told at once of a choice of cookies** is 923's, unchanged.

## Alternatives rejected

- **A kept Chromium profile per site** — each card's Chromium on the site's
  profile: two cards of one site at once cannot both hold it, a sign-in's
  identity provider (`accounts.google.com`) is another site, and it still
  needs `Chromium Safe Storage` and its dialog.
- **Copying a kept profile's `Cookies` database into each new profile** —
  Chromium's own format, sealed under Chromium's own key, which the mock
  keychain does not keep.
- **The jar as plain text beside the chats** — a session token in a file
  any program running as the person can copy (ADR 0011 § 2); sealed it is
  worthless off this keychain.
- **The chats' key** — *Forget all* could not replace it without losing
  every chat.

## Consequences

Built with this ADR (queue item 935): `kept_cookie.rs`, `sign_ins.rs`,
`sign_in_key.rs`, `chrome_sign_ins.rs`, `conversation_sign_ins.rs`;
`chat_seal::seal_as`/`open_as`; `chrome_protocol`'s three `Storage`
commands; `Tab::ask_browser`; `ChromeServing::start_as_a_kept_card`;
`ChromeOpener::start_kept`; *Settings › Privacy*'s third row; `alo
--sign-ins DIR`. ADR 0060's *What this does not decide* and ADR 0110 § 7
are answered here: sign-ins outlive a Chromium, a profile still does not.

# ADR 0091 — The providers' keys are carried inside the application

**Status:** accepted — the owner decided on 31 October 2026; built for the
search key by queue item 748
**Date:** 2026-10-31
**Context:** ADR 0083 § 1 (*In release — left open for the owner*: the key
shipped inside the application, or a small server of alo's own); ADR 0082
§ 0 (the Brave Search API, paid by alo through a subscription); ADR 0081
(the model alo thinks with — proposed; its owner's note said the provider
keys *cannot ship inside the application*); ADR 0067 § 6 and ADR 0079
(`alo-bundle` makes and seals the macOS application); ADR 0005 (a renderer
has no network); queue items 708 and 748

## The question in one line

In a release, how does alo hold the keys it pays its providers with — and
what does that cost?

## 0. The owner's decision

On 31 October 2026, verbatim:

> *"The ai and search apis will be shipped inside the app"*

That settles ADR 0083 § 1 *In release* as its **option 1, the key shipped
inside the application**, for search; and it states the same carrying for
the AI provider's key. **The AI provider itself is not chosen by this**:
ADR 0081 stays proposed for the model, and no AI call is wired (707, 704
stay blocked). Only *how its key travels, once there is one* is decided.

It supersedes, on this one point, ADR 0081's owner's note of 30 October
(*"the provider keys cannot ship inside the application, so requests go
through a service of alo's own that holds them"*). The rest of that note —
a subscription, and what a subscription needs — stands.

## 1. What is decided

- **A release of alo carries each provider's key inside the application**,
  in a named slot: one file per slot at `Contents/Resources/keys/SLOT`.
  Today there is one slot, `search` (the Brave Search API's key). The AI
  slot is **not defined** until ADR 0081 names the provider — a slot names
  the keychain item its key is taken from, and that is the provider's
  (queued, 773). *(Since built, 1 November 2026: the `answer` slot, OpenAI's
  key from the keychain item `alo-openai`, read by the browser process
  alone — queue item 773, ADR 0095 § 2. `--keys-from` takes both.)*
- **The browser process alone reads it**, as the conversation opens
  (`search_key::for_this_application`), and hands it to the one request
  header that carries it, as before (ADR 0083 §§ 2–3). No renderer, no
  Chromium and no agent is ever sent it; it is never written to a log, the
  terminal or a sentence a person reads.
- **A key carried is preferred; a key carried that cannot be read is
  said**, and the keychain is not then asked — an application's own key is
  never quietly swapped for another. An application carrying no key — a
  development build, a test's application, one made without `--keys-from`
  — reads the keychain at run time exactly as ADR 0083 § 1 built it, and
  with no key says *Not set up yet*, as now.
- **A refusal says where to look.** Brave refusing a key (401, 403) now says
  which key: the keychain's (*may be wrong or revoked*, as before) or the
  one this application carries (*may have been revoked: a newer alo carries
  a new one*).

## 2. The risk, plainly

- **Anyone who has the application can read the key out of it.** A
  program's bytes and its resources are the person's to read. The key is
  written masked — XOR a fixed stream, so it is not a plain `strings` hit —
  but the mask is in every copy of `alo` and in this repository's source.
  **Masking slows a casual reader. It does not stop anyone, and it is not
  protection.** Nothing a client can do is: whatever the program can
  unmask to send, a person running it can unmask too, or read off the
  wire before TLS with their own debugger.
- **A key read out can be used from anywhere**, against alo's quota and on
  alo's bill, until it is revoked.
- **Revoking it breaks search in every copy that carries it.** There is no
  way to revoke one copy: every application made with that key stops
  searching at once, and says the key was refused.

The owner chose this knowing it; ADR 0083 § 1's option 2 (a small server of
alo's own holding the key) remains the road if the cost becomes real.

## 3. What follows

- **A key is rotated by a release.** A new key means a new application made
  with it; the old key is revoked once enough people have the new one, and
  copies left behind then say their key was refused.
- **Per-release keys, if cheap.** If the provider lets alo hold several keys
  at once on one account, each release carries its own, so revoking one
  leaked release's key leaves the others searching. Nothing here assumes it:
  the slot holds whatever key it was made with.
- **Spending is watched at the provider**, in its own dashboard and limits:
  alo itself counts nothing and sends no telemetry (ADR 0083 § 3).

## 4. How a key gets in — never through the repository

- **From outside the repository, as the application is made.**
  `alo-bundle --keys-from keychain` takes each slot's key from this Mac's
  login keychain (the search key from the item `alo-brave-search`, for the
  person running it); `alo-bundle --keys-from DIRECTORY` takes each from the
  file in `DIRECTORY` named as its slot. A slot with no key refuses, saying
  which, and nothing is made. The key is put in before the application is
  sealed, so `codesign` refuses an application whose key was changed after.
- **Packaged, not compiled in.** The key is never in a program: it never
  passes through `cargo`, whose dependency files would keep an environment
  variable's value in `target/`, and a release's programs are the same
  bytes whether or not a key is carried.
- **Never** in the repository, a test, a fixture, a log, a commit or a build
  log kept. Tests carry a fake key from a directory of their own; no test
  reads the real keychain item. `alo-bundle` says which slots it carried and
  where from, never a key.

A release is made:

```sh
cargo build --release
CEF_PATH=… cargo build --release --locked \
  --manifest-path crates/alo-cef-host/Cargo.toml --target-dir target/alo-cef-host
CEF_PATH=… target/release/alo-bundle --keys-from keychain
```

## 5. What is not here

- The AI provider's slot (773), and any AI call (ADR 0081, 707, 704).
- Per-release keys, until the provider is read for them.
- Developer ID signing and notarisation (735): the seal today is ad hoc.
- Windows and Linux applications (728), which will carry their keys in
  their own packages.

## Amended (9 November 2026, ADR 0106, queue items 904–906)

What is decided above is now one connector's: the service is asked
through an interface (`search_service.rs`, `ai_service.rs`) and chosen by
configuration, Brave and OpenAI the defaults. A key is the person's own
(given in *Settings*, keychain `alo-search-brave` / `alo-ai-openai`) before
the one a release carries, before the older item this ADR named, which is
still read. No sentence a person reads names the service, OpenAI included
(*the AI service*); ADR 0106 is where that stands now.

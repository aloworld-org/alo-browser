# ADR 0098 — Kept chats are sealed under a key in the person's keychain

**Status:** accepted — built by queue item 790 (6 November 2026)
**Date:** 2026-11-06
**Context:** ADR 0092 (chats are kept on the machine, a file each, in
words — § 2 left them unencrypted and named this item); ADR 0091 (the
providers' keys are carried inside the application, read from the keychain
by `/usr/bin/security`); ADR 0001 (rent the physics); ADR 0010 (whose
`unsafe` a rented crate's is); the owner's approval of 6 November 2026
(*"yes implement the 4 mentioned"*), which named this design: a key in the
person's macOS keychain, made by alo on first use and never printed; a
rented, audited AEAD crate behind one file; plain chats converted once; a
file that will not open reported and left untouched.

## The decision in one line

Every kept chat is **sealed with XChaCha20-Poly1305 under a 256-bit key
alo makes once and keeps in the person's login keychain** (item `alo-chats`);
a chat kept in words before is sealed the first time it is read, its words'
file removed only after the sealed one is on the disk; a file that does not
open is said and left as it is.

## 1. What it protects, and what it does not — in plain words

**It protects** the chats from anyone who has the *files* but not the
person's keychain:

- a copy of `~/Library/Application Support/org.aloworld.Browser/Chats`
  taken off the disk — a backup on another machine, a synced folder, a
  stolen drive without FileVault;
- another user account on the same Mac (the directory is already `0700`,
  so this is a second wall);
- another program running as the same person that can read files but has
  not been given the keychain item — macOS asks the person before a
  program other than the one that made an item reads it.

**It does not protect** against:

- **someone using the person's logged-in account** — they can open alo
  and read the chats in its panel, or ask the keychain for the item with
  the person's own password;
- a program the person allowed to read the keychain item, or one running
  with the person's rights that waits for alo to open a chat and reads
  alo's memory;
- what the chats' words say leaving the machine some other way: a search
  is still asked of Brave and an answer of OpenAI as ADR 0083 and 0095
  say. Sealing is about the file at rest, nothing else.

## 2. The key: made once, in the keychain, never printed

- **Where.** A generic password of the service `alo-chats`, account the
  person's login name (`USER`), labelled *alo: the key the chats on this
  Mac are sealed under*, in the person's default keychain — the login
  keychain. 64 hexadecimal digits: 32 bytes from `/dev/urandom`.
- **Made only when there is none.** alo asks the keychain for it at
  launch; the tool's own *item not found* (exit 44) is the only answer that
  makes one. A keychain that is locked, refuses, or holds something that
  is not 64 hexadecimal digits is said — *chats are not kept: …* — and **no
  key is made**, so a key that exists is never replaced by another
  (replaced, every chat sealed under it would never open again). An
  existing item is never written over (`security` without `-U`), and a
  new one is read back before use, so two launches racing use one key.
- **Never on a command line.** alo reads and makes the item through
  `/usr/bin/security`, as it reads its other keys (ADR 0091): reading with
  `find-generic-password -w`, whose output is taken and nothing else done
  with it; making with `security -i` — the tool's interactive mode — given
  the command **on its standard input**, because any program can read any
  process's arguments with `ps`. No `unsafe`, no Security framework crate.
- **Never printed, logged or written** anywhere but the keychain: `Key`'s
  `Debug` writes `Key(…)`; the terminal says only that a key was made, and
  as which item.
- **Tests never touch the person's keychain.** The store's tests seal with
  a key of their own (`Key::of`); the key's own test makes a keychain
  **file** of its own under the temporary directory (`security
  create-keychain`), makes and reads the item there, and deletes the file.

## 3. If the key is lost, the chats are lost

If the keychain item is deleted, the login keychain is reset (as macOS
offers when a password is forgotten), or the chats' directory is copied to
another Mac without the item, **the sealed chats cannot be read by anyone,
alo included.** There is no second copy of the key and no way to recover
it. alo then makes a new key at the next launch (there is none), and every
file sealed under the old one is said — *it does not open with the key in
this keychain: it was sealed under another key, or changed since* — and
**left on the disk untouched**, its number never given to a new chat, so a
person who finds the old key (a keychain backup) can put it back and read
them. New chats are sealed under the new key.

Keeping a copy of the key elsewhere — exporting it, printing a recovery
code — was not built: every copy is another place it can be taken from,
and the owner asked for the keychain alone.

## 4. The cipher: RustCrypto's XChaCha20-Poly1305, rented, behind one file

- **`chacha20poly1305` =0.10.1** (RustCrypto, `MIT OR Apache-2.0`), default
  features off and `alloc` on — no `getrandom` (the nonce is read from
  `/dev/urandom` with the standard library). Its ChaCha20Poly1305 was
  audited by NCC Group in 2020 with no significant findings. Brings seven
  crates new to the lockfile, all `MIT OR Apache-2.0`, each declaring
  `rust-version` 1.56 or none: `aead` 0.5.2, `chacha20` 0.9.1, `cipher`
  0.4.4, `inout` 0.1.4, `opaque-debug` 0.3.1, `poly1305` 0.8.0,
  `universal-hash` 0.5.1 (`generic-array`, `typenum`, `crypto-common`,
  `subtle`, `zeroize`, `cpufeatures` were here already). Their `unsafe`
  (SIMD back ends, `inout`'s buffers) is theirs, by ADR 0010's line.
- **XChaCha20** rather than ChaCha20 or AES-GCM: its 24-byte nonce can be
  drawn at random for every write with no counter to keep — a chat is
  rewritten at every change, across launches, and a repeated 12-byte nonce
  under one key is the one mistake these ciphers do not survive. AES-GCM
  has the same 12-byte limit and no constant-time software path without
  the CPU's instructions.
- **Behind one file**, `alo-window`'s `chat_seal.rs`, on the gate's
  boundary list. What crosses it is ours: a `Key`, the chat's number, and
  bytes.
- **A sealed file** is `alo sealed chat 1\n`, then the nonce, then the
  chat's words (ADR 0092 § 3's JSON) encrypted, then the 16-byte tag. The
  magic **and the chat's number** are the associated data: a file copied
  over another chat's number, or any changed byte, does not open. The
  file is `chat-N.sealed`; the atomic write of ADR 0092 § 3 is unchanged
  (`.writing`, `fsync`, rename, the directory `fsync`ed).

## 5. Chats kept in words before: sealed once

A `chat-N.json` found at launch is read as ADR 0092 reads it, then sealed:
the sealed file written whole and flushed **first**, and only then the
words' file removed and the directory flushed. Stopped between the two,
the next launch finds both: the sealed one is read, and the words' file
removed only if it says the same chat — otherwise both are left, and said.
A chat that cannot be sealed (a disk full, a directory not writable) is
read from its words as before, said, and sealed at its next write, its
words' file removed then. A words' file that does not read is damaged, as
before, and left.

## 6. Windows and Linux: left open

Elsewhere the key would live in the platform's own store — the Secret
Service on Linux, the Credential Manager or DPAPI on Windows — read
without `unsafe` of ours. Not decided and not built: alo is built and
tested on macOS (ADR 0077), and `chat_key.rs` refuses nowhere else only
because `/usr/bin/security` is not there to start, which is said as
*chats are not kept*. A later item decides it with the platform.

## Consequences

- `alo-window`: `chat_seal.rs` (the cipher, the only file naming it),
  `chat_key.rs` (the keychain item, made or read), `chat_store.rs` (sealed
  files, conversion, `words`, `remove`), `window.rs` (the key read before
  the window opens), `ConversationWindow::key_made`.
- Tests: `chat_seal` (sealed and opened under its key and number alone; a
  fresh nonce each time; a changed byte anywhere not opening; a key never
  printed), `chat_key` (64 hexadecimal digits; no account asked; a key made
  once in a keychain file of the test's own, read back after, another
  account's its own, an item that is not a key said and left), `chat_store`
  (sealed and read back, no word in the clear; a chat in words sealed once
  and its file removed; stopped between the two; sealed under another key
  said and left), and through the window's road
  (`tests/chats_kept_across_launches.rs`: no word of either chat in its
  file; a file that is not sealed said and left). Mutations — the
  chat's number left out of the associated data, the words written
  unsealed — each fail.
- The hand run of 6 November 2026 (STATE.md) confirms the files on disk do
  not hold a searched word, and that a relaunch reads them.

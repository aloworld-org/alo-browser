#!/usr/bin/env bash
# The gate from CLAUDE.md, as a command that fails.
#
# `CLAUDE.md` says nothing is done until all of this passes. A gate that lives
# only in prose is a gate that erodes on a tired afternoon, so the mechanical
# half of it is here and the half that cannot be mechanised is named at the
# bottom rather than quietly dropped.
#
# Run it before every commit:  scripts/gate.sh

set -euo pipefail
cd "$(dirname "$0")/.."

fail=0

# What this verdict is about, printed before any of it.
#
# A measurement must name its own subject. Taken from alo OS, who landed the
# same lesson after five of them and wrote the cure down: their gate had
# silently measured a different checkout, and making the subject a field in
# the result is cheaper than remembering to ask.
#
# It is the same mistake this repository has made twice in one week in the
# supervisor — processor time sampled off the wrong process tree because
# `pgrep -f` matched the measuring command itself, and a worker's progress read
# from a transcript four iterations stale. Both were clean measurements of the
# wrong thing, and both would have been obvious if the number had arrived with
# its subject attached.
subject() {
  local where commit state
  where="$(pwd -P)"
  commit="$(git rev-parse --short HEAD 2>/dev/null || echo 'no commit')"
  if git diff --quiet 2>/dev/null && git diff --cached --quiet 2>/dev/null; then
    state='clean'
  else
    state='UNCOMMITTED CHANGES'
  fi
  printf '\033[1m== the subject of this verdict\033[0m\n'
  printf '   checkout  %s\n' "$where"
  printf '   commit    %s\n' "$commit"
  printf '   tree      %s\n' "$state"
}
subject

# Every step this gate has, counted so that a run which stopped early cannot
# read as one that passed.
#
# alo OS's cure, which is better than the one this file had. Theirs asserts a
# *positive* count — their gate refuses a log that does not hold exactly nine
# passes — and the general form they drew out of it is worth keeping verbatim:
# **assert what must be present, never the absence of what must not.**
#
# This file earned it the same day. A run piped through `head -6` died of
# SIGPIPE at step two of ten; grepping that log for failures found none, so it
# looked exactly like a pass. Naming the subject of a verdict, which this gate
# now does, does not catch that — a truncated log names its subject correctly
# and then stops. Only a count does.
STEPS_DECLARED=11
steps_run=0
step() {
  steps_run=$(( steps_run + 1 ))
  printf '\n\033[1m== %s\033[0m\n' "$1"
}
bad() { printf '\033[31mFAIL\033[0m  %s\n' "$1"; fail=1; }
good() { printf '\033[32mok\033[0m    %s\n' "$1"; }

# --- Rented crates, and the files each may be named in ----------------------
#
# ADR 0001 rents the physics and writes the engine. A rented crate that leaks
# past its boundary is how "we hold the tree" turns into "they hold the tree"
# without anybody deciding to. The files allowed to name each one are listed
# here, separated by commas, and adding to a list is a decision somebody makes
# in a diff rather than by writing an import.
#
# `html5ever` is one file because an HTML parser is used once, at the door. A
# CSS tokeniser is not: selector text, media conditions and declaration values
# are all read from the same token stream, so `cssparser` is named wherever CSS
# text is read, and `selectors` wherever a selector is built or run. What holds
# for both is that no type of theirs appears in `alo-css`'s public API.
#
# `serde_json` is three files because there are three protocols, each read
# from a stranger: the agent's lines on its socket, Chrome's messages on its
# pipe (ADR 0058), and the search provider's answers (ADR 0083). Each is a
# reason to change of its own, so none is another's.
#
# Prose may say the name anywhere — comments are stripped before this looks.
declare -a BOUNDARIES=(
  "html5ever:crates/alo-dom/src/parse.rs"
  "quick_xml:crates/alo-dom/src/xml.rs"
  "cssparser:crates/alo-css/src/ident.rs,crates/alo-css/src/matching.rs,crates/alo-css/src/media.rs,crates/alo-css/src/parse.rs,crates/alo-css/src/selector.rs,crates/alo-style/src/variables.rs,crates/alo-value/src/parse.rs"
  "selectors:crates/alo-css/src/matching.rs,crates/alo-css/src/parse.rs,crates/alo-css/src/selector.rs"
  "taffy:crates/alo-layout/src/arena.rs,crates/alo-layout/src/auto_repeat.rs,crates/alo-layout/src/engine.rs"
  "rustybuzz:crates/alo-text/src/font.rs,crates/alo-text/src/shape.rs"
  "unicode_linebreak:crates/alo-text/src/linebreak.rs"
  "icu_casemap:crates/alo-box/src/case.rs"
  "icu_locale_core:crates/alo-box/src/case.rs"
  "ttf_parser:crates/alo-paint/src/glyph.rs,crates/alo-text/src/font.rs"
  "tiny_skia:crates/alo-paint/src/raster.rs"
  "png:crates/alo-paint/src/encode.rs"
  "jpeg_decoder:crates/alo-paint/src/jpeg_picture.rs"
  "gif:crates/alo-paint/src/gif_picture.rs"
  "image_webp:crates/alo-paint/src/webp_picture.rs"
  "url:crates/alo-url/src/parse.rs"
  "psl:crates/alo-url/src/site.rs"
  "encoding_rs:crates/alo-net/src/encoding.rs,crates/alo-text/src/macintosh.rs"
  "rustls:crates/alo-net/src/tls.rs"
  "flate2:crates/alo-net/src/decompress.rs"
  "brotli_decompressor:crates/alo-net/src/decompress.rs"
  "ruzstd:crates/alo-net/src/decompress.rs"
  "sha2:crates/alo-net/src/digest.rs"
  "unicode_id_start:crates/alo-js/src/unicode.rs"
  "winit:crates/alo-window/src/window.rs"
  "softbuffer:crates/alo-window/src/present.rs"
  "raw_window_handle:crates/alo-window/src/present.rs"
  "rustix:crates/alo-storage/src/volume.rs"
  "serde_json:crates/alo-window/src/protocol.rs,crates/alo-window/src/chrome_protocol.rs,crates/alo-window/src/search_protocol.rs"
  # alo's CEF host (ADR 0079 § 3, option H), its own workspace: CEF where it
  # is loaded and run, where the host's windows and pages are, and where its
  # application object answers CEF's protocols; AppKit only in the
  # application object and its menu bar (ADR 0080).
  "cef:crates/alo-cef-host/src/host.rs,crates/alo-cef-host/src/window.rs,crates/alo-cef-host/src/application_mac.rs"
  "objc2:crates/alo-cef-host/src/application_mac.rs,crates/alo-cef-host/src/menu_mac.rs"
  "objc2_app_kit:crates/alo-cef-host/src/application_mac.rs,crates/alo-cef-host/src/menu_mac.rs"
  "objc2_foundation:crates/alo-cef-host/src/application_mac.rs,crates/alo-cef-host/src/menu_mac.rs"
)

# --- alo's CEF host, and the one `unsafe` boundary it holds -------------------
#
# The host (`crates/alo-cef-host`) is its own workspace, outside the
# repository's, because `cef-dll-sys` downloads CEF when `CEF_PATH` names no
# unpacked distribution (queue item 725): so the steps above never build it,
# and the one below does, against `CEF_PATH`, when it names one.
#
# ADR 0080 allows it one `unsafe` boundary, on macOS: the `NSApplication`
# subclass CEF requires, in one file. The `unsafe_code` lint is lifted on that
# one module, in `main.rs`, which denies it everywhere else; and the `cef`
# crate's `wrap_*` macros, whose expansion clippy reads, need one clippy lint
# expected, in `window.rs` alone. Each is checked to be there and nowhere
# else, and the workspace's crates to write no `unsafe` at all.
HOST=crates/alo-cef-host
HOST_UNSAFE="$HOST/src/application_mac.rs"
HOST_LIFTED="$HOST/src/main.rs"
HOST_TRANSMUTE="$HOST/src/window.rs"
# A CEF distribution unpacked where `CEF_PATH` names, as the host's tests look
# for one: its framework, and the archive's description beside it.
cef_distribution() {
  [ -n "${CEF_PATH:-}" ] \
    && [ -d "$CEF_PATH/Chromium Embedded Framework.framework" ] \
    && [ -f "$CEF_PATH/archive.json" ]
}
# A file's lines as code: comment lines blanked, as for the boundaries.
code_of() { sed -E 's@^[[:space:]]*(//!|///|//).*$@@' "$1"; }

step "cargo fmt"
if cargo fmt --all -- --check >/dev/null 2>&1; then
  good "formatting is clean"
else
  cargo fmt --all -- --check || true
  bad "cargo fmt --all -- --check"
fi

step "cargo clippy — zero warnings and zero errors"
if cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/alo-clippy.log | tail -n 3; then
  if grep -qE '^(error|warning)' /tmp/alo-clippy.log; then
    bad "clippy said something"
  else
    good "clippy is silent"
  fi
else
  bad "clippy failed"
fi

step "the tests, and the doctests with them"
# `cargo nextest` runs each test in its own process rather than threading them
# inside one binary. Measured on this workspace on 2026-10-13, both warm and
# under the same load: **18.9 minutes against `cargo test`'s 27.9** — a third
# off, with not one test skipped.
#
# Not one *test*. Doctests are the exception: nextest does not run them, and
# this repository has some, two of them `compile_fail` — which assert that
# code must **not** compile, and are exactly the kind of thing a faster runner
# would silently stop checking. So both are run. The doctests cost six
# seconds, so "both" is free and "either" would be a weakened gate.
#
# What it was worth, measured rather than projected: the test step went from
# 27.9 minutes to 19.0, but the **gate** went from 13.5 to 12.3 — nine per
# cent, not the thirty the step's own saving suggested. Tests are not the bulk
# of this gate; clippy and the rest are a larger share than they look. Worth
# keeping, and worth remembering which number was measured and which inferred.
#
# **A failing run prints why it failed, not only that it did.** Until
# 2026-10-25 both commands were piped through `tail -n 20`, which keeps the
# summary and throws the panic away — so every failure said which test and
# nothing else, and each one had to be reproduced by hand before it could be
# read. One of them could not be reproduced at all and stopped the loop twice
# in a night. So the output is kept, a passing run prints its tail as before,
# and a failing one prints the part with the failures in it. This makes the
# gate no weaker: the same commands decide it, and only what is *shown*
# changed.
RUN_LOG="${TMPDIR:-/tmp}/alo-gate-tests.$$.log"
trap 'rm -f "$RUN_LOG"' EXIT
# What a reader needs from a failing run: the failures, with their output.
#
# Not `tail`. A run of five thousand tests prints a line per test, so a
# failure early in it is thousands of lines from the end — which is how the
# panic went missing even when the whole log was kept. So the failures are
# found by what they say and printed with the lines around them, and the
# summary is printed too because it says how many there were.
say_why() {
  grep -n -B2 -A25 -E 'panicked at|--- STDERR|stderr ───|^ *FAIL|^ *TIMEOUT|^ *SIGSEGV|test result: FAILED' "$1" \
    | head -n 250
  printf '   ─────\n'
  tail -n 12 "$1"
}
if ! command -v cargo-nextest >/dev/null; then
  bad "cargo-nextest is not installed — cargo install cargo-nextest"
elif cargo nextest run --workspace --all-features > "$RUN_LOG" 2>&1; then
  tail -n 20 "$RUN_LOG"
  if cargo test --workspace --all-features --doc > "$RUN_LOG" 2>&1; then
    tail -n 20 "$RUN_LOG"
    good "tests pass, and the doctests with them"
  else
    tail -n 60 "$RUN_LOG"
    bad "doctests fail"
  fi
else
  say_why "$RUN_LOG"
  bad "tests fail"
fi

step "alo's CEF host — formatted, linted and tested in its own workspace"
# Formatting needs no CEF, so it is checked on every run. Building needs a
# distribution, and without one the host is **not built, and this says so**:
# the steps above never reached it either, so nothing it checked is lost.
if (cd "$HOST" && cargo fmt -- --check) >/dev/null 2>&1; then
  good "the CEF host's formatting is clean"
else
  (cd "$HOST" && cargo fmt -- --check) || true
  bad "cargo fmt -- --check in $HOST"
fi
if cef_distribution; then
  HOST_LOG="${TMPDIR:-/tmp}/alo-gate-host.$$.log"
  if cargo clippy --manifest-path "$HOST/Cargo.toml" --locked --target-dir target/alo-cef-host \
    --all-targets -- -D warnings > "$HOST_LOG" 2>&1; then
    if grep -qE '^(error|warning)' "$HOST_LOG"; then
      cat "$HOST_LOG"
      bad "clippy said something about the CEF host"
    else
      good "clippy is silent about the CEF host, built against $CEF_PATH"
    fi
  else
    tail -n 60 "$HOST_LOG"
    bad "clippy failed on the CEF host"
  fi
  if cargo test --manifest-path "$HOST/Cargo.toml" --locked --target-dir target/alo-cef-host \
    > "$HOST_LOG" 2>&1; then
    grep -E '^test result' "$HOST_LOG" || true
    good "the CEF host's tests pass"
  else
    say_why "$HOST_LOG"
    bad "the CEF host's tests fail"
  fi
  rm -f "$HOST_LOG"
else
  printf '\033[33mNOT BUILT\033[0m  the CEF host was not built, linted or tested: CEF_PATH names no unpacked CEF distribution (%s)\n' \
    "${CEF_PATH:-unset}"
fi

step "no stubs"
# Law 3 of LOOP.md: built whole. A stub is a promise the gate cannot check.
# Binary files are skipped (-I): a reference picture cannot hold a stub, and
# its compressed bytes once spelt XXX by chance (queue item 711).
if stubs=$(grep -rnIE '\b(todo!|unimplemented!|FIXME|XXX)' crates/*/src crates/*/tests 2>/dev/null); then
  echo "$stubs"
  bad "a stub or an unfinished note is in the tree"
else
  good "nothing is stubbed"
fi

step "no unsafe without an ADR"
# The workspace forbids it at the compiler, so this catches only a manifest
# that has quietly relaxed the lint.
if relaxed=$(grep -rn 'unsafe_code' crates/*/Cargo.toml 2>/dev/null); then
  echo "$relaxed"
  bad "a crate has opted out of the workspace's unsafe ban — it needs an ADR"
else
  good "unsafe is forbidden everywhere"
fi
# Nor in any source of the workspace's crates: an `unsafe` block, function,
# implementation, trait, extern block or attribute, the lint named at all, or
# the clippy lint the CEF host's macros need — the compiler already refuses
# the first two, and this says so without building. The words a page may say
# (`'unsafe-inline'`) are not Rust's, and are not matched.
written=""
while IFS= read -r file; do
  if found=$(code_of "$file" | grep -nE '(^|[^A-Za-z0-9_])unsafe[[:space:]]*(\{|\(|fn([^A-Za-z0-9_]|$)|impl([^A-Za-z0-9_]|$)|trait([^A-Za-z0-9_]|$)|extern([^A-Za-z0-9_]|$))|unsafe_code|transmute_ptr_to_ptr'); then
    written="$written$(printf '%s\n' "$found" | sed "s@^@  $file:@")"$'\n'
  fi
done < <(find crates -name '*.rs' -not -path '*/target/*' -not -path "$HOST/*")
if [ -n "$written" ]; then
  printf '%s' "$written"
  bad "unsafe is written in a crate of the workspace — it needs an ADR"
else
  good "no crate of the workspace writes unsafe"
fi
# The host's one boundary (ADR 0080): `unsafe` in the application object's
# file and no other; the lint named twice in the whole program, both in
# `main.rs` — denied for all of it, and lifted on the one attribute whose item
# is that file's module; the clippy lint the `cef` crate's macros need,
# expected in `window.rs` and nowhere else. Comments may name any of them.
outside=""
named=""
transmuted=""
while IFS= read -r file; do
  if [ "$file" != "$HOST_UNSAFE" ] \
    && found=$(code_of "$file" | grep -nE '(^|[^A-Za-z0-9_])unsafe([^A-Za-z0-9_]|$)'); then
    outside="$outside$(printf '%s\n' "$found" | sed "s@^@  $file:@")"$'\n'
  fi
  if found=$(code_of "$file" | grep -n 'unsafe_code'); then
    named="$named$(printf '%s\n' "$found" | sed "s@^@$file:@")"$'\n'
  fi
  if code_of "$file" | grep -q 'transmute_ptr_to_ptr'; then
    transmuted="$transmuted $file"
  fi
done < <(find "$HOST" -name '*.rs' -not -path '*/target/*' | sort)
if [ -n "$outside" ]; then
  printf '%s' "$outside"
  bad "unsafe is written in the CEF host outside $HOST_UNSAFE (ADR 0080)"
elif ! code_of "$HOST_UNSAFE" | grep -qE '(^|[^A-Za-z0-9_])unsafe([^A-Za-z0-9_]|$)'; then
  bad "$HOST_UNSAFE holds no unsafe — ADR 0080 and this check name the wrong file"
else
  good "the CEF host writes unsafe only in $HOST_UNSAFE (ADR 0080)"
fi
# The item the lint's second naming is an attribute of: the first line after
# it that is neither the attribute's own nor blank.
lifted_on=$(code_of "$HOST_LIFTED" | awk '
  /unsafe_code/ && !/^#!\[(deny|forbid)\(unsafe_code\)\]$/ { after = 1; next }
  after && /^[[:space:]]*(reason|\)\]|$)/ { next }
  after { print; exit }')
denied=$(printf '%s' "$named" | grep -cE "^$HOST_LIFTED:[0-9]+:#!\[(deny|forbid)\(unsafe_code\)\]$" || true)
in_main=$(printf '%s' "$named" | grep -c "^$HOST_LIFTED:" || true)
all=$(printf '%s' "$named" | grep -c . || true)
if [ "$denied" -ne 1 ]; then
  printf '%s' "$named"
  bad "$HOST_LIFTED does not deny unsafe_code for the whole CEF host"
elif [ "$all" -ne 2 ] || [ "$in_main" -ne 2 ] || [ "$lifted_on" != "mod application_mac;" ]; then
  printf '%s' "$named"
  bad "unsafe_code may be lifted once in the CEF host, on mod application_mac in $HOST_LIFTED (ADR 0080)"
else
  good "the CEF host denies unsafe_code, lifted on mod application_mac alone"
fi
if [ "$transmuted" != " $HOST_TRANSMUTE" ]; then
  printf '  %s\n' $transmuted
  bad "clippy::transmute_ptr_to_ptr may be expected in $HOST_TRANSMUTE alone (ADR 0080)"
else
  good "the cef crate's macros' one clippy lint is expected in $HOST_TRANSMUTE alone"
fi

step "every source file carries the licence notice"
# ADR 0009 relicensed this engine to MPL-2.0, which is copyleft *per file* — so
# which licence a file is under is a property of the file, and a recipient who
# has only that file must be able to read it there. The root `LICENSE` is the
# fallback the licence permits; the notice is the answer.
#
# It is checked rather than remembered because a header nobody checks is a
# header that stops being true: 198 files got it in one commit and the
# one-hundred-and-ninety-ninth is written by somebody who never saw that commit.
notice=$'/* This Source Code Form is subject to the terms of the Mozilla Public\n * License, v. 2.0. If a copy of the MPL was not distributed with this\n * file, You can obtain one at http://mozilla.org/MPL/2.0/. */'
bare=""
while IFS= read -r file; do
  [ "$(head -n 3 "$file")" = "$notice" ] || bare="$bare $file"
done < <(find crates -name '*.rs' -not -path '*/target/*')
if [ -n "$bare" ]; then
  for file in $bare; do echo "  $file"; done
  printf 'Put these three lines at the top of each, followed by a blank line:\n\n%s\n\n' "$notice"
  bad "a source file does not say what licence it is under (ADR 0009)"
else
  good "every source file carries Exhibit A"
fi

step "rented crates stay behind their boundary"
for entry in "${BOUNDARIES[@]}"; do
  crate="${entry%%:*}"
  allowed="${entry#*:}"
  offenders=""
  while IFS= read -r file; do
    case ",$allowed," in *",$file,"*) continue ;; esac
    # Strip comment lines: naming a rented crate in prose is fine and good.
    # What counts as naming it is a path — `crate::Thing` or `use crate` — and
    # not a field that happens to share the word, which is why `rule.selectors`
    # is not a boundary violation.
    if sed -E 's@^[[:space:]]*(//!|///|//).*$@@' "$file" \
      | grep -qE "(^|[^A-Za-z0-9_.])${crate}::|^[[:space:]]*use[[:space:]]+${crate}[[:space:]{:;]"; then
      offenders="$offenders $file"
    fi
  done < <(find crates -name '*.rs' -not -path '*/target/*')
  if [ -n "$offenders" ]; then
    echo "  $crate is named in:$offenders"
    bad "$crate may only be named in $allowed (ADR 0001)"
  else
    good "$crate stays behind its boundary"
  fi
done

step "no verb takes a coordinate"
# ADR 0002: *"No verb takes a coordinate, because a coordinate is a guess about
# a layout that may have changed between the reading and the acting."* The
# agent surface is where that rule lives, so it is checkable there: no function
# in it may take a point, or an `x` and a `y`.
if [ -d crates/alo-agent/src ]; then
  if coordinates=$(sed -E 's@^[[:space:]]*(//!|///|//).*$@@' crates/alo-agent/src/*.rs \
    | grep -nE 'fn [a-z_]+\(.*(\bx: *f(32|64)|\by: *f(32|64)|Point)' ); then
    echo "$coordinates"
    bad "a function in the agent surface takes a coordinate (ADR 0002)"
  else
    good "the agent surface names things rather than pointing at them"
  fi
else
  good "no agent surface yet"
fi

step "the supervisor's stop rule holds"
# The one piece of logic in `scripts/loop.sh` (ADR 0006): whether the journal is
# telling the loop to stop *now*. Getting it wrong is the failure that looks
# exactly like success — a supervisor that reads a stale marker stops on its
# first tick and reports the queue complete with ninety-nine items open, which
# is what the journal's own `LOOP COMPLETE` from the end of stage 1 caused.
if [ -x scripts/loop.sh ]; then
  if { ./scripts/loop.sh --self-test && ./scripts/test-loop.sh; } >/tmp/alo-gate-loop.log 2>&1; then
    good "stop markers, worker failures, locks and preserved work hold"
  else
    cat /tmp/alo-gate-loop.log
    bad "the supervisor would start or stop at the wrong time"
  fi
else
  bad "scripts/loop.sh is missing or not executable"
fi

step "documentation changed with the code"
# Only meaningful while there is something uncommitted to judge.
if [ -n "$(git status --porcelain -- crates 2>/dev/null)" ]; then
  if [ -z "$(git status --porcelain -- CHANGELOG.md 2>/dev/null)" ]; then
    bad "crates changed and CHANGELOG.md did not"
  else
    good "CHANGELOG.md changed with the code"
  fi
else
  good "no uncommitted code to judge"
fi

printf '\n'
cat <<'NOTE'
What this cannot check, and a person must:

  - One file, one responsibility. A file that gained a second reason to change
    gets split in the change that discovered it.
  - That a verb's *meaning* does not depend on a position, which the check
    above cannot see: it only looks at what a function takes.
  - A layout assertion in numbers for anything that positions or sizes.
  - A reference render for anything visual.
  - That an item is in docs/features.md before it is built.
  - That a tick means done rather than written.

NOTE

if [ "$steps_run" -ne "$STEPS_DECLARED" ]; then
  printf '\033[31mFAIL\033[0m  %s\n' \
    "$steps_run of $STEPS_DECLARED steps ran — this run did not finish, and a \
run that did not finish is not a pass"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  printf '\033[31mThe gate is not met.\033[0m  (%s of %s steps)\n' \
    "$steps_run" "$STEPS_DECLARED"
  exit 1
fi
printf '\033[32mThe gate is met.\033[0m  (%s of %s steps)\n' \
  "$steps_run" "$STEPS_DECLARED"

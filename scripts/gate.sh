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
step() { printf '\n\033[1m== %s\033[0m\n' "$1"; }
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
# Prose may say the name anywhere — comments are stripped before this looks.
declare -a BOUNDARIES=(
  "html5ever:crates/alo-dom/src/parse.rs"
  "cssparser:crates/alo-css/src/ident.rs,crates/alo-css/src/matching.rs,crates/alo-css/src/media.rs,crates/alo-css/src/parse.rs,crates/alo-css/src/selector.rs,crates/alo-style/src/variables.rs,crates/alo-value/src/parse.rs"
  "selectors:crates/alo-css/src/matching.rs,crates/alo-css/src/parse.rs,crates/alo-css/src/selector.rs"
  "taffy:crates/alo-layout/src/arena.rs,crates/alo-layout/src/engine.rs"
  "rustybuzz:crates/alo-text/src/font.rs,crates/alo-text/src/shape.rs"
  "unicode_linebreak:crates/alo-text/src/linebreak.rs"
  "ttf_parser:crates/alo-paint/src/glyph.rs,crates/alo-text/src/font.rs"
  "tiny_skia:crates/alo-paint/src/raster.rs"
  "png:crates/alo-paint/src/encode.rs"
  "jpeg_decoder:crates/alo-paint/src/picture.rs"
  "url:crates/alo-url/src/parse.rs"
  "psl:crates/alo-url/src/site.rs"
  "encoding_rs:crates/alo-net/src/encoding.rs"
  "rustls:crates/alo-net/src/tls.rs"
  "flate2:crates/alo-net/src/decompress.rs"
  "brotli_decompressor:crates/alo-net/src/decompress.rs"
  "ruzstd:crates/alo-net/src/decompress.rs"
)

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

step "cargo test"
if cargo test --workspace --all-features 2>&1 | tail -n 20; then
  good "tests pass"
else
  bad "tests fail"
fi

step "no stubs"
# Law 3 of LOOP.md: built whole. A stub is a promise the gate cannot check.
if stubs=$(grep -rnE '\b(todo!|unimplemented!|FIXME|XXX)' crates/*/src crates/*/tests 2>/dev/null); then
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
  if ./scripts/loop.sh --self-test >/tmp/alo-gate-loop.log 2>&1; then
    good "a stale marker is history and a live one stops the loop"
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

if [ "$fail" -ne 0 ]; then
  printf '\033[31mThe gate is not met.\033[0m\n'
  exit 1
fi
printf '\033[32mThe gate is met.\033[0m\n'

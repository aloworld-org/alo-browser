#!/usr/bin/env bash
# scripts/loop.sh — the build loop's supervisor, for macOS (ADR 0006).
#
# One `claude` invocation per queue item, until the journal says to stop.
# `docs/autonomy/LOOP.md` is what an iteration reads; this file only decides
# when to start one and when to stop starting them.
#
#   scripts/loop.sh                 # run until the journal says stop
#   scripts/loop.sh --once          # a single iteration, then exit
#   scripts/loop.sh --dry-run       # say what it would do, start nothing
#   scripts/loop.sh --self-test     # check the stop-marker rule, start nothing
#
# Ctrl+C is always safe. Every finished item was committed and pushed by the
# iteration that built it, so interrupting one loses at most the item in
# progress, which the next iteration redoes from the queue.

set -uo pipefail
cd "$(dirname "$0")/.."

JOURNAL="docs/autonomy/STATE.md"
QUEUE="docs/autonomy/QUEUE.md"
PROMPT="Read docs/autonomy/LOOP.md and execute exactly ONE iteration of the build loop, then exit."

# How long a worker may be *silent* before it is presumed hung, and the
# absolute ceiling regardless of how busy it looks. Idle rather than duration,
# because a hung worker stops writing to its transcript while an honest long
# item keeps writing to it — a duration-only guard in the script this replaces
# once killed ninety minutes of real work (ADR 0006).
IDLE_KILL_MIN="${IDLE_KILL_MIN:-20}"
CEILING_MIN="${CEILING_MIN:-240}"
MAX_ITERATIONS="${MAX_ITERATIONS:-500}"
BACKOFF_MIN="${BACKOFF_MIN:-15}"

once=0
dry=0
selftest=0
for arg in "$@"; do
  case "$arg" in
    --once) once=1 ;;
    --dry-run) dry=1 ;;
    --self-test) selftest=1 ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "loop: unknown argument $arg — try --help"; exit 2 ;;
  esac
done

say()  { printf '\033[1m[loop]\033[0m %s\n' "$1"; }
bad()  { printf '\033[31m[loop]\033[0m %s\n' "$1"; }

# --- Before anything: is this a tree an iteration should open on? ------------

[ -f "$JOURNAL" ] || { bad "no journal at $JOURNAL — is this the alo-browser checkout?"; exit 2; }
[ -f "$QUEUE" ]   || { bad "no queue at $QUEUE";   exit 2; }
command -v claude >/dev/null || { bad "no \`claude\` on PATH — nothing could run an iteration"; exit 2; }

# Whether the journal says to stop *now*, and which way.
#
# Two things this has to get right, and the second one is not obvious.
#
# The pattern is anchored to the start of a line and tolerates a heading or bold
# prefix, because the journal quotes both markers in its own prose — always
# behind a bullet or a backtick, never starting a line. An unanchored match once
# stopped a loop with 58 items open while reporting success.
#
# And **only the last marker counts, and only if nothing came after it.** The
# journal is append-only and a marker is a record of a decision that was true
# when it was written. This one has `LOOP COMPLETE` at line 1531 of 2500-odd:
# stage 1 finished, said so, and stage 2 was started afterwards by a person. A
# supervisor that greps the whole file finds that, stops on its first tick, and
# reports the queue complete with ninety-nine items open — which is the failure
# that looks exactly like the work being done. So: a marker is live only when no
# iteration entry follows it.
stop_marker() {
  local marker iteration kind
  marker=$(grep -nE '^#{0,6} *\*{0,2}LOOP (COMPLETE|HALT)' "$JOURNAL" 2>/dev/null | tail -1)
  [ -n "$marker" ] || { echo ""; return; }
  iteration=$(grep -nE '^#{1,6} *Iteration ' "$JOURNAL" 2>/dev/null | tail -1 | cut -d: -f1)
  iteration="${iteration:-0}"
  # An entry written after the marker means the loop was deliberately resumed,
  # so the marker is history rather than an instruction.
  [ "${marker%%:*}" -gt "$iteration" ] || { echo ""; return; }
  kind=$(grep -oE 'LOOP (COMPLETE|HALT)' <<<"$marker" | head -1)
  echo "${kind#LOOP }"
}

open_items() { grep -c '^- \[ \]' "$QUEUE" 2>/dev/null || echo 0; }

# --- The stop-marker rule, as assertions -------------------------------------
#
# The gate asks for unit tests for logic, and marker detection is the only
# logic in this file — everything else is spawning a process and watching a
# clock. This is what a test looks like in bash: fixtures in, decision out.
if [ "${selftest:-0}" -eq 1 ]; then
  failures=0
  check() {
    local name="$1" want="$2" body="$3" got
    JOURNAL="$(mktemp)"; printf '%s\n' "$body" > "$JOURNAL"
    got="$(stop_marker)"
    if [ "$got" = "$want" ]; then
      printf '\033[32mok\033[0m    %s\n' "$name"
    else
      printf '\033[31mFAIL\033[0m  %s — wanted %s, got %s\n' "$name" "${want:-none}" "${got:-none}"
      failures=1
    fi
    rm -f "$JOURNAL"
  }

  check "a plain marker stops the loop" "COMPLETE" \
    "## Iteration 1
did a thing

LOOP COMPLETE"

  check "a marker written as a heading still stops it" "HALT" \
    "## Iteration 1

## LOOP HALT: the gate is wrong"

  check "a marker written in bold still stops it" "COMPLETE" \
    "## Iteration 1

**LOOP COMPLETE** — every item is [x]"

  check "the journal quoting a marker mid-sentence does not stop it" "" \
    "## Iteration 1
- **Next:** LOOP COMPLETE — when every item is checked
  and see LOOP.md on when LOOP HALT is the right answer"

  check "a marker an iteration was written after is history, not an instruction" "" \
    "## Iteration 12
stage 1 finished

LOOP COMPLETE

## Iteration 13 — stage 2 began
a person restarted this"

  check "the last marker wins over an earlier one" "HALT" \
    "## Iteration 1

LOOP COMPLETE

## Iteration 2

LOOP HALT"

  check "no marker at all runs" "" "## Iteration 1
nothing to report"

  [ "$failures" -eq 0 ] && printf '\n\033[32m[loop]\033[0m the stop-marker rule holds.\n'
  exit "$failures"
fi

if [ "$dry" -eq 1 ]; then
  say "would run:  claude -p \"\$PROMPT\" --dangerously-skip-permissions"
  marker="$(stop_marker)"
  say "journal:    $JOURNAL  (stop marker: ${marker:-none})"
  say "queue:      $(open_items) items still open"
  say "guards:     silent for ${IDLE_KILL_MIN}m, or ${CEILING_MIN}m total; ${MAX_ITERATIONS} iterations at most"
  exit 0
fi

# The gate, before the first iteration only. An iteration that opens on
# somebody else's red tree will either work around the failure or spend itself
# diagnosing it, and both are worse than not starting (ADR 0006).
say "checking the tree is green before starting…"
if ! ./scripts/gate.sh >/tmp/alo-loop-gate.log 2>&1; then
  bad "the gate does not pass on this tree, so no iteration will start."
  bad "the loop may never work around a failure it did not cause — LOOP.md."
  tail -20 /tmp/alo-loop-gate.log
  exit 4
fi
say "the gate is met. $(open_items) queue items open."

# One supervisor per checkout, machine-wide. Stopped wrappers have survived as
# detached processes and spawned rival workers editing the same files, so a
# live owner is refused and a dead one is taken over (ADR 0006).
LOCK="$HOME/.alo-browser-loop.lock"
if [ -f "$LOCK" ]; then
  owner="$(cat "$LOCK" 2>/dev/null || true)"
  if [ -n "$owner" ] && kill -0 "$owner" 2>/dev/null; then
    bad "another supervisor (PID $owner) already owns this checkout — refusing to start."
    bad "if it is truly dead: rm $LOCK"
    exit 3
  fi
  say "stale lock from dead PID $owner — taking over."
fi

echo $$ > "$LOCK"
trap 'rm -f "$LOCK"; say "stopped."' EXIT

for (( i = 1; i <= MAX_ITERATIONS; i++ )); do
  case "$(stop_marker)" in
    COMPLETE)
      echo
      say "the journal says LOOP COMPLETE — stopping, and not restarting."
      say "what is left is a person's: LOOP.md's stage boundaries say so."
      say "read the last entry in $JOURNAL for what it is asking you to decide."
      exit 0 ;;
    HALT)
      echo
      bad "the journal says LOOP HALT — something is wrong that the loop must not work around."
      bad "read the last entry in $JOURNAL, fix the reason, remove the marker, start again."
      exit 5 ;;
  esac

  printf '\n\033[1m%s\033[0m\n' "════════════════════════════════════════════════════════"
  say "iteration $i  ·  $(date '+%Y-%m-%d %H:%M')  ·  $(open_items) items open"

  git pull --rebase origin main >/dev/null 2>&1 || say "could not pull — working from the local tree."

  # macOS `stat -f %m`. The script this replaces needed a GNU fallback and was
  # bitten by it; this one is for one platform and says so (ADR 0006).
  transcripts="$HOME/.claude/projects/$(pwd | sed 's#[/: ]#-#g')"
  started=$(date +%s)

  claude -p "$PROMPT" --dangerously-skip-permissions &
  worker=$!
  code=""

  while kill -0 "$worker" 2>/dev/null; do
    sleep 30
    now=$(date +%s)
    newest=$(find "$transcripts" -name '*.jsonl' -exec stat -f %m {} \; 2>/dev/null \
             | grep -E '^[0-9]+$' | sort -rn | head -1)
    # The first two minutes are grace: the worker has not opened a transcript
    # yet, and an empty answer here is not silence.
    if [ -z "$newest" ] || [ "$newest" -lt "$started" ]; then newest=$started; fi
    idle=$(( now - newest ))
    running=$(( now - started ))

    why=""
    [ "$idle" -ge $(( IDLE_KILL_MIN * 60 )) ] && why="silent for $(( idle / 60 )) minutes"
    [ "$running" -ge $(( CEILING_MIN * 60 )) ] && why="past the ${CEILING_MIN}-minute ceiling"
    if [ -n "$why" ]; then
      bad "killing the worker — $why."
      kill -TERM "$worker" 2>/dev/null; sleep 10; kill -KILL "$worker" 2>/dev/null
      # Leave a clean tree for the next iteration. Anything finished was
      # already committed; what is dropped here is a half-built item.
      git rebase --abort >/dev/null 2>&1
      git checkout -- . >/dev/null 2>&1
      code=124
      break
    fi
  done

  if [ -z "$code" ]; then wait "$worker"; code=$?; else wait "$worker" 2>/dev/null || true; fi

  if [ "$once" -eq 1 ]; then say "--once: one iteration, done."; exit 0; fi

  if [ "$code" -eq 124 ]; then
    say "the hang already cost time — going again in 30 seconds."
    sleep 30
  elif [ "$code" -ne 0 ]; then
    # Almost always a rate limit. Restarting straight into one spends a model
    # call to be told to wait.
    bad "iteration exited $code — waiting ${BACKOFF_MIN} minutes rather than spinning."
    sleep $(( BACKOFF_MIN * 60 ))
  fi
done

bad "stopped after $MAX_ITERATIONS iterations without the journal saying to."
bad "that is a backstop, not a result — read $JOURNAL before restarting."
exit 6

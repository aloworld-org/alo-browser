#!/usr/bin/env bash
# scripts/loop.sh — the build loop's supervisor, for macOS (ADR 0006).
#
# One `codex exec` invocation per queue item, until the journal says to stop.
# `docs/autonomy/LOOP.md` is what an iteration reads; this file only decides
# when to start one and when to stop starting them.
#
#   scripts/loop.sh                 # run until the journal says stop
#   scripts/loop.sh --once          # a single iteration, then exit
#   scripts/loop.sh --items 5       # five iterations, then exit
#   scripts/loop.sh --dry-run       # say what it would do, start nothing
#   scripts/loop.sh --self-test     # check the stop-marker rule, start nothing
#
# Everything it does is written to docs/autonomy/loop.log as well as to the
# terminal, so a run you walked away from is a run you can still read.
#
# Finished items are local commits. Interruptions preserve uncommitted work;
# inspect it before resuming. Publication is a separate, explicit action.

set -uo pipefail
cd "$(dirname "$0")/.."

JOURNAL="docs/autonomy/STATE.md"
QUEUE="docs/autonomy/QUEUE.md"
PROMPT="Follow all applicable system, developer and user instructions and repository rules. Before choosing or changing anything, read applicable AGENTS.md files, CLAUDE.md (the repository constitution), docs/autonomy/LOOP.md, ROADMAP.md, and the latest journal entry; read the selected item’s relevant ADRs and feature contract. Execute exactly ONE eligible iteration. Treat every applicable rule as mandatory: never weaken the gate, disable tests, add stubs, bypass dependency or stage gates, or claim unverified completion. Preserve unfinished work and keep sibling repositories read-only. Before committing, review compliance with the rules you read, run the complete mechanical gate and applicable manual checks, and record the evidence and any unresolved obligation in the journal. Update the queue, roadmap, features and changelog as applicable. Commit verified iteration changes locally before exiting. Do not push or launch another supervisor. If a rule cannot be met, report the blocker and stop rather than bypassing it. A blocked queue is not a finished roadmap."
WORKER=(codex exec --sandbox danger-full-access -c 'approval_policy="never"' --json "$PROMPT")

# How long a worker may be *silent* before it is presumed hung, and the
# absolute ceiling regardless of how busy it looks. Idle rather than duration,
# because a hung worker stops writing to its transcript while an honest long
# item keeps writing to it — a duration-only guard in the script this replaces
# once killed ninety minutes of real work (ADR 0006).
IDLE_KILL_MIN="${IDLE_KILL_MIN:-20}"
CEILING_MIN="${CEILING_MIN:-240}"
MAX_ITERATIONS="${MAX_ITERATIONS:-500}"


# Where a run is written down.
#
# From the environment first, because the self-test starts this script eight
# times to check what the arguments mean and needs those children to write
# nowhere — including the ones that fail *during argument parsing*, which is
# before any flag is known. A variable read here, at the top, is the only thing
# that arrives early enough.
LOG="${ALO_LOOP_LOG:-docs/autonomy/loop.log}"

# Everything to the terminal *and* to a file. A run somebody walked away from is
# a run they should still be able to read, and a terminal is the one place that
# does not survive closing a window. Appended rather than replaced, so two runs
# are two records instead of one overwriting the other.
note() { printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$1" >>"$LOG" 2>/dev/null || true; }
say() { printf '\033[1m[loop]\033[0m %s\n' "$1"; note "$1"; }
bad() { printf '\033[31m[loop]\033[0m %s\n' "$1"; note "FAILED: $1"; }

once=0
dry=0
selftest=0
# How many iterations to run before stopping of its own accord.
#
# `--items N` exists because "run until the queue is empty" is a large thing to
# agree to on faith, and somebody deciding whether to trust this at all should
# be able to buy five iterations rather than five hundred. It is the same loop
# either way; only the number differs.
wanted="$MAX_ITERATIONS"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --once) once=1 ;;
    --dry-run) dry=1 ;;
    --self-test) selftest=1 ;;
    --items)
      shift
      wanted="${1:-}"
      [ -n "$wanted" ] || { bad "--items wants a number after it"; exit 2; }
      ;;
    --items=*) wanted="${1#--items=}" ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) bad "unknown argument $1 — try --help"; exit 2 ;;
  esac
  shift
done

case "$wanted" in
  ''|*[!0-9]*)
    bad "--items wants a number, and got ${wanted:-nothing}"
    exit 2
    ;;
esac
if [ "$wanted" -lt 1 ]; then
  bad "--items wants at least one"
  exit 2
fi
[ "$once" -eq 1 ] && wanted=1

# Only a real run is written down.
#
# `--self-test` starts this script eight times to check what the arguments
# mean, and every one of those children was appending its own startup lines and
# its own deliberate `FAILED:` messages to the same log. So the record of a run
# that genuinely failed would sit among a dozen failures that were tests
# passing. A log somebody has to filter before reading is a log they will stop
# reading.
#
# A dry run is not a run either, and neither is a self-test.
if [ "$dry" -eq 1 ] || [ "${selftest:-0}" -eq 1 ]; then
  LOG=/dev/null
fi

# Guard values are arithmetic input, never shell expressions.
for guard in IDLE_KILL_MIN CEILING_MIN; do
  value="${!guard}"
  case "$value" in
    ''|*[!0-9]*) bad "$guard wants a positive integer"; exit 2 ;;
  esac
  [ "$value" -ge 1 ] && [ "$value" -le 1000000 ] || {
    bad "$guard must be between 1 and 1000000"; exit 2;
  }
  printf -v "$guard" '%s' "$((10#$value))"
done
[ "$wanted" -le 1000000 ] || { bad "too many iterations"; exit 2; }
wanted=$((10#$wanted))

# --- Before anything: is this a tree an iteration should open on? ------------

[ -f "$JOURNAL" ] || { bad "no journal at $JOURNAL — is this the alo-browser checkout?"; exit 2; }
[ -f "$QUEUE" ]   || { bad "no queue at $QUEUE";   exit 2; }

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

open_items() { awk '/^- \[ \]/ { n++ } END { print n+0 }' "$QUEUE"; }

# What the run has actually done, said once at the end.
#
# Iterations are not the measure and never were: an iteration that halts
# honestly is worth more than one that invented a way past a problem. What a
# person who walked away wants to know is what **closed** and what was
# **committed**, so those are what this counts — against where the run started,
# which is why the two variables below are read before the first iteration.
finished() {
  local ran="$1"
  local closed=$(( started_open - $(open_items) ))
  local commits
  commits=$(git rev-list --count "$started_at..HEAD" 2>/dev/null || echo 0)
  echo
  say "done after $ran iteration(s)."
  say "  queue:   $closed item(s) closed, $(open_items) still open"
  say "  commits: $commits"
  if [ "$closed" -eq 0 ] && [ "$commits" -eq 0 ]; then
    bad "  nothing closed and nothing committed — read $LOG and $JOURNAL before running it again."
  fi
  say "  log:     $LOG"
}

# --- The stop-marker rule, as assertions -------------------------------------
#
# The gate asks for unit tests for logic, and marker detection is the only
# logic in this file — everything else is spawning a process and watching a
# clock. This is what a test looks like in bash: fixtures in, decision out.
if [ "${selftest:-0}" -eq 1 ]; then
  failures=0
  journal_was="$JOURNAL"
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

  # The other logic worth a test: what the arguments mean. A supervisor that
  # accepted `--items abc` as five hundred, or treated a typo as a request to
  # run forever, would be one nobody should trust with an unattended run.
  args() {
    JOURNAL="$journal_was"
    # `ALO_LOOP_LOG` so a child writes nowhere: a check that a bad argument is
    # refused would otherwise put its refusal in the log, where it reads
    # exactly like a run that failed.
    ( ALO_LOOP_LOG=/dev/null "$0" "$@" --dry-run >/dev/null 2>&1 )
    echo "$?"
  }
  expect() {
    local name="$1" want="$2"
    shift 2
    local got
    got="$(args "$@")"
    if [ "$got" = "$want" ]; then
      printf '\033[32mok\033[0m    %s\n' "$name"
    else
      printf '\033[31mFAIL\033[0m  %s — wanted exit %s, got %s\n' "$name" "$want" "$got"
      failures=1
    fi
  }
  expect "a number of items is accepted" 0 --items 5
  expect "the same, written with an equals sign" 0 --items=5
  expect "no arguments at all is accepted" 0
  expect "--once is accepted" 0 --once
  expect "a number that is not one is refused" 2 --items abc
  expect "zero items is refused" 2 --items 0
  expect "--items with nothing after it is refused" 2 --items
  expect "a typo is refused rather than ignored" 2 --run-forever
  expect "an excessive count is refused" 2 --items 999999999999999999999

  # The regression this test exists for: those eight children each refused an
  # argument, and every refusal used to land in the real log — so a run that
  # genuinely failed sat among a dozen failures that were tests passing. A log
  # somebody has to filter before reading is a log they stop reading.
  real="docs/autonomy/loop.log"
  before=$( [ -f "$real" ] && wc -l < "$real" || echo 0 )
  ( ALO_LOOP_LOG=/dev/null "$0" --items abc --dry-run >/dev/null 2>&1 )
  after=$( [ -f "$real" ] && wc -l < "$real" || echo 0 )
  if [ "$before" = "$after" ]; then
    printf '\033[32mok\033[0m    %s\n' "a child told to log nowhere writes nowhere"
  else
    printf '\033[31mFAIL\033[0m  %s — the log grew from %s to %s\n' \
      "a child told to log nowhere writes nowhere" "$before" "$after"
    failures=1
  fi

  [ "$failures" -eq 0 ] && printf '\n\033[32m[loop]\033[0m the stop rule and the arguments hold.\n'
  exit "$failures"
fi

if [ "$dry" -eq 1 ]; then
  say 'would run: codex exec --sandbox danger-full-access -c approval_policy="never" --json "$PROMPT"'
  marker="$(stop_marker)"
  say "journal:    $JOURNAL  (stop marker: ${marker:-none})"
  say "queue:      $(open_items) items still open"
  say "guards:     silent for ${IDLE_KILL_MIN}m, or ${CEILING_MIN}m total"
  say "iterations:  $wanted at most"
  say "log:         $LOG"
  exit 0
fi

command -v codex >/dev/null || { bad "no codex on PATH"; exit 2; }
if ! codex login status >/dev/null 2>&1; then
  bad "Codex is not logged in; run codex login before starting."
  exit 2
fi

# Claim the checkout atomically before the gate or any worker. A stale lock
# requires inspection: guessing its owner is dead can start rival workers.
LOCK="$(git rev-parse --git-path alo-loop.lock)"
if ! mkdir "$LOCK" 2>/dev/null; then
  bad "checkout is locked at $LOCK; inspect its owner before removing it."
  exit 3
fi
printf '%s\n' "$$" > "$LOCK/pid"
worker=""
stop_tree() {
  local parent="$1" child
  for child in $(pgrep -P "$parent" 2>/dev/null); do
    stop_tree "$child"
  done
  kill -TERM "$parent" 2>/dev/null || true
  kill -KILL "$parent" 2>/dev/null || true
}
cleanup() {
  if [ -n "$worker" ]; then
    stop_tree "$worker"
    wait "$worker" 2>/dev/null || true
  fi
  rm -f "$LOCK/pid" "$LOCK/gate.log"
  rmdir "$LOCK" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [ -n "$(git status --porcelain)" ]; then
  bad "uncommitted changes exist; preserve and finish them before starting the loop."
  exit 6
fi
RUNS="$(git rev-parse --git-path alo-loop-runs)"
mkdir -p "$RUNS" || { bad "cannot create $RUNS"; exit 2; }
RUN_DIR="$(mktemp -d "$RUNS/run.XXXXXX")" || exit 2
say "checking the tree is green before starting…"
if ! ./scripts/gate.sh >"$LOCK/gate.log" 2>&1; then
  bad "the baseline gate failed; no worker started."
  cat "$LOCK/gate.log" >> "$LOG"
  tail -20 "$LOCK/gate.log"
  rm -f "$LOCK/gate.log"
  exit 4
fi
rm -f "$LOCK/gate.log"
say "the gate is met. $(open_items) queue items open."

# Where the run started, so the summary at the end can say what it changed
# rather than how long it took.
started_open="$(open_items)"
started_at="$(git rev-parse HEAD 2>/dev/null || echo HEAD)"


for (( i = 1; i <= wanted; i++ )); do
  case "$(stop_marker)" in
    COMPLETE)
      echo
      say "the journal says LOOP COMPLETE — stopping, and not restarting."
      say "this means available work is exhausted, not that all stages are finished."
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

  iteration_head="$(git rev-parse HEAD)"
  iteration_journal="$(git hash-object "$JOURNAL")"

  started=$(date +%s)
  transcript="$RUN_DIR/iteration-$i.jsonl"
  say "worker events: $transcript"
  "${WORKER[@]}" > "$transcript" 2>&1 &
  worker=$!
  code=""
  newest=$started
  previous_bytes=0

  while kill -0 "$worker" 2>/dev/null; do
    sleep 30
    kill -0 "$worker" 2>/dev/null || break
    now=$(date +%s)
    # Observe only this worker's event stream. Another session's activity
    # cannot hide a hung worker, and no provider-private transcript path is used.
    bytes=$(wc -c < "$transcript")
    if [ "$bytes" -ne "$previous_bytes" ]; then newest=$now; fi
    previous_bytes=$bytes
    idle=$(( now - newest ))
    running=$(( now - started ))

    why=""
    [ "$idle" -ge $(( IDLE_KILL_MIN * 60 )) ] && why="silent for $(( idle / 60 )) minutes"
    [ "$running" -ge $(( CEILING_MIN * 60 )) ] && why="past the ${CEILING_MIN}-minute ceiling"
    if [ -n "$why" ]; then
      bad "killing the worker — $why."
      stop_tree "$worker"
      # Preserve every change. A timed-out iteration needs inspection, not a reset.
      code=124
      break
    fi
  done

  if [ -z "$code" ]; then wait "$worker"; code=$?; else wait "$worker" 2>/dev/null || true; fi

  worker=""
  if [ "$code" -ne 0 ]; then
    bad "worker exited $code; stopping with its changes preserved. Events: $transcript"
    tail -8 "$transcript"
    finished "$i"
    exit "$code"
  fi
  case "$(stop_marker)" in
    HALT) bad "worker recorded LOOP HALT"; finished "$i"; exit 5 ;;

  esac
  if [ -n "$(git status --porcelain)" ]; then
    bad "worker left uncommitted changes; stopping for inspection."
    exit 6
  fi
  if [ "$(git rev-parse HEAD)" = "$iteration_head" ] ||
     [ "$(git hash-object "$JOURNAL")" = "$iteration_journal" ]; then
    bad "worker made no committed, journalled progress; stopping."
    exit 7
  fi
  if ! ./scripts/gate.sh >"$LOCK/gate.log" 2>&1; then
    bad "the completed iteration failed independent verification."
    cat "$LOCK/gate.log" >> "$LOG"
    tail -20 "$LOCK/gate.log"
    rm -f "$LOCK/gate.log"
    exit 4
  fi
  rm -f "$LOCK/gate.log"
  if [ "$(stop_marker)" = COMPLETE ]; then
    say "available work exhausted; read the journal's remaining gates."
    finished "$i"
    exit 0
  fi
  if [ "$i" -ge "$wanted" ]; then
    finished "$wanted"
    exit 0
  fi

done

finished "$wanted"
exit 0

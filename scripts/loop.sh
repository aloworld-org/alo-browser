#!/usr/bin/env bash
# scripts/loop.sh — the build loop's supervisor, for macOS (ADR 0006).
#
# One worker invocation per queue item, until the journal says to stop.
# `docs/autonomy/LOOP.md` is what an iteration reads; this file only decides
# when to start one and when to stop starting them.
#
#   scripts/loop.sh                 # run until the journal says stop
#   scripts/loop.sh --once          # a single iteration, then exit
#   scripts/loop.sh --items 5       # five iterations, then exit
#   scripts/loop.sh --dry-run       # say what it would do, start nothing
#   scripts/loop.sh --self-test     # check the stop-marker rule, start nothing
#
# ALO_LOOP_WORKER names the worker explicitly — `codex` or `claude`. Unset, it
# takes whichever is here.
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
PROMPT="Follow all applicable system, developer and user instructions and repository rules. Before choosing or changing anything, read applicable AGENTS.md files, CLAUDE.md (the repository constitution), docs/autonomy/LOOP.md, ROADMAP.md, and the latest journal entry; read the selected item’s relevant ADRs and feature contract. Execute exactly ONE eligible iteration. Treat every applicable rule as mandatory: never weaken the gate, disable tests, add stubs, bypass dependency or stage gates, or claim unverified completion. Preserve unfinished work and keep sibling repositories read-only. Before committing, review compliance with the rules you read, run the complete mechanical gate and applicable manual checks, and record the evidence and any unresolved obligation in the journal. Run the gate in the foreground and read its result in the same step; do not start it in the background and then spend turns waiting for it, because an iteration that ends while waiting leaves its work uncommitted and never learns what the gate said. Update the queue, roadmap, features and changelog as applicable. Commit verified iteration changes locally before exiting. Do not push or launch another supervisor. If a rule cannot be met, report the blocker and stop rather than bypassing it. A blocked queue is not a finished roadmap."

# Which program runs an iteration, and whether it is actually on this machine.
#
# Codex first, because that is what the owner chose; Claude Code when Codex is
# not here, because a supervisor that runs on one person's machine is a
# supervisor that mostly does not run. `ALO_LOOP_WORKER` names one explicitly
# and is then told plainly that it is missing, rather than quietly handed the
# other — a loop that silently swapped workers would make every journal entry
# after it ambiguous about who wrote it.
WORKER=()
WORKER_NAME=""
WORKER_WHY=""
WORKER_STATUS=0
choose_worker() {
  local wanted="${ALO_LOOP_WORKER:-}"
  case "$wanted" in
    '' | codex | claude) ;;
    *)
      WORKER_WHY="ALO_LOOP_WORKER=$wanted names no worker this script knows"
      WORKER_STATUS=2
      return 1
      ;;
  esac

  if [ "$wanted" != claude ] && command -v codex >/dev/null; then
    # Installed but not logged in stops here rather than reaching for the
    # other worker. Absence and misconfiguration are not the same thing: one
    # is a machine that never had Codex, the other is a login somebody let
    # expire, and quietly substituting a different worker for the second turns
    # a thing to fix into a silent change of who wrote the next commit.
    if ! codex login status >/dev/null 2>&1; then
      WORKER_WHY="Codex is not logged in; run codex login before starting."
      WORKER_STATUS=2
      return 1
    fi
    WORKER=(codex exec --sandbox danger-full-access \
      -c 'approval_policy="never"' --json "$PROMPT")
    WORKER_NAME=codex
    return 0
  fi

  if [ "$wanted" = codex ]; then
    WORKER_WHY="no codex on PATH"
    WORKER_STATUS=8
    return 1
  fi

  if ! command -v claude >/dev/null; then
    if [ "$wanted" = claude ]; then
      WORKER_WHY="no claude on PATH"
    else
      WORKER_WHY="no worker on PATH: neither codex nor claude"
    fi
    WORKER_STATUS=8
    return 1
  fi
  # `--output-format stream-json --verbose` is not a preference. Without it
  # `claude -p` buffers its whole response and writes the transcript once, at
  # the end — zero bytes from the first second to the last — and the idle
  # guard below, which presumes a worker that writes as it goes, reads every
  # iteration as silent from the start. It then stops being a hang detector
  # and becomes a flat IDLE_KILL_MIN wall clock on all work. That is not a
  # theory: it killed iteration 125 mid-item, and the three before it finished
  # one to two minutes inside the window without anyone noticing how close
  # they were.
  WORKER=(claude -p "$PROMPT" --dangerously-skip-permissions \
    --output-format stream-json --verbose)
  WORKER_NAME=claude
  return 0
}

# How long a worker may do *nothing* before it is presumed hung, and the
# absolute ceiling regardless of how busy it looks. Idle rather than duration,
# because a hung worker stops working while an honest long item carries on — a
# duration-only guard in the script this replaces once killed ninety minutes of
# real work (ADR 0006).
#
# Doing nothing means two things at once: writing nothing to its transcript
# *and* burning no processor time. Either alone is wrong. Bytes alone cannot
# see inside a single long tool call, because the stream carries a tool's
# result and not its progress, so a long compile reads as silence. Processor
# time alone cannot see a worker that is waiting on a network call it will
# never get an answer to.
#
# The cost, which is deliberate: a worker spinning in a loop burns processor
# time and so is no longer idle by this measure. Nothing here will stop it, and
# CEILING_MIN is what bounds it.
IDLE_KILL_MIN="${IDLE_KILL_MIN:-20}"
CEILING_MIN="${CEILING_MIN:-240}"

# How long a worker may produce *no output at all* before it is presumed to be
# going round in circles, whatever its processor is doing.
#
# The guard above treats processor time as evidence of work, which is what
# lets a long compile finish. A worker stuck in a loop burns processor time
# too, and so is never idle by that measure — without this it would run to
# CEILING_MIN, four hours, before anything stopped it. This asks the slower
# question: not "is it doing anything" but "has it produced anything". An
# honest tool call answers in minutes. An hour of heat and no output is a
# runaway, and the ceiling is too blunt an instrument to be the only one.
SILENT_KILL_MIN="${SILENT_KILL_MIN:-60}"

# How long a gate *this script* runs may do nothing before it is presumed hung.
#
# The worker has been watched since ADR 0006; the gates run around it were not,
# and a hung one blocks the loop for ever with nothing written down. On
# 2026-10-10 the machine hibernated mid-verification and the `sed` and `grep`
# of a boundary check never resumed: the loop sat alive and stopped for four
# hours after waking, its log saying only that an iteration had started, and
# the stall was found by hand. Nothing reported it because nothing was looking.
GATE_STALL_MIN="${GATE_STALL_MIN:-15}"

# Whether a verified iteration is published to origin.
#
# On by default. An unattended run that only commits locally is a run nobody
# can see until somebody thinks to look, and a night's work sits on one
# machine where a disk is the only copy. The worker is forbidden to push, by
# its own prompt, so this is the single place a commit leaves here and the
# single place to check that it did. `ALO_LOOP_PUSH=0` keeps a run local.
PUSH="${ALO_LOOP_PUSH:-1}"
MAX_ITERATIONS="${MAX_ITERATIONS:-500}"

# How often a running worker is looked at. Also the unit the bounds are
# counted in: one observation is worth at most one interval, so time the
# machine spent asleep cannot be mistaken for time a worker spent working.
INTERVAL="${INTERVAL:-30}"


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
for guard in IDLE_KILL_MIN SILENT_KILL_MIN GATE_STALL_MIN CEILING_MIN; do
  value="${!guard}"
  case "$value" in
    ''|*[!0-9]*) bad "$guard wants a positive integer"; exit 2 ;;
  esac
  [ "$value" -ge 1 ] && [ "$value" -le 1000000 ] || {
    bad "$guard must be between 1 and 1000000"; exit 2;
  }
  printf -v "$guard" '%s' "$((10#$value))"
done
# A silence bound under the idle bound would retire the idle guard without
# saying so: everything it catches, the shorter one would have caught first.
case "$PUSH" in
  0 | 1) ;;
  *) bad "ALO_LOOP_PUSH wants 0 or 1"; exit 2 ;;
esac
if [ "$SILENT_KILL_MIN" -lt "$IDLE_KILL_MIN" ]; then
  bad "SILENT_KILL_MIN below IDLE_KILL_MIN leaves the idle guard nothing to do"
  exit 2
fi
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
  # An accepted argument exits 0 where a worker is installed and 4 where none
  # is. That second thing is not what these cases ask about, so both read as
  # accepted and only an argument refusal fails them — otherwise every one of
  # them would go red on a machine that simply has no worker yet, which is a
  # misleading way to report a missing program.
  accepts() {
    local name="$1"
    shift
    local got
    got="$(args "$@")"
    if [ "$got" != 2 ]; then
      printf '\033[32mok\033[0m    %s\n' "$name"
    else
      printf '\033[31mFAIL\033[0m  %s — the argument was refused\n' "$name"
      failures=1
    fi
  }
  accepts "a number of items is accepted" --items 5
  accepts "the same, written with an equals sign" --items=5
  accepts "no arguments at all is accepted"
  accepts "--once is accepted" --once
  expect "a number that is not one is refused" 2 --items abc
  expect "zero items is refused" 2 --items 0
  expect "--items with nothing after it is refused" 2 --items
  expect "a typo is refused rather than ignored" 2 --run-forever
  expect "an excessive count is refused" 2 --items 999999999999999999999

  # The regression this test exists for: those eight children each refused an
  # argument, and every refusal used to land in the real log — so a run that
  # genuinely failed sat among a dozen failures that were tests passing. A log
  # somebody has to filter before reading is a log they stop reading.
  # What the dry run says about a worker that is not there. It used to print a
  # confident "would run: codex ..." whatever the machine had on it, so the one
  # failure that stops every iteration was the one it did not look for.
  absent="$( ALO_LOOP_LOG=/dev/null ALO_LOOP_WORKER=nonsense-worker \
    "$0" --dry-run 2>&1 )"
  absent_status=$?
  if [ "$absent_status" = 2 ] && ! printf '%s' "$absent" | grep -q 'would run: '
  then
    printf '\033[32mok\033[0m    %s\n' \
      "a dry run claims no worker it has not found"
  else
    printf '\033[31mFAIL\033[0m  %s — exit %s, said %s\n' \
      "a dry run claims no worker it has not found" "$absent_status" "$absent"
    failures=1
  fi

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
  # The worker is looked for rather than described. This line used to be a
  # fixed string naming a program the dry run had never checked was installed,
  # so it reported every precondition except the only one that stops a run
  # dead — and said "would run" about a command that could not.
  if choose_worker; then
    say "would run:  ${WORKER[*]//"$PROMPT"/\$PROMPT}"
  else
    bad "would run nothing: $WORKER_WHY"
  fi
  marker="$(stop_marker)"
  say "journal:    $JOURNAL  (stop marker: ${marker:-none})"
  say "queue:      $(open_items) items still open"
  say "guards:     idle ${IDLE_KILL_MIN}m, no output ${SILENT_KILL_MIN}m, ceiling ${CEILING_MIN}m, gate stall ${GATE_STALL_MIN}m"
  if [ "$PUSH" = 1 ]; then
    say "publish:    every verified iteration, to $(git remote get-url origin 2>/dev/null || echo 'no origin configured')"
  else
    say "publish:    nothing; this run stays local"
  fi
  say "iterations:  $wanted at most"
  say "log:         $LOG"
  [ -n "$WORKER_NAME" ] || exit "$WORKER_STATUS"
  exit 0
fi

# Exit 8 where no worker is installed, keeping 2 for the things a person typed
# wrongly — a bad flag, a worker name that is not one, a Codex login that has
# expired. "This machine has nothing to run an iteration with" is a different
# thing to go and fix, and a caller reading only the status can tell them apart.
choose_worker || { bad "$WORKER_WHY"; exit "$WORKER_STATUS"; }
say "worker:     $WORKER_NAME"

# Claim the checkout atomically before the gate or any worker. A stale lock
# requires inspection: guessing its owner is dead can start rival workers.
LOCK="$(git rev-parse --git-path alo-loop.lock)"
if ! mkdir "$LOCK" 2>/dev/null; then
  bad "checkout is locked at $LOCK; inspect its owner before removing it."
  exit 3
fi
printf '%s\n' "$$" > "$LOCK/pid"
worker=""

# Every process in a tree, parent first. `stop_tree` walks this same shape to
# kill it; these two walk it to ask whether it is doing anything.
tree_pids() {
  local parent="$1" child
  printf '%s\n' "$parent"
  for child in $(pgrep -P "$parent" 2>/dev/null); do
    tree_pids "$child"
  done
}

# Hundredths of a processor second the worker's tree has burned.
#
# The other half of "is this worker doing anything". A transcript answers that
# between tool calls and cannot answer it during one, because the stream
# carries a tool's result and not its progress — so by bytes alone a
# fifteen-minute compile is indistinguishable from a deadlock. Processor time
# tells them apart: the compile is burning it, the deadlock is not.
#
# `ps` prints [[dd-]hh:]mm:ss[.ff]; hundredths keep the comparison integer.
tree_cpu() {
  local pids
  pids="$(tree_pids "$1" | paste -sd, -)"
  [ -n "$pids" ] || { printf '0\n'; return; }
  ps -o time= -p "$pids" 2>/dev/null | awk '
    {
      t = $1; d = 0
      if (t ~ /-/) { split(t, p, "-"); d = p[1]; t = p[2] }
      n = split(t, p, ":")
      s = 0
      for (i = 1; i <= n; i++) { s = s * 60 + p[i] }
      total += (s + d * 86400) * 100
    }
    END { printf "%d\n", total }
  '
}

# Run `scripts/gate.sh`, watched the way a worker is.
#
# Stalled means what it means for a worker: the log is not growing *and*
# nothing in the tree is burning processor time. `cargo test` writes nothing
# here for minutes together and is plainly working; a pipeline that did not
# survive hibernation does neither.
#
# Answers 0 if the gate passed, 1 if it failed, 125 if it stopped doing
# anything. 125 rather than a small number because a gate's own exit codes
# live down there.
watched_gate() {
  : > "$LOCK/gate.log"
  ./scripts/gate.sh > "$LOCK/gate.log" 2>&1 &
  local gate=$! bytes previous=0 cpu previous_cpu stalled=0 step now last
  previous_cpu=$(tree_cpu "$gate")
  last=$(date +%s)
  while kill -0 "$gate" 2>/dev/null; do
    sleep "$INTERVAL"
    kill -0 "$gate" 2>/dev/null || break
    now=$(date +%s)
    step=$(( now - last ))
    last=$now
    [ "$step" -gt $(( INTERVAL * 3 )) ] && step=$INTERVAL
    [ "$step" -lt 0 ] && step=0
    bytes=$(wc -c < "$LOCK/gate.log")
    cpu=$(tree_cpu "$gate")
    if [ "$bytes" -ne "$previous" ] || [ "$cpu" -ne "$previous_cpu" ]; then
      stalled=0
    else
      stalled=$(( stalled + step ))
    fi
    previous=$bytes
    previous_cpu=$cpu
    if [ "$stalled" -ge $(( GATE_STALL_MIN * 60 )) ]; then
      bad "the gate has done nothing for $(( stalled / 60 )) minutes; stopping."
      stop_tree "$gate"
      wait "$gate" 2>/dev/null || true
      return 125
    fi
  done
  wait "$gate"
}

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
watched_gate
baseline_code=$?
if [ "$baseline_code" -eq 125 ]; then
  cat "$LOCK/gate.log" >> "$LOG"
  rm -f "$LOCK/gate.log"
  exit 9
fi
if [ "$baseline_code" -ne 0 ]; then
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

  transcript="$RUN_DIR/iteration-$i.jsonl"
  say "worker events: $transcript"
  "${WORKER[@]}" > "$transcript" 2>&1 &
  worker=$!
  code=""
  # All three bounds count seconds this machine was *awake*, accumulated an
  # observation at a time, rather than subtracting two wall-clock readings.
  #
  # A laptop that hibernates overnight comes back with the clock hours ahead
  # while the worker has done nothing and been asked for nothing. Subtracting
  # readings reads that as eighteen hours of runtime and of silence, and the
  # first poll after waking kills a healthy worker on every bound at once.
  # That is not hypothetical: iteration 2 of 2026-10-06 was killed at 17:41
  # "past the 240-minute ceiling" after starting at 22:58 the night before,
  # and the power log puts a `hibernate user wake` at 17:42.
  #
  # So an unattended overnight run — the thing this script exists for — could
  # not survive the night on this machine.
  elapsed=0
  idle=0
  quiet=0
  previous_bytes=0
  previous_cpu=$(tree_cpu "$worker")
  last=$(date +%s)

  while kill -0 "$worker" 2>/dev/null; do
    sleep "$INTERVAL"
    kill -0 "$worker" 2>/dev/null || break
    now=$(date +%s)
    # What this observation is worth. A gap far longer than the interval is
    # the machine having been away, not the worker having been busy, so it
    # counts as one interval and no more; a clock that moved backwards counts
    # as nothing.
    step=$(( now - last ))
    last=$now
    [ "$step" -gt $(( INTERVAL * 3 )) ] && step=$INTERVAL
    [ "$step" -lt 0 ] && step=0
    elapsed=$(( elapsed + step ))
    # Observe only this worker's event stream. Another session's activity
    # cannot hide a hung worker, and no provider-private transcript path is used.
    bytes=$(wc -c < "$transcript")
    cpu=$(tree_cpu "$worker")
    # Changed, in either direction, rather than grown. A tree's total falls
    # when a child exits, and a child exiting is work finishing, not a worker
    # hanging — measured on a live tree, which went from 16 to 13 hundredths
    # across eight seconds as the gate's processes came and went. Only a
    # frozen set of processes burning a frozen amount is doing nothing.
    if [ "$bytes" -ne "$previous_bytes" ]; then
      quiet=0
    else
      quiet=$(( quiet + step ))
    fi
    if [ "$bytes" -ne "$previous_bytes" ] || [ "$cpu" -ne "$previous_cpu" ]; then
      idle=0
    else
      idle=$(( idle + step ))
    fi
    previous_bytes=$bytes
    previous_cpu=$cpu

    why=""
    [ "$idle" -ge $(( IDLE_KILL_MIN * 60 )) ] \
      && why="silent and burning no processor time for $(( idle / 60 )) minutes"
    [ "$quiet" -ge $(( SILENT_KILL_MIN * 60 )) ] \
      && why="burning processor time but producing nothing for \
$(( quiet / 60 )) minutes"
    [ "$elapsed" -ge $(( CEILING_MIN * 60 )) ] \
      && why="past the ${CEILING_MIN}-minute ceiling"
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
  watched_gate
  gate_code=$?
  if [ "$gate_code" -eq 125 ]; then
    cat "$LOCK/gate.log" >> "$LOG"
    rm -f "$LOCK/gate.log"
    finished "$i"
    exit 9
  fi
  if [ "$gate_code" -ne 0 ]; then
    bad "the completed iteration failed independent verification."
    cat "$LOCK/gate.log" >> "$LOG"
    tail -20 "$LOCK/gate.log"
    rm -f "$LOCK/gate.log"
    exit 4
  fi
  rm -f "$LOCK/gate.log"

  # Publish what has just been verified, and only that. After the independent
  # gate rather than after the worker's commit, so what reaches origin is what
  # passed verification on this machine, not what a worker believed it had
  # finished.
  #
  # A failure here does not stop the run. The commits are safe locally and
  # `git push` sends everything outstanding, so the next iteration carries
  # them; losing a night of work to one refused connection would be the worse
  # trade. It is said loudly instead, every time, so a remote that has been
  # refusing for hours cannot look like silence.
  if [ "$PUSH" = 1 ]; then
    if git push --quiet origin HEAD >>"$LOG" 2>&1; then
      say "published to origin."
    else
      bad "push to origin failed. The work is committed here and the next \
iteration will carry it; if this repeats, the remote needs attention."
    fi
  fi

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

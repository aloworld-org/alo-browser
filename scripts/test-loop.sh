#!/usr/bin/env bash
# Supervisor regressions with an isolated git repository and a fake worker.
set -euo pipefail
cd "$(dirname "$0")/.."
source_script="$PWD/scripts/loop.sh"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/scripts" "$fixture/docs/autonomy" "$fixture/bin"
cp "$source_script" "$fixture/scripts/loop.sh"
cd "$fixture"
git init -q
git config user.name 'Loop test'
git config user.email 'loop-test@example.invalid'
printf '%s\n' 'docs/autonomy/loop.log' bin origin.git > .gitignore
printf '%s\n' '- [ ] **1. Fixture.**' > docs/autonomy/QUEUE.md
printf '%s\n' '## Iteration 0' > docs/autonomy/STATE.md
cat > scripts/gate.sh <<'GATE'
#!/usr/bin/env bash
[ ! -f broken ]
GATE
cat > bin/codex <<'WORKER'
#!/usr/bin/env bash
if [ "${1:-}" = login ]; then
  [ "${TEST_MODE:-}" != unauthenticated ]
  exit $?
fi
[ "${1:-}" = exec ] || exit 98
[ "${2:-}" = --sandbox ] && [ "${3:-}" = danger-full-access ] || exit 98
[ "${4:-}" = -c ] && [ "${5:-}" = 'approval_policy="never"' ] || exit 98
[ "${6:-}" = --json ] && [ -n "${7:-}" ] || exit 98
case "${7:-}" in
  *AGENTS.md*CLAUDE.md*LOOP.md*ROADMAP.md*"review compliance"*"report the blocker and stop"*) ;;
  *) exit 98 ;;
esac
exec worker-modes
WORKER
# What a worker does once its arguments are accepted, shared by both stubs so
# that a selection test cannot pass by accident against a worker that behaves
# differently from the one it replaced.
cat > bin/worker-modes <<'MODES'
#!/usr/bin/env bash
case "$TEST_MODE" in
  fail) echo preserved > work; exit 42 ;;
  timeout) echo preserved > work; /bin/sleep 30; exit 0 ;;
  noop) exit 0 ;;
  halt) printf '\nLOOP HALT\n' >> docs/autonomy/STATE.md; exit 0 ;;
  dirty) echo preserved > work; exit 0 ;;
  badgate) touch broken ;;
  # Writes as it goes and takes longer than the idle window, which is the one
  # thing the guard must not treat as a hang.
  # Writes nothing at all and is plainly working: the inside of a single long
  # tool call, which the transcript cannot see into. Pure shell arithmetic, so
  # the processor time lands on the worker rather than on a child that comes
  # and goes.
  # Writes nothing and its tree's processor total only ever falls, which is
  # what `bin/ps` is scripted to report here. A falling total means a child
  # exited, and a child exiting is work finishing.
  shrinking) /bin/sleep 2.5 ;;
  # Writes nothing and burns processor time without end: the runaway the idle
  # guard deliberately cannot catch, since heat is what it reads as work.
  spinning)
    spin=0
    while :; do spin=$(( spin + 1 )); done
    ;;
  busy)
    stop=$(( SECONDS + 3 ))
    spin=0
    while [ "$SECONDS" -lt "$stop" ]; do spin=$(( spin + 1 )); done
    ;;
  # Works normally throughout; the clock is what misbehaves. `bin/date` puts
  # an eighteen-hour jump in the middle of this, the way a hibernating laptop
  # does, and a worker that was never asked for anything during it must
  # survive.
  slept)
    for n in 1 2 3 4 5 6 7 8 9 10 11 12; do
      printf '{"type":"event","n":%s}\n' "$n"
      /bin/sleep 0.15
    done
    ;;
  streaming)
    for n in 1 2 3 4 5 6 7 8 9 10 11 12; do
      printf '{"type":"event","n":%s}\n' "$n"
      /bin/sleep 0.15
    done
    ;;
esac
printf '\n## Iteration 1\nCompleted fixture.\n' >> docs/autonomy/STATE.md
printf '%s\n' '- [x] **1. Fixture.**' > docs/autonomy/QUEUE.md
if [ "$TEST_MODE" = complete ]; then
  printf '\nLOOP COMPLETE\n' >> docs/autonomy/STATE.md
fi
git add .
git commit -qm 'test: complete fixture'
MODES
# The other worker the supervisor knows. Its argument shape is checked just as
# strictly: a fallback that invoked Claude Code wrongly would be a fallback
# that never ran, and the fixture would not notice.
cat > bin/claude <<'CLAUDE'
#!/usr/bin/env bash
[ "${1:-}" = -p ] && [ -n "${2:-}" ] || exit 98
[ "${3:-}" = --dangerously-skip-permissions ] || exit 98
# The streaming flags are part of the contract, not a preference: without them
# this worker writes its transcript once at the end and the idle guard reads
# every iteration as silent from the first second.
[ "${4:-}" = --output-format ] && [ "${5:-}" = stream-json ] || exit 98
[ "${6:-}" = --verbose ] || exit 98
case "${2:-}" in
  *AGENTS.md*CLAUDE.md*LOOP.md*ROADMAP.md*"review compliance"*"report the blocker and stop"*) ;;
  *) exit 98 ;;
esac
exec worker-modes
CLAUDE
# Avoid the production thirty-second observation interval in fixture runs.
cat > bin/sleep <<'SLEEP'
#!/usr/bin/env bash
/bin/sleep 0.05
SLEEP
cat > bin/date <<'DATE'
#!/usr/bin/env bash
case "$TEST_MODE" in
  timeout | streaming | busy | shrinking | spinning | slept) fake=1 ;;
  *) fake=0 ;;
esac
if [ "$fake" = 1 ] \
  && [ "${1:-}" = +%s ]; then
  count=$(cat bin/clock 2>/dev/null || echo 0)
  # Coarse enough in `timeout` to cross the idle window in one observation;
  # finer in `streaming` so the window is crossed only by a worker that has
  # genuinely stopped writing, rather than by the clock outrunning it.
  step=60
  case "$TEST_MODE" in streaming | busy | shrinking | spinning | slept) step=5 ;;
  esac
  # One observation lands after the machine has been away for eighteen hours.
  # Counted as wall-clock it is past every bound at once; counted as what the
  # worker was asked for, it is one interval.
  if [ "$TEST_MODE" = slept ] && [ "$count" -ge 40 ] \
    && [ ! -f bin/slept-once ]; then
    : > bin/slept-once
    count=$(( count + 18 * 3600 ))
  fi
  count=$((count + step))
  echo "$count" > bin/clock
  echo "$count"
else
  /bin/date "$@"
fi
DATE
# Processor times the test chooses, for the one case real processes cannot
# stage honestly. `busy` already exercises reading them off a live tree; what
# this isolates is the guard's *interpretation* of a total that falls, which
# needs a decrease-only window — and staging that with real children depends
# on timings fine enough to make the check flaky.
cat > bin/ps <<'PS'
#!/usr/bin/env bash
if [ "${TEST_MODE:-}" = shrinking ] && [ "${1:-}" = -o ] && [ "${2:-}" = time= ]
then
  n=$(cat bin/cpu 2>/dev/null || echo 1000)
  n=$(( n - 1 ))
  echo "$n" > bin/cpu
  printf '  0:%02d.%02d\n' $(( n / 100 )) $(( n % 100 ))
  exit 0
fi
exec /bin/ps "$@"
PS
chmod +x scripts/gate.sh bin/*
export PATH="$fixture/bin:$PATH"
# Somewhere to publish to. A bare repository on the same disk is a real
# remote as far as `git push` is concerned, and it means the check can ask the
# question that matters — did the commit arrive — rather than whether a
# command was spelled correctly.
git init -q --bare "$fixture/origin.git"
git remote add origin "$fixture/origin.git"
git add .
git commit -qm 'test: initial fixture'
base="$(git rev-parse HEAD)"
check() {
  local mode="$1" expected="$2" actual=0
  # These resets affect only this disposable fixture, never the real checkout.
  git reset --hard -q "$base"
  rm -f work broken bin/clock bin/cpu bin/slept-once
  TEST_MODE="$mode" IDLE_KILL_MIN=1 scripts/loop.sh --once > "$fixture/result" 2>&1 || actual=$?
  if [ "$actual" != "$expected" ]; then cat "$fixture/result"; exit 1; fi
  [ ! -d .git/alo-loop.lock ]
  printf 'ok    %s (exit %s)\n' "$mode" "$actual"
}
# Keep the captured output outside git's work inventory.
printf '%s\n' result >> .git/info/exclude
check unauthenticated 2
check fail 42
[ "$(cat work)" = preserved ]
check timeout 124
[ "$(cat work)" = preserved ]
check noop 7
check halt 5
check dirty 6
[ "$(cat work)" = preserved ]
check badgate 4
check complete 0
check success 0
# The regression that cost iteration 125: a worker writing all the way through
# an iteration longer than the idle window is working, not hung.
check streaming 0
# And the limit streaming does not reach: a worker inside one long tool call
# writes nothing, and is still working. `timeout` above is the counterpart —
# it sleeps, so it burns no processor time and is still killed.
check busy 0
# The gap the processor-time rule left when it asked whether time had grown
# rather than changed: a total that only falls is a tree whose children are
# finishing, not a worker that has stopped.
check shrinking 0
# The night this script could not survive: a laptop hibernates mid-iteration
# and the clock returns hours ahead. `timeout` still proves a worker that
# genuinely stops being asked for anything is killed.
check slept 0
# A pre-existing change must never reach a worker.
git reset --hard -q "$base"
echo original > work
code=0
TEST_MODE=fail scripts/loop.sh --once > result 2>&1 || code=$?
[ "$code" = 6 ] && [ "$(cat work)" = original ]
printf 'ok    pre-existing work preserved\n'
rm work
mkdir .git/alo-loop.lock
code=0
TEST_MODE=success scripts/loop.sh --once > result 2>&1 || code=$?
[ "$code" = 3 ] && [ -d .git/alo-loop.lock ]
rmdir .git/alo-loop.lock
printf 'ok    existing lock refused\n'

# Which worker runs, and what happens when none can. Absence falls through to
# the other worker; a login that has expired does not (`unauthenticated`
# above), because a machine that never had Codex and a Codex nobody is signed
# in to are different problems and only one of them is solved by using
# something else.
git reset --hard -q "$base"
rm -f work broken bin/clock
mkdir -p bin/parked
mv bin/codex bin/parked/
code=0
TEST_MODE=success scripts/loop.sh --once > result 2>&1 || code=$?
[ "$code" = 0 ] || { cat result; exit 1; }
grep -q 'worker:     claude' result
grep -q 'Completed fixture' docs/autonomy/STATE.md
printf 'ok    no codex falls through to claude\n'

# And a worker demanded by name that is not installed. Asked for rather than
# merely absent, because the real `claude` on the machine running this test is
# still on PATH behind the fixture's own bin and cannot be hidden by moving a
# stub — so "neither is installed" is not a state this fixture can honestly
# stage, while "the one you asked for is not here" is.
git reset --hard -q "$base"
rm -f work broken bin/clock
code=0
TEST_MODE=success ALO_LOOP_WORKER=codex scripts/loop.sh --once > result 2>&1 \
  || code=$?
[ "$code" = 8 ] || { cat result; exit 1; }
grep -q 'no codex on PATH' result
[ ! -d .git/alo-loop.lock ]
git diff --quiet HEAD
printf 'ok    a worker asked for and absent refuses before taking the lock\n'

# The runaway the idle guard is deliberately blind to. Heat counts as work
# there, so a worker going round in circles is never idle; what catches it is
# the longer question of whether it has produced anything at all.
git reset --hard -q "$base"
rm -f work broken bin/clock bin/cpu
code=0
TEST_MODE=spinning IDLE_KILL_MIN=1 SILENT_KILL_MIN=1 \
  scripts/loop.sh --once > result 2>&1 || code=$?
[ "$code" = 124 ] || { cat result; exit 1; }
grep -q 'producing nothing' result
[ ! -d .git/alo-loop.lock ]
printf 'ok    a worker burning processor time and producing nothing is killed\n'

# And that bound cannot be set below the idle one, which would quietly retire
# the idle guard: everything it catches the shorter bound catches first.
code=0
SILENT_KILL_MIN=1 IDLE_KILL_MIN=5 scripts/loop.sh --dry-run > result 2>&1 \
  || code=$?
[ "$code" = 2 ] || { cat result; exit 1; }
grep -q 'leaves the idle guard nothing to do' result
printf 'ok    a silence bound under the idle bound is refused\n'

# Publishing. The worker is forbidden to push, so the supervisor is the only
# thing that can, and an unattended night's work existing on one disk only is
# the failure this prevents.
git reset --hard -q "$base"
rm -f work broken bin/clock bin/cpu
git push -q --force origin "$base":refs/heads/main
code=0
TEST_MODE=success scripts/loop.sh --once > result 2>&1 || code=$?
[ "$code" = 0 ] || { cat result; exit 1; }
[ "$(git rev-parse HEAD)" = "$(git -C "$fixture/origin.git" rev-parse main)" ] \
  || { echo 'origin did not receive the verified iteration'; exit 1; }
grep -q 'published to origin' result
printf 'ok    a verified iteration reaches origin\n'

# And a run told to stay local stays local, including its commits.
git reset --hard -q "$base"
rm -f work broken bin/clock bin/cpu
git push -q --force origin "$base":refs/heads/main
code=0
TEST_MODE=success ALO_LOOP_PUSH=0 scripts/loop.sh --once > result 2>&1 || code=$?
[ "$code" = 0 ] || { cat result; exit 1; }
[ "$(git -C "$fixture/origin.git" rev-parse main)" = "$base" ] \
  || { echo 'origin moved on a run told to stay local'; exit 1; }
printf 'ok    a run told to stay local does not publish\n'

# A remote that refuses must not cost the run its work.
git reset --hard -q "$base"
rm -f work broken bin/clock bin/cpu
git remote set-url origin "$fixture/not-a-repository"
code=0
TEST_MODE=success scripts/loop.sh --once > result 2>&1 || code=$?
git remote set-url origin "$fixture/origin.git"
[ "$code" = 0 ] || { cat result; exit 1; }
grep -q 'push to origin failed' result
git diff --quiet HEAD
printf 'ok    a refused push is reported and the iteration still counts\n'
mv bin/parked/codex bin/

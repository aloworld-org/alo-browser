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
printf '%s\n' 'docs/autonomy/loop.log' bin > .gitignore
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
if [ "$TEST_MODE" = timeout ] && [ "${1:-}" = +%s ]; then
  count=$(cat bin/clock 2>/dev/null || echo 0)
  count=$((count + 60))
  echo "$count" > bin/clock
  echo "$count"
else
  /bin/date "$@"
fi
DATE
chmod +x scripts/gate.sh bin/*
export PATH="$fixture/bin:$PATH"
git add .
git commit -qm 'test: initial fixture'
base="$(git rev-parse HEAD)"
check() {
  local mode="$1" expected="$2" actual=0
  # These resets affect only this disposable fixture, never the real checkout.
  git reset --hard -q "$base"
  rm -f work broken bin/clock
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
mv bin/parked/codex bin/

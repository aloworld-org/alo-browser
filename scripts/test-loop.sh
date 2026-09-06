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
cat > bin/claude <<'WORKER'
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
WORKER
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

#!/usr/bin/env bash
# One task: copy its tests in, let the agent work until the gate passes,
# then commit. Called by agent.slurm, on the compute node, offline.
#
#   run_task.sh <task dir> <rounds>
set -uo pipefail

TASK=${1:?task directory}
ROUNDS=${2:-6}
BASE=/ptmp/binl/agold/anvil
WORK=$BASE/work
NAME=$(basename "$TASK")
LOG=$BASE/logs/$NAME.log
export CARGO_HOME=$BASE/cargo
export PATH=$CARGO_HOME/bin:$PATH
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-16}

exec > >(tee -a "$LOG") 2>&1
echo "=== $NAME: start $(date) ==="
cd "$WORK"

# The tests are the contract. Copy them in and remember their checksums.
SUMS=$BASE/logs/$NAME.sums
: > "$SUMS"
if [ -d "$TASK/tests" ]; then
  for f in "$TASK"/tests/*; do
    case "$f" in
      *anvil-ui*|*ribbon_panels.rs) dest=crates/anvil-ui/tests/$(basename "$f") ;;
      *plane_ref.rs)                dest=crates/anvil-feature/tests/$(basename "$f") ;;
      *neutral_import.rs)           dest=crates/anvil-io/tests/$(basename "$f") ;;
      *)                            dest=crates/anvil-ui/tests/$(basename "$f") ;;
    esac
    mkdir -p "$(dirname "$dest")"
    cp "$f" "$dest"
    sha256sum "$dest" >> "$SUMS"
    echo "  test: $dest"
  done
fi
if [ -d "$TASK/fixtures" ]; then
  case "$NAME" in
    *plane-ref*) fdest=crates/anvil-feature/tests/fixtures ;;
    *)           fdest=crates/anvil-io/tests/fixtures ;;
  esac
  mkdir -p "$fdest"
  cp "$TASK"/fixtures/* "$fdest/"
  for f in "$TASK"/fixtures/*; do sha256sum "$fdest/$(basename "$f")" >> "$SUMS"; done
fi

gate() {
  cargo fmt --all \
    && cargo fmt --all --check \
    && cargo clippy --workspace --all-targets -- -D warnings \
    && cargo test --workspace \
    && cargo test -p anvil-ui --features devtools
}

AIDER=$BASE/envs/aider/bin/aider
COMMON=(
  --model "openai/$SERVED_MODEL"
  --openai-api-base http://127.0.0.1:8000/v1
  --openai-api-key dummy
  --yes-always
  --no-analytics
  --no-auto-commits
  --no-gitignore
  --map-tokens 4096
  --test-cmd "cargo test --workspace"
)

MSG=$(cat "$TASK/SPEC.md")
for round in $(seq 1 "$ROUNDS"); do
  echo "--- $NAME round $round ---"
  if [ "$round" -eq 1 ]; then
    "$AIDER" "${COMMON[@]}" --message "$MSG"
  else
    OUT=$(gate 2>&1 | tail -120)
    "$AIDER" "${COMMON[@]}" --message "The gate still fails. Fix the cause, do not edit the test files.

$OUT"
  fi
  if gate; then
    echo "--- $NAME: the gate passed on round $round ---"
    break
  fi
done

if ! gate; then
  echo "=== $NAME: FAILED, leaving the work tree dirty for review ==="
  git -C "$WORK" diff > "$BASE/out/$NAME.failed.diff"
  exit 1
fi

# The contract must be untouched.
if ! sha256sum -c "$SUMS" --quiet; then
  echo "=== $NAME: FAILED, a test file was edited ==="
  git -C "$WORK" diff > "$BASE/out/$NAME.tampered.diff"
  exit 2
fi

git -C "$WORK" add -A
git -C "$WORK" commit -q -m "$NAME: $(head -1 "$TASK/SPEC.md" | sed 's/^# //')

Written on Nova by $SERVED_MODEL, to the specification in
nova/tasks/$NAME/SPEC.md. The tests in that folder were the gate and
were not edited.
" || echo "nothing to commit"
echo "=== $NAME: done $(date) ==="

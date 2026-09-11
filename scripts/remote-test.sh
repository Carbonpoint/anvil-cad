#!/usr/bin/env bash
# Build or test Anvil on a lab host instead of WSL.
# Usage: scripts/remote-test.sh [cargo subcommand...]
#   REMOTE_HOST  host name from ~/lanssh (default stalker)
#   REMOTE_DIR   directory on the host (default anvil-cad); use a unique
#                name per parallel worker, for example anvil-wt-fillet
#   REMOTE_JOBS  cargo -j value (default 16)
# The local tree (minus target and .git) is copied over each time, so the
# remote always builds exactly what is on disk here.
set -euo pipefail
HOST="${REMOTE_HOST:-stalker}"
DIR="${REMOTE_DIR:-anvil-cad}"
JOBS="${REMOTE_JOBS:-16}"
CMD="${*:-test --workspace}"
cd "$(dirname "$0")/.."
tar czf - --exclude=target --exclude=.git . | ~/lanssh/lanssh "$HOST" \
  "mkdir -p ~/$DIR && cd ~/$DIR && find . -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} + && tar xzf - && source ~/.cargo/env && export CARGO_BUILD_JOBS=$JOBS && cargo $CMD 2>&1 | tail -80"

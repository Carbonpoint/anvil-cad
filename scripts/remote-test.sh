#!/usr/bin/env bash
# Run the Anvil test suite on a lab host (default: stalker) instead of WSL.
# Usage: scripts/remote-test.sh [host] [cargo subcommand...]
# Requires ~/lanssh/lanssh and a Rust toolchain on the host (rustup, minimal).
set -euo pipefail
HOST="${1:-stalker}"; shift || true
CMD="${*:-test --workspace}"
cd "$(dirname "$0")/.."
tar czf - --exclude=target --exclude=.git . | ~/lanssh/lanssh "$HOST" \
  "rm -rf ~/anvil-cad && mkdir -p ~/anvil-cad && cd ~/anvil-cad && tar xzf - && source ~/.cargo/env && cargo $CMD -j 32 2>&1 | tail -40"

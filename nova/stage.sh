#!/usr/bin/env bash
# Stage everything the batch job needs. Run this on the Nova LOGIN node,
# which has internet. The compute node has none.
#
#   ssh hpc 'bash /ptmp/binl/agold/anvil/work/nova/stage.sh'
set -euo pipefail

BASE=/ptmp/binl/agold/anvil
WORK=$BASE/work
export CARGO_HOME=$BASE/cargo
export RUSTUP_HOME=$BASE/rustup
export HF_HOME=/ptmp/binl/agold/hf
MODEL=Qwen/Qwen3-235B-A22B-Instruct-2507-FP8

mkdir -p "$BASE" "$BASE/out" "$BASE/logs"
cd "$WORK"

echo "== 1. Rust toolchain on /ptmp (HOME is capped at 10 G) =="
if [ ! -x "$CARGO_HOME/bin/cargo" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --profile minimal --component rustfmt --component clippy
fi
export PATH=$CARGO_HOME/bin:$PATH
cargo --version
rustc --version

echo "== 2. System libraries the GUI crates need =="
for p in gtk+-3.0 xkbcommon wayland-client x11 fontconfig; do
  printf '   %-14s ' "$p"
  pkg-config --modversion "$p" || { echo "MISSING"; exit 1; }
done

echo "== 3. Vendor every crate, so the compute node builds offline =="
cargo fetch
rm -rf "$BASE/vendor"
mkdir -p "$WORK/.cargo"
cargo vendor "$BASE/vendor" > "$WORK/.cargo/config.toml"
echo "   vendored $(ls "$BASE/vendor" | wc -l) crates"

echo "== 4. Smoke test the toolchain, gently: a login node is shared =="
# The full gate runs inside the job, where the CPU time belongs.
nice -n 19 cargo check -p anvil-math -j 4 --offline
echo "   the toolchain works and the vendored crates resolve"

echo "== 5. The agent's python environment =="
module load python/3.10.10-zwlkg4l 2>/dev/null || module load python || true
if [ ! -d "$BASE/envs/aider" ]; then
  python3 -m venv "$BASE/envs/aider"
fi
"$BASE/envs/aider/bin/pip" install --quiet --upgrade pip
"$BASE/envs/aider/bin/pip" install --quiet aider-chat
"$BASE/envs/aider/bin/aider" --version

echo "== 6. Model weights =="
if [ ! -d "$HF_HOME/hub/models--${MODEL//\//--}" ]; then
  echo "   downloading $MODEL, this takes about 25 minutes"
  "$BASE/envs/aider/bin/pip" install --quiet huggingface_hub
  "$BASE/envs/aider/bin/hf" download "$MODEL"
else
  echo "   already here: $(du -sh "$HF_HOME/hub/models--${MODEL//\//--}" | cut -f1)"
fi

echo "== 7. vLLM =="
ls -d /ptmp/binl/agold/envs/vllm_venv >/dev/null
/ptmp/binl/agold/envs/vllm_venv/bin/python -c 'import vllm; print("   vllm", vllm.__version__)'

echo
echo "Staged. Submit the pilot with:"
echo "  sbatch $WORK/nova/agent.slurm --pilot"

#!/usr/bin/env bash
#SBATCH --job-name=anvil-agent
#SBATCH --account=binl
#SBATCH --partition=nova
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=32
#SBATCH --gres=gpu:a100:4
#SBATCH --mem=250G
#SBATCH --time=12:00:00
#SBATCH --output=/ptmp/binl/agold/anvil/logs/slurm-%j.out
#
# vLLM serves the model on localhost. The agent runs on the same node and
# edits the repository there. Nothing is served to the network.
#
#   sbatch agent.slurm            all three tasks
#   sbatch agent.slurm --pilot    task 1 only, for the first run
set -uo pipefail

BASE=/ptmp/binl/agold/anvil
WORK=$BASE/work
export HF_HOME=/ptmp/binl/agold/hf
export HF_HUB_OFFLINE=1
export SERVED_MODEL=Qwen/Qwen3-235B-A22B-Instruct-2507-FP8
export CARGO_HOME=$BASE/cargo
export PATH=$CARGO_HOME/bin:$PATH
export CARGO_NET_OFFLINE=true
# The workstation CPU rule does not apply here, but keep cargo polite.
export CARGO_BUILD_JOBS=16

echo "node $SLURMD_NODENAME job $SLURM_JOB_ID $(date)"
nvidia-smi --query-gpu=name,memory.total --format=csv,noheader

module load python/3.10.10-zwlkg4l 2>/dev/null || module load python || true

echo "== starting vLLM =="
/ptmp/binl/agold/envs/vllm_venv/bin/vllm serve "$SERVED_MODEL" \
  --served-model-name "$SERVED_MODEL" \
  --tensor-parallel-size 4 \
  --max-model-len 65536 \
  --gpu-memory-utilization 0.92 \
  --port 8000 \
  --host 127.0.0.1 \
  > "$BASE/logs/vllm-$SLURM_JOB_ID.log" 2>&1 &
VLLM=$!
trap 'kill $VLLM 2>/dev/null' EXIT

echo "   waiting for the endpoint (model load takes several minutes)"
for i in $(seq 1 180); do
  if curl -s http://127.0.0.1:8000/v1/models | grep -q "$SERVED_MODEL"; then
    echo "   up after $((i * 10)) s"
    break
  fi
  if ! kill -0 $VLLM 2>/dev/null; then
    echo "   vLLM died, see logs/vllm-$SLURM_JOB_ID.log"
    tail -40 "$BASE/logs/vllm-$SLURM_JOB_ID.log"
    exit 1
  fi
  sleep 10
done

# A branch of its own, so main on the workstation is never touched.
cd "$WORK"
git checkout -B nova/agent-"$SLURM_JOB_ID"
git config user.email "noreply@anthropic.com"
git config user.name "Anvil Nova agent"

# aider does not know this model, so tell it the window and the price.
cat > "$BASE/model_metadata.json" <<JSON
{
  "openai/$SERVED_MODEL": {
    "max_tokens": 16384,
    "max_input_tokens": 65536,
    "max_output_tokens": 16384,
    "input_cost_per_token": 0.0,
    "output_cost_per_token": 0.0,
    "litellm_provider": "openai",
    "mode": "chat"
  }
}
JSON

echo "== baseline gate, before the model touches anything =="
if ! (cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace); then
  echo "the tree was not green on arrival, stopping"
  exit 1
fi
echo "   baseline is green"

TASKS=("$WORK/nova/tasks/01-ribbon")
if [ "${1:-}" != "--pilot" ]; then
  TASKS+=("$WORK/nova/tasks/02-plane-ref" "$WORK/nova/tasks/03-neutral-import")
fi

for t in "${TASKS[@]}"; do
  bash "$WORK/nova/run_task.sh" "$t" 6 || echo "task $(basename "$t") did not finish, carrying on"
done

echo "== pictures of the result, no display needed =="
bash "$WORK/scripts/ui_shots.sh" "$BASE/out/images" || echo "picture step failed"

echo "== bundle for the workstation =="
git -C "$WORK" bundle create "$BASE/out/anvil-nova.bundle" --branches
git -C "$WORK" log --oneline main..HEAD > "$BASE/out/commits.txt"
echo "done $(date)"

# Anvil work on Nova

The programming for three features runs on the ISU Nova cluster, on the
`binl` allocation, with an open weight model. Nothing here is needed to
build or use Anvil. It is the recipe for the batch job.

## Why this shape

A compute node is not reachable from outside, so nothing is served to
the network. The job starts vLLM on `127.0.0.1` inside the job, and the
agent runs on the same node. The agent edits a clone of this repository
on `/ptmp`, builds it offline from vendored crates, and commits to a
branch. The result comes back as a git bundle.

    login node  (has internet)     compute node  (no internet)
    ------------------------       -----------------------------
    stage.sh                       vLLM on 127.0.0.1:8000
      rust toolchain                 |
      cargo vendor                   +-- aider  --edits-->  /ptmp/.../anvil-cad
      model weights                  |
      aider venv                     +-- cargo fmt / clippy / test  (the gate)
                                     |
                                     +-- git bundle back to /ptmp

## The gate

Every task has a specification and a test file. The test file is
copied into the repository before the agent starts. The agent must make
the tests pass without editing them: `run_task.sh` records the checksum
of every test file first and refuses to commit if one changed.

A commit also needs `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings` and `cargo test --workspace` to pass, plus
the developer tests (`--features devtools`).

## The tasks

| # | Folder | What it builds |
| --- | --- | --- |
| 1 | `tasks/01-ribbon` | One row of ribbon panels with menus, like Fusion |
| 2 | `tasks/02-plane-ref` | A plane parameter that can name a face, and its picker |
| 3 | `tasks/03-neutral-import` | STEP and 3MF readers, so Fusion exports open here |

They run in that order. Each one commits on its own. If the walltime
ends, the finished tasks are still there.

## Running it

On this workstation:

    rsync -a --exclude target --exclude .git ~/anvil-cad/ hpc:/ptmp/binl/agold/anvil/work/
    ssh hpc 'bash /ptmp/binl/agold/anvil/work/nova/stage.sh'
    ssh hpc 'sbatch /ptmp/binl/agold/anvil/work/nova/agent.slurm'

Pilot first. `agent.slurm --pilot` runs task 1 only, with a two hour
walltime, to prove the endpoint and the gate before a long run. The
`binl` allocation is shared, so this is a house rule, not a preference.

## Bringing the work back

    ssh hpc 'ls -l /ptmp/binl/agold/anvil/out'
    scp hpc:/ptmp/binl/agold/anvil/out/anvil-nova.bundle /tmp/
    git fetch /tmp/anvil-nova.bundle 'refs/heads/*:refs/nova/*'

Then read the diff, run the tests here, and merge what is good.

#!/bin/bash -l
#SBATCH --job-name=kettle-truchas
#SBATCH --account=binl
#SBATCH --partition=nova
#SBATCH --output=/ptmp/binl/agold/truchas/logs/run-%j.out
#
# Run one deck of the Anvil kettle case with Truchas, then make pictures.
#   sbatch --nodes=1 --ntasks=64 --time=00:30:00 run.slurm pilot casting.inp 1500
#   sbatch --nodes=2 --ntasks=128 --time=24:00:00 run.slurm fill casting.inp
# Arguments: run name, deck, and an optional limit in seconds for the solver
# (a pilot stops itself early and still writes its step rate).
set -uo pipefail
B=/ptmp/binl/agold/truchas
NAME=$1
DECK=$2
LIMIT=${3:-0}
module purge
for m in gcc/11.4.1-l3zl3n5 openmpi/4.1.8-lctrfpx openblas/0.3.30-openmp-37t7ek7; do
  module load $m
done
export LD_LIBRARY_PATH=$B/tpl/lib:$B/tpl/lib64:${LD_LIBRARY_PATH:-}
RUN=$B/runs/$NAME
mkdir -p $RUN
cp $B/runs/$DECK $RUN/
ln -sf $B/runs/mesh.exo $RUN/mesh.exo
cd $RUN
echo "node $SLURM_JOB_NODELIST job $SLURM_JOB_ID ranks $SLURM_NTASKS deck $DECK $(date)"
start=$(date +%s)
if [ "$LIMIT" -gt 0 ]; then
  timeout $LIMIT srun --mpi=pmix --cpu-bind=cores $B/install/bin/t-linux.x86_64.gnu -f $DECK > truchas.log 2>&1
else
  srun --mpi=pmix --cpu-bind=cores $B/install/bin/t-linux.x86_64.gnu -f $DECK > truchas.log 2>&1
fi
status=$?
end=$(date +%s)
steps=$(grep -a -E '^ +[0-9]+: t = ' truchas.log | tail -1)
echo "solver exit $status after $((end - start)) s; last step: $steps"
grep -a -E 'FATAL|SIGSEGV|terminated' truchas.log | head -3
h5=$(ls ${DECK%.inp}_output/*.h5 2>/dev/null | head -1)
mode=fill
[ "$DECK" = freeze.inp ] && mode=freeze
if [ -n "$h5" ]; then
  $B/venv/bin/python $B/truchas_pictures.py "$h5" $RUN/pictures $mode
fi
echo "done $(date)"

# The kettle Truchas run on Nova

Batch scripts for the Anvil kettle casting case on the ISU Nova cluster
(`binl` allocation). Everything lives under `/ptmp/binl/agold/truchas`.

1. On the login node (it has internet): clone `truchas` and `truchas-tpl`
   (master; its `tarfiles/` hold every library source, so the build needs
   no internet), and make `venv` with numpy, h5py, fypp and matplotlib
   (`module load python/3.11.11-dnlm5qp`).
2. Copy the case from `anvil-cli kettle --variant gated --voxel 2` into
   `runs/`: `mesh.exo`, `casting.inp`, `freeze.inp`.
3. `sbatch build.slurm`: libraries and Truchas with GCC 11.4.1, OpenMPI
   4.1.8 and OpenBLAS, on a compute node.
4. A pilot to measure the step rate:
   `sbatch --nodes=1 --ntasks=64 --time=00:30:00 run.slurm pilot casting.inp 1500`
5. The runs:
   `sbatch --nodes=2 --ntasks=128 --time=24:00:00 run.slurm fill casting.inp` and
   `sbatch --nodes=1 --ntasks=64 --time=12:00:00 run.slurm freeze freeze.inp`

Each run makes its pictures with `truchas_pictures.py` (a copy of
`scripts/truchas_pictures.py`) into `runs/NAME/pictures/`.

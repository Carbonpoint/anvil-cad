#!/bin/bash -l
#SBATCH --job-name=truchas-build
#SBATCH --account=binl
#SBATCH --partition=nova
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=32
#SBATCH --mem=64G
#SBATCH --time=03:00:00
#SBATCH --output=/ptmp/binl/agold/truchas/logs/build-%j.out
#
# Build the Truchas third-party libraries and Truchas itself for the Anvil
# kettle casting run. Sources and tarballs were staged on the login node;
# nothing is downloaded here.
set -euo pipefail
B=/ptmp/binl/agold/truchas
module purge
for m in gcc/11.4.1-l3zl3n5 openmpi/4.1.8-lctrfpx cmake/3.31.9-jfzpfbl openblas/0.3.30-openmp-37t7ek7; do
  module load $m
done
export PATH=$B/venv/bin:$PATH
export FFLAGS="-O2 -fallow-argument-mismatch"
export FCFLAGS="$FFLAGS"
echo "node $SLURMD_NODENAME job $SLURM_JOB_ID $(date)"
mpifort --version | head -1

cd $B
rm -rf tpl-build tpl && mkdir tpl-build && cd tpl-build
cmake -C ../truchas-tpl/config/linux-gcc.cmake -D CMAKE_INSTALL_PREFIX=$B/tpl \
  -D "CMAKE_Fortran_FLAGS=$FFLAGS" -D MUMPS_LAPACK_VENDOR=OpenBLAS -D BLA_VENDOR=OpenBLAS \
  ../truchas-tpl > cmake.log 2>&1
make -j 32 > make.log 2>&1
echo "TPL done $(date)"

cd $B
rm -rf truchas-build && mkdir truchas-build && cd truchas-build
cmake ../truchas -C ../truchas/config/linux-gcc.cmake -D CMAKE_BUILD_TYPE=Release \
  -D CMAKE_PREFIX_PATH=$B/tpl -D CMAKE_INSTALL_PREFIX=$B/install \
  -D PYTHON_EXECUTABLE=$B/venv/bin/python -D BLA_VENDOR=OpenBLAS > cmake.log 2>&1
make -j 32 > make.log 2>&1
make install > install.log 2>&1
ls $B/install/bin
echo "TRUCHAS done $(date)"

#!/usr/bin/env python3
"""How full and how frozen the cavity is at each Truchas output time.

Usage: truchas_fill.py casting_output/casting.h5

Reads a run of the case Anvil writes (anvil-cli kettle --variant gated, or
the truchas_plate example): block 2 is the cavity. The VOF columns follow
the materials and phases in the deck: sand, metal solid, metal liquid,
void. Needs h5py and numpy.
"""
import sys

import h5py
import numpy as np

f = h5py.File(sys.argv[1], "r")
sim = f["Simulations/MAIN"]
cavity = sim["Non-series Data/BLOCKID"][()] == 2
n = int(cavity.sum())
series = sorted(sim["Series Data"].keys(), key=lambda s: int(s.split()[-1]))
print(f"{n} cavity cells")
print("   time s   filled   frozen   hottest metal C")
for name in series:
    s = sim["Series Data"][name]
    vof = s["VOF"][()]
    t = s.attrs.get("time", [float("nan")])
    t = float(np.atleast_1d(t)[0])
    solid, liquid = vof[cavity, 1], vof[cavity, 2]
    metal = solid + liquid
    temp = s["Z_TEMP"][()][cavity]
    hot = temp[metal > 0.5].max() - 273.15 if (metal > 0.5).any() else float("nan")
    print(f"{t:9.3f}  {metal.sum() / n:6.1%}  {solid.sum() / max(metal.sum(), 1e-12):6.1%}  {hot:12.0f}")

#!/usr/bin/env python3
"""Isolated liquid pockets in a Truchas freeze run of an Anvil case.

Usage: truchas_pockets.py FREEZE.h5 OUT.json RISER_X RISER_Y RISER_Z

At each output time, the cavity cells that are not yet solid (solid
fraction under 0.99) are split into connected regions (six neighbours).
A region that no longer touches the riser, the cell nearest the given
riser point, is cut off from its feed while it freezes: a shrinkage risk,
the pocket Anvil's Hot spots feature predicts. Writes the pockets per
time, and when the part outside the riser is fully solid.
"""
import json
import sys

import h5py
import numpy as np

path, out = sys.argv[1], sys.argv[2]
riser = np.array([float(v) for v in sys.argv[3:6]])
f = h5py.File(path, "r")
sim = f["Simulations/MAIN"]
xyz = f["Meshes/DEFAULT/Nodal Coordinates"][()] * 1000.0
con = f["Meshes/DEFAULT/Element Connectivity"][()] - 1
cent = xyz[con].mean(axis=1)
step = float(np.min(np.ptp(xyz[con[0]], axis=0)))
lo = cent.min(axis=0)
ijk = np.rint((cent - lo) / step).astype(int)
n = ijk.max(axis=0) + 1
cav = sim["Non-series Data/BLOCKID"][()] == 2
cells = np.where(cav)[0]
index = -np.ones(n, dtype=np.int64)
index[ijk[cells, 0], ijk[cells, 1], ijk[cells, 2]] = np.arange(len(cells))
feed = int(np.argmin(np.linalg.norm(cent[cells] - riser, axis=1)))
# Cells within 20 mm of the riser point count as the riser.
near = np.linalg.norm(cent[cells] - riser, axis=1) < 20.0


def regions(liquid):
    """Connected regions of liquid cells, as lists of cavity indices."""
    seen = np.zeros(len(cells), bool)
    out = []
    for start in np.where(liquid)[0]:
        if seen[start]:
            continue
        stack, members = [start], []
        seen[start] = True
        while stack:
            c = stack.pop()
            members.append(c)
            i, j, k = ijk[cells[c]]
            for di, dj, dk in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)):
                a, b, e = i + di, j + dj, k + dk
                if 0 <= a < n[0] and 0 <= b < n[1] and 0 <= e < n[2]:
                    m = index[a, b, e]
                    if m >= 0 and liquid[m] and not seen[m]:
                        seen[m] = True
                        stack.append(m)
        out.append(members)
    return out


names = sorted(sim["Series Data"].keys(), key=lambda s: int(s.split()[-1]))
report = {"voxel_mm": step, "times": []}
part_solid = None
for name in names:
    s = sim["Series Data"][name]
    t = float(np.atleast_1d(s.attrs["time"])[0])
    vof = s["VOF"][()][cells]
    metal = vof[:, 1] + vof[:, 2]
    frac = np.where(metal > 1e-6, vof[:, 1] / np.maximum(metal, 1e-12), 1.0)
    liquid = frac < 0.99
    if part_solid is None and not (liquid & ~near).any():
        part_solid = t
    pockets = []
    for r in regions(liquid):
        r = np.array(r)
        if near[r].any():
            continue
        if len(r) < 3:
            continue
        c = cent[cells[r]].mean(axis=0)
        xs = cent[cells[r], 0]
        pockets.append(
            {
                "cells": int(len(r)),
                "cm3": round(len(r) * step**3 * 1e-3, 2),
                "centre_mm": np.round(c, 1).tolist(),
                "x_range_mm": [round(float(xs.min()), 1), round(float(xs.max()), 1)],
                "z_range_mm": [round(float(cent[cells[r], 2].min()), 1), round(float(cent[cells[r], 2].max()), 1)],
            }
        )
    pockets.sort(key=lambda p: -p["cells"])
    report["times"].append({"t": t, "liquid_cm3": round(float(liquid.sum()) * step**3 * 1e-3, 1), "pockets": pockets[:8]})
report["part_solid_s"] = part_solid
json.dump(report, open(out, "w"), indent=1)
for row in report["times"]:
    p = row["pockets"]
    desc = "; ".join(f"{q['cm3']} cm3 x {q['x_range_mm']} z {q['z_range_mm']}" for q in p[:3])
    print(f"{row['t']:6.0f} s  liquid {row['liquid_cm3']:6.1f} cm3  cut-off pockets {len(p)}  {desc}")
print("part outside the riser fully solid at", part_solid, "s")

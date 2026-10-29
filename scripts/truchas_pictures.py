#!/usr/bin/env python3
"""Pictures of a Truchas casting run from an Anvil case.

Usage: truchas_pictures.py RESULT.h5 OUT_DIR [fill|freeze]

The Anvil case is a voxel mesh (block 1 sand, block 2 the cavity), so the
cell centres put every cell back on a regular grid. Writes PNG frames and
a summary.json with the fill or freeze curve:

  fill:   side view (how full each column of the cavity is, seen along
          y) and a slice through the middle, at each output time.
  freeze: temperature and solid fraction on the middle slice, and a map
          of when each place froze (the last places are the hot spots).

Needs h5py, numpy and matplotlib.
"""
import json
import os
import sys

import h5py
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

path, out = sys.argv[1], sys.argv[2]
mode = sys.argv[3] if len(sys.argv) > 3 else "fill"
os.makedirs(out, exist_ok=True)

f = h5py.File(path, "r")
sim = f["Simulations/MAIN"]
xyz = f["Meshes/DEFAULT/Nodal Coordinates"][()] * 1000.0  # mm
con = f["Meshes/DEFAULT/Element Connectivity"][()] - 1
cent = xyz[con].mean(axis=1)
step = float(np.min(np.ptp(xyz[con[0]], axis=0)))
lo = cent.min(axis=0)
ijk = np.rint((cent - lo) / step).astype(int)
n = ijk.max(axis=0) + 1
cavity_cell = sim["Non-series Data/BLOCKID"][()] == 2


def grid(values, fill=np.nan):
    g = np.full(n, fill, dtype=float)
    g[ijk[:, 0], ijk[:, 1], ijk[:, 2]] = values
    return g


cav = grid(cavity_cell.astype(float), 0.0) > 0.5
extent_xz = [lo[0] - step / 2, lo[0] + (n[0] - 0.5) * step, lo[2] - step / 2, lo[2] + (n[2] - 0.5) * step]
names = sorted(sim["Series Data"].keys(), key=lambda s: int(s.split()[-1]))
times = [float(np.atleast_1d(sim["Series Data"][s].attrs["time"])[0]) for s in names]
# The slice through the middle of the cavity: its median y.
jmid = int(np.median(ijk[cavity_cell, 1]))
outline = cav[:, jmid, :].T


def frame(ax, img, title, cmap, vmin, vmax, label):
    im = ax.imshow(img, origin="lower", extent=extent_xz, cmap=cmap, vmin=vmin, vmax=vmax, interpolation="nearest")
    ax.contour(
        np.linspace(extent_xz[0], extent_xz[1], outline.shape[1]),
        np.linspace(extent_xz[2], extent_xz[3], outline.shape[0]),
        outline.astype(float),
        levels=[0.5],
        colors="0.35",
        linewidths=0.6,
    )
    ax.set_title(title, fontsize=10)
    ax.set_xlabel("x mm")
    ax.set_ylabel("z mm")
    ax.set_aspect("equal")
    plt.colorbar(im, ax=ax, shrink=0.8, label=label)


summary = {"mode": mode, "cells": int(cavity_cell.sum()), "voxel_mm": step, "times": [], "value": []}
ncav = cavity_cell.sum()
solid_time = np.full(len(cavity_cell), np.nan)
for k, (name, t) in enumerate(zip(names, times)):
    s = sim["Series Data"][name]
    vof = s["VOF"][()]
    temp = s["Z_TEMP"][()] - 273.15
    solid, liquid = vof[:, 1], vof[:, 2]
    metal = solid + liquid
    if mode == "fill":
        summary["times"].append(t)
        summary["value"].append(float(metal[cavity_cell].sum() / ncav))
        g = grid(metal, 0.0)
        side = np.where(cav.any(axis=1), (g * cav).sum(axis=1) / np.maximum(cav.sum(axis=1), 1), np.nan).T
        mid = np.where(cav[:, jmid, :], g[:, jmid, :], np.nan).T
        hot = np.where(metal > 0.5, temp, np.nan)
        tmid = np.where(cav[:, jmid, :], grid(hot)[:, jmid, :], np.nan).T
        fig, axs = plt.subplots(1, 3, figsize=(18, 5.2))
        frame(axs[0], side, f"Filled, seen along y, t = {t:.2f} s", "Blues", 0, 1, "share of the column")
        frame(axs[1], mid, "Metal on the middle slice", "Blues", 0, 1, "metal fraction")
        frame(axs[2], tmid, "Metal temperature, middle slice", "inferno", 1100, 1400, "C")
        fig.tight_layout()
        fig.savefig(os.path.join(out, f"fill_{k:03d}.png"), dpi=110)
        plt.close(fig)
    else:
        frac = np.where(metal > 1e-6, solid / np.maximum(metal, 1e-12), 0.0)
        newly = cavity_cell & np.isnan(solid_time) & (frac > 0.99)
        solid_time[newly] = t
        summary["times"].append(t)
        summary["value"].append(float((frac * cavity_cell).sum() / ncav))
        tm = np.where(cav[:, jmid, :], grid(temp)[:, jmid, :], np.nan).T
        fm = np.where(cav[:, jmid, :], grid(frac)[:, jmid, :], np.nan).T
        fig, axs = plt.subplots(1, 2, figsize=(13, 5.2))
        frame(axs[0], tm, f"Temperature, middle slice, t = {t:.0f} s", "inferno", 900, 1400, "C")
        frame(axs[1], fm, "Solid fraction, middle slice", "viridis", 0, 1, "solid")
        fig.tight_layout()
        fig.savefig(os.path.join(out, f"freeze_{k:03d}.png"), dpi=110)
        plt.close(fig)

if mode == "freeze":
    last = np.where(np.isnan(solid_time) & cavity_cell, np.inf, solid_time)
    g = grid(np.where(cavity_cell, last, np.nan))
    g[np.isinf(g)] = times[-1] * 1.05
    fig, axs = plt.subplots(1, 2, figsize=(13, 5.2))
    frame(axs[0], np.where(cav[:, jmid, :], g[:, jmid, :], np.nan).T, "When each place froze, middle slice", "magma", 0, times[-1], "s")
    side = np.nanmax(np.where(cav, g, np.nan), axis=1).T
    frame(axs[1], side, "Latest freeze along y (side view)", "magma", 0, times[-1], "s")
    fig.tight_layout()
    fig.savefig(os.path.join(out, "freeze_time_map.png"), dpi=120)
    plt.close(fig)
    still = cavity_cell & np.isnan(solid_time)
    order = np.argsort(np.where(cavity_cell, np.nan_to_num(solid_time, nan=np.inf), -1))[::-1]
    worst = [i for i in order if cavity_cell[i]][:50]
    summary["still_liquid_cells"] = int(still.sum())
    summary["last_to_freeze_mm"] = np.round(cent[worst].mean(axis=0), 1).tolist()
    summary["full_freeze_s"] = None if still.any() else float(np.nanmax(solid_time[cavity_cell]))

fig, ax = plt.subplots(figsize=(6, 3.4))
ax.plot(summary["times"], [100 * v for v in summary["value"]], marker="o", ms=3)
ax.set_xlabel("time s")
ax.set_ylabel("filled %" if mode == "fill" else "solid %")
ax.set_ylim(0, 102)
ax.grid(alpha=0.3)
fig.tight_layout()
fig.savefig(os.path.join(out, f"{mode}_curve.png"), dpi=120)
json.dump(summary, open(os.path.join(out, f"{mode}_summary.json"), "w"), indent=1)
print(json.dumps({k: v for k, v in summary.items() if k not in ("times", "value")}))
print("final", summary["value"][-1] if summary["value"] else None)

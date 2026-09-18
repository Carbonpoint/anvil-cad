#!/usr/bin/env python3
"""A design loop over an Anvil document from a script.

Runs `anvil-cli run` for a grid of wing parameters, reads each report,
and prints the lift to drag ratio the Aircraft feature estimated, so the
best planform is picked without opening the desktop app. Swap the
estimate for a solver result (XFOIL polar, OpenFOAM forces) by reading
your own numbers instead of the note.

Usage:
    python3 docs/examples/wing_loop.py plane.anvil --cli target/release/anvil-cli

The document must use expressions for the parameters it varies; the
Aircraft feature's fields can reference them (for example set the
feature's Span to `span` and define `span` in the Expressions panel).
"""
import argparse
import itertools
import json
import re
import subprocess
import tempfile
from pathlib import Path


def run(cli, doc, out, **params):
    args = [cli, "run", str(doc), "--formats", "3mf-group", "--out", str(out)]
    for name, value in params.items():
        args += ["--set", f"{name}={value}"]
    subprocess.run(args, check=True, capture_output=True, text=True)
    return json.loads((out / "report.json").read_text())


def numbers(report):
    """Lifting line L/D and the vortex lattice CL and CDi from the note.

    Lifting line is blind to sweep; the vortex lattice is not, so the
    loop ranks by the vortex lattice lift to induced drag when present.
    """
    for f in report["features"]:
        note = f.get("note") or ""
        ld = re.search(r"L/D ([0-9.]+)", note)
        vlm = re.search(r"vortex lattice at the same angle: CL ([0-9.]+), CDi ([0-9.]+)", note)
        cd0 = re.search(r"CD0 ([0-9.]+)", note)
        if ld:
            out = {"ld_lifting_line": float(ld.group(1))}
            if vlm and cd0:
                cl, cdi = float(vlm.group(1)), float(vlm.group(2))
                out["cl_vlm"] = cl
                out["cdi_vlm"] = cdi
                out["ld_vlm"] = cl / (cdi + float(cd0.group(1)))
            return out
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("doc", type=Path)
    ap.add_argument("--cli", default="anvil-cli")
    ap.add_argument("--sweep", default="0,5,10,15")
    ap.add_argument("--taper", default="0.4,0.6,0.8,1.0")
    a = ap.parse_args()
    sweeps = [float(x) for x in a.sweep.split(",")]
    tapers = [float(x) for x in a.taper.split(",")]
    best = None
    with tempfile.TemporaryDirectory() as tmp:
        for sweep, taper in itertools.product(sweeps, tapers):
            out = Path(tmp) / f"s{sweep}_t{taper}"
            out.mkdir()
            report = run(a.cli, a.doc, out, sweep=sweep, taper=taper)
            n = numbers(report) or {}
            ld = n.get("ld_vlm", n.get("ld_lifting_line"))
            print(
                f"sweep {sweep:5.1f}  taper {taper:4.2f}  L/D lifting line {n.get('ld_lifting_line')}"
                f"  vortex lattice {n.get('ld_vlm') and round(n['ld_vlm'], 2)}"
            )
            if ld is not None and (best is None or ld > best[0]):
                best = (ld, sweep, taper)
    if best:
        print(f"best: L/D {best[0]:.2f} at sweep {best[1]} and taper {best[2]}")


if __name__ == "__main__":
    main()

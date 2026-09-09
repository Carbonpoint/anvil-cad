# Research summary

Four literature and technology reviews were run on 2026-09-09 by Sonnet
research agents. Each report lists its sources. This page is the digest and
the design decisions that came from it.

| Report | Question | Key finding |
| --- | --- | --- |
| [01 Geometry kernels](01-geometry-kernels.md) | Own kernel, truck, or OCCT? | Hybrid: OCCT behind a trait for booleans, fillets, STEP; native and truck-style Rust where guarantees are achievable. From-scratch curved kernels have a poor track record (Fornjot). |
| [02 Parametric architecture](02-parametric-architecture-and-constraints.md) | How to structure history, solver, expressions, undo | One dependency-graph engine for features and expressions. Decide persistent naming early. Reuse a proven solver algorithm (SolveSpace-style Newton with least-squares dragging). Snapshot or persistent data structures for undo. |
| [03 GUI, rendering, plugins](03-gui-rendering-and-plugins.md) | Which toolkit, how to render on CPU, how to extend | egui/eframe (permissive, repaint on demand). wgpu falls back to lavapipe/llvmpipe but is unproven for CAD; a software path is a safe start. Data-driven ribbon with static registration (`inventory`). WASM for third-party plugins, Rhai for scripting. |
| [04 CAM, simulation, formats](04-cam-simulation-and-formats.md) | What NX CAM does, what exists in Rust, which formats | Keep a neutral toolpath and pluggable posts. Rust lacks a drop-cutter/waterline library and a Clipper2 port; `cavalier_contours` covers 2D offsets. STEP in pure Rust is not production ready; OCCT is. Shell-out plus file exchange for FEA hooks. |

## Decisions taken in the draft

1. Workspace of small crates with a strict dependency order.
2. `Kernel` trait as the backend seam. Native planar kernel now, OCCT at M3.
3. Expressions and features share one "evaluate in dependency order" model.
4. Features self-register with `inventory`; the ribbon is built from the registry.
5. egui with a software viewport. No GPU required.
6. Neutral toolpath plus `Post` trait for G-code.
7. JSON `.anvil` now, zip container later.

## Open questions

* Kernel choice at M3 is the one decision that changes the build and the
  license story. It is written up in ADR 0001 and should be revisited with
  fresh data on `truck` fillets and `opencascade-rs` API stability.
* Persistent naming design: shape-history mapping (FreeCAD 1.0 approach) is
  the recommended path once OCCT is in.

# ADR 0001: Geometry kernel strategy

Status: accepted for the draft; decision point at milestone M3.
Date: 2026-09-09.

## Context

The kernel decides whether Anvil can reach NX-class modeling. The research
review in `docs/research/01-geometry-kernels.md` reached these conclusions:

* A from-scratch curved B-rep kernel with robust booleans and fillets is a
  multi-year research project. Fornjot, a funded pure-Rust attempt, was shut
  down by its author without reaching that goal.
* `truck` is the best pure-Rust foundation. It has NURBS, topology, booleans,
  and STEP, but no fillets and much less real-world hardening than OCCT.
* OpenCASCADE (OCCT) is the only open-source kernel with NX-comparable
  booleans, fillets, and STEP AP242. It is LGPL 2.1 with a static-linking
  exception. Rust bindings exist (`opencascade-rs`, `occt-rs`).

## Decision

1. The draft ships a small **native planar-facet kernel** behind a `Kernel`
   trait. It exists so the feature engine, sketcher, UI, and CAM can be built
   and tested now, on any machine, with one `cargo build`.
2. All modeling code calls the `Kernel` trait, never the structs. This is
   enforced by review.
3. At milestone M3 the project adds an `OcctKernel` implementation as the
   load-bearing kernel for booleans, fillets, and STEP. The native kernel
   stays as the zero-dependency fallback and test kernel.
4. In parallel, Rust-native work is welcome where the guarantee is achievable:
   exact predicates (`robust` crate), 2D offsets (`cavalier_contours`), mesh
   booleans (Manifold) for preview and export.

## Consequences

* "Written from scratch in Rust" is true for everything except the curved
  kernel once M3 lands. The README must say this plainly.
* The build gains a C++ toolchain dependency at M3. CI must build OCCT on all
  three platforms. Vendoring through `opencascade-rs` keeps it to `cargo build`.
* Persistent naming of faces and edges must be designed together with the
  OCCT integration, using its shape-history API, not added later.

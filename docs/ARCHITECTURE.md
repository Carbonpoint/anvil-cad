# Architecture

This page describes how Anvil is put together today and which seams are
designed for replacement. Read `docs/research/00-summary.md` for the reasons.

## Data flow

```
ribbon click
  -> FeatureDescriptor::create()          (anvil-feature registry)
  -> Document::add_feature()              (snapshot for undo, push node)
  -> Document::regenerate()
       exprs.evaluate()                   (anvil-expr, dependency order)
       for each feature in order:
         feature.regenerate(ctx)          (anvil-sketch solve, anvil-kernel ops)
  -> anvil_io::document_mesh()            (parallel tessellation, anvil-kernel)
  -> viewport::draw()                     (CPU projection, egui shapes)
```

## Crate dependency order

```
anvil-math
  anvil-expr
  anvil-sketch
  anvil-kernel
    anvil-feature
      anvil-cam  anvil-io
        anvil-ui
          anvil-app
```

Lower crates never depend on higher crates. `anvil-feature` knows nothing
about egui. `anvil-ui` knows nothing about concrete features except in the
demo loader.

## Seams designed for replacement

* **Kernel backend.** Everything above the kernel calls the `Kernel` trait.
  `NativeKernel` is the planar-facet implementation. An OCCT-backed
  implementation (see ADR 0001) is a second struct implementing the same
  trait. Feature code does not change.
* **Sketch solver.** `anvil_sketch::solver::solve` is one function over a
  `Sketch`. A graph-decomposition front end or a SolveSpace port can replace
  it without touching entities or constraints.
* **Viewport.** `viewport::draw` takes a `TriMesh` and a camera. A glow or
  wgpu renderer is a second implementation of the same call (ADR 0003).
* **Posts.** `Post` is a trait. Each controller dialect is one struct.
* **Native file format.** `Document` is plain serde. The JSON writer in
  `anvil-io` is the only place that knows the container (ADR 0002).

## Extensibility

Features register themselves with `inventory::submit!`. The registry is read
once at startup to build the ribbon. This is compile-time extensibility for
first-party features. Sandboxed third-party plugins (WASM) are planned; the
research review in `docs/research/03-gui-rendering-and-plugins.md` explains
why WASM was chosen over dynamic libraries.

## Known limits of the draft

* Bodies do not combine. Each Extrude or Revolve makes a separate body. A
  boolean step is needed before "Extrude: subtract" can exist.
* Fillet returns `Unsupported`.
* The sketch has no interactive editor yet. Sketches are built in code.
* Face and edge selection in the viewport does not exist yet.
* Topological naming: features reference upstream features by index, and
  fillets reference all edges. Persistent naming is a roadmap item, not a
  retrofit; see the research summary for why it must come early.

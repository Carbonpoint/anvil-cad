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
  -> scene::Scene::build()                (parallel tessellation, feature edges)
  -> raster::Framebuffer::draw_scene()    (CPU z-buffer, id buffer for picking)
     or gpu::GpuViewport::callback()      (OpenGL through an egui paint callback,
                                           with raster::draw_scene_ids for picking)
  -> egui texture + overlays              (sketch entities, datum planes, triad)
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
* **Viewport.** `raster::Framebuffer::draw_scene` takes a `Scene` and a
  `Projector`. `gpu::GpuViewport` is the second implementation of the same
  call, in glow (ADR 0003), chosen in View > Settings > GPU viewport and
  used only when an OpenGL context is there. Picking reads the id buffer,
  so it stays on the CPU on either path: with the GPU drawing,
  `raster::Framebuffer::draw_scene_ids` fills a half resolution id and
  depth buffer and nothing else.
* **Posts.** `Post` is a trait. Each controller dialect is one struct.
* **Native file format.** `Document` is plain serde. The JSON writer in
  `anvil-io` is the only place that knows the container (ADR 0002).

## Extensibility

Features register themselves with `inventory::submit!`. The registry is read
once at startup to build the ribbon. This is compile-time extensibility for
first-party features. Sandboxed third-party plugins (WASM) are planned; the
research review in `docs/research/03-gui-rendering-and-plugins.md` explains
why WASM was chosen over dynamic libraries.

## Developer builds: pictures of the window

`anvil-ui` has one optional feature, `devtools`. It is off in every
normal build, so the shipped application carries none of it.

```
cargo run -p anvil-ui --features devtools --example shot -- --help
scripts/ui_shots.sh              # every picture the documents use
cargo test -p anvil-ui --features devtools
```

`crates/anvil-ui/src/shot.rs` runs the whole window headless and draws
egui's own triangles into a pixel buffer, so a picture needs no display
and no GPU. The 3D view inside the picture is the software viewport, the
same one the application uses.

Two things it buys:

* **Layout checks.** Every pixel starts magenta. A pixel still magenta
  at the end is an area no panel painted, which is how the gap beside an
  overflowing side panel was found. The test
  `shot::tests::the_window_has_no_unpainted_holes` guards it.
* **Documents.** `scripts/ui_shots.sh` writes `docs/images/ui_*.png`,
  used by `docs/USING_ANVIL.md` and `docs/tutorials/README.md`. Rerun it
  after a change to the ribbon, the panels or the theme.

A picture taken with the GPU viewport on shows a hole where the 3D view
is: the GL callback has no software form. Leave the GPU viewport off for
pictures.

## Known limits of the draft

* Bodies do not combine. Each Extrude or Revolve makes a separate body. A
  boolean step is needed before "Extrude: cut" and Hole can exist.
* Fillet, Chamfer, Shell, Combine, Hole return `Unsupported`.
* A sketch on a face stores the face plane as geometry, not as a reference.
  If the face moves, the sketch stays where it was.
* Edge selection in the viewport does not exist yet. Faces and bodies can
  be picked.
* Dimensions in sketches are numbers, not expressions, for now.
* Topological naming: features reference upstream features by index, and
  fillets reference all edges. Persistent naming is a roadmap item, not a
  retrofit; see the research summary for why it must come early.

# ADR 0003: CPU-first viewport

Status: accepted. Date: 2026-09-09.

## Context

The project must run smoothly with no GPU. The research review found that
`wgpu` falls back to Mesa `lavapipe`/`llvmpipe` on Linux and WARP on
Windows, but no Rust CAD project has proven that path end to end, and
software Vulkan is one to two orders of magnitude slower than hardware.

## Decision

* The draft viewport is a **software renderer inside egui**: transform, cull
  back faces, sort far to near, and emit filled polygons. Zero GPU
  dependency, identical output on all platforms, and easy to debug.
* egui is chosen for the whole UI because it repaints only on input, which
  leaves the CPU free for regeneration.
* At milestone M6 a `glow` (OpenGL) renderer becomes the default when a
  context is available, with the software path kept as the automatic
  fallback. Picking stays on the CPU (ray cast against a BVH) so it behaves
  the same on both paths.

## Consequences

* Triangle budget for the software path is on the order of tens of thousands
  per frame. Tessellation must be adaptive before large models are usable.
* Painter's-algorithm sorting can misdraw intersecting triangles. This is
  acceptable for the draft and disappears with a depth buffer at M6.

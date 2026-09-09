# Rust GUI, 3D Viewport Rendering, and Plugin Architecture for a CPU-Only Parametric CAD Application

Review date: 2026-09-09. Produced by a Sonnet research agent for the Anvil project.

## 1. Overview and constraints

The target is a Siemens NX-class parametric CAD/CAM application, written from scratch in Rust, that runs well on integrated graphics or pure CPU rendering, works on Linux, Windows, and macOS, and has a ribbon-style extensible GUI. That combination narrows the field quickly. Most Rust GUI and rendering crates assume a GPU is present. The good news is that several of them can degrade gracefully to a software rasterizer without any change to application code, because the software rasterizer sits underneath the graphics API rather than being a separate code path the app has to maintain.

## 2. GUI toolkits

**egui / eframe** is an immediate-mode toolkit: you call widget functions every frame and the UI is a direct result of that code, with no separate retained widget tree to keep in sync. It renders through either a `glow` (OpenGL) backend or a `wgpu` backend; `eframe` currently defaults to `wgpu` when both are compiled in. Because it only repaints on input or an explicit animation request, idle CPU use on a CAD app that is not being actively manipulated is close to zero, which matters when the same cores are also running a geometry kernel. Docking is handled by two well-known add-on crates, `egui_dock` and `egui_tiles` (see section 4). Accessibility comes from optional `AccessKit` integration, enabled by default in `eframe`, giving native screen-reader support on Windows and macOS and an experimental built-in screen reader on the web target; Linux/Orca support is behind AccessKit's own platform coverage. Licensing is MIT OR Apache-2.0, the friendliest possible choice for a from-scratch open-source app.

**iced** follows an Elm-inspired architecture (state, message, update, view) that behaves like a retained-mode toolkit even though the view function is re-evaluated on each update; its default native renderer is `iced_wgpu`. It is production-usable in 2026 but has real documentation gaps and a steeper learning curve than egui because of the message-passing model. License is MIT.

**Slint** is the toolkit most explicitly aimed at commercial desktop and embedded products. It uses a separate declarative `.slint` DSL compiled ahead of time, and it ships its own software renderer alongside GPU-backed (Skia, OpenGL, Vulkan) backends, which makes CPU-only operation a first-class, supported mode rather than an incidental fallback. AccessKit is integrated. Licensing changed in 2023: Slint 1.1 added a royalty-free permissive license (free for desktop/mobile/web with attribution, excluding embedded use), alongside the existing GPLv3 option and a paid commercial license that covers embedded targets and lets you set custom terms. This is worth flagging clearly to the project's stakeholders since it is not a plain MIT/Apache project.

**Dioxus** is React-style and, on desktop, uses a system webview (WebView2, WebKit) much like Tauri, so it inherits the OS webview's own rendering and gets platform accessibility (Narrator, VoiceOver, IME) largely for free. It also has an experimental native renderer, Blitz, that targets WebGPU directly instead of a webview. License is MIT OR Apache-2.0.

**gpui**, the framework behind the Zed editor, talks straight to the GPU (Metal on macOS, Vulkan on Linux) and is licensed Apache-2.0 even though Zed itself is GPL/AGPL. It is fast and clearly capable of driving a full IDE-class UI, but it has no documented CPU-only or software-rasterizer fallback path, and outside of Zed itself it remains immature as a general-purpose third-party toolkit with no ready-made docking or ribbon layer.

**Xilem/Masonry**, from the former Druid team, renders through Vello, a GPU compute-centric 2D renderer, and integrates AccessKit at the framework level rather than bolting it on later. As of the most recent releases it is explicitly alpha quality, still adding basics like multi-window support, and not yet recommended for production. Worth watching, not worth betting the project on today.

**Tauri** is not really a competing "GUI toolkit" for a 3D viewport; it wraps the OS's native webview (WebView2, WebKit, WebKitGTK) via `wry`/`tao`, keeping binaries small and letting the Rust backend own system access while the frontend is HTML/CSS/JS. A 3D viewport inside Tauri would have to go through the webview's own WebGL/WebGPU stack, adding an extra abstraction layer between the app and wgpu, which is the opposite of what a from-scratch CAD app wants for its main viewport.

For this project, egui/eframe plus egui_dock/egui_tiles is the most defensible default: permissive license, wgpu-based with automatic CPU fallback, existing docking crates, and an ecosystem full of tool-style apps (inspectors, editors) that already validate the "professional tool chrome" use case. Slint is worth a second look specifically because of its built-in software renderer, if the team is comfortable with its licensing model.

## 3. 3D viewport rendering on CPU-only hardware

`wgpu` is the natural choice for the 3D viewport: it is a safe, pure-Rust graphics API running on Vulkan, Metal, DX12, and OpenGL natively, and on WebGPU/WebGL2 in the browser. The key fact for this project is that `wgpu` gets CPU-only operation almost for free. On Linux, Mesa's `lavapipe` (a full software Vulkan 1.4 implementation built on the `llvmpipe` LLVM-JIT rasterizer) and `llvmpipe` itself for the GL backend both present themselves to the OS as ordinary adapters; `wgpu` will happily enumerate and use them, so an app written against `wgpu` runs unmodified on a machine with no GPU driver at all, just slower. On Windows, the DX12 backend has an equivalent CPU fallback in Microsoft's WARP software rasterizer. This means the "CPU fallback" story does not need bespoke code, only correct adapter selection and honest performance expectations (software rasterization is commonly one to two orders of magnitude slower than a real GPU, so LOD and culling matter more, not less).

The alternative is to skip `wgpu` entirely and build on `softbuffer` plus `tiny-skia`: `softbuffer` only blits an already-rendered pixel buffer into a window cross-platform, and `tiny-skia` is a minimal, CPU-only 2D rasterizer (fills, strokes, gradients, clipping, no text). Together they are a proven way to get pure-software 2D rendering, but there is no equivalent mature crate for software 3D triangle rasterization with a depth buffer; using this path for a 3D viewport means writing your own rasterizer core. Given that `wgpu`-on-`lavapipe`/`llvmpipe` already provides a full shader pipeline, depth buffering, and MSAA without extra work, it is the more practical route for the viewport; `tiny-skia`/`softbuffer` remain useful for 2D sketch views or lightweight overlay drawing.

It is instructive that no existing Rust CAD project has actually proven the "native wgpu viewport with CPU fallback" combination end to end. Fornjot, an early-stage B-rep kernel in Rust with its own `fj-viewer` crate, is explicitly discontinued; its own README states the project's goals were not reached and mainline development stalled for over a year before experiments replaced it. CADmium, a browser-based parametric CAD program, keeps its geometry kernel in Rust (built on the `truck` B-rep library, compiled to WebAssembly) but hands the actual 3D rendering to three.js/WebGL through a SvelteKit frontend rather than a native Rust renderer. Zoo's (KittyCAD) modeling-app goes further in the other direction: it runs a Rust/WASM client but streams rendered video frames back from a remote, GPU-accelerated geometry engine over WebSockets, which is explicitly not a local, GPU-optional architecture. So this project's specific goal, a native Rust viewport that is genuinely CPU-only-capable, is closer to novel systems work than to picking a proven template off the shelf.

Tessellating a B-rep for display is its own discipline. NURBS/B-rep faces need adaptive triangulation controlled by a chordal-deviation or angular tolerance, watertight along shared edges so adjacent faces do not crack apart at the seam, and cheap enough to redo often, since a from-scratch parametric app should retessellate only the faces that a given edit actually touched rather than the whole model. Level of detail matters for large assemblies: a component seen at assembly scale can use a much coarser tessellation than the same component isolated and zoomed in.

Picking (click-to-select) has two well-established strategies. The GPU id-buffer approach renders an extra pass that writes a per-object or per-sub-entity id into a target buffer, then reads back the pixel under the cursor; NVIDIA's own `vk_idbuffer_rasterization` sample exists specifically for efficient per-part IDs in CAD models, which shows this is the professional-tool-standard technique. Its cost is a GPU readback, which can stall the pipeline if not double- or triple-buffered, and it must also work correctly on the software-rasterizer fallback path. The CPU alternative is ray casting into a bounding volume hierarchy; the `parry3d` crate (from the same Dimforge ecosystem as the `rapier` physics engine) ships a SIMD-accelerated, rebalancing BVH built exactly for ray, point, and AABB queries. For a CPU-only-first CAD app, CPU BVH picking is attractive: it never needs a GPU readback, behaves identically whether the backend is real hardware or `lavapipe`/`llvmpipe`/WARP, and can test against the exact analytic surface rather than its tessellated approximation if the BVH indexes B-rep faces directly.

## 4. Docking and a data-driven ribbon

`egui_dock` provides binary-split docking (drag tabs left/right/top/bottom, pop tabs into new windows) and is the more mature, actively released crate. `egui_tiles`, sponsored by the Rerun project, is a newer and more flexible alternative that supports full grid layouts, not just binary splits, and exposes a `Behavior` trait for deep customization, at the cost of being earlier in development with fewer features than `egui_dock`.

There is no ready-made "ribbon" crate in the Rust ecosystem; the Microsoft Office-style ribbon (contextual tabs, galleries, overflow chevrons) has to be built from a toolkit's raw layout primitives regardless of which GUI library is chosen. The practical design is to make the ribbon data-driven rather than hand-coded: define a manifest type such as `RibbonTab { groups: Vec<RibbonGroup> }` where each group holds `RibbonControl` entries referencing a `CommandId`, an icon key, and an enabled predicate. A separate `CommandRegistry` maps each `CommandId` to its actual handler. Feature modules then contribute a manifest fragment plus their commands at startup using static registration (see next section), so a new feature is added by writing one `RibbonContribution` struct and registering it, never by editing a central ribbon-building function. This mirrors how extension systems in other tools (for example VS Code's `contributes.commands`) let independent modules add UI without touching shared code, and it gives the project a genuine "standard template for adding a new feature to the ribbon."

## 5. Extensibility architecture

For first-party, compiled-in features, Rust has two well-known static-registration crates. `inventory` collects plugin registrations across every linked source file using life-before-main constructors (similar to C's `__attribute__((constructor))`), works across Linux, macOS, Windows, WASM, and more, and even picks up registrations from dynamically loaded libraries at `dlopen` time. `linkme` achieves a similar distributed-registry effect through linker sections instead of runtime constructors, avoiding constructor-time cost. Either is a good fit for letting feature crates self-register ribbon commands and geometry operators without a central list, but both require the plugin code to be compiled into the same binary (or a shared library loaded before the registry is read).

For true third-party binary plugins, the low-level tool is `libloading`, a thin cross-platform wrapper over `dlopen`/`LoadLibrary`. The hard problem is that Rust has no stable ABI, so a plugin built with a different compiler version, or even a different build of the same version, can silently disagree with the host about type layout. `abi_stable` and the newer `stabby` both address this by giving you a C-compatible-safe subset of Rust types (structs, enums, trait objects, and in stabby's case even `Future`s) with a guaranteed stable layout, so host and plugin only exchange ABI-safe types across the boundary; `stabby` additionally checks canary symbols so a mismatched plugin fails to load cleanly instead of crashing. The constraint is that your entire plugin API surface must be expressed in that ABI-safe vocabulary, a real ergonomics cost.

WASM plugins (via `wasmtime`, the Cranelift-JIT runtime, or `extism`, a friendly plugin SDK built on top of it) trade a little raw speed for real sandboxing: a broken or malicious plugin cannot corrupt host memory or the document model, plugin authors can write in any language that compiles to WASM, instantiation is fast enough (microseconds) for interactive use, and the resulting plugin binary is portable across every OS and CPU architecture the host runs on, which matches this project's own cross-platform goal directly. The cost is that data has to be marshaled across the WASM boundary (serialized geometry buffers rather than shared native Rust structs) and heavy geometry computation inside WASM is somewhat slower than native code.

Scripting engines serve a different need: end-user macros and design automation rather than compiled extensions. `rhai` gives the most native-feeling two-way binding to Rust functions and types without `unsafe`, and is a reasonable analog to NX's Journaling or SolidWorks macros. `mlua` embeds Lua, already familiar to many CAM/manufacturing users from CNC post-processor customization. `rune` is younger, with an async-first VM and Rust-like syntax but a smaller ecosystem. `pyo3` embeds full CPython, unlocking numpy/scipy for engineering users at the cost of the heaviest packaging burden and the weakest sandboxing of the group. A sensible layering for this project is native Rust modules for built-in features, WASM for sandboxed third-party plugins, and `rhai` (with `pyo3` as an optional power-user add-on) for scripting and journaling.

## 6. Performance for smooth CPU-only operation

`rayon`'s work-stealing `par_iter` scales automatically to whatever core count is available, which matters because "CPU-only" target hardware ranges from a four-core laptop to a thirty-two-core workstation; it is the natural tool for parallel tessellation, BVH construction, and per-face surface evaluation, since B-rep faces are largely independent work items. For regeneration, the real performance problem in parametric CAD is not raw speed but doing only the necessary recompute after one parameter changes. `salsa`, the incremental-computation framework that powers `rust-analyzer`, models computations as memoized, dependency-tracked queries with "early cutoff" (a query that recomputes to the same value does not force its dependents to rerun); mapping each CAD feature (extrude, fillet, pattern) onto a salsa-style query graph gives exactly the selective, history-based regeneration users expect from NX or SolidWorks. Regardless of GUI toolkit, avoiding per-frame allocation matters more on CPU-only hardware than on a GPU-backed system, since there is no spare headroom to hide allocator stalls; this means reusing preallocated vertex/index buffers across tessellation passes instead of reallocating them each time. Finally, the immediate-versus-retained choice has a direct performance consequence here specifically because it is CPU-only: an immediate-mode toolkit like egui that only repaints on input or animation keeps idle CPU near zero, leaving the cores free for the geometry kernel, while a naively continuous 60fps redraw loop in any toolkit would compete directly with that kernel for the same cores. Whatever toolkit is chosen, "repaint on demand" rather than "always redraw" is the property that actually matters for this project.

## References

- [egui GitHub repository](https://github.com/emilk/egui)
- [wgpu GitHub repository](https://github.com/gfx-rs/wgpu)
- [Slint official site](https://slint.dev/)
- [Slint royalty-free license text](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)
- [Dioxus project structure docs](https://dioxuslabs.com/learn/0.7/beyond/project_structure/)
- [Zed is now open source](https://zed.dev/blog/zed-is-now-open-source)
- [gpui crate source in Zed repository](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- [Xilem GitHub repository](https://github.com/linebender/xilem)
- [Tauri v2 architecture concept](https://v2.tauri.app/concept/architecture/)
- [Fornjot GitHub repository (no longer in development)](https://github.com/hannobraun/fornjot)
- [Fornjot project site](https://www.fornjot.app/)
- [CADmium README](https://github.com/CADmium-Co/CADmium/blob/main/README.md)
- [CADmium: A Local-First CAD Program Built for the Browser](https://mattferraro.dev/posts/cadmium)
- [KittyCAD/modeling-app repository](https://github.com/KittyCAD/modeling-app)
- [KittyCAD/modeling-app DeepWiki overview](https://deepwiki.com/KittyCAD/modeling-app)
- [LLVMpipe and Lavapipe software renderers explainer](https://deepwiki.com/arehnman/virtio-win-mesa/3.4-llvmpipe-and-lavapipe:-software-renderers)
- [wgpu headless llvmpipe issue discussion](https://github.com/gfx-rs/wgpu/issues/1551)
- [NVIDIA vk_idbuffer_rasterization sample for CAD part IDs](https://github.com/nvpro-samples/vk_idbuffer_rasterization)
- [Parry collision-detection library](https://parry.rs/)
- [Parry GitHub repository](https://github.com/dimforge/parry)
- [Crash course on CAD data, Part 3: BRep vs. Mesh](https://cadexchanger.com/blog/crash-course-on-cad-data-part-3/)
- [egui_dock on crates.io](https://crates.io/crates/egui_dock)
- [egui_tiles on lib.rs](https://lib.rs/crates/egui_tiles)
- [inventory crate GitHub repository](https://github.com/dtolnay/inventory)
- [stabby GitHub repository](https://github.com/ZettaScaleLabs/stabby)
- [Plugins in Rust: Getting Started (NullDeref)](https://nullderef.com/blog/plugin-start/)
- [Plugins in Rust: Reducing the Pain with Dependencies (NullDeref)](https://nullderef.com/blog/plugin-abi-stable/)
- [Extism on lib.rs](https://lib.rs/crates/extism)
- [Wasmtime security documentation](https://docs.wasmtime.dev/security.html)
- [A Survey of Rust Embeddable Scripting Languages](https://www.boringcactus.com/2020/09/16/survey-of-rust-embeddable-scripting-languages.html)
- [Rust embedded scripting languages benchmark](https://github.com/khvzak/script-bench-rs)
- [Rayon GitHub repository](https://github.com/rayon-rs/rayon)
- [Salsa GitHub repository](https://github.com/salsa-rs/salsa)
- [Salsa overview documentation](https://salsa-rs.github.io/salsa/overview.html)
- [Proving Immediate Mode GUIs are Performant](https://www.forrestthewoods.com/blog/proving-immediate-mode-guis-are-performant/)
- [AccessKit project site](https://accesskit.dev/)
- [softbuffer GitHub repository](https://github.com/rust-windowing/softbuffer)
- [tiny-skia GitHub repository](https://github.com/linebender/tiny-skia)
- [softbuffer announcement thread](https://users.rust-lang.org/t/new-library-for-gpu-less-2d-display-in-winit-softbuffer-is-now-ready-for-use/70591)
- [The Rust GUI Landscape in 2026](https://wrenlearnsrust.com/posts/2026-03-11-rust-gui-landscape-2026.html)

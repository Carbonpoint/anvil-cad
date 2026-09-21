# Task 3: read STEP and 3MF, so Fusion files open in Anvil

## Why

The user has a large library of Fusion `.f3d` files. That format is
Autodesk's own and there is no public reader, so the route in is a
neutral file: Fusion exports STEP or 3MF, and Anvil reads it. Anvil
already writes both.

## What to build

In `crates/anvil-io`:

```rust
/// Bodies read from a file, with anything the reader could not use.
pub struct Imported {
    pub solids: Vec<anvil_kernel::Solid>,
    pub meshes: Vec<anvil_kernel::TriMesh>,
    /// One line per thing skipped, for example
    /// "12 faces on a CYLINDRICAL_SURFACE were skipped".
    pub notes: Vec<String>,
}

pub fn read_step(path: &std::path::Path) -> Result<Imported, IoError>;
pub fn read_3mf(path: &std::path::Path) -> Result<Imported, IoError>;
```

### STEP

Part 21 text. Read the entity list, then build Bodies from it.

Handle: `CARTESIAN_POINT`, `DIRECTION`, `AXIS2_PLACEMENT_3D`,
`VERTEX_POINT`, `EDGE_CURVE` with `LINE`, `ORIENTED_EDGE`, `EDGE_LOOP`,
`FACE_BOUND`, `FACE_OUTER_BOUND`, `ADVANCED_FACE` on a `PLANE`,
`CLOSED_SHELL`, `MANIFOLD_SOLID_BREP`, and the millimetre unit that the
Anvil writer emits.

A face on any other surface (`CYLINDRICAL_SURFACE`, `B_SPLINE_SURFACE`
and the rest) is skipped, counted, and named in `notes`. A file made
only of such faces gives an empty `solids` and a note, not an error.

A file that is not Part 21, or that ends in the middle of an entity,
gives `Err`. Never panic on input.

Entity ids are not dense and not ordered. Do not assume `#1` is first.
Values may span lines, and strings may hold commas and quotes.

### 3MF

A 3MF is a zip holding `3D/3dmodel.model`, which is XML with
`<vertices>` and `<triangles>`. Read both stored and deflated entries:
`flate2` is already a dependency of this crate for that purpose.

Each `<object>` with a `<mesh>` becomes one `TriMesh`. Ignore the
`<components>` graph in this task; note it instead. Units: 3MF says
`unit="millimeter"` in the model tag. Refuse any other unit with a
clear error, rather than importing the wrong size.

### In the window

* `MeshFeature` ("Insert Mesh") takes `.stl`, `.3mf` and `.step`. It
  keeps its meshes for STL and 3MF.
* A STEP file gives Bodies, not a mesh, so it goes to a new Feature
  `import_step` labelled "Import STEP", in the Solid tab, Insert panel.
  Copy `MeshFeature` for the file path parameter and the dialog.
* Anything the reader skipped shows as the Feature's note, so the user
  sees "18 faces on a CYLINDRICAL_SURFACE were skipped" in the Part
  Navigator rather than a silently wrong part.

## The test file

`crates/anvil-io/tests/neutral_import.rs` is copied in. Do not edit it.

## Done means

* `cargo fmt --all --check`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace`

## Do not

* Do not add a dependency. The crates you may use are already in
  `Cargo.toml` and vendored; the node has no internet.
* Do not try to read `.f3d`. It is Autodesk Shape Manager's own format
  and it is out of scope.
* Do not use an em dash anywhere.

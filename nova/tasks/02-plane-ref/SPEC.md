# Task 2: a plane parameter that can name a face

## The problem

Midplane takes two planes. Both parameters are free text today, and
`plane_by_ref` in `crates/anvil-feature/src/features/solid_extra.rs`
accepts only `XY`, `XZ`, `YZ` or the number of a plane Feature. A face
of a Body is not a plane Feature, so the user cannot say "the bottom of
the Revolve and the top of the square", which is the first thing anyone
tries. There is also no picker: the Properties panel shows a text box.

Offset Plane, Plane at Angle and Split Body have the same limit.

## What to build

### 1. `PlaneRef`, a reference to any plane

New file `crates/anvil-feature/src/plane_ref.rs`, exported as
`anvil_feature::plane_ref` and re-exported as `anvil_feature::PlaneRef`:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlaneRef {
    /// Nothing chosen yet.
    None,
    /// A datum plane by name: "XY", "XZ" or "YZ".
    Datum(String),
    /// A Feature whose output carries a plane (a sketch, an offset
    /// plane, a midplane, and so on).
    Feature(usize),
    /// A planar face of a Body, found again by its geometry.
    Face {
        /// Feature that made the Body.
        feature: usize,
        /// Which Body of that Feature, by index.
        body: usize,
        /// The plane as measured when the user picked the face.
        plane: anvil_math::Plane,
    },
}

impl PlaneRef {
    /// Short text for the Properties panel, for example "XY",
    /// "3: Extrude" or "Face of 3: Extrude".
    pub fn label(&self, doc: &Document) -> String;
    /// True when this reference needs `feature` to exist.
    pub fn depends_on(&self) -> Option<usize>;
}
```

`Plane` already derives what it needs for serde. Check, and add it if
not.

### 2. Resolving a reference

On `RegenContext`, next to `plane_of`:

```rust
pub fn plane_of_ref(&self, r: &PlaneRef) -> Result<Plane, RegenError>;
```

* `None` gives `RegenError::Other("no plane chosen")`.
* `Datum(name)` gives the datum plane; an unknown name is an error that
  names it.
* `Feature(i)` is exactly `plane_of(i)` today.
* `Face { feature, body, plane }` looks in that Feature's Body for a
  planar face that matches `plane`:
  * the face's normal is within 1e-6 of the stored normal, in either
    direction, and
  * the face's centre lies on the stored plane within 1e-6 times the
    Body's diagonal.
  * Several matches: take the one whose centre is nearest the stored
    origin.
  * The matched face gives the plane that is returned, with the stored
    x axis projected onto it, so a plane keeps its orientation.
  * **No match**: return the stored plane and record a note on the
    Feature: "the picked face moved, the last known plane is used".
    Do not fail. A missing Feature or Body **is** an error, through
    `RegenError::BadReference`.

This is the first half of the "persistent face and edge references" item
on the roadmap. Keep it in the kernel-free part: use only
`anvil_kernel::Solid` accessors that already exist
(`faces`, `face_normal`, `face_normal_area`, `pos`, `bounds`).

### 3. The parameter kinds

In `crates/anvil-feature/src/param.rs`:

* `ParamValue::Plane(PlaneRef)`
* `ParamKind::PlaneRef`
* `ParamSpec::plane(name, label, value)` helper, like the others.

### 4. Midplane uses it

`MidplaneFeature { first: PlaneRef, second: PlaneRef }`, with
`Default` = `(Datum("XY"), Datum("XZ"))`.

Old documents must still load. A saved file holds
`"first":"XY","second":"0"`. Accept both shapes: a string becomes
`Datum` if it is XY, XZ or YZ, and `Feature(n)` if it parses as a
number. Do this with a custom `Deserialize` for `PlaneRef`, or a
`#[serde(untagged)]` helper. `nova/tasks/02-plane-ref/fixtures/old_midplane.json`
is such a file, and the test loads it.

`name()` stays in the same shape: `Midplane (XY | 3: Extrude)`.

Offset Plane, Plane at Angle and Split Body keep their current
parameters in this task. Only Midplane changes. Adding the rest is the
next task, not this one.

### 5. The picker in the window

In `crates/anvil-ui`:

* `panels::property_panel` draws `ParamKind::PlaneRef` as a row with:
  * a dropdown listing `XY`, `XZ`, `YZ` and every earlier Feature whose
    output has a plane, and
  * a **Pick in view** button.
* Pressing it puts the application in a picking state:
  `AnvilApp::plane_pick: Option<(usize, &'static str)>`, the Feature and
  the parameter name.
* While picking:
  * the status bar says "Click a flat face, or a datum plane. Escape
    cancels.",
  * hovering a planar face highlights it as face hover does today,
  * a click sets the parameter to `PlaneRef::Face { .. }` with the plane
    from `Scene::face_plane`, then leaves the picking state,
  * clicking a datum square sets `PlaneRef::Datum`,
  * Escape leaves the picking state and changes nothing.
* A face that is not planar is refused with a message. Use the face's
  `Surface` to tell.

Region picking, from the last session, is the pattern to copy: a mode
that is on only while the user asked for it.

## The test files

Copied in before you start. Do not edit them:

* `crates/anvil-feature/tests/plane_ref.rs`
* `crates/anvil-feature/tests/fixtures/old_midplane.json`

## Done means

* `cargo fmt --all --check`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace`
* `cargo test -p anvil-ui --features devtools`

## Do not

* Do not store a `FaceId` in a document. It is a slot key and it does
  not survive a regenerate.
* Do not break the file format for documents that have no Midplane.
* Do not use an em dash anywhere.

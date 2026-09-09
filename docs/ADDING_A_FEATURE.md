# Adding a feature

This page shows how to add a new modeling feature to Anvil. A feature is one
step in the part history, for example Extrude. When you follow these steps,
the ribbon, the part navigator, the property panel, save and load, and undo
pick up the feature without further edits.

## 1. Copy the template

Copy `crates/anvil-feature/src/features/template.rs` to a new file in the
same folder, for example `chamfer.rs`. Add one line to `features/mod.rs`:

```rust
pub mod chamfer;
```

## 2. Define the data

Rename `TemplateFeature`. Add the fields your feature needs. Store lengths and
angles as `String` expressions, not as numbers. The document evaluates them at
regeneration time, so `distance = "h * 2"` keeps working when `h` changes.

The struct must derive `Serialize` and `Deserialize`. The `#[typetag::serde(name = "...")]`
attribute on the `impl Feature` block must use a unique name. That name is
written into `.anvil` files. Do not change it after release.

## 3. Describe the parameters

`params()` returns a `Vec<ParamSpec>`. Each spec tells the property panel what
to draw. Use the helpers: `ParamSpec::length`, `angle`, `boolean`,
`feature_ref`, `choice`. A `feature_ref` lists the feature type ids it accepts,
so a Chamfer that needs a body can accept `["extrude", "revolve"]`.

`set_param()` receives the edited value. Update your field. Return `Err` with a
short message for anything you do not accept.

## 4. Build geometry

`regenerate()` receives a `RegenContext`. It gives you:

* `ctx.eval("expr")` to evaluate an expression string.
* `ctx.profiles_of(idx)` to get the plane and closed profiles of a sketch feature.
* `ctx.upstream[idx]` for the full output of any earlier feature.
* `ctx.kernel` for geometry operations.

Return a `FeatureOutput`. Put new solids in `bodies`. A feature that only
makes construction geometry returns an empty output.

If the kernel cannot do what you need yet, return the kernel error. The
document stores it on the feature and shows it in the navigator as `!`. The
rest of the history still regenerates.

## 5. Register the descriptor

Edit the `inventory::submit!` block. Remove the `#[cfg(any())]` line above
it. Set:

* `id`: stable string, same as `type_id()`.
* `label`: button text.
* `tab` and `group`: where the button goes. Existing tabs: File, Home, CAM,
  View. A new tab name creates a new tab.
* `tooltip`: one sentence.
* `order`: lower numbers sit further left inside the group.

## 6. Test

Add a test in `crates/anvil-feature/tests/`. Build a `Document`, add your
feature, and check `doc.bodies()` or the feature output. Run:

```
cargo test -p anvil-feature
```

## Adding an app command instead

Commands that are not features (Save, Fit view, Export) live in
`crates/anvil-ui/src/ribbon.rs`. Add a `RibbonAction` variant, add a row in
`app_actions()`, and handle the variant in `AnvilApp::run_action`.

# Anvil CAD: notes for AI assistants

* Read `README.md`, then `docs/ARCHITECTURE.md`, then `docs/ROADMAP.md`.
* Vocabulary is fixed in `README.md` under "Vocabulary". Use those words.
* Never use em dashes in code comments, docs, or commit messages.
* All geometry calls go through the `Kernel` trait. Do not call
  `anvil_kernel::ops` from features or UI.
* New features: copy `crates/anvil-feature/src/features/template.rs` and
  follow `docs/ADDING_A_FEATURE.md`. Do not edit the ribbon to add a feature.
* Lengths and angles in features are expression strings, not numbers.
* Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace` before committing. CI enforces all three.
* Keep the software viewport working. A GPU path is an addition, not a
  replacement.

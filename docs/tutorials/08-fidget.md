# 8. The Fidget back end: what it buys, and its limits

Goal: know when to compile a field instead of interpreting it, and know
what you give up. This tutorial is shorter than the others because the
back end is behind a cargo feature and is not wired into the app yet.

## Step 1: the expression tree

`crates/anvil-implicit/src/expr.rs` holds a second way to describe a
field: not a Rust type implementing `Field`, but a tree of arithmetic.

```
pub enum Expr {
    X, Y, Z, Const(f64),
    Add, Sub, Mul, Div, Neg, Sqrt, Abs, Min, Max, Sin, Cos,
}
```

The `build` module writes the same shapes the rest of the crate has, in
that language: `sphere`, `boxed`, `union`, `intersect`, `subtract`,
`offset`, `shell`, `smooth_union`, `gyroid` and `schwarz`. A tree
evaluates natively with `Expr::eval`, and `Expr::size` counts its nodes,
which is how you tell a cheap field from an expensive one.

The point of writing a field twice is that a tree can be **compiled**. A
`Field` implementation is a Rust closure the CPU calls once per sample.
A tree can be handed to a JIT that emits machine code for the whole
expression, evaluates it over an array of points at once, and prunes
whole regions by interval arithmetic before it evaluates anything.

## Step 2: turn it on

Fidget is optional, and off by default, so CI and the Docker image stay
small:

```
cargo test -p anvil-implicit --features fidget
```

```
test expr::tests::expressions_match_the_native_fields ... ok
test expr::tests::fidget_agrees_and_meshes ... ok
...
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.36s
```

Without the feature the same command runs 30 tests. The thirty first,
`expr::tests::fidget_agrees_and_meshes`, is the one the feature adds.

## Step 3: what that test actually checks

It is the whole contract of the back end in twenty lines. A sphere of
radius 10 with a 6 x 6 mm square hole drilled through it, built as a
tree, then:

1. `jit::Compiled::new(&e)` compiles the tree.
2. `eval_many` evaluates 50 points in one call and every value agrees
   with `Expr::eval` to 1e-3.
3. `Compiled::mesh(lo, hi, depth)` meshes it with Fidget's manifold dual
   contouring, and the volume of the result is within 5 percent of the
   exact answer, `4/3 pi r^3` minus the drilled prism.

So: the compiled field agrees with the interpreted one, and the mesh it
produces is a real solid, not just a picture.

## Step 4: when it is worth it

```
   native Field                    compiled tree
   one virtual call per sample     machine code, many points per call
   no pruning                      interval arithmetic prunes empty regions
   any Rust code, any data         closed form only, no lookups
```

Use the tree when the field is closed form and you will evaluate it
millions of times: a parameter sweep over the same shape, an interactive
preview, or a mesh at a fine resolution. Stay with the `Field` types
when the field touches data (a `Sampled` mesh, a `PointMap` CSV, a
`GridField`), because a lookup table is not an expression.

That last point is the real boundary. Every tutorial before this one
sampled an STL or read a CSV at some stage, so none of them can go
through the tree as written.

## Why it works

An implicit shape is arithmetic, and arithmetic can be compiled. The
same tree that evaluates as a scalar at a point evaluates as an interval
over a box, and an interval that does not contain zero proves the
surface is not in that box. That is the pruning, and it is why a
compiled implicit kernel can mesh a complicated shape faster than an
interpreted one even though both do the same arithmetic. Anvil's own
surface nets does a weaker version of the same trick, skipping blocks
whose corners are further from the surface than the block diagonal.

## Not yet available

This is the shortest tutorial because most of the back end is still
future work.

* **Nothing routes through it.** Lattice fill, Density body and Aircraft
  all use the native `Field` types. Milestone F5 lists "routing Lattice
  fill through the tree when every part of its field is closed form" as
  open.
* **No command line or UI switch.** The only way in is Rust, behind the
  `fidget` cargo feature.
* **No sampled or data fields in the tree.** `Sampled`, `PointMap` and
  `GridField` have no `Expr` form, and cannot have one without a lookup
  node.
* **No beam lattices, honeycomb, `Warp`, `CellRamp` or `Graded` in the
  tree.** The `build` module stops at gyroid and Schwarz P.
* **No GPU path.** Fidget can do one; Anvil does not use it.
* **No benchmark in the repository.** The speed claims above are what
  the method gives in general, not a measurement of this code. If you
  need the number for your field, measure it.

Back to the [index](README.md).

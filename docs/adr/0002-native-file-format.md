# ADR 0002: Native file format

Status: accepted. Date: 2026-09-09.

## Decision

Today `.anvil` is pretty-printed JSON of the `Document` produced by serde.
Feature trait objects are tagged by `typetag` name. Geometry is never saved;
it is rebuilt by regeneration on load.

At milestone M4 the format becomes a zip container:

```
manifest.json     document graph, expressions, feature parameters, version
geom/<id>.bin     optional cached B-rep or mesh blobs, content addressed
thumb.png         preview
```

This follows FreeCAD's `.FCStd` pattern, which the research review found to
be the strongest open precedent. SQLite was rejected as a heavier dependency
for no gain at this size.

## Rules

* `manifest.json` carries a `format_version` integer. Loaders migrate forward.
* Typetag names are part of the format. Never rename one after release.
* Cached geometry is optional. A file with no `geom/` folder must load.

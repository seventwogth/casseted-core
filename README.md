# casseted-core

`casseted-core` is the foundational Rust workspace for Casseted, an open-source shader and GPU-processing library focused on physically and mathematically grounded analog and VHS-style image transformation.

The repository currently provides only the initial core layout:

- a Cargo workspace with small, focused crates
- a shared shader directory for WGSL sources
- lightweight docs for architecture and early decisions
- placeholders for reference assets and examples

At this stage the project does not implement a full GPU pipeline, real image processing, video support, web targets, or API infrastructure.

## Workspace crates

- `casseted-types`: shared domain types such as frame size and pixel format
- `casseted-signal`: minimal signal-domain configuration for analog-style transforms
- `casseted-shaderlib`: built-in WGSL shader source registry
- `casseted-gpu`: small `wgpu`-backed GPU configuration helpers
- `casseted-pipeline`: composition layer that ties types, signal settings, and shaders together
- `casseted-cli`: tiny CLI entry point for inspecting the current scaffold
- `casseted-testing`: shared test helpers for workspace crates

## Repository layout

```text
.
|-- assets/
|   `-- reference-images/
|-- crates/
|   |-- casseted-cli/
|   |-- casseted-gpu/
|   |-- casseted-pipeline/
|   |-- casseted-shaderlib/
|   |-- casseted-signal/
|   |-- casseted-testing/
|   `-- casseted-types/
|-- docs/
|   |-- architecture/
|   `-- decisions/
|-- examples/
`-- shaders/
```

## Current status

The workspace is intended as a clean starting point for the next iteration. Every crate contains minimal but meaningful code and is expected to compile together with `cargo check`.

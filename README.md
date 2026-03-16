# casseted-core

`casseted-core` is the foundational Rust workspace for Casseted, an open-source shader and GPU-processing library focused on physically and mathematically grounded analog and VHS-style image transformation.

The repository currently provides a compact first milestone:

- a Cargo workspace with small, focused crates
- a shared shader directory for WGSL sources
- lightweight docs for architecture and early decisions
- placeholders for reference assets and examples
- a small GPU-independent domain model for frame metadata, current still-preview controls, and the formal VHS / analog v1 parameter model
- a first still-image GPU pipeline that now implements a model-aligned subset of signal-model v1
- a CLI utility for running one PNG image through that pipeline

At this stage the project does not implement video support, a multi-pass render graph, web targets, SDK layers, or API infrastructure.

## Workspace crates

- `casseted-types`: shared frame/image metadata and pixel format types
- `casseted-signal`: current prototype effect controls plus the formal VHS / analog v1 signal model
- `casseted-shaderlib`: built-in WGSL shader source registry
- `casseted-gpu`: thin headless `wgpu` runtime setup and shader-module helpers
- `casseted-pipeline`: first still-image processing pipeline built on the core foundation
- `casseted-cli`: local PNG-to-PNG CLI utility for developer-facing pipeline checks
- `casseted-testing`: shared helpers for test images, image diffs, and basic assertions

The main layer boundary is intentionally simple:

- `casseted-shaderlib` owns repository shader assets
- `casseted-gpu` owns low-level runtime setup
- `casseted-pipeline` is the layer that bridges the two for real processing

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

The workspace now acts as a clean first milestone plus the first algorithmic signal-model step: it contains one real still-image GPU effect, one working CLI, and documentation that ties the current implementation to the formal v1 model.

The current implementation path is anchored by:

- [`docs/architecture/signal-model-v1.md`](./docs/architecture/signal-model-v1.md)
- [`docs/math/signal-model-v1-formulas.md`](./docs/math/signal-model-v1-formulas.md)

## CLI

The current CLI is a local developer utility for running the first still-image pipeline on one PNG image.

Basic usage:

```bash
cargo run -p casseted-cli -- input.png output.png
```

Example with a few effect overrides:

```bash
cargo run -p casseted-cli -- input.png output.png --luma-blur 1.5 --chroma-offset 1.25 --line-jitter 0.8
```

Current notes:

- input is read as PNG
- output is written as PNG
- if no flags are provided, the built-in mild analog defaults are projected from `VhsModel::default()`

## Testing

Current testing is intentionally lightweight:

- unit tests for domain and support crates
- GPU smoke tests for the still-image pipeline
- a CLI smoke test that reads a PNG, runs the pipeline, and writes a PNG
- shared helpers in [`docs/testing.md`](./docs/testing.md)

### TESTING // HELLO

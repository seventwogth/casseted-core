# casseted-core

`casseted-core` is the foundational Rust workspace for Casseted, an open-source shader and GPU-processing library focused on physically and mathematically grounded analog and VHS-style image transformation.

The repository currently provides a compact still-image v1 foundation:

- a Cargo workspace with small, focused crates
- a shared shader directory for WGSL sources
- architecture, formulas, testing, and stage-log docs for the current signal-model-aligned path
- committed reference-image fixtures for the still-image pipeline
- a small GPU-independent domain model for frame metadata, current still-preview controls, and the formal VHS / analog v1 parameter model
- a still-image GPU pipeline that implements a model-aligned subset of signal-model v1 through five logical stages on a compact four-pass runtime
- a CLI utility for running one PNG image through that pipeline

At this stage the project does not implement video support, a multi-pass render graph, web targets, SDK layers, or API infrastructure.

## Workspace crates

- `casseted-types`: shared frame/image metadata and pixel format types
- `casseted-signal`: current prototype effect controls plus the formal VHS / analog v1 signal model
- `casseted-shaderlib`: built-in WGSL shader source registry
- `casseted-gpu`: thin headless `wgpu` runtime setup and shader-module helpers
- `casseted-pipeline`: current still-image processing pipeline plus the compiled runtime reuse layer for the fixed pass chain
- `casseted-cli`: local PNG-to-PNG CLI utility for developer-facing pipeline checks
- `casseted-testing`: shared helpers for test images, stage reference fixtures, image diffs, and basic assertions

The main layer boundary is intentionally simple:

- `casseted-shaderlib` owns repository shader assets
- `casseted-gpu` owns low-level runtime setup
- `casseted-pipeline` is the layer that bridges the two for real processing and currently resolves the still path into five logical implementation stages across a compact four-pass runtime

## Repository layout

```text
.
|-- assets/
|   `-- reference-images/
|       `-- still-pipeline-v1/
|-- crates/
|   |-- casseted-cli/
|   |-- casseted-gpu/
|   |-- casseted-pipeline/
|   |-- casseted-shaderlib/
|   |-- casseted-signal/
|   |-- casseted-testing/
|   `-- casseted-types/
|-- docs/
|   |-- agent-log/
|   |-- architecture/
|   |-- decisions/
|   |-- math/
|   |-- reviews/
|   `-- testing.md
|-- examples/
`-- shaders/
```

## Current status

The workspace now acts as a compact still-image v1 signal-model implementation: it contains one real still-image GPU path, one working CLI, committed stage-oriented reference fixtures, and documentation that ties the implementation to the formal v1 model.

The current still-image path is deliberately compact:

- five logical implementation stages
- four WGSL render passes with three intermediate textures
- one compact compiled runtime layer for reusing prepared GPU execution objects across repeated still-image runs
- refined dedicated luma and chroma branch passes inside that fixed runtime
- one final reconstruction/output pass that now keeps dropout-conditioned reconstruction, contamination, and decode explicit inside the same pass boundary
- no render graph or plugin-style orchestration
- restrained highlight bleed, chroma contamination, and refined final-stage dropout/contamination integrated into the existing branch/output stages instead of separate effect passes

Current visual priority remains intentionally signal-first rather than glitch-first:

- tone shoulder and luma softness before aggressive transport wobble
- chroma bandwidth loss and coarse chroma reconstruction before decorative color splitting
- restrained final-stage contamination and dropout before heavy distortion

The current implementation path is anchored by:

- [`docs/architecture/overview.md`](./docs/architecture/overview.md)
- [`docs/architecture/pipeline.md`](./docs/architecture/pipeline.md)
- [`docs/architecture/signal-model-v1.md`](./docs/architecture/signal-model-v1.md)
- [`docs/math/signal-model-v1-formulas.md`](./docs/math/signal-model-v1-formulas.md)

Agent stage log:

Full chronological notes live in [`docs/agent-log/`](./docs/agent-log/).

Recent refinement steps:

- [`docs/agent-log/0007-compiled-still-runtime-layer.md`](./docs/agent-log/0007-compiled-still-runtime-layer.md)
- [`docs/agent-log/0008-luma-path-refinement.md`](./docs/agent-log/0008-luma-path-refinement.md)
- [`docs/agent-log/0009-chroma-path-refinement-v2.md`](./docs/agent-log/0009-chroma-path-refinement-v2.md)
- [`docs/agent-log/0010-final-reconstruction-stage-cleanup.md`](./docs/agent-log/0010-final-reconstruction-stage-cleanup.md)

## Build and setup

Canonical workspace toolchain:

- Rust `1.88.0`
- `clippy` and `rustfmt` installed through `rust-toolchain.toml`
- `Cargo.lock` committed and expected for workspace commands

Recommended local verification:

```bash
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

If you use `just`, the equivalent commands are available through `just check`, `just test`, `just clippy`, and `just ci`.

## CLI

The current CLI is a local developer utility for running the current still-image pipeline on one PNG image.

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
- the pipeline keeps projected preview base state and explicit preview overrides separate, so model-backed overrides only affect the terms you actually touch
- repeated still-image processing can now reuse a compiled `StillPipelineRuntime` instead of recreating render pipelines, bind-group layouts, and the sampler on every run
- the current limited multi-pass calibration emphasizes tone shoulder, luma softness, restrained highlight bleed, chroma bandwidth loss, brightness-shaped luma contamination, softer chroma contamination, and mild dropout ahead of jitter-heavy distortion
- aggressive manual overrides are softened into effective preview ranges before the WGSL passes run
- when that happens, the CLI prints a `preview-guardrails` line and reports the effective applied values

## Testing

Current testing is intentionally lightweight:

- unit tests for domain and support crates
- GPU smoke tests for the still-image pipeline
- a CLI smoke test that reads a PNG, runs the pipeline, and writes a PNG
- committed stage-oriented PNG fixtures for the current limited multi-pass still path in `assets/reference-images/still-pipeline-v1/`
- stage regression coverage for resolved defaults, bounded perturbations, and reference PNG comparisons when a GPU adapter is available
- shared helpers in [`docs/testing.md`](./docs/testing.md)

seventwogth - 2026

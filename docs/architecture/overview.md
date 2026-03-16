# Overview

The current `casseted-core` foundation is intentionally small and split into four layers:

1. Domain layer: `casseted-types` and `casseted-signal` describe frame metadata, prototype shader controls, and the formal VHS / analog v1 signal model.
2. Asset/runtime layer: `casseted-shaderlib` embeds WGSL shaders from the repository, while `casseted-gpu` owns headless `wgpu` setup.
3. Composition layer: `casseted-pipeline` now runs the first still-image render pass and returns processed pixels.
4. Developer tooling layer: `casseted-cli` and `casseted-testing` support local verification and smoke-level checks.

Current data flow:

- CLI code reads a PNG image into an `ImageFrame`
- pipeline code chooses a built-in shader identifier and builds a small uniform block from `SignalSettings`
- shaderlib resolves it to WGSL source embedded from `shaders/passes/`
- gpu code compiles raw WGSL provided by the pipeline and executes a fullscreen pass
- the processed texture is copied back to CPU memory as an `ImageFrame`
- CLI code writes the processed image back to disk as PNG

This keeps shader loading and GPU runtime concerns concrete, while leaving more advanced image processing, caching, and multi-pass orchestration for later stages.

For the next phase, the signal model itself is specified separately in [`signal-model-v1.md`](./signal-model-v1.md) so future implementation work can grow from an explicit domain contract instead of ad-hoc shader parameters.

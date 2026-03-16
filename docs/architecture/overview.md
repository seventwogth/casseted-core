# Overview

The current `casseted-core` workspace is intentionally split into four layers:

1. Domain layer: `casseted-types` and `casseted-signal`
2. Asset/runtime layer: `casseted-shaderlib` and `casseted-gpu`
3. Composition layer: `casseted-pipeline`
4. Developer tooling layer: `casseted-cli` and `casseted-testing`

Current data flow:

- CLI code reads a PNG into an `ImageFrame`
- pipeline code either accepts manual `SignalSettings` or projects a formal `VhsModel` into the current still-preview controls
- pipeline code resolves those controls into a compact WGSL uniform block
- `casseted-shaderlib` resolves the embedded WGSL source
- `casseted-gpu` compiles and executes the fullscreen pass
- the processed image is copied back to CPU memory as an `ImageFrame`
- CLI code writes the result as PNG

The key point in the current phase is that the single-pass implementation is now anchored to a formal signal contract instead of being just an ad-hoc shader parameter bundle.

Reference documents:

- [`signal-model-v1.md`](./signal-model-v1.md)
- [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md)

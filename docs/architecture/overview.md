# Overview

The current `casseted-core` workspace is intentionally split into four layers:

1. Domain layer: `casseted-types` and `casseted-signal`
2. Asset/runtime layer: `casseted-shaderlib` and `casseted-gpu`
3. Composition layer: `casseted-pipeline`
4. Developer tooling layer: `casseted-cli` and `casseted-testing`

Current data flow:

- CLI code reads a PNG into an `ImageFrame`
- pipeline code either accepts manual `SignalSettings` or projects a formal `VhsModel` into the current still-preview controls
- `casseted-pipeline` resolves those controls into five logical implementation stages:
  `input conditioning / tone shaping`, `luma/chroma transform`, `luma degradation`,
  `chroma degradation`, and `reconstruction / output`
- those stage-aligned controls are packed into one compact WGSL uniform block for the current fused still pass
- `casseted-shaderlib` resolves the embedded WGSL source
- `casseted-gpu` compiles and executes the single fullscreen pass that contains all five logical stages
- the processed image is copied back to CPU memory as an `ImageFrame`
- CLI code writes the result as PNG

The key point in the current phase is that the still-image path is now explicit at two levels:

- the canonical signal model in `casseted-signal` still defines the eight formal v1 stages
- the working GPU path groups them into five implementation stages while remaining one render pass

This is the current minimal decomposition: it makes model-to-implementation mapping readable without adding intermediate textures, pass scheduling, or a render graph.

Reference documents:

- [`signal-model-v1.md`](./signal-model-v1.md)
- [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md)

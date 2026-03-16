# Overview

The current `casseted-core` foundation is intentionally small and split into three layers:

1. Domain layer: `casseted-types` and `casseted-signal` describe frame metadata and analog-style effect parameters.
2. Asset/runtime layer: `casseted-shaderlib` embeds WGSL shaders from the repository, while `casseted-gpu` owns headless `wgpu` setup.
3. Composition layer: `casseted-pipeline` now runs the first still-image render pass and returns processed pixels.

Current data flow:

- pipeline code chooses a built-in shader identifier and builds a small uniform block from `SignalSettings`
- shaderlib resolves it to WGSL source embedded from `shaders/passes/`
- gpu code creates a `wgpu::ShaderModule` from that WGSL and executes a fullscreen pass
- the processed texture is copied back to CPU memory as an `ImageFrame`

This keeps shader loading and GPU runtime concerns concrete, while leaving more advanced image processing and multi-pass orchestration for later stages.

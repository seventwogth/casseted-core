# Overview

The current `casseted-core` foundation is intentionally small and split into three layers:

1. Domain layer: `casseted-types` and `casseted-signal` describe frame metadata and analog-style effect parameters.
2. Asset/runtime layer: `casseted-shaderlib` embeds WGSL shaders from the repository, while `casseted-gpu` owns headless `wgpu` setup.
3. Composition layer: `casseted-pipeline` selects the domain settings and shader asset that a future render or compute pass will use.

Current data flow:

- pipeline code chooses a built-in shader identifier
- shaderlib resolves it to WGSL source embedded from `shaders/passes/`
- gpu code creates a `wgpu::ShaderModule` from that WGSL
- later stages will attach bind groups, textures, and render/compute pipeline state

This keeps shader loading and GPU runtime concerns concrete, while leaving image processing and multi-pass orchestration for later stages.

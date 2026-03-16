# Still-Image Pipeline

The current real pipeline in `casseted-core` is still intentionally small:

- input: one `ImageFrame` in `RGBA8`
- execution: one fullscreen `wgpu` render pass
- shader: `shaders/passes/still_analog.wgsl`
- output: one processed `ImageFrame` read back to CPU memory

What changed in the current phase is not the runtime shape, but the signal-model alignment inside that pass.

## Current signal subset in the pass

The single pass now implements a narrow but explicit subset of signal-model v1:

- input interpretation under `sRGB` / BT.601-like assumptions
- tone shaping with soft highlight compression
- `RGB -> YUV` working decomposition
- luma-oriented horizontal softening
- chroma delay / blur / saturation degradation
- reconstruction back to RGB

Secondary prototype terms still integrated into the same pass:

- line-based horizontal instability
- additive luma/chroma noise

## Projection layer

The pipeline now owns a very small projection bridge from the formal domain model into the single-pass preview path:

- `StillImagePipeline::from_vhs_model()`
- `prototype_signal_from_model()`
- `effect_uniforms()`

This is intentionally narrow. It does not introduce:

- a pass graph
- a plugin system
- a generalized planning runtime
- multi-texture luma/chroma orchestration

## Why it stays single-pass for now

Keeping the current stage in one pass is still the right tradeoff because it preserves:

- the existing clean crate boundaries
- the thin `casseted-gpu` runtime
- the shader asset bridge in `casseted-pipeline`
- a compact implementation path for the first algorithmic phase

The pass is now better viewed as a reference-consistent fused subset of v1, not as an accidental prototype.

## Deferred on purpose

The following are still deferred:

- explicit multi-pass luma/chroma textures
- dropout masking
- head-switching artifacts
- chroma phase error
- video and temporal state
- pipeline caching and resource reuse work

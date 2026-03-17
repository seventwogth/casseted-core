# Still-Image Pipeline

The current real pipeline in `casseted-core` is still intentionally small:

- input: one `ImageFrame` in `RGBA8`
- execution: one fullscreen `wgpu` render pass
- shader: `shaders/passes/still_analog.wgsl`
- output: one processed `ImageFrame` read back to CPU memory

What changed in the current phase is the internal structure of that pass: the code now names a small set of logical implementation stages instead of presenting the shader as one undifferentiated effect bundle.

## Current implementation stages

The still-image path currently uses five logical stages:

1. input conditioning / tone shaping
2. luma/chroma transform
3. luma degradation
4. chroma degradation
5. reconstruction / output

All five stages are still executed inside one WGSL pass today.

## Formal-to-implementation mapping

| Implementation stage | Formal v1 stage coverage | Current code location | Current pass boundary |
| --- | --- | --- | --- |
| Input conditioning / tone shaping | `InputDecode`, `ToneShaping`, and the currently spatial part of `TransportInstability` | `resolve_input_conditioning_stage()` in `casseted-pipeline`, `apply_input_conditioning()` and `apply_tone_shaping()` in WGSL | fused into `still_analog.wgsl` |
| Luma/chroma transform | `RgbToLumaChroma` | `sample_working_signal()` in WGSL | fused into `still_analog.wgsl` |
| Luma degradation | `LumaRecordPath` | `resolve_luma_degradation_stage()` and `degrade_luma()` | fused into `still_analog.wgsl` |
| Chroma degradation | `ChromaRecordPath` | `resolve_chroma_degradation_stage()` and `degrade_chroma()` | fused into `still_analog.wgsl` |
| Reconstruction / output | `NoiseAndDropouts` (noise-only subset) plus `DecodeOutput` | `resolve_reconstruction_output_stage()`, `sample_output_noise()`, `reconstruct_output()` | fused into `still_analog.wgsl` |

Important detail:
the formal transport stage still exists canonically in `casseted-signal`, but the current still path only implements its spatial still-frame subset, so it is grouped into input conditioning rather than split into its own pass.

## Projection layer

The pipeline owns a narrow projection bridge from the formal domain model into the current fused pass:

- `StillImagePipeline::from_vhs_model()`
- `project_vhs_model_to_preview_signal()`
- `effective_preview_signal()`
- `resolve_still_stages()`
- `EffectUniforms`

This is intentionally narrow. It does not introduce:

- a pass graph
- a plugin system
- a generalized planning runtime
- multi-texture luma/chroma orchestration

## Why it stays single-pass for now

Keeping the current path in one pass is still the right tradeoff because the implementation-stage split is now clear enough for further work, while preserving:

- the existing clean crate boundaries
- the thin `casseted-gpu` runtime
- the shader asset bridge in `casseted-pipeline`
- a compact implementation path for the first algorithmic phase

Splitting into multiple passes would add temporary textures and more orchestration, but it would not yet buy enough clarity to justify the weight.

## Deferred on purpose

The following are still deferred:

- explicit multi-pass luma/chroma textures
- dropout masking
- head-switching artifacts
- chroma phase error
- video and temporal state
- pipeline caching and resource reuse work

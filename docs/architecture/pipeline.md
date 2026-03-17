# Still-Image Pipeline

The current real still-image pipeline in `casseted-core` is intentionally small:

- input: one `ImageFrame` in `RGBA8`
- execution: four fullscreen `wgpu` render passes
- shaders:
  `shaders/passes/still_input_conditioning.wgsl`,
  `shaders/passes/still_luma_degradation.wgsl`,
  `shaders/passes/still_chroma_degradation.wgsl`,
  `shaders/passes/still_reconstruction_output.wgsl`
- intermediate textures:
  working YUV,
  degraded luma,
  degraded chroma
- output: one processed `ImageFrame` read back to CPU memory

## Current implementation stages

The still-image path keeps five logical implementation stages:

1. input conditioning / tone shaping
2. luma/chroma transform
3. luma degradation
4. chroma degradation
5. reconstruction / output

Those five stages are now executed as a limited four-pass runtime.

## Physical pass layout

| Physical pass | Primary output | Logical implementation stages covered | Formal v1 stage coverage |
| --- | --- | --- | --- |
| `still_input_conditioning` | working YUV texture | input conditioning / tone shaping + luma/chroma transform | `InputDecode`, `ToneShaping`, `RgbToLumaChroma`, and the current still-frame spatial subset of `TransportInstability` |
| `still_luma_degradation` | degraded luma texture | luma degradation with restrained highlight bleed | `LumaRecordPath` |
| `still_chroma_degradation` | degraded chroma texture | chroma degradation via low-pass, coarse chroma reconstruction, restrained smear, and optional vertical line blend | `ChromaRecordPath` |
| `still_reconstruction_output` | final `RGBA8` output | reconstruction / output with additive noise and restrained line-segment dropout concealment | `NoiseAndDropouts` (noise + still-image dropout subset) and `DecodeOutput` |

Important detail:
the formal transport stage still exists canonically in `casseted-signal`, but the current still path only implements its spatial still-frame subset, so it remains fused into the first pass instead of becoming a standalone transport pass.

## Projection layer

The pipeline still owns a narrow projection bridge from the formal domain model into the current runtime:

- `StillImagePipeline::from_vhs_model()`
- `project_vhs_model_to_preview_signal()`
- `effective_preview_signal()`
- `resolve_still_stages()`
- `EffectUniforms`

That bridge remains intentionally narrow. It does not introduce:

- a render graph
- a plugin system
- a generalized planning runtime
- pass scheduling outside the fixed still-image sequence

## Why this is the chosen decomposition

Four passes are the minimal useful split for the current stage because they:

- create one explicit working-signal fan-out point after tone shaping
- give luma and chroma independent branch passes without inventing a graph
- keep noise and decode coupled, which avoids over-splitting the still path too early
- keep highlight bleed inside the luma branch and dropout inside the final reconstruction pass, so the architecture stays compact while the formal signal chain gets less "too clean"

This is enough to support further still-image algorithm growth inside the current architecture while keeping orchestration compact.

The current chroma refinement stays inside that boundary: it deepens the chroma branch behavior without adding passes, new runtime abstractions, or a wider public control surface.

## Deferred on purpose

The following are still deferred:

- render-graph planning
- dedicated dropout-only masking passes
- head-switching artifacts
- chroma phase error
- video and temporal state
- aggressive pipeline caching and resource reuse work

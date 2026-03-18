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
| `still_reconstruction_output` | final `RGBA8` output | reconstruction / output with brightness-shaped luma contamination, softer chroma contamination, and restrained line-segment dropout concealment | `NoiseAndDropouts` (refined noise contamination + still-image dropout subset) and `DecodeOutput` |

Important detail:
the formal transport stage still exists canonically in `casseted-signal`, but the current still path only implements its spatial still-frame subset, so it remains fused into the first pass instead of becoming a standalone transport pass.

## Projection layer

The pipeline still owns a narrow projection bridge from the formal domain model into the current runtime:

- `StillImagePipeline::from_vhs_model()`
- `StillImagePipeline::preview_base_signal()`
- `StillImagePipeline::preview_overrides()`
- `StillImagePipeline::set_model()`
- `StillImagePipeline::set_preview_overrides()`
- `StillImagePipeline::clear_preview_overrides()`
- `project_vhs_model_to_preview_signal()`
- `preview_signal()`
- `effective_preview_signal()`
- `resolve_still_stages()`
- `EffectUniforms`

Stabilization note:

- the shared frame block now carries the frame/procedural seed used by both input conditioning and reconstruction-side noise/dropout helpers, so `effect.reconstruction_output` stays focused on output-stage terms
- `StillImagePipeline` now keeps `model`, projected `preview_base_signal`, and explicit `SignalOverrides` as separate internal responsibilities
- model-backed preview overrides are merged per explicit override instead of inferring user intent from float equality or re-normalizing untouched projected terms

That bridge remains intentionally narrow. It does not introduce:

- a render graph
- a plugin system
- a generalized planning runtime
- pass scheduling outside the fixed still-image sequence

Current internal code split inside `casseted-pipeline`:

- `state.rs`: public pipeline API plus model/base/override state ownership
- `projection.rs`: formal-model projection and preview guardrails
- `stages.rs`: logical-stage resolution and uniform packing
- `runtime.rs`: `wgpu` execution, bind groups, and readback

## Why this is the chosen decomposition

Four passes are the minimal useful split for the current stage because they:

- create one explicit working-signal fan-out point after tone shaping
- give luma and chroma independent branch passes without inventing a graph
- keep refined noise contamination and decode coupled, which avoids over-splitting the still path too early
- keep highlight bleed inside the luma branch and dropout inside the final reconstruction pass, so the architecture stays compact while the formal signal chain gets less "too clean"

This is enough to support further still-image algorithm growth inside the current architecture while keeping orchestration compact.

The current chroma and noise refinements stay inside that boundary: they deepen branch/output behavior without adding passes, new runtime abstractions, or a wider public control surface.

## Deferred on purpose

The following are still deferred:

- render-graph planning
- dedicated dropout-only masking passes
- head-switching artifacts
- chroma phase error
- video and temporal state
- aggressive pipeline caching and resource reuse work

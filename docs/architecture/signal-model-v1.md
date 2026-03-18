# Signal Model v1

This document defines the first formal still-image signal model for `casseted-core`.

It is the architectural companion to the formula-level reference in [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md). The architecture document states what the model is and where it lives. The formulas document states the concrete discrete approximations used by the current implementation.

The purpose of v1 is not to emulate an entire VHS deck. The goal is to keep one canonical, signal-oriented chain stable enough that later WGSL work expands a known model instead of accumulating ad-hoc filters.

The corresponding domain types live in `casseted-signal` as:

- `VhsModel`
- `VhsSignalStage`
- `VhsToneSettings`
- `VhsInputSettings`
- `VhsLumaSettings`
- `VhsChromaSettings`
- `VhsTransportSettings`
- `VhsNoiseSettings`
- `VhsDecodeSettings`

## Boundaries

v1 models a single still-frame projection of consumer VHS-like playback starting from an already-decoded digital image.

Inside the model:

- input interpretation under explicit transfer and matrix assumptions
- tone shaping with soft highlight compression
- RGB to luma/chroma decomposition
- separate luma and chroma degradation paths
- still-frame transport instability that can be expressed spatially
- signal-shaped noise contamination and dropout-style corruption
- reconstruction back to display RGB

Explicitly outside the model:

- RF carrier or FM sideband simulation
- deck-accurate helical-scan geometry
- AGC and servo control loops
- video-sequence behavior as the primary implementation target
- multi-generation dubbing loss
- audio-path simulation

This boundary is deliberate: the model should stay signal-motivated without forcing a runtime rewrite.

## Canonical Signal Flow

`casseted-signal` exposes the canonical stage order as `VHS_SIGNAL_FLOW_V1`:

1. `InputDecode`
2. `ToneShaping`
3. `RgbToLumaChroma`
4. `LumaRecordPath`
5. `ChromaRecordPath`
6. `TransportInstability`
7. `NoiseAndDropouts`
8. `DecodeOutput`

Conceptually:

```text
R'G'B' input
  -> normalize input assumptions
  -> apply tone rolloff / soft highlight compression
  -> decompose into luma/chroma
  -> degrade luma bandwidth/detail
  -> degrade chroma bandwidth / delay / saturation
  -> apply line-wise spatial instability
  -> inject signal-shaped luma/chroma contamination and optional corruption
  -> reconstruct output RGB
```

The stage order is canonical even if a concrete GPU implementation groups or fuses several stages for a compact runtime.

## Current Implementation Grouping

The current still-image implementation keeps the formal eight-stage order above, but groups it into five smaller engineering stages:

1. input conditioning / tone shaping
2. luma/chroma transform
3. luma degradation
4. chroma degradation
5. reconstruction / output

Those stages now execute through a limited four-pass runtime:

| Physical pass | Implementation stages covered | Formal v1 stages included | Current WGSL pass |
| --- | --- | --- | --- |
| Input conditioning pass | input conditioning / tone shaping + luma/chroma transform | `InputDecode`, `ToneShaping`, `RgbToLumaChroma`, plus the currently spatial subset of `TransportInstability` | `still_input_conditioning.wgsl` |
| Luma pass | luma degradation | `LumaRecordPath` | `still_luma_degradation.wgsl` |
| Chroma pass | chroma degradation | `ChromaRecordPath` | `still_chroma_degradation.wgsl` |
| Reconstruction pass | reconstruction / output | `NoiseAndDropouts` (brightness-shaped luma contamination, softer chroma contamination, and the restrained still-dropout subset) and `DecodeOutput` | `still_reconstruction_output.wgsl` |

Why this grouping is used now:

- it creates one explicit working-signal fan-out point after tone shaping
- it gives luma and chroma separate physical branches without introducing a render graph
- it keeps signal-shaped noise and decode fused so the orchestration stays compact for still-image work

## Visual Regression Mapping

The current visual regression foundation keeps one committed source image plus one committed output PNG per implementation stage in `assets/reference-images/still-pipeline-v1/`.

| Implementation stage | Formulas reference | Uniform focus | WGSL entry points | Reference PNG |
| --- | --- | --- | --- | --- |
| Input conditioning / tone shaping | `4.1` plus transport note in `5.1` | `effect.input_conditioning` | `conditioned_sample_uv()`, `apply_tone_shaping()` | `input-conditioning-tone.png` |
| Luma/chroma transform | `4.2` | no stage-specific uniform group; verified as the neutral transform case for the working-signal fan-out path | `rgb_to_yuv()` in `still_input_conditioning.wgsl` | `luma-chroma-transform.png` |
| Luma degradation | `4.3` | `effect.luma_degradation` | `degrade_luma()`, `highlight_bleed()` | `luma-degradation.png` |
| Chroma degradation | `4.4` | `effect.chroma_degradation` | `degrade_chroma()` | `chroma-degradation.png` |
| Reconstruction / output | `4.5` plus notes in `5.2` and `5.3` | `effect.reconstruction_output`, `effect.reconstruction_aux` | `sample_output_noise()`, `apply_dropout()`, `reconstruct_output()` | `reconstruction-output.png` |

Current fixture policy:

- reference comparisons use fixed tolerances for the compact multi-pass outputs
- stage tests also verify resolved defaults and bounded output changes under small parameter perturbations
- fixtures remain stage-oriented end-to-end outputs; they do not introduce a separate intermediate-texture review tool at this phase

## Stage Intent

### 1. InputDecode

Purpose:
define the input transfer and matrix assumptions explicitly.

Current v1 assumption:
gamma-coded `sRGB` input interpreted with a BT.601-like luma/chroma matrix.

### 2. ToneShaping

Purpose:
introduce tone rolloff before spatial degradation so highlight compression is part of the signal path rather than a post-look flourish.

Current v1 shape:
soft-knee highlight compression on luma, applied by rescaling RGB to preserve chromaticity.

### 3. RgbToLumaChroma

Purpose:
split the signal into a luma branch and a chroma branch so each can degrade differently.

Current v1 shape:
BT.601-like `YUV` working representation.

### 4. LumaRecordPath

Purpose:
reduce horizontal detail and microcontrast while keeping image structure intact.

Current v1 shape:
compact horizontal low-pass plus a very small pre/de-emphasis-inspired residual term, extended by a highlight-gated asymmetric bleed approximation that only activates around bright luma.

### 5. ChromaRecordPath

Purpose:
make chroma lower-fidelity and less well-registered than luma.

Current v1 shape:
chroma delay, horizontal low-pass, coarse horizontal chroma reconstruction, restrained smear / bleed, chroma saturation scaling, and optional vertical line blend.

### 6. TransportInstability

Purpose:
project line-wise time-base instability into a still frame.

Current v1 shape:
deterministic horizontal line jitter and small vertical offset.

### 7. NoiseAndDropouts

Purpose:
remove the "pure digital filter" feel by injecting stochastic corruption.

Current v1 shape:
brightness-shaped luma contamination, softer lower-bandwidth chroma contamination, and a restrained line-oriented dropout approximation driven by the formal dropout parameters and resolved through adjacent-line concealment instead of temporal logic.

### 8. DecodeOutput

Purpose:
reconstruct a display-space RGB image from the degraded working signal.

Current v1 shape:
`YUV -> RGB` reconstruction with a small Y/C leakage term.

## Domain Ownership

### What belongs in `casseted-signal`

`casseted-signal` owns the signal contract:

- `VhsModel`
- `VhsSignalStage` and `VHS_SIGNAL_FLOW_V1`
- grouped parameter families for tone, input, luma, chroma, transport, noise, and decode
- compact still-image controls in `SignalSettings` for the current preview path

This is domain structure, not GPU structure.

### What belongs in `casseted-types`

`casseted-types` still owns only shared frame/image types:

- `FrameSize`
- `PixelFormat`
- `FrameDescriptor`
- `ImageFrame`

No signal-specific types need to move there for v1.

### What stays out of the public signal API for now

The following remain implementation details:

- exact filter taps and weights
- uniform packing layout
- random hash details
- temporary texture allocation
- pass fusion or pass splitting
- pipeline caching and resource reuse

## Parameter Groups In Code

The current formal parameter groups are:

- `VhsInputSettings`
- `VhsToneSettings`
- `VhsLumaSettings`
- `VhsChromaSettings`
- `VhsTransportSettings`
- `VhsNoiseSettings`
- `VhsDecodeSettings`

The compact still-preview layer in `SignalSettings` remains intentionally smaller:

- `ToneSettings`
- `LumaSettings`
- `ChromaSettings`
- `NoiseSettings`
- `TrackingSettings`

That preview layer is not a competing domain model. It is a narrow control surface for the current still-image implementation.

## Mapping To The Current Pipeline

The current still-image pipeline now has an explicit narrow projection from `VhsModel` into the limited multi-pass still implementation:

- `StillImagePipeline::from_vhs_model()` creates the current still-preview configuration from a formal `VhsModel`
- `StillImagePipeline::preview_base_signal()` exposes the projected preview/runtime subset without exposing it as a mutable second source of truth
- `StillImagePipeline::preview_overrides()` exposes the explicit preview override layer
- `StillImagePipeline::set_model()` reprojects the preview base when the formal model changes
- `StillImagePipeline::set_preview_overrides()` and `clear_preview_overrides()` manage preview-only user intent explicitly
- `project_vhs_model_to_preview_signal()` converts the formal model into compact preview controls
- `resolve_still_stages()` groups those controls into the five implementation stages
- `EffectUniforms` packs those stage controls into the shared WGSL uniform block used by the four still passes
- the runtime writes three intermediate textures:
  working YUV,
  degraded luma,
  degraded chroma

There are two intentional modes:

- `StillImagePipeline::from_vhs_model()` keeps the current model-aligned subset active
- `StillImagePipeline::new(signal)` is a narrower manual preview path and keeps the model-only decode/projection/dropout terms neutral

Preview-specific guardrail rule:

- manual preview controls are resolved through `effective_preview_signal()` before `resolve_still_stages()` packs uniforms
- those guardrails only affect preview-facing `SignalSettings` terms
- the formal `VhsModel` and its projection rules remain unchanged
- if a model-projected pipeline later receives manual signal overrides, only explicit override terms are normalized, while untouched projected terms stay at the model-projected values
- coupled chroma override terms are still normalized together so offset-heavy overrides cannot collapse back into a digital color-split look

Important constraint:
this is a projection layer, not a graph engine and not a new runtime abstraction.

Current stage-aligned mapping:

- input conditioning / tone shaping:
  `VhsToneSettings` -> `SignalSettings.tone` -> `effect.input_conditioning.xy`
- luma degradation:
  `VhsLumaSettings.bandwidth_mhz` -> stronger preview luma blur proxy -> `effect.luma_degradation.x`
  `VhsLumaSettings.preemphasis_db` -> restrained detail residual gain -> `effect.luma_degradation.y`
  existing tone + luma terms -> derived highlight-bleed threshold / amount -> `effect.luma_degradation.zw`
- chroma degradation:
  `VhsChromaSettings.delay_us` -> signed attenuated preview chroma offset proxy -> `effect.chroma_degradation.x`
  `VhsChromaSettings.bandwidth_khz` -> stronger preview chroma bandwidth-loss proxy -> `effect.chroma_degradation.y`
  `VhsChromaSettings.saturation_gain` -> `effect.chroma_degradation.z`
  `VhsDecodeSettings.chroma_vertical_blend` -> `effect.chroma_degradation.w`
- reconstruction / output:
  `VhsDecodeSettings.luma_chroma_crosstalk` -> `effect.reconstruction_output.z`
  `VhsNoiseSettings.{dropout_probability_per_line,dropout_mean_span_us}` -> restrained dropout probability / span terms -> `effect.reconstruction_aux.xy`

The chroma pass keeps that uniform contract compact on purpose: the shader derives low-pass span, effective chroma cell width, and restrained smear from the same bandwidth-loss proxy instead of expanding the public preview API.

Secondary mappings that are still present but not the main focus of this phase:

- `VhsTransportSettings.line_jitter_us` -> attenuated input-conditioning jitter proxy -> `effect.input_conditioning.z`
- `VhsTransportSettings.vertical_wander_lines` -> still-frame vertical offset snapshot -> `effect.input_conditioning.w`
- `FrameDescriptor.frame_index` -> shared frame/procedural seed -> `effect.frame.w`, reused by input conditioning and reconstruction-side noise/dropout indexing without making reconstruction the owner of transport semantics
- `VhsNoiseSettings.{luma_sigma,chroma_sigma}` -> restrained reconstruction contamination amplitudes that the final pass reshapes into brightness-dependent luma noise and softer band-correlated chroma contamination -> `effect.reconstruction_output.xy`

Current preview guardrails for manual / override-driven `SignalSettings`:

- `tracking.line_jitter_px` uses a soft cap so strong values remain expressive but asymptotically stay below the current glitch-prone range
- `chroma.offset_px` uses a signed soft cap and `chroma.bleed_px` gains a minimum bandwidth-loss floor tied to the effective offset
- `noise.{luma_amount,chroma_amount}` use soft caps so noise does not jump ahead of tone and bandwidth loss
- `tracking.vertical_offset_lines` also uses a signed soft cap so still-image transport wobble stays secondary
- in model-backed pipelines, those guardrails now preserve untouched projected preview terms instead of re-normalizing the entire preview signal blob
- these rules are intentionally preview-only and do not redefine the formal model

## Current Visual Calibration Priorities

The current limited multi-pass still-image implementation is intentionally not balanced equally across all formal stages. For the current phase, the visual priority is:

- tone rolloff and soft highlight compression
- luma softness and microcontrast loss
- restrained highlight bleed that reads like scan-direction signal smear, not bloom
- chroma bandwidth loss, coarse horizontal chroma resolution loss, and restrained bleed
- only mild chroma misregistration
- only mild transport wobble, noise contamination, and dropout

Why this changed:

- earlier weights let line jitter and chroma delay read as decorative RGB-split wobble
- that pushed the result toward glitch-like distortion art instead of signal degradation
- the current calibration therefore strengthens bandwidth-loss proxies and attenuates transport / delay proxies

Scene-level calibration notes for the current limited multi-pass path:

- text and hard verticals should soften and halo slightly before they wobble
- neutral surfaces should show chroma softness before obvious hue splitting
- bright highlights should roll into a shoulder instead of clipping to flat white
- bright highlight edges should spread a little before they read as glow
- dark scenes should keep noise and dropout subordinate to tone and bandwidth loss
- neutral surfaces should pick up faint line/band contamination before they read as a uniform grain overlay
- skin and portrait areas should look softer and dirtier, not decoratively torn apart

## Implementation Status

The current repository now implements a reference-consistent subset of v1 as five logical stages executed through four WGSL passes:

- input conditioning / tone shaping plus `RGB -> YUV` fan-out into a working-signal texture
- luma low-pass/detail attenuation biased toward microcontrast loss, with restrained highlight bleed embedded in the same branch
- chroma delay plus low-pass/coarse-reconstruction/smear degradation biased toward bandwidth loss over misregistration
- reconstruction back to RGB with brightness-shaped luma contamination, softer chroma contamination, restrained line-segment dropout handling, and restrained Y/C leakage
- line jitter and vertical offset kept as integrated but restrained input-conditioning terms
- the final pass reuses the transport-conditioned line phase only as a procedural seed for noise/dropout placement; it does not reapply transport resampling to luma/chroma textures

Still deferred:

- chroma phase error
- head switching behavior
- temporal model
- render-graph planning
- video support
- richer authoring workflows for override presets and inspection tooling; the current explicit override API is intentionally minimal and still-image-focused

## Consequence

The next step is to extend this signal-model-aligned subset deliberately, not to replace the current architecture.

The likely next implementation moves are:

- refine luma and chroma branch behavior inside the current pass structure
- refine line-level transport/dropout interplay only if the current fused output stage stops being sufficient
- improve resource reuse and calibration workflow without changing the domain contract

All of that should keep the same domain contract anchored in `casseted-signal` and the same formula reference anchored in [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md).

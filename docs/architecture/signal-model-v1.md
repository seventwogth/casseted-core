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
- additive noise and dropout-style corruption
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
  -> inject noise and optional corruption
  -> reconstruct output RGB
```

The stage order is canonical even if a concrete GPU implementation fuses several stages into one pass.

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
compact horizontal low-pass plus a very small pre/de-emphasis-inspired residual term.

### 5. ChromaRecordPath

Purpose:
make chroma lower-fidelity and less well-registered than luma.

Current v1 shape:
chroma delay, chroma blur, chroma saturation scaling, and optional vertical chroma blend.

### 6. TransportInstability

Purpose:
project line-wise time-base instability into a still frame.

Current v1 shape:
deterministic horizontal line jitter and small vertical offset.

### 7. NoiseAndDropouts

Purpose:
remove the "pure digital filter" feel by injecting stochastic corruption.

Current v1 shape:
additive luma/chroma noise. Dropout parameters exist in the formal model but are not yet implemented in the shader.

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

That preview layer is not a competing domain model. It is a narrow control surface for the current single-pass implementation.

## Mapping To The Current Pipeline

The current still-image pipeline now has an explicit narrow projection from `VhsModel` into the single-pass preview path:

- `StillImagePipeline::from_vhs_model()` creates the current still-preview configuration from a formal `VhsModel`
- `prototype_signal_from_model()` converts the formal model into compact preview controls
- `effect_uniforms()` resolves those controls into the WGSL uniform block used by `shaders/passes/still_analog.wgsl`

There are two intentional modes:

- `StillImagePipeline::from_vhs_model()` keeps the current model-aligned subset active
- `StillImagePipeline::new(signal)` is a narrower manual preview path and keeps the model-only decode/projection terms neutral

Important constraint:
this is a projection layer, not a graph engine and not a new runtime abstraction.

Current implemented mapping:

- `VhsToneSettings` -> `SignalSettings.tone` -> `effect.tone_luma.xy`
- `VhsLumaSettings.bandwidth_mhz` -> preview luma blur proxy -> `effect.tone_luma.z`
- `VhsLumaSettings.preemphasis_db` -> small detail residual gain -> `effect.tone_luma.w`
- `VhsChromaSettings.delay_us` -> preview chroma offset proxy -> `effect.chroma.x`
- `VhsChromaSettings.bandwidth_khz` -> preview chroma blur proxy -> `effect.chroma.y`
- `VhsChromaSettings.saturation_gain` -> `effect.chroma.z`
- `VhsDecodeSettings.chroma_vertical_blend` -> `effect.chroma.w`
- `VhsDecodeSettings.luma_chroma_crosstalk` -> `effect.noise_decode.z`

Secondary mappings that are still present but not the main focus of this phase:

- `VhsTransportSettings.line_jitter_us` -> preview line jitter proxy
- `VhsTransportSettings.vertical_wander_lines` -> still-frame vertical offset snapshot
- `VhsNoiseSettings.{luma_sigma,chroma_sigma}` -> preview noise amplitudes

## Implementation Status

The current repository now implements a reference-consistent subset of v1 in one WGSL pass:

- tone shaping
- `RGB -> YUV` decomposition
- luma low-pass/detail attenuation
- chroma delay/blur/saturation degradation
- reconstruction back to RGB
- line jitter and additive noise as secondary terms

Still deferred:

- chroma phase error
- dropouts
- head switching behavior
- temporal model
- multi-pass separation of luma/chroma textures
- video support

## Consequence

The next step is to extend this signal-model-aligned subset deliberately, not to replace the current architecture.

The likely next implementation moves are:

- separate or strengthen the chroma degradation path
- add a more explicit transport/dropout stage
- decide when single-pass fusion stops being clearer than limited multi-pass staging

All of that should keep the same domain contract anchored in `casseted-signal` and the same formula reference anchored in [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md).

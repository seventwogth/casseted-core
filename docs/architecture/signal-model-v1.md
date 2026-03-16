# Signal Model v1

This document defines the first formal still-image signal model for `casseted-core`.

The purpose of v1 is not to fully emulate a VHS deck or to rewrite the runtime immediately. The goal is to make one domain-meaningful model explicit enough that future WGSL passes, pipeline planning, and presets all grow from the same foundation.

Core goals:

- establish a canonical still-image signal-flow for v1
- separate physically/signally motivated stages from engineering approximations
- define the minimum parameter groups that belong in the domain model
- keep current crate boundaries intact while preparing the next algorithmic phase

The corresponding domain types live in `casseted-signal` as `VhsModel`, `VhsSignalStage`, and the `Vhs*Settings` groups.

## Boundaries

v1 models a single-generation consumer VHS-like playback look starting from an already decoded digital still image.

Inside the model:

- input normalization into a defined working interpretation
- RGB to luma/chroma decomposition
- luma and chroma degradation as separate signal paths
- line-wise transport instability that can be meaningfully projected onto a still frame
- additive noise and dropout-like corruption
- reconstruction back into display RGB

Explicitly outside the model:

- exact RF carrier simulation and FM sideband behavior
- physical helical-scan geometry as a deck emulator
- AGC, servo control loops, and deck calibration dynamics
- true video/time-sequence behavior beyond what a still-frame snapshot can justify
- multi-generation dubbing loss
- audio-path simulation

This boundary is intentional: v1 is meant to be signal-domain accurate enough to organize implementation work, while remaining small enough to integrate into the current workspace.

## Canonical Signal Flow

`casseted-signal` exposes the canonical stage order as `VHS_SIGNAL_FLOW_V1`:

1. `InputDecode`
2. `RgbToLumaChroma`
3. `LumaRecordPath`
4. `ChromaRecordPath`
5. `TransportInstability`
6. `NoiseAndDropouts`
7. `DecodeOutput`

Conceptually:

```text
R'G'B' input
  -> normalize transfer/matrix assumptions
  -> decompose into luma/chroma
  -> degrade luma bandwidth/detail
  -> degrade chroma bandwidth, delay, and phase
  -> apply line-wise spatial instability
  -> inject noise and dropout-like corruption
  -> reconstruct output RGB
```

The stage order is canonical for v1 even if future GPU implementations fuse or split stages differently.

## Stage Breakdown

### 1. Input Representation

Purpose:
bring the incoming image into a stable working interpretation before any analog-style degradation is applied.

Mathematical meaning:
the input is treated as gamma-coded RGB with an explicit transfer assumption and luma/chroma matrix selection.

Expected visual effect:
by itself none; this stage exists to prevent the rest of the model from depending on hidden color assumptions.

Physical plausibility:
medium. It is physically motivated in the sense that analog degradation happens on encoded signals, but the exact camera/decoder chain is deliberately collapsed into one normalized entry point.

First GPU approximation:
assume `sRGB` input and a `BT.601`-like matrix in uniforms or shared shader code, without modeling a full camera pipeline.

### 2. Luma / Chroma Decomposition

Purpose:
separate detail-carrying luminance-like content from color-difference content so they can degrade differently.

Mathematical meaning:
an operator `RGB -> {Y, C}` using a fixed matrix, with luma and chroma treated as separate branches after decomposition.

Expected visual effect:
enables sharper luma than chroma, color smearing, and luma/chroma mismatch typical of consumer analog playback.

Physical plausibility:
high. Different luma/chroma treatment is central to VHS-like behavior.

First GPU approximation:
convert sampled RGB into a YUV-like representation inside a pass, then process Y and C with different kernels or offsets.

### 3. Luma Bandwidth Shaping

Purpose:
reduce fine detail, especially horizontally, while preserving overall image structure.

Mathematical meaning:
apply a luma low-pass operator `H_y`, optionally paired with broad pre-emphasis/de-emphasis.

Expected visual effect:
softened fine texture, reduced crispness, slight analog smear instead of pure Gaussian blur.

Physical plausibility:
high for the bandwidth loss, medium for the exact kernel used in v1.

First GPU approximation:
use a compact separable horizontal blur or weighted tap filter whose width is driven by `VhsLumaSettings.bandwidth_mhz`.

### 4. Chroma Degradation

Purpose:
make color visibly less stable and lower-resolution than luma.

Mathematical meaning:
apply a lower-bandwidth chroma operator `H_c`, plus chroma delay `tau_c`, saturation scaling `g_c`, and phase perturbation `phi_c`.

Expected visual effect:
color bleed, chroma lag, soft color edges, slight hue instability, and imperfect registration against luma.

Physical plausibility:
high for reduced chroma bandwidth and delay; medium for how phase error is approximated in still-image v1.

First GPU approximation:
sample chroma from offset coordinates, blur it more aggressively than luma, and optionally perturb chroma channels before reconstruction.

### 5. Line-Wise Transport Instability

Purpose:
introduce spatial distortion that corresponds to tape transport / time-base instability without requiring video support.

Mathematical meaning:
apply a line-dependent horizontal displacement field `dx_line(y)` and a small slow vertical displacement term `dy`.

Expected visual effect:
horizontal wiggle, scanline-level skew, and subtle frame instability that reads as analog transport error even on still images.

Physical plausibility:
medium to high. The phenomenon is real; the still-image version is a single-frame spatial snapshot of a temporal process.

First GPU approximation:
derive a per-line horizontal offset from deterministic functions or hashed line indices, then warp lookup coordinates before final sampling.

### 6. Noise And Dropouts

Purpose:
add stochastic corruption so the image no longer feels like a purely filtered digital frame.

Mathematical meaning:
inject additive luma/chroma noise and optionally mask or attenuate short horizontal segments to mimic dropouts.

Expected visual effect:
fine grain, chroma roughness, low-level flicker impression, and occasional local signal collapse.

Physical plausibility:
high for the existence of noise and dropouts, medium for the chosen probability distributions in v1.

First GPU approximation:
use deterministic hash-based noise and simple horizontal dropout masks driven by line-local random values.

### 7. Reconstruction

Purpose:
map the degraded signal representation back into an output image that downstream code can consume as a normal frame.

Mathematical meaning:
decode `{Y, C}` back into RGB with optional vertical chroma blending and residual luma/chroma crosstalk.

Expected visual effect:
recombined output with softened color, controlled leakage, and a coherent final analog-looking frame.

Physical plausibility:
medium. Recombination is required, but v1 uses a pragmatic decode stage rather than a fully modeled consumer decoder.

First GPU approximation:
recombine YUV-like values into RGB in the final fragment path and clamp to the working output format.

## Mathematical Shape

At the operator level, v1 can be summarized as:

```text
Y_out(x, y) = H_y{Y_in(x + dx_line(y), y + dy)} + n_y(x, y)
C_out(x, y) = H_c{g_c * C_in(x + dx_line(y) - tau_c, y + dy)}
              with additional phase error phi_c and chroma noise n_c
RGB_out = D{Y_out, C_out, decode_settings}
```

Where:

- `H_y` is the luma-loss operator
- `H_c` is the chroma-loss operator
- `dx_line` is line-wise horizontal instability
- `dy` is slow vertical displacement
- `tau_c` is chroma delay relative to luma
- `phi_c` is chroma phase perturbation
- `n_y` / `n_c` are stochastic noise terms
- `D` is the reconstruction/decode operator

The exact kernels, distributions, and pass decomposition are deliberately left open for the next implementation phase.

## Domain Ownership

### What belongs in `casseted-signal`

`casseted-signal` should own the signal contract:

- `VhsModel`
- `VhsSignalStage` / `VHS_SIGNAL_FLOW_V1`
- the grouped parameter families for input, luma, chroma, transport, noise, and reconstruction
- compact still-image prototype controls in `SignalSettings`

This is domain structure, not GPU structure.

### What belongs in `casseted-types`

For v1, `casseted-types` should stay focused on generic frame data:

- frame size and descriptor metadata
- pixel format
- owned image buffers

No additional signal-specific enums need to move there yet. The current `FrameDescriptor`, `FrameSize`, `PixelFormat`, and `ImageFrame` remain sufficient because the new work is about signal semantics, not a new shared frame container API.

### What should stay out of public domain API for now

The following are implementation concerns and should remain in `casseted-pipeline` / WGSL rather than inflating `casseted-signal`:

- exact filter tap counts and kernel weights
- pass fusion or pass splitting strategy
- random hash functions and noise sampling details
- uniform packing layout
- temporary textures and intermediate buffer orchestration
- cache/pipeline reuse policies

## Parameter Groups In Code

The formal domain model in `casseted-signal` is intentionally small:

- `VhsInputSettings`: transfer, matrix, temporal semantics
- `VhsLumaSettings`: luma bandwidth/detail-shaping controls
- `VhsChromaSettings`: chroma bandwidth, gain, delay, and phase
- `VhsTransportSettings`: line-wise and frame-wise spatial instability
- `VhsNoiseSettings`: luma/chroma noise and dropout behavior
- `VhsDecodeSettings`: reconstruction softness and Y/C leakage

This is enough to anchor future implementation without introducing presets, schemas, or a universal analog-signal abstraction.

## Mapping To Future Pipeline Stages

The future pipeline does not need to mirror the domain model one-to-one, but the mapping should stay legible:

- `VhsInputSettings` -> shared conversion code / initial uniform assumptions
- `VhsLumaSettings` -> luma blur or luma bandwidth-loss stage
- `VhsChromaSettings` -> chroma offset / bleed / phase stage
- `VhsTransportSettings` -> coordinate warp stage or integrated line-wise lookup distortion
- `VhsNoiseSettings` -> noise/dropout stage or noise injection inside the final pass
- `VhsDecodeSettings` -> final reconstruction step back to output RGB

The current `SignalSettings`-driven still shader is a projection of only part of this model:

- `luma.blur_px` ~= coarse proxy for `VhsLumaSettings.bandwidth_mhz`
- `chroma.offset_px` and `chroma.bleed_px` ~= coarse proxies for chroma delay and bandwidth loss
- `noise.*` ~= subset of `VhsNoiseSettings`
- `tracking.*` ~= subset of `VhsTransportSettings`

## Explicitly Deferred

The following work is intentionally not part of v1 formalization:

- full multi-pass GPU implementation of the entire model
- video and frame-sequence support
- preset systems and user-facing schemas
- a full reference CPU engine
- deck-accurate RF emulation
- generalized pass graphs or runtime redesign

## Implementation Consequence

The next step should be a planning/projection layer that maps `VhsModel` into concrete still-image pipeline stages, while keeping the current runtime architecture intact.

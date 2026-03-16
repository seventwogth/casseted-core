# VHS / Analog Model v1

This document defines the first formal signal model for `casseted-core`.

It is intentionally not a full rewrite plan for the current GPU prototype. The goal of v1 is narrower:

- establish one stable signal-flow for future implementation work
- define which parts of VHS/analog behavior are inside the model
- make the parameter taxonomy explicit in `casseted-signal`
- keep the existing crate boundaries intact

The corresponding domain types live in `casseted-signal` as `VhsModel` and the `Vhs*Settings` groups.

## Scope

VHS / analog v1 models a single-generation consumer playback look at the signal level, starting from an already decoded digital frame.

Included:

- gamma-coded RGB input normalization
- RGB to luma/chroma decomposition
- separate luma and chroma bandwidth loss
- chroma delay and phase error
- horizontal time-base instability and slow vertical wander
- head-switching band displacement
- additive luma/chroma noise and scan-line dropouts
- decode/reconstruction back to display RGB

Explicitly deferred:

- exact RF carrier simulation and FM sideband math
- helical scan geometry as a physical tape-head model
- AGC, servo loops, and deck-specific calibration behavior
- exact field-comb behavior for interlaced broadcast ingest
- multi-generation dubbing loss
- audio path simulation

This keeps v1 grounded enough to look and feel structurally like VHS, while avoiding a research-heavy RF/deck emulator before the rest of the pipeline is ready.

## Signal Flow

`casseted-signal` exposes the canonical stage order as `VHS_SIGNAL_FLOW_V1`:

1. `InputDecode`
2. `RgbToLumaChroma`
3. `LumaRecordPath`
4. `ChromaRecordPath`
5. `TransportInstability`
6. `NoiseAndDropouts`
7. `DecodeOutput`

Conceptually the flow is:

```text
R'G'B' input
  -> normalize transfer/matrix assumptions
  -> Y'/C decomposition
  -> luma bandwidth + pre-emphasis path
  -> chroma bandwidth + delay/phase path
  -> transport displacement and head-switching region
  -> additive noise and dropout masking
  -> decode / reconstruct to display RGB
```

Two details matter for later implementation:

- v1 is a signal-domain model, not a shader-layout prescription
- luma and chroma are modeled as separate paths before final reconstruction

## Mathematical Shape

The v1 model is intentionally compact. A useful mental form is:

```text
Y_out(x, y, t) = H_y{Y_in(x + dx_line(y, t), y + dy(t), t)} + n_y(x, y, t)
C_out(x, y, t) = H_c{g_c * C_in(x + dx_line(y, t) - tau_c, y + dy(t), t)}
                 with additional phase error phi_c and chroma noise n_c
RGB_out = D{Y_out, C_out, decode_settings}
```

Where:

- `H_y` is the luma-loss operator with VHS-like horizontal bandwidth limits
- `H_c` is the chroma-loss operator with much lower bandwidth than luma
- `dx_line` is line-level horizontal instability
- `dy` is slow vertical wander
- `tau_c` is luma/chroma delay mismatch
- `phi_c` is chroma phase error
- `n_y` / `n_c` are stochastic noise terms
- `D` is the output decode/reconstruction step

The math here is deliberately operator-level. v1 fixes the structure of the model before committing to exact filter kernels, pass counts, or random-process implementations.

## Parameter Taxonomy

`VhsModel` groups parameters by signal responsibility rather than by current shader uniforms.

`VhsInputSettings`

- `matrix`: working luma/chroma matrix, currently `Bt601`
- `transfer`: current assumption for incoming RGB transfer
- `temporal_sampling`: progressive-frame vs interlaced-field semantics

`VhsLumaSettings`

- `bandwidth_mhz`: luma cutoff after record/playback loss
- `preemphasis_db`: broad luma pre-emphasis / de-emphasis amount

`VhsChromaSettings`

- `bandwidth_khz`: much lower chroma bandwidth
- `saturation_gain`: chroma amplitude scaling
- `delay_us`: chroma delay relative to luma
- `phase_error_deg`: chroma phase perturbation

`VhsTransportSettings`

- `line_jitter_us`: per-line horizontal time-base error
- `vertical_wander_lines`: slow vertical displacement
- `head_switching_band_lines`: bottom-band size affected by head switching
- `head_switching_offset_us`: horizontal displacement inside that band

`VhsNoiseSettings`

- `luma_sigma`: additive luma noise strength
- `chroma_sigma`: additive chroma noise strength
- `chroma_phase_noise_deg`: stochastic chroma phase noise
- `dropout_probability_per_line`: scan-line dropout frequency
- `dropout_mean_span_us`: typical dropout span

`VhsDecodeSettings`

- `chroma_vertical_blend`: vertical chroma reconstruction softness
- `luma_chroma_crosstalk`: residual Y/C leakage retained in output
- `output_transfer`: display transfer assumption

## Relationship To The Current Prototype

The existing still-image shader remains valid, but it should now be treated as a preview operator over a smaller subset of the full model.

Current `SignalSettings` roughly correspond to:

- `luma.blur_px` -> a coarse proxy for `VhsLumaSettings.bandwidth_mhz`
- `chroma.offset_px` / `chroma.bleed_px` -> coarse proxies for chroma delay and bandwidth loss
- `noise.*` -> part of `VhsNoiseSettings`
- `tracking.*` -> part of `VhsTransportSettings`

What the prototype does not yet express explicitly:

- signal standard (`NTSC` vs `PAL`)
- transfer/matrix assumptions
- chroma phase behavior
- head-switching region semantics
- dropout structure
- decode-stage reconstruction controls

That separation is intentional. `SignalSettings` can keep serving the current single-pass shader, while `VhsModel` becomes the source of truth for future CPU-side planning, uniform packing, and multi-pass implementation.

## Implementation Guidance

The next phase should not begin with a crate reshuffle.

Recommended sequence:

1. keep `VhsModel` in `casseted-signal` as the domain contract
2. add projection/planning code that turns `VhsModel` into concrete pipeline stages
3. evolve `casseted-pipeline` to consume that planned representation
4. only then decide whether any multi-pass GPU decomposition is warranted

This preserves the current repository shape and moves complexity into signal formalization first, which is the right pressure point for the project at this stage.

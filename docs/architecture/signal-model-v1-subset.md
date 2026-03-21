# Signal Model v1 Runtime Subset Status

This note records how the current still-image runtime relates to the formal `VhsModel`.

Important distinction:

- a formal stage can already be active while some fields in its owning parameter group are still deferred
- the clearest example is `InputDecode`: the runtime currently assumes gamma-coded `sRGB` input and a BT.601-like matrix, but changing `VhsInputSettings` does not yet change shader behavior

Use this as the field-level companion to:

- [`signal-model-v1.md`](./signal-model-v1.md)
- [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md)

## Fully Active

### Tone shaping

- `VhsToneSettings.highlight_soft_knee`
  Runtime path: `SignalSettings.tone.highlight_soft_knee` -> `effect.input_conditioning.x` -> `soft_highlight_knee()`.
  Notes: also feeds the derived highlight-bleed threshold in the luma stage.
- `VhsToneSettings.highlight_compression`
  Runtime path: `SignalSettings.tone.highlight_compression` -> `effect.input_conditioning.y` -> `soft_highlight_knee()`.
  Notes: also feeds the derived highlight-bleed amount in the luma stage.

### Chroma / reconstruction direct terms

- `VhsChromaSettings.saturation_gain`
  Runtime path: `SignalSettings.chroma.saturation` -> `effect.chroma_degradation.z` -> final chroma gain in `degrade_chroma()`.
- `VhsDecodeSettings.chroma_vertical_blend`
  Runtime path: `effect.chroma_degradation.w` -> vertical line blend inside `degrade_chroma()`.
- `VhsDecodeSettings.luma_chroma_crosstalk`
  Runtime path: `effect.reconstruction_output.z` -> `y_c_leakage_luma()` in the reconstruction pass.
  Notes: the shader backs this off slightly inside stronger dropout concealment, but the field itself is live and direct.

## Partially Active / Approximated

### Input / working-signal assumptions

- `InputDecode` as a formal stage is active, but it currently runs as a fixed assumption set rather than a field-driven selector:
  gamma-coded `sRGB` input, BT.601-like `YUV`, and progressive still-frame interpretation.
  Notes: the stage exists in the runtime, while `VhsInputSettings` still do not parameterize it.

### Luma parameters

- `VhsLumaSettings.bandwidth_mhz`
  Runtime path: `luma_blur_from_bandwidth()` -> `SignalSettings.luma.blur_px` -> `effect.luma_degradation.x` -> `degrade_luma()`.
  Why partial: the runtime does not implement a literal MHz-domain transfer function; it uses one compact bandwidth-loss proxy that expands into sample spacing, low/mid/fine-band attenuation, and part of the highlight-bleed derivation.
- `VhsLumaSettings.preemphasis_db`
  Runtime path: `detail_mix_from_preemphasis()` -> `effect.luma_degradation.y`.
  Why partial: live, but only as one restrained detail-recovery scalar rather than a fuller record/playback emphasis curve.
- `highlight bleed`
  Runtime status: active, but derived from tone + luma state rather than represented as a standalone formal field.
  Runtime path: `highlight_bleed_threshold()` / `highlight_bleed_amount()` -> `effect.luma_degradation.zw` -> `highlight_bleed()`.

### Chroma parameters

- `VhsChromaSettings.delay_us`
  Runtime path: `chroma_offset_from_delay()` -> `SignalSettings.chroma.offset_px` -> `effect.chroma_degradation.x`.
  Why partial: the live effect is a signed, attenuated horizontal offset proxy kept subordinate to bandwidth loss by preview guardrails.
- `VhsChromaSettings.bandwidth_khz`
  Runtime path: `chroma_bleed_from_bandwidth()` -> `SignalSettings.chroma.bleed_px` -> `effect.chroma_degradation.y`.
  Why partial: the shader expands one bandwidth-loss proxy into low-pass span, coarse cell size, integration step, and restrained smear instead of implementing a carrier-accurate chroma bandwidth model.
- `VhsChromaSettings.phase_error_deg`
  Runtime path: model-only resolved radians -> `effect.reconstruction_aux.z` -> `rotate_chroma()` in `still_chroma_degradation.wgsl`.
  Why partial: the live effect is a direct UV-plane rotation of the degraded chroma vector at the chroma/reconstruction boundary, not a carrier-reference or decoder-locked phase model.
- `ChromaRecordPath` overall
  Runtime status: active as a compact `prefilter -> cell integration -> coarse reconstruction -> restrained trailing contamination -> optional vertical blend -> restrained phase bias` approximation, not as a full chroma-carrier path.

### Tracking / transport parameters

- `VhsTransportSettings.line_jitter_us`
  Runtime path: `line_jitter_px_from_timebase()` -> `SignalSettings.tracking.line_jitter_px` -> `effect.input_conditioning.z`.
  Why partial: the shader uses one deterministic still-frame sinusoid as jitter amplitude; there is no temporal time-base evolution or standalone transport pass.
- `VhsTransportSettings.vertical_wander_lines`
  Runtime path: `SignalSettings.tracking.vertical_offset_lines` -> `effect.input_conditioning.w`.
  Why partial: it is interpreted as a still-frame vertical offset snapshot, not a fuller slow transport process.
- `VhsTransportSettings.head_switching_band_lines`
  Runtime path: model-only resolved line count -> `effect.frame.z` -> `apply_head_switching_approximation()` in `still_reconstruction_output.wgsl`.
  Why partial: it only localizes a restrained lower-frame switching band for a seam/disturbance approximation; it is not a field-timing or deck-geometry simulation.
- `VhsTransportSettings.head_switching_offset_us`
  Runtime path: `head_switching_offset_px_from_time()` -> `effect.reconstruction_output.w` -> `apply_head_switching_approximation()` in `still_reconstruction_output.wgsl`.
  Why partial: it becomes a bounded horizontal switching displacement plus mild chroma-support loss inside the lower switching band, not a full timing relock, tearing bar, or decoder-accurate head-switching model.
- `TransportInstability` overall
  Runtime status: only the spatial still-image subset is active. The global still-frame jitter/vertical offset terms remain fused into `still_input_conditioning.wgsl`, while the lower-band head-switching approximation is inserted later in `still_reconstruction_output.wgsl` as a compact model-only transport-side auxiliary.
  Current status note: no `head_switching_*` field remains in `Deferred / Documented Only`; what remains deferred is only a fuller timing-/deck-accurate head-switching model.

### Noise / dropout parameters

- `VhsNoiseSettings.luma_sigma`
  Runtime path: `luma_noise_amount_from_sigma()` -> `SignalSettings.noise.luma_amount` -> `effect.reconstruction_output.x`.
  Why partial: the final pass reshapes it into brightness-weighted, partly line/band-correlated reconstruction contamination instead of injecting raw white-noise sigma.
- `VhsNoiseSettings.chroma_sigma`
  Runtime path: `chroma_noise_amount_from_sigma()` -> `SignalSettings.noise.chroma_amount` -> `effect.reconstruction_output.y`.
  Why partial: the current still path attenuates and broadens it into softer additive chroma contamination instead of injecting raw chroma-noise sigma.
- `VhsNoiseSettings.chroma_phase_noise_deg`
  Runtime path: model-only resolved radians -> `effect.reconstruction_aux.w` -> low-band chroma-vector perturbation in `sample_reconstruction_contamination()`.
  Why partial: the runtime maps the formal sigma into a bounded, line/band-correlated local phase perturbation of the current chroma vector rather than simulating stochastic carrier-phase noise directly.
- `VhsNoiseSettings.dropout_probability_per_line`
  Runtime path: `effect.reconstruction_aux.x` -> `line_dropout_mask()`.
- `VhsNoiseSettings.dropout_mean_span_us`
  Runtime path: `dropout_span_px_from_time()` -> `effect.reconstruction_aux.y` -> `line_dropout_mask()`.
  Why partial: both fields are active only through the restrained still-image dropout subset: local line masks, neighboring-line concealment, chroma collapse, and contamination backoff.
- `dropout-related terms`
  Runtime status: active, but only as a still-frame local concealment approximation inside the final pass, not as temporal dropout recovery.

### Projection / preview boundary

- `StillImagePipeline::from_vhs_model()` activates the model-backed subset above.
- `StillImagePipeline::new(SignalSettings)` keeps model-only auxiliaries neutral:
  luma `detail_mix`, chroma `vertical_blend`, chroma-phase auxiliaries, head-switching auxiliaries, Y/C leakage, and dropout terms stay at zero unless a formal model is present.
- `preview_base_signal()` still ignores the formal chroma-phase and head-switching terms on purpose:
  they bypass the compact preview control surface and resolve only during stage packing.
- preview guardrails remain active only on the compact preview surface; they do not redefine the formal model.

## Deferred / Documented Only

### Input / temporal selectors

- `VhsInputSettings.matrix`
- `VhsInputSettings.transfer`
- `VhsInputSettings.temporal_sampling`
  Current state: documented assumptions exist and the stage is active, but changing these fields does not yet change projection, uniforms, or WGSL behavior.

### Decode / output selector

- `VhsDecodeSettings.output_transfer`
  Current state: present in the formal model and docs, but changing it does not change projection, resolved stages, packed uniforms, or WGSL behavior in the current still runtime subset.
  Current fixed assumptions: `still_reconstruction_output.wgsl` stops at `decode_output_rgb()`, which applies the inverse BT.601-like working matrix and clamps into `[0, 1]`, and `runtime.rs` writes those decoded RGB numerics directly to `Rgba8Unorm`.
  Boundary note: the active subset therefore ends at reconstructed, contamination-shaped, decode-approximated RGB; display/output transfer behavior is still only an implicit numeric-storage assumption, not a formal-model-controlled term.
  Why deferred: the current still path never establishes a separate output-referred or linear-light handoff, so activating `Srgb` versus `Bt1886Like` right now would mostly invent a new post-decode look layer rather than expose an already-grounded transfer stage.

### Standard metadata

- `VhsModel.standard`
- `VideoStandard::{frame_rate_hz, field_rate_hz, line_period_us}`
  Current state: used to define formal presets and future mapping context, but the still runtime does not branch on them once a concrete `VhsModel` already contains resolved field values.

## Most Justified Next Activations

1. `VhsInputSettings.*`
   Why next: they are still the clearest remaining decode-side selectors that could later make the current fixed input assumptions explicit without expanding the runtime into a broader subsystem.
2. A broader decode/output milestone, if later justified
   Why later: `VhsDecodeSettings.output_transfer` should only move when the still path grows an explicit post-decode output semantic boundary, not as a standalone post-look toggle.

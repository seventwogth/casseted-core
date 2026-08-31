# 0020 Resolution-Invariant Reconstruction Contamination

Date: 2026-08-31

Stage:
extends the correctness pass of `0019` from the luma and chroma branches into the reconstruction/output pass, covering the horizontal half of its resolution dependence

Status:
implemented in the repository working tree

Agent commit status:
this stage was committed by the agent on `main`.

## Why this stage was needed

`0019` fixed the branch passes and explicitly listed the reconstruction pass as still resolution-dependent. That entry named three things: procedural noise band frequencies stated per output pixel, a quiet-region probe sampling immediate neighbours, and single-line vertical neighbourhoods.

The practical symptom is the inverse of the branch defect. There, high resolution changed how much detail survived. Here, high resolution changes how much contamination is visible: noise structure specified per output pixel becomes finer as the raster grows, so a 4K render of the same content reads cleaner than the reference-width render, and much of the contamination averages away as soon as the image is viewed scaled down.

Measured on a flat grey field, comparing contamination power at matched cycles-per-frame:

| band, cycles/frame | 720 px | 3600 px, before |
| --- | --- | --- |
| 1-60 | 2.03e-07 | 6.84e-08 (0.34x) |
| 60-200 | 1.36e-07 | 1.09e-07 (0.80x) |
| 200-340 | 1.29e-07 | 4.71e-08 (0.37x) |

The structured low band was the worst affected, which is expected: those are exactly the terms with an intended relative frequency.

## What changed

All in `still_reconstruction_output.wgsl`, using the same clamped `s_hat = max(frame.x / 720, 1)` factor the branch passes already derive.

- `smooth_noise_x` divides its phase by `s_hat`, so `cells_per_px` is now stated per reference pixel. This covers every band term at once: luma band and surface drift, chroma bands and surfaces, and the phase-noise band
- the finest luma noise carrier moved from a per-output-pixel hash to `reference_scaled_fine_noise`, which samples the hash on the reference-pixel grid and interpolates horizontally. This is the largest single component of the luma contamination mix, so leaving it at raster frequency would have undone much of the rest
- the dropout segment floor, edge softness, and breakup frequency are resolved against `s_hat`; the dropout concealment noise uses the same scaled fine-noise carrier
- the head-switching seam breakup frequency is resolved against `s_hat`
- the quiet-region probe steps by `s_hat` on both axes, since its gradient thresholds are calibrated against reference-pixel neighbours

`reference_scaled_fine_noise` was written so that at `s_hat = 1` the interpolation weight is exactly zero for integer coordinates, which makes it reduce to the original per-pixel hash. Combined with the clamp, output at or below the reference width is unchanged bit for bit. This was verified directly: a 720 px render of a corpus image is byte-identical to a render made before this stage, and the contamination probe reports the same value to the last digit.

## Result

| probe | before | after |
| --- | --- | --- |
| contamination power at 2160 px, relative to reference width | 0.42x | 0.85x |
| contamination power at 3600 px, relative to reference width | 0.35x | 0.84x |
| total noise energy at matched relative frequencies, 3600 px | 0.48x | 0.71x |

The remaining shortfall is understood rather than unexplained. Two causes:

- the per-line terms are functions of the scan-line index only and are still absolute, so they do not scale
- smoothstep interpolation rolls off the top of the fine carrier's spectrum, so its shape at high resolution is not identical to white noise at the reference width even though its band limit now is

Both belong to the deferred vertical work rather than to this stage.

## Verification added

`reconstruction_contamination_stays_resolution_invariant_when_gpu_is_available` renders a flat field at 720, 2160, and 3600 px wide under a model with only luma contamination active, and measures the contamination power carried at a fixed set of cycles-per-frame with a sparse spectral probe. The probe frequencies stay below the 360 cycles/frame Nyquist limit of the narrowest raster so the measurement is valid at every width.

Confirmed to fail on the unfixed pass at 0.42x against a 0.45 tolerance, and to pass at 0.85x, so the tolerance sits between the two with roughly 2.8x headroom on the passing side.

A first attempt used the spectral centroid over a wide window and was discarded: the window exceeded the Nyquist limit of the narrowest raster, and an unnormalized transform made the values incomparable across widths. Band power at matched frequencies is the metric that actually answers the question.

## Remaining resolution dependence

Only the vertical axis now. The per-line contamination and phase terms and the single-line neighbourhoods used by chroma vertical blend, dropout concealment, and the head-switching band all model scan lines, and normalizing them needs an explicit reference height in the model rather than the width-derived factor used here.

The quiet-region probe is the one exception already handled: it is an isotropic gradient estimate rather than a scan-line term, so it uses `s_hat` on both axes as an approximation until that reference height exists.

## Recommended next milestone

Two candidates, in this order.

First, the calibration-metric rework described in `0019`. It remains the blocking item for moving the calibration suite to the reference width, and its absence is why the suite still cannot observe either invariance property. Its edge-retention metric returns values above 1.0 on the current synthetic images at 720 px because added contamination outweighs the bandwidth loss it is meant to measure; the metric has to separate the two before it can be trusted at any width.

Second, an explicit reference height, which unblocks the vertical terms above.

The chroma-vs-luma pass for small colored accents recommended by `0018` is still open and unaffected.

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- byte-identity check of a 720 px render against a pre-change render
- flat-field spectral measurement at 720, 2160, and 3600 px

## Related touched areas

- `shaders/passes/still_reconstruction_output.wgsl`
- `crates/casseted-pipeline/src/stage_regression.rs`
- `docs/math/signal-model-v1-formulas.md`
- `docs/architecture/signal-model-v1.md`
- `README.md`

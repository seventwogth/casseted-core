# 0019 Resolution-Invariant Spatial Calibration

Date: 2026-08-31

Stage:
correctness pass on the horizontal spatial terms of the luma and chroma branches, so the calibrated still-image look no longer depends on output frame width

Status:
implemented in the repository working tree

Agent commit status:
this stage was committed by the agent on `main`.

## Why this stage was needed

Previous milestones were calibration passes: they moved the look. This one is different. It fixes a defect that made the look move on its own.

The still path states horizontal spatial quantities in reference pixels at 720 px wide and resolves them against the real frame through `s_ref = W / 720`. The discrete formulas mixed two kinds of pixel quantity in the same expression:

- terms already multiplied by `s_ref` on the CPU side, such as `r_Y` and `r_C`
- fixed constants written straight into the formulas, which stayed in absolute pixels

Because both are in pixels, the mixture is silent. Nothing asserts it, and it reads as ordinary tuning.

## Measured symptom

Rendering one image at increasing widths with identical settings, then comparing everything at 720:

| render width | s_ref | luma detail kept | chroma detail kept | chroma/luma loss ratio |
| --- | --- | --- | --- | --- |
| 720 | 1.00 | 0.613 | 0.204 | 2.06 |
| 1080 | 1.50 | 0.565 | 0.260 | 1.70 |
| 1440 | 2.00 | 0.554 | 0.269 | 1.64 |
| 1920 | 2.67 | 0.532 | 0.278 | 1.54 |
| 2400 | 3.33 | 0.517 | 0.285 | 1.48 |

Luma degraded progressively harder while chroma degraded progressively less. The chroma-over-luma hierarchy that the whole signal-first calibration is built on flattened by 28% across a 3.3x width change.

Confirmed independently on a step chart generated natively at each resolution, so no resampling was involved: the edge transition width, expressed as a fraction of frame width, shrank by about a third from 720 to 3600 px.

The visible consequence at high resolution was the wrong pairing: mushier fine texture together with sharper chroma edges.

## Root cause

Three groups of terms, all the same defect class.

Luma, in `still_luma_degradation.wgsl`:

- `bandwidth_mix(r) = r / (r + 1.35)` received an `s_ref`-scaled radius against an absolute constant, so the mix saturated toward 1 as width grew and the residual-band attenuation strengthened with resolution
- `max(0.5, 0.55 r_Y + 0.45)` kept an absolute additive term, so the sample footprint grew sub-linearly and became relatively narrower

Chroma, in `still_chroma_degradation.wgsl`:

- the same saturating-mix problem in `bandwidth_mix(r) = r / (r + 1.0)`
- the leading constants in `r_L = 0.40 + ...` and `d_C = 1.0 + ...` stayed absolute, so the coarse chroma cell became relatively finer as width grew; this is the term that dominates how much horizontal chroma resolution survives, which is why chroma moved opposite to luma
- the prefilter step floors, the cell-integration floor, the `delay_mix` constant, and the single-pixel luma-edge guard were all absolute as well

## What changed

Both branch shaders now derive `s_hat = max(effect.frame.x / 720, 1)` and carry every horizontal constant through it. The saturating mixes are evaluated on the reference-pixel radius `r / s_hat`, so attenuation strength tracks the modelled bandwidth rather than the output raster.

No uniform lane was added: frame width was already in the shared block, so the factor is derived in the shaders.

`s_hat` is clamped at `1.0` deliberately. The look is defined at the reference width, and below it the raster is already the narrower limit, so the calibration is scaled up but never down. This also means behavior at or below 720 px is bit-identical to before, which is why every existing test still passes untouched.

What did **not** change:

- no new pass, uniform lane, or preview control
- no change to the projection layer, guardrails, or crate boundaries
- no recalibration of any look constant; all tuned values keep their reference-width meaning
- the reconstruction pass was left alone

## Result

| probe | before | after |
| --- | --- | --- |
| luma modulation transfer spread, 80 cycles/frame, 720..3600 px | 14.0% | 0.3% |
| chroma modulation transfer spread, same | 97.6% | 4.3% |
| chroma/luma hierarchy drift on a real image, 720..2400 px | 34% | 4% |

## Verification added

`horizontal_bandwidth_response_stays_resolution_invariant_when_gpu_is_available` in `stage_regression.rs` renders one frame carrying a luma grating and a constant-luma chroma grating at a fixed relative frequency, at 720, 2160, and 3600 px wide, and measures modulation transfer with a single-bin DFT at the grating frequency.

That metric was chosen after two weaker ones were rejected: a direct edge-width measurement is quantization-bound at 720 px, and an aggregate gradient ratio conflates bandwidth loss with added contamination. The DFT probe is insensitive to sub-pixel sampling phase.

The test was confirmed to fail on the unfixed shaders, with a measured drift of 23% against a 10% tolerance.

## Coverage gap this exposed

The existing suites do not exercise the calibrated regime:

- `stage_regression.rs` runs at 96 px wide, `s_ref = 0.13`
- `calibration.rs` runs at 160 px wide, `s_ref = 0.22`

At those widths the default spatial terms are sub-pixel and the filters are close to inactive, which is why a defect of this size lived in the shaders without any test noticing. The new grating test is currently the only coverage at or above the reference width.

Raising the calibration suite to the reference width was attempted during this stage and reverted. At 720 px its edge-retention metric returns values above 1.0 on the existing synthetic images, because those images are smooth enough that added reconstruction contamination outweighs the bandwidth loss the metric is meant to observe. Fixing that means reworking the metric, not just the frame size, and it belongs to its own milestone.

## Remaining resolution dependence

The reconstruction pass was intentionally left out of scope, and still carries absolute-pixel behavior:

- procedural noise band frequencies in `smooth_noise_x` are expressed per pixel, so the contamination texture becomes finer as width grows
- the quiet-region gradient probe samples immediate neighbours, so more of the frame reads as calm at higher resolution
- vertical single-line neighbourhoods used by chroma vertical blend, dropout concealment, and head switching model adjacent scan lines but are fixed at one pixel

The first two change the character of noise rather than the luma/chroma hierarchy, and the third needs a vertical reference height rather than the width-based factor used here. Both were kept separate so this stage stays a bounded correctness fix with no look recalibration attached.

## Recommended next milestone

Reference-width normalization of the reconstruction pass, paired with the calibration-metric rework the reverted experiment above described. Those two belong together: the metric has to be able to separate bandwidth loss from added contamination before the noise terms can be safely rescaled.

The chroma-vs-luma pass for small colored accents recommended by `0018` remains open and is unaffected by this stage.

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- manual CLI renders across 720..3600 px on the reference corpus and on external high-resolution inputs

## Related touched areas

- `shaders/passes/still_luma_degradation.wgsl`
- `shaders/passes/still_chroma_degradation.wgsl`
- `crates/casseted-pipeline/src/stage_regression.rs`
- `docs/math/signal-model-v1-formulas.md`
- `docs/architecture/signal-model-v1.md`
- `README.md`

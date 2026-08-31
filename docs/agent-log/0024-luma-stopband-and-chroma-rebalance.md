# 0024 Luma Stopband And Chroma Rebalance

Date: 2026-08-31

Stage:
acts on the luma-stopband finding recorded in `0023`, and rebalances chroma so the signal-first hierarchy survives the change

Status:
implemented in the repository working tree

Agent commit status:
this stage was committed by the agent on `main`, together with the measurement prerequisite described below.

## Why this stage happened now

`0023` characterized a defect and deliberately left it alone: the luma path had no stopband, and the obvious repair broke the calibration suite. It listed three questions for a later milestone. The repository owner answered two of them — the broad softening the repair costs is acceptable, because the picture already reads too clean, and chroma should be softened in step so the hierarchy survives.

The third question, whether the hierarchy measurement itself needed fixing first, turned out to be answered by the work rather than by opinion.

## The measurement prerequisite

With the repair applied, the suite still failed on `neutral-low-saturation`. Measuring the same fixtures with contamination disabled showed why: the hierarchy was intact, and the failure came from the metric.

Edge retention is a property of the branch filters, but reconstruction contamination adds chroma gradient energy of its own. On low-saturation fixtures it adds more than the fixture carries. Measured on the full render, `dark-quiet-floor` reported chroma retention of `0.76` against luma `0.47`; measured without contamination, the same fixture reported `0.19` against `0.46`. The apparent inversion was entirely the noise.

So the suite now renders each case twice. The hierarchy and retention assertions read a contamination-free render, because that is what describes the filters; the quiet-region and contamination-character assertions keep reading the full render, because that is what describes the final pass. No thresholds moved — the existing bounds hold on the corrected measurement.

That is committed separately as `Contamination-Free Hierarchy Measurement`, since it is a measurement correctness fix and stands on its own.

## What changed in the look

Luma, in `still_luma_degradation.wgsl`: the finest residual band is now `fine - mid` rather than `center - mid`, where `fine` is a narrow three-tap. `center` is a single unfiltered sample carrying every frequency up to Nyquist, so the old form never let the band vanish and the pass response tended toward `micro_gain` instead of rolling off.

Chroma, in `projection.rs`: the bandwidth proxy divisor tightened from `300` to `220`, taking the default chroma radius from `2.333` to `3.182` reference pixels.

## Result

Modulation transfer at the reference raster, default settings:

| cycles/frame | period, px | luma before | luma after | chroma before | chroma after |
| --- | --- | --- | --- | --- | --- |
| 20 | 36.0 | 0.997 | 0.992 | 0.842 | 0.802 |
| 40 | 18.0 | 0.917 | 0.901 | 0.606 | 0.496 |
| 60 | 12.0 | 0.814 | 0.778 | 0.347 | 0.222 |
| 90 | 8.0 | 0.673 | 0.599 | 0.102 | 0.051 |
| 160 | 4.5 | 0.581 | 0.433 | 0.019 | 0.008 |
| 260 | 2.8 | 0.629 | 0.489 | 0.001 | 0.002 |

Broad structure is essentially untouched — at a 36 px period luma moves by half a percent. The stopband is about a quarter deeper, and chroma stays far below luma at every frequency.

On `05_ui-text-detail/interface.png` the letterforms soften visibly at a 1:1 crop while remaining fully legible, which is the direction the reference notes have asked for since `0017`.

## Why the repair is restrained

The three-tap is weighted `0.10, 0.80, 0.10`, not the full band-pass `0.25, 0.50, 0.25`.

The full form deepens the stopband further, to `0.21` at 160 cycles instead of `0.43`. It also softens luma far enough to invert the hierarchy on low-saturation content even with contamination excluded: `neutral-low-saturation` goes to luma `0.310` against chroma `0.400`.

The reason is structural rather than incidental. The coarse chroma cell reconstruction of section `4.4` puts a floor under chroma edge energy — its cell boundaries contribute gradient energy in proportion to the signal, so widening the chroma low-pass barely moves the ratio. Pushing the chroma divisor from `300` to `190` moved retention on that fixture only from `0.400` to `0.380`. Luma can be softened until it approaches that floor and no further.

Measured margins on the contamination-free render, luma retention minus chroma retention:

| case | before | after |
| --- | --- | --- |
| colored-edges | 0.349 | 0.148 |
| portrait-midtones | 0.329 | 0.136 |
| bright-highlights | 0.210 | 0.124 |
| neutral-low-saturation | 0.166 | 0.047 |
| ui-detail-edges | 0.300 | 0.145 |
| dark-quiet-floor | 0.274 | 0.175 |

Every case stays ordered, but `neutral-low-saturation` now clears by `0.047` where it used to clear by `0.166`. That is the binding constraint on any further softening, and it should be treated as spent headroom rather than as room to keep taking.

## Recommended next milestone

If more luma softening is wanted, the chroma cell floor has to be addressed first — otherwise there is nothing left to soften into. That means looking at whether the cell reconstruction should band-limit its own staircase, which is a change to section `4.4`'s reconstruction rather than to any gain.

Remaining deferred model surface is unchanged: `VhsInputSettings.{transfer,temporal_sampling}` and `VhsDecodeSettings.output_transfer`.

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`
- MTF sweeps for both branches, 20 to 340 cycles/frame, before and after
- per-case hierarchy margins measured with contamination excluded
- 1:1 visual comparison on `05_ui-text-detail/interface.png`

## Related touched areas

- `shaders/passes/still_luma_degradation.wgsl`
- `crates/casseted-pipeline/src/projection.rs`
- `crates/casseted-pipeline/src/calibration.rs`
- `crates/casseted-pipeline/src/stage_regression.rs`
- `docs/math/signal-model-v1-formulas.md`
- `docs/testing.md`

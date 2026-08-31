# 0022 Reference-Raster Line Geometry

Date: 2026-08-31

Stage:
completes the resolution work started in `0019` by giving the model an explicit reference line count and resolving every line-oriented term against it

Status:
implemented in the repository working tree

Agent commit status:
this stage was committed by the agent on `main`.

## Why this stage was needed

`0019` and `0020` normalized the horizontal axis and both closed with the same remaining item: the vertical one. Line-oriented terms were still expressed in output rows, and the width-derived factor could not fix them because it says nothing about how many lines a picture has.

The clearest symptom is dropout. `VhsNoiseSettings.dropout_probability_per_line` is a probability per line, drawn once per output row. On a raster with five times the rows it fired five times as often. Measured on a flat field with the default model:

| frame height | dropout bands, before |
| --- | --- |
| 480 | 4 |
| 2400 | 21 |

Those 21 events are also one output row tall each, so on a tall raster the artifact was simultaneously five times more frequent and five times thinner relative to the picture. Neither matches the model.

The same class of error applied to the head-switching band height, the jitter phase, the still-frame vertical offset, the per-line contamination carriers, and the single-line vertical neighbourhoods used by chroma vertical blend and dropout concealment.

## The decision this required

Unlike the horizontal work, this could not be done inside the shaders. The horizontal reference is a fixed 720 samples per line, identical for both standards, so it could be hardcoded. The vertical reference is not: NTSC carries 480 active lines and PAL 576.

That made `VhsModel.standard` the source of truth, and it was listed in three documents as intentionally not projected into the runtime. Activating it was a model-boundary decision rather than an implementation detail, so it was put to the repository owner, who chose the standard-dependent form over a fixed 480.

The activation is deliberately narrow. The runtime consults the standard for the active line count and for nothing else; `frame_rate_hz`, `field_rate_hz`, and `line_period_us` stay unprojected, since they are temporal and the still path has no temporal model. Changing the standard changes what counts as one line. It does not branch the pass chain.

## What changed

`VideoStandard::active_lines()` was added to `casseted-signal`, returning 480 for `NtscM` and 576 for `Pal`.

Both reference factors are now resolved and clamped in `stages.rs` and passed to the shaders in a new `effect.reference = (s_hat, s_hat_v, 0, 0)` lane, widening the uniform block from 24 to 28 floats. This also removed the hardcoded 720 from three shaders: the reference-raster policy now has one owner instead of being restated in WGSL, and the standard-dependent line count never has to be encoded there. The manual preview path has no model to consult and assumes the NTSC line count.

Resolved against `s_hat_v`:

- every per-line hash is drawn once per reference line, so dropout, head-switching breakup, and the per-line contamination carriers keep their specified rate
- quantities stated in lines are scaled to output rows: the head-switching band height, its seam falloff, and the vertical offset
- the jitter phase advances per reference line, keeping the wobble's vertical period fixed
- single-line neighbourhoods step by the reference-line spacing: chroma vertical blend, dropout concealment, and the vertical half of the quiet-region probe

The finest noise carrier interpolates horizontally but holds one value per reference line. That is a deliberate asymmetry: it makes the carrier line-correlated on tall rasters, which is how analog line noise reads, and it avoids the hard vertical blocking a nearest-neighbour grid on both axes would produce.

Both factors stay clamped at 1.0, so output at or below the reference raster is unchanged. Verified directly: a 720x480 render is byte-identical to a render made before this stage.

## Result

Dropout bands on a flat field, same model, varying height:

| frame height | before | after |
| --- | --- | --- |
| 480 | 4 | 4 |
| 1440 | — | 4 |
| 2400 | 21 | 4 |

Row coverage stays near 0.8 percent throughout, confirming that each band now spans the right number of output rows rather than staying one row thin.

## Verification added

`line_oriented_artifacts_stay_invariant_across_frame_height_when_gpu_is_available` renders a flat field at 480, 1440, and 2400 px tall under a dropout-only model and asserts the band count matches the reference-height count exactly. Heights are exact multiples of the NTSC active line count so reference lines map cleanly and the comparison can be exact rather than tolerance-based.

Confirmed to fail without the fix, reporting 25 / 70 / 137 bands against an expected 25.

## Where the resolution work now stands

All three axes of the original defect are closed, each with its own probe: horizontal bandwidth response, horizontal contamination density, and vertical line geometry. The calibration suite runs at the reference width with a metric that measures bandwidth loss rather than contamination.

Nothing in the current still path is known to depend on output resolution any more. The one documented approximation is the vertically line-correlated fine carrier described above, which is a character choice rather than a drift.

## Recommended next milestone

The chroma-vs-luma pass for very small colored accents recommended by `0018` is now the oldest open item and the highest-value one. It is better supported than when it was first proposed: `03_color-edges-chroma` and `05_ui-text-detail` can now be judged at the reference width with a metric that reports bandwidth loss honestly.

Remaining deferred model surface is unchanged: `VhsInputSettings.{transfer,temporal_sampling}` and `VhsDecodeSettings.output_transfer`, both of which still need a semantic boundary rather than a look toggle.

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`
- byte-identity check of a 720x480 render against a pre-change render
- dropout band measurement at 480, 1440, and 2400 px tall, before and after

## Related touched areas

- `crates/casseted-signal/src/vhs.rs`
- `crates/casseted-pipeline/src/stages.rs`
- `crates/casseted-pipeline/src/stage_regression.rs`
- all four passes in `shaders/passes/`
- `docs/math/signal-model-v1-formulas.md`
- `docs/architecture/signal-model-v1.md`
- `docs/architecture/signal-model-v1-subset.md`
- `docs/testing.md`
- `README.md`

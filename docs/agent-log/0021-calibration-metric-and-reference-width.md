# 0021 Calibration Metric And Reference Width

Date: 2026-08-31

Stage:
replaces the edge-retention metric in the synthetic calibration suite and moves that suite to the reference width, closing the item `0019` and `0020` both listed as blocking

Status:
implemented in the repository working tree

Agent commit status:
this stage was committed by the agent on `main`.

## Why this stage was needed

`0019` tried to move the calibration suite from 160 px to the reference width and reverted, because at 720 px the suite's `luma_edge_retention` and `chroma_edge_retention` returned values above `1.0` — the output appeared to gain edge energy. That made the suite unusable at the width the look is actually calibrated for, and it was recorded as the next milestone.

The cause turned out to be the metric itself rather than the frame size.

Both ratios were built from the mean *absolute* horizontal step. That quantity is total variation, and total variation is conserved when a monotonic edge is blurred: spreading a step of height `h` over more pixels leaves the sum of absolute steps at `h`. So on step-like synthetic content the metric could not observe bandwidth loss at all. What it could observe was the contamination the final pass adds, which raises the sum. The smoother the test image, the more the metric reported noise instead of filtering.

Measured at 720 px with the old metric, retention reached 2.06 for luma and 2.92 for chroma on `portrait-midtones`, and 2.69 / 4.54 on `dark-quiet-floor`. Those are not degradation figures; they are noise-injection figures wearing a degradation label.

At 160 px the same metric looked plausible only because the features occupy fewer pixels, so per-pixel signal gradients are steeper and the added contamination is proportionally smaller. The suite was passing for the wrong reason.

## What changed

Edge energy is now the mean *squared* horizontal step. Squared gradient energy does fall as an edge spreads — for a step spread over `n` pixels it goes as `h²/n` — which is the property these ratios were always meant to report. `chroma_distance_squared` was split out for the edge accumulation, while the quiet-region detector keeps the original absolute distance, since that threshold is a neighbourhood test rather than an energy measure.

With the metric corrected, `CALIBRATION_SIZE` moved from 160x120 to 720x540.

One threshold needed retuning: `ui-detail-edges` asserted `luma_edge_retention() > 0.45`, a bound written against the old scale. Under squared-gradient semantics that case retains 0.30, which is the strongest degradation of the six and exactly what the highest-frequency content should show. The bound is now `0.20`, preserving the original intent — do not collapse UI/text structure into mush — on the new scale.

Every other assertion in the suite passed unchanged at the new width and metric.

## Result

Squared-gradient retention at 720 px, default pipeline:

| case | luma | chroma |
| --- | --- | --- |
| colored-edges | 0.446 | 0.092 |
| portrait-midtones | 0.440 | 0.140 |
| bright-highlights | 0.418 | 0.326 |
| neutral-low-saturation | 0.517 | 0.276 |
| ui-detail-edges | 0.300 | 0.065 |
| dark-quiet-floor | 0.500 | 0.760 |

Chroma sits below luma on every case except the last, which is the known limitation described below. The signal-first hierarchy the architecture claims is now visible in the numbers instead of being masked.

The metric is also close to insensitive to contamination, which was the whole point. Rendering `colored-edges` with noise disabled versus enabled gives luma retention 0.443 against 0.446; under the old metric the same comparison was 1.054 against 2.056.

## Verification that the suite has teeth

Passing tests prove little on their own here, since the change makes a previously failing configuration pass. So the suite was checked against a deliberately broken pipeline: forcing `chroma_blur_px` to zero in the chroma pass, which removes chroma bandwidth loss entirely.

The suite fails on that, at `neutral-low-saturation should keep neutral structure ahead of chroma breakup`. The reworked assertions detect a real hierarchy regression.

## Known limitation

Edge retention stays trustworthy only where the content's own gradient energy is meaningfully above the contamination floor. `dark-quiet-floor` in chroma is the case where it is not: retention there moves from 0.187 to 0.760 depending on whether contamination is enabled, because a dark near-monochrome field carries very little chroma gradient energy of its own.

That case currently has no chroma-retention assertion, and none should be added. This is the same effect noted during review on low-saturation real images, where default chroma contamination is a large fraction of the actual chroma content.

## What this unblocks

The calibration suite now runs in the regime the look is calibrated for, so future calibration milestones can be judged there rather than at a width where the branch filters are close to inactive.

It does not, by itself, give the suite resolution coverage: it renders at one width. Drift across widths remains the job of the two dedicated probes added in `0019` and `0020`.

## Recommended next milestone

An explicit reference height in the model. It is now the only structural item left from the resolution work: the per-line contamination and phase terms and the single-line vertical neighbourhoods used by chroma vertical blend, dropout concealment, and the head-switching band are still absolute, and normalizing them needs a vertical reference rather than the width-derived factor.

The chroma-vs-luma pass for very small colored accents recommended by `0018` is still open. It is now better supported than before, since `03_color-edges-chroma` and `05_ui-text-detail` can be judged with a metric that actually reports bandwidth loss.

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`
- negative check with chroma bandwidth loss disabled, confirming the suite fails

## Related touched areas

- `crates/casseted-pipeline/src/calibration.rs`
- `docs/testing.md`
- `docs/math/signal-model-v1-formulas.md`
- `README.md`

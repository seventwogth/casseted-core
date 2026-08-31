# 0023 Calibration Fixture Scale And Luma Stopband Finding

Date: 2026-08-31

Stage:
corrects the synthetic calibration fixtures so they represent their content classes at the reference width, and records a characterized luma-path defect that was deliberately not fixed in this stage

Status:
fixture correction implemented in the repository working tree; the luma finding is documented only

Agent commit status:
this stage was committed by the agent on `main`.

## Part one: fixture scale

### Why this was needed

`0021` moved the calibration suite from 160x120 to the reference width. The fixture generators mix relative coordinates (`fx`, `fy`) with absolute pixel arithmetic, and the absolute half did not move with the frame.

The worst case is `ui_detail_edges_image`, which draws its grid with `y % 8`, `x % 16`, its underline stripes with `x % 5`, and its colour segments with `x / 6` — all in output pixels. At the 160 px width those were sensible proportions: a 16 px grid gives ten columns across the frame. At 720 px the same arithmetic gives forty-five columns, and `x % 5` becomes 144 single-pixel vertical lines. The fixture named "UI and text detail" had quietly become a near-Nyquist grating.

`neutral_low_saturation_image` (a 20 px checker) and `dark_quiet_floor_image` (a 4 px stripe pattern) had the same problem, and several fixtures had margins that shrank from 7.5 percent of the frame to 1.7 percent.

The suite still passed, so nothing drew attention to it. It was found only because a separate experiment produced results on the UI fixture that did not match the same measurement on the real `05_ui-text-detail/interface.png` reference — the synthetic case collapsed roughly three times faster.

This is a defect the `0021` size change introduced the conditions for, and it belongs with that work rather than with whatever happened to trip over it.

### What changed

A `CALIBRATION_DESIGN_WIDTH` of 160 and a `design_unit()` helper were added, and every absolute feature size in the fixtures is now expressed in those units. Periodic patterns keep both their spacing and their stroke width proportional, so a line authored as one pixel of a 160 px frame renders as `unit` pixels wide rather than staying one output pixel thin.

The suite passes unchanged against the corrected fixtures with no threshold edits, which is the result to want: the fixtures now describe what they claim to, and the existing intent-shaped bounds still hold.

### Effect on the reported numbers

Squared-gradient retention at the reference width, default pipeline, before and after the fixture correction:

| case | luma before | luma after |
| --- | --- | --- |
| colored-edges | 0.446 | 0.440 |
| portrait-midtones | 0.440 | 0.438 |
| bright-highlights | 0.418 | 0.418 |
| neutral-low-saturation | 0.517 | 0.759 |
| ui-detail-edges | 0.300 | 0.392 |
| dark-quiet-floor | 0.500 | 0.473 |

The two cases that moved are exactly the two carrying periodic patterns. Their previous figures were reporting how the chain treats near-Nyquist gratings, not how it treats UI detail or a neutral interior.

## Part two: the luma stopband, documented not fixed

### The finding

The luma path has no stopband. Its modulation transfer at the reference width does not roll off toward Nyquist; it flattens and then rises:

| cycles/frame | period, px | luma MTF |
| --- | --- | --- |
| 60 | 12.0 | 0.814 |
| 100 | 7.2 | 0.639 |
| 140 | 5.1 | 0.579 |
| 180 | 4.0 | 0.596 |
| 220 | 3.3 | 0.620 |
| 260 | 2.8 | 0.630 |
| 340 | 2.1 | 0.533 |

Detail at a three-pixel period survives better than detail at a five-pixel period. For a stage whose stated purpose is bandwidth limitation that is the wrong sign, and it sits exactly where text and UI strokes live.

The mechanism is in `degrade_luma`. The finest band is taken as `center - mid_luma`, where `center` is a single unfiltered sample carrying every frequency up to Nyquist. So the band never vanishes: as `mid_luma`'s response oscillates toward zero at high frequency, the band tends toward the raw signal, and the pass response tends toward `micro_gain` — about 0.56 at default settings — rather than toward zero. The non-monotonic ripple on top of that floor is the oscillation of `mid_luma`'s own response, sampled at 1.5 px steps.

### Why it was not fixed here

The obvious repair is to take the band as a difference of two low-passes, `fine - mid`, using a narrow three-tap for `fine`. That was implemented and measured. It works: the floor drops from 0.58 to 0.21 at 160 cycles and low frequencies are essentially untouched.

It also fails the calibration suite, and not on a technicality. Softening luma broadly moves `neutral-low-saturation` from luma 0.759 / chroma 0.672 to luma 0.651 / chroma 0.672, inverting the signal-first hierarchy the whole architecture rests on. Even the mildest useful setting crosses it.

The chroma figure in that comparison is itself contamination-dominated — the same low-gradient-energy limitation `0021` recorded for `dark-quiet-floor`, which turns out to affect `neutral-low-saturation` too. So the assertion that blocks the change is measuring something fragile.

That is a reason to be careful, not a licence to proceed. Loosening a guard to admit a change the guard exists to catch would be the wrong move, and there is no clean-source-plus-VHS reference pair in the corpus to validate a look change of this size against. The corpus holds target looks, not matched pairs.

So this stays a documented finding with measurements attached, for a deliberate calibration milestone to take up.

### What such a milestone would need to decide

- whether the luma stopband is worth the broad softening it costs, given the look is already described as reading too clean on UI and text
- whether chroma should be softened in step so the hierarchy survives, which makes it a two-branch calibration rather than a one-line repair
- whether the low-saturation chroma-retention fragility should be fixed first, so the hierarchy assertions can be trusted while judging the tradeoff

The third is arguably the prerequisite for the other two.

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`
- MTF sweeps at the reference width, 5 to 340 cycles/frame, with and without the candidate stopband repair
- per-case retention measurements before and after the fixture correction

## Related touched areas

- `crates/casseted-pipeline/src/calibration.rs`
- `docs/testing.md`

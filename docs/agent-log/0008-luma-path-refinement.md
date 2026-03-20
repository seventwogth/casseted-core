# 0008 Luma Path Refinement

Date: 2026-03-20

Stage:
deepened the still-image luma path inside the existing limited multi-pass architecture

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The current still-image pipeline was already structurally sound, but the luma branch was still too compact relative to its visual importance:

- the previous luma path could read too much like a symmetric horizontal blur with a small residual add-back
- microcontrast loss, edge softness, and highlight behavior were not separated clearly enough
- secondary artifacts had improved, but the luma foundation still risked feeling less intentional than the surrounding chroma/noise refinements

The next useful move was therefore to deepen the luma branch itself without adding passes, widening the public API, or disturbing the compiled runtime layer.

## What changed

- `still_luma_degradation.wgsl` no longer treats the luma stage as one compact blur kernel plus one residual term
- the luma shader now builds the output from:
  a broader low-pass baseline,
  a narrower mid-band estimate,
  separate attenuation of mid-band and fine-band residuals,
  a small bright-edge lag bias on the low-pass branch,
  and a contour-gated directional highlight bleed term
- the same compact `effect.luma_degradation` contract is preserved:
  `x` remains the luma bandwidth-loss proxy,
  `y` remains the pre-emphasis-derived detail recovery mix,
  `zw` remain the derived highlight threshold and amount
- `SignalSettings.luma.blur_px` is now documented more explicitly as a legacy preview name for a bandwidth-loss proxy rather than a literal blur radius
- formulas and architecture docs now describe the two-scale luma approximation instead of the old 5-tap blur model
- the stage reference PNGs that depend on luma output were refreshed:
  `luma-degradation.png`
  `reconstruction-output.png`

## Why this integration path was chosen

- luma remains the primary visual foundation, so the refinement belongs inside `LumaRecordPath`, not in a new pass or a decorative post-effect
- the existing multi-pass architecture already gives luma its own branch, so deeper signal behavior can live there without changing crate boundaries or runtime structure
- keeping the uniform contract compact preserves preview guardrails and the compiled runtime layer

## Still-image v1 approximations used

- this is still not a deck-calibrated analog transfer function or a temporal luma model
- the implementation uses a compact two-scale FIR-like decomposition to separate broad structure, mid-band edges, and fine microcontrast
- highlight bleed is still restrained and directional, but it is now gated by bright contour energy so flat highlights do not drift toward bloom-like behavior

## Verification completed for this stage

- `cargo check --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `crates/casseted-signal`
- `shaders/passes/still_luma_degradation.wgsl`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`
- `assets/reference-images/still-pipeline-v1/luma-degradation.png`
- `assets/reference-images/still-pipeline-v1/reconstruction-output.png`

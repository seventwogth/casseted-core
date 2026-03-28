# 0014: Head-Switching Status Closure

Date: 2026-03-20

## Summary

Closed the remaining status drift around `VhsTransportSettings.head_switching_*`.

Final classification:

- `head_switching_band_lines`
- `head_switching_offset_us`

are `Partially Active / Approximated` in the current still-image runtime.

They are not deferred, and they are not fully modeled.

## Status drift that was found

The repository already had real runtime activation:

- `resolve_reconstruction_output_stage()` resolves both terms from `VhsModel`
- `EffectUniforms` packs them into `effect.frame.z` and `effect.reconstruction_output.w`
- `still_reconstruction_output.wgsl` consumes them in `apply_head_switching_approximation()`
- existing tests already showed stage-state changes and GPU-output changes when the terms are enabled

The remaining ambiguity was mostly historical/documentary:

- older agent-log entries still contained pre-activation wording
- those entries were accurate when written, but could be misread as the current state if read in isolation

## Why Activation remains the honest path

The active implementation is already visible in the runtime path and stays within the existing still-image scope:

- bottom-band localization only
- bounded horizontal offset proxy only
- restrained luma/chroma disturbance only
- no temporal state
- no deck-accurate geometry
- no glitch-bar presentation

That is enough to count as a real still-image subset activation, but not enough to claim a fuller transport model.

## Evidence kept aligned

- code path: `crates/casseted-pipeline/src/stages.rs`
- uniform mapping: `crates/casseted-pipeline/src/stages.rs`
- WGSL consumption: `shaders/passes/still_reconstruction_output.wgsl`
- subset classification: `docs/architecture/signal-model-v1-subset.md`
- formulas description: `docs/math/signal-model-v1-formulas.md`
- runtime invariants: `crates/casseted-pipeline/src/tests.rs`

## Closure result

After this pass, the intended reading is:

- `head_switching_*` is active in the model-backed still-image runtime subset
- `head_switching_*` bypasses the preview control surface on purpose
- only fuller timing-/deck-accurate head-switching behavior remains deferred

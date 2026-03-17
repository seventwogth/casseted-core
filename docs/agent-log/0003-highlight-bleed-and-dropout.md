# 0003 Highlight Bleed And Dropout

Date: 2026-03-17

Stage:
added restrained highlight bleed and dropout to the current limited multi-pass still-image signal chain

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The current still-image path had become structurally sound, but the output could still read slightly too intact:

- bright areas rolled off well, yet some highlights still stayed cleaner than the surrounding analog-like degradation
- the final output carried noise, but not the localized signal-loss imperfections that help analog playback feel less pristine
- the next useful move was therefore not more passes or more controls, but two restrained second-order artifacts inside the existing architecture

## What changed

- `highlight bleed` was added inside `still_luma_degradation.wgsl`
- the implementation is thresholded and directional:
  bright luma spills from preceding horizontal samples into the current pixel
- the bleed is derived from existing tone/luma terms rather than exposed as a new preview control
- `dropout` was added inside `still_reconstruction_output.wgsl`
- the implementation uses the existing formal `VhsNoiseSettings.dropout_*` fields
- the dropout shape is line-oriented and local, with soft horizontal masks and neighboring-line concealment
- the manual preview path stays neutral for dropout, so the public preview API did not grow for this milestone
- the four-pass still architecture, crate boundaries, and preview guardrails all stayed intact

## Why this integration path was chosen

- highlight bleed belongs most naturally to `LumaRecordPath`, not to a standalone bloom-style pass
- dropout already belongs formally to `NoiseAndDropouts`, so the final reconstruction/output pass was the minimal place to implement it
- this keeps the current fan-out and branch structure unchanged:
  input conditioning,
  luma degradation,
  chroma degradation,
  reconstruction/output

## Still-image v1 approximations used

- highlight bleed is not modeled as optics or modern glow; it is a highlight-gated asymmetric luma smear
- dropout is not modeled with temporal history, previous-field recovery, or decoder-specific compensation
- instead, the final pass uses deterministic line hashes plus adjacent-line concealment to approximate mild local signal loss in one still frame

## Verification completed for this stage

- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `shaders/passes/still_luma_degradation.wgsl`
- `shaders/passes/still_reconstruction_output.wgsl`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`
- `assets/reference-images/still-pipeline-v1/reconstruction-output.png`

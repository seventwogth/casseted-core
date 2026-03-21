# 0015 Output Transfer Boundary Review

Date: 2026-03-21

Stage:
review of the `output_transfer` formal-model gap and clarification of the current still-image decode/output boundary

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The current still-image architecture had already reached a good compact shape:

- the limited multi-pass split was stable
- the compiled runtime/resource-reuse layer was in place
- luma, chroma, reconstruction cleanup, chroma phase, and head-switching subset work had already landed

That left one small but important honesty gap near the output edge:

- the formal model still exposed `VhsDecodeSettings.output_transfer`
- the runtime docs already said it was deferred
- but the code path also already had fixed output-facing assumptions that were easy to overlook if "deferred" was read too loosely

The right move here was therefore a review of the boundary, not an automatic activation.

## What was analyzed

- `crates/casseted-signal/`
- `crates/casseted-pipeline/`
- `shaders/passes/still_reconstruction_output.wgsl`
- `shaders/passes/still_input_conditioning.wgsl`
- the current decode/output mapping in `docs/architecture/` and `docs/math/`
- the existing subset/milestone history in `docs/agent-log/`

## Findings

1. `output_transfer` is still runtime-deferred:
   changing `VhsDecodeSettings.output_transfer` does not change preview projection, resolved stages, packed uniforms, or WGSL behavior.
2. The runtime already has fixed output assumptions:
   the final pass ends at `decode_output_rgb()`, which performs the inverse BT.601-like matrix and clamps to `[0, 1]`.
3. Output behavior is partly implicit, not parameterized:
   the runtime writes those decoded/clamped RGB numerics directly into `Rgba8Unorm`, so there is no additional active output-transfer stage after decode.
4. A compact activation would still be semantically risky right now:
   because the still path never establishes a separate output-referred or linear-light handoff, activating `Srgb` versus `Bt1886Like` now would mostly introduce a new post-decode look layer rather than exposing an already-grounded transfer stage.

## Decision

`VhsDecodeSettings.output_transfer` remains `Deferred`.

This was the more engineering-honest choice for the current milestone because it:

- keeps the current architecture intact
- avoids widening the uniform/runtime contract for a semantically weak selector
- avoids overlapping a new post-decode shaper with the already established tone hierarchy
- makes the current decode/output boundary explicit instead of hiding it behind a pseudo-activation

## What changed

- added an explicit invariant test proving that `output_transfer` still does not change current stage resolution or uniform packing
- added code comments in the final WGSL pass and runtime output-format definition so the deferred status matches the code path itself
- updated subset/architecture/formulas docs to state the fixed current boundary more directly:
  reconstructed `Y/C` signal -> contamination/leakage/decode approximation -> clamped RGB numerics written to `RGBA8`
- updated the next-step guidance so future `output_transfer` work is tied to a broader decode/output milestone rather than treated as an isolated look toggle

## Why activation was not chosen

- there is no separate post-decode/output-referred domain in the current still pipeline
- there is no current linear-light handoff that would make `Srgb` versus `Bt1886Like` a grounded transfer selection
- the output target is plain unorm storage, not a broader display-management path
- a restrained activation at this point would therefore risk being cosmetically plausible but formally misleading

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `shaders/passes/`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`

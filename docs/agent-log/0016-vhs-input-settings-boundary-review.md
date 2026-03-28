# 0016 VhsInputSettings Boundary Review

Date: 2026-03-28

Stage:
review of the `VhsInputSettings` formal-model gap and clarification of the current still-image input boundary

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The current still-image architecture was already in a good place:

- the limited multi-pass split was stable
- the compiled runtime/resource-reuse layer was in place
- the base still-image signal chain, deep luma/chroma refinement, reconstruction cleanup, chroma phase, head switching, and output-transfer boundary review had already landed

That left one compact but important honesty gap on the input side:

- the runtime already entered the still path under fixed input assumptions
- the formal model still exposed `VhsInputSettings.{matrix,transfer,temporal_sampling}`
- the boundary between active `InputDecode` semantics and deferred field-level control was still too easy to over-read

The right move here was therefore a boundary review first, not a large new input-management feature.

## What was analyzed

- `crates/casseted-signal/`
- `crates/casseted-pipeline/`
- `shaders/passes/still_input_conditioning.wgsl`
- `shaders/passes/still_reconstruction_output.wgsl`
- `docs/architecture/signal-model-v1.md`
- `docs/architecture/signal-model-v1-subset.md`
- `docs/math/signal-model-v1-formulas.md`
- prior subset/boundary history in `docs/agent-log/`

## Findings

1. The still runtime already has fixed input-side assumptions:
   the first pass samples gamma-coded RGB, converts it through a BT.601-like working transform, and treats the image as one progressive still frame.
2. `transfer` and `temporal_sampling` are still runtime-deferred:
   changing either field does not change preview projection, resolved stages, packed uniforms, or WGSL behavior.
3. `matrix` is narrower than the other input fields:
   the current formal surface only exposes `VideoMatrix::Bt601`, and the active WGSL path already hardcodes the matching BT.601-like working transform in both `rgb_to_yuv()` and `yuv_to_rgb()`.
4. Broader field-level activation is not justified yet:
   `InputTransfer::{Srgb,Bt601}` would need a grounded transfer interpretation rather than a cosmetic look toggle, and `TemporalSampling::{ProgressiveFrame,InterlacedFields}` would need a real temporal/field boundary rather than a still-image guess.

## Decision

Chose `Partial Activation`, but only in the narrowest engineering-honest sense:

- `VhsInputSettings.matrix` is now treated as fixed-active under the current still-image subset
- `VhsInputSettings.transfer` remains deferred
- `VhsInputSettings.temporal_sampling` remains deferred

This was preferable to either extreme:

- not a large new feature step
- not a pseudo-activation of transfer or temporal behavior
- not an over-broad "everything is deferred" label that hid the already-fixed matrix boundary

## What changed

- tightened code comments so the active WGSL/runtime path explicitly states the current fixed BT.601-like matrix boundary
- clarified `VhsInputSettings` field docs in `casseted-signal`
- narrowed the runtime invariant test so it proves only the actually deferred input selectors stay ignored
- updated subset / architecture / formulas docs so `matrix` is classified separately from `transfer` and `temporal_sampling`

## Why broader activation was not chosen

- the current still path has no real input-transfer-management layer
- there is no temporal or field-aware execution boundary to attach `temporal_sampling` to
- widening the runtime now would mostly introduce selector surface area without grounded semantics
- the existing visual calibration is already stable and did not need a new decode framework

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `crates/casseted-signal`
- `crates/casseted-pipeline`
- `shaders/passes/`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`

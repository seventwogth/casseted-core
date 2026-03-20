# 0011 Formal Model Runtime Subset Review

Date: 2026-03-20

Stage:
review and formalization of the current active still-image runtime subset against formal signal-model v1

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The architecture, limited multi-pass split, and compiled runtime layer had already settled into a good shape. The next risk was no longer pass structure. It was clarity:

- the formal `VhsModel` is intentionally broader than the current still runtime
- the current still runtime already covers a meaningful subset of that model
- but the exact boundary between fully active, approximated, and deferred formal fields was still spread across code, formulas, and milestone history instead of living in one compact engineering reference

Before activating another algorithmic block, the repository needed that boundary to become explicit.

## Problems found

1. Field-level subset status was still implicit:
   stage docs were strong, but they did not yet give one direct answer to "which formal fields are really live right now?"
2. `InputDecode` could be misread as fully field-driven:
   the runtime does implement fixed `sRGB` + BT.601-like + progressive assumptions, but `VhsInputSettings` do not yet change runtime behavior and that distinction was not explicit enough.
3. Formulas docs still carried a small amount of naming drift:
   some reconstruction-stage helper names still reflected older final-pass terminology instead of the current reconstruction-centered naming in WGSL.

## What changed

- added [`docs/architecture/signal-model-v1-subset.md`](../architecture/signal-model-v1-subset.md) as the compact field-level reference for:
  `Fully Active`,
  `Partially Active / Approximated`,
  `Deferred / Documented Only`
- updated `docs/architecture/signal-model-v1.md` to point to that subset reference, to call out the stage-vs-field distinction explicitly, and to tighten the next-step guidance
- updated `docs/math/signal-model-v1-formulas.md` so the field-status summary, transport wording, and final-pass helper names match the current code
- updated architecture index/overview docs to expose the subset reference as a first-class document
- added one narrow invariant test proving that currently documented-only formal fields do not change the projected preview signal or packed still-image uniforms
- added one compact comment in `projection.rs` so the code states the same subset boundary that the docs now describe

## What is clearer now

- `highlight_soft_knee`, `highlight_compression`, `saturation_gain`, `chroma_vertical_blend`, and `luma_chroma_crosstalk` are plainly live terms
- luma/chroma bandwidth, transport, contamination, and dropout terms are active, but only through compact still-image projections and approximations
- `VhsInputSettings.*`, chroma phase fields, head-switching fields, `output_transfer`, and runtime use of `VideoStandard` remain consciously deferred
- the runtime subset boundary is now documented in one place instead of being reconstructed from several milestone docs

## Most justified next activations

1. `VhsChromaSettings.phase_error_deg` plus `VhsNoiseSettings.chroma_phase_noise_deg`
   These are the clearest remaining chroma-side formal fields, and they fit naturally inside the existing chroma/reconstruction boundary without requiring new passes or a larger public API.
2. `VhsTransportSettings.head_switching_*`
   These are the strongest remaining spatial transport terms already present in the formal model and can be introduced later as a restrained still-image subset without forcing a temporal/video architecture jump.

## Remaining debts

- the input/decode selector fields are still formal/documented rather than runtime-active
- the transport and dropout model is still the still-frame spatial subset, not a temporal transport system
- the chosen next activation should still stay inside the current architecture and should not reopen pass-graph or video-pipeline work

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`

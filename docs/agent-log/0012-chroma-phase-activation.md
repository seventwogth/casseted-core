# 0012 Chroma Phase Activation

Date: 2026-03-20

Stage:
activation of the formal chroma-phase terms inside the existing still-image runtime subset

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

After the subset review, the most natural remaining formal-model gap on the color side was no longer bandwidth or reconstruction cleanup. It was chroma phase:

- `VhsChromaSettings.phase_error_deg` existed formally but did not influence the still runtime
- `VhsNoiseSettings.chroma_phase_noise_deg` also existed formally but remained ignored
- the current chroma branch was already mature enough that the next useful gain was phase-like behavior in `Y/C` space, not more blur, offset, or decorative RGB splitting

The right move was therefore activation inside the existing chroma/reconstruction boundary, not a new pass and not a jump to a temporal/video architecture.

## Integration choice

- `phase_error_deg` now lives as a deterministic chroma-vector phase bias applied at the chroma/reconstruction boundary in `still_chroma_degradation.wgsl`
- `chroma_phase_noise_deg` now lives as a restrained stochastic phase perturbation of the current chroma vector inside `still_reconstruction_output.wgsl`
- both terms stay off the compact preview surface and resolve as model-only auxiliaries during stage packing

This keeps the public still-image preview API compact while making the formal model boundary more honest.

## What changed

- `crates/casseted-pipeline/src/stages.rs` now resolves:
  - deterministic chroma phase bias from `VhsChromaSettings.phase_error_deg`
  - stochastic chroma phase-noise scale from `VhsNoiseSettings.chroma_phase_noise_deg`
- the shared compact uniform block reuses the previously spare `effect.reconstruction_aux.zw` lanes for those model-only chroma-phase auxiliaries
- `shaders/passes/still_chroma_degradation.wgsl` now rotates the resolved degraded chroma vector by the deterministic phase bias before handing it to reconstruction
- `shaders/passes/still_reconstruction_output.wgsl` now applies low-band, dropout-aware chroma phase noise as a local chroma-vector perturbation instead of piggybacking on generic chroma contamination
- tests now prove:
  - the chroma-phase terms still do not alter `preview_base_signal()`
  - they do alter resolved runtime stages and packed uniforms
  - they do alter GPU output when a GPU adapter is available
- subset, architecture, and formulas docs now classify those terms as active approximations rather than deferred fields

## Still-image v1 approximation used

- deterministic phase error is a direct UV-plane rotation of the degraded chroma vector, not a carrier-reference or decoder-locked phase model
- stochastic phase noise is a bounded, line/band-correlated perturbation of the current chroma vector, not a physical subcarrier simulation
- both activations stay intentionally restrained and subordinate to bandwidth loss, luma structure, and the refined still-image reconstruction hierarchy

## What became clearer

- the chroma phase terms are now real runtime inputs instead of documented-only placeholders
- the preview surface remains intentionally smaller than the formal model
- the current still-image subset can absorb more formal-model fidelity without adding passes or widening crate boundaries

## Verification completed for this stage

- `cargo fmt --all`
- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `shaders/passes/`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`

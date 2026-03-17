# 0005 Signal Chain Stabilization Review

Date: 2026-03-18

Stage:
stabilization review of the base still-image signal chain as one coordinated system, with only targeted fixes to preview/model boundaries and shared stage semantics

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Review focus

This stage reviewed the current base signal chain end to end:

- input conditioning / tone shaping
- luma degradation
- chroma degradation
- highlight bleed
- dropout
- noise
- reconstruction / output

The main risks checked were:

- visual-priority conflicts between foundation stages and secondary artifacts
- reconstruction/output accumulating too much shared-state responsibility
- duplicated or unclear logic between formal signal semantics and preview-only approximations
- drift between formulas/docs and the current implementation

## Problems found

1. Preview/model boundary blur:
   `effective_preview_signal()` normalized the entire preview signal blob whenever a model-backed pipeline diverged from its projected preview settings. In practice that could rewrite untouched model-projected terms during an unrelated override.
2. Shared frame seed parked in an output-stage slot:
   the procedural frame seed used by both input conditioning and reconstruction-side helpers lived in `effect.reconstruction_output.w`, which made the reconstruction block look like the owner of a global cross-stage term.
3. Final-pass transport boundary not explicit enough:
   the reconstruction shader reused the same conditioned line phase for noise/dropout placement, but the code and docs did not say clearly enough that this was only procedural seeding, not a second transport resample.

## What changed

- model-backed preview overrides are now normalized per overridden term instead of re-normalizing untouched projected terms
- coupled chroma offset / bandwidth-loss overrides still normalize together so the guardrail keeps chroma degradation ahead of RGB-split-like misregistration
- the shared frame/procedural seed moved into the shared `effect.frame` block
- WGSL helpers now make the reconstruction-side procedural seeding intent explicit
- a focused regression test now checks that model-backed overrides do not rewrite untouched projected terms
- architecture and formulas docs were updated to match the stabilized semantics

## What did not need rebalancing

The review did not find a reason for a large visual rebalance:

- tone shaping, luma softening, and chroma bandwidth loss still remain the visual foundation
- highlight bleed remains derived and gated inside the luma path rather than behaving like bloom
- dropout remains restrained and line-local rather than glitch-art oriented
- noise remains signal-shaped and subordinate rather than turning into a uniform grain overlay

## Remaining debts

- `StillImagePipeline` still exposes both public `model` and `signal` fields, so direct caller mutation of `model` after construction can still require caller-managed reprojection for strict lockstep behavior
- reconstruction/output still intentionally keeps noise, dropout, and decode fused in one pass; this is acceptable for the current phase, but it remains an architectural pressure point to revisit only if deeper branch refinement later demands it

## Verification completed for this stage

- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `shaders/passes/still_input_conditioning.wgsl`
- `shaders/passes/still_luma_degradation.wgsl`
- `shaders/passes/still_chroma_degradation.wgsl`
- `shaders/passes/still_reconstruction_output.wgsl`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`

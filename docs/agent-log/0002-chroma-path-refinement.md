# 0002 Chroma Path Refinement

Date: 2026-03-17

Stage:
refined chroma degradation inside the existing limited multi-pass still-image pipeline

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The limited multi-pass split solved the structural problem, but the chroma branch still behaved like a compact proxy:

- one symmetric horizontal blur around a delayed center
- optional vertical blend layered on top
- a result that could still read too much like a softened RGB split instead of bandwidth-limited analog chroma

That was good enough for the transition stage, but it underused the luma/chroma separation that now exists physically in the pipeline.

## What changed

- the public still-preview controls stayed compact: `offset_px`, `bleed_px`, `saturation`, and decode-driven vertical blend
- `bleed_px` now acts as a shared chroma bandwidth-loss proxy instead of only a blur radius proxy
- the chroma pass now runs a compact sequence:
  1. horizontal chroma prefilter
  2. coarse horizontal chroma reconstruction on a derived cell width
  3. restrained delay-coupled smear / bleed
  4. optional vertical line blend
- the neutral branch remains exact when chroma blur is disabled
- preview guardrails, pass count, crate boundaries, and the formal model all stayed intact

## Why this is better

- horizontal chroma bandwidth loss now reads more like reduced chroma resolution than like a plain symmetric blur
- color boundaries smear and soften without turning into an aggressive chromatic aberration look
- the chroma branch stays visibly subordinate to luma detail, which better matches the intended analog / Video8-like character

## Verification completed for this stage

- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `shaders/passes/still_chroma_degradation.wgsl`
- `crates/casseted-pipeline`
- `crates/casseted-signal`
- `docs/math/`
- `docs/architecture/`
- `docs/agent-log/`
- `assets/reference-images/still-pipeline-v1/`

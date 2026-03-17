# 0001 Limited Multi-Pass Transition

Date: 2026-03-17

Stage:
limited multi-pass transition for the current still-image pipeline

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The calibrated still-image path had become visually stable enough that the main bottleneck was no longer look development, but implementation structure.

The previous fused single-pass path already encoded the intended signal logic, but it constrained further growth in three ways:

- luma and chroma development stayed tightly coupled inside one fragment path
- intermediate working signals could not be inspected or calibrated as real branch outputs
- new algorithmic work risked making the fused shader harder to reason about instead of closer to the formal signal model

This stage therefore focused on improving implementation health without changing the formal model and without introducing a render graph.

## Chosen decomposition

The chosen runtime split was a compact four-pass still-image pipeline:

1. input conditioning + tone shaping + `RGB -> YUV` fan-out
2. luma degradation
3. chroma degradation
4. reconstruction / output

This was treated as the minimal useful split because a three-pass design would still have kept luma and chroma degradation fused, while a larger pass count would have added orchestration weight too early.

## What changed

- the old fused `still_analog.wgsl` path was replaced by four fixed passes
- the pipeline now allocates three intermediate textures:
  working YUV,
  degraded luma,
  degraded chroma
- luma and chroma now exist as explicit physical branches
- the final output pass still keeps noise and decode/reconstruction together
- preview/manual guardrails were preserved
- the formal signal model and formulas were not redefined for implementation convenience

## Result

The repository now has a limited multi-pass still-image architecture that is closer to the formal signal-flow while remaining compact:

- the still path remains end-to-end working
- visual intent stays aligned with the recalibrated still-image priorities
- luma/chroma work can now evolve inside explicit branch passes
- the system still avoids a render graph, plugin system, temporal model, or video support

Verification completed for this stage:

- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

Related touched areas:

- `crates/casseted-pipeline`
- `crates/casseted-shaderlib`
- `crates/casseted-cli`
- `shaders/passes/`
- `docs/architecture/`
- `docs/math/`
- `assets/reference-images/still-pipeline-v1/`

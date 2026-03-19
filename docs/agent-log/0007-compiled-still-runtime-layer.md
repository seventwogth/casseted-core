# 0007 Compiled Still Runtime Layer

Date: 2026-03-19

Stage:
compact runtime reuse layer for the still-image pipeline

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Review focus

This stage targeted the next obvious runtime debt in the current still-image pipeline:
the algorithmic pass chain had matured, but the GPU execution path still recreated too much fixed execution state per run.

The goal was to improve repeated-run readiness without changing the formal model, without changing visual behavior, and without introducing a render graph or generic rendering framework.

## Problems confirmed

1. Localized runtime object creation:
   `runtime.rs` recreated the sampler, bind-group layouts, and all four `wgpu::RenderPipeline` objects inside each `process_with_gpu()` call.
2. Missing config vs compiled-state boundary:
   `StillImagePipeline` already cleanly owned model projection and preview state, but there was no separate representation for "GPU objects already prepared for execution".
3. Fixed pass chain without reusable compiled form:
   the still-image runtime already had a stable four-pass layout, yet that layout was not represented as a reusable compiled object.
4. Repeated-run overhead on the hot path:
   single-image CLI usage tolerated the setup churn, but the same structure would become technical debt for repeated runs, batch-style usage, and deeper pass refinement.

## What changed

- `casseted-pipeline` now exposes `StillPipelineRuntime` as a compact compiled runtime layer for the current still-image pass chain
- `StillPipelineRuntime` owns the reusable GPU execution objects for one `GpuContext`:
  the shared sampler,
  the single-input bind-group layout,
  the dual-input bind-group layout,
  and the four compiled render pipelines
- `StillImagePipeline` remains the high-level still-image description and now gains `process_with_runtime()`
- the existing `process_with_gpu()` entry point remains available and now layers on top of the compiled runtime path
- the runtime code now makes the reuse boundary explicit:
  compiled execution state in `StillPipelineRuntime`,
  per-run textures/buffers/bind groups in a narrow run-resource setup path
- test coverage now includes a repeated processing scenario that reuses one compiled runtime across multiple still-image runs and checks parity with the legacy GPU entry point

## What is intentionally reused now

- `wgpu::RenderPipeline` for:
  `still_input_conditioning`,
  `still_luma_degradation`,
  `still_chroma_degradation`,
  `still_reconstruction_output`
- the shared linear sampler
- the single-texture and dual-texture bind-group layouts
- the fixed compiled pass wiring for the current four-pass still-image chain

## What still remains per-run on purpose

- input texture upload
- intermediate and output textures
- uniform buffer
- bind groups that point at per-run views/buffers
- readback buffer

That remaining per-run allocation is a conscious still-image v1 compromise. It keeps the runtime layer small and useful without pulling in premature texture pooling, batch orchestration, or generalized pass scheduling.

## What did not change

- the formal signal model v1 contract
- the `VhsModel -> preview -> resolved stages` projection path
- the current four-pass still-image architecture
- visual calibration and current WGSL behavior
- crate boundaries
- deferred areas such as render-graph planning, video/temporal state, and generic engine-style abstractions

## Remaining debts

- textures and readback buffers are still allocated per run
- bind groups are still rebuilt per run because they reference per-run texture views and buffers
- the runtime layer is still purpose-built for the fixed still-image pass chain; broader cache keys and multi-shape reuse are intentionally deferred
- there is still no texture pool manager or batch subsystem, which remains acceptable for this stage

## Verification completed for this stage

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline/src/lib.rs`
- `crates/casseted-pipeline/src/runtime.rs`
- `crates/casseted-pipeline/src/state.rs`
- `crates/casseted-pipeline/src/tests.rs`
- `README.md`
- `docs/architecture/README.md`
- `docs/architecture/overview.md`
- `docs/architecture/pipeline.md`
- `docs/architecture/signal-model-v1.md`
- `docs/agent-log/`

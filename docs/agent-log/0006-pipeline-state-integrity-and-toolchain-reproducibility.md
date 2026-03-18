# 0006 Pipeline State Integrity And Toolchain Reproducibility

Date: 2026-03-18

Stage:
state-integrity hardening for the still-image pipeline plus reproducibility cleanup for the workspace toolchain and build entrypoints

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Review focus

This stage targeted two structural risks that had become more important than adding new image effects:

- drift between declared Rust requirements and the toolchain actually selected by the repository
- unsafe dual-source-of-truth behavior between the formal `VhsModel` and the projected preview/runtime representation inside `StillImagePipeline`

It also aimed to reduce local overload in `casseted-pipeline/src/lib.rs` without changing crate boundaries or the four-pass still-image architecture.

## Problems confirmed

1. Toolchain reproducibility drift:
   `Cargo.toml` declared `rust-version = 1.85`, while `rust-toolchain.toml` selected floating `stable`. After verification against the locked dependency set, the real minimum viable toolchain for the current checkout turned out to be Rust `1.88.0` because `image 0.25.10` no longer supports `1.85.0`.
2. Dual mutable sources of truth:
   `StillImagePipeline` exposed both `model` and `signal` as public mutable fields, so model reprojection and preview mutation could drift apart.
3. Override intent inferred indirectly:
   model-backed preview overrides were still inferred from equality against projected floats instead of being represented explicitly.
4. Formal/runtime mismatch on chroma delay sign:
   the formal model treated chroma delay as a relative quantity, but preview projection clamped negative delay to zero.
5. Localized responsibility overload:
   projection, guardrails, stage resolution, uniform packing, runtime setup, and readback all lived in one `lib.rs`.

## What changed

- the canonical workspace toolchain is now pinned to Rust `1.88.0` in `rust-toolchain.toml`
- the workspace `rust-version` now matches that exact canonical toolchain version and the current locked dependency floor
- `justfile` now uses `--locked` for check/test/clippy and exposes a small `ci` aggregate recipe
- `StillImagePipeline` now owns private pipeline state instead of exposing public mutable `model` and `signal` fields
- the internal state is now separated into:
  formal `model`,
  projected `preview_base_signal`,
  explicit `SignalOverrides`
- preview override intent is now represented explicitly through `SignalOverrides` instead of inferred from float equality
- `set_model()` now reprojections the preview base while preserving explicit override intent
- `clear_model()` and `clear_preview_overrides()` now collapse state intentionally instead of leaving stale projected values behind
- model-backed guardrails still preserve untouched projected terms, but now do so from explicit override state
- projection now preserves the sign of `VhsChromaSettings.delay_us`
- `casseted-pipeline` is now split internally into `state.rs`, `projection.rs`, `stages.rs`, and `runtime.rs`
- regression coverage now includes explicit-override persistence across model reprojection and signed chroma-delay projection

## What did not change

- the formal signal model v1 contract in `casseted-signal`
- the four-pass still-image runtime layout
- the current visual calibration priorities
- crate boundaries or package decomposition
- deferred algorithmic areas like chroma phase error, head switching, and temporal state

## Remaining debts

- override authoring is still intentionally low-level; there is no preset or inspector layer yet
- toolchain reproducibility is now explicit at checkout time, but the repository still does not add a larger CI platform in this stage
- the runtime still keeps decode, refined noise, and dropout in one output pass; that remains acceptable until deeper refinement work proves a harder split is necessary

## Verification completed for this stage

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `Cargo.toml`
- `rust-toolchain.toml`
- `justfile`
- `README.md`
- `crates/casseted-pipeline`
- `crates/casseted-cli`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`

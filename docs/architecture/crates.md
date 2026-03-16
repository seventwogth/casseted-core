# Crate Notes

## `casseted-types`

`casseted-types` contains the small, dependency-light types that are expected to be shared across the core workspace:

- frame/image size
- pixel format
- frame descriptor metadata
- owned image buffers

This crate should stay free of GPU runtime concerns and avoid collecting test-only conveniences, so it can be used by pipeline planning, CPU-side orchestration, and future serialization work.

## `casseted-signal`

`casseted-signal` defines the first minimal parameter model for analog-inspired image degradation. The model is intentionally grouped into a few practical buckets:

- luma softness
- chroma offset and bleed
- noise amounts
- line/tracking instability

The goal is not to describe every VHS characteristic upfront. The crate only provides a compact grouped settings model that later pipeline stages can consume without being tied to `wgpu` or shader implementation details.

## `casseted-shaderlib`

`casseted-shaderlib` keeps the repository-owned WGSL sources addressable from Rust code. It exposes a tiny shader registry with stable identifiers and embedded source strings, without adding a custom include or asset pipeline.

## `casseted-gpu`

`casseted-gpu` is the thin `wgpu` integration layer for the workspace. It currently provides:

- headless `Instance` / `Adapter` / `Device` / `Queue` initialization
- a compact GPU context descriptor
- helper functions for building shader modules from raw WGSL

This crate stays intentionally ignorant of repository shader identifiers. `casseted-pipeline` is the layer that bridges `casseted-shaderlib` assets to GPU execution.

## `casseted-pipeline`

`casseted-pipeline` now contains the first concrete still-image processing path. It owns the compact render-to-texture flow that connects:

- `ImageFrame` input data
- `SignalSettings` domain parameters
- built-in WGSL shader lookup from `casseted-shaderlib`
- `wgpu` execution via `casseted-gpu`

At this stage the crate intentionally implements one small effect pipeline rather than a generalized pass system.

## `casseted-cli`

`casseted-cli` is the developer-facing entry point for local checks. It currently loads one PNG image, runs the still-image pipeline, and writes one PNG result.

The crate intentionally keeps argument parsing and UX simple so it stays useful as a lightweight utility instead of becoming a second configuration layer.

## `casseted-testing`

`casseted-testing` holds small, reusable helpers for workspace tests:

- frame assertions
- deterministic gradient image generation
- simple image-difference statistics

It is not a visual regression platform; it only provides enough support to keep smoke tests readable and consistent.

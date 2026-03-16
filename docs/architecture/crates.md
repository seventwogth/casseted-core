# Crate Notes

## `casseted-types`

`casseted-types` contains the small, dependency-light types that are expected to be shared across the core workspace:

- frame/image size
- pixel format
- frame descriptor metadata

This crate should stay free of GPU runtime concerns so it can be used by pipeline planning, testing, CPU-side orchestration, and future serialization work.

## `casseted-signal`

`casseted-signal` defines the first minimal parameter model for analog-inspired image degradation. The model is intentionally grouped into a few practical buckets:

- luma softness
- chroma offset and bleed
- noise amounts
- line/tracking instability

The goal is not to describe every VHS characteristic upfront. The crate only provides a compact settings model and a small `SignalPlan` wrapper that can be consumed by later pipeline stages without being tied to `wgpu` or shader implementation details.

## `casseted-shaderlib`

`casseted-shaderlib` keeps the repository-owned WGSL sources addressable from Rust code. It exposes a tiny shader registry with stable identifiers and embedded source strings, without adding a custom include or asset pipeline.

## `casseted-gpu`

`casseted-gpu` is the thin `wgpu` integration layer for the workspace. It currently provides:

- headless `Instance` / `Adapter` / `Device` / `Queue` initialization
- a compact GPU context descriptor
- helper functions for building shader modules from WGSL

This crate should stay focused on runtime setup and low-level GPU utilities so the first pipeline can build on top of it without mixing in signal-domain logic.

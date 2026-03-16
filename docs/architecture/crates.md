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

# 0002: Keep shader loading and GPU setup thin

## Status

Accepted

## Context

The repository needs a practical foundation for the first pipeline step: shaders must live in the repo, be addressable from Rust, and compile into `wgpu` shader modules. At the same time, the project should avoid premature asset systems, custom preprocessors, and large runtime layers.

## Decision

Use a simple repository-owned WGSL layout:

- `shaders/include/` for future shared snippets
- `shaders/passes/` for pipeline-oriented shaders
- `shaders/debug/` for diagnostic shaders

Expose built-in shader assets from `casseted-shaderlib` through a compact enum-based registry, and keep `casseted-gpu` as a thin headless `wgpu` context layer with shader-module helpers.

## Consequences

- WGSL sources remain easy to find and review in the repository.
- Rust code can refer to shaders through stable identifiers instead of raw paths.
- The workspace gains a usable GPU runtime foundation without introducing a larger asset pipeline too early.

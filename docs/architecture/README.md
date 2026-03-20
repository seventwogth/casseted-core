# Architecture Notes

The current repository is intentionally split into small crates with narrow responsibilities:

- `casseted-types` keeps shared domain types stable and dependency-light.
- `casseted-signal` describes analog-style signal assumptions without binding them to GPU code.
- `casseted-shaderlib` owns built-in WGSL sources.
- `casseted-gpu` contains lightweight `wgpu` configuration helpers.
- `casseted-pipeline` composes the pieces into a minimal executable plan, with high-level still-image description kept separate from the compiled runtime layer.
- `casseted-cli` offers a small manual entry point for development.
- `casseted-testing` holds helpers that can be reused as the workspace grows.

This keeps the early core focused while leaving room for later implementation work.

Further crate notes:

- [`overview.md`](./overview.md)
- [`crates.md`](./crates.md)
- [`pipeline.md`](./pipeline.md)
- [`signal-model-v1.md`](./signal-model-v1.md)
- [`signal-model-v1-subset.md`](./signal-model-v1-subset.md)
- [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md)
- [`../testing.md`](../testing.md)

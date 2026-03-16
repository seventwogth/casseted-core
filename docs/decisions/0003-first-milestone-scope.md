# 0003: Keep the first milestone narrow

## Status

Accepted

## Context

`casseted-core` now has enough pieces to run a real still-image GPU effect and expose it through a local CLI. At this point the main risk is drifting into premature architecture: multi-pass systems, caching layers, richer preset flows, or early visual-regression infrastructure before the core processing path has settled.

## Decision

Treat the current repository state as a narrow first milestone:

- one still-image pipeline
- one PNG-oriented CLI utility
- one small shader registry
- one thin `wgpu` runtime layer
- lightweight smoke tests and image-difference helpers

Defer broader systems such as video support, pass graphs, preset migration, and visual-regression tooling until the still-image foundation needs them.

## Consequences

- The repository stays easier to reason about and extend.
- Documentation can describe the current implementation honestly.
- The next milestone can focus on improving the processing path instead of unwinding early abstractions.

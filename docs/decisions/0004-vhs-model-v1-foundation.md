# 0004: Formalize VHS / analog v1 before expanding the pipeline

## Status

Accepted

## Context

The workspace already has a clean architectural foundation and one functioning still-image GPU prototype. The next phase needs a more explicit VHS / analog model, but rewriting the pipeline immediately would risk mixing architectural work with unresolved signal assumptions.

## Decision

Adopt a formal VHS / analog v1 model in `casseted-signal` and document it in `docs/architecture/vhs-model-v1.md`.

The formal model will:

- define a stable signal-flow for the next implementation phase
- group parameters by signal responsibility instead of current shader uniform layout
- make approximation boundaries explicit
- coexist with the current prototype-oriented `SignalSettings`

The current still-image GPU path remains a valid technical prototype and should be treated as a projection of a subset of the full model, not as the final architecture of the VHS implementation.

## Consequences

- future implementation work can target an agreed signal contract before pass-graph work starts
- `casseted-signal` becomes the source of truth for the parameter taxonomy
- `casseted-pipeline` can evolve incrementally from projection/planning code instead of a full rewrite
- existing crate boundaries remain intact

# 0001: Start with a focused Cargo workspace

## Status

Accepted

## Context

The repository needs a clean base for shader, signal, GPU, and pipeline work without prematurely introducing web, API, or plugin-oriented concerns.

## Decision

Use a single Cargo workspace with a small set of crates that mirrors the current problem boundaries:

- shared types
- signal-domain configuration
- shader source library
- GPU helpers
- pipeline composition
- CLI entry point
- testing helpers

## Consequences

- The repository stays easy to navigate.
- Crate boundaries are explicit but still lightweight.
- Future work can grow inside existing crates before any deeper architectural split is considered.

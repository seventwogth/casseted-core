# 0009 Chroma Path Refinement v2

Date: 2026-03-20

Stage:
deeper chroma-path refinement inside the existing limited multi-pass still-image pipeline

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

After the luma branch was deepened, the chroma branch still lagged behind in maturity:

- it still leaned on one shared bandwidth-loss proxy that expanded mostly into a prefilter plus a simple coarse interpolation
- smear was present, but it still behaved too much like a softened shifted color layer in some edges
- the branch could still read as a compact proxy next to the more signal-shaped luma implementation

The architecture itself was already in a good place. What was missing was a richer chroma approximation inside that existing pass boundary.

## What changed

- the public preview/runtime contract stayed the same: `offset_px`, `bleed_px`, `saturation`, and decode-driven vertical blend
- the chroma shader now expands that compact contract into:
  1. broader horizontal chroma low-pass filtering
  2. cell-integrated coarse chroma sampling
  3. smooth coarse reconstruction with a quadratic B-spline-like basis
  4. restrained trailing contamination derived from coarse neighboring chroma cells
  5. luma-edge restraint so that extra color smear backs off on strong structural edges
  6. bandwidth-shaped vertical line blend before final saturation scaling
- the final reconstruction pass did not gain a new subsystem; it simply receives a more plausible degraded chroma texture

## Why this is better

- chroma bandwidth loss now reads more like reduced chroma sampling density and softer analog color spread, not just blur around an offset center
- color contamination has a more directional, scan-like tail without turning into decorative chromatic aberration
- strong luma edges stay in charge, so the color branch supports the image structure instead of becoming the main effect
- the result is closer to the intended analog degradation reference while preserving the compact multi-pass architecture and compiled runtime reuse layer

## Verification target for this stage

- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `shaders/passes/still_chroma_degradation.wgsl`
- `crates/casseted-pipeline`
- `crates/casseted-signal`
- `docs/math/`
- `docs/architecture/`
- `docs/agent-log/`

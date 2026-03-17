# 0004 Noise Path Refinement

Date: 2026-03-17

Stage:
refined the still-image noise path inside the existing limited multi-pass architecture so output noise reads more like analog signal contamination than like a uniform additive overlay

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The previous highlight-bleed / dropout milestone was successful, but the remaining noise path was still too compact:

- luma noise and chroma noise were both driven by simple per-pixel hash samples in the final pass
- that made the output noise read too much like a neutral grain overlay instead of like secondary signal contamination
- the final output needed more separation between luma and chroma behavior without adding new passes or expanding the public preview API

## What changed

- `still_reconstruction_output.wgsl` now derives luma noise from a compact mix of:
  fine hash noise,
  soft horizontal band correlation,
  and a per-line bias term
- luma contamination is brightness-dependent, so it stays more visible in darker and mid-tone regions and weakens in brighter regions
- chroma contamination is now broader and softer than luma contamination instead of sharing the same pixelwise character
- the chroma term also gains a small phase-like perturbation derived from the current chroma vector so it reads less like RGB grain
- the final pass lightly attenuates its general output-noise terms inside active dropout segments so dropout remains legible as localized signal loss
- the current four-pass architecture, crate boundaries, preview guardrails, and compact preview noise API stayed intact

## Why this integration path was chosen

- the current simplification lived in the reconstruction/output stage already, so that was the minimal place to refine it
- luma/chroma differentiation could be improved inside the existing final pass without introducing a new render stage or a new crate-level abstraction
- the formal model already separates luma noise and chroma noise conceptually, so the refinement makes the current implementation more faithful to that distinction without overclaiming video-accurate behavior

## Still-image v1 approximations used

- luma contamination is still deterministic hash-based noise, but it is now brightness-shaped and mildly line/band-correlated
- chroma contamination is still a compact still-image approximation; it is softened and made partly phase-like, but it is not a carrier-accurate chroma decoder model
- dropout remains a reconstruction-adjacent still-image approximation based on local line concealment rather than on temporal recovery or field history

## Verification completed for this stage

- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `shaders/passes/still_reconstruction_output.wgsl`
- `crates/casseted-signal/src/prototype.rs`
- `docs/math/`
- `docs/architecture/`
- `docs/agent-log/`
- `assets/reference-images/still-pipeline-v1/reconstruction-output.png`

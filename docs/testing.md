# Testing

The current repository keeps a deliberately small verification surface that matches the compact still-image v1 scope.

Current layers:

- unit tests for domain types and small utility crates
- GPU smoke tests for the still-image pipeline
- a CLI smoke test that exercises PNG input, pipeline execution, and PNG output
- shared helpers in `casseted-testing` for deterministic images, PNG loading, and image-difference stats
- stage-oriented regression tests in `casseted-pipeline` that verify:
  resolved uniforms/defaults on a deterministic in-memory reference card,
  bounded output changes under small parameter perturbations,
  and metadata coverage for the current stage/formulas mapping
- a small synthetic calibration set in `casseted-pipeline` that exercises the default still-image baseline on:
  colored edges / shapes,
  portrait-like midtones,
  bright highlights,
  neutral / low-saturation scenes,
  UI-like high-frequency detail,
  and a dark quiet-floor case used to sanity-check low-amplitude reconstruction activity
- corpus-backed sanity checks over `assets/reference-images/` that verify:
  the current bucket structure exists,
  the bucket PNGs load,
  the default pipeline produces non-identical output,
  and the compiled runtime path matches the direct GPU path on that corpus
- calibration metrics now also track quiet-region and dark-quiet-region deltas on the synthetic set so quiet-content refinements stay measurable without introducing a larger golden-image system
- the synthetic calibration set runs at the reference width, since below it the horizontal spatial terms are sub-pixel and the branch filters are close to inactive
- resolution-invariance checks in `casseted-pipeline` that render a fixed relative grating and a flat field at 720, 2160, and 3600 px wide, so horizontal bandwidth response and reconstruction contamination are both verified not to drift with frame width
- a matching vertical check that renders a flat field at 480, 1440, and 2400 px tall and asserts the dropout band count matches the reference-height count, so a per-line probability cannot fire more often just because the raster has more rows

A note on the edge-retention metric:
it is a ratio of mean *squared* horizontal steps. Total variation is conserved when a monotonic edge is blurred, so a mean-absolute ratio cannot observe bandwidth loss on step-like content and instead tracks whatever contamination the final pass adds. The squared form falls as an edge spreads and is largely insensitive to additive contamination, which is what these assertions are meant to measure.

The one place that insensitivity does not hold is content whose own gradient energy is very low, such as the dark quiet-floor case in chroma, where added contamination can outweigh the signal. Assertions should not lean on edge retention for that combination.

What is intentionally not present yet:

- large-scale golden-image review tooling
- image review tooling
- batch visual regression dashboards
- cross-platform rendering baselines

That keeps verification practical while still proving that the current end-to-end path works, that bucketized calibration references stay wired into engineering work, and that runtime reuse does not drift from the direct GPU path.

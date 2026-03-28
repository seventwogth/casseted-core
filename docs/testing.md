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

What is intentionally not present yet:

- large-scale golden-image review tooling
- image review tooling
- batch visual regression dashboards
- cross-platform rendering baselines

That keeps verification practical while still proving that the current end-to-end path works, that bucketized calibration references stay wired into engineering work, and that runtime reuse does not drift from the direct GPU path.

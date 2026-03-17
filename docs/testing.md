# Testing

The current repository uses a deliberately small testing strategy that matches the first milestone.

Current layers:

- unit tests for domain types and small utility crates
- GPU smoke tests for the still-image pipeline
- a CLI smoke test that exercises PNG input, pipeline execution, and PNG output
- shared helpers in `casseted-testing` for deterministic images, PNG fixtures, and tolerance-based image-difference checks
- committed visual regression fixtures for the current limited multi-pass still pipeline in `assets/reference-images/still-pipeline-v1/`
- stage-oriented regression tests in `casseted-pipeline` that verify:
  stage reference PNGs,
  resolved uniforms/defaults,
  and bounded output changes under small parameter perturbations

What is intentionally not present yet:

- large-scale golden-image review tooling
- image review tooling
- batch visual regression runs
- cross-platform rendering baselines

This keeps the test surface practical for early development while still proving that the current end-to-end path works.

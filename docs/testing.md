# Testing

The current repository uses a deliberately small testing strategy that matches the first milestone.

Current layers:

- unit tests for domain types and small utility crates
- GPU smoke tests for the still-image pipeline
- a CLI smoke test that exercises PNG input, pipeline execution, and PNG output
- shared helpers in `casseted-testing` for deterministic images and simple image-difference checks

What is intentionally not present yet:

- golden-image snapshot management
- image review tooling
- batch visual regression runs
- cross-platform rendering baselines

This keeps the test surface practical for early development while still proving that the current end-to-end path works.

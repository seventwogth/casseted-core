# Still Pipeline v1 References

This directory contains the committed visual regression fixtures for the current limited multi-pass still-image pipeline.

Files:

- `reference-card-96x64.png`: deterministic still-image input used by all current stage regression tests
- `input-conditioning-tone.png`: reference output for the input conditioning / tone shaping case
- `luma-chroma-transform.png`: neutral transform reference for the fused working `RGB -> YUV -> RGB` path
- `luma-degradation.png`: reference output for the luma degradation case
- `chroma-degradation.png`: reference output for the refined chroma degradation case with low-pass, coarse reconstruction, and restrained smear
- `reconstruction-output.png`: reference output for the reconstruction / output case

Current compare tolerance for the committed stage outputs:

- `max_changed_bytes = 1024`
- `max_mean_absolute_difference = 0.35`
- `max_absolute_difference = 3`

The current tests also run small parameter perturbations per stage-oriented case to ensure the limited multi-pass path remains responsive but bounded under small control changes.

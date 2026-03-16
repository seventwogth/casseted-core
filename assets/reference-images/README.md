# Reference Images

This directory stores committed PNG fixtures used for still-image visual regression and parameter-verification tests.

Current committed set:

- `still-pipeline-v1/reference-card-96x64.png`: deterministic source card used as the shared still-image input
- `still-pipeline-v1/*.png`: one reference output per current single-pass implementation stage

The current stage fixtures are documented in:

- `assets/reference-images/still-pipeline-v1/README.md`
- `docs/math/signal-model-v1-formulas.md`

To regenerate the committed stage PNGs:

```bash
cargo test -p casseted-pipeline bless_stage_reference_images -- --ignored
```

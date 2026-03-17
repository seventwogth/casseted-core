# Overview

The current `casseted-core` workspace is intentionally split into four layers:

1. Domain layer: `casseted-types` and `casseted-signal`
2. Asset/runtime layer: `casseted-shaderlib` and `casseted-gpu`
3. Composition layer: `casseted-pipeline`
4. Developer tooling layer: `casseted-cli` and `casseted-testing`

Current still-image data flow:

- CLI code reads a PNG into an `ImageFrame`
- pipeline code either accepts manual `SignalSettings` or projects a formal `VhsModel` into the current still-preview controls
- manual preview controls are softly normalized into effective preview ranges before stage resolution when they diverge from the model-projected path
- `casseted-pipeline` resolves those controls into five logical implementation stages:
  `input conditioning / tone shaping`, `luma/chroma transform`, `luma degradation`,
  `chroma degradation`, and `reconstruction / output`
- those stage-aligned controls are packed into one compact WGSL uniform block shared by the current still passes
- the runtime executes a limited four-pass chain:
  `still_input_conditioning`,
  `still_luma_degradation`,
  `still_chroma_degradation`,
  `still_reconstruction_output`
- three intermediate textures carry the working YUV signal, degraded luma, and degraded chroma between passes
- the processed image is copied back to CPU memory as an `ImageFrame`
- CLI code writes the result as PNG

The key point in the current phase is that the still-image path is now explicit at two levels:

- the canonical signal model in `casseted-signal` still defines the eight formal v1 stages
- the working GPU path groups them into five implementation stages and executes them as a compact four-pass runtime without a render graph

Why this degree of decomposition was chosen:

- it gives the still path a real branch point between luma and chroma
- it makes intermediate signals inspectable and easier to recalibrate
- it stays small enough to avoid graph planning, plugin hooks, or broad orchestration machinery

What remains intentionally fused:

- input interpretation, still-frame transport offsets, tone shaping, and `RGB -> YUV` fan-out share the first pass
- refined noise contamination, restrained still-image dropout handling, and decode reconstruction remain together in the final pass

Within that compact multi-pass path, the current visual calibration still intentionally favors tone shaping, luma softening, restrained highlight bleed, and chroma bandwidth loss over transport wobble. The chroma branch expresses bandwidth loss as horizontal low-pass filtering plus coarse chroma reconstruction and restrained bleed, while the luma branch adds a thresholded asymmetric highlight smear so bright edges spread as part of signal loss instead of as post-process bloom. The final pass now keeps luma noise brightness-shaped and mildly line/band-correlated, while chroma contamination stays broader and softer than luma. Jitter, crosstalk, refined noise, and mild line-segment dropout remain present, but they are kept subordinate so the result reads as analog signal degradation instead of glitch-like distortion.

The current verification foundation mirrors that structure:

- committed PNG fixtures live in `assets/reference-images/still-pipeline-v1/`
- `casseted-pipeline` runs stage-oriented reference tests against those fixtures with fixed tolerances
- `casseted-testing` provides the deterministic source card, PNG helpers, and image-difference assertions

Reference documents:

- [`signal-model-v1.md`](./signal-model-v1.md)
- [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md)

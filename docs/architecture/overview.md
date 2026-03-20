# Overview

The current `casseted-core` workspace is intentionally split into four layers:

1. Domain layer: `casseted-types` and `casseted-signal`
2. Asset/runtime layer: `casseted-shaderlib` and `casseted-gpu`
3. Composition layer: `casseted-pipeline`
4. Developer tooling layer: `casseted-cli` and `casseted-testing`

Current still-image data flow:

- CLI code reads a PNG into an `ImageFrame`
- pipeline code either accepts manual `SignalSettings` or projects a formal `VhsModel` into a private `preview_base_signal`
- model-backed preview edits now travel through explicit `SignalOverrides` instead of mutating the projected preview blob in place
- manual preview controls are softly normalized into effective preview ranges before stage resolution; on model-backed pipelines, untouched projected terms stay intact and only explicit overrides are normalized
- `casseted-pipeline` resolves those controls into five logical implementation stages:
  `input conditioning / tone shaping`, `luma/chroma transform`, `luma degradation`,
  `chroma degradation`, and `reconstruction / output`
- those stage-aligned controls are packed into one compact WGSL uniform block shared by the current still passes
- the runtime executes a limited four-pass chain:
  `still_input_conditioning`,
  `still_luma_degradation`,
  `still_chroma_degradation`,
  `still_reconstruction_output`
- a compact compiled runtime layer can now hold the reusable GPU execution objects for that fixed pass chain
- three intermediate textures carry the working YUV signal, degraded luma, and degraded chroma between passes
- the processed image is copied back to CPU memory as an `ImageFrame`
- CLI code writes the result as PNG

The key point in the current phase is that the still-image path is now explicit at two levels:

- the canonical signal model in `casseted-signal` still defines the eight formal v1 stages
- the working GPU path groups them into five implementation stages and executes them as a compact four-pass runtime without a render graph
- the compiled runtime layer owns reusable GPU execution state for that fixed pass chain, while `StillImagePipeline` remains only the high-level description of the still-image effect

Why this degree of decomposition was chosen:

- it gives the still path a real branch point between luma and chroma
- it makes intermediate signals inspectable and easier to recalibrate
- it stays small enough to avoid graph planning, plugin hooks, or broad orchestration machinery

What remains intentionally fused:

- input interpretation, still-frame transport offsets, tone shaping, and `RGB -> YUV` fan-out share the first pass
- dropout-conditioned reconstruction, refined contamination, and decode reconstruction remain together in the final pass
- the final pass only reuses the conditioned scan-line phase as a procedural seed for noise/dropout placement; it does not resample luma/chroma through transport a second time

What is now intentionally reused across repeated runs:

- `wgpu::RenderPipeline` objects for the four still passes
- the single-texture and dual-texture bind-group layouts
- the shared linear sampler
- the fixed compiled pass wiring represented by `StillPipelineRuntime`

What still remains per-run on purpose:

- input, working, luma, chroma, and output textures
- uniform and readback buffers
- bind groups that reference per-run textures and buffers

That split is deliberate for the current phase: it removes the obvious compile/setup churn without introducing a render graph, generic texture pool, or batch subsystem before the still-image path actually needs them.

Toolchain note:

- the canonical workspace toolchain is now pinned to Rust `1.88.0` in `rust-toolchain.toml`
- workspace verification commands are expected to run with `Cargo.lock` via `--locked`

Within that compact multi-pass path, the current visual calibration still intentionally favors tone shaping, luma softness, restrained highlight bleed, and chroma bandwidth loss over transport wobble. The chroma branch now expresses bandwidth loss as horizontal low-pass filtering plus cell-integrated coarse chroma reconstruction and a restrained trailing contamination tail, with extra smear held back on strong luma edges so color stays subordinate to structure. The luma branch continues to use a broader low-pass foundation with separate fine-detail and mid-band attenuation so the image loses crisp digital sharpness without collapsing into plain blur. Bright contours also pick up a small asymmetric lag/bleed bias from preceding samples, so they spread as part of signal loss instead of as post-process bloom. The final pass now makes its own sequence explicit too: it first conditions the branch outputs through restrained dropout concealment, then applies `Y/C`-space contamination, then decodes through a small residual leakage term. That keeps luma contamination brightness-shaped and mildly line/band-correlated, keeps chroma contamination broader and softer than luma, and stops stronger dropout concealment from immediately reading as freshly re-colored leakage. Jitter, crosstalk, refined contamination, and mild line-segment dropout remain present, but they are kept subordinate so the result reads as analog signal degradation instead of glitch-like distortion.

The current verification foundation mirrors that structure:

- committed PNG fixtures live in `assets/reference-images/still-pipeline-v1/`
- `casseted-pipeline` runs stage-oriented reference tests against those fixtures with fixed tolerances
- `casseted-testing` provides the deterministic source card, PNG helpers, and image-difference assertions

Reference documents:

- [`signal-model-v1.md`](./signal-model-v1.md)
- [`../math/signal-model-v1-formulas.md`](../math/signal-model-v1-formulas.md)

# Still-Image Pipeline

The first real pipeline in `casseted-core` is intentionally a single still-image GPU pass.

Current shape:

- input: one `ImageFrame` in `RGBA8`
- execution: one fullscreen render pass in `wgpu`
- shader: `shaders/passes/still_analog.wgsl`
- output: one processed `ImageFrame` read back to CPU memory

The shader applies a small analog-inspired degradation by combining:

- horizontal luma softening
- chroma offset and bleed
- very soft deterministic noise
- line-based horizontal instability

Why it is kept this small:

- it proves the end-to-end path from CPU pixels to GPU and back
- it already consumes real domain parameters from `casseted-signal`
- it avoids introducing a pass graph or intermediate texture orchestration too early
- it now acts as a preview-oriented subset of the formal VHS / analog v1 model rather than pretending to be the full model

Deferred on purpose:

- multi-pass processing
- explicit Y/C separation passes
- formal `VhsModel` projection into pipeline stages
- video/frame-sequence support
- file I/O and image codecs
- optimized resource reuse and cached pipeline objects

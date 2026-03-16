# Crate Notes

## `casseted-types`

`casseted-types` still contains only the dependency-light frame and image types shared across the workspace:

- `FrameSize`
- `PixelFormat`
- `FrameDescriptor`
- `ImageFrame`

No signal-specific types were moved here in the current phase.

## `casseted-signal`

`casseted-signal` now owns both:

- the formal VHS / analog v1 domain model in `VhsModel`
- the compact still-preview control layer in `SignalSettings`

The formal layer is grouped by signal responsibility:

- input assumptions
- tone shaping
- luma path
- chroma path
- transport instability
- noise and corruption
- decode / reconstruction

The compact layer is deliberately smaller and only covers the currently fused still-pass controls.
Those compact controls now map more explicitly onto the five implementation stages used by the still pipeline, without becoming a second full domain model.

## `casseted-shaderlib`

`casseted-shaderlib` keeps repository WGSL assets embedded and addressable by stable shader identifiers.

## `casseted-gpu`

`casseted-gpu` remains the thin `wgpu` runtime layer. It still knows nothing about the shader registry or signal model.

## `casseted-pipeline`

`casseted-pipeline` remains the orchestration layer that bridges:

- `ImageFrame`
- `SignalSettings` / `VhsModel`
- embedded shader assets from `casseted-shaderlib`
- `wgpu` execution from `casseted-gpu`

Important change in the current phase:
this crate now contains a narrow projection from the formal signal model into five explicit implementation stages for the current still path, and then packs those stages into one fused WGSL pass. It still does not become a graph engine.

## `casseted-cli`

`casseted-cli` remains a developer utility for running one PNG image through the current still-image pipeline. It still does not own domain logic beyond simple flag-to-setting overrides.

## `casseted-testing`

`casseted-testing` remains a small helper crate for deterministic test images and basic frame assertions.

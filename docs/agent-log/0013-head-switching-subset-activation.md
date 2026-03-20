# 0013: Head-Switching Subset Activation

Date: 2026-03-20

## Summary

Activated the formal `VhsTransportSettings.head_switching_*` subset inside the existing still-image runtime as a restrained lower-frame transport-side approximation.

The implementation stays intentionally compact:

- no temporal or video model
- no new pass architecture
- no wider preview control surface
- no new crate-level abstractions

## Why this milestone was justified

After the subset review and the chroma-phase activation milestone, the strongest remaining transport-side gap between the formal model and the active still-image runtime was `head_switching_*`.

Those fields were already present formally, but the runtime still treated them as documented-only. That made the subset map less honest than it needed to be, especially now that the rest of the still-image signal chain is already stable.

## Chosen approximation

The runtime now treats `head_switching_*` as a restrained lower-band switching disturbance:

- `head_switching_band_lines` localizes a bottom-band region
- `head_switching_offset_us` becomes a bounded horizontal offset proxy inside that band
- the final pass partially mixes toward a horizontally shifted reconstruction signal inside that region
- chroma support is reduced more than luma support there
- a very small seam-localized luma disturbance is added near the top of the switching band

This is intentionally not:

- field timing simulation
- a deck-accurate head-switching model
- screen tearing
- a full-width glitch bar
- dropout reuse under a different name

## Runtime boundary

The activation follows the same pattern already used for chroma-phase terms:

- the preview projection layer still ignores `head_switching_*`
- the manual preview path keeps them neutral
- the formal model path resolves them as model-only auxiliaries during stage packing
- the compact uniform layout is reused instead of widened

Concretely:

- the shared frame block now stores the resolved switching-band line count in `effect.frame.z`
- `effect.reconstruction_output.w` stores the bounded switching offset proxy
- `still_reconstruction_output.wgsl` applies the switching approximation before dropout conditioning

## Classification outcome

`head_switching_band_lines` and `head_switching_offset_us` are now:

- `Partially Active / Approximated`

They are not `Fully Active` because the runtime still uses a still-image bottom-band seam/disturbance approximation rather than a timing-accurate switching model.

## Verification

Added compact activation tests that verify:

- preview projection still ignores `head_switching_*`
- resolved runtime stage state changes when `head_switching_*` changes
- GPU output changes when the terms are enabled

The stage-regression defaults remain stable because the committed reconstruction reference fixture still uses a neutralized model with head switching disabled.

# 0018 Quiet-Content Reconstruction Calibration

Date: 2026-03-28

Stage:
reference-driven refinement of final reconstruction/output behavior for calm still-image content inside the existing limited multi-pass architecture

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The baseline calibration pass had already established that the strongest parts of the still-image chain were no longer the main problem:

- tone shaping and luma degradation were coherent
- chroma bandwidth loss and reconstruction were mature enough to trust
- the final stage was already restrained enough not to dominate the image

The main remaining quality gap was narrower:

- quiet scenes could still look a little too clean or too proxy-like
- calm surfaces often lost obvious digital crispness, but still did not quite pick up a believable analog carrier
- the issue was not “we need more artifacts”
- the issue was that low-amplitude reconstruction character was still too conservative between stronger artifacts

## Primary reference buckets for this milestone

This stage was explicitly reference-driven.

Primary engineering buckets:

- `04_portrait-skin`
- `05_ui-text-detail`
- `06_neutral-interior`
- `08_dark-screen-noise`

Secondary cross-check buckets:

- `01_target-look`
- `02_highlights-specular`
- `03_color-edges-chroma`
- `07_silhouette-low-detail`

The primary buckets were used to judge quiet-content character directly.
The secondary buckets were used to confirm that the refinement stayed subordinate to the current visual hierarchy and did not turn into a new effect family.

## Baseline weakness by primary bucket

### `04_portrait-skin`

- portrait structure and saturation restraint were already directionally good
- the remaining weakness was that skin-like midtones still felt a bit too “resolved” between stronger luma/chroma degradations
- subtle analog dirt was present only weakly, so portraits could read softened but still slightly sterile

### `05_ui-text-detail`

- edge softness and luma hierarchy were already reasonable
- the quieter light and dark fields between text strokes could still feel too uniform
- that made some UI/text outputs feel softened yet still slightly digitally composed instead of quietly analog-reconstructed

### `06_neutral-interior`

- this bucket showed the clearest quiet-content gap
- ordinary walls, desks, and low-drama surfaces were softened and somewhat chroma-limited, but the reconstruction floor still felt too clean between visible artifacts
- the result was not wrong in a loud way; it was conservative enough to feel a little proxy-like

### `08_dark-screen-noise`

- dark-floor behavior was restrained and credible, which was good
- but the low-amplitude contamination floor could still feel too inactive unless a stronger highlight, edge, or preexisting noise structure carried the scene
- the gap was not “needs more dropout” or “needs louder noise”; it was a lack of subtle analog presence in non-eventful dark regions

## Root cause inside reconstruction/output

The final-stage weakness was traced to the current contamination behavior:

- luma contamination was brightness-shaped, but not sufficiently biased toward locally calm low-detail surfaces
- chroma contamination was already soft and subordinate, but quiet surfaces still lacked enough signal-carried luma-side presence
- dropout/head-switching interaction was already reasonably restrained and was not the right lever
- the final stage was avoiding overt instability successfully, but it was also avoiding some of the low-level imperfectness that makes calm analog content feel “hosted” by a medium

In short:
the output was not too dirty.
It was too careful in the low-amplitude reconstruction regime.

## What changed

The milestone stayed intentionally small and local to `still_reconstruction_output.wgsl`.

Implemented refinement:

- added a compact local quiet-region profile inside the final pass using immediate `Y/C` neighborhood differences
- used that profile to bias luma contamination away from fine per-pixel grain and toward slower line/band-carried low-amplitude structure on calm surfaces
- gave dark quiet regions a slightly stronger luma-side carrier without turning chroma into a tinty dark-noise showcase
- kept chroma contamination softer and smaller than luma contamination
- kept chroma phase noise restrained and only modestly boosted in the same quiet regime
- left dropout, head switching, compiled runtime packing, preview guardrails, crate boundaries, and public parameter surface unchanged

What did **not** change:

- no new pass
- no new artifact family
- no wider preview API
- no crate-level abstraction changes
- no output-transfer activation
- no temporal/video logic

## Expected improvement by bucket

Most visible expected gains:

- `06_neutral-interior`:
  calmer walls, desks, and ordinary surfaces should feel less sterile and less like a softened digital proxy
- `08_dark-screen-noise`:
  dark quiet floors should carry a more believable low-amplitude analog presence without turning into a grain overlay
- `04_portrait-skin`:
  skin-like midtones should keep current restraint while feeling slightly more lived-in between stronger degradations
- `05_ui-text-detail`:
  quiet fields around text and UI shapes should feel less empty while luma edges remain structurally ahead of chroma breakup

## Minimal verification added

- extended the synthetic calibration layer with a `dark-quiet-floor` case
- added quiet-region and dark-quiet-region delta metrics to the calibration test path
- kept the compiled-runtime parity check on the calibration set

This verification remains intentionally small:
it gives the quiet-content calibration an engineering foothold without introducing a bigger image-review framework.

## Remaining still-image subset limits

This milestone does not change the existing subset boundaries:

- `VhsDecodeSettings.output_transfer` is still deferred
- `VhsInputSettings.transfer` and `VhsInputSettings.temporal_sampling` are still deferred runtime selectors
- the final stage is still a still-image approximation, not a temporal transport or carrier-accurate decode model
- the new quiet-region profile is a compact reconstruction heuristic, not a new physical medium simulation layer

## Recommended next milestone

The next highest-value step is now narrower than this one:

- a small chroma-vs-luma calibration pass for very small colored high-frequency accents

Primary buckets for that next step should be:

- `03_color-edges-chroma`
- `05_ui-text-detail`

That follows naturally from the current state:
quiet-content character is no longer the broadest practical gap, while tiny colored accents can still leave chroma slightly too intact relative to softened luma structure.

## Verification completed for this stage

- `cargo test -p casseted-pipeline calibration -- --nocapture`
- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `shaders/passes/still_reconstruction_output.wgsl`
- `crates/casseted-pipeline/src/calibration.rs`
- `assets/reference-images/README.md`
- `docs/math/signal-model-v1-formulas.md`
- `docs/architecture/signal-model-v1.md`
- `docs/architecture/overview.md`
- `docs/testing.md`

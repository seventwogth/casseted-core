# 0017 Still-Image Baseline Calibration Pass

Date: 2026-03-28

Stage:
whole-chain still-image baseline calibration review of the existing compact signal-model-aligned still pipeline

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

The repository had already closed the largest architecture and subset-activation ambiguities:

- the limited multi-pass architecture was stable
- the compiled runtime reuse layer was in place
- the base still-image signal chain, deep luma/chroma refinement, final-stage cleanup, chroma phase activation, and head-switching status work had already landed
- input-side and output-side boundary reviews had already clarified what is still deferred

That made this the right time to stop asking "what can be activated next?" and instead ask:

- how coherent is the current still-image baseline as one system?
- which stages are already mature enough to trust?
- which parts now limit perceived quality the most?
- where is the next highest-value gain without reopening the architecture?

## What was reviewed

- current still-image chain end to end:
  input conditioning,
  luma degradation,
  chroma degradation,
  highlight bleed,
  dropout/noise/reconstruction,
  output/decode,
  preview guardrails,
  and the compiled runtime path
- current architecture, subset, and formulas docs
- committed stage-regression fixtures in `assets/reference-images/still-pipeline-v1/`
- existing stage/defaults/perturbation tests
- a new small synthetic calibration set covering:
  colored edges / shapes,
  portrait-like midtones,
  bright highlights,
  neutral / low-saturation scenes,
  and UI-like high-frequency detail

## What became clearer

### Strongest parts of the current baseline

- Input conditioning plus luma degradation now form the strongest part of the chain.
  The output reliably loses digital crispness through tone shoulder, luma softness, and restrained highlight spread before secondary artifacts start reading loudly.
- The chroma branch is mature enough that colored-shape scenes read as bandwidth loss and coarse chroma support rather than as decorative RGB splitting.
- The final reconstruction stage is no longer the stage that dominates perceived character.
  It contributes restrained contamination, dropout, and lower-band transport disturbance without pulling the image away from the luma/chroma foundation.
- The compiled runtime layer looks trustworthy for the current still-image scope.
  The new calibration tests show that runtime reuse stays output-identical to the direct GPU path on the representative still-image cases.

### Weakest parts of the current baseline

- Quiet low-saturation scenes still expose the cleanest remaining gap.
  They mostly read through chroma softening and mild luma loss, but the default reconstruction-side character stays restrained enough that some results still feel a little too clean or proxy-like.
- Portrait / skin-like midtones are directionally good, but they still share some of that quiet-scene cleanliness.
  The output is softer and less digital, but not yet especially rich in low-amplitude analog dirt.
- High-frequency UI/text-like detail is now the clearest structural weak class.
  The luma path softens those edges effectively, but very small colored accents can still leave chroma looking slightly too intact relative to the softened luma base.

### Systematic balance

The review did not find a runaway stage.

The current imbalance is mostly subtractive:

- not that transport, dropout, or final-stage contamination are too strong
- but that the default low-amplitude reconstruction character is still conservative on quiet content

That is an important distinction because it means the next gain should be a narrow calibration milestone, not a new effect wave.

## Small repository changes made during this pass

- added a compact synthetic calibration test layer in `crates/casseted-pipeline/src/calibration.rs`
- added representative baseline checks for:
  colored edges,
  portrait-like midtones,
  bright highlights,
  neutral / low-saturation scenes,
  and UI-like detail
- added a parity check proving that `StillPipelineRuntime` matches the direct GPU path on those same cases
- updated README / architecture / formulas / testing docs so the current baseline assessment is explicit instead of implied

No stage defaults or shader coefficients were changed during this pass.

That was intentional:

- the review found useful calibration observations
- but not one obviously safe global default tweak that justified changing the committed stage references at this point

## Current baseline assessment

- current baseline quality: coherent and mature for a compact still-image v1 foundation
- strongest stages: input conditioning, luma degradation, chroma bandwidth-loss/reconstruction
- weakest stages: quiet-content reconstruction character and the chroma-vs-luma balance on very small high-frequency colored detail
- final-stage status: restrained and no longer overbearing
- architecture status: still good and not in need of restructuring

## Recommended next milestone

Primary recommendation:
one compact reconstruction-side calibration milestone for quiet content.

Scope:

- keep the same architecture
- keep the same public surface
- do not add a new effect family
- recalibrate low-amplitude reconstruction character for neutral surfaces, skin-like midtones, and UI/text detail

Why this is the highest-value next move:

- it targets the classes that still most often read too clean
- it improves perceived analog character where the current chain is already structurally sound
- it does not require reopening the luma/chroma foundation or the runtime architecture

Reserve recommendation:
if the primary milestone is postponed, the next-best targeted gain is a narrow chroma-vs-luma calibration pass for very small colored high-frequency accents.

Why it is secondary:

- it is real, but narrower than the quiet-scene character gap
- the current colored-shape and highlight classes already behave reasonably well overall
- the broader practical gain still lies in making quiet still outputs feel less proxy-like

## Verification completed for this stage

- `cargo test -p casseted-pipeline calibration -- --nocapture`
- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `docs/architecture/`
- `docs/math/`
- `docs/testing.md`
- `docs/agent-log/`

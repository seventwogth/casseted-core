# Reference Images

This directory is the still-image visual calibration corpus for the current `casseted-core` baseline.

It is used for:

- whole-chain baseline review of the current still-image signal path
- qualitative comparison against real-world VHS-like reference behavior
- documentation and agent notes about current strengths, weaknesses, and next milestones
- lightweight corpus-backed sanity checks that the default pipeline still renders and that the compiled runtime stays aligned with the direct GPU path

Important distinction:

- `01_target-look/` contains holistic target-look references
- `02_` through `08_` are narrower calibration buckets

These images are not clean-source / golden-output pairs for one-to-one image matching.
They are reference targets that help us judge whether the current still-image chain is converging on the right visual hierarchy and failure modes.

Stage-level regression in code stays synthetic and deterministic through the in-memory reference card used by `casseted-pipeline`.
This directory is the real-world look corpus.

Current note:
the quiet-content reconstruction calibration milestone is explicitly reference-driven.
Its primary engineering buckets are `04_portrait-skin`, `05_ui-text-detail`, `06_neutral-interior`, and `08_dark-screen-noise`; the other buckets remain cross-checks for hierarchy and regression restraint.

## Current Buckets

| Bucket | Purpose | Primary visual aspects | Most relevant stages |
| --- | --- | --- | --- |
| `01_target-look` | Holistic target references for the current still-image baseline | overall analog character, visual hierarchy, cross-stage balance, whether the image feels signal-first instead of effect-first | full chain |
| `02_highlights-specular` | Bright highlight and specular references | highlight shoulder, bright-edge spread, whether highlight response stays luma-led, whether final contamination stays subordinate | input conditioning, luma degradation, highlight bleed, reconstruction/output |
| `03_color-edges-chroma` | Chroma-heavy edges, colored shapes, and UI-adjacent color boundaries | chroma bandwidth loss, chroma-vs-luma hierarchy, misregistration restraint, whether color breakup reads like bandwidth loss instead of RGB-split decoration | chroma degradation, luma degradation, reconstruction/output |
| `04_portrait-skin` | Portrait and skin-like midtone references | midtone cohesion, skin saturation restraint, whether portrait structure stays ahead of chroma breakup, whether quiet analog character is present without looking dirty-for-its-own-sake | input conditioning, luma degradation, chroma degradation, reconstruction/output |
| `05_ui-text-detail` | Text, diagrams, and UI-like high-frequency detail | luma edge retention, text softness, small colored-accent handling, whether the result stays analog-soft instead of mushy or too digitally clean | luma degradation, chroma degradation, preview guardrails, reconstruction/output |
| `06_neutral-interior` | Everyday low-drama interior scenes and ordinary objects | neutral-surface character, low-saturation behavior, baseline contamination/noise restraint, whether quiet scenes still feel analog rather than proxy-like | luma degradation, noise, reconstruction/output |
| `07_silhouette-low-detail` | Large shapes, silhouettes, and low-detail gradients | gradient smoothness, silhouette readability, whether secondary artifacts stay subordinate to tone/luma foundation | input conditioning, luma degradation, reconstruction/output |
| `08_dark-screen-noise` | Dark scenes and low-light displays | dark-scene noise floor, chroma noise visibility, dropout/head-switching restraint, whether dark content stays credible instead of collapsing into a generic overlay | noise, dropout, reconstruction/output |

## How The Current Baseline Review Uses The Buckets

- Overall target-look sanity: `01_target-look`
- Bright highlights / speculars: `02_highlights-specular`
- Colored edges / colored shapes: `03_color-edges-chroma`
- Portrait / skin-like midtones: `04_portrait-skin`, with `02_highlights-specular/portrait.png` as a bright-portrait cross-check
- High-frequency detail / text / UI-like edges: `05_ui-text-detail`, with `03_color-edges-chroma/interface.png` as the colored-edge cross-check
- Neutral interior / ordinary objects: `06_neutral-interior`, with `01_target-look/wall.png` as a lower-drama holistic check
- Low-detail silhouettes / large gradients: `07_silhouette-low-detail`
- Dark scenes / low-light noise visibility: `08_dark-screen-noise`
- Quiet-content reconstruction calibration:
  use `04_portrait-skin`, `05_ui-text-detail`, `06_neutral-interior`, and `08_dark-screen-noise` as the primary buckets when judging whether calm surfaces pick up plausible low-amplitude analog character without turning into a dirt effect

That split is intentional:

- `01_target-look` tells us whether the chain reads coherently as a system
- `02` through `08` tell us which content classes are currently strongest or weakest

## Practical Guidance

- Use `01_target-look` first when judging whether a change improves or hurts the baseline as a whole.
- Use the narrower buckets next when the question is class-specific, such as highlights, text detail, or quiet dark scenes.
- For quiet-content work, write down which of `04`, `05`, `06`, and `08` were primary and which of `01`, `02`, `03`, or `07` were only cross-checks.
- Prefer documenting which buckets informed a conclusion instead of writing generic visual opinions.
- If a future change adds or removes bucket directories, update this README in the same milestone.

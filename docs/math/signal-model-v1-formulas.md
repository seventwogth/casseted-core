# Signal Model v1 Formulas

This document is the engineering reference for the subset of signal-model v1 that is currently implemented in the still-image pipeline and for the immediately adjacent implementation path.

It is intentionally narrower than a full VHS deck model. The goal is to define the exact discrete approximations that the repository currently uses for:

- input conditioning and tone shaping
- BT.601-like luma/chroma working decomposition
- luma-oriented horizontal bandwidth loss
- one controllable chroma degradation path
- reconstruction back to RGB with the current signal-shaped noise subset

The current GPU implementation lives in:

- `crates/casseted-pipeline/src/projection.rs`
- `crates/casseted-pipeline/src/stages.rs`
- `crates/casseted-pipeline/src/runtime.rs`
- `crates/casseted-pipeline/src/state.rs`
- `shaders/passes/still_input_conditioning.wgsl`
- `shaders/passes/still_luma_degradation.wgsl`
- `shaders/passes/still_chroma_degradation.wgsl`
- `shaders/passes/still_reconstruction_output.wgsl`

For the field-level split between active, approximated, and deferred formal terms, use [`../architecture/signal-model-v1-subset.md`](../architecture/signal-model-v1-subset.md) together with this formulas reference.

## 1. Scope

The implemented still-image subset is:

1. input conditioning / tone shaping: gamma-coded `sRGB` input assumptions, still-frame transport offset, and luma-preserving soft-knee highlight compression
2. `RGB -> YUV` decomposition
3. luma low-pass/detail attenuation plus restrained highlight bleed
4. chroma horizontal delay + band-limited, cell-integrated chroma reconstruction + optional vertical line blend
5. `YUV -> RGB` reconstruction with a small Y/C leakage term, brightness-shaped luma contamination, softer chroma contamination, and restrained still-image dropout handling

The current implementation keeps those stages in a compact four-pass runtime and names them explicitly in code through:

- `resolve_still_stages()` in `casseted-pipeline`
- stage-aligned uniform groups in `EffectUniforms`
- stage helper functions across the four still-image WGSL passes

### Current Implementation Stage Layout

| Physical pass | Logical implementation stages covered | Current code / shader entry points | Current pass boundary |
| --- | --- | --- | --- |
| `still_input_conditioning` | input conditioning / tone shaping + luma/chroma transform | `resolve_input_conditioning_stage()`, `conditioned_sample_uv()`, `apply_tone_shaping()`, `rgb_to_yuv()` | one working-signal pass |
| `still_luma_degradation` | luma degradation | `resolve_luma_degradation_stage()`, `degrade_luma()`, `highlight_bleed()` | one luma branch pass |
| `still_chroma_degradation` | chroma degradation | `resolve_chroma_degradation_stage()`, `degrade_chroma()` | one chroma branch pass |
| `still_reconstruction_output` | reconstruction / output | `resolve_reconstruction_output_stage()`, `apply_dropout_approximation()`, `sample_reconstruction_contamination()`, `compose_display_yuv()`, `decode_output_rgb()` | one final output pass |

### Current Visual Regression Fixtures

Committed fixtures now live in `assets/reference-images/still-pipeline-v1/`.

| Stage case | Reference PNG | Formulas section | Primary uniform focus | Default resolved values used by the fixture |
| --- | --- | --- | --- | --- |
| Input conditioning / tone shaping | `input-conditioning-tone.png` | `4.1`, `5.1` | `effect.input_conditioning` | `k_h = 0.64`, `rho_h = 0.62`, `p_J = 0.35 * s_ref`, `delta_V = 0.25` |
| Luma/chroma transform | `luma-chroma-transform.png` | `4.2` | none beyond the shared frame block; this is the neutral transform fixture for the fused `RGB -> YUV -> RGB` working path | neutral controls via `StillImagePipeline::new(SignalSettings::neutral())` |
| Luma degradation | `luma-degradation.png` | `4.3` | `effect.luma_degradation` | `r_Y = 1.92 * s_ref`, `alpha_p = 0.045`, `theta_H = 0.96`, `beta_H = 0.0363`; the shader derives low/mid/fine-band luma attenuation from that compact state |
| Chroma degradation | `chroma-degradation.png` | `4.4` | `effect.chroma_degradation` | `r_tau = 0.432 * s_ref`, `r_C = 2.333 * s_ref`, `g_C = 0.94`, `beta_V = 0.35` |
| Reconstruction / output | `reconstruction-output.png` | `4.5`, `5.2`, `5.3` | `effect.reconstruction_output`, `effect.reconstruction_aux` | `a_Y = 0.018`, `a_C = 0.0077`, `epsilon_YC = 0.04`, `q_D = 0.06`, `s_D = 3.24`, `f = 0` |

Current committed output tolerance for those PNG comparisons:

- `max_changed_bytes = 1024`
- `max_mean_absolute_difference = 0.35`
- `max_absolute_difference = 3`

Those tolerances are intentionally small enough to catch behavioral regressions while still allowing minor backend-level float differences in the compact multi-pass path.

## 2. Notation And Variables

### Coordinates and frame geometry

| Symbol | Meaning | Range / units |
| --- | --- | --- |
| \(x, y\) | pixel coordinates in image space | \(x \in [0, W - 1]\), \(y \in [0, H - 1]\) |
| \(u, v\) | normalized texture coordinates | \([0, 1]\) |
| \(W, H\) | frame width and height | pixels |
| \(\ell\) | scan-line index used for line-wise effects | \([0, H - 1]\) |
| \(f\) | frame index from `FrameDescriptor.frame_index` | integer |
| \(s_{\text{ref}}\) | reference-width scale | \(W / 720\) |

### Color and working signal quantities

| Symbol | Meaning | Range |
| --- | --- | --- |
| \(R, G, B\) | input gamma-coded color channels | \([0, 1]\) |
| \(Y\) | luma-like working component | approximately \([0, 1]\) |
| \(U, V\) | BT.601-like chroma-difference components | centered around 0 |
| \(C = (U, V)\) | chroma vector | 2-vector |

### Model and approximation parameters

| Symbol | Meaning | Code mapping |
| --- | --- | --- |
| \(k_h\) | highlight soft-knee threshold | `VhsToneSettings.highlight_soft_knee` |
| \(\rho_h\) | highlight compression strength | `VhsToneSettings.highlight_compression` |
| \(b_Y\) | luma bandwidth proxy | `VhsLumaSettings.bandwidth_mhz` |
| \(\alpha_p\) | pre/de-emphasis approximation gain | derived from `VhsLumaSettings.preemphasis_db` |
| \(\tau_C\) | chroma delay relative to luma | `VhsChromaSettings.delay_us` |
| \(b_C\) | chroma bandwidth proxy | `VhsChromaSettings.bandwidth_khz` |
| \(g_C\) | chroma saturation gain | `VhsChromaSettings.saturation_gain` |
| \(\beta_V\) | vertical chroma blend | `VhsDecodeSettings.chroma_vertical_blend` |
| \(\epsilon_{YC}\) | residual Y/C leakage | `VhsDecodeSettings.luma_chroma_crosstalk` |
| \(\tau_J\) | formal line-jitter amplitude | `VhsTransportSettings.line_jitter_us` |
| \(\delta_V\) | still-frame vertical offset snapshot | `SignalSettings.tracking.vertical_offset_lines` |
| \(a_Y\) | luma noise amplitude | `SignalSettings.noise.luma_amount` |
| \(a_C\) | chroma noise amplitude | `SignalSettings.noise.chroma_amount` |
| \(p_J\) | line-jitter amplitude in reference pixels | `SignalSettings.tracking.line_jitter_px` |
| \(q_D\) | dropout probability per line | `VhsNoiseSettings.dropout_probability_per_line` |
| \(\tau_D\) | mean dropout span in microseconds | `VhsNoiseSettings.dropout_mean_span_us` |

### Discrete terms used by the still shader

| Symbol | Meaning | Current source |
| --- | --- | --- |
| \(r_Y\) | resolved luma bandwidth-loss proxy in pixels | `SignalSettings.luma.blur_px * s_ref` |
| \(\Delta_Y\) | derived luma sample step in pixels | `max(0.5, 0.55 * r_Y + 0.45)` |
| \(\eta_Y\) | derived luma bandwidth mix | `r_Y / (r_Y + 1.35)` |
| \(\theta_H\) | resolved highlight-bleed threshold | `clamp(k_h + 0.12, 0.72, 0.96)` |
| \(\beta_H\) | resolved highlight-bleed amount | `min(0.16, (p_Y / (p_Y + 1.25)) * (0.06 + 0.14 * rho_h / (rho_h + 1)))` |
| \(r_\tau\) | resolved chroma delay in pixels | `SignalSettings.chroma.offset_px * s_ref` |
| \(r_C\) | resolved chroma bandwidth-loss proxy in pixels | `SignalSettings.chroma.bleed_px * s_ref` |
| \(\eta_C\) | derived chroma bandwidth mix | `r_C / (r_C + 1.0)` |
| \(r_L\) | derived chroma low-pass span | `0.40 + 0.72 * r_C + 0.28 * eta_C` for the bandwidth-loss branch |
| \(d_C\) | derived chroma reconstruction cell width | `1.0 + 0.52 * r_C + 0.38 * eta_C` |
| \(\eta_\tau\) | derived delay/bandwidth balance mix for smear | `abs(r_tau) / (abs(r_tau) + 0.5 * r_C + 0.35)` |
| \(s_C\) | derived chroma cell-integration step | `max(0.35, 0.24 * d_C)` |
| \(\alpha_S\) | restrained trailing-smear mix | `clamp(0.08 + 0.14 * eta_C + 0.05 * eta_tau, 0, 0.27)` |
| \(\gamma_Y\) | local luma-edge guard used to keep chroma subordinate | `clamp(2.8 * max(abs(Y_0 - Y_-), abs(Y_0 - Y_+)), 0, 1)` |
| \(\beta_N\) | derived chroma vertical-neighbor weight | `0.18 + 0.06 * eta_C` |
| \(s_D\) | resolved dropout mean span in pixels | `min(48 * s_ref, 13.5 * tau_D * s_ref)` |

### Current range rules used by stage verification

| Uniform term | Current rule in the still-image implementation |
| --- | --- |
| `effect.input_conditioning.x` | `highlight_soft_knee` clamped to `[0, 0.999]` |
| `effect.input_conditioning.y` | `highlight_compression >= 0` |
| `effect.input_conditioning.z` | model-projected line jitter is non-negative and intentionally attenuated in the still path; manual preview values are further soft-capped into an effective range |
| `effect.input_conditioning.w` | vertical offset snapshot is signed; manual preview values are soft-capped into an effective range |
| `effect.luma_degradation.x` | resolved luma bandwidth-loss proxy `>= 0`; manual preview values are softly capped at high magnitudes |
| `effect.luma_degradation.y` | detail recovery mix derived from pre-emphasis and clamped to `[0, 0.12]` |
| `effect.luma_degradation.z` | derived highlight-bleed threshold, clamped to `[0.72, 0.96]` |
| `effect.luma_degradation.w` | derived highlight-bleed amount, clamped to `[0, 0.16]` |
| `effect.chroma_degradation.x` | current model projection preserves the sign of `VhsChromaSettings.delay_us` and remains intentionally attenuated relative to blur; manual preview values use a signed soft cap |
| `effect.chroma_degradation.y` | resolved bandwidth-loss proxy `>= 0`; manual preview values are softly capped and also floored against the effective chroma offset |
| `effect.chroma_degradation.z` | saturation gain `>= 0` |
| `effect.chroma_degradation.w` | vertical blend clamped to `[0, 1]` |
| `effect.reconstruction_output.xy` | reconstruction-contamination amplitudes `>= 0`; manual preview values are soft-capped into restrained output ranges before the final pass reshapes them into brightness-dependent luma contamination and softer chroma contamination |
| `effect.reconstruction_output.z` | Y/C crosstalk clamped to `[0, 1]` |
| `effect.frame.w` | shared frame / procedural seed from `FrameDescriptor.frame_index` |
| `effect.reconstruction_aux.x` | model-driven dropout line probability clamped to `[0, 0.08]`; manual preview path keeps it at `0` |
| `effect.reconstruction_aux.y` | model-driven dropout span proxy in pixels clamped to `[0, 48 * s_ref]`; manual preview path keeps it at `0` |

Current packing note:
the compact uniform block now uses `effect.frame = (W, H, 1 / W, f)`. The shaders derive `1 / H` from `H` so the frame index stays in the shared frame block instead of leaking into an output-stage-specific slot.

## 3. Input And Working Representation

### Input interpretation

The current pipeline treats the input image as already-decoded, gamma-coded `sRGB` data in \([0, 1]\). There is no linear-light conversion at this stage.

Purpose:
stabilize a known entry assumption for the still-image MVP without modeling a full camera or decoder chain.

Mathematical meaning:
all further processing works on gamma-coded values, not on scene-linear radiance.

Visual effect:
none by itself.

Signal motivation:
moderate. Real analog pipelines are not scene-linear at the visible artifact level, and the current goal is a controllable playback-look approximation rather than a full imaging pipeline.

Engineering approximation:
`sRGB` input is accepted directly and converted to a BT.601-like working representation in shader code.

Activation note:
the stage is active under the fixed `sRGB` + BT.601-like + progressive still-frame assumptions above, but changing `VhsInputSettings` does not yet change the runtime path.

Pipeline mapping:
executed in `shaders/passes/still_input_conditioning.wgsl`, which writes the working YUV texture used by the later luma/chroma passes.

### Working representation

The shader uses a BT.601-like `YUV` decomposition:

\[
\begin{aligned}
Y &= 0.299R + 0.587G + 0.114B \\
U &= (B - Y) \cdot 0.492111 \\
V &= (R - Y) \cdot 0.877283
\end{aligned}
\]

Why this representation is used in v1:

- it separates detail-carrying luma from visibly lower-fidelity chroma
- it maps directly to the most important still-image analog degradations
- it keeps the implementation compact enough for the current still-image MVP

## 4. Implemented Stages

### 4.1 Input Conditioning / Tone Shaping

Purpose:
condition the still-frame sample location and compress high-luma regions before luma/chroma degradation so highlights roll off instead of clipping abruptly.

Mathematical meaning:
apply a soft-knee compression to luma, then rescale RGB by the luma ratio to preserve chromaticity.

Current approximation:

\[
t = \operatorname{clamp}\left(\frac{Y - k_h}{1 - k_h}, 0, 1\right)
\]

\[
S(t; \rho_h) =
\begin{cases}
t, & \rho_h = 0 \\
\dfrac{\log_2(1 + \rho_h t)}{\log_2(1 + \rho_h)}, & \rho_h > 0
\end{cases}
\]

\[
\tilde{Y} =
\begin{cases}
Y, & Y \le k_h \\
k_h + (1 - k_h) S(t; \rho_h), & Y > k_h
\end{cases}
\]

\[
(R_t, G_t, B_t) = (R, G, B)\cdot \frac{\tilde{Y}}{\max(Y, \varepsilon)}
\]

with \(\varepsilon = 10^{-5}\).

Visual effect:
highlight compression, milder hard-white clipping, and a less synthetic digital shoulder.

Signal motivation:
medium. This is not a full analog transfer model, but it is consistent with the observed rolloff priority in the current VHS-like reference.

Engineering approximation:
luma is compressed; RGB is scaled by a ratio instead of tonemapping each channel independently.

Pipeline / shader mapping:

- domain parameter group: `VhsToneSettings`
- current still controls: `SignalSettings.tone`
- stage-aligned uniform group: `effect.input_conditioning.xy`
- shader implementation: `conditioned_sample_uv()`, `soft_highlight_knee()`, and `apply_tone_shaping()`

### 4.2 Luma / Chroma Decomposition

Purpose:
split the working signal into a higher-detail luma branch and a more aggressively degraded chroma branch.

Mathematical meaning:
the decomposition above is applied after tone shaping.

Visual effect:
enables luma to remain structurally sharper than chroma even when both are degraded.

Signal motivation:
high. Different luma/chroma treatment is central to VHS-like playback.

Engineering approximation:
the working representation is BT.601-like rather than a deck-accurate encode/decode chain.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::RgbToLumaChroma`
- shader implementation: `rgb_to_yuv()` in `still_input_conditioning.wgsl`

### 4.3 Luma Bandwidth Limitation

Purpose:
reduce horizontal luma detail and microcontrast without collapsing large-scale structure.

Mathematical meaning:
apply a compact horizontal luma bandwidth approximation built from:

- a broader symmetric low-pass baseline
- a narrower mid-band estimate
- separate attenuation of mid-band and fine-band residual detail
- a small bright-edge lag bias on the low-pass branch
- a restrained highlight-gated directional bleed term that now depends on bright contour energy rather than bright plateaus alone

For \(r_Y > \varepsilon\), the current shader evaluates luma samples at:

\[
Y_{-3}, Y_{-2}, Y_{-1}, Y_0, Y_{+1}, Y_{+2}, Y_{+3}
\]

with offsets:

\[
x + \{-3\Delta_Y, -2\Delta_Y, -\Delta_Y, 0, \Delta_Y, 2\Delta_Y, 3\Delta_Y\}
\]

where:

\[
\Delta_Y = \max(0.5,\; 0.55r_Y + 0.45)
\]

and:

\[
r_Y = s_{\text{ref}} \cdot p_Y
\]

with \(p_Y = \texttt{SignalSettings.luma.blur\_px}\).

The broad low-pass baseline is:

\[
L_Y =
0.06Y_{-3} + 0.12Y_{-2} + 0.18Y_{-1} + 0.28Y_0
+ 0.18Y_{+1} + 0.12Y_{+2} + 0.06Y_{+3}
\]

The narrower mid-band estimate is:

\[
M_Y =
0.10Y_{-2} + 0.22Y_{-1} + 0.36Y_0 + 0.22Y_{+1} + 0.10Y_{+2}
\]

The luma bandwidth-loss mix is:

\[
\eta_Y = \frac{r_Y}{r_Y + 1.35}
\]

The pre/de-emphasis projection is still:

\[
\alpha_p = \operatorname{clamp}(0.015 \cdot p_{\text{db}}, 0, 0.12)
\]

where \(p_{\text{db}} = \texttt{VhsLumaSettings.preemphasis\_db}\).

The shader normalizes that compact pre-emphasis term into a recovery mix:

\[
\hat{\alpha}_p = \operatorname{clamp}\left(\frac{\alpha_p}{0.12}, 0, 1\right)
\]

Bright luma can also bias the low-pass branch slightly toward prior horizontal samples so strong bright contours feel more analog-lagged before the explicit bleed term is added:

\[
\lambda_H =
M_H(\max(Y_{-1}, Y_0); \theta_H)\; 0.18 \eta_Y
\]

\[
L_Y^{\leftarrow} =
0.09Y_{-3} + 0.16Y_{-2} + 0.22Y_{-1} + 0.26Y_0
+ 0.16Y_{+1} + 0.08Y_{+2} + 0.03Y_{+3}
\]

\[
\bar{L}_Y = (1 - \lambda_H)L_Y + \lambda_H L_Y^{\leftarrow}
\]

The residual bands are then:

\[
D_M = M_Y - \bar{L}_Y
\]

\[
D_F = Y_0 - M_Y
\]

with gains:

\[
g_M = \operatorname{clamp}\left(1 - \eta_Y (0.46 - 0.20\hat{\alpha}_p), 0.30, 1\right)
\]

\[
g_F = \operatorname{clamp}\left(1 - \eta_Y (0.88 - 0.34\hat{\alpha}_p), 0.10, 1\right)
\cdot \left(1 - M_H(\max(Y_{-1}, Y_0); \theta_H)(0.10 + 0.08\eta_Y)\right)
\]

The base luma approximation is:

\[
Y_B = \operatorname{clamp}(\bar{L}_Y + g_M D_M + g_F D_F, 0, 1)
\]

To keep bright contours from staying too clean after the bandwidth loss, the same pass then derives a restrained directional bleed term from the current tone and luma settings instead of introducing a separate bloom-like control:

\[
\theta_H = \operatorname{clamp}(k_h + 0.12, 0.72, 0.96)
\]

\[
\beta_H = \min\left(
0.16,\;
\frac{p_Y}{p_Y + 1.25}
\left(0.06 + 0.14\frac{\rho_h}{\rho_h + 1}\right)
\right)
\]

\[
M_H(Y;\theta_H) = \operatorname{clamp}\left(
\frac{Y - \theta_H}{\max(1 - \theta_H, \varepsilon)},
0,
1
\right)
\]

\[
H_H =
0.56M_H(Y_{-1}; \theta_H)
+ 0.28M_H(Y_{-2}; \theta_H)
+ 0.10M_H(Y_{-3}; \theta_H)
+ 0.06M_H(Y_0; \theta_H)
\]

\[
E_H =
0.60\max(Y_{-1} - Y_B, 0)
+ 0.28\max(Y_{-2} - Y_B, 0)
+ 0.12\max(Y_{-3} - Y_B, 0)
\]

\[
Y_L = \operatorname{clamp}\left(
Y_B + \beta_H H_H E_H (1 - 0.82Y_B),
0,
1
\right)
\]

The asymmetry is intentional: the shader biases only toward preceding horizontal samples for the bright-edge lag and bleed terms, so bright contours smear in scan direction instead of blooming isotropically. Using both \(H_H\) and \(E_H\) keeps flat highlight plateaus from glowing as aggressively as actual bright edges.

Visual effect:
horizontal band limitation, reduced digital crispness, stronger fine-texture microcontrast loss than mid-scale structure loss, and restrained bright-edge smear that reads as signal spill rather than post-process glow.

Signal motivation:
high for luma-oriented bandwidth limitation and bright-edge asymmetry, medium for the exact discrete kernels and gain curves.

Engineering approximation:
the shader now uses a compact two-scale FIR-like decomposition with gain-shaped residual attenuation and contour-gated directional highlight spill instead of a calibrated analog transfer function, temporal filter, or separate bloom pass.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::LumaRecordPath`
- pipeline projection: `luma_blur_from_bandwidth()`
- uniform mapping: `effect.luma_degradation`

### 4.4 Chroma Degradation

Purpose:
make chroma softer and less precisely registered than luma while letting bandwidth loss dominate over visible color splitting.

Mathematical meaning:
apply a lightly delayed chroma sample, prefilter it horizontally, integrate it onto a coarser horizontal chroma grid, reconstruct it with a smooth low-resolution basis, add a restrained trailing contamination term, optionally blend adjacent lines, then saturation scaling.

Resolved radii:

\[
r_\tau = s_{\text{ref}} \cdot p_\tau
\qquad
r_C = s_{\text{ref}} \cdot p_C
\]

where:

- \(p_\tau = \texttt{SignalSettings.chroma.offset\_px}\)
- \(p_C = \texttt{SignalSettings.chroma.bleed\_px}\), where `bleed_px` is a legacy preview name for the shared chroma bandwidth-loss proxy

Neutral-preservation branch:

\[
\text{if } r_C \le \varepsilon,\qquad C_S(x, y) = C(x + r_\tau, y)
\]

with the same optional vertical line blend and final saturation scaling. This keeps the neutral transform case exact when blur is disabled.

For the bandwidth-loss branch \(r_C > \varepsilon\), derive:

\[
\eta_C = \frac{r_C}{r_C + 1}
\qquad
\eta_\tau = \frac{|r_\tau|}{|r_\tau| + 0.5r_C + 0.35}
\]

\[
r_L = 0.40 + 0.72r_C + 0.28\eta_C
\qquad
d_C = 1 + 0.52r_C + 0.38\eta_C
\]

\[
s_C = \max(0.35,\; 0.24d_C)
\qquad
\alpha_S = \operatorname{clamp}\left(0.08 + 0.14\eta_C + 0.05\eta_\tau, 0, 0.27\right)
\]

Horizontal chroma prefilter:

\[
\Delta_n = \max(0.45, 0.42r_L + 0.30)
\qquad
\Delta_m = \max(\Delta_n + 0.55, 0.95r_L + 0.55)
\qquad
\Delta_f = \max(\Delta_m + 0.65, 1.55r_L + 0.85)
\]

\[
L(x, y; r_L) =
0.07C(x - \Delta_f, y)
+ 0.12C(x - \Delta_m, y)
+ 0.18C(x - \Delta_n, y)
+ 0.26C(x, y)
+ 0.18C(x + \Delta_n, y)
+ 0.12C(x + \Delta_m, y)
+ 0.07C(x + \Delta_f, y)
\]

Cell integration before coarse reconstruction:

\[
Q(x, y) =
0.22L(x - s_C, y; 1.02r_L)
+ 0.56L(x, y; r_L)
+ 0.22L(x + s_C, y; 1.02r_L)
\]

Let \(x' = x + r_\tau\). The coarse cell center used by the current pixel is:

\[
x_0 = \left(\left\lfloor\frac{x'}{d_C}\right\rfloor + 0.5\right)d_C
\qquad
\phi = \operatorname{fract}\left(\frac{x'}{d_C}\right)
\]

The current shader reconstructs chroma from the integrated coarse cells with a quadratic B-spline-like basis:

\[
w_- = \frac{1}{2}(1 - \phi)^2
\qquad
w_0 = 0.75 - (\phi - 0.5)^2
\qquad
w_+ = \frac{1}{2}\phi^2
\]

\[
C_R(x, y) =
w_-Q(x_0 - d_C, y)
+ w_0Q(x_0, y)
+ w_+Q(x_0 + d_C, y)
\]

Restrained trailing contamination:

\[
\sigma_\tau =
\begin{cases}
1, & |r_\tau| \le \varepsilon \\
\operatorname{sign}(r_\tau), & |r_\tau| > \varepsilon
\end{cases}
\]

\[
T_C(x, y) =
0.60Q(x_0, y)
+ 0.28Q(x_0 - \sigma_\tau d_C, y)\Big|_{r_L \mapsto 1.10r_L}
+ 0.12Q(x_0 - 2\sigma_\tau d_C, y)\Big|_{r_L \mapsto 1.25r_L}
\]

To keep the chroma branch subordinate to the refined luma branch, the current shader suppresses part of that trailing contamination on strong luma edges:

\[
\gamma_Y(x, y) = \operatorname{clamp}\left(
2.8 \max\left(
|Y(x, y) - Y(x - 1, y)|,\;
|Y(x, y) - Y(x + 1, y)|
\right),
0,
1
\right)
\]

\[
\alpha_S' = \alpha_S \left(1 - \gamma_Y(0.22 + 0.18\eta_C)\right)
\]

\[
C_S(x, y) = (1 - \alpha_S')C_R(x, y)
+ \alpha_S'\left[0.76C_R(x, y) + 0.24T_C(x, y)\right]
\]

Vertical chroma blend:

\[
\beta_N = 0.18 + 0.06\eta_C
\]

\[
C_V(x, y) = \beta_N C_S(x, y - 1)
+ (1 - 2\beta_N)C_S(x, y)
+ \beta_N C_S(x, y + 1)
\]

Final chroma approximation:

\[
C_D(x, y) = g_C \left[(1 - \beta_V)C_S(x, y) + \beta_V C_V(x, y)\right]
\]

where:

- \(g_C = \texttt{SignalSettings.chroma.saturation}\)
- \(\beta_V = \texttt{VhsDecodeSettings.chroma\_vertical\_blend}\)

Visual effect:
color bleeding, softened color edges, more convincing horizontal chroma resolution loss, and only mild luma/chroma misregistration.

Signal motivation:
high for lower chroma bandwidth and registration error.

Engineering approximation:
current still-image v1 still avoids a full encoded chroma carrier model, but it now uses a compact `prefilter -> cell integration -> coarse B-spline-like reconstruction -> restrained trailing contamination` approximation instead of one symmetric delayed blur. The still-path calibration deliberately keeps bandwidth loss and coarse chroma resolution stronger than the registration error so the result reads as analog chroma loss rather than RGB-split glitching, and it now suppresses part of the trailing contamination on strong luma edges so the chroma branch stays visually subordinate to the refined luma branch.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::ChromaRecordPath`
- pipeline projection: `project_vhs_model_to_preview_signal()` and `chroma_bleed_from_bandwidth()`
- uniform mapping: `effect.chroma_degradation`
- shader implementation: `degrade_chroma()`

### 4.5 Reconstruction To Output RGB

Purpose:
recombine degraded luma and chroma into a display RGB image through one explicit still-image final-stage sequence:

1. condition the branch outputs with the restrained dropout approximation
2. apply reconstruction/output contamination in `Y/C` space
3. apply the residual Y/C leakage term and decode to RGB

Mathematical meaning:
take the dropout-conditioned signal from section `5.3`, apply the luma/chroma-specific contamination terms from section `5.2`, apply the residual Y/C leakage term to the luma reconstruction basis, then invert the working matrix.

Current approximation:

\[
g_{YC} = 1 - 0.15\gamma_D
\]

\[
Y_R = \operatorname{clamp}\left(Y_L^\star + g_{YC}\epsilon_{YC}(0.10U_D^\star - 0.05V_D^\star) + n_Y^\star, 0, 1\right)
\]

\[
(U_R, V_R) = (U_D^\star, V_D^\star) + \Delta C_A + \Delta C_\perp
\]

\[
\begin{aligned}
R_{\text{out}} &= Y_R + 1.13983V_R \\
G_{\text{out}} &= Y_R - 0.39465U_R - 0.58060V_R \\
B_{\text{out}} &= Y_R + 2.03211U_R
\end{aligned}
\]

Visual effect:
coherent recombination with mild color leakage and contamination that reads more like analog signal dirt than like a uniform grain overlay, while stronger dropout concealment does not immediately reintroduce full leakage strength.

Signal motivation:
medium. Reconstruction is required, but the exact consumer-decoder behavior is simplified.

Engineering approximation:
the still pass still reconstructs directly to clamped RGB in one final fragment stage, but it now keeps `dropout-conditioned reconstruction signal -> contamination -> decode` explicit inside that pass instead of treating the whole stage like one fused catch-all helper.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::DecodeOutput`
- uniform mapping: `effect.reconstruction_output`
- shader implementation: `sample_reconstruction_contamination()`, `compose_display_yuv()`, `decode_output_rgb()`, `yuv_to_rgb()`

## 5. Secondary Integrated Terms

These are implemented now because they already existed in the prototype path, but they are not the main normative focus of this stage.

### 5.1 Line Jitter

\[
\Delta x(\ell, f) = p_J \cdot s_{\text{ref}} \cdot \sin\left(0.37(\ell + 0.5f)\right)
\]

The current fragment pass also applies a vertical still-frame offset:

\[
\Delta y = \delta_V
\]

and evaluates transport-adjusted coordinates as:

\[
\ell = \left\lfloor vH + \delta_V \right\rfloor
\qquad
u' = u + \frac{\Delta x(\ell, f)}{W}
\qquad
v' = v + \frac{\delta_V}{H}
\]

Mapping:

- formal source: `VhsTransportSettings.line_jitter_us`
- formal source for \(\delta_V\): `VhsTransportSettings.vertical_wander_lines`
- pipeline projection: `project_vhs_model_to_preview_signal()` converts the non-negative formal jitter amplitude into a restrained still-image reference-pixel amplitude; manual preview overrides are normalized separately by magnitude
- shader uniforms: `effect.input_conditioning.z`, `effect.input_conditioning.w`, and `effect.frame.w`

Calibration note:
the current still-image path keeps transport instability intentionally subordinate to tone shaping, luma softening, and chroma bandwidth loss. This avoids the decorative wobble / glitch-art failure mode that appeared when transport terms were weighted too aggressively relative to the signal-loss stages.

Boundary note:
the reconstruction pass reuses the same conditioned line phase only to derive procedural noise/dropout coordinates. It does not resample the already degraded luma/chroma textures through transport a second time.

### 5.2 Signal-Shaped Reconstruction Contamination

The shader still uses deterministic hash noise, but the final stage now treats the result as an explicit reconstruction/output contamination step in `Y/C` space instead of as an anonymous additive overlay.

Define a centered hash term

\[
\xi(p) = \operatorname{hash}(p) - 0.5
\]

and a smoothed horizontal band helper

\[
s(t) = t^2(3 - 2t)
\]

\[
b(x, \ell; \kappa, \sigma_x, \sigma_y) =
\operatorname{mix}\left(
\xi(\lfloor \kappa x \rfloor + \sigma_x,\; \sigma_y\ell + f + 1.37\sigma_x),
\xi(\lfloor \kappa x \rfloor + \sigma_x + 1,\; \sigma_y\ell + f + 1.37\sigma_x),
s(\operatorname{fract}(\kappa x))
\right)
\]

Brightness-shaped luma contamination:

\[
m_Y = 0.35 + 0.65(1 - Y_L^\star)^{0.7}
\]

\[
g_Y = \operatorname{mix}(1.0, 0.72, \gamma_D)
\]

\[
n_Y^\star =
a_Y m_Y g_Y \left(
0.45\xi(x + f,\; y + 3)
+ 0.35b(x, y; 0.12, 11, 0.31)
+ 0.20\xi(y + 29,\; f + 13)
\right)
\]

Lower-bandwidth chroma contamination:

\[
m_C = 0.55 + 0.25(1 - Y_L^\star)^{0.5}
\]

\[
g_C^D = \operatorname{mix}(1.0, 0.45, \gamma_D)
\]

\[
\eta_U = 0.72b(x, y; 0.08, 47, 0.23) + 0.28\xi(0.5y + 97,\; f + 23)
\]

\[
\eta_V = 0.72b(x, y; 0.06, 71, 0.19) + 0.28\xi(0.5y + 131,\; f + 31)
\]

\[
\eta_\perp = \xi(\lfloor 0.14x \rfloor + 0.12y + 149,\; f + 37)
\]

\[
\Delta C_A = a_C m_C g_C^D (\eta_U,\eta_V)
\]

\[
\Delta C_\perp = 0.45a_C g_C^D \eta_\perp (-V_D^\star, U_D^\star)
\]

Here \(\Delta C_A\) is the broader additive chroma contamination term, while \(\Delta C_\perp\) is a small phase-like perturbation aligned to the current chroma vector instead of to RGB space.

Mapping:

- formal source: `VhsNoiseSettings.luma_sigma`, `VhsNoiseSettings.chroma_sigma`
- pipeline projection: `project_vhs_model_to_preview_signal()`
- shader uniforms: `effect.reconstruction_output.x`, `effect.reconstruction_output.y`

Calibration note:
the current still-image v1 path keeps luma contamination more visible in dark/mid tones, gives it mild line/band correlation, keeps chroma contamination broader and softer than luma contamination, and attenuates both inside dropout masks so localized signal loss remains readable.

### 5.3 Dropout Approximation

Purpose:
introduce restrained local signal loss so the still output stops feeling perfectly intact, without turning into tearing, broken-file corruption, or temporal glitch logic.

Mathematical meaning:
activate a small number of line-oriented dropout segments from the formal dropout parameters, build a soft horizontal mask per active line, then replace part of the signal with a neighboring-line concealment approximation while collapsing chroma more strongly than luma.

Resolved control terms:

\[
q_D = \operatorname{clamp}(\texttt{dropout\_probability\_per\_line}, 0, 0.08)
\]

\[
s_D = \min(48s_{\text{ref}}, 13.5\tau_D s_{\text{ref}})
\]

where \(\tau_D = \texttt{VhsNoiseSettings.dropout\_mean\_span\_us}\).

For line \(\ell\), activate a dropout only if:

\[
h_\ell = \operatorname{hash}(\ell + 17,\; f + 5) < q_D
\]

If the line is active, derive a segment span and center:

\[
s_\ell = \max(1,\; s_D \cdot \operatorname{mix}(0.6, 1.8, \operatorname{hash}(\ell + 41,\; f + 9)))
\]

\[
x_\ell = W \cdot \operatorname{hash}(\ell + 59,\; f + 21)
\]

\[
e_\ell = \max(0.75,\; 0.2s_\ell)
\]

\[
m_S(x, \ell) =
1 - \operatorname{smoothstep}\left(
0.5s_\ell,\;
0.5s_\ell + e_\ell,\;
|x - x_\ell|
\right)
\]

\[
m_B(x, \ell) =
\operatorname{mix}\left(
0.82,\;
1.0,\;
\operatorname{hash}(\lfloor 0.35x \rfloor + \ell,\; f + 37)
\right)
\]

\[
m_D(x, \ell) = m_S(x, \ell)m_B(x, \ell)
\]

The current still-image v1 approximation then builds a local concealment blend:

\[
Y_C(x, y) = 0.55Y_L(x, y - 1) + 0.45Y_L(x, y + 1)
\]

\[
C_C(x, y) = 0.55C_D(x, y - 1) + 0.45C_D(x, y + 1)
\]

\[
\gamma_D(x, \ell) =
m_D(x, \ell)\operatorname{mix}\left(
0.35,\;
0.72,\;
\operatorname{hash}(\ell + 73,\; f + 11)
\right)
\]

\[
\eta_D(x, y) =
0.08\gamma_D(x, \ell)\left(
\operatorname{hash}(x, y + 29 + f) - 0.5
\right)
\]

\[
Y_L^\star(x, y) =
\operatorname{clamp}\left(
(1 - \gamma_D)Y_L(x, y)
+ \gamma_D Y_C(x, y)
+ 0.05\gamma_D
+ \eta_D(x, y),
0,
1
\right)
\]

\[
\kappa_C = 0.35(1 - 0.25\gamma_D)
\]

\[
C_D^\star(x, y) =
(1 - 0.85\gamma_D)C_D(x, y)
+ 0.85\gamma_D \cdot \kappa_C C_C(x, y)
\]

Visual effect:
small local line segments that look slightly washed, noisy, and chroma-depleted rather than digitally torn apart.

Signal motivation:
medium to high. Real dropout handling is often temporal or decoder-specific; this still-image subset instead focuses on plausible local signal loss without introducing frame history.

Engineering approximation:
the shader uses deterministic line hashes and neighboring-line concealment in the final pass. Stronger dropout concealment now also reduces the amount of chroma that neighboring lines are allowed to reintroduce, and the same pass lightly attenuates its general reconstruction-contamination terms inside active dropout regions so the concealment approximation does not read like a uniform noisy overlay. This is intentionally a still-image v1 approximation, not a temporal dropout compensator.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::NoiseAndDropouts`
- formal source: `VhsNoiseSettings.{dropout_probability_per_line,dropout_mean_span_us}`
- shader uniforms: `effect.reconstruction_aux.xy`
- shader implementation: `line_dropout_mask()`, `apply_dropout_approximation()`

## 6. Mapping To Code

### 6.1 Formal parameters to `casseted-signal`

| Formula symbol | Formal parameter | Current still control |
| --- | --- | --- |
| \(k_h\) | `VhsToneSettings.highlight_soft_knee` | `SignalSettings.tone.highlight_soft_knee` |
| \(\rho_h\) | `VhsToneSettings.highlight_compression` | `SignalSettings.tone.highlight_compression` |
| \(b_Y\) | `VhsLumaSettings.bandwidth_mhz` | projected to `SignalSettings.luma.blur_px` |
| \(\alpha_p\) input | `VhsLumaSettings.preemphasis_db` | projected directly into the uniform block |
| \(\tau_C\) | `VhsChromaSettings.delay_us` | projected to `SignalSettings.chroma.offset_px` |
| \(b_C\) | `VhsChromaSettings.bandwidth_khz` | projected to `SignalSettings.chroma.bleed_px` |
| \(g_C\) | `VhsChromaSettings.saturation_gain` | `SignalSettings.chroma.saturation` |
| \(\beta_V\) | `VhsDecodeSettings.chroma_vertical_blend` | projected directly into the uniform block |
| \(\epsilon_{YC}\) | `VhsDecodeSettings.luma_chroma_crosstalk` | projected directly into the uniform block |
| \(q_D\) | `VhsNoiseSettings.dropout_probability_per_line` | projected directly into the reconstruction auxiliary uniform block |
| \(\tau_D\) | `VhsNoiseSettings.dropout_mean_span_us` | projected to a pixel-space span in the reconstruction auxiliary uniform block |

### 6.2 Formal parameters to pipeline stages

| Formal stage | Current pipeline representation | Current WGSL location |
| --- | --- | --- |
| `InputDecode` | implicit working assumptions in `resolve_still_stages()` and the first-pass working-signal write | `still_input_conditioning.wgsl` |
| `ToneShaping` | `SignalSettings.tone` + `effect.input_conditioning.xy` | `soft_highlight_knee()`, `apply_tone_shaping()` |
| `RgbToLumaChroma` | first-pass working decomposition | `rgb_to_yuv()` in `still_input_conditioning.wgsl` |
| `LumaRecordPath` | `SignalSettings.luma.blur_px` bandwidth-loss proxy + projected pre-emphasis gain + derived highlight-bleed threshold/amount from the current tone+luma state | `degrade_luma()`, `highlight_bleed()` |
| `ChromaRecordPath` | `SignalSettings.chroma.*` + projected decode blend | `degrade_chroma()` |
| `TransportInstability` | `SignalSettings.tracking.*`; fused into the input-conditioning pass ahead of the working-signal fan-out | `conditioned_sample_uv()` |
| `NoiseAndDropouts` | brightness-shaped luma contamination and softer band-correlated chroma contamination from `SignalSettings.noise.*`, plus model-driven dropout auxiliaries from `VhsNoiseSettings.dropout_*` | `sample_reconstruction_contamination()`, `line_dropout_mask()`, `apply_dropout_approximation()` |
| `DecodeOutput` | projected crosstalk + inverse matrix | `compose_display_yuv()`, `decode_output_rgb()`, `yuv_to_rgb()` |

### 6.3 What is implemented now vs later

Use [`../architecture/signal-model-v1-subset.md`](../architecture/signal-model-v1-subset.md) for the full field-level status map. In short:

Fully active:

- `VhsToneSettings.highlight_soft_knee`
- `VhsToneSettings.highlight_compression`
- `VhsChromaSettings.saturation_gain`
- `VhsDecodeSettings.chroma_vertical_blend`
- `VhsDecodeSettings.luma_chroma_crosstalk`

Partially active / approximated:

- fixed `sRGB` + BT.601-like + progressive input assumptions at the stage level, without field-driven `VhsInputSettings` switching yet
- `VhsLumaSettings.{bandwidth_mhz,preemphasis_db}` through the compact luma bandwidth/detail approximation
- `VhsChromaSettings.{delay_us,bandwidth_khz}` through the compact chroma offset/bandwidth-loss approximation
- `VhsTransportSettings.{line_jitter_us,vertical_wander_lines}` through the still-frame spatial transport subset
- `VhsNoiseSettings.{luma_sigma,chroma_sigma}` through brightness-shaped luma contamination and softer chroma contamination
- `VhsNoiseSettings.{dropout_probability_per_line,dropout_mean_span_us}` through restrained local still-image dropout concealment
- derived highlight bleed from the current tone + luma state

Documented here but not implemented yet:

- `VhsInputSettings.{matrix,transfer,temporal_sampling}` as runtime selectors
- chroma phase error from `VhsChromaSettings.phase_error_deg`
- chroma phase noise from `VhsNoiseSettings.chroma_phase_noise_deg`
- head-switching region behavior from `VhsTransportSettings.head_switching_*`
- explicit output-transfer shaping from `VhsDecodeSettings.output_transfer`
- `VhsModel.standard` as a runtime selector once a concrete model already carries resolved field values

## 7. Projection Rules Used By The Current Still Pipeline

The current still pipeline uses a narrow projection from formal `VhsModel` defaults into the compact `SignalSettings` preview layer, plus a small set of model-only auxiliary terms for pre-emphasis, decode, and dropout handling.

These are engineering approximations, not physical constants:

\[
p_Y = \min\left(4.5,\; 1.6 \cdot \max(0, 4.2 - b_Y)\right)
\]

\[
p_\tau = 13.5 \cdot 0.4 \cdot \tau_C
\]

\[
p_C = \min\left(4.5,\; \frac{\max(0, 1000 - b_C)}{300}\right)
\]

\[
p_J = 13.5 \cdot \tau_J \cdot 0.22
\]

\[
a_Y = \min(1,\; \sigma_Y)
\qquad
a_C = \min(1,\; 0.35 \cdot \sigma_C)
\]

\[
q_D = \operatorname{clamp}(p_{\mathrm{drop}}, 0, 0.08)
\qquad
s_D = \min(48s_{\text{ref}}, 13.5\tau_D s_{\text{ref}})
\]

where:

- \(b_Y\) is in MHz
- \(b_C\) is in kHz
- \(\tau_C\) and \(\tau_J\) are in microseconds
- \(\sigma_Y\) and \(\sigma_C\) are the formal luma/chroma noise sigmas projected into the preview amplitudes that the reconstruction shader reshapes into luma/chroma-specific contamination

These projection rules currently live across `crates/casseted-pipeline/src/projection.rs` and `crates/casseted-pipeline/src/stages.rs`:

- `project_vhs_model_to_preview_signal()`
- `line_jitter_px_from_timebase()`
- `luma_blur_from_bandwidth()`
- `chroma_bleed_from_bandwidth()`
- `luma_noise_amount_from_sigma()`
- `chroma_noise_amount_from_sigma()`
- `highlight_bleed_threshold()`
- `highlight_bleed_amount()`
- `dropout_line_probability()`
- `dropout_span_px_from_time()`
- `detail_mix_from_preemphasis()`

Important runtime note:
`StillImagePipeline::from_vhs_model()` uses the full projection above and stores it as the private preview base. `StillImagePipeline::new(signal)` is the narrower manual preview path; in that mode the model-only terms \(\alpha_p\), \(\beta_V\), \(\epsilon_{YC}\), \(q_D\), and \(s_D\) are held at zero unless a formal model is also present. Model-backed preview edits now travel through explicit `SignalOverrides` instead of being inferred from equality between two mutable `SignalSettings` blobs.

Current calibration intent:
the projection now overweights luma/chroma bandwidth loss relative to transport and delay terms so the limited multi-pass path reads as technical analog degradation rather than glitch-oriented distortion art.

### 7.1 Preview/manual guardrails

The still pipeline applies an additional preview-only normalization layer to manual `SignalSettings` before stage resolution.

This layer does not modify the formal `VhsModel`. It only converts raw manual preview inputs into effective applied values when a manual path or explicit preview override path diverges from the model projection.

For a non-negative preview control \(x\), the current soft-cap function is:

\[
G(x; r, h) =
\begin{cases}
\max(0, x), & x \le r \\
r + \frac{(x - r)(h - r)}{(x - r) + (h - r)}, & x > r
\end{cases}
\]

where \(r\) is the recommended cap and \(h\) is the asymptotic hard cap.

For signed controls, the still path uses:

\[
G_{\pm}(x; r, h) = \operatorname{sign}(x)\, G(|x|; r, h)
\]

Current effective preview rules:

\[
p_{Y,\mathrm{eff}} = G(p_Y; 3.25, 4.75)
\]

\[
p_{\tau,\mathrm{eff}} = G_{\pm}(p_\tau; 0.35, 0.60)
\]

\[
p_{C,\mathrm{eff}} = \max\left(G(p_C; 3.0, 4.25),\; 2.5|p_{\tau,\mathrm{eff}}|\right)
\]

\[
a_{Y,\mathrm{eff}} = G(a_Y; 0.02, 0.04)
\qquad
a_{C,\mathrm{eff}} = G(a_C; 0.012, 0.025)
\]

\[
p_{J,\mathrm{eff}} = G(|p_J|; 0.35, 0.55)
\qquad
\delta_{V,\mathrm{eff}} = G_{\pm}(\delta_V; 0.35, 0.75)
\]

Interpretation:

- strong preview values are still allowed, but they stop scaling linearly into the glitch-prone region
- chroma offset is intentionally coupled to a minimum chroma bandwidth-loss proxy so the image reads as chroma loss rather than RGB splitting
- noise and transport terms remain secondary to tone shaping, luma softening, and chroma bandwidth loss
- on model-backed pipelines, this normalization now applies only to overridden preview terms; untouched model-projected terms remain unchanged
- if either chroma offset or chroma bandwidth-loss proxy is overridden, that pair is still normalized together so the guardrail can preserve the intended analog priority order

## 8. Explicitly Not Modeled At This Stage

The current implementation and this formulas document intentionally do not model:

- temporal history or frame-to-frame state
- true interlacing dynamics
- full tape transport mechanics
- advanced head switching behavior
- full RF / FM carrier behavior
- deep nonlinear analog electronics
- multi-generation dubbing accumulation
- a separate CPU reference engine
- a generalized render graph or pass planner

Those topics can be added later, but they should be introduced only when the repository actually gains the corresponding execution path.

# Signal Model v1 Formulas

This document is the engineering reference for the subset of signal-model v1 that is currently implemented in the still-image pipeline and for the immediately adjacent implementation path.

It is intentionally narrower than a full VHS deck model. The goal is to define the exact discrete approximations that the repository currently uses for:

- tone shaping with soft highlight compression
- BT.601-like luma/chroma working decomposition
- luma-oriented horizontal bandwidth loss
- one controllable chroma degradation path
- reconstruction back to RGB

The current GPU implementation lives in:

- `crates/casseted-pipeline/src/lib.rs`
- `shaders/passes/still_analog.wgsl`

## 1. Scope

The implemented still-image subset is:

1. input interpretation: gamma-coded `sRGB` input, BT.601-like working coefficients
2. tone shaping: luma-preserving soft-knee highlight compression
3. `RGB -> YUV` decomposition
4. luma low-pass/detail attenuation
5. chroma horizontal delay + chroma blur + optional vertical chroma blend
6. `YUV -> RGB` reconstruction with a small Y/C leakage term

Secondary prototype terms that are still present in the current shader:

- deterministic per-line horizontal jitter
- additive luma/chroma noise

These secondary terms are documented here because they are implemented, but they are not the main normative focus of this stage.

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

### Discrete radii used by the still shader

| Symbol | Meaning | Current source |
| --- | --- | --- |
| \(r_Y\) | resolved luma blur radius in pixels | `SignalSettings.luma.blur_px * s_ref` |
| \(r_\tau\) | resolved chroma delay in pixels | `SignalSettings.chroma.offset_px * s_ref` |
| \(r_C\) | resolved chroma blur radius in pixels | `SignalSettings.chroma.bleed_px * s_ref` |

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

Pipeline mapping:
fused into `sample_working_yuv()` inside `shaders/passes/still_analog.wgsl`.

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
- it keeps the implementation compact enough for a single-pass MVP

## 4. Implemented Stages

### 4.1 Tone Shaping / Soft Highlight Compression

Purpose:
compress high-luma regions before luma/chroma degradation so highlights roll off instead of clipping abruptly.

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
- shader implementation: `soft_highlight_knee()` and `apply_tone_curve()`

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
- shader implementation: `rgb_to_yuv()` inside `sample_working_yuv()`

### 4.3 Luma Bandwidth Limitation

Purpose:
reduce horizontal luma detail and microcontrast without collapsing large-scale structure.

Mathematical meaning:
apply a compact 5-tap horizontal low-pass filter in the luma branch, then optionally add a small edge residual derived from the model pre-emphasis term.

The current shader evaluates luma samples at:

\[
Y_{-2}, Y_{-1}, Y_0, Y_{+1}, Y_{+2}
\]

with offsets:

\[
x + \{-2r_Y, -r_Y, 0, r_Y, 2r_Y\}
\]

where:

\[
r_Y = s_{\text{ref}} \cdot p_Y
\]

and \(p_Y = \texttt{SignalSettings.luma.blur\_px}\).

The low-pass output is:

\[
H_Y = 0.12Y_{-2} + 0.23Y_{-1} + 0.30Y_0 + 0.23Y_{+1} + 0.12Y_{+2}
\]

The compact detail residual is:

\[
D_Y = Y_0 - (0.2Y_{-1} + 0.6Y_0 + 0.2Y_{+1})
\]

The final luma approximation is:

\[
Y_L = \operatorname{clamp}(H_Y + \alpha_p D_Y, 0, 1)
\]

The current projection from the formal pre-emphasis setting is:

\[
\alpha_p = \operatorname{clamp}(0.025 \cdot p_{\text{db}}, 0, 0.20)
\]

where \(p_{\text{db}} = \texttt{VhsLumaSettings.preemphasis\_db}\).

Visual effect:
horizontal softening, less digital crispness, and reduced microcontrast in fine textures.

Signal motivation:
high for the low-pass concept, medium for the exact kernel.

Engineering approximation:
the shader uses a compact weighted FIR-like kernel rather than a calibrated analog transfer function.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::LumaRecordPath`
- pipeline projection: `luma_blur_from_bandwidth()`
- uniform mapping: `effect.tone_luma.z` and `effect.tone_luma.w`

### 4.4 Chroma Degradation

Purpose:
make chroma softer and less precisely registered than luma.

Mathematical meaning:
apply a delayed chroma sample, horizontal chroma blur, optional vertical chroma blend, then saturation scaling.

Resolved radii:

\[
r_\tau = s_{\text{ref}} \cdot p_\tau
\qquad
r_C = s_{\text{ref}} \cdot p_C
\]

where:

- \(p_\tau = \texttt{SignalSettings.chroma.offset\_px}\)
- \(p_C = \texttt{SignalSettings.chroma.bleed\_px}\), where `bleed_px` is a legacy preview name for the chroma blur radius proxy

The delayed chroma taps are sampled at:

\[
C_0 = C(x + r_\tau, y)
\]

\[
C_- = C(x + r_\tau - r_C, y), \qquad C_+ = C(x + r_\tau + r_C, y)
\]

Horizontal chroma blur:

\[
C_H = 0.25C_- + 0.5C_0 + 0.25C_+
\]

Vertical chroma blend:

\[
C_\uparrow = C(x + r_\tau, y - 1), \qquad C_\downarrow = C(x + r_\tau, y + 1)
\]

\[
C_V = 0.25(C_\uparrow + 2C_H + C_\downarrow)
\]

Final chroma approximation:

\[
C_D = g_C \left[(1 - \beta_V)C_H + \beta_V C_V\right]
\]

where:

- \(g_C = \texttt{SignalSettings.chroma.saturation}\)
- \(\beta_V = \texttt{VhsDecodeSettings.chroma\_vertical\_blend}\)

Visual effect:
color bleeding, softened color edges, and mild luma/chroma misregistration.

Signal motivation:
high for lower chroma bandwidth and registration error.

Engineering approximation:
current still-image v1 uses one delayed, blurred chroma path instead of a full encoded chroma carrier model.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::ChromaRecordPath`
- pipeline projection: `prototype_signal_from_model()` and `chroma_bleed_from_bandwidth()`
- shader implementation: `chroma_horizontal`, `chroma_vertical`, `chroma`

### 4.5 Reconstruction To Output RGB

Purpose:
recombine degraded luma and chroma into a display RGB image.

Mathematical meaning:
add a small Y/C leakage term to luma, add chroma noise, then invert the working matrix.

Current approximation:

\[
Y_R = \operatorname{clamp}\left(Y_L + \epsilon_{YC}(0.10U_D - 0.05V_D) + n_Y, 0, 1\right)
\]

\[
(U_R, V_R) = (U_D, V_D) + (n_C, -0.5n_C)
\]

\[
\begin{aligned}
R_{\text{out}} &= Y_R + 1.13983V_R \\
G_{\text{out}} &= Y_R - 0.39465U_R - 0.58060V_R \\
B_{\text{out}} &= Y_R + 2.03211U_R
\end{aligned}
\]

Visual effect:
coherent recombination with mild color leakage and softened chroma detail.

Signal motivation:
medium. Reconstruction is required, but the exact consumer-decoder behavior is simplified.

Engineering approximation:
the still pass reconstructs directly to clamped RGB in the final fragment stage.

Pipeline / shader mapping:

- formal stage: `VhsSignalStage::DecodeOutput`
- uniform mapping: `effect.noise_decode.z`
- shader implementation: `reconstructed_y`, `reconstructed_chroma`, `yuv_to_rgb()`

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
- pipeline projection: `prototype_signal_from_model()` converts \(\mu s \to\) reference pixels
- shader uniforms: `effect.transport.x`, `effect.transport.y`, `effect.transport.z`

### 5.2 Additive Noise

The shader uses deterministic hash noise:

\[
n_Y = a_Y \cdot (h_Y - 0.5)
\qquad
n_C = a_C \cdot (h_C - 0.5)
\]

where \(h_Y, h_C \in [0, 1]\) are hash-derived pseudo-random samples.

Mapping:

- formal source: `VhsNoiseSettings.luma_sigma`, `VhsNoiseSettings.chroma_sigma`
- pipeline projection: `prototype_signal_from_model()`
- shader uniforms: `effect.noise_decode.x`, `effect.noise_decode.y`

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

### 6.2 Formal parameters to pipeline stages

| Formal stage | Current pipeline representation | Current WGSL location |
| --- | --- | --- |
| `InputDecode` | implicit working assumptions in `effect_uniforms()` and `sample_working_yuv()` | `sample_working_yuv()` |
| `ToneShaping` | `SignalSettings.tone` + `effect.tone_luma.xy` | `soft_highlight_knee()`, `apply_tone_curve()` |
| `RgbToLumaChroma` | fused working decomposition | `rgb_to_yuv()` |
| `LumaRecordPath` | `SignalSettings.luma.blur_px` + projected pre-emphasis gain | `blurred_luma`, `edge_band`, `luma` |
| `ChromaRecordPath` | `SignalSettings.chroma.*` + projected decode blend | `chroma_horizontal`, `chroma_vertical`, `chroma` |
| `TransportInstability` | `SignalSettings.tracking.*`; fused ahead of both luma and chroma sampling | `line_jitter`, `base_uv` |
| `NoiseAndDropouts` | noise-only subset of the stage via `SignalSettings.noise.*` | `luma_noise`, `chroma_noise` |
| `DecodeOutput` | projected crosstalk + inverse matrix | `reconstructed_y`, `reconstructed_chroma`, `yuv_to_rgb()` |

### 6.3 What is implemented now vs later

Implemented now:

- tone shaping with soft highlight compression
- BT.601-like working `YUV` decomposition
- luma low-pass/detail attenuation
- delayed and blurred chroma path with saturation control
- reconstruction back to RGB
- line jitter and additive luma/chroma noise

Documented here but not implemented yet:

- chroma phase error from `VhsChromaSettings.phase_error_deg`
- dropout segments from `VhsNoiseSettings.dropout_*`
- head-switching region behavior from `VhsTransportSettings.head_switching_*`
- explicit output-transfer shaping from `VhsDecodeSettings.output_transfer`

## 7. Projection Rules Used By The Current Still Pipeline

The current single-pass pipeline uses a narrow projection from formal `VhsModel` defaults into the compact `SignalSettings` preview layer.

These are engineering approximations, not physical constants:

\[
p_Y = \min\left(4,\; 1.25 \cdot \frac{\max(0, 4.2 - b_Y)}{1.2}\right)
\]

\[
p_\tau = 13.5 \cdot \tau_C
\]

\[
p_C = \min\left(4,\; \frac{\max(0, 1000 - b_C)}{400}\right)
\]

\[
p_J = 13.5 \cdot \tau_J \cdot 0.5
\]

\[
a_Y = \min(1,\; 1.25 \cdot \sigma_Y)
\qquad
a_C = \min(1,\; 0.5 \cdot \sigma_C)
\]

where:

- \(b_Y\) is in MHz
- \(b_C\) is in kHz
- \(\tau_C\) and \(\tau_J\) are in microseconds
- \(\sigma_Y\) and \(\sigma_C\) are the formal luma/chroma noise sigmas

These projection rules currently live in `crates/casseted-pipeline/src/lib.rs`:

- `prototype_signal_from_model()`
- `line_jitter_px_from_timebase()`
- `luma_blur_from_bandwidth()`
- `chroma_bleed_from_bandwidth()`
- `luma_noise_amount_from_sigma()`
- `chroma_noise_amount_from_sigma()`
- `detail_mix_from_preemphasis()`

Important runtime note:
`StillImagePipeline::from_vhs_model()` uses the full projection above. `StillImagePipeline::new(signal)` is the narrower manual preview path; in that mode the model-only terms \(\alpha_p\), \(\beta_V\), and \(\epsilon_{YC}\) are held at zero unless a formal model is also present.

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

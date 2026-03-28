use crate::{StillImagePipeline, StillPipelineRuntime};
use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
use casseted_testing::assert_images_not_identical;
use casseted_types::{FrameSize, ImageFrame};

const CALIBRATION_SIZE: FrameSize = FrameSize {
    width: 160,
    height: 120,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalibrationCase {
    ColoredEdges,
    PortraitMidtones,
    BrightHighlights,
    NeutralLowSaturation,
    UiDetailEdges,
    DarkQuietFloor,
}

const CALIBRATION_CASES: [CalibrationCase; 6] = [
    CalibrationCase::ColoredEdges,
    CalibrationCase::PortraitMidtones,
    CalibrationCase::BrightHighlights,
    CalibrationCase::NeutralLowSaturation,
    CalibrationCase::UiDetailEdges,
    CalibrationCase::DarkQuietFloor,
];

#[derive(Debug, Clone, Copy)]
struct CalibrationMetrics {
    mean_luma_diff: f32,
    mean_chroma_diff: f32,
    input_luma_edge_energy: f32,
    output_luma_edge_energy: f32,
    input_chroma_edge_energy: f32,
    output_chroma_edge_energy: f32,
    input_mean_chroma_magnitude: f32,
    output_mean_chroma_magnitude: f32,
    highlight_band_luma_diff: f32,
    quiet_region_mean_luma_diff: f32,
    quiet_region_mean_chroma_diff: f32,
    dark_quiet_region_mean_luma_diff: f32,
    dark_quiet_region_mean_chroma_diff: f32,
}

impl CalibrationMetrics {
    fn luma_edge_retention(self) -> f32 {
        self.output_luma_edge_energy / self.input_luma_edge_energy.max(1e-5)
    }

    fn chroma_edge_retention(self) -> f32 {
        self.output_chroma_edge_energy / self.input_chroma_edge_energy.max(1e-5)
    }
}

impl CalibrationCase {
    fn key(self) -> &'static str {
        match self {
            Self::ColoredEdges => "colored-edges",
            Self::PortraitMidtones => "portrait-midtones",
            Self::BrightHighlights => "bright-highlights",
            Self::NeutralLowSaturation => "neutral-low-saturation",
            Self::UiDetailEdges => "ui-detail-edges",
            Self::DarkQuietFloor => "dark-quiet-floor",
        }
    }

    fn input(self) -> ImageFrame {
        match self {
            Self::ColoredEdges => colored_edges_image(CALIBRATION_SIZE),
            Self::PortraitMidtones => portrait_midtones_image(CALIBRATION_SIZE),
            Self::BrightHighlights => bright_highlights_image(CALIBRATION_SIZE),
            Self::NeutralLowSaturation => neutral_low_saturation_image(CALIBRATION_SIZE),
            Self::UiDetailEdges => ui_detail_edges_image(CALIBRATION_SIZE),
            Self::DarkQuietFloor => dark_quiet_floor_image(CALIBRATION_SIZE),
        }
    }

    fn assert_signal_first_hierarchy(self, metrics: CalibrationMetrics) {
        match self {
            Self::ColoredEdges => {
                assert!(
                    metrics.luma_edge_retention() > metrics.chroma_edge_retention() + 0.04,
                    "{} should keep luma edges more stable than chroma edges",
                    self.key()
                );
                assert!(
                    metrics.mean_chroma_diff > 0.010,
                    "{} should visibly soften or shift chroma",
                    self.key()
                );
            }
            Self::PortraitMidtones => {
                assert!(
                    metrics.luma_edge_retention() + 0.04 >= metrics.chroma_edge_retention(),
                    "{} should keep portrait structure at least as stable as chroma detail",
                    self.key()
                );
                assert!(
                    metrics.quiet_region_mean_luma_diff > 0.0014,
                    "{} should carry some low-amplitude quiet-region luma character",
                    self.key()
                );
                assert!(
                    metrics.output_mean_chroma_magnitude
                        <= metrics.input_mean_chroma_magnitude * 1.02 + 0.002,
                    "{} should not oversaturate skin-like midtones",
                    self.key()
                );
            }
            Self::BrightHighlights => {
                assert!(
                    metrics.highlight_band_luma_diff > 0.012,
                    "{} should show a real highlight-band response",
                    self.key()
                );
                assert!(
                    metrics.mean_luma_diff > metrics.mean_chroma_diff * 2.0,
                    "{} should react more strongly in luma than in chroma on highlight content",
                    self.key()
                );
            }
            Self::NeutralLowSaturation => {
                assert!(
                    metrics.luma_edge_retention() > metrics.chroma_edge_retention(),
                    "{} should keep neutral structure ahead of chroma breakup",
                    self.key()
                );
                assert!(
                    metrics.quiet_region_mean_luma_diff > 0.0011,
                    "{} should no longer leave quiet neutral regions too untouched",
                    self.key()
                );
                assert!(
                    metrics.output_mean_chroma_magnitude
                        <= metrics.input_mean_chroma_magnitude + 0.010,
                    "{} should not inject strong chroma contamination into neutral content",
                    self.key()
                );
            }
            Self::UiDetailEdges => {
                assert!(
                    metrics.luma_edge_retention() > 0.45,
                    "{} should preserve enough luma structure to avoid mush",
                    self.key()
                );
                assert!(
                    metrics.quiet_region_mean_luma_diff > 0.0016,
                    "{} should still introduce subtle quiet-region character around UI/text detail",
                    self.key()
                );
                assert!(
                    metrics.mean_luma_diff > metrics.mean_chroma_diff * 2.0,
                    "{} should still respond primarily through luma degradation",
                    self.key()
                );
            }
            Self::DarkQuietFloor => {
                assert!(
                    metrics.dark_quiet_region_mean_luma_diff > 0.0012,
                    "{} should lift the dark quiet floor with restrained analog activity",
                    self.key()
                );
                assert!(
                    metrics.dark_quiet_region_mean_chroma_diff
                        <= metrics.dark_quiet_region_mean_luma_diff * 0.95 + 0.0015,
                    "{} should keep dark-floor contamination primarily luma-led",
                    self.key()
                );
                assert!(
                    metrics.luma_edge_retention() > 0.35,
                    "{} should not collapse the remaining dark-scene structure",
                    self.key()
                );
            }
        }
    }
}

fn try_gpu_context() -> Result<GpuContext, GpuInitError> {
    pollster::block_on(GpuContext::request(&GpuContextDescriptor::default()))
}

fn default_pipeline() -> StillImagePipeline {
    StillImagePipeline::default()
}

fn render_with_default_pipeline(gpu: &GpuContext, input: &ImageFrame) -> ImageFrame {
    default_pipeline()
        .process_with_gpu(gpu, input)
        .expect("default pipeline should render calibration case")
}

fn render_with_runtime(
    runtime: &StillPipelineRuntime,
    pipeline: &StillImagePipeline,
    input: &ImageFrame,
) -> ImageFrame {
    pipeline
        .process_with_runtime(runtime, input)
        .expect("compiled runtime should render calibration case")
}

fn calibration_metrics(input: &ImageFrame, output: &ImageFrame) -> CalibrationMetrics {
    let mut mean_luma_diff = 0.0;
    let mut mean_chroma_diff = 0.0;
    let mut input_luma_edge_energy = 0.0;
    let mut output_luma_edge_energy = 0.0;
    let mut input_chroma_edge_energy = 0.0;
    let mut output_chroma_edge_energy = 0.0;
    let mut input_mean_chroma_magnitude = 0.0;
    let mut output_mean_chroma_magnitude = 0.0;
    let mut highlight_band_luma_diff = 0.0;
    let mut highlight_samples = 0_u32;
    let mut quiet_region_mean_luma_diff = 0.0;
    let mut quiet_region_mean_chroma_diff = 0.0;
    let mut dark_quiet_region_mean_luma_diff = 0.0;
    let mut dark_quiet_region_mean_chroma_diff = 0.0;
    let mut quiet_region_samples = 0_u32;
    let mut dark_quiet_region_samples = 0_u32;
    let width = input.descriptor.size.width as usize;
    let height = input.descriptor.size.height as usize;
    let mut pixels = 0_u32;
    let mut edge_samples = 0_u32;

    for y in 0..height {
        for x in 0..width {
            let input_yuv = sample_yuv(input, width, x, y);
            let output_yuv = sample_yuv(output, width, x, y);

            mean_luma_diff += (input_yuv.0 - output_yuv.0).abs();
            mean_chroma_diff +=
                (input_yuv.1 - output_yuv.1).abs() + (input_yuv.2 - output_yuv.2).abs();
            input_mean_chroma_magnitude +=
                (input_yuv.1 * input_yuv.1 + input_yuv.2 * input_yuv.2).sqrt();
            output_mean_chroma_magnitude +=
                (output_yuv.1 * output_yuv.1 + output_yuv.2 * output_yuv.2).sqrt();

            if input_yuv.0 > 0.75 {
                highlight_band_luma_diff += (input_yuv.0 - output_yuv.0).abs();
                highlight_samples += 1;
            }

            if x > 0 && x + 1 < width && y > 0 && y + 1 < height {
                let input_left_yuv = sample_yuv(input, width, x - 1, y);
                let input_right_yuv = sample_yuv(input, width, x + 1, y);
                let input_up_yuv = sample_yuv(input, width, x, y - 1);
                let input_down_yuv = sample_yuv(input, width, x, y + 1);
                let quiet_luma_gradient = f32::max(
                    f32::max((input_yuv.0 - input_left_yuv.0).abs(), (input_yuv.0 - input_right_yuv.0).abs()),
                    f32::max((input_yuv.0 - input_up_yuv.0).abs(), (input_yuv.0 - input_down_yuv.0).abs()),
                );
                let quiet_chroma_gradient = f32::max(
                    f32::max(
                        chroma_distance(input_yuv.1, input_yuv.2, input_left_yuv.1, input_left_yuv.2),
                        chroma_distance(input_yuv.1, input_yuv.2, input_right_yuv.1, input_right_yuv.2),
                    ),
                    f32::max(
                        chroma_distance(input_yuv.1, input_yuv.2, input_up_yuv.1, input_up_yuv.2),
                        chroma_distance(input_yuv.1, input_yuv.2, input_down_yuv.1, input_down_yuv.2),
                    ),
                );
                let is_quiet_region = quiet_luma_gradient < 0.035 && quiet_chroma_gradient < 0.024;

                if is_quiet_region {
                    let luma_diff = (input_yuv.0 - output_yuv.0).abs();
                    let chroma_diff =
                        (input_yuv.1 - output_yuv.1).abs() + (input_yuv.2 - output_yuv.2).abs();
                    quiet_region_mean_luma_diff += luma_diff;
                    quiet_region_mean_chroma_diff += chroma_diff;
                    quiet_region_samples += 1;

                    if input_yuv.0 < 0.22 {
                        dark_quiet_region_mean_luma_diff += luma_diff;
                        dark_quiet_region_mean_chroma_diff += chroma_diff;
                        dark_quiet_region_samples += 1;
                    }
                }
            }

            if x + 1 < width {
                let input_right_yuv = sample_yuv(input, width, x + 1, y);
                let output_right_yuv = sample_yuv(output, width, x + 1, y);
                input_luma_edge_energy += (input_yuv.0 - input_right_yuv.0).abs();
                output_luma_edge_energy += (output_yuv.0 - output_right_yuv.0).abs();
                input_chroma_edge_energy += chroma_distance(
                    input_yuv.1,
                    input_yuv.2,
                    input_right_yuv.1,
                    input_right_yuv.2,
                );
                output_chroma_edge_energy += chroma_distance(
                    output_yuv.1,
                    output_yuv.2,
                    output_right_yuv.1,
                    output_right_yuv.2,
                );
                edge_samples += 1;
            }

            pixels += 1;
        }
    }

    CalibrationMetrics {
        mean_luma_diff: mean_luma_diff / pixels as f32,
        mean_chroma_diff: mean_chroma_diff / pixels as f32,
        input_luma_edge_energy: input_luma_edge_energy / edge_samples as f32,
        output_luma_edge_energy: output_luma_edge_energy / edge_samples as f32,
        input_chroma_edge_energy: input_chroma_edge_energy / edge_samples as f32,
        output_chroma_edge_energy: output_chroma_edge_energy / edge_samples as f32,
        input_mean_chroma_magnitude: input_mean_chroma_magnitude / pixels as f32,
        output_mean_chroma_magnitude: output_mean_chroma_magnitude / pixels as f32,
        highlight_band_luma_diff: if highlight_samples == 0 {
            0.0
        } else {
            highlight_band_luma_diff / highlight_samples as f32
        },
        quiet_region_mean_luma_diff: if quiet_region_samples == 0 {
            0.0
        } else {
            quiet_region_mean_luma_diff / quiet_region_samples as f32
        },
        quiet_region_mean_chroma_diff: if quiet_region_samples == 0 {
            0.0
        } else {
            quiet_region_mean_chroma_diff / quiet_region_samples as f32
        },
        dark_quiet_region_mean_luma_diff: if dark_quiet_region_samples == 0 {
            0.0
        } else {
            dark_quiet_region_mean_luma_diff / dark_quiet_region_samples as f32
        },
        dark_quiet_region_mean_chroma_diff: if dark_quiet_region_samples == 0 {
            0.0
        } else {
            dark_quiet_region_mean_chroma_diff / dark_quiet_region_samples as f32
        },
    }
}

fn chroma_distance(u0: f32, v0: f32, u1: f32, v1: f32) -> f32 {
    let du = u0 - u1;
    let dv = v0 - v1;
    (du * du + dv * dv).sqrt()
}

fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let u = (b - y) * 0.492_111;
    let v = (r - y) * 0.877_283;
    (y, u, v)
}

fn sample_yuv(frame: &ImageFrame, width: usize, x: usize, y: usize) -> (f32, f32, f32) {
    let pixel_index = (y * width + x) * 4;
    rgb_to_yuv(
        frame.data[pixel_index],
        frame.data[pixel_index + 1],
        frame.data[pixel_index + 2],
    )
}

fn build_image<F>(size: FrameSize, mut pixel: F) -> ImageFrame
where
    F: FnMut(u32, u32) -> [u8; 4],
{
    let mut data = Vec::with_capacity(size.pixels() as usize * 4);
    for y in 0..size.height {
        for x in 0..size.width {
            data.extend_from_slice(&pixel(x, y));
        }
    }

    ImageFrame::rgba8(size, data).expect("generated calibration image should be valid")
}

fn colored_edges_image(size: FrameSize) -> ImageFrame {
    build_image(size, |x, y| {
        let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        let mut rgb = [0.06, 0.06, 0.07];

        if x > 12 && x < size.width / 2 - 8 && y > 16 && y < size.height / 2 {
            rgb = [0.88, 0.18, 0.16];
        } else if x >= size.width / 2 && x < size.width - 14 && y > 16 && y < size.height / 2 {
            rgb = [0.18, 0.72, 0.24];
        } else if x > 18 && x < size.width / 2 && y >= size.height / 2 {
            rgb = [0.18, 0.32, 0.90];
        } else if x >= size.width / 2 && x < size.width - 18 && y >= size.height / 2 {
            rgb = [0.90, 0.82, 0.18];
        }

        let diagonal = (fy - fx * 0.72 - 0.18).abs();
        if diagonal < 0.018 {
            rgb = [0.12, 0.92, 0.92];
        }

        let ring_dx = fx - 0.76;
        let ring_dy = fy - 0.32;
        let ring = (ring_dx * ring_dx + ring_dy * ring_dy).sqrt();
        if ring > 0.10 && ring < 0.16 {
            rgb = [0.96, 0.18, 0.82];
        }

        rgba8(rgb)
    })
}

fn portrait_midtones_image(size: FrameSize) -> ImageFrame {
    build_image(size, |x, y| {
        let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        let mut rgb = [0.24 + fx * 0.06, 0.28 + fy * 0.06, 0.32 + fx * 0.04];

        let face_dx = (fx - 0.5) / 0.24;
        let face_dy = (fy - 0.50) / 0.34;
        let face_mask = 1.0 - (face_dx * face_dx + face_dy * face_dy).clamp(0.0, 1.0);
        if face_mask > 0.0 {
            let cheek = 1.0 - ((fx - 0.5) * 3.8).abs();
            let face_light = 0.62 + 0.12 * (1.0 - (fy - 0.42).abs() * 1.8).clamp(0.0, 1.0);
            rgb = [
                0.73 + cheek * 0.05,
                0.56 + cheek * 0.04 + face_light * 0.03,
                0.47 + face_light * 0.02,
            ];
        }

        let hair_band = (fy - 0.18).abs() < 0.10 && (fx - 0.5).abs() < 0.22;
        if hair_band {
            rgb = [0.20, 0.13, 0.10];
        }

        if (fx - 0.42).abs() < 0.03 && (fy - 0.46).abs() < 0.012
            || (fx - 0.58).abs() < 0.03 && (fy - 0.46).abs() < 0.012
        {
            rgb = [0.16, 0.12, 0.11];
        }

        if (fx - 0.5).abs() < 0.015 && fy > 0.48 && fy < 0.63 {
            rgb = [0.58, 0.42, 0.34];
        }

        if (fx - 0.5).abs() < 0.06 && (fy - 0.70).abs() < 0.018 {
            rgb = [0.60, 0.26, 0.26];
        }

        rgba8(rgb)
    })
}

fn bright_highlights_image(size: FrameSize) -> ImageFrame {
    build_image(size, |x, y| {
        let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        let mut rgb = [0.04 + fx * 0.06, 0.04 + fx * 0.05, 0.05 + fy * 0.06];

        if y < size.height / 3 {
            let shoulder = 0.72 + fx * 0.28;
            rgb = [shoulder, shoulder * 0.98, shoulder * 0.95];
        }

        let spec_dx = fx - 0.32;
        let spec_dy = fy - 0.62;
        let spec = (spec_dx * spec_dx + spec_dy * spec_dy).sqrt();
        if spec < 0.11 {
            let glow = (1.0 - spec / 0.11).clamp(0.0, 1.0);
            rgb = [0.82 + glow * 0.18, 0.80 + glow * 0.18, 0.76 + glow * 0.20];
        }

        if x > size.width / 2 + 10
            && x < size.width - 16
            && y > size.height / 2
            && y < size.height - 18
        {
            let ramp =
                ((x - (size.width / 2 + 10)) as f32 / (size.width / 2 - 26) as f32).clamp(0.0, 1.0);
            let hot = 0.78 + ramp * 0.22;
            rgb = [hot, hot * 0.92 + 0.05, hot * 0.62 + 0.18];
        }

        rgba8(rgb)
    })
}

fn neutral_low_saturation_image(size: FrameSize) -> ImageFrame {
    build_image(size, |x, y| {
        let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        let tile = ((x / 20) + (y / 20)) % 2;
        let checker = if tile == 0 { 0.0 } else { 0.035 };
        let base = 0.28 + fx * 0.28 + fy * 0.06;
        let cool = 0.01 * (1.0 - fy);
        let warm = 0.015 * fx;
        let rgb = [base + warm + checker, base + checker * 0.8, base + cool];
        rgba8(rgb)
    })
}

fn ui_detail_edges_image(size: FrameSize) -> ImageFrame {
    build_image(size, |x, y| {
        let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        let mut rgb = [0.94, 0.95, 0.97];

        if x > 10 && x < size.width - 10 && y > 10 && y < size.height - 10 {
            rgb = [0.86, 0.88, 0.90];
        }

        if y % 8 == 0 || x % 16 == 0 {
            rgb = [0.18, 0.20, 0.23];
        }

        if y > 22 && y < 30 && x > 18 && x < size.width - 18 {
            rgb = [0.12, 0.13, 0.15];
        }

        if y > 40 && y < 44 && x > 22 && x < size.width - 28 {
            rgb = [0.08, 0.09, 0.10];
        }

        if y > 58 && y < 64 {
            let segment = (x / 6) % 3;
            rgb = match segment {
                0 => [0.90, 0.20, 0.20],
                1 => [0.16, 0.66, 0.22],
                _ => [0.18, 0.34, 0.88],
            };
        }

        if fy > 0.70 && fy < 0.84 && fx > 0.16 && fx < 0.86 {
            rgb = [0.97, 0.97, 0.98];
            if x % 5 == 0 {
                rgb = [0.06, 0.06, 0.07];
            }
        }

        rgba8(rgb)
    })
}

fn dark_quiet_floor_image(size: FrameSize) -> ImageFrame {
    build_image(size, |x, y| {
        let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        let mut rgb = [
            0.035 + fy * 0.020,
            0.038 + fy * 0.018,
            0.044 + fy * 0.026,
        ];

        let vertical_band = (fx - 0.76).abs();
        if vertical_band < 0.08 {
            let band = (1.0 - vertical_band / 0.08).clamp(0.0, 1.0);
            rgb[0] += band * 0.028;
            rgb[1] += band * 0.024;
            rgb[2] += band * 0.040;
        }

        let logo_dx = fx - 0.42;
        let logo_dy = fy - 0.34;
        let logo_radius = (logo_dx * logo_dx + logo_dy * logo_dy).sqrt();
        if logo_radius < 0.12 {
            let ring = ((0.12 - logo_radius) / 0.08).clamp(0.0, 1.0);
            rgb = [
                0.12 + ring * 0.62,
                0.11 + ring * 0.54,
                0.10 + ring * 0.30,
            ];
            if logo_radius < 0.055 {
                rgb = [0.86, 0.84, 0.80];
            }
        }

        if x > size.width / 2 - 8
            && x < size.width / 2 + 10
            && y > size.height - 18
            && y < size.height - 8
        {
            rgb = [0.78, 0.78, 0.80];
        }

        if x > 12 && x < 20 && y > size.height - 26 && y < size.height - 10 {
            rgb = [0.06, 0.07, 0.08];
            if y % 4 == 0 {
                rgb = [0.78, 0.79, 0.82];
            }
        }

        rgba8(rgb)
    })
}

fn rgba8(rgb: [f32; 3]) -> [u8; 4] {
    [
        (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        255,
    ]
}

#[test]
fn representative_calibration_cases_preserve_signal_first_hierarchy_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };

    for case in CALIBRATION_CASES {
        let input = case.input();
        let output = render_with_default_pipeline(&gpu, &input);
        let metrics = calibration_metrics(&input, &output);

        println!(
            "{}: luma_diff={:.4} chroma_diff={:.4} luma_retention={:.4} chroma_retention={:.4} highlight_band_luma_diff={:.4} quiet_luma_diff={:.4} quiet_chroma_diff={:.4} dark_quiet_luma_diff={:.4} dark_quiet_chroma_diff={:.4}",
            case.key(),
            metrics.mean_luma_diff,
            metrics.mean_chroma_diff,
            metrics.luma_edge_retention(),
            metrics.chroma_edge_retention(),
            metrics.highlight_band_luma_diff,
            metrics.quiet_region_mean_luma_diff,
            metrics.quiet_region_mean_chroma_diff,
            metrics.dark_quiet_region_mean_luma_diff,
            metrics.dark_quiet_region_mean_chroma_diff,
        );

        assert_images_not_identical(&input, &output);
        case.assert_signal_first_hierarchy(metrics);
    }
}

#[test]
fn compiled_runtime_matches_direct_gpu_path_on_calibration_cases_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let runtime = StillPipelineRuntime::new(&gpu);
    let pipeline = default_pipeline();

    for case in CALIBRATION_CASES {
        let input = case.input();
        let direct = pipeline
            .process_with_gpu(&gpu, &input)
            .unwrap_or_else(|error| panic!("{} should render on direct path: {error}", case.key()));
        let reused = render_with_runtime(&runtime, &pipeline, &input);

        assert_eq!(direct, reused, "{} runtime output drifted", case.key());
    }
}

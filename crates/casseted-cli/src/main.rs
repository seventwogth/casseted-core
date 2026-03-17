use casseted_pipeline::{PipelineError, StillImagePipeline};
use casseted_types::{FrameDescriptor, FrameSize, ImageDataError, ImageFrame, PixelFormat};
use image::{ImageFormat, ImageReader, RgbaImage};
use std::fmt;
use std::path::{Path, PathBuf};

const USAGE: &str = "\
Usage:
  casseted-cli <input.png> <output.png> [options]

Options:
  --luma-blur <f32>      Override horizontal luma blur amount in pixels
  --chroma-offset <f32>  Override chroma-like horizontal offset in pixels
  --chroma-bleed <f32>   Override chroma bleed amount in pixels
  --luma-noise <f32>     Override luma noise amount
  --chroma-noise <f32>   Override chroma noise amount
  --line-jitter <f32>    Override line jitter amount in pixels
  -h, --help             Show this help message

Notes:
  - The current CLI reads PNG input and writes PNG output.
  - If no effect flags are provided, the built-in mild analog defaults are used.
";

#[derive(Debug, Clone, PartialEq)]
struct CliArgs {
    input: PathBuf,
    output: PathBuf,
    luma_blur: Option<f32>,
    chroma_offset: Option<f32>,
    chroma_bleed: Option<f32>,
    luma_noise: Option<f32>,
    chroma_noise: Option<f32>,
    line_jitter: Option<f32>,
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(std::io::Error),
    Image(image::ImageError),
    ImageData(ImageDataError),
    Pipeline(PipelineError),
    InvalidImageBuffer,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => write!(f, "{message}\n\n{USAGE}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Image(error) => write!(f, "{error}"),
            Self::ImageData(error) => write!(f, "{error}"),
            Self::Pipeline(error) => write!(f, "{error}"),
            Self::InvalidImageBuffer => {
                f.write_str("failed to rebuild output image buffer from pipeline output")
            }
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<image::ImageError> for CliError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(value)
    }
}

impl From<ImageDataError> for CliError {
    fn from(value: ImageDataError) -> Self {
        Self::ImageData(value)
    }
}

impl From<PipelineError> for CliError {
    fn from(value: PipelineError) -> Self {
        Self::Pipeline(value)
    }
}

fn main() {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => {}
        Err(CliError::Usage(message)) if message == "help requested" => {
            println!("{USAGE}");
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn run(args: Vec<String>) -> Result<(), CliError> {
    let cli = parse_args(args)?;
    let input = load_png(&cli.input)?;
    let pipeline = pipeline_from_args(&cli);
    let output = pipeline.process_blocking(&input)?;
    let effective_signal = pipeline.effective_preview_signal();

    save_png(&cli.output, output)?;

    println!("input:  {}", cli.input.display());
    println!("output: {}", cli.output.display());
    println!("shader: {}", pipeline.shader_id.label());
    let mut guardrail_changes = Vec::new();
    if pipeline.signal.luma.blur_px != effective_signal.luma.blur_px {
        guardrail_changes.push("luma_blur");
    }
    if pipeline.signal.chroma.offset_px != effective_signal.chroma.offset_px {
        guardrail_changes.push("chroma_offset");
    }
    if pipeline.signal.chroma.bleed_px != effective_signal.chroma.bleed_px {
        guardrail_changes.push("chroma_bleed");
    }
    if pipeline.signal.noise.luma_amount != effective_signal.noise.luma_amount {
        guardrail_changes.push("luma_noise");
    }
    if pipeline.signal.noise.chroma_amount != effective_signal.noise.chroma_amount {
        guardrail_changes.push("chroma_noise");
    }
    if pipeline.signal.tracking.line_jitter_px != effective_signal.tracking.line_jitter_px {
        guardrail_changes.push("line_jitter");
    }
    if !guardrail_changes.is_empty() {
        println!(
            "preview-guardrails: softened {}",
            guardrail_changes.join(", ")
        );
    }
    println!(
        "effect: highlight_knee={} highlight_compression={} luma_blur={} chroma_offset={} chroma_bleed={} chroma_saturation={} luma_noise={} chroma_noise={} line_jitter={}",
        effective_signal.tone.highlight_soft_knee,
        effective_signal.tone.highlight_compression,
        effective_signal.luma.blur_px,
        effective_signal.chroma.offset_px,
        effective_signal.chroma.bleed_px,
        effective_signal.chroma.saturation,
        effective_signal.noise.luma_amount,
        effective_signal.noise.chroma_amount,
        effective_signal.tracking.line_jitter_px
    );

    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<CliArgs, CliError> {
    if args.is_empty() {
        return Err(CliError::Usage(
            "missing input and output image paths".to_owned(),
        ));
    }

    let mut positional = Vec::new();
    let mut parsed = CliArgs {
        input: PathBuf::new(),
        output: PathBuf::new(),
        luma_blur: None,
        chroma_offset: None,
        chroma_bleed: None,
        luma_noise: None,
        chroma_noise: None,
        line_jitter: None,
    };

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(CliError::Usage("help requested".to_owned())),
            "--luma-blur" => parsed.luma_blur = Some(parse_f32_flag("--luma-blur", &mut iter)?),
            "--chroma-offset" => {
                parsed.chroma_offset = Some(parse_f32_flag("--chroma-offset", &mut iter)?)
            }
            "--chroma-bleed" => {
                parsed.chroma_bleed = Some(parse_f32_flag("--chroma-bleed", &mut iter)?)
            }
            "--luma-noise" => parsed.luma_noise = Some(parse_f32_flag("--luma-noise", &mut iter)?),
            "--chroma-noise" => {
                parsed.chroma_noise = Some(parse_f32_flag("--chroma-noise", &mut iter)?)
            }
            "--line-jitter" => {
                parsed.line_jitter = Some(parse_f32_flag("--line-jitter", &mut iter)?)
            }
            value if value.starts_with('-') => {
                return Err(CliError::Usage(format!("unknown flag: {value}")));
            }
            value => positional.push(PathBuf::from(value)),
        }
    }

    if positional.len() != 2 {
        return Err(CliError::Usage(
            "expected input and output image paths".to_owned(),
        ));
    }

    parsed.input = positional.remove(0);
    parsed.output = positional.remove(0);
    Ok(parsed)
}

fn parse_f32_flag(flag: &str, iter: &mut impl Iterator<Item = String>) -> Result<f32, CliError> {
    let value = iter
        .next()
        .ok_or_else(|| CliError::Usage(format!("missing value for {flag}")))?;

    value
        .parse::<f32>()
        .map_err(|_| CliError::Usage(format!("invalid float for {flag}: {value}")))
}

fn pipeline_from_args(args: &CliArgs) -> StillImagePipeline {
    let mut pipeline = StillImagePipeline::default();

    if let Some(value) = args.luma_blur {
        pipeline.signal.luma.blur_px = value;
    }
    if let Some(value) = args.chroma_offset {
        pipeline.signal.chroma.offset_px = value;
    }
    if let Some(value) = args.chroma_bleed {
        pipeline.signal.chroma.bleed_px = value;
    }
    if let Some(value) = args.luma_noise {
        pipeline.signal.noise.luma_amount = value;
    }
    if let Some(value) = args.chroma_noise {
        pipeline.signal.noise.chroma_amount = value;
    }
    if let Some(value) = args.line_jitter {
        pipeline.signal.tracking.line_jitter_px = value;
    }

    pipeline
}

fn load_png(path: &Path) -> Result<ImageFrame, CliError> {
    let image = ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?
        .to_rgba8();
    let (width, height) = image.dimensions();

    ImageFrame::new(
        FrameDescriptor::new(FrameSize::new(width, height), PixelFormat::Rgba8Unorm, 0),
        image.into_raw(),
    )
    .map_err(CliError::ImageData)
}

fn save_png(path: &Path, image: ImageFrame) -> Result<(), CliError> {
    let width = image.descriptor.size.width;
    let height = image.descriptor.size.height;
    let rgba = RgbaImage::from_raw(width, height, image.into_bytes())
        .ok_or(CliError::InvalidImageBuffer)?;

    rgba.save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CliError, parse_args, run};
    use casseted_pipeline::PipelineError;
    use casseted_testing::{assert_images_not_identical, gradient_rgba8_image};
    use casseted_types::{FrameDescriptor, ImageFrame, PixelFormat};
    use image::{ImageFormat, RgbaImage};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_args_reads_required_paths_and_overrides() {
        let args = parse_args(vec![
            "input.png".to_owned(),
            "output.png".to_owned(),
            "--luma-blur".to_owned(),
            "1.5".to_owned(),
            "--line-jitter".to_owned(),
            "0.8".to_owned(),
        ])
        .expect("args should parse");

        assert_eq!(args.input, PathBuf::from("input.png"));
        assert_eq!(args.output, PathBuf::from("output.png"));
        assert_eq!(args.luma_blur, Some(1.5));
        assert_eq!(args.line_jitter, Some(0.8));
    }

    #[test]
    fn parse_args_requires_two_positional_paths() {
        let error = parse_args(vec!["input.png".to_owned()]).expect_err("expected usage error");

        match error {
            CliError::Usage(message) => assert!(message.contains("expected input and output")),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn cli_smoke_processes_png_when_gpu_is_available() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be valid")
            .as_nanos();
        let input_path = std::env::temp_dir().join(format!("casseted-cli-input-{unique}.png"));
        let output_path = std::env::temp_dir().join(format!("casseted-cli-output-{unique}.png"));

        let source = gradient_rgba8_image(casseted_types::FrameSize::new(8, 8));
        let input = RgbaImage::from_raw(
            source.descriptor.size.width,
            source.descriptor.size.height,
            source.data.clone(),
        )
        .expect("test input should rebuild as an image");
        input
            .save_with_format(&input_path, ImageFormat::Png)
            .expect("input image should be written");

        let result = run(vec![
            input_path.display().to_string(),
            output_path.display().to_string(),
            "--luma-blur".to_owned(),
            "1.4".to_owned(),
            "--chroma-offset".to_owned(),
            "1.2".to_owned(),
        ]);

        if let Err(CliError::Pipeline(PipelineError::GpuInit(_))) = &result {
            let _ = std::fs::remove_file(&input_path);
            let _ = std::fs::remove_file(&output_path);
            return;
        }

        result.expect("cli run should succeed");

        let output = image::open(&output_path)
            .expect("output image should be readable")
            .to_rgba8();
        let output = ImageFrame::new(
            FrameDescriptor::new(
                source.descriptor.size,
                PixelFormat::Rgba8Unorm,
                source.descriptor.frame_index,
            ),
            output.into_raw(),
        )
        .expect("output image must rebuild as a frame");
        assert_images_not_identical(&source, &output);

        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }
}

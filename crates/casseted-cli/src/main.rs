use casseted_pipeline::StillImagePipeline;
use casseted_types::{FrameSize, ImageFrame};

fn main() {
    let pipeline = StillImagePipeline::default();
    let input = ImageFrame::solid_rgba8(FrameSize::new(8, 8), [128, 96, 80, 255]);

    println!("casseted-core workspace scaffold");
    println!(
        "demo frame: {}x{}",
        input.descriptor.size.width, input.descriptor.size.height
    );
    println!("shader: {}", pipeline.shader_id.label());
    println!(
        "signal: blur={} chroma_offset={} noise={}",
        pipeline.signal.luma.blur_px,
        pipeline.signal.chroma.offset_px,
        pipeline.signal.noise.luma_amount
    );
}

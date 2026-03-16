use casseted_pipeline::PipelineDefinition;
use casseted_types::FrameDescriptor;

fn main() {
    let pipeline = PipelineDefinition::signal_preview(FrameDescriptor::default());
    let shader = pipeline.shader();

    println!("casseted-core workspace scaffold");
    println!(
        "default frame: {}x{}",
        pipeline.frame.size.width, pipeline.frame.size.height
    );
    println!("preset: {:?}", pipeline.preset);
    println!("shader: {}", shader.name);
}

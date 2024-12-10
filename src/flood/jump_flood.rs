use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    prelude::*,
    render::{
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedRenderPipelineId,
            FragmentState, Operations, PipelineCache, RenderPassColorAttachment,
            RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, Sampler,
            SamplerDescriptor, ShaderType, UniformBuffer,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::CachedTexture,
    },
};
use wgpu_types::{
    ColorTargetState, ColorWrites, MultisampleState, PrimitiveState, SamplerBindingType,
    ShaderStages, TextureFormat, TextureSampleType,
};

use super::JUMP_FLOOD_SHADER_HANDLE;

#[derive(ShaderType)]
pub(crate) struct JumpFloodUniform {
    #[align(16)]
    pub(crate) size: u32,
}

#[derive(Resource)]
pub(crate) struct JumpFloodPipeline {
    pub(crate) layout: BindGroupLayout,
    pub(crate) sampler: Sampler,
    pub(crate) pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for JumpFloodPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "outline_jump_flood_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<JumpFloodUniform>(false),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("outline_jump_flood_pipeline".into()),
                    layout: vec![layout.clone()],
                    vertex: fullscreen_shader_vertex_state(),
                    fragment: Some(FragmentState {
                        shader: JUMP_FLOOD_SHADER_HANDLE,
                        shader_defs: vec![],
                        entry_point: "fragment".into(),
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    push_constant_ranges: vec![],
                    zero_initialize_workgroup_memory: false,
                });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

pub(crate) fn jump_flood_pass(
    pipeline: &JumpFloodPipeline,
    render_queue: &RenderQueue,
    render_pipeline: &RenderPipeline,
    render_context: &mut RenderContext<'_>,
    input: &CachedTexture,
    output: &CachedTexture,
    size: u32,
) {
    let mut uniform_buffer = UniformBuffer::from(JumpFloodUniform { size });

    uniform_buffer.write_buffer(render_context.render_device(), render_queue);

    let bind_group = render_context.render_device().create_bind_group(
        "outline_jump_flood_bind_group",
        &pipeline.layout,
        &BindGroupEntries::sequential((
            &input.default_view,
            &pipeline.sampler,
            uniform_buffer.binding().unwrap(),
        )),
    );

    let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("outline_jump_flood_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &output.default_view,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    render_pass.set_render_pipeline(render_pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

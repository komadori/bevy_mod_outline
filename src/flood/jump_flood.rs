use bevy::{
    core_pipeline::FullscreenShader,
    prelude::*,
    render::{
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, DynamicUniformBuffer, FragmentState, Operations, PipelineCache,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
            RenderPipelineDescriptor, Sampler, SamplerDescriptor, ShaderType,
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
    pub(crate) size: u32,
    _padding: Vec3,
}

#[derive(Resource)]
pub(crate) struct JumpFloodPipeline {
    pub(crate) layout: BindGroupLayoutDescriptor,
    pub(crate) sampler: Sampler,
    pub(crate) pipeline_id: CachedRenderPipelineId,
    pub(crate) lookup_buffer: DynamicUniformBuffer<JumpFloodUniform>,
    pub(crate) lookup_offsets: Vec<u32>,
}

pub(crate) fn init_jump_flood_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "outline_jump_flood_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<JumpFloodUniform>(true),
            ),
        ),
    );

    let sampler = render_device.create_sampler(&SamplerDescriptor::default());

    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("outline_jump_flood_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: JUMP_FLOOD_SHADER_HANDLE,
            shader_defs: vec![],
            entry_point: None,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        immediate_size: 0,
        zero_initialize_workgroup_memory: false,
    });

    let mut uniform_buffer = DynamicUniformBuffer::new_with_alignment(
        render_device.limits().min_uniform_buffer_offset_alignment as u64,
    );
    let mut offsets = Vec::new();
    for bit in 0..32 {
        offsets.push(uniform_buffer.push(&JumpFloodUniform {
            size: 1 << bit,
            _padding: Vec3::default(),
        }));
    }
    uniform_buffer.write_buffer(&render_device, &render_queue);

    commands.insert_resource(JumpFloodPipeline {
        layout,
        sampler,
        pipeline_id,
        lookup_buffer: uniform_buffer,
        lookup_offsets: offsets,
    });
}

pub(crate) struct JumpFloodPass<'w> {
    pipeline: &'w JumpFloodPipeline,
    render_pipeline: &'w RenderPipeline,
}

impl<'w> JumpFloodPass<'w> {
    pub fn new(world: &'w World) -> Option<Self> {
        let pipeline = world.resource::<JumpFloodPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let render_pipeline = pipeline_cache.get_render_pipeline(pipeline.pipeline_id)?;

        Some(Self {
            pipeline,
            render_pipeline,
        })
    }

    pub fn execute(
        &mut self,
        render_context: &mut RenderContext<'_, '_>,
        input: &CachedTexture,
        output: &CachedTexture,
        pipeline_cache: &PipelineCache,
        size: u32,
        bounds: &URect,
    ) {
        let bind_group = render_context.render_device().create_bind_group(
            "outline_jump_flood_bind_group",
            &pipeline_cache.get_bind_group_layout(&self.pipeline.layout),
            &BindGroupEntries::sequential((
                &input.default_view,
                &self.pipeline.sampler,
                self.pipeline.lookup_buffer.binding().unwrap(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("outline_jump_flood_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &output.default_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_scissor_rect(bounds.min.x, bounds.min.y, bounds.width(), bounds.height());
        render_pass.set_render_pipeline(self.render_pipeline);
        render_pass.set_bind_group(
            0,
            &bind_group,
            &[self.pipeline.lookup_offsets[size as usize]],
        );
        render_pass.draw(0..3, 0..1);
    }
}

use bevy::{
    core_pipeline::FullscreenShader,
    prelude::*,
    render::{
        render_resource::{
            binding_types::texture_2d, BindGroupEntries, BindGroupLayoutDescriptor,
            BindGroupLayoutEntries, CachedRenderPipelineId, FragmentState, Operations,
            PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
            RenderPipelineDescriptor,
        },
        renderer::{RenderContext, RenderDevice},
        texture::CachedTexture,
    },
};
use wgpu_types::{
    ColorTargetState, ColorWrites, MultisampleState, PrimitiveState, ShaderStages, TextureFormat,
    TextureSampleType,
};

use super::SOBEL_INIT_SHADER_HANDLE;

#[derive(Resource)]
pub(crate) struct SobelInitPipeline {
    pub(crate) layout: BindGroupLayoutDescriptor,
    pub(crate) pipeline_id: CachedRenderPipelineId,
}

pub(crate) fn init_sobel_init_pipeline(
    mut commands: Commands,
    _render_device: Res<RenderDevice>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "outline_flood_sobel_init_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Float { filterable: false }),),
        ),
    );

    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("outline_flood_sobel_init_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: SOBEL_INIT_SHADER_HANDLE,
            shader_defs: vec![],
            entry_point: None,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rg16Float,
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

    commands.insert_resource(SobelInitPipeline {
        layout,
        pipeline_id,
    });
}

pub(crate) struct SobelInitPass<'w> {
    pipeline: &'w SobelInitPipeline,
    render_pipeline: &'w RenderPipeline,
}

impl<'w> SobelInitPass<'w> {
    pub fn new(world: &'w World) -> Option<Self> {
        let pipeline = world.resource::<SobelInitPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let render_pipeline = pipeline_cache.get_render_pipeline(pipeline.pipeline_id)?;
        Some(Self {
            pipeline,
            render_pipeline,
        })
    }

    pub fn execute(
        &self,
        render_context: &mut RenderContext<'_, '_>,
        coverage: &CachedTexture,
        output: &CachedTexture,
        pipeline_cache: &PipelineCache,
        bounds: &URect,
    ) {
        let bind_group = render_context.render_device().create_bind_group(
            "outline_flood_sobel_init_bind_group",
            &pipeline_cache.get_bind_group_layout(&self.pipeline.layout),
            &BindGroupEntries::single(&coverage.default_view),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("outline_flood_sobel_init_pass"),
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
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

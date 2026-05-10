use bevy::{
    core_pipeline::blit::{BlitPipeline, BlitPipelineKey},
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
    },
    render::{
        render_resource::{
            CachedRenderPipelineId, LoadOp, Operations, PipelineCache, RenderPassColorAttachment,
            RenderPassDescriptor, SpecializedRenderPipelines, StoreOp,
        },
        renderer::{RenderContext, ViewQuery},
        view::{Msaa, ViewTarget},
    },
};

#[derive(Component)]
pub(crate) struct MsaaExtraWritebackPipeline(CachedRenderPipelineId);

pub(crate) fn msaa_extra_writeback_pass(
    view: ViewQuery<(&ViewTarget, &MsaaExtraWritebackPipeline)>,
    blit_pipeline: Res<BlitPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut render_context: RenderContext,
) {
    let (target, blit_pipeline_id) = view.into_inner();

    let Some(pipeline) = pipeline_cache.get_render_pipeline(blit_pipeline_id.0) else {
        return;
    };

    let post_process = target.post_process_write();

    let pass_descriptor = RenderPassDescriptor {
        label: Some("msaa_extra_writeback"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: target.sampled_main_texture_view().unwrap(),
            depth_slice: None,
            resolve_target: Some(post_process.destination),
            ops: Operations {
                load: LoadOp::Clear(wgpu_types::Color::BLACK),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };

    let bind_group = blit_pipeline.create_bind_group(
        render_context.render_device(),
        post_process.source,
        &pipeline_cache,
    );

    let mut render_pass = render_context
        .command_encoder()
        .begin_render_pass(&pass_descriptor);

    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

pub(crate) fn prepare_msaa_extra_writeback_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BlitPipeline>>,
    blit_pipeline: Res<BlitPipeline>,
    view_targets: Query<(Entity, &ViewTarget, &Msaa)>,
) {
    for (entity, view_target, msaa) in view_targets.iter() {
        if *msaa != Msaa::Off {
            let key = BlitPipelineKey {
                target_format: view_target.main_texture_format(),
                samples: msaa.samples(),
                blend_state: None,
                source_space: None,
            };

            let pipeline = pipelines.specialize(&pipeline_cache, &blit_pipeline, key);
            commands
                .entity(entity)
                .insert(MsaaExtraWritebackPipeline(pipeline));
        } else {
            commands
                .entity(entity)
                .remove::<MsaaExtraWritebackPipeline>();
        }
    }
}

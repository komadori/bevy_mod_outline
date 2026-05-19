use bevy::prelude::*;
use bevy::{
    core_pipeline::blit::{BlitPipeline, BlitPipelineKey},
    ecs::query::QueryItem,
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    render::{
        camera::ExtractedCamera,
        extract_component::ExtractComponent,
        render_resource::{
            CachedRenderPipelineId, Extent3d, LoadOp, Operations, PipelineCache,
            RenderPassColorAttachment, RenderPassDescriptor, SpecializedRenderPipelines, StoreOp,
            TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        sync_component::SyncComponent,
        texture::{CachedTexture, TextureCache},
        view::{Msaa, ViewDepthTexture, ViewTarget},
    },
};

use crate::view_uniforms::OutlineViewUniform;
use crate::OutlineMsaa;

#[derive(Component, Copy, Clone, Deref)]
pub(crate) struct ResolvedOutlineMsaa(pub Msaa);

impl SyncComponent for ResolvedOutlineMsaa {
    type Target = ResolvedOutlineMsaa;
}

impl ExtractComponent for ResolvedOutlineMsaa {
    type QueryData = (&'static Msaa, Option<&'static OutlineMsaa>);
    type QueryFilter = ();
    type Out = ResolvedOutlineMsaa;

    fn extract_component(
        (msaa, outline_msaa): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        let resolved = match outline_msaa {
            Some(OutlineMsaa::Msaa(msaa)) => *msaa,
            _ => *msaa,
        };
        Some(ResolvedOutlineMsaa(resolved))
    }
}

#[derive(Component)]
pub(crate) struct OutlineViewTextures {
    pub color: Option<CachedTexture>,
    pub depth: ViewDepthTexture,
}

pub(crate) fn prepare_outline_view_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<
        (
            Entity,
            &Msaa,
            &ResolvedOutlineMsaa,
            &ExtractedCamera,
            &ViewTarget,
        ),
        With<OutlineViewUniform>,
    >,
) {
    for (entity, msaa, resolved, camera, view_target) in views.iter() {
        if resolved.0 == *msaa {
            commands.entity(entity).remove::<OutlineViewTextures>();
            continue;
        }

        let Some(target_size) = camera.physical_target_size else {
            commands.entity(entity).remove::<OutlineViewTextures>();
            continue;
        };

        let size = Extent3d {
            width: target_size.x,
            height: target_size.y,
            depth_or_array_layers: 1,
        };
        let samples = resolved.samples();

        let color = if samples > 1 {
            Some(texture_cache.get(
                &render_device,
                TextureDescriptor {
                    label: Some("outline_msaa_colour"),
                    size,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: TextureDimension::D2,
                    format: view_target.main_texture_format(),
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            ))
        } else {
            None
        };

        let depth = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("outline_msaa_depth"),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: TextureDimension::D2,
                format: TextureFormat::Depth32Float,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );

        commands.entity(entity).insert(OutlineViewTextures {
            color,
            depth: ViewDepthTexture::new(depth, Some(0.0)),
        });
    }
}

#[derive(Component)]
pub(crate) struct MsaaExtraWritebackPipeline(CachedRenderPipelineId);

pub(crate) fn msaa_extra_writeback_pass(
    view: ViewQuery<(
        &ViewTarget,
        &MsaaExtraWritebackPipeline,
        Option<&OutlineViewTextures>,
    )>,
    blit_pipeline: Res<BlitPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut render_context: RenderContext,
) {
    let (target, blit_pipeline_id, outline_textures) = view.into_inner();

    let Some(pipeline) = pipeline_cache.get_render_pipeline(blit_pipeline_id.0) else {
        return;
    };

    let post_process = target.post_process_write();

    let colour_view = match outline_textures.and_then(|t| t.color.as_ref()) {
        Some(colour) => &colour.default_view,
        None => target.sampled_main_texture_view().unwrap(),
    };

    let pass_descriptor = RenderPassDescriptor {
        label: Some("msaa_extra_writeback"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: colour_view,
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
    view_targets: Query<(Entity, &ViewTarget, &ResolvedOutlineMsaa)>,
) {
    for (entity, view_target, resolved) in view_targets.iter() {
        if **resolved != Msaa::Off {
            let key = BlitPipelineKey {
                target_format: view_target.main_texture_format(),
                samples: resolved.0.samples(),
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

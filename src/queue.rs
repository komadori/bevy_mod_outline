use bevy::core_pipeline::prepass::MotionVectorPrepass;
use bevy::ecs::component::Tick;
use bevy::prelude::*;
use bevy::render::batching::gpu_preprocessing::GpuPreprocessingSupport;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItemExtraIndex,
    ViewBinnedRenderPhases, ViewSortedRenderPhases,
};
use bevy::render::render_resource::{PipelineCache, SpecializedMeshPipelines};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, RenderLayers, RetainedViewEntity};

use crate::node::{
    NoPhaseItemBatchSetKey, OpaqueOutline, OutlineBinKey, StencilOutline, TransparentOutline,
};
use crate::pipeline::{OutlinePipeline, PassType, PipelineKey};
use crate::render::DrawOutline;
use crate::uniforms::{DrawMode, ExtractedOutline};
use crate::view_uniforms::OutlineQueueStatus;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    opaque_draw_functions: Res<DrawFunctions<OpaqueOutline>>,
    transparent_draw_functions: Res<DrawFunctions<TransparentOutline>>,
    outline_pipeline: Res<OutlinePipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    material_meshes: Query<(Entity, &MainEntity, &ExtractedOutline)>,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(
        &ExtractedView,
        Entity,
        Option<&RenderLayers>,
        Has<MotionVectorPrepass>,
        &Msaa,
        &mut OutlineQueueStatus,
    )>,
) {
    let draw_stencil = stencil_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_opaque_outline = opaque_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_transparent_outline = transparent_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    for (view, view_entity, view_mask, motion_vector_prepass, msaa, mut queue_status) in
        views.iter_mut()
    {
        let base_key = PipelineKey::new().with_msaa(*msaa);
        let view_mask = view_mask.cloned().unwrap_or_default();
        let world_from_view = view.world_from_view.affine().matrix3;
        let rangefinder = view.rangefinder3d();
        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            stencil_phases.get_mut(&RetainedViewEntity::new(view_entity.into(), None, 0)),
            opaque_phases.get_mut(&RetainedViewEntity::new(view_entity.into(), None, 0)),
            transparent_phases.get_mut(&RetainedViewEntity::new(view_entity.into(), None, 0)),
        ) else {
            continue; // No render phase
        };
        for (entity, main_entity, outline) in material_meshes.iter() {
            if !view_mask.intersects(&outline.layers) {
                continue; // Layer not enabled
            }
            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue; // No mesh
            };
            let instance_base_key = base_key
                .with_primitive_topology(mesh.primitive_topology())
                .with_depth_mode(outline.depth_mode)
                .with_morph_targets(mesh.morph_targets.is_some())
                .with_motion_vector_prepass(motion_vector_prepass)
                .with_double_sided(outline.double_sided);
            if outline.stencil {
                let stencil_key = instance_base_key
                    .with_vertex_offset_zero(outline.instance_data.stencil_offset == 0.0)
                    .with_plane_offset_zero(outline.instance_data.world_plane_offset == Vec3::ZERO)
                    .with_pass_type(PassType::Stencil)
                    .with_alpha_mask_texture(outline.alpha_mask_id.is_some())
                    .with_alpha_mask_channel(outline.alpha_mask_channel);
                if let Ok(pipeline) = pipelines.specialize(
                    &pipeline_cache,
                    &outline_pipeline,
                    stencil_key,
                    &mesh.layout,
                ) {
                    stencil_phase.add(
                        NoPhaseItemBatchSetKey,
                        OutlineBinKey {
                            pipeline,
                            draw_function: draw_stencil,
                            asset_id: outline.mesh_id,
                            texture_id: outline.alpha_mask_id,
                        },
                        (entity, *main_entity),
                        InputUniformIndex(0),
                        BinnedRenderPhaseType::mesh(outline.automatic_batching, &GpuPreprocessingSupport { max_supported_mode: bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode::Culling }),
                        Tick::new(0),
                    );
                }
            }
            if outline.volume && outline.draw_mode == DrawMode::Extrude {
                queue_status.has_volume = true;
                let transparent = outline.instance_data.volume_colour[3] < 1.0;
                let draw_key = instance_base_key
                    .with_vertex_offset_zero(outline.instance_data.volume_offset == 0.0)
                    .with_plane_offset_zero(outline.instance_data.world_plane_offset == Vec3::ZERO)
                    .with_pass_type(if transparent {
                        PassType::Transparent
                    } else {
                        PassType::Opaque
                    })
                    .with_hdr_format(view.hdr);
                if let Ok(pipeline) =
                    pipelines.specialize(&pipeline_cache, &outline_pipeline, draw_key, &mesh.layout)
                {
                    if transparent {
                        let world_plane = outline.instance_data.world_plane_origin
                            + world_from_view.mul_vec3(-Vec3::Z)
                                * outline.instance_data.world_plane_offset;
                        let distance = rangefinder.distance_translation(&world_plane);
                        transparent_phase.add(TransparentOutline {
                            entity,
                            main_entity: *main_entity,
                            pipeline,
                            draw_function: draw_transparent_outline,
                            distance,
                            batch_range: 0..0,
                            extra_index: PhaseItemExtraIndex::None,
                        });
                    } else {
                        opaque_phase.add(
                            NoPhaseItemBatchSetKey,
                            OutlineBinKey {
                                pipeline,
                                draw_function: draw_opaque_outline,
                                asset_id: outline.mesh_id,
                                texture_id: outline.alpha_mask_id,
                            },
                            (entity, *main_entity),
                        InputUniformIndex(0),
                        BinnedRenderPhaseType::mesh(outline.automatic_batching, &GpuPreprocessingSupport { max_supported_mode: bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode::Culling }),
                        Tick::new(0),
                        );
                    }
                }
            }
        }
    }
}

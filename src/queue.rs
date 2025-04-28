use bevy::core_pipeline::prepass::MotionVectorPrepass;
use bevy::ecs::component::Tick;
use bevy::prelude::*;
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItemExtraIndex,
    ViewBinnedRenderPhases, ViewSortedRenderPhases,
};
use bevy::render::render_resource::{PipelineCache, SpecializedMeshPipelines};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, RenderLayers};

use crate::node::{
    OpaqueOutline, OutlineBatchSetKey, OutlineBinKey, StencilOutline, TransparentOutline,
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
    mesh_allocator: Res<MeshAllocator>,
    material_meshes: Query<(Entity, &MainEntity, &ExtractedOutline)>,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(
        &ExtractedView,
        Option<&RenderLayers>,
        Has<MotionVectorPrepass>,
        &Msaa,
        &mut OutlineQueueStatus,
    )>,
    mut change_tick: Local<Tick>,
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

    for (view, view_mask, motion_vector_prepass, msaa, mut queue_status) in views.iter_mut() {
        let base_key = PipelineKey::new().with_msaa(*msaa);
        let view_mask = view_mask.cloned().unwrap_or_default();
        let world_from_view = view.world_from_view.affine().matrix3;
        let rangefinder = view.rangefinder3d();
        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            stencil_phases.get_mut(&view.retained_view_entity),
            opaque_phases.get_mut(&view.retained_view_entity),
            transparent_phases.get_mut(&view.retained_view_entity),
        ) else {
            continue; // No render phase
        };
        let this_tick = change_tick.get() + 1;
        change_tick.set(this_tick);
        for (entity, main_entity, outline) in material_meshes.iter() {
            if !view_mask.intersects(&outline.layers) {
                continue; // Layer not enabled
            }
            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue; // No mesh
            };
            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&outline.mesh_id);
            let instance_base_key = base_key
                .with_primitive_topology(mesh.primitive_topology())
                .with_depth_mode(outline.depth_mode)
                .with_morph_targets(mesh.morph_targets.is_some())
                .with_motion_vector_prepass(motion_vector_prepass)
                .with_double_sided(outline.double_sided);
            let phase_type = if outline.automatic_batching {
                BinnedRenderPhaseType::BatchableMesh
            } else {
                BinnedRenderPhaseType::UnbatchableMesh
            };
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
                        OutlineBatchSetKey {
                            pipeline,
                            draw_function: draw_stencil,
                            material_bind_group_id: None,
                            vertex_slab: vertex_slab.unwrap_or_default(),
                            index_slab,
                        },
                        OutlineBinKey {
                            asset_id: outline.mesh_id,
                            texture_id: outline.alpha_mask_id,
                        },
                        (entity, *main_entity),
                        InputUniformIndex::default(),
                        phase_type,
                        *change_tick,
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
                            indexed: index_slab.is_some(),
                        });
                    } else {
                        opaque_phase.add(
                            OutlineBatchSetKey {
                                pipeline,
                                draw_function: draw_opaque_outline,
                                material_bind_group_id: Some(0),
                                vertex_slab: vertex_slab.unwrap_or_default(),
                                index_slab,
                            },
                            OutlineBinKey {
                                asset_id: outline.mesh_id,
                                texture_id: outline.alpha_mask_id,
                            },
                            (entity, *main_entity),
                            InputUniformIndex::default(),
                            phase_type,
                            *change_tick,
                        );
                    }
                }
            }
        }
    }
}

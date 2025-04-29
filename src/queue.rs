use bevy::core_pipeline::prepass::MotionVectorPrepass;
use bevy::ecs::component::Tick;
use bevy::ecs::system::SystemChangeTick;
use bevy::pbr::ViewSpecializationTicks;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItemExtraIndex,
    ViewBinnedRenderPhases, ViewSortedRenderPhases,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, PipelineCache, SpecializedMeshPipelines,
};
use bevy::render::sync_world::{MainEntity, MainEntityHashMap};
use bevy::render::view::{ExtractedView, RenderLayers, RetainedViewEntity};
use bevy::render::Extract;

use crate::node::{
    OpaqueOutline, OutlineBatchSetKey, OutlineBinKey, StencilOutline, TransparentOutline,
};
use crate::{
    pipeline::{OutlinePipeline, PassType, PipelineKey},
    render::DrawOutline,
    uniforms::{DrawMode, ExtractedOutline},
    view_uniforms::OutlineQueueStatus,
    ComputedOutline,
};

#[derive(Clone, Resource, Debug, Default)]
pub struct OutlineEntitiesNeedingSpecialisation {
    entities: Vec<Entity>,
}

#[derive(Resource, Deref, DerefMut, Clone, Debug, Default)]
pub struct OutlineEntitySpecialisationTicks {
    entity_map: MainEntityHashMap<Tick>,
}

#[derive(Resource, Default)]
pub struct OutlinePipelineCache {
    view_map: HashMap<RetainedViewEntity, OutlineViewPipelineCache>,
}

#[derive(Default)]
pub struct OutlineViewPipelineCache {
    entity_map: MainEntityHashMap<OutlinePipelineCacheEntry>,
}

#[derive(Debug, Clone, Copy)]
pub struct OutlinePipelineCacheEntry {
    pub tick: Tick,
    pub stencil_pipeline_id: CachedRenderPipelineId,
    pub volume_pipeline_id: CachedRenderPipelineId,
}

#[allow(clippy::type_complexity)]
pub(crate) fn check_outline_entities_needing_specialisation(
    needs_specialisation: Query<
        Entity,
        Or<(
            Changed<ComputedOutline>,
            Changed<Mesh3d>,
            AssetChanged<Mesh3d>,
        )>,
    >,
    mut entities_needing_specialisation: ResMut<OutlineEntitiesNeedingSpecialisation>,
) {
    entities_needing_specialisation.entities.clear();
    for entity in &needs_specialisation {
        entities_needing_specialisation.entities.push(entity);
    }
}

pub(crate) fn extract_outline_entities_needing_specialisation(
    entities_needing_specialisation: Extract<Res<OutlineEntitiesNeedingSpecialisation>>,
    mut entity_specialisation_ticks: ResMut<OutlineEntitySpecialisationTicks>,
    views: Query<&ExtractedView>,
    mut outline_pipeline_cache: ResMut<OutlinePipelineCache>,
    mut removed_outlines_query: Extract<RemovedComponents<ComputedOutline>>,
    ticks: SystemChangeTick,
) {
    for entity in entities_needing_specialisation.entities.iter() {
        // Update the entity's specialisation tick with this run's tick
        entity_specialisation_ticks.insert((*entity).into(), ticks.this_run());
    }

    for entity in removed_outlines_query.read() {
        for view in &views {
            if let Some(outline_view_pipeline_cache) = outline_pipeline_cache
                .view_map
                .get_mut(&view.retained_view_entity)
            {
                outline_view_pipeline_cache
                    .entity_map
                    .remove(&MainEntity::from(entity));
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn specialise_outlines(
    render_meshes: Res<RenderAssets<RenderMesh>>,
    entity_specialisation_ticks: Res<OutlineEntitySpecialisationTicks>,
    view_specialisation_ticks: Res<ViewSpecializationTicks>,
    mut outline_pipeline_cache: ResMut<OutlinePipelineCache>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut all_views: Local<HashSet<RetainedViewEntity>>,
    outline_pipeline: Res<OutlinePipeline>,
    pipeline_cache: Res<PipelineCache>,
    ticks: SystemChangeTick,
    views: Query<(&ExtractedView, Has<MotionVectorPrepass>, &Msaa)>,
    outlines: Query<(&MainEntity, &ExtractedOutline)>,
) {
    all_views.clear();

    for (view, motion_vector_prepass, msaa) in &views {
        all_views.insert(view.retained_view_entity);

        let base_key = PipelineKey::new().with_msaa(*msaa);

        let view_tick = view_specialisation_ticks
            .get(&view.retained_view_entity)
            .unwrap();
        let outline_view_pipeline_cache = outline_pipeline_cache
            .view_map
            .entry(view.retained_view_entity)
            .or_default();

        for (main_entity, outline) in outlines.iter() {
            let entity_tick = entity_specialisation_ticks
                .get(main_entity)
                .copied()
                .unwrap_or_default();

            let last_specialised_tick = outline_view_pipeline_cache
                .entity_map
                .get(main_entity)
                .map(|entry| entry.tick);

            let needs_specialisation = last_specialised_tick.is_none_or(|tick| {
                entity_tick.is_newer_than(tick, ticks.this_run())
                    || view_tick.is_newer_than(tick, ticks.this_run())
            });

            if !needs_specialisation {
                continue;
            }

            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue;
            };

            let base_instance_key = base_key
                .with_primitive_topology(mesh.primitive_topology())
                .with_depth_mode(outline.depth_mode)
                .with_morph_targets(mesh.morph_targets.is_some())
                .with_motion_vector_prepass(motion_vector_prepass)
                .with_double_sided(outline.double_sided);

            // Specialise stencil pipeline
            let stencil_pipeline_id = if outline.stencil {
                let stencil_key = base_instance_key
                    .with_vertex_offset_zero(outline.instance_data.stencil_offset == 0.0)
                    .with_plane_offset_zero(outline.instance_data.world_plane_offset == Vec3::ZERO)
                    .with_pass_type(PassType::Stencil)
                    .with_alpha_mask_texture(outline.alpha_mask_id.is_some())
                    .with_alpha_mask_channel(outline.alpha_mask_channel);

                match pipelines.specialize(
                    &pipeline_cache,
                    &outline_pipeline,
                    stencil_key,
                    &mesh.layout,
                ) {
                    Ok(pipeline_id) => pipeline_id,
                    Err(err) => {
                        error!("Failed to specialise stencil pipeline: {}", err);
                        CachedRenderPipelineId::INVALID
                    }
                }
            } else {
                CachedRenderPipelineId::INVALID
            };

            // Specialise volume pipeline if needed
            let volume_pipeline_id = if outline.volume && outline.draw_mode == DrawMode::Extrude {
                let transparent = outline.instance_data.volume_colour[3] < 1.0;
                let draw_key = base_instance_key
                    .with_vertex_offset_zero(outline.instance_data.volume_offset == 0.0)
                    .with_plane_offset_zero(outline.instance_data.world_plane_offset == Vec3::ZERO)
                    .with_pass_type(if transparent {
                        PassType::Transparent
                    } else {
                        PassType::Opaque
                    })
                    .with_hdr_format(view.hdr);

                match pipelines.specialize(
                    &pipeline_cache,
                    &outline_pipeline,
                    draw_key,
                    &mesh.layout,
                ) {
                    Ok(pipeline_id) => pipeline_id,
                    Err(err) => {
                        error!("Failed to specialise volume pipeline: {}", err);
                        CachedRenderPipelineId::INVALID
                    }
                }
            } else {
                CachedRenderPipelineId::INVALID
            };

            outline_view_pipeline_cache.entity_map.insert(
                *main_entity,
                OutlinePipelineCacheEntry {
                    tick: ticks.this_run(),
                    stencil_pipeline_id,
                    volume_pipeline_id,
                },
            );
        }
    }

    // Delete specialized pipelines belonging to views that have expired.
    outline_pipeline_cache
        .view_map
        .retain(|retained_view_entity, _| all_views.contains(retained_view_entity));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    opaque_draw_functions: Res<DrawFunctions<OpaqueOutline>>,
    transparent_draw_functions: Res<DrawFunctions<TransparentOutline>>,
    mesh_allocator: Res<MeshAllocator>,
    outline_pipeline_cache: Res<OutlinePipelineCache>,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(
        &ExtractedView,
        Option<&RenderLayers>,
        &mut OutlineQueueStatus,
    )>,
    outlines: Query<(Entity, &MainEntity, &ExtractedOutline)>,
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

    for (view, view_mask, mut queue_status) in views.iter_mut() {
        let view_mask = view_mask.cloned().unwrap_or_default();
        let world_from_view = view.world_from_view.affine().matrix3;
        let rangefinder = view.rangefinder3d();

        let outline_view_pipeline_cache = outline_pipeline_cache
            .view_map
            .get(&view.retained_view_entity)
            .unwrap();

        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            stencil_phases.get_mut(&view.retained_view_entity),
            opaque_phases.get_mut(&view.retained_view_entity),
            transparent_phases.get_mut(&view.retained_view_entity),
        ) else {
            continue; // No render phase
        };

        for (render_entity, main_entity, outline) in outlines.iter() {
            if !view_mask.intersects(&outline.layers) {
                continue; // Layer not enabled
            }

            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&outline.mesh_id);
            let phase_type = if outline.automatic_batching {
                BinnedRenderPhaseType::BatchableMesh
            } else {
                BinnedRenderPhaseType::UnbatchableMesh
            };

            let Some(OutlinePipelineCacheEntry {
                tick: last_specialised_tick,
                stencil_pipeline_id,
                volume_pipeline_id,
            }) = outline_view_pipeline_cache.entity_map.get(main_entity)
            else {
                continue;
            };

            // Queue stencil pass if needed
            if outline.stencil
                && !stencil_phase.validate_cached_entity(*main_entity, *last_specialised_tick)
            {
                stencil_phase.add(
                    OutlineBatchSetKey {
                        pipeline: *stencil_pipeline_id,
                        draw_function: draw_stencil,
                        vertex_slab: vertex_slab.unwrap_or_default(),
                        index_slab,
                    },
                    OutlineBinKey {
                        asset_id: outline.mesh_id,
                        texture_id: outline.alpha_mask_id,
                    },
                    (render_entity, *main_entity),
                    InputUniformIndex::default(),
                    phase_type,
                    *last_specialised_tick,
                );
            }

            // Queue volume pass if needed
            if outline.volume && outline.draw_mode == DrawMode::Extrude {
                queue_status.has_volume = true;
                let transparent = outline.instance_data.volume_colour[3] < 1.0;

                if transparent {
                    let world_plane = outline.instance_data.world_plane_origin
                        + world_from_view.mul_vec3(-Vec3::Z)
                            * outline.instance_data.world_plane_offset;
                    let distance = rangefinder.distance_translation(&world_plane);
                    transparent_phase.add(TransparentOutline {
                        entity: render_entity,
                        main_entity: *main_entity,
                        pipeline: *volume_pipeline_id,
                        draw_function: draw_transparent_outline,
                        distance,
                        batch_range: 0..0,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: index_slab.is_some(),
                    });
                } else if !opaque_phase.validate_cached_entity(*main_entity, *last_specialised_tick)
                {
                    opaque_phase.add(
                        OutlineBatchSetKey {
                            pipeline: *volume_pipeline_id,
                            draw_function: draw_opaque_outline,
                            vertex_slab: vertex_slab.unwrap_or_default(),
                            index_slab,
                        },
                        OutlineBinKey {
                            asset_id: outline.mesh_id,
                            texture_id: outline.alpha_mask_id,
                        },
                        (render_entity, *main_entity),
                        InputUniformIndex::default(),
                        phase_type,
                        *last_specialised_tick,
                    );
                }
            }
        }
    }
}

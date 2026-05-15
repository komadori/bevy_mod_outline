use bevy::core_pipeline::prepass::MotionVectorPrepass;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::camera::{DirtySpecializations, PendingQueues};
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
use bevy::render::sync_world::MainEntityHashMap;
use bevy::render::view::{ExtractedView, RetainedViewEntity};
use bevy::render::Extract;

use crate::node::{
    OpaqueOutline, OutlineBatchSetKey, OutlineBinKey, OutlineSortingInfo, StencilOutline,
    TransparentOutline,
};
use crate::pipeline_key::ComputedOutlineKey;
use crate::pipeline_key::{DerivedPipelineKey, EntityPipelineKey, PassType, ViewPipelineKey};
use crate::uniforms::RenderOutlineInstances;
use crate::{
    pipeline::OutlinePipeline, render::DrawOutline, uniforms::DrawMode,
    view_uniforms::OutlineQueueStatus,
};
use crate::{ComputedOutline, RenderOutlineEntities};

#[derive(Clone, Resource, Debug, Default)]
pub(crate) struct OutlineEntitiesNeedingSpecialisation {
    changed: Vec<Entity>,
    removed: Vec<Entity>,
}

#[derive(Resource, Default)]
pub(crate) struct OutlineCache {
    pub(crate) view_map: HashMap<RetainedViewEntity, OutlineViewCache>,
}

#[derive(Default)]
pub(crate) struct OutlineViewCache {
    pub(crate) entity_map: MainEntityHashMap<OutlineCacheEntry>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OutlineCacheEntry {
    pub(crate) stencil_pipeline_id: CachedRenderPipelineId,
    pub(crate) volume_pipeline_id: CachedRenderPipelineId,
}

#[derive(Default, Deref, DerefMut, Resource)]
pub(crate) struct PendingOutlineQueues(pub PendingQueues);

#[derive(Clone, Resource, Default, Deref, DerefMut)]
pub(crate) struct DirtyOutlineSpecialisations(pub DirtySpecializations);

pub(crate) fn clear_dirty_outline_specialisations(
    mut specialisations: ResMut<DirtyOutlineSpecialisations>,
) {
    specialisations.changed_renderables.clear();
    specialisations.removed_renderables.clear();
    specialisations.views.clear();
}

pub(crate) fn expire_outline_specialisations_for_views(
    views: Query<&ExtractedView>,
    mut specialisations: ResMut<DirtyOutlineSpecialisations>,
) {
    let all_live_retained_view_entities: HashSet<_> =
        views.iter().map(|view| view.retained_view_entity).collect();
    specialisations.views.retain(|retained_view_entity| {
        all_live_retained_view_entities.contains(retained_view_entity)
    });
}

#[allow(clippy::type_complexity)]
pub(crate) fn check_outline_entities_needing_specialisation(
    changed_entities: Query<Entity, Or<(Changed<ComputedOutline>, Changed<ComputedOutlineKey>)>>,
    mut removed_components: RemovedComponents<ComputedOutline>,
    mut needing_specialisation: ResMut<OutlineEntitiesNeedingSpecialisation>,
) {
    needing_specialisation.changed.clear();
    needing_specialisation.removed.clear();
    for entity in changed_entities.iter() {
        needing_specialisation.changed.push(entity);
    }
    for entity in removed_components.read() {
        needing_specialisation.removed.push(entity);
    }
}

pub(crate) fn extract_outline_entities_needing_specialisation(
    needing_specialisation: Extract<Res<OutlineEntitiesNeedingSpecialisation>>,
    mut specialisations: ResMut<DirtyOutlineSpecialisations>,
) {
    for &entity in needing_specialisation.changed.iter() {
        specialisations.changed_renderables.insert(entity.into());
    }
}

pub(crate) fn extract_outline_entities_needing_specialisation_removed(
    needing_specialisation: Extract<Res<OutlineEntitiesNeedingSpecialisation>>,
    mut specialisations: ResMut<DirtyOutlineSpecialisations>,
) {
    for &entity in needing_specialisation.removed.iter() {
        specialisations.removed_renderables.insert(entity.into());
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn specialise_outlines(
    render_meshes: Res<RenderAssets<RenderMesh>>,
    render_outlines: Res<RenderOutlineInstances>,
    render_visible: Res<RenderOutlineEntities>,
    mut pending_queues: ResMut<PendingOutlineQueues>,
    specialisations: Res<DirtyOutlineSpecialisations>,
    mut outline_cache: ResMut<OutlineCache>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut all_views: Local<HashSet<RetainedViewEntity>>,
    mut warm_up_keys: Local<Vec<EntityPipelineKey>>,
    outline_pipeline: Res<OutlinePipeline>,
    pipeline_cache: Res<PipelineCache>,
    views: Query<(&ExtractedView, Has<MotionVectorPrepass>, &Msaa)>,
) {
    all_views.clear();

    for (view, motion_vector_prepass, msaa) in &views {
        all_views.insert(view.retained_view_entity);

        let view_key = ViewPipelineKey::new()
            .with_msaa(*msaa)
            .with_target_format(view.target_format)
            .with_motion_vector_prepass(motion_vector_prepass);

        let outline_view_cache = outline_cache
            .view_map
            .entry(view.retained_view_entity)
            .or_default();

        if specialisations.must_wipe_specializations_for_view(view.retained_view_entity) {
            outline_view_cache.entity_map.clear();
        } else {
            for entity in specialisations.iter_to_despecialize() {
                outline_view_cache.entity_map.remove(entity);
            }
        }

        let Some(render_view_visible) = render_visible.views.get(&view.retained_view_entity) else {
            continue;
        };
        let view_pending_queues = pending_queues.prepare_for_new_frame(view.retained_view_entity);

        for (render_entity, main_entity) in specialisations.iter_to_specialize(
            view.retained_view_entity,
            &render_view_visible.0,
            &view_pending_queues.prev_frame,
        ) {
            let Some(outline) = render_outlines.get(main_entity) else {
                continue;
            };

            if outline_view_cache.entity_map.contains_key(main_entity) {
                continue; // Already in entity cache
            };

            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                view_pending_queues
                    .current_frame
                    .insert((*render_entity, *main_entity));
                continue; // Mesh not found
            };

            warm_up_keys.clear();
            warm_up_keys.push(outline.pipeline_key);

            if outline.warm_up.transparency {
                let range = 0..warm_up_keys.len();
                for i in range {
                    let key = warm_up_keys[i];
                    warm_up_keys.push(key.with_transparent(!key.transparent()));
                }
            }

            if outline.warm_up.vertex_offsets {
                let range = 0..warm_up_keys.len();
                for i in range {
                    let key = warm_up_keys[i];
                    warm_up_keys.push(
                        key.with_vertex_offset_zero(!key.vertex_offset_zero())
                            .with_stencil_vertex_offset_zero(!key.stencil_vertex_offset_zero()),
                    );
                }
            }

            let mut first_key = true;
            for warm_up_key in warm_up_keys.iter() {
                // Specialise stencil pipeline
                let stencil_pipeline_id = if outline.stencil {
                    let stencil_key =
                        DerivedPipelineKey::new(view_key, *warm_up_key, PassType::Stencil);

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

                // Specialise volume pipeline
                let volume_pipeline_id = if outline.volume {
                    let pass_type = match outline.draw_mode {
                        DrawMode::Extrude => PassType::Volume,
                        #[cfg(feature = "flood")]
                        DrawMode::JumpFlood => PassType::FloodInit,
                    };
                    let volume_key = DerivedPipelineKey::new(view_key, *warm_up_key, pass_type);

                    match pipelines.specialize(
                        &pipeline_cache,
                        &outline_pipeline,
                        volume_key,
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

                if first_key {
                    outline_view_cache.entity_map.insert(
                        *main_entity,
                        OutlineCacheEntry {
                            stencil_pipeline_id,
                            volume_pipeline_id,
                        },
                    );
                    first_key = false;
                }
            }
        }
    }

    // Delete specialized pipelines belonging to views that have expired.
    outline_cache
        .view_map
        .retain(|retained_view_entity, _| all_views.contains(retained_view_entity));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    opaque_draw_functions: Res<DrawFunctions<OpaqueOutline>>,
    transparent_draw_functions: Res<DrawFunctions<TransparentOutline>>,
    mesh_allocator: Res<MeshAllocator>,
    outline_cache: Res<OutlineCache>,
    render_outlines: Res<RenderOutlineInstances>,
    render_visible: Res<RenderOutlineEntities>,
    pending_queues: Res<PendingOutlineQueues>,
    specialisations: Res<DirtyOutlineSpecialisations>,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(&ExtractedView, &mut OutlineQueueStatus)>,
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

    for (view, mut queue_status) in views.iter_mut() {
        let outline_view_cache = outline_cache
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

        let Some(render_view_visible) = render_visible.views.get(&view.retained_view_entity) else {
            continue;
        };

        let Some(view_pending_queues) = pending_queues.get(&view.retained_view_entity) else {
            continue;
        };

        for &main_entity in
            specialisations.iter_to_dequeue(view.retained_view_entity, &render_view_visible.0)
        {
            stencil_phase.remove(main_entity);
            opaque_phase.remove(main_entity);
            transparent_phase.remove(Entity::PLACEHOLDER, main_entity);
        }

        for (_, main_entity) in specialisations.iter_to_queue(
            view.retained_view_entity,
            &render_view_visible.0,
            &view_pending_queues.prev_frame,
        ) {
            let Some(outline) = render_outlines.get(main_entity) else {
                continue;
            };

            let mesh_slabs = mesh_allocator.mesh_slabs(&outline.mesh_id);
            let vertex_slab = mesh_slabs.map(|s| s.vertex_slab_id);
            let index_slab = mesh_slabs.and_then(|s| s.index_slab_id);
            let phase_type = if outline.automatic_batching {
                BinnedRenderPhaseType::BatchableMesh
            } else {
                BinnedRenderPhaseType::UnbatchableMesh
            };

            let Some(OutlineCacheEntry {
                stencil_pipeline_id,
                volume_pipeline_id,
            }) = outline_view_cache.entity_map.get(main_entity)
            else {
                continue;
            };

            // Queue stencil pass if needed
            if outline.stencil {
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
                    (Entity::PLACEHOLDER, *main_entity),
                    InputUniformIndex::default(),
                    phase_type,
                );
            }

            // Queue volume pass if needed
            if outline.volume && outline.draw_mode == DrawMode::Extrude {
                let transparent = outline.instance_data.volume_colour[3] < 1.0;

                if transparent {
                    let sorting_info = OutlineSortingInfo {
                        world_plane_origin: outline.instance_data.world_plane_origin,
                        world_plane_offset: outline.instance_data.world_plane_offset,
                    };
                    transparent_phase.add(TransparentOutline {
                        sorting_info,
                        entity: Entity::PLACEHOLDER,
                        main_entity: *main_entity,
                        pipeline: *volume_pipeline_id,
                        draw_function: draw_transparent_outline,
                        distance: 0.0,
                        batch_range: 0..0,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: index_slab.is_some(),
                    });
                } else {
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
                        (Entity::PLACEHOLDER, *main_entity),
                        InputUniformIndex::default(),
                        phase_type,
                    );
                }
            }
        }

        if !opaque_phase.is_empty() || !transparent_phase.items.is_empty() {
            queue_status.has_volume = true;
        }
    }
}

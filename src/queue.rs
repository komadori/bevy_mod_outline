use bevy::core_pipeline::prepass::MotionVectorPrepass;
use bevy::prelude::*;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{DrawFunctions, PhaseItemExtraIndex, ViewSortedRenderPhases};
use bevy::render::render_resource::{PipelineCache, SpecializedMeshPipelines};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, RenderLayers};

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::{OutlinePipeline, PassType, PipelineKey};
use crate::render::DrawOutline;
use crate::uniforms::ExtractedOutline;

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
    mut stencil_phases: ResMut<ViewSortedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewSortedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(
        &ExtractedView,
        Entity,
        Option<&RenderLayers>,
        Has<MotionVectorPrepass>,
        &Msaa,
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

    for (view, view_entity, view_mask, motion_vector_prepass, msaa) in views.iter_mut() {
        let base_key = PipelineKey::new().with_msaa(*msaa);
        let view_mask = view_mask.cloned().unwrap_or_default();
        let rangefinder = view.rangefinder3d();
        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            stencil_phases.get_mut(&view_entity),
            opaque_phases.get_mut(&view_entity),
            transparent_phases.get_mut(&view_entity),
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
                .with_motion_vector_prepass(motion_vector_prepass);
            let distance = rangefinder.distance(&Mat4::from_translation(
                outline.instance_data.origin_in_world,
            ));
            if outline.stencil {
                let stencil_key = instance_base_key
                    .with_offset_zero(outline.instance_data.stencil_offset == 0.0)
                    .with_pass_type(PassType::Stencil);
                if let Ok(pipeline) = pipelines.specialize(
                    &pipeline_cache,
                    &outline_pipeline,
                    stencil_key,
                    &mesh.layout,
                ) {
                    stencil_phase.add(StencilOutline {
                        entity,
                        main_entity: *main_entity,
                        pipeline,
                        draw_function: draw_stencil,
                        distance,
                        batch_range: 0..0,
                        extra_index: PhaseItemExtraIndex::NONE,
                    });
                }
            }
            if outline.volume {
                let transparent = outline.instance_data.volume_colour[3] < 1.0;
                let draw_key = instance_base_key
                    .with_offset_zero(outline.instance_data.volume_offset == 0.0)
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
                        transparent_phase.add(TransparentOutline {
                            entity,
                            main_entity: *main_entity,
                            pipeline,
                            draw_function: draw_transparent_outline,
                            distance,
                            batch_range: 0..0,
                            extra_index: PhaseItemExtraIndex::NONE,
                        });
                    } else {
                        opaque_phase.add(OpaqueOutline {
                            entity,
                            main_entity: *main_entity,
                            pipeline,
                            draw_function: draw_opaque_outline,
                            distance,
                            batch_range: 0..1,
                            extra_index: PhaseItemExtraIndex::NONE,
                        });
                    }
                }
            }
        }
    }
}

use bevy::core_pipeline::prepass::{
    DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass,
};
use bevy::pbr::{DrawMesh, MeshPipelineViewLayoutKey, SetMeshBindGroup, SetMeshViewBindGroup};
use bevy::prelude::*;
use bevy::render::mesh::GpuMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    DrawFunctions, PhaseItemExtraIndex, SetItemPipeline, ViewSortedRenderPhases,
};
use bevy::render::render_resource::{PipelineCache, SpecializedMeshPipelines};
use bevy::render::view::{ExtractedView, RenderLayers};

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::{OutlinePipeline, PassType, PipelineKey};
use crate::uniforms::{
    ExtractedOutline, OutlineFragmentUniform, OutlineStencilUniform, OutlineVolumeUniform,
    SetOutlineStencilBindGroup, SetOutlineVolumeBindGroup,
};
use crate::view_uniforms::SetOutlineViewBindGroup;
use crate::OutlineRenderLayers;

pub(crate) type DrawStencil = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineViewBindGroup<2>,
    SetOutlineStencilBindGroup<3>,
    DrawMesh,
);

fn build_mesh_pipeline_view_layout_key(
    msaa: Msaa,
    (normal_prepass, depth_prepass, motion_vector_prepass, deferred_prepass): (
        bool,
        bool,
        bool,
        bool,
    ),
) -> MeshPipelineViewLayoutKey {
    let mut view_key = MeshPipelineViewLayoutKey::empty();

    if msaa != Msaa::Off {
        view_key |= MeshPipelineViewLayoutKey::MULTISAMPLED;
    }

    if normal_prepass {
        view_key |= MeshPipelineViewLayoutKey::NORMAL_PREPASS;
    }

    if depth_prepass {
        view_key |= MeshPipelineViewLayoutKey::DEPTH_PREPASS;
    }

    if motion_vector_prepass {
        view_key |= MeshPipelineViewLayoutKey::MOTION_VECTOR_PREPASS;
    }

    if deferred_prepass {
        view_key |= MeshPipelineViewLayoutKey::DEFERRED_PREPASS;
    }

    view_key
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_stencil_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    stencil_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<GpuMesh>>,
    material_meshes: Query<(
        Entity,
        &OutlineStencilUniform,
        &ExtractedOutline,
        &OutlineRenderLayers,
    )>,
    mut stencil_phases: ResMut<ViewSortedRenderPhases<StencilOutline>>,
    mut views: Query<(
        &ExtractedView,
        Entity,
        Option<&RenderLayers>,
        (
            Has<NormalPrepass>,
            Has<DepthPrepass>,
            Has<MotionVectorPrepass>,
            Has<DeferredPrepass>,
        ),
    )>,
) {
    let draw_stencil = stencil_draw_functions
        .read()
        .get_id::<DrawStencil>()
        .unwrap();

    let base_key = PipelineKey::new()
        .with_msaa(*msaa)
        .with_pass_type(PassType::Stencil);

    for (view, view_entity, view_mask, prepasses) in views.iter_mut() {
        let rangefinder = view.rangefinder3d();
        let view_mask = view_mask.cloned().unwrap_or_default();
        let Some(stencil_phase) = stencil_phases.get_mut(&view_entity) else {
            continue; // No render phase
        };
        for (entity, stencil_uniform, outline, outline_mask) in material_meshes.iter() {
            if !view_mask.intersects(outline_mask) {
                continue; // Layer not enabled
            }
            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue; // No mesh
            };
            let key = base_key
                .with_primitive_topology(mesh.primitive_topology())
                .with_depth_mode(outline.depth_mode)
                .with_offset_zero(stencil_uniform.offset == 0.0)
                .with_morph_targets(mesh.morph_targets.is_some())
                .with_view_key(build_mesh_pipeline_view_layout_key(*msaa, prepasses));
            let Ok(pipeline) =
                pipelines.specialize(&pipeline_cache, &stencil_pipeline, key, &mesh.layout)
            else {
                continue; // No pipeline
            };
            let distance = rangefinder.distance(&Mat4::from_translation(stencil_uniform.origin));
            stencil_phase.add(StencilOutline {
                entity,
                pipeline,
                draw_function: draw_stencil,
                distance,
                batch_range: 0..0,
                extra_index: PhaseItemExtraIndex::NONE,
            });
        }
    }
}

pub(crate) type DrawOutline = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineViewBindGroup<2>,
    SetOutlineVolumeBindGroup<3>,
    DrawMesh,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_volume_mesh(
    opaque_draw_functions: Res<DrawFunctions<OpaqueOutline>>,
    transparent_draw_functions: Res<DrawFunctions<TransparentOutline>>,
    outline_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<GpuMesh>>,
    material_meshes: Query<(
        Entity,
        &OutlineVolumeUniform,
        &ExtractedOutline,
        &OutlineFragmentUniform,
        &OutlineRenderLayers,
    )>,
    mut opaque_phases: ResMut<ViewSortedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    mut views: Query<(
        &ExtractedView,
        Entity,
        Option<&RenderLayers>,
        (
            Has<NormalPrepass>,
            Has<DepthPrepass>,
            Has<MotionVectorPrepass>,
            Has<DeferredPrepass>,
        ),
    )>,
) {
    let draw_opaque_outline = opaque_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_transparent_outline = transparent_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    let base_key = PipelineKey::new().with_msaa(*msaa);

    for (view, view_entity, view_mask, prepasses) in views.iter_mut() {
        let view_mask = view_mask.cloned().unwrap_or_default();
        let rangefinder = view.rangefinder3d();
        let (Some(opaque_phase), Some(transparent_phase)) = (
            opaque_phases.get_mut(&view_entity),
            transparent_phases.get_mut(&view_entity),
        ) else {
            continue; // No render phase
        };
        for (entity, volume_uniform, outline, fragment_uniform, outline_mask) in
            material_meshes.iter()
        {
            if !view_mask.intersects(outline_mask) {
                continue; // Layer not enabled
            }
            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue; // No mesh
            };
            let transparent = fragment_uniform.colour[3] < 1.0;
            let key = base_key
                .with_primitive_topology(mesh.primitive_topology())
                .with_pass_type(if transparent {
                    PassType::Transparent
                } else {
                    PassType::Opaque
                })
                .with_depth_mode(outline.depth_mode)
                .with_offset_zero(volume_uniform.offset == 0.0)
                .with_hdr_format(view.hdr)
                .with_morph_targets(mesh.morph_targets.is_some())
                .with_view_key(build_mesh_pipeline_view_layout_key(*msaa, prepasses));
            let Ok(pipeline) =
                pipelines.specialize(&pipeline_cache, &outline_pipeline, key, &mesh.layout)
            else {
                continue; // No pipeline
            };
            let distance = rangefinder.distance(&Mat4::from_translation(volume_uniform.origin));
            if transparent {
                transparent_phase.add(TransparentOutline {
                    entity,
                    pipeline,
                    draw_function: draw_transparent_outline,
                    distance,
                    batch_range: 0..0,
                    extra_index: PhaseItemExtraIndex::NONE,
                });
            } else {
                opaque_phase.add(OpaqueOutline {
                    entity,
                    pipeline,
                    draw_function: draw_opaque_outline,
                    distance,
                    batch_range: 0..0,
                    extra_index: PhaseItemExtraIndex::NONE,
                });
            }
        }
    }
}

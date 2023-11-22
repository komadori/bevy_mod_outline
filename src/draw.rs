use bevy::pbr::{DrawMesh, SetMeshBindGroup};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{DrawFunctions, RenderPhase, SetItemPipeline};
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
    SetOutlineViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineStencilBindGroup<2>,
    DrawMesh,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn queue_outline_stencil_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    stencil_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    material_meshes: Query<(
        Entity,
        &OutlineStencilUniform,
        &ExtractedOutline,
        &OutlineRenderLayers,
    )>,
    mut views: Query<(
        &ExtractedView,
        &mut RenderPhase<StencilOutline>,
        Option<&RenderLayers>,
    )>,
) {
    let draw_stencil = stencil_draw_functions
        .read()
        .get_id::<DrawStencil>()
        .unwrap();

    let base_key = PipelineKey::new()
        .with_msaa(*msaa)
        .with_pass_type(PassType::Stencil);

    for (view, mut stencil_phase, view_mask) in views.iter_mut() {
        let rangefinder = view.rangefinder3d();
        let view_mask = view_mask.copied().unwrap_or_default();
        for (entity, stencil_uniform, outline, outline_mask) in material_meshes.iter() {
            if !view_mask.intersects(outline_mask) {
                continue; // Layer not enabled
            }
            let Some(mesh) = render_meshes.get(outline.mesh_id) else {
                continue; // No mesh
            };
            let key = base_key
                .with_primitive_topology(mesh.primitive_topology)
                .with_depth_mode(outline.depth_mode)
                .with_offset_zero(stencil_uniform.offset == 0.0)
                .with_morph_targets(mesh.morph_targets.is_some());
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
                dynamic_offset: None,
            });
        }
    }
}

pub(crate) type DrawOutline = (
    SetItemPipeline,
    SetOutlineViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineVolumeBindGroup<2>,
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
    render_meshes: Res<RenderAssets<Mesh>>,
    material_meshes: Query<(
        Entity,
        &OutlineVolumeUniform,
        &ExtractedOutline,
        &OutlineFragmentUniform,
        &OutlineRenderLayers,
    )>,
    mut views: Query<(
        &ExtractedView,
        &mut RenderPhase<OpaqueOutline>,
        &mut RenderPhase<TransparentOutline>,
        Option<&RenderLayers>,
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

    for (view, mut opaque_phase, mut transparent_phase, view_mask) in views.iter_mut() {
        let view_mask = view_mask.copied().unwrap_or_default();
        let rangefinder = view.rangefinder3d();
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
                .with_primitive_topology(mesh.primitive_topology)
                .with_pass_type(if transparent {
                    PassType::Transparent
                } else {
                    PassType::Opaque
                })
                .with_depth_mode(outline.depth_mode)
                .with_offset_zero(volume_uniform.offset == 0.0)
                .with_hdr_format(view.hdr)
                .with_morph_targets(mesh.morph_targets.is_some());
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
                    dynamic_offset: None,
                });
            } else {
                opaque_phase.add(OpaqueOutline {
                    entity,
                    pipeline,
                    draw_function: draw_opaque_outline,
                    distance,
                    batch_range: 0..0,
                    dynamic_offset: None,
                });
            }
        }
    }
}

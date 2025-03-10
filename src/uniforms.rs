use bevy::{
    math::Affine3,
    prelude::*,
    render::{
        batching::{no_gpu_preprocessing::BatchedInstanceBuffer, NoAutomaticBatching},
        extract_component::ExtractComponent,
        render_asset::RenderAssets,
        render_resource::{BindGroup, BindGroupEntries, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
        texture::{FallbackImage, GpuImage},
        view::RenderLayers,
    },
    utils::HashMap,
};

use crate::{pipeline::OutlinePipeline, ComputedOutline, TextureChannel};

#[derive(Component)]
pub struct ExtractedOutline {
    pub(crate) stencil: bool,
    pub(crate) volume: bool,
    pub(crate) depth_mode: DepthMode,
    pub(crate) draw_mode: DrawMode,
    pub(crate) double_sided: bool,
    pub(crate) mesh_id: AssetId<Mesh>,
    pub(crate) alpha_mask_id: Option<AssetId<Image>>,
    pub(crate) alpha_mask_channel: TextureChannel,
    pub(crate) automatic_batching: bool,
    pub(crate) instance_data: OutlineInstanceUniform,
    pub(crate) layers: RenderLayers,
}

#[derive(Clone, ShaderType)]
pub(crate) struct OutlineInstanceUniform {
    pub world_from_local: [Vec4; 3],
    pub world_plane_origin: Vec3,
    pub world_plane_offset: Vec3,
    pub volume_offset: f32,
    pub volume_colour: Vec4,
    pub stencil_offset: f32,
    pub alpha_mask_threshold: f32,
    pub first_vertex_index: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DepthMode {
    Flat = 1,
    Real = 2,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DrawMode {
    Extrude = 1,
    #[cfg(feature = "flood")]
    JumpFlood = 2,
}

#[derive(Resource)]
pub(crate) struct OutlineInstanceBindGroup {
    pub bind_group: BindGroup,
}

pub(crate) fn set_outline_visibility(mut query: Query<(&mut ViewVisibility, &ComputedOutline)>) {
    for (mut visibility, computed) in query.iter_mut() {
        if let ComputedOutline(Some(computed)) = computed {
            if computed.volume.value.enabled || computed.stencil.value.enabled {
                visibility.set();
            }
        }
    }
}

impl ExtractComponent for ComputedOutline {
    type QueryData = (
        &'static ComputedOutline,
        &'static GlobalTransform,
        &'static Mesh3d,
        Has<NoAutomaticBatching>,
    );
    type QueryFilter = ();
    type Out = ExtractedOutline;

    fn extract_component(
        (computed, transform, mesh, no_automatic_batching): bevy::ecs::query::QueryItem<
            '_,
            Self::QueryData,
        >,
    ) -> Option<Self::Out> {
        let ComputedOutline(Some(computed)) = computed else {
            return None;
        };
        Some(ExtractedOutline {
            stencil: computed.stencil.value.enabled,
            volume: computed.volume.value.enabled,
            depth_mode: computed.mode.value.depth_mode,
            draw_mode: computed.mode.value.draw_mode,
            double_sided: computed.mode.value.double_sided,
            layers: computed.layers.value.clone(),
            mesh_id: mesh.id(),
            alpha_mask_id: computed
                .alpha_mask
                .value
                .texture
                .as_ref()
                .map(|texture| texture.id()),
            alpha_mask_channel: computed.alpha_mask.value.channel,
            automatic_batching: !no_automatic_batching
                && computed.mode.value.draw_mode == DrawMode::Extrude,
            instance_data: OutlineInstanceUniform {
                world_from_local: Affine3::from(&transform.affine()).to_transpose(),
                world_plane_origin: computed.depth.value.world_plane_origin,
                world_plane_offset: computed.depth.value.world_plane_offset,
                stencil_offset: computed.stencil.value.offset,
                volume_offset: computed.volume.value.offset,
                volume_colour: computed.volume.value.colour.to_vec4(),
                alpha_mask_threshold: computed.alpha_mask.value.threshold,
                first_vertex_index: 0,
            },
        })
    }
}

pub(crate) fn prepare_outline_instance_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    batched_instance_buffer: Res<BatchedInstanceBuffer<OutlineInstanceUniform>>,
    outline_pipeline: Res<OutlinePipeline>,
) {
    if let Some(instance_binding) = batched_instance_buffer.instance_data_binding() {
        let bind_group = render_device.create_bind_group(
            "outline_instance_mesh_bind_group",
            &outline_pipeline.outline_instance_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: instance_binding,
            }],
        );
        commands.insert_resource(OutlineInstanceBindGroup { bind_group });
    };
}

#[derive(Resource)]
pub(crate) struct AlphaMaskBindGroups {
    pub bind_groups: HashMap<AssetId<Image>, BindGroup>,
    pub default_bind_group: BindGroup,
}

impl FromWorld for AlphaMaskBindGroups {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let fallback_image = world.resource::<FallbackImage>();
        let outline_pipeline = world.resource::<OutlinePipeline>();

        Self {
            bind_groups: HashMap::new(),
            default_bind_group: render_device.create_bind_group(
                "default_outline_alpha_mask_bind_group",
                &outline_pipeline.alpha_mask_bind_group_layout,
                &BindGroupEntries::sequential((
                    &fallback_image.d2.texture_view,
                    &fallback_image.d2.sampler,
                )),
            ),
        }
    }
}

pub(crate) fn prepare_alpha_mask_bind_groups(
    mut alpha_mask_bind_groups: ResMut<AlphaMaskBindGroups>,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    outlines: Query<&ExtractedOutline>,
) {
    alpha_mask_bind_groups.bind_groups.clear();

    // Collect all unique textures used by outline alpha masks
    for outline in outlines.iter() {
        if let Some(texture_id) = outline.alpha_mask_id {
            if let Some(gpu_image) = gpu_images.get(texture_id) {
                alpha_mask_bind_groups
                    .bind_groups
                    .entry(texture_id)
                    .or_insert_with(|| {
                        render_device.create_bind_group(
                            "outline_alpha_mask_bind_group",
                            &outline_pipeline.alpha_mask_bind_group_layout,
                            &BindGroupEntries::sequential((
                                &gpu_image.texture_view,
                                &gpu_image.sampler,
                            )),
                        )
                    });
            }
        }
    }
}

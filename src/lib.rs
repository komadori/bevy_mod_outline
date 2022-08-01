use bevy::asset::load_internal_asset;
use bevy::core_pipeline::core_3d::{Opaque3d, Transparent3d};
use bevy::ecs::system::lifetimeless::{Read, SQuery, SRes};
use bevy::ecs::system::SystemParamItem;
use bevy::pbr::{
    DrawMesh, MeshPipeline, MeshPipelineKey, MeshUniform, SetMeshBindGroup, SetMeshViewBindGroup,
};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::extract_component::ExtractComponentPlugin;
use bevy::render::mesh::{MeshVertexBufferLayout, PrimitiveTopology};
use bevy::render::render_asset::{PrepareAssetError, RenderAsset, RenderAssetPlugin, RenderAssets};
use bevy::render::render_phase::{
    AddRenderCommand, DrawFunctions, EntityRenderCommand, RenderCommandResult, RenderPhase,
    SetItemPipeline, TrackedRenderPass,
};
use bevy::render::render_resource::{
    AsBindGroup, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, BufferSize,
    DynamicUniformBuffer, Face, PipelineCache, PreparedBindGroup, RenderPipelineDescriptor,
    ShaderSize, ShaderStages, ShaderType, SpecializedMeshPipeline, SpecializedMeshPipelineError,
    SpecializedMeshPipelines,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::FallbackImage;
use bevy::render::view::ExtractedView;
use bevy::render::{Extract, RenderApp, RenderStage};
use libm::nextafterf;

// See https://alexanderameye.github.io/notes/rendering-outlines/

const OUTLINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2101625026478770097);

/// An asset for rendering outlines around meshes.
#[derive(Clone, TypeUuid, AsBindGroup)]
#[uuid = "552e416b-2766-4e6a-9ee5-9ebd0e8c0230"]
pub struct Outline {
    /// Colour of the outline
    #[uniform(1, visibility(fragment))]
    pub colour: Color,
    /// Width of the outline in logical pixels
    #[uniform(0, visibility(vertex))]
    pub width: f32,
}

impl RenderAsset for Outline {
    type ExtractedAsset = Outline;

    type PreparedAsset = GpuOutline;

    type Param = (
        SRes<RenderDevice>,
        SRes<OutlinePipeline>,
        SRes<RenderAssets<Image>>,
        SRes<FallbackImage>,
    );

    fn extract_asset(&self) -> Self::ExtractedAsset {
        self.clone()
    }

    fn prepare_asset(
        outline: Self::ExtractedAsset,
        (render_device, outline_pipeline, images, fallback_image): &mut bevy::ecs::system::SystemParamItem<Self::Param>,
    ) -> Result<
        Self::PreparedAsset,
        bevy::render::render_asset::PrepareAssetError<Self::ExtractedAsset>,
    > {
        if let Ok(pbg) = outline.as_bind_group(
            &outline_pipeline.outline_bind_group_layout,
            render_device,
            images,
            fallback_image,
        ) {
            Ok(GpuOutline {
                bind_group: pbg,
                transparent: outline.colour.a() < 1.0,
            })
        } else {
            Err(PrepareAssetError::RetryNextUpdate(outline))
        }
    }
}

#[derive(Clone, Component, ShaderType)]
struct ViewSizeUniform {
    logical_size: Vec2,
}

#[derive(Default)]
struct ViewSizeUniforms {
    pub uniforms: DynamicUniformBuffer<ViewSizeUniform>,
}

#[derive(Component)]
struct ViewSizeUniformOffset {
    pub offset: u32,
}

struct GpuViewSize {
    bind_group: BindGroup,
}

pub struct GpuOutline {
    bind_group: PreparedBindGroup<Outline>,
    transparent: bool,
}

/// Adds support for the [Outline] asset type.
pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            OUTLINE_SHADER_HANDLE,
            "outline.wgsl",
            Shader::from_wgsl
        );
        app.add_asset::<Outline>()
            .add_plugin(ExtractComponentPlugin::<Handle<Outline>>::default())
            .add_plugin(RenderAssetPlugin::<Outline>::default())
            .sub_app_mut(RenderApp)
            .add_render_command::<Opaque3d, DrawOutline>()
            .add_render_command::<Transparent3d, DrawOutline>()
            .init_resource::<OutlinePipeline>()
            .init_resource::<SpecializedMeshPipelines<OutlinePipeline>>()
            .init_resource::<ViewSizeUniforms>()
            .add_system_to_stage(RenderStage::Extract, extract_view_size_uniforms)
            .add_system_to_stage(RenderStage::Prepare, prepare_view_size_uniforms)
            .add_system_to_stage(RenderStage::Queue, queue_outline);
    }
}

type DrawOutline = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetViewSizeBindGroup<2>,
    SetOutlineBindGroup<3>,
    DrawMesh,
);

struct SetViewSizeBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetViewSizeBindGroup<I> {
    type Param = (SRes<GpuViewSize>, SQuery<Read<ViewSizeUniformOffset>>);
    #[inline]
    fn render<'w>(
        view: Entity,
        _item: Entity,
        (gpu_view_size, offset_query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let uniform_offset = offset_query.get_inner(view).unwrap();
        pass.set_bind_group(
            I,
            &gpu_view_size.into_inner().bind_group,
            &[uniform_offset.offset],
        );

        RenderCommandResult::Success
    }
}

struct SetOutlineBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineBindGroup<I> {
    type Param = (SRes<RenderAssets<Outline>>, SQuery<Read<Handle<Outline>>>);
    fn render<'w>(
        _view: Entity,
        item: Entity,
        (outlines, query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let outline_handle = query.get(item).unwrap();
        let outline = outlines.into_inner().get(outline_handle).unwrap();
        pass.set_bind_group(I, &outline.bind_group.bind_group, &[]);
        RenderCommandResult::Success
    }
}

fn extract_view_size_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Camera), With<Camera3d>>>,
) {
    for (entity, camera) in query.iter() {
        if !camera.is_active {
            continue;
        }
        if let Some(size) = camera.logical_viewport_size() {
            commands
                .get_or_spawn(entity)
                .insert(ViewSizeUniform { logical_size: size });
        }
    }
}

fn prepare_view_size_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    outline_pipeline: Res<OutlinePipeline>,
    mut view_size_uniforms: ResMut<ViewSizeUniforms>,
    views: Query<(Entity, &ViewSizeUniform)>,
) {
    view_size_uniforms.uniforms.clear();
    for (entity, view_size_uniform) in views.iter() {
        let view_size_uniforms = ViewSizeUniformOffset {
            offset: view_size_uniforms.uniforms.push(view_size_uniform.clone()),
        };
        commands.entity(entity).insert(view_size_uniforms);
    }

    view_size_uniforms
        .uniforms
        .write_buffer(&render_device, &render_queue);

    if let Some(view_binding) = view_size_uniforms.uniforms.binding() {
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            }],
            label: Some("outline_view_size_bind_group"),
            layout: &outline_pipeline.view_size_bind_group_layout,
        });
        commands.insert_resource(GpuViewSize { bind_group });
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_outline(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    outline_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    render_outlines: Res<RenderAssets<Outline>>,
    material_meshes: Query<(Entity, &MeshUniform, &Handle<Mesh>, &Handle<Outline>)>,
    mut views: Query<(
        &ExtractedView,
        &mut RenderPhase<Opaque3d>,
        &mut RenderPhase<Transparent3d>,
    )>,
) {
    let draw_opaque_outline = opaque_3d_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_transparent_outline = transparent_3d_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    let base_key = MeshPipelineKey::from_msaa_samples(msaa.samples)
        | MeshPipelineKey::from_primitive_topology(PrimitiveTopology::TriangleList);

    for (view, mut opaque_phase, mut transparent_phase) in views.iter_mut() {
        let inverse_view_matrix = view.transform.compute_matrix().inverse();
        let inverse_view_row_2 = inverse_view_matrix.row(2);
        for (entity, mesh_uniform, mesh_handle, outline_handle) in material_meshes.iter() {
            if let Some(mesh) = render_meshes.get(mesh_handle) {
                if let Some(outline) = render_outlines.get(outline_handle) {
                    let key = if outline.transparent {
                        base_key | MeshPipelineKey::TRANSPARENT_MAIN_PASS
                    } else {
                        base_key
                    };
                    let pipeline = pipelines
                        .specialize(&mut pipeline_cache, &outline_pipeline, key, &mesh.layout)
                        .unwrap();
                    // Increase distance to just behind the non-outline mesh
                    let distance = nextafterf(
                        inverse_view_row_2.dot(mesh_uniform.transform.col(3)),
                        f32::NEG_INFINITY,
                    );
                    if outline.transparent {
                        transparent_phase.add(Transparent3d {
                            entity,
                            pipeline,
                            draw_function: draw_transparent_outline,
                            distance,
                        });
                    } else {
                        opaque_phase.add(Opaque3d {
                            entity,
                            pipeline,
                            draw_function: draw_opaque_outline,
                            distance: -distance,
                        });
                    }
                }
            }
        }
    }
}

pub struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    view_size_bind_group_layout: BindGroupLayout,
    outline_bind_group_layout: BindGroupLayout,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let view_size_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_view_size_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(ViewSizeUniform::SHADER_SIZE.get()),
                    },
                    count: None,
                }],
            });
        let outline_bind_group_layout = Outline::bind_group_layout(&render_device);
        OutlinePipeline {
            mesh_pipeline,
            view_size_bind_group_layout,
            outline_bind_group_layout,
        }
    }
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;
        descriptor.primitive.cull_mode = Some(Face::Front);
        descriptor.vertex.shader = OUTLINE_SHADER_HANDLE.typed();
        descriptor.fragment.as_mut().unwrap().shader = OUTLINE_SHADER_HANDLE.typed();
        descriptor.layout = Some(vec![
            self.mesh_pipeline.view_layout.clone(),
            self.mesh_pipeline.mesh_layout.clone(),
            self.view_size_bind_group_layout.clone(),
            self.outline_bind_group_layout.clone(),
        ]);
        Ok(descriptor)
    }
}

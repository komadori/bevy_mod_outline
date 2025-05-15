use std::borrow::Cow;

use bevy::asset::weak_handle;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::ecs::system::SystemParamItem;
use bevy::pbr::{setup_morph_and_skinning_defs, skins_use_uniform_buffers, MeshPipelineKey};
use bevy::prelude::*;
use bevy::render::batching::{gpu_preprocessing, GetBatchData, GetFullBatchData};
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutEntries, BlendState, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, Face, FragmentState, FrontFace,
    GpuArrayBuffer, MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, ShaderDefVal,
    ShaderStages, ShaderType, StencilState, TextureFormat, VertexState,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::WgpuSettings;
use bevy::render::sync_world::MainEntity;
use bevy::render::view::ViewTarget;
use bevy::{
    pbr::MeshPipeline,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
        },
    },
};
use bitfield::{bitfield_bitrange, bitfield_fields};
use nonmax::NonMaxU32;
use wgpu_types::{Backends, PushConstantRange, SamplerBindingType, TextureSampleType};

use crate::uniforms::{DepthMode, OutlineInstanceUniform, RenderOutlineInstances};
use crate::view_uniforms::OutlineViewUniform;
use crate::{TextureChannel, ATTRIBUTE_OUTLINE_NORMAL};

pub(crate) const COMMON_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("aee41cd9-fc8f-4788-9ea4-f85bd8070c65");

pub(crate) const OUTLINE_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("910c269f-f115-47ba-b757-6ae51bf0c79f");

pub(crate) const FRAGMENT_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("1f5b5967-7cbb-4392-8f34-421587938a12");

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PassType {
    Stencil = 1,
    Opaque = 2,
    Transparent = 3,
    FloodInit = 4,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PipelineKey(u32);
bitfield_bitrange! {struct PipelineKey(u32)}

impl PipelineKey {
    bitfield_fields! {
        u32;
        msaa_samples_minus_one, set_msaa_samples_minus_one: 5, 0;
        primitive_topology_int, set_primitive_topology_int: 8, 6;
        pass_type_int, set_pass_type_int: 11, 9;
        depth_mode_int, set_depth_mode_int: 13, 12;
        pub vertex_offset_zero, set_vertex_offset_zero: 14;
        pub plane_offset_zero, set_plane_offset_zero: 15;
        pub hdr_format, set_hdr_format: 16;
        pub morph_targets, set_morph_targets: 17;
        pub motion_vector_prepass, set_motion_vector_prepass: 18;
        pub double_sided, set_double_sided: 19;
        pub alpha_mask_texture, set_alpha_mask_texture: 20;
        pub alpha_mask_channel_int, set_alpha_mask_channel_int: 22, 21;
    }

    pub(crate) fn new() -> Self {
        PipelineKey(0)
    }

    pub(crate) fn with_msaa(mut self, msaa: Msaa) -> Self {
        self.set_msaa_samples_minus_one(msaa as u32 - 1);
        self
    }

    pub(crate) fn msaa(&self) -> Msaa {
        match self.msaa_samples_minus_one() + 1 {
            x if x == Msaa::Off as u32 => Msaa::Off,
            x if x == Msaa::Sample2 as u32 => Msaa::Sample2,
            x if x == Msaa::Sample4 as u32 => Msaa::Sample4,
            x if x == Msaa::Sample8 as u32 => Msaa::Sample8,
            x => panic!("Invalid value for Msaa: {}", x),
        }
    }

    pub(crate) fn with_primitive_topology(mut self, primitive_topology: PrimitiveTopology) -> Self {
        self.set_primitive_topology_int(primitive_topology as u32);
        self
    }

    pub(crate) fn primitive_topology(&self) -> PrimitiveTopology {
        match self.primitive_topology_int() {
            x if x == PrimitiveTopology::PointList as u32 => PrimitiveTopology::PointList,
            x if x == PrimitiveTopology::LineList as u32 => PrimitiveTopology::LineList,
            x if x == PrimitiveTopology::LineStrip as u32 => PrimitiveTopology::LineStrip,
            x if x == PrimitiveTopology::TriangleList as u32 => PrimitiveTopology::TriangleList,
            x if x == PrimitiveTopology::TriangleStrip as u32 => PrimitiveTopology::TriangleStrip,
            x => panic!("Invalid value for PrimitiveTopology: {}", x),
        }
    }

    pub(crate) fn with_pass_type(mut self, pass_type: PassType) -> Self {
        self.set_pass_type_int(pass_type as u32);
        self
    }

    pub(crate) fn pass_type(&self) -> PassType {
        match self.pass_type_int() {
            x if x == PassType::Stencil as u32 => PassType::Stencil,
            x if x == PassType::Opaque as u32 => PassType::Opaque,
            x if x == PassType::Transparent as u32 => PassType::Transparent,
            x if x == PassType::FloodInit as u32 => PassType::FloodInit,
            x => panic!("Invalid value for PassType: {}", x),
        }
    }

    pub(crate) fn with_depth_mode(mut self, depth_mode: DepthMode) -> Self {
        self.set_depth_mode_int(depth_mode as u32);
        self
    }

    pub(crate) fn depth_mode(&self) -> DepthMode {
        match self.depth_mode_int() {
            x if x == DepthMode::Flat as u32 => DepthMode::Flat,
            x if x == DepthMode::Real as u32 => DepthMode::Real,
            x => panic!("Invalid value for DepthMode: {}", x),
        }
    }

    pub(crate) fn with_vertex_offset_zero(mut self, vertex_offset_zero: bool) -> Self {
        self.set_vertex_offset_zero(vertex_offset_zero);
        self
    }

    pub(crate) fn with_plane_offset_zero(mut self, plane_offset_zero: bool) -> Self {
        self.set_plane_offset_zero(plane_offset_zero);
        self
    }

    pub(crate) fn with_hdr_format(mut self, hdr_format: bool) -> Self {
        self.set_hdr_format(hdr_format);
        self
    }

    pub(crate) fn with_morph_targets(mut self, morph_targets: bool) -> Self {
        self.set_morph_targets(morph_targets);
        self
    }

    pub(crate) fn with_motion_vector_prepass(mut self, motion_vector_prepass: bool) -> Self {
        self.set_motion_vector_prepass(motion_vector_prepass);
        self
    }

    pub(crate) fn with_double_sided(mut self, double_sided: bool) -> Self {
        self.set_double_sided(double_sided);
        self
    }

    pub(crate) fn with_alpha_mask_texture(mut self, alpha_mask_texture: bool) -> Self {
        self.set_alpha_mask_texture(alpha_mask_texture);
        self
    }

    pub(crate) fn with_alpha_mask_channel(mut self, channel: TextureChannel) -> Self {
        let channel_int = match channel {
            TextureChannel::R => 0,
            TextureChannel::G => 1,
            TextureChannel::B => 2,
            TextureChannel::A => 3,
        };
        self.set_alpha_mask_channel_int(channel_int);
        self
    }
}

impl From<PipelineKey> for MeshPipelineKey {
    fn from(key: PipelineKey) -> Self {
        let mut mesh_key = MeshPipelineKey::empty();
        if key.morph_targets() {
            mesh_key |= MeshPipelineKey::MORPH_TARGETS;
        }
        if key.motion_vector_prepass() {
            mesh_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }
        mesh_key
    }
}

#[derive(Resource)]
pub(crate) struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    pub outline_view_bind_group_layout: BindGroupLayout,
    pub outline_instance_bind_group_layout: BindGroupLayout,
    pub alpha_mask_bind_group_layout: BindGroupLayout,
    pub instance_batch_size: Option<u32>,
    pub skins_use_uniform_buffers: bool,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let outline_view_bind_group_layout = render_device.create_bind_group_layout(
            "outline_view_bind_group_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX,
                uniform_buffer_sized(true, Some(OutlineViewUniform::min_size())),
            ),
        );
        let outline_instance_bind_group_layout = render_device.create_bind_group_layout(
            "outline_instance_bind_group_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX,
                GpuArrayBuffer::<OutlineInstanceUniform>::binding_layout(render_device),
            ),
        );
        let alpha_mask_bind_group_layout = render_device.create_bind_group_layout(
            "alpha_mask_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );

        let instance_batch_size =
            GpuArrayBuffer::<OutlineInstanceUniform>::batch_size(render_device);
        let skins_use_uniform_buffers = skins_use_uniform_buffers(render_device);
        OutlinePipeline {
            mesh_pipeline,
            outline_view_bind_group_layout,
            outline_instance_bind_group_layout,
            alpha_mask_bind_group_layout,
            instance_batch_size,
            skins_use_uniform_buffers,
        }
    }
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = PipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut targets = vec![];
        let mut vertex_defs = vec![];
        let mut fragment_defs = vec![];
        let mut buffer_attrs = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];

        let bind_layouts = vec![
            self.outline_view_bind_group_layout.clone(),
            setup_morph_and_skinning_defs(
                &self.mesh_pipeline.mesh_layouts,
                layout,
                5,
                &key.into(),
                &mut vertex_defs,
                &mut buffer_attrs,
                self.skins_use_uniform_buffers,
            ),
            self.outline_instance_bind_group_layout.clone(),
            self.alpha_mask_bind_group_layout.clone(),
        ];

        if key.alpha_mask_texture() {
            let val = ShaderDefVal::from("ALPHA_MASK_TEXTURE");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);

            let channel_def = ShaderDefVal::UInt(
                "ALPHA_MASK_CHANNEL".to_string(),
                key.alpha_mask_channel_int(),
            );
            fragment_defs.push(channel_def);

            buffer_attrs.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(2));
        }

        if let Some(sz) = self.instance_batch_size {
            vertex_defs.push(ShaderDefVal::Int(
                "INSTANCE_BATCH_SIZE".to_string(),
                sz as i32,
            ));
        }

        let cull_mode;
        if key.depth_mode() == DepthMode::Flat {
            let val = ShaderDefVal::from("FLAT_DEPTH");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);
            if key.double_sided() {
                cull_mode = None;
            } else {
                cull_mode = Some(Face::Back);
            }
        } else if key.pass_type() == PassType::Stencil {
            cull_mode = Some(Face::Back);
        } else {
            cull_mode = Some(Face::Front);
        }
        if key.vertex_offset_zero() {
            vertex_defs.push(ShaderDefVal::from("VERTEX_OFFSET_ZERO"));
        } else {
            buffer_attrs.push(
                if layout.0.contains(ATTRIBUTE_OUTLINE_NORMAL) {
                    ATTRIBUTE_OUTLINE_NORMAL
                } else {
                    Mesh::ATTRIBUTE_NORMAL
                }
                .at_shader_location(1),
            );
        }
        if key.plane_offset_zero() {
            vertex_defs.push(ShaderDefVal::from("PLANE_OFFSET_ZERO"));
        }
        match key.pass_type() {
            PassType::Stencil => {}
            PassType::Opaque | PassType::Transparent => {
                let val = ShaderDefVal::from("VOLUME");
                vertex_defs.push(val.clone());
                fragment_defs.push(val);
                targets.push(Some(ColorTargetState {
                    format: if key.hdr_format() {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    blend: Some(if key.pass_type() == PassType::Transparent {
                        BlendState::ALPHA_BLENDING
                    } else {
                        BlendState::REPLACE
                    }),
                    write_mask: ColorWrites::ALL,
                }));
            }
            PassType::FloodInit => {
                let val = ShaderDefVal::from("FLOOD_INIT");
                vertex_defs.push(val.clone());
                fragment_defs.push(val);
                targets.push(Some(ColorTargetState {
                    format: TextureFormat::Rgba16Float,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                }));
            }
        }
        let depth_stencil = if key.pass_type() == PassType::FloodInit {
            None
        } else {
            Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            })
        };
        let buffers = vec![layout.0.get_layout(&buffer_attrs)?];
        let mut push_constant_ranges = Vec::with_capacity(1);
        // Proxy for webgl feature flag in bevy
        if WgpuSettings::default().backends == Some(Backends::GL) {
            push_constant_ranges.push(PushConstantRange {
                stages: ShaderStages::VERTEX,
                range: 0..4,
            });
        }
        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: OUTLINE_SHADER_HANDLE,
                entry_point: "vertex".into(),
                shader_defs: vertex_defs,
                buffers,
            },
            fragment: Some(FragmentState {
                shader: FRAGMENT_SHADER_HANDLE,
                shader_defs: fragment_defs,
                entry_point: "fragment".into(),
                targets,
            }),
            layout: bind_layouts,
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: key.primitive_topology(),
                strip_index_format: None,
            },
            depth_stencil,
            multisample: MultisampleState {
                count: key.msaa().samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            push_constant_ranges,
            label: Some(Cow::Borrowed("outline_pipeline")),
            zero_initialize_workgroup_memory: false,
        })
    }
}

impl GetBatchData for OutlinePipeline {
    type Param = (SRes<RenderOutlineInstances>, SRes<MeshAllocator>);
    type CompareData = (AssetId<Mesh>, Option<AssetId<Image>>);
    type BufferData = OutlineInstanceUniform;

    fn get_batch_data(
        (render_outlines, mesh_allocator): &SystemParamItem<Self::Param>,
        (_entity, main_entity): (Entity, MainEntity),
    ) -> Option<(Self::BufferData, Option<Self::CompareData>)> {
        let outline = render_outlines.get(&main_entity)?;
        let mut instance_data = outline.instance_data.clone();
        instance_data.first_vertex_index = mesh_allocator
            .mesh_vertex_slice(&outline.mesh_id)
            .map(|x| x.range.start)
            .unwrap_or(0);

        // Only batch entities with the same mesh and alpha mask texture
        let batch_data = if outline.automatic_batching {
            Some((outline.mesh_id, outline.alpha_mask_id))
        } else {
            None
        };

        Some((instance_data, batch_data))
    }
}

impl GetFullBatchData for OutlinePipeline {
    type BufferInputData = ();

    fn get_binned_batch_data(
        (render_outlines, mesh_allocator): &SystemParamItem<Self::Param>,
        main_entity: MainEntity,
    ) -> Option<Self::BufferData> {
        let outline = render_outlines.get(&main_entity)?;
        let mut instance_data = outline.instance_data.clone();
        instance_data.first_vertex_index = mesh_allocator
            .mesh_vertex_slice(&outline.mesh_id)
            .map(|x| x.range.start)
            .unwrap_or(0);
        Some(instance_data)
    }

    fn get_index_and_compare_data(
        _param: &SystemParamItem<Self::Param>,
        _main_entity: MainEntity,
    ) -> Option<(NonMaxU32, Option<Self::CompareData>)> {
        unimplemented!("GPU batching is not used.");
    }

    fn get_binned_index(
        _param: &SystemParamItem<Self::Param>,
        _main_entity: MainEntity,
    ) -> Option<NonMaxU32> {
        unimplemented!("GPU batching is not used.");
    }

    fn write_batch_indirect_parameters_metadata(
        _indexed: bool,
        _base_output_index: u32,
        _batch_set_index: Option<NonMaxU32>,
        _phase_indirect_parameters_buffers: &mut gpu_preprocessing::UntypedPhaseIndirectParametersBuffers,
        _indirect_parameters_offset: u32,
    ) {
        unimplemented!("GPU batching is not used.");
    }
}

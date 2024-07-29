use std::borrow::Cow;

use bevy::ecs::system::lifetimeless::SQuery;
use bevy::ecs::system::SystemParamItem;
use bevy::pbr::{setup_morph_and_skinning_defs, MeshPipelineKey};
use bevy::prelude::*;
use bevy::render::batching::{GetBatchData, GetFullBatchData};
use bevy::render::render_resource::binding_types::uniform_buffer_sized;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutEntries, BlendState, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, Face, FragmentState, FrontFace,
    GpuArrayBuffer, MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, ShaderDefVal,
    ShaderStages, ShaderType, StencilState, TextureFormat, VertexState,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::WgpuSettings;
use bevy::render::texture::BevyDefault;
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
use wgpu_types::{Backends, PushConstantRange};

use crate::uniforms::{DepthMode, ExtractedOutline, OutlineInstanceUniform};
use crate::view_uniforms::OutlineViewUniform;
use crate::ATTRIBUTE_OUTLINE_NORMAL;

pub(crate) const COMMON_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(158939267822951776165272591102639985656);

pub(crate) const OUTLINE_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(223498151714529302374103749587714613067);

pub(crate) const FRAGMENT_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(330091643565174537467176491706815552661);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassType {
    Stencil = 1,
    Opaque = 2,
    Transparent = 3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PipelineKey(u32);
bitfield_bitrange! {struct PipelineKey(u32)}

impl PipelineKey {
    bitfield_fields! {
        u32;
        msaa_samples_minus_one, set_msaa_samples_minus_one: 5, 0;
        primitive_topology_int, set_primitive_topology_int: 8, 6;
        pass_type_int, set_pass_type_int: 10, 9;
        depth_mode_int, set_depth_mode_int: 12, 11;
        pub offset_zero, set_offset_zero: 13;
        pub hdr_format, set_hdr_format: 14;
        pub morph_targets, set_morph_targets: 15;
        pub motion_vector_prepass, set_motion_vector_prepass: 16;
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

    pub(crate) fn with_offset_zero(mut self, offset_zero: bool) -> Self {
        self.set_offset_zero(offset_zero);
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
    pub instance_batch_size: Option<u32>,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let outline_view_bind_group_layout = render_device.create_bind_group_layout(
            "oiutline_view_bind_group_layout",
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
        let instance_batch_size =
            GpuArrayBuffer::<OutlineInstanceUniform>::batch_size(render_device);
        OutlinePipeline {
            mesh_pipeline,
            outline_view_bind_group_layout,
            outline_instance_bind_group_layout,
            instance_batch_size,
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
            ),
            self.outline_instance_bind_group_layout.clone(),
        ];

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
            cull_mode = Some(Face::Back);
        } else if key.pass_type() == PassType::Stencil {
            cull_mode = Some(Face::Back);
        } else {
            cull_mode = Some(Face::Front);
        }
        if key.offset_zero() {
            vertex_defs.push(ShaderDefVal::from("OFFSET_ZERO"));
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
        }
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
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.msaa().samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            push_constant_ranges,
            label: Some(Cow::Borrowed("outline_pipeline")),
        })
    }
}

impl GetBatchData for OutlinePipeline {
    type Param = SQuery<&'static ExtractedOutline>;
    type CompareData = AssetId<Mesh>;
    type BufferData = OutlineInstanceUniform;

    fn get_batch_data(
        outline_query: &SystemParamItem<Self::Param>,
        entity: Entity,
    ) -> Option<(Self::BufferData, Option<Self::CompareData>)> {
        let outline = outline_query.get(entity).ok()?;
        Some((
            outline.instance_data.clone(),
            outline.automatic_batching.then_some(outline.mesh_id),
        ))
    }
}

impl GetFullBatchData for OutlinePipeline {
    type BufferInputData = ();

    fn get_binned_batch_data(
        outline_query: &SystemParamItem<Self::Param>,
        entity: Entity,
    ) -> Option<Self::BufferData> {
        let outline = outline_query.get(entity).ok()?;
        Some(outline.instance_data.clone())
    }

    fn get_index_and_compare_data(
        _param: &SystemParamItem<Self::Param>,
        _query_item: Entity,
    ) -> Option<(NonMaxU32, Option<Self::CompareData>)> {
        unimplemented!("GPU batching is not used.");
    }

    fn get_binned_index(
        _param: &SystemParamItem<Self::Param>,
        _query_item: Entity,
    ) -> Option<NonMaxU32> {
        unimplemented!("GPU batching is not used.");
    }

    fn get_batch_indirect_parameters_index(
        _param: &SystemParamItem<Self::Param>,
        _indirect_parameters_buffer: &mut bevy::render::batching::gpu_preprocessing::IndirectParametersBuffer,
        _entity: Entity,
        _instance_index: u32,
    ) -> Option<NonMaxU32> {
        unimplemented!("GPU batching is not used.");
    }
}

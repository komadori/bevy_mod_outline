use std::borrow::Cow;

use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, BufferSize, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
    DepthStencilState, Face, FragmentState, FrontFace, MultisampleState, PolygonMode,
    PrimitiveState, PrimitiveTopology, ShaderSize, ShaderStages, StencilState, TextureFormat,
    VertexState,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::BevyDefault;
use bevy::{
    pbr::MeshPipeline,
    render::{
        mesh::MeshVertexBufferLayout,
        render_resource::{
            RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
        },
    },
};
use bitfield::{bitfield_bitrange, bitfield_fields};

use crate::uniforms::{OutlineFragmentUniform, OutlineStencilUniform, OutlineVolumeUniform};
use crate::view_uniforms::OutlineViewUniform;
use crate::ATTRIBUTE_OUTLINE_NORMAL;

pub const OUTLINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2101625026478770097);

pub const FRAGMENT_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 12033806834125368121);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PassType {
    Stencil = 1,
    Opaque = 2,
    Transparent = 3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct PipelineKey(u32);
bitfield_bitrange! {struct PipelineKey(u32)}

impl PipelineKey {
    bitfield_fields! {
        u32;
        msaa_samples_minus_one, set_msaa_samples_minus_one: 5, 0;
        primitive_topology_int, set_primitive_topology_int: 8, 6;
        pass_type_int, set_pass_type_int: 10, 9;
        pub offset_zero, set_offset_zero: 11;
    }

    pub fn new() -> Self {
        PipelineKey(0)
    }

    pub fn with_msaa_samples(mut self, msaa_samples: u32) -> Self {
        self.set_msaa_samples_minus_one(msaa_samples - 1);
        self
    }

    pub fn msaa_samples(&self) -> u32 {
        self.msaa_samples_minus_one() + 1
    }

    pub fn with_primitive_topology(mut self, primitive_topology: PrimitiveTopology) -> Self {
        self.set_primitive_topology_int(primitive_topology as u32);
        self
    }

    pub fn primitive_topology(&self) -> PrimitiveTopology {
        match self.primitive_topology_int() {
            x if x == PrimitiveTopology::PointList as u32 => PrimitiveTopology::PointList,
            x if x == PrimitiveTopology::LineList as u32 => PrimitiveTopology::LineList,
            x if x == PrimitiveTopology::LineStrip as u32 => PrimitiveTopology::LineStrip,
            x if x == PrimitiveTopology::TriangleList as u32 => PrimitiveTopology::TriangleList,
            x if x == PrimitiveTopology::TriangleStrip as u32 => PrimitiveTopology::TriangleStrip,
            x => panic!("Invalid value for PrimitiveTopology: {}", x),
        }
    }

    pub fn with_pass_type(mut self, pass_type: PassType) -> Self {
        self.set_pass_type_int(pass_type as u32);
        self
    }

    pub fn pass_type(&self) -> PassType {
        match self.pass_type_int() {
            x if x == PassType::Stencil as u32 => PassType::Stencil,
            x if x == PassType::Opaque as u32 => PassType::Opaque,
            x if x == PassType::Transparent as u32 => PassType::Transparent,
            x => panic!("Invalid value for PassType: {}", x),
        }
    }

    pub fn with_offset_zero(mut self, offset_zero: bool) -> Self {
        self.set_offset_zero(offset_zero);
        self
    }
}

pub struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    pub outline_view_bind_group_layout: BindGroupLayout,
    pub outline_stencil_bind_group_layout: BindGroupLayout,
    pub outline_volume_bind_group_layout: BindGroupLayout,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let outline_view_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_view_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(OutlineViewUniform::SHADER_SIZE.get()),
                    },
                    count: None,
                }],
            });
        let outline_volume_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_volume_bind_group_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: BufferSize::new(
                                OutlineVolumeUniform::SHADER_SIZE.get(),
                            ),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: BufferSize::new(
                                OutlineFragmentUniform::SHADER_SIZE.get(),
                            ),
                        },
                        count: None,
                    },
                ],
            });
        let outline_stencil_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_stencil_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(OutlineStencilUniform::SHADER_SIZE.get()),
                    },
                    count: None,
                }],
            });
        OutlinePipeline {
            mesh_pipeline,
            outline_view_bind_group_layout,
            outline_stencil_bind_group_layout,
            outline_volume_bind_group_layout,
        }
    }
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = PipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        mesh_layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut targets = vec![];
        let mut bind_layouts = vec![self.mesh_pipeline.view_layout.clone()];
        let mut buffer_attrs = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
        let mut vertex_defs = vec![];
        let mut fragment_defs = vec![];
        bind_layouts.push(
            if mesh_layout.contains(Mesh::ATTRIBUTE_JOINT_INDEX)
                && mesh_layout.contains(Mesh::ATTRIBUTE_JOINT_WEIGHT)
            {
                vertex_defs.push("SKINNED".to_string());
                buffer_attrs.push(Mesh::ATTRIBUTE_JOINT_INDEX.at_shader_location(2));
                buffer_attrs.push(Mesh::ATTRIBUTE_JOINT_WEIGHT.at_shader_location(3));
                self.mesh_pipeline.skinned_mesh_layout.clone()
            } else {
                self.mesh_pipeline.mesh_layout.clone()
            },
        );
        bind_layouts.push(self.outline_view_bind_group_layout.clone());
        if key.offset_zero() {
            vertex_defs.push("OFFSET_ZERO".to_string());
        } else {
            buffer_attrs.push(
                if mesh_layout.contains(ATTRIBUTE_OUTLINE_NORMAL) {
                    ATTRIBUTE_OUTLINE_NORMAL
                } else {
                    Mesh::ATTRIBUTE_NORMAL
                }
                .at_shader_location(1),
            );
        }
        match key.pass_type() {
            PassType::Stencil => {
                bind_layouts.push(self.outline_stencil_bind_group_layout.clone());
            }
            PassType::Opaque | PassType::Transparent => {
                fragment_defs.push("VOLUME".to_string());
                targets.push(Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(if key.pass_type() == PassType::Transparent {
                        BlendState::ALPHA_BLENDING
                    } else {
                        BlendState::REPLACE
                    }),
                    write_mask: ColorWrites::ALL,
                }));

                bind_layouts.push(self.outline_volume_bind_group_layout.clone());
            }
        }
        let buffers = vec![mesh_layout.get_layout(&buffer_attrs)?];
        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: OUTLINE_SHADER_HANDLE.typed::<Shader>(),
                entry_point: "vertex".into(),
                shader_defs: vertex_defs,
                buffers,
            },
            fragment: Some(FragmentState {
                shader: FRAGMENT_SHADER_HANDLE.typed::<Shader>(),
                shader_defs: fragment_defs,
                entry_point: "fragment".into(),
                targets,
            }),
            layout: Some(bind_layouts),
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
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
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some(Cow::Borrowed("outline_stencil_pipeline")),
        })
    }
}

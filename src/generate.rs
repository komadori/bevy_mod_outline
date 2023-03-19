use bevy::{
    prelude::*,
    render::{
        mesh::VertexAttributeValues,
        render_resource::{PrimitiveTopology, VertexFormat},
    },
    utils::{FloatOrd, HashMap, HashSet},
};

use crate::ATTRIBUTE_OUTLINE_NORMAL;

/// Failed to generate outline normals for the mesh.
#[derive(thiserror::Error, Debug)]
pub enum GenerateOutlineNormalsError {
    #[error("unsupported primitive topology '{0:?}'")]
    UnsupportedPrimitiveTopology(PrimitiveTopology),
    #[error("missing vertex attributes '{0}'")]
    MissingVertexAttribute(&'static str),
    #[error("the '{0}' vertex attribute should have {1:?} format, but had {2:?} format")]
    InvalidVertexAttributeFormat(&'static str, VertexFormat, VertexFormat),
}

/// Extension methods for [`Mesh`].
pub trait OutlineMeshExt {
    /// Generates outline normals for the mesh from the face normals.
    ///
    /// Vertex extrusion only works for meshes with smooth surface normals. Hard edges cause
    /// visual artefacts. This function generates faux-smooth normals for outlining purposes
    /// by grouping vertices by their position and averaging the normals at each point. These
    /// outline normals are then inserted as a separate vertex attribute so that the regular
    /// normals remain untouched. However, insofar as the outline normals are not
    /// perpendicular to the surface of the mesh, this technique may result in non-uniform
    /// outline thickness.
    ///
    /// This function only supports meshes with TriangleList topology.
    fn generate_outline_normals(&mut self) -> Result<(), GenerateOutlineNormalsError>;
}

impl OutlineMeshExt for Mesh {
    fn generate_outline_normals(&mut self) -> Result<(), GenerateOutlineNormalsError> {
        if self.primitive_topology() != PrimitiveTopology::TriangleList {
            return Err(GenerateOutlineNormalsError::UnsupportedPrimitiveTopology(
                self.primitive_topology(),
            ));
        }
        let positions = match self.attribute(Mesh::ATTRIBUTE_POSITION).ok_or(
            GenerateOutlineNormalsError::MissingVertexAttribute(Mesh::ATTRIBUTE_POSITION.name),
        )? {
            VertexAttributeValues::Float32x3(p) => Ok(p),
            v => Err(GenerateOutlineNormalsError::InvalidVertexAttributeFormat(
                Mesh::ATTRIBUTE_POSITION.name,
                VertexFormat::Float32x3,
                v.into(),
            )),
        }?;
        let mut map = HashMap::<[FloatOrd; 3], Vec3>::with_capacity(positions.len());
        let mut proc = |p0: Vec3, p1: Vec3, p2: Vec3| {
            let face_normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            for (cp0, cp1, cp2) in [(p0, p1, p2), (p1, p2, p0), (p2, p0, p1)] {
                let angle = (cp1 - cp0).angle_between(cp2 - cp0);
                let n = map
                    .entry([FloatOrd(cp0.x), FloatOrd(cp0.y), FloatOrd(cp0.z)])
                    .or_default();
                *n += angle * face_normal;
            }
        };
        if let Some(indices) = self.indices() {
            let mut it = indices.iter();
            while let (Some(i0), Some(i1), Some(i2)) = (it.next(), it.next(), it.next()) {
                proc(
                    Vec3::from_array(positions[i0]),
                    Vec3::from_array(positions[i1]),
                    Vec3::from_array(positions[i2]),
                );
            }
        } else {
            let mut it = positions.iter();
            while let (Some(p0), Some(p1), Some(p2)) = (it.next(), it.next(), it.next()) {
                proc(
                    Vec3::from_array(*p0),
                    Vec3::from_array(*p1),
                    Vec3::from_array(*p2),
                );
            }
        }
        let mut outlines = Vec::with_capacity(positions.len());
        for p in positions.iter() {
            let key = [FloatOrd(p[0]), FloatOrd(p[1]), FloatOrd(p[2])];
            outlines.push(map.get(&key).unwrap().normalize_or_zero().to_array());
        }
        self.insert_attribute(
            ATTRIBUTE_OUTLINE_NORMAL,
            VertexAttributeValues::Float32x3(outlines),
        );
        Ok(())
    }
}

fn auto_generate_outline_normals(
    mut meshes: ResMut<Assets<Mesh>>,
    mut events: EventReader<'_, '_, AssetEvent<Mesh>>,
    mut squelch: Local<HashSet<Handle<Mesh>>>,
) {
    for event in events.iter() {
        match event {
            AssetEvent::Created { handle } | AssetEvent::Modified { handle } => {
                if squelch.contains(handle) {
                    // Suppress modification events created by this system
                    squelch.remove(handle);
                } else if let Some(mesh) = meshes.get_mut(handle) {
                    let _ = mesh.generate_outline_normals();
                    squelch.insert(handle.clone_weak());
                }
            }
            AssetEvent::Removed { handle } => {
                squelch.remove(handle);
            }
        }
    }
}

/// Automatically runs [`generate_outline_normals`](OutlineMeshExt::generate_outline_normals)
/// on every mesh.
///
/// This is provided as a convenience for simple projects. It runs the outline normal
/// generator every time a mesh asset is created or modified without consideration for
/// whether this is necessary or appropriate.
pub struct AutoGenerateOutlineNormalsPlugin;

impl Plugin for AutoGenerateOutlineNormalsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(auto_generate_outline_normals);
    }
}

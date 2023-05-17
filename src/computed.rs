use bevy::prelude::*;

use crate::uniforms::DepthMode;

/// A component for storing the computed depth at which the outline lies.
#[derive(Clone, Component, Default)]
pub struct ComputedOutlineDepth {
    pub(crate) world_origin: Vec3,
    pub(crate) depth_mode: DepthMode,
    pub(crate) inherited: Option<Entity>,
}

/// A component which specifies how the outline depth should be computed.
#[derive(Clone, Component)]
#[non_exhaustive]
pub enum SetOutlineDepth {
    /// A flat plane facing the camera and intersecting the specified point in model-space.
    Flat { model_origin: Vec3 },
    /// Real model-space.
    Real,
}

/// A component which specifies that this outline lies at the same depth as its parent.
#[derive(Clone, Component, Default)]
pub struct InheritOutlineDepth;

#[allow(clippy::type_complexity)]
pub(crate) fn compute_outline_depth(
    mut root_query: Query<
        (
            Entity,
            &mut ComputedOutlineDepth,
            &GlobalTransform,
            Changed<GlobalTransform>,
            Option<(&SetOutlineDepth, Changed<SetOutlineDepth>)>,
            Option<&Children>,
        ),
        Without<InheritOutlineDepth>,
    >,
    mut computed_query: Query<&mut ComputedOutlineDepth, With<InheritOutlineDepth>>,
    child_query: Query<&Children>,
) {
    for (entity, mut computed, transform, changed_transform, set_depth, children) in
        root_query.iter_mut()
    {
        let changed = !computed.depth_mode.is_valid()
            || computed.inherited.is_some()
            || changed_transform
            || set_depth.filter(|(_, c)| *c).is_some();
        if changed {
            let (origin, depth_mode) = if let Some((sd, _)) = set_depth {
                match sd {
                    SetOutlineDepth::Flat {
                        model_origin: origin,
                    } => (*origin, DepthMode::Flat),
                    SetOutlineDepth::Real => (Vec3::NAN, DepthMode::Real),
                }
            } else {
                (Vec3::ZERO, DepthMode::Flat)
            };
            let matrix = transform.compute_matrix();
            computed.world_origin = matrix.project_point3(origin);
            computed.depth_mode = depth_mode;
            computed.inherited = None;
        }
        if let Some(cs) = children {
            for child in cs.iter() {
                propagate_outline_depth(
                    &computed,
                    changed,
                    entity,
                    *child,
                    &mut computed_query,
                    &child_query,
                );
            }
        }
    }
}

fn propagate_outline_depth(
    root_computed: &ComputedOutlineDepth,
    mut changed: bool,
    parent: Entity,
    entity: Entity,
    computed_query: &mut Query<&mut ComputedOutlineDepth, With<InheritOutlineDepth>>,
    child_query: &Query<&Children>,
) {
    if let Ok(mut computed) = computed_query.get_mut(entity) {
        changed |= !computed.depth_mode.is_valid() | (computed.inherited != Some(parent));
        if changed {
            *computed = root_computed.clone();
            computed.inherited = Some(parent);
        }
        if let Ok(cs) = child_query.get(entity) {
            for child in cs.iter() {
                propagate_outline_depth(
                    root_computed,
                    changed,
                    entity,
                    *child,
                    computed_query,
                    child_query,
                );
            }
        }
    }
}

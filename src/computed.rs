use bevy::{ecs::query::QueryItem, prelude::*};

use crate::{uniforms::DepthMode, InheritOutline, OutlineMode, OutlineStencil, OutlineVolume};

/// A component for storing the computed depth at which the outline lies.
#[derive(Clone, Component, Default)]
pub struct ComputedOutline {
    pub(crate) is_valid: bool,
    pub(crate) inherited_from: Option<Entity>,
    pub(crate) volume_enabled: bool,
    pub(crate) volume_offset: f32,
    pub(crate) volume_colour: Vec4,
    pub(crate) set_volume: bool,
    pub(crate) stencil_enabled: bool,
    pub(crate) stencil_offset: f32,
    pub(crate) set_stencil: bool,
    pub(crate) world_origin: Vec3,
    pub(crate) depth_mode: DepthMode,
    pub(crate) set_mode: bool,
}

type OutlineComponents<'a> = (
    (&'a GlobalTransform, Changed<GlobalTransform>),
    Option<(&'a OutlineVolume, Changed<OutlineVolume>)>,
    Option<(&'a OutlineStencil, Changed<OutlineStencil>)>,
    Option<(&'a OutlineMode, Changed<OutlineMode>)>,
);

#[allow(clippy::type_complexity)]
pub(crate) fn compute_outline(
    mut root_query: Query<
        (
            Entity,
            &mut ComputedOutline,
            OutlineComponents,
            Option<&Children>,
        ),
        Without<InheritOutline>,
    >,
    mut child_query_mut: Query<(&mut ComputedOutline, OutlineComponents), With<InheritOutline>>,
    child_query: Query<&Children>,
) {
    for (entity, mut computed, components, children) in root_query.iter_mut() {
        let changed = update_computed_outline(&mut computed, components, None, None, false);
        if let Some(cs) = children {
            for child in cs.iter() {
                propagate_computed_outline(
                    &computed,
                    changed,
                    entity,
                    *child,
                    &mut child_query_mut,
                    &child_query,
                );
            }
        }
    }
}

fn propagate_computed_outline(
    parent_computed: &ComputedOutline,
    parent_changed: bool,
    parent_entity: Entity,
    entity: Entity,
    child_query_mut: &mut Query<(&mut ComputedOutline, OutlineComponents), With<InheritOutline>>,
    child_query: &Query<&Children>,
) {
    if let Ok((mut computed, components)) = child_query_mut.get_mut(entity) {
        let changed = update_computed_outline(
            &mut computed,
            components,
            Some(parent_computed),
            Some(parent_entity),
            parent_changed,
        );
        if let Ok(cs) = child_query.get(entity) {
            let parent_computed = computed.clone();
            for child in cs.iter() {
                propagate_computed_outline(
                    &parent_computed,
                    changed,
                    entity,
                    *child,
                    child_query_mut,
                    child_query,
                );
            }
        }
    }
}

fn update_computed_outline(
    computed: &mut ComputedOutline,
    ((transform, changed_transform), volume, stencil, mode): QueryItem<'_, OutlineComponents>,
    parent_computed: Option<&ComputedOutline>,
    parent_entity: Option<Entity>,
    force_update: bool,
) -> bool {
    let changed = force_update
        || !computed.is_valid
        || computed.inherited_from != parent_entity
        || (changed_transform && matches!(mode, Some((OutlineMode::FlatVertex { .. }, _))))
        || is_changed(volume, computed.set_volume)
        || is_changed(stencil, computed.set_stencil)
        || is_changed(mode, computed.set_mode);
    if changed {
        if let Some(base) = parent_computed {
            *computed = base.clone();
        }
        computed.is_valid = true;
        computed.inherited_from = parent_entity;
        if let Some((vol, _)) = volume {
            computed.volume_enabled = vol.visible && vol.colour.a() != 0.0;
            computed.volume_offset = vol.width;
            computed.volume_colour = vol.colour.into();
            computed.set_volume = true;
        } else {
            computed.set_volume = false;
        }
        if let Some((sten, _)) = stencil {
            computed.stencil_enabled = sten.enabled;
            computed.stencil_offset = sten.offset;
            computed.set_stencil = true;
        } else {
            computed.set_stencil = false;
        }
        if let Some((m, _)) = mode {
            match m {
                OutlineMode::FlatVertex {
                    model_origin: origin,
                } => {
                    computed.world_origin = transform.compute_matrix().project_point3(*origin);
                    computed.depth_mode = DepthMode::Flat;
                }
                OutlineMode::RealVertex => {
                    computed.world_origin = Vec3::NAN;
                    computed.depth_mode = DepthMode::Real;
                }
            }
            computed.set_mode = true;
        } else {
            computed.set_mode = false;
        }
    }
    changed
}

fn is_changed<T: Component>(tuple: Option<(&T, bool)>, previously_set: bool) -> bool {
    tuple.is_some() != previously_set || if let Some((_, c)) = tuple { c } else { false }
}

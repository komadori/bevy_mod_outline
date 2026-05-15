use bevy::camera::primitives::{Aabb, Frustum};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers, SetViewVisibility};
use bevy::camera::Camera;
use bevy::math::primitives::ViewFrustum;
use bevy::math::{Affine3A, Mat4, UVec2, Vec3, Vec4Swizzles};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::sync_world::{MainEntity, MainEntityHashMap};
use bevy::render::view::{
    RenderExtractedVisibleEntitiesClass, RenderVisibleEntitiesClass, RetainedViewEntity,
};
use bevy::render::Extract;

use crate::computed::ComputedOutline;

/// Per-entity, per-view information collected during visibility checking.
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct OutlineVisibleEntity {
    /// Screen-space bounds of the outline, in physical pixels of the render
    /// target.
    pub screen_space_bounds: URect,
}

/// Per-view information about outlined entities visible from a view, in the
/// main world.
#[derive(Default, Clone, Debug)]
pub(crate) struct OutlineViewVisibleEntities {
    pub visible_entities: Vec<(Entity, OutlineVisibleEntity)>,
}

#[derive(Resource, Default, Clone, Debug)]
pub(crate) struct OutlineVisibleEntities {
    pub views: HashMap<RetainedViewEntity, OutlineViewVisibleEntities>,
}

#[derive(Default, Clone, Debug)]
pub(crate) struct RenderExtractedOutlineViewEntities {
    pub visible_entities_info: MainEntityHashMap<OutlineVisibleEntity>,
    pub visible_entities: RenderExtractedVisibleEntitiesClass,
}

#[derive(Resource, Default, Clone, Debug)]
pub(crate) struct RenderExtractedOutlineEntities {
    pub views: HashMap<RetainedViewEntity, RenderExtractedOutlineViewEntities>,
}

#[derive(Default, Clone, Debug)]
pub(crate) struct RenderOutlineViewEntities(pub RenderVisibleEntitiesClass);

#[derive(Resource, Default, Clone, Debug)]
pub(crate) struct RenderOutlineEntities {
    pub views: HashMap<RetainedViewEntity, RenderOutlineViewEntities>,
}

#[allow(clippy::type_complexity)]
pub(crate) fn check_outline_view_visibility(
    mut visible: ResMut<OutlineVisibleEntities>,
    views: Query<(
        Entity,
        &Camera,
        &GlobalTransform,
        Option<&RenderLayers>,
        &Frustum,
    )>,
    mut outlines: Query<(
        Entity,
        &ComputedOutline,
        Option<&Aabb>,
        &GlobalTransform,
        Has<NoFrustumCulling>,
        &mut ViewVisibility,
    )>,
) {
    visible.views.retain(|retained, _| {
        views
            .get(retained.main_entity.id())
            .ok()
            .and_then(|(_, camera, _, _, _)| live_viewport(camera))
            .is_some()
    });

    for (view_entity, camera, view_transform, view_mask, frustum) in views.iter() {
        let Some(physical_viewport) = live_viewport(camera) else {
            continue;
        };

        let retained_view_entity = RetainedViewEntity::new(MainEntity::from(view_entity), None, 0);
        let view_visible = visible.views.entry(retained_view_entity).or_default();
        view_visible.visible_entities.clear();

        let view_mask = view_mask.cloned().unwrap_or_default();
        let view_from_world = view_transform.to_matrix().inverse();
        let clip_from_world = camera.clip_from_view() * view_from_world;
        let scale_factor = camera.target_scaling_factor().unwrap_or(1.0);

        for (entity, computed, aabb, transform, no_frustum_culling, mut entity_in_view) in
            outlines.iter_mut()
        {
            let Some(computed) = &computed.0 else {
                continue;
            };

            // 1) Outline enabled.
            let volume_enabled = computed.volume.value.enabled;
            let stencil_enabled = computed.stencil.value.enabled.is_enabled(volume_enabled);
            if !volume_enabled && !stencil_enabled {
                continue;
            }

            // 2) Render layer intersection.
            if !view_mask.intersects(&computed.layers.value) {
                continue;
            }

            let world_from_local = transform.affine();

            let screen_space_bounds = if let (Some(aabb), false) = (aabb, no_frustum_culling) {
                // 3) Mesh AABB at least partly in front of the near plane.
                let near = &frustum.half_spaces[ViewFrustum::NEAR_PLANE_IDX];
                let aabb_center_world = world_from_local.transform_point3a(aabb.center).extend(1.0);
                let relative_radius =
                    aabb.relative_radius(&near.normal(), &world_from_local.matrix3);
                if near.normal_d().dot(aabb_center_world) + relative_radius <= 0.0 {
                    continue;
                }

                // 4) Compute screen-space bounds and check overlap with the viewport.
                let border = (scale_factor * computed.volume.value.offset).ceil() as u32;
                let Some(bounds) = compute_screen_space_bounds(
                    aabb,
                    &world_from_local,
                    &clip_from_world,
                    physical_viewport,
                    border,
                ) else {
                    continue;
                };
                bounds
            } else {
                physical_viewport
            };

            view_visible.visible_entities.push((
                entity,
                OutlineVisibleEntity {
                    screen_space_bounds,
                },
            ));
            entity_in_view.set_visible();
        }
    }
}

fn live_viewport(camera: &Camera) -> Option<URect> {
    if !camera.is_active {
        return None;
    }
    let physical_viewport = camera.physical_viewport_rect()?;
    let viewport_size = physical_viewport.size();
    if viewport_size.x == 0 || viewport_size.y == 0 {
        return None;
    }
    Some(physical_viewport)
}

pub(crate) fn extract_outline_visible_entities(
    main_visible: Extract<Res<OutlineVisibleEntities>>,
    mut render_extracted: ResMut<RenderExtractedOutlineEntities>,
) {
    let render_extracted = &mut *render_extracted;

    render_extracted
        .views
        .retain(|view, _| main_visible.views.contains_key(view));

    for (view, main_view) in main_visible.views.iter() {
        let render_view = render_extracted.views.entry(*view).or_default();

        render_view.visible_entities_info.clear();
        render_view.visible_entities_info.extend(
            main_view
                .visible_entities
                .iter()
                .map(|(entity, info)| (MainEntity::from(*entity), *info)),
        );

        let entities = &mut render_view.visible_entities.entities;
        entities.clear();
        entities.extend(
            main_view
                .visible_entities
                .iter()
                .map(|(entity, _)| (Entity::PLACEHOLDER, MainEntity::from(*entity))),
        );
    }
}

pub(crate) fn collect_outline_cpu_culled_entities(
    mut render_extracted: ResMut<RenderExtractedOutlineEntities>,
    mut render_outline: ResMut<RenderOutlineEntities>,
) {
    let render_extracted = &mut *render_extracted;
    let render_outline = &mut *render_outline;

    render_outline
        .views
        .retain(|view, _| render_extracted.views.contains_key(view));

    for (view, render_view_extracted) in render_extracted.views.iter_mut() {
        let render_view_outline = render_outline.views.entry(*view).or_default();

        render_view_outline.0.prepare_for_new_frame();

        render_view_extracted
            .visible_entities
            .entities
            .sort_unstable_by_key(|(_, main_entity)| *main_entity);

        render_view_outline
            .0
            .update_cpu_culled_entities(&render_view_extracted.visible_entities.entities);
    }
}

fn compute_screen_space_bounds(
    aabb: &Aabb,
    world_from_local: &Affine3A,
    clip_from_world: &Mat4,
    physical_viewport: URect,
    border: u32,
) -> Option<URect> {
    let min = aabb.min();
    let max = aabb.max();
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
    ];

    let viewport_size = physical_viewport.size();
    let width = viewport_size.x;
    let height = viewport_size.y;

    let mut min_screen = UVec2::MAX;
    let mut max_screen = UVec2::MIN;

    for corner in corners.iter() {
        let world_pos = world_from_local.transform_point3(*corner);
        let clip_pos = *clip_from_world * world_pos.extend(1.0);
        let ndc = clip_pos.xyz() / clip_pos.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            continue;
        }
        let screen_x = ((ndc.x + 1.0) * 0.5 * width as f32).max(0.0) as u32;
        let screen_y = ((-ndc.y + 1.0) * 0.5 * height as f32).max(0.0) as u32;
        min_screen.x = min_screen.x.min(screen_x);
        min_screen.y = min_screen.y.min(screen_y);
        max_screen.x = max_screen.x.max(screen_x);
        max_screen.y = max_screen.y.max(screen_y);
    }

    if min_screen.x >= max_screen.x || min_screen.y >= max_screen.y {
        None
    } else {
        Some(URect::new(
            physical_viewport.min.x + min_screen.x.saturating_sub(border).min(width),
            physical_viewport.min.y + min_screen.y.saturating_sub(border).min(height),
            physical_viewport.min.x + (max_screen.x + border).min(width),
            physical_viewport.min.y + (max_screen.y + border).min(height),
        ))
    }
}

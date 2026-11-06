use bevy::prelude::*;

pub(crate) struct ViewportInfo {
    pub physical_rect: URect,
    pub logical_size: Vec2,
    pub scaling_factor: f32,
}

pub(crate) fn live_viewport(camera: &Camera) -> Option<ViewportInfo> {
    if !camera.is_active {
        return None;
    }
    let info = ViewportInfo {
        physical_rect: camera.physical_viewport_rect()?,
        logical_size: camera.logical_viewport_size()?,
        scaling_factor: camera.target_scaling_factor()?,
    };
    if info.physical_rect.is_empty() {
        return None;
    }
    Some(info)
}

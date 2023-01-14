use std::f32::consts::PI;

use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    prelude::{shape::Torus, *},
    render::{camera::Viewport, view::RenderLayers},
    window::close_on_esc,
};
use bevy_mod_outline::{OutlineBundle, OutlinePlugin, OutlineRenderLayers, OutlineVolume};

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins)
        .add_plugin(OutlinePlugin)
        .add_startup_system(setup)
        .add_system(close_on_esc)
        .add_system(set_camera_viewports)
        .run();
}

const OBJECT_LAYER_ID: u8 = 1;
const OUTLINE_LAYER_ID: u8 = 2;

#[derive(Copy, Clone, Component)]
struct CameraMode {
    object_layer: bool,
    outline_layer: bool,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add torus using the regular surface normals for outlining
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Torus {
                radius: 0.6,
                ring_radius: 0.2,
                subdivisions_segments: 40,
                subdivisions_sides: 20,
            })),
            material: materials.add(Color::rgb(0.1, 0.1, 0.9).into()),
            transform: Transform::from_rotation(Quat::from_rotation_x(0.5 * PI))
                .with_translation(0.8 * Vec3::Y),
            ..default()
        })
        .insert(OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                colour: Color::WHITE,
                width: 10.0,
            },
            ..default()
        })
        .insert(RenderLayers::layer(OBJECT_LAYER_ID))
        .insert(OutlineRenderLayers(RenderLayers::layer(OUTLINE_LAYER_ID)));

    // Add plane and light source
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(bevy::prelude::shape::Plane { size: 5.0 })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..default()
    });
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
    });

    // Add cameras for different combinations of render-layers
    for i in 0..4 {
        let object_layer = i & 1 == 1;
        let outline_layer = (i >> 1) & 1 == 1;
        let mut layers = RenderLayers::default();
        if object_layer {
            layers = layers.with(OBJECT_LAYER_ID);
        }
        if outline_layer {
            layers = layers.with(OUTLINE_LAYER_ID);
        }
        commands
            .spawn(Camera3dBundle {
                camera: Camera {
                    priority: i,
                    ..default()
                },
                camera_3d: Camera3d {
                    clear_color: if i > 0 {
                        ClearColorConfig::None
                    } else {
                        ClearColorConfig::Default
                    },
                    ..default()
                },
                transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                ..default()
            })
            .insert(CameraMode {
                object_layer,
                outline_layer,
            })
            .insert(layers);
    }
}

fn set_camera_viewports(windows: Res<Windows>, mut query: Query<(&mut Camera, &CameraMode)>) {
    if windows.is_changed() {
        // Divide window into quadrants
        let win = windows.primary();
        let size = UVec2::new(win.physical_width() / 2, win.physical_height() / 2);
        for (mut camera, mode) in query.iter_mut() {
            let offset = UVec2::new(
                if mode.object_layer { size.x } else { 0 },
                if mode.outline_layer { size.y } else { 0 },
            );
            camera.viewport = Some(Viewport {
                physical_position: offset,
                physical_size: size,
                ..default()
            });
        }
    }
}

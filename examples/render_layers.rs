use std::f32::consts::PI;

use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    prelude::{
        shape::{Plane, Torus},
        *,
    },
    render::{camera::Viewport, view::RenderLayers},
    window::{close_on_esc, PrimaryWindow},
};
use bevy_mod_outline::{OutlineBundle, OutlinePlugin, OutlineRenderLayers, OutlineVolume};

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (close_on_esc, set_camera_viewports))
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
        mesh: meshes.add(Mesh::from(Plane {
            size: 5.0,
            subdivisions: 0,
        })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..default()
    });
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
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
                    order: i,
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

fn set_camera_viewports(
    win_query: Query<(&Window, Changed<Window>), With<PrimaryWindow>>,
    mut query: Query<(&mut Camera, &CameraMode)>,
) {
    let (win, win_changed) = win_query.get_single().unwrap();
    if win_changed {
        // Divide window into quadrants
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

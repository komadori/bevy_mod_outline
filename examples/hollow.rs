use std::f32::consts::{PI, TAU};

use bevy::{prelude::*, scene::SceneInstance, window::close_on_esc};
use bevy_mod_gltf_patched::GltfPlugin;
use bevy_mod_outline::*;

fn main() {
    App::new()
        // Disable built-in glTF plugin
        .add_plugins(DefaultPlugins.build().disable::<bevy::gltf::GltfPlugin>())
        // Register outline normal vertex attribute with bevy_mod_gltf_patched
        .add_plugins(
            GltfPlugin::default()
                .add_custom_vertex_attribute("_OUTLINE_NORMAL", ATTRIBUTE_OUTLINE_NORMAL),
        )
        .add_plugins(OutlinePlugin)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1.0,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (setup_scene_once_loaded, rotates, rotates_hue, close_on_esc),
        )
        .run();
}

#[derive(Component)]
struct Rotates;

#[derive(Component)]
struct RotatesHue;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(20.0, 20.0, 30.0)
            .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        ..default()
    });

    // Light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        ..default()
    });

    // Hollow
    commands
        .spawn(SceneBundle {
            scene: asset_server.load("hollow.glb#Scene0"),
            ..default()
        })
        .insert(Rotates)
        .insert(ComputedOutlineDepth::default());
}

// Once the scene is loaded, start the animation and add an outline
fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<&SceneInstance>,
    scene_manager: Res<SceneSpawner>,
    name_query: Query<&Name, With<Handle<StandardMaterial>>>,
    mut done: Local<bool>,
) {
    if !*done {
        if let Ok(scene) = scene_query.get_single() {
            if scene_manager.instance_is_ready(**scene) {
                for entity in scene_manager.iter_instance_entities(**scene) {
                    commands
                        .entity(entity)
                        .insert(OutlineBundle {
                            outline: OutlineVolume {
                                visible: true,
                                width: 7.5,
                                colour: Color::BLUE,
                            },
                            stencil: OutlineStencil {
                                enabled: true,
                                offset: 0.0,
                            },
                            ..default()
                        })
                        .insert(InheritOutlineDepth);
                    if let Ok(name) = name_query.get(entity) {
                        if name.as_str() == "inside" {
                            commands.entity(entity).insert(RotatesHue);
                        }
                    }
                }
                *done = true;
            }
        }
    }
}

fn rotates(mut query: Query<&mut Transform, With<Rotates>>, timer: Res<Time>, mut t: Local<f32>) {
    *t = (*t + timer.delta_seconds()) % TAU;
    let a = t.sin();
    for mut transform in query.iter_mut() {
        *transform = Transform::from_rotation(Quat::from_rotation_y(a));
    }
}

fn rotates_hue(
    query: Query<&mut Handle<StandardMaterial>, With<RotatesHue>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    timer: Res<Time>,
) {
    for handle in query.iter() {
        let material = materials.get_mut(handle).unwrap();
        let mut colour = material.base_color.as_hsla_f32();
        colour[0] = (colour[0] + 15.0 * timer.delta_seconds()) % 360.0;
        material.base_color = Color::Hsla {
            hue: colour[0],
            saturation: colour[1],
            lightness: colour[2],
            alpha: colour[3],
        };
    }
}

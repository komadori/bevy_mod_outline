use std::f32::consts::TAU;

use bevy::prelude::*;

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, rotates)
        .run();
}

#[derive(Component)]
struct Rotates;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add sphere with child meshes sticking out of it
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere::new(0.75).mesh().uv(30, 30)),
            material: materials.add(StandardMaterial::from(Color::srgb(0.9, 0.1, 0.1))),
            transform: Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            ..default()
        })
        .insert(OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                colour: Color::WHITE,
                width: 10.0,
            },
            stencil: OutlineStencil {
                offset: 5.0,
                ..default()
            },
            ..default()
        })
        .insert(Rotates)
        .with_children(|parent| {
            parent
                .spawn(PbrBundle {
                    mesh: meshes.add(
                        Capsule3d::new(0.2, 1.0)
                            .mesh()
                            .rings(15)
                            .latitudes(15)
                            .longitudes(15)
                            .build(),
                    ),
                    material: materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9))),
                    transform: Transform::from_rotation(Quat::from_axis_angle(Vec3::X, TAU / 4.0))
                        .with_translation(Vec3::new(0.0, 0.0, 0.75)),
                    ..default()
                })
                .insert(InheritOutlineBundle::default());
            parent
                .spawn(PbrBundle {
                    mesh: meshes.add(
                        Torus {
                            minor_radius: 0.1,
                            major_radius: 0.5,
                        }
                        .mesh()
                        .minor_resolution(15)
                        .major_resolution(30)
                        .build(),
                    ),
                    material: materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9))),
                    transform: Transform::from_rotation(Quat::from_axis_angle(Vec3::Z, TAU / 4.0))
                        .with_translation(Vec3::new(0.0, 0.0, -0.75)),
                    ..default()
                })
                .insert(InheritOutlineBundle::default());
        });

    // Add plane, light source, and camera
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::new(5.0, 5.0)).mesh().build()),
        material: materials.add(StandardMaterial::from(Color::srgb(0.3, 0.5, 0.3))),
        ..default()
    });
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

fn rotates(mut query: Query<&mut Transform, With<Rotates>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.rotate_axis(Dir3::Y, 0.75 * timer.delta_seconds());
    }
}

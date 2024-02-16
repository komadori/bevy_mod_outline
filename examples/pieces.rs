use std::f32::consts::TAU;

use bevy::{
    prelude::{
        shape::{Capsule, Plane, Torus, UVSphere},
        *,
    },
    window::close_on_esc,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (close_on_esc, rotates))
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
            mesh: meshes.add(Mesh::from(UVSphere {
                radius: 0.75,
                sectors: 30,
                stacks: 30,
            })),
            material: materials.add(StandardMaterial::from(Color::rgb(0.9, 0.1, 0.1))),
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
                    mesh: meshes.add(Mesh::from(Capsule {
                        radius: 0.2,
                        rings: 15,
                        depth: 1.0,
                        latitudes: 15,
                        longitudes: 15,
                        ..Default::default()
                    })),
                    material: materials.add(StandardMaterial::from(Color::rgb(0.1, 0.1, 0.9))),
                    transform: Transform::from_rotation(Quat::from_axis_angle(Vec3::X, TAU / 4.0))
                        .with_translation(Vec3::new(0.0, 0.0, 0.75)),
                    ..default()
                })
                .insert(InheritOutlineBundle::default());
            parent
                .spawn(PbrBundle {
                    mesh: meshes.add(Mesh::from(Torus {
                        radius: 0.5,
                        ring_radius: 0.1,
                        subdivisions_segments: 30,
                        subdivisions_sides: 15,
                    })),
                    material: materials.add(StandardMaterial::from(Color::rgb(0.1, 0.1, 0.9))),
                    transform: Transform::from_rotation(Quat::from_axis_angle(Vec3::Z, TAU / 4.0))
                        .with_translation(Vec3::new(0.0, 0.0, -0.75)),
                    ..default()
                })
                .insert(InheritOutlineBundle::default());
        });

    // Add plane, light source, and camera
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Plane {
            size: 5.0,
            subdivisions: 0,
        })),
        material: materials.add(StandardMaterial::from(Color::rgb(0.3, 0.5, 0.3))),
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
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

fn rotates(mut query: Query<&mut Transform, With<Rotates>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.rotate_axis(Vec3::Y, 0.75 * timer.delta_seconds());
    }
}

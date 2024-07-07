use std::f32::consts::{PI, TAU};

use bevy::{
    core_pipeline::bloom::{BloomCompositeMode, BloomPrefilterSettings, BloomSettings},
    prelude::*,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (rotates, pulses))
        .run();
}

#[derive(Component)]
struct Rotates;

#[derive(Component)]
struct Pulses(f32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add emissive sphere
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere::new(0.75).mesh().uv(50, 50)),
            material: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                ..default()
            }),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            ..default()
        })
        .insert(OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                colour: Color::WHITE,
                width: 5.0,
            },
            ..default()
        })
        .insert(Pulses(0.0));

    // Add satellite
    commands
        .spawn(TransformBundle::default())
        .insert(Rotates)
        .with_children(|parent| {
            parent
                .spawn(PbrBundle {
                    mesh: meshes.add(Sphere::new(0.25).mesh().uv(25, 25)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(1.0, 0.0, 0.0),
                        emissive: LinearRgba::rgb(100.0, 0.0, 0.0),
                        ..default()
                    }),
                    transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.25)),
                    ..default()
                })
                .insert(OutlineBundle {
                    outline: OutlineVolume {
                        visible: true,
                        colour: Color::WHITE,
                        width: 5.0,
                    },
                    ..default()
                })
                .insert(Pulses(PI));
        });

    // Add HDR camera
    commands
        .spawn(Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),

            ..default()
        })
        .insert(BloomSettings {
            intensity: 1.0,
            low_frequency_boost: 0.5,
            low_frequency_boost_curvature: 0.5,
            high_pass_frequency: 0.5,
            prefilter_settings: BloomPrefilterSettings {
                threshold: 3.0,
                threshold_softness: 0.6,
            },
            composite_mode: BloomCompositeMode::Additive,
        });
}

fn rotates(mut query: Query<&mut Transform, With<Rotates>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.rotate_axis(Dir3::Y, 0.75 * timer.delta_seconds());
    }
}

fn pulses(
    mut query: Query<(&mut OutlineVolume, &Pulses)>,
    timer: Res<Time>,
    mut state: Local<f32>,
) {
    *state = (*state + 0.3 * timer.delta_seconds()) % TAU;
    for (mut outline, phase) in query.iter_mut() {
        let t = (*state + phase.0).sin().max(0.0);
        outline.width = (15.0 * t).min(7.5);
    }
}

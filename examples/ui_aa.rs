use bevy::{
    core_pipeline::{
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        fxaa::Fxaa,
        smaa::{SmaaPreset, SmaaSettings},
    },
    prelude::*,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, TemporalAntiAliasPlugin, OutlinePlugin))
        .insert_resource(Msaa::Off)
        .insert_state(AAMode::NoAA)
        .add_systems(Startup, setup)
        .add_systems(Update, (bounce, highlight, interaction))
        .add_systems(OnEnter(AAMode::MSAAx4), |mut commands: Commands| {
            commands.insert_resource(Msaa::Sample4)
        })
        .add_systems(OnExit(AAMode::MSAAx4), |mut commands: Commands| {
            commands.insert_resource(Msaa::Off)
        })
        .add_systems(
            OnEnter(AAMode::FXAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .insert(Fxaa::default());
            },
        )
        .add_systems(
            OnExit(AAMode::FXAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .remove::<Fxaa>();
            },
        )
        .add_systems(
            OnEnter(AAMode::SMAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .insert(SmaaSettings {
                        preset: SmaaPreset::Ultra,
                    });
            },
        )
        .add_systems(
            OnExit(AAMode::SMAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .remove::<SmaaSettings>();
            },
        )
        .add_systems(
            OnEnter(AAMode::TAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .insert(TemporalAntiAliasBundle::default());
            },
        )
        .add_systems(
            OnExit(AAMode::TAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .remove::<TemporalAntiAliasBundle>();
            },
        )
        .run();
}

#[derive(Component)]
struct Bounce;

#[derive(Component)]
struct TheCamera;

#[derive(States, Component, Clone, Debug, PartialEq, Eq, Hash)]
enum AAMode {
    NoAA,
    MSAAx4,
    FXAA,
    SMAA,
    TAA,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add spheres
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere::new(1.0).mesh().ico(9).unwrap()),
            material: materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5))),
            transform: Transform::from_translation(Vec3::new(-1.5, 0.0, 0.0)),
            ..default()
        })
        .insert(OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                width: 25.0,
                colour: Color::srgb(1.0, 1.0, 0.0),
            },
            ..default()
        })
        .insert(Bounce);
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere::new(1.0).mesh().ico(20).unwrap()),
            material: materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5))),
            transform: Transform::from_translation(Vec3::new(1.5, 0.0, 0.0)),
            ..default()
        })
        .insert(Bounce);

    // Add buttons
    commands
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            for mode in [
                AAMode::NoAA,
                AAMode::MSAAx4,
                AAMode::FXAA,
                AAMode::SMAA,
                AAMode::TAA,
            ] {
                parent
                    .spawn(ButtonBundle {
                        style: Style {
                            margin: UiRect::all(Val::Px(5.0)),
                            padding: UiRect::all(Val::Px(5.0)),
                            border: UiRect::all(Val::Px(5.0)),
                            ..default()
                        },
                        ..default()
                    })
                    .insert(BackgroundColor(Color::WHITE))
                    .insert(mode.clone())
                    .with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section(
                                format!("{:?}", mode),
                                TextStyle {
                                    color: Color::BLACK,
                                    font_size: 64.0,
                                    ..default()
                                },
                            ),
                            ..default()
                        });
                    });
            }
        });

    // Add light source and camera
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .insert(TheCamera);
}

fn bounce(mut query: Query<&mut Transform, With<Bounce>>, timer: Res<Time>, mut t: Local<f32>) {
    *t = (*t + timer.delta_seconds()) % 4.0;
    let y = (*t - 2.0).abs() - 1.0;
    for mut transform in query.iter_mut() {
        transform.translation.y = y;
    }
}

fn highlight(mut query: Query<(&mut BorderColor, &AAMode)>, state: Res<State<AAMode>>) {
    for (mut border, m) in query.iter_mut() {
        *border = if m == state.get() {
            BorderColor(Color::srgb(0.0, 0.0, 1.0))
        } else {
            BorderColor(Color::BLACK)
        };
    }
}

fn interaction(
    query: Query<(&Interaction, &AAMode)>,
    mut armed: Local<bool>,
    mut state: ResMut<NextState<AAMode>>,
) {
    let mut any_pressed = false;
    for (i, m) in query.iter() {
        match i {
            Interaction::Pressed => {
                any_pressed = true;
                if *armed {
                    state.set(m.clone());
                }
            }
            _ => {}
        }
    }
    *armed = !any_pressed;
}

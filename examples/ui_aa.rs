use bevy::{
    anti_alias::{
        fxaa::Fxaa,
        smaa::{Smaa, SmaaPreset},
        taa::TemporalAntiAliasing,
    },
    feathers::{
        controls::{
            FeathersMenu, FeathersMenuButton, FeathersMenuItem, FeathersMenuPopup, FeathersSlider,
        },
        dark_theme::create_dark_theme,
        theme::{ThemedText, UiTheme},
        FeathersPlugins,
    },
    input_focus::tab_navigation::TabGroup,
    prelude::*,
    state::state::FreelyMutableState,
    ui_widgets::{
        popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide},
        slider_self_update, Activate, SliderPrecision, SliderStep, SliderValue,
    },
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins,
            OutlinePlugin::EXTRUDE_VERTEX,
            FeathersPlugins,
        ))
        .insert_resource(UiTheme(create_dark_theme()))
        .init_state::<AAMode>()
        .init_state::<OutlineAAMode>()
        .init_state::<OutlineKind>()
        .add_systems(Startup, (setup, ui.spawn()))
        .add_systems(
            Update,
            (
                bounce,
                update_label::<AAMode, AaLabel>.run_if(state_changed::<AAMode>),
                update_label::<OutlineAAMode, OutlineMsaaLabel>
                    .run_if(state_changed::<OutlineAAMode>),
                update_label::<OutlineKind, OutlineKindLabel>.run_if(state_changed::<OutlineKind>),
                apply_scene_aa.run_if(state_changed::<AAMode>),
                apply_outline_msaa.run_if(state_changed::<OutlineAAMode>),
                apply_outline_kind.run_if(state_changed::<OutlineKind>),
            ),
        )
        .run();
}

#[derive(Component)]
struct Bounce;

#[derive(Component)]
struct TheCamera;

#[derive(Component)]
struct TheOutline;

#[derive(Component, Default, Clone)]
struct AaLabel;

#[derive(Component, Default, Clone)]
struct OutlineMsaaLabel;

#[derive(Component, Default, Clone)]
struct OutlineKindLabel;

#[derive(Component, Default, Clone)]
struct SpeedSlider;

#[derive(Copy, Clone, States, Default, PartialEq, Eq, Hash)]
enum AAMode {
    #[default]
    None,
    Msaa(Msaa),
    Fxaa,
    Smaa,
    Taa,
}

impl std::fmt::Debug for AAMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AAMode::None => write!(f, "None"),
            AAMode::Msaa(msaa) => write!(f, "MSAA x{}", msaa.samples()),
            AAMode::Fxaa => write!(f, "FXAA"),
            AAMode::Smaa => write!(f, "SMAA"),
            AAMode::Taa => write!(f, "TAA"),
        }
    }
}

#[derive(Copy, Clone, States, Debug, Default, PartialEq, Eq, Hash)]
enum OutlineAAMode {
    #[default]
    Auto,
    Off,
    Sample2,
    Sample4,
    Sample8,
}

#[derive(Copy, Clone, States, Debug, Default, PartialEq, Eq, Hash)]
enum OutlineKind {
    #[default]
    Extrude,
    Flood,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add spheres
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(9).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        Transform::from_translation(Vec3::new(-1.5, 0.0, 0.0)),
        OutlineVolume {
            visible: true,
            width: 25.0,
            colour: Color::srgb(1.0, 1.0, 0.0),
        },
        TheOutline,
        Bounce,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(20).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        Transform::from_translation(Vec3::new(1.5, 0.0, 0.0)),
        Bounce,
    ));

    // Add light source and camera
    commands.spawn((
        PointLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
        TheCamera,
    ));
}

/// A toolbar pinned to the top-left corner.
fn ui() -> impl SceneList {
    bsn_list![(
        Node {
            position_type: PositionType::Absolute,
            top: px(8),
            left: px(8),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        TabGroup
        Children [
            aa_menu(),
            outline_msaa_menu(),
            outline_method_menu(),
            (Text("Speed") ThemedText),
            (
                @FeathersSlider {
                    @min: 0.0,
                    @max: 2.0,
                    @value: 1.0,
                }
                Node { width: px(220) }
                SliderStep(0.1)
                SliderPrecision(1)
                SpeedSlider
                on(slider_self_update)
            ),
        ]
    )]
}

/// A drop-down menu: a button captioned `caption` that opens a popup listing `items`.
fn menu(caption: impl SceneList + 'static, items: impl SceneList + 'static) -> impl Scene {
    bsn! {
        @FeathersMenu
        Children [
            (@FeathersMenuButton {
                @caption: {Box::new(caption) as Box<dyn SceneList>}
            }),
            (@FeathersMenuPopup Children [ {Box::new(items) as Box<dyn SceneList>} ])
        ]
    }
}

/// Menu for selecting the scene-wide anti-aliasing mode.
fn aa_menu() -> impl Scene {
    menu(
        bsn_list![
            (Text("Anti-aliasing: ") ThemedText),
            (Text("") ThemedText AaLabel),
        ],
        bsn_list![
            menu_item(AAMode::None),
            msaa_submenu(),
            menu_item(AAMode::Fxaa),
            menu_item(AAMode::Smaa),
            menu_item(AAMode::Taa),
        ],
    )
}

/// Nested sub-menu for picking the MSAA sample count when MSAA is selected.
fn msaa_submenu() -> impl Scene {
    bsn! {
        @FeathersMenu
        Children [
            (@FeathersMenuButton {
                @caption: {bsn! { Text("MSAA") ThemedText }}
            }),
            (
                @FeathersMenuPopup
                Popover {
                    positions: vec![
                        PopoverPlacement {
                            side: PopoverSide::Right,
                            align: PopoverAlign::Start,
                            gap: 2.0,
                        },
                        PopoverPlacement {
                            side: PopoverSide::Left,
                            align: PopoverAlign::Start,
                            gap: 2.0,
                        },
                    ],
                    window_margin: 10.0,
                }
                Children [
                    (menu_item(AAMode::Msaa(Msaa::Sample2))),
                    (menu_item(AAMode::Msaa(Msaa::Sample4))),
                    (menu_item(AAMode::Msaa(Msaa::Sample8))),
                ]
            )
        ]
    }
}

/// Menu for selecting the outline MSAA override.
fn outline_msaa_menu() -> impl Scene {
    menu(
        bsn_list![
            (Text("Outline MSAA: ") ThemedText),
            (Text("") ThemedText OutlineMsaaLabel),
        ],
        bsn_list![
            menu_item(OutlineAAMode::Auto),
            menu_item(OutlineAAMode::Off),
            menu_item(OutlineAAMode::Sample2),
            menu_item(OutlineAAMode::Sample4),
            menu_item(OutlineAAMode::Sample8),
        ],
    )
}

/// Menu for selecting the outline rendering method.
fn outline_method_menu() -> impl Scene {
    menu(
        bsn_list![
            (Text("Outline method: ") ThemedText),
            (Text("") ThemedText OutlineKindLabel),
        ],
        bsn_list![
            menu_item(OutlineKind::Extrude),
            menu_item(OutlineKind::Flood),
        ],
    )
}

/// A menu item, captioned with the `Debug` representation of `value`, which selects `value` for
/// state `S` when activated.
fn menu_item<S: FreelyMutableState + Copy>(value: S) -> impl Scene {
    let label = format!("{value:?}");
    bsn! {
        @FeathersMenuItem {
            @caption: {bsn! { Text(label) ThemedText }}
        }
        on(move |_: On<Activate>, mut next: ResMut<NextState<S>>| {
            next.set(value);
        })
    }
}

fn bounce(
    mut query: Query<&mut Transform, With<Bounce>>,
    slider: Option<Single<&SliderValue, With<SpeedSlider>>>,
    timer: Res<Time>,
    mut t: Local<f32>,
) {
    let speed = slider.map_or(1.0, |s| s.0);
    *t = (*t + timer.delta_secs() * speed) % 4.0;
    let y = (*t - 2.0).abs() - 1.0;
    for mut transform in query.iter_mut() {
        transform.translation.y = y;
    }
}

/// Writes the `Debug` representation of state `S` into every `Text` marked with `L`.
fn update_label<S: States + std::fmt::Debug, L: Component>(
    state: Res<State<S>>,
    mut query: Query<&mut Text, With<L>>,
) {
    for mut text in query.iter_mut() {
        text.0 = format!("{:?}", state.get());
    }
}

/// Applies the selected scene-wide anti-aliasing mode to the camera.
fn apply_scene_aa(
    state: Res<State<AAMode>>,
    camera: Single<Entity, With<TheCamera>>,
    mut commands: Commands,
) {
    let mut entity = commands.entity(*camera);
    entity
        .remove::<Fxaa>()
        .remove::<Smaa>()
        .remove::<TemporalAntiAliasing>();
    match *state.get() {
        AAMode::None => {
            entity.insert(Msaa::Off);
        }
        AAMode::Msaa(msaa) => {
            entity.insert(msaa);
        }
        AAMode::Fxaa => {
            entity.insert((Msaa::Off, Fxaa::default()));
        }
        AAMode::Smaa => {
            entity.insert((
                Msaa::Off,
                Smaa {
                    preset: SmaaPreset::Ultra,
                },
            ));
        }
        AAMode::Taa => {
            entity.insert((Msaa::Off, TemporalAntiAliasing::default()));
        }
    }
}

fn apply_outline_msaa(
    state: Res<State<OutlineAAMode>>,
    camera: Single<Entity, With<TheCamera>>,
    mut commands: Commands,
) {
    let outline_msaa = match state.get() {
        OutlineAAMode::Auto => OutlineMsaa::Auto,
        OutlineAAMode::Off => OutlineMsaa::Msaa(Msaa::Off),
        OutlineAAMode::Sample2 => OutlineMsaa::Msaa(Msaa::Sample2),
        OutlineAAMode::Sample4 => OutlineMsaa::Msaa(Msaa::Sample4),
        OutlineAAMode::Sample8 => OutlineMsaa::Msaa(Msaa::Sample8),
    };
    commands.entity(*camera).insert(outline_msaa);
}

fn apply_outline_kind(
    state: Res<State<OutlineKind>>,
    outline: Single<Entity, With<TheOutline>>,
    mut commands: Commands,
) {
    let mode = match state.get() {
        OutlineKind::Extrude => OutlineMode::ExtrudeFlat,
        OutlineKind::Flood => OutlineMode::FloodFlat,
    };
    commands.entity(*outline).insert(mode);
}

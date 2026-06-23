use std::f32::consts::FRAC_PI_2;

use bevy::{
    feathers::{
        controls::{
            FeathersMenu, FeathersMenuButton, FeathersMenuItem, FeathersMenuPopup, FeathersSlider,
        },
        dark_theme::create_dark_theme,
        theme::{ThemedText, UiTheme},
        FeathersPlugins,
    },
    input_focus::tab_navigation::TabGroup,
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
    render::RenderDebugFlags,
    state::state::FreelyMutableState,
    ui_widgets::{slider_self_update, Activate, SliderPrecision, SliderStep, SliderValue},
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
            WireframePlugin {
                debug_flags: RenderDebugFlags::empty(),
            },
        ))
        .insert_resource(UiTheme(create_dark_theme()))
        .init_state::<DrawMode>()
        .init_state::<DrawFace>()
        .init_state::<DrawShape>()
        .init_resource::<Shapes>()
        .add_systems(Startup, (setup, ui.spawn()))
        .add_systems(
            Update,
            (
                update_label::<DrawMode, ModeLabel>.run_if(state_changed::<DrawMode>),
                update_label::<DrawFace, FaceLabel>.run_if(state_changed::<DrawFace>),
                update_label::<DrawShape, ShapeLabel>.run_if(state_changed::<DrawShape>),
                apply_mode.run_if(state_changed::<DrawMode>),
                apply_face.run_if(state_changed::<DrawFace>),
                apply_shape.run_if(state_changed::<DrawShape>),
                apply_rotation,
            ),
        )
        .run();
}

#[derive(Component)]
struct TheObject;

#[derive(Component, Default, Clone)]
struct ModeLabel;

#[derive(Component, Default, Clone)]
struct FaceLabel;

#[derive(Component, Default, Clone)]
struct ShapeLabel;

#[derive(Component, Default, Clone)]
struct RotationSlider;

#[derive(Copy, Clone, States, Debug, Default, PartialEq, Eq, Hash)]
enum DrawMode {
    #[default]
    ExtrudeFlat,
    ExtrudeReal,
    FloodFlat,
}

#[derive(Copy, Clone, States, Debug, Default, PartialEq, Eq, Hash)]
enum DrawFace {
    #[default]
    Front,
    DoubleSided,
}

#[derive(Copy, Clone, States, Debug, Default, PartialEq, Eq, Hash)]
enum DrawShape {
    #[default]
    Cone,
    Triangle,
    Rectangle,
    Circle,
}

#[derive(Resource)]
struct Shapes {
    cone: Handle<Mesh>,
    triangle: Handle<Mesh>,
    rectangle: Handle<Mesh>,
    circle: Handle<Mesh>,
}

impl Shapes {
    fn get(&self, shape: DrawShape) -> Handle<Mesh> {
        match shape {
            DrawShape::Cone => self.cone.clone(),
            DrawShape::Triangle => self.triangle.clone(),
            DrawShape::Rectangle => self.rectangle.clone(),
            DrawShape::Circle => self.circle.clone(),
        }
    }
}

impl FromWorld for Shapes {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.get_resource_mut::<Assets<Mesh>>().unwrap();
        let settings = GenerateOutlineNormalsFrom::ExternalBisector.into();
        Self {
            cone: meshes.add(
                Cone::new(1.0, 1.0)
                    .mesh()
                    .build()
                    .rotated_by(Quat::from_rotation_x(FRAC_PI_2))
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
            triangle: meshes.add(
                Triangle2d::new(
                    Vec2::new(0.0, 1.0),
                    Vec2::new(1.0, -1.0),
                    Vec2::new(-1.0, -1.0),
                )
                .mesh()
                .build()
                .with_generated_outline_normals(&settings)
                .unwrap(),
            ),
            rectangle: meshes.add(
                Rectangle::new(2.0, 2.0)
                    .mesh()
                    .build()
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
            circle: meshes.add(
                Circle::new(1.0)
                    .mesh()
                    .build()
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
        }
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shapes: Res<Shapes>,
) {
    // Add shape
    commands.spawn((
        Mesh3d(shapes.triangle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        OutlineVolume {
            visible: true,
            width: 25.0,
            colour: Color::srgb(1.0, 1.0, 0.0),
        },
        Wireframe,
        TheObject,
    ));

    // Add light source and camera
    commands.spawn((PointLight::default(), Transform::from_xyz(4.0, 8.0, 4.0)));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
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
            mode_menu(),
            face_menu(),
            shape_menu(),
            (Text("Rotation") ThemedText),
            (
                @FeathersSlider {
                    @min: -180.0,
                    @max: 180.0,
                    @value: 0.0,
                }
                Node { width: px(220) }
                SliderStep(1.0)
                SliderPrecision(0)
                RotationSlider
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

/// Menu for selecting the outline mode.
fn mode_menu() -> impl Scene {
    menu(
        bsn_list![
            (Text("Outline mode: ") ThemedText),
            (Text("") ThemedText ModeLabel),
        ],
        bsn_list![
            menu_item(DrawMode::ExtrudeFlat),
            menu_item(DrawMode::ExtrudeReal),
            menu_item(DrawMode::FloodFlat),
        ],
    )
}

/// Menu for selecting which faces of the outline are rendered.
fn face_menu() -> impl Scene {
    menu(
        bsn_list![
            (Text("Outline face: ") ThemedText),
            (Text("") ThemedText FaceLabel),
        ],
        bsn_list![menu_item(DrawFace::Front), menu_item(DrawFace::DoubleSided),],
    )
}

/// Menu for selecting which shape mesh is displayed.
fn shape_menu() -> impl Scene {
    menu(
        bsn_list![
            (Text("Shape: ") ThemedText),
            (Text("") ThemedText ShapeLabel),
        ],
        bsn_list![
            menu_item(DrawShape::Cone),
            menu_item(DrawShape::Triangle),
            menu_item(DrawShape::Rectangle),
            menu_item(DrawShape::Circle),
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

/// Writes the `Debug` representation of state `S` into every `Text` marked with `L`.
fn update_label<S: States + std::fmt::Debug, L: Component>(
    state: Res<State<S>>,
    mut query: Query<&mut Text, With<L>>,
) {
    for mut text in query.iter_mut() {
        text.0 = format!("{:?}", state.get());
    }
}

fn apply_mode(
    state: Res<State<DrawMode>>,
    object: Single<Entity, With<TheObject>>,
    mut commands: Commands,
) {
    let mode = match state.get() {
        DrawMode::ExtrudeFlat => OutlineMode::ExtrudeFlat,
        DrawMode::ExtrudeReal => OutlineMode::ExtrudeReal,
        DrawMode::FloodFlat => OutlineMode::FloodFlat,
    };
    commands.entity(*object).insert(mode);
}

fn apply_face(
    state: Res<State<DrawFace>>,
    object: Single<Entity, With<TheObject>>,
    mut commands: Commands,
) {
    let face = match state.get() {
        DrawFace::Front => OutlineFace::Front,
        DrawFace::DoubleSided => OutlineFace::DoubleSided,
    };
    commands.entity(*object).insert(face);
}

fn apply_shape(
    state: Res<State<DrawShape>>,
    object: Single<Entity, With<TheObject>>,
    shapes: Res<Shapes>,
    mut commands: Commands,
) {
    commands
        .entity(*object)
        .insert(Mesh3d(shapes.get(*state.get())));
}

fn apply_rotation(
    slider: Option<Single<&SliderValue, With<RotationSlider>>>,
    mut query: Query<&mut Transform, With<TheObject>>,
) {
    let Some(slider) = slider else {
        return;
    };
    let radians = slider.0.to_radians();
    for mut transform in query.iter_mut() {
        transform.rotation = Quat::from_rotation_y(radians);
    }
}

use bevy::ecs::component::Components;
use bevy::prelude::*;

use crate::{
    CursorLocked, EYE_HEIGHT, GroundedSecs, Gun, Item, ItemKey, ItemRegistry, ItemState,
    PLATFORM_HALF, PLATFORM_THICKNESS, PLATFORM_TOP_Y, Player, Rusty, View, ViewOf, look_around,
    register_builtin_items, toggle_cursor, update_player, view_on_rust, view_on_state_change,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let mut registry = ItemRegistry::default();
        register_builtin_items(&mut registry);

        app.insert_resource(CursorLocked::default())
            .insert_resource(registry)
            .add_observer(view_on_state_change)
            .add_observer(view_on_rust)
            .add_systems(Startup, (setup, spawn_gun))
            .add_systems(
                Update,
                (
                    toggle_cursor,
                    look_around,
                    update_player,
                    pickup_item,
                    drop_item,
                    rust_grounded_items,
                    update_hud,
                ),
            );
    }
}

const PICKUP_RANGE: f32 = 1.5;
const DROP_DISTANCE: f32 = 2.0;
/// Cumulative seconds on the ground before an item rusts.
const RUST_AFTER_SECS: f32 = 5.0;

#[derive(Component)]
struct ModelHudText;

#[derive(Component)]
struct ViewHudText;

/// The `Without`s prove to the scheduler that the text nodes are disjoint
/// from the `EntityRef` queries over models and views.
type HudTextQuery<'w, 's, T, O> =
    Query<'w, 's, &'static mut Text, (With<T>, Without<O>, Without<Item>, Without<ViewOf>)>;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            PLATFORM_HALF * 2.0,
            PLATFORM_THICKNESS,
            PLATFORM_HALF * 2.0,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.3, 0.9),
            ..default()
        })),
        Transform::from_xyz(0.0, PLATFORM_TOP_Y - PLATFORM_THICKNESS * 0.5, 0.0),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, PLATFORM_TOP_Y + EYE_HEIGHT, 0.0),
            Player::default(),
            AmbientLight {
                brightness: 200.0,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.15, 0.6))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.6, 0.4),
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(0.3, -0.3, -0.5),
            ));
        });

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        Text::new("model:"),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        ModelHudText,
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(10.0),
            ..default()
        },
        Text::new("view:"),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        ViewHudText,
    ));
}

fn spawn_gun(mut commands: Commands) {
    // Spawning the model is all it takes: the `On<Insert, ItemState>`
    // observer builds the view.
    commands.spawn((
        Item {
            key: ItemKey("core::item::gun".to_string()),
            label: "Gun".to_string(),
        },
        Gun,
        GroundedSecs::default(),
        ItemState::OnGround(Vec3::new(0.0, 0.0, -5.0)),
    ));
}

/// Walking over a grounded item picks it up. The transition is a single
/// component re-insert on the model; the observer does the rest.
fn pickup_item(
    player: Query<(Entity, &Transform), With<Player>>,
    items: Query<(Entity, &ItemState), With<Item>>,
    mut commands: Commands,
) {
    let Ok((player_e, player_t)) = player.single() else {
        return;
    };

    for (item_e, state) in &items {
        let ItemState::OnGround(pos) = state else {
            continue;
        };
        let dist = (player_t.translation - *pos).with_y(0.0).length();
        if dist < PICKUP_RANGE {
            commands
                .entity(item_e)
                .insert(ItemState::EquippedBy(player_e));
        }
    }
}

/// G drops the equipped item in front of the player.
fn drop_item(
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(Entity, &Transform), With<Player>>,
    items: Query<(Entity, &ItemState), With<Item>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyG) {
        return;
    }
    let Ok((player_e, player_t)) = player.single() else {
        return;
    };

    for (item_e, state) in &items {
        if !matches!(state, ItemState::EquippedBy(holder) if *holder == player_e) {
            continue;
        }
        let forward = player_t.forward().with_y(0.0).normalize_or_zero();
        let pos = (player_t.translation + forward * DROP_DISTANCE).with_y(PLATFORM_TOP_Y);
        commands.entity(item_e).insert(ItemState::OnGround(pos));
        return;
    }
}

/// Ground exposure accumulates on the model and eventually rusts it. `Rusty`
/// goes on the model entity, so it survives every later transition — this is
/// the property the whole architecture exists to guarantee.
type RustableItems<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ItemState, &'static mut GroundedSecs),
    (With<Item>, Without<Rusty>),
>;

fn rust_grounded_items(time: Res<Time>, mut items: RustableItems, mut commands: Commands) {
    for (item_e, state, mut grounded) in &mut items {
        if !matches!(state, ItemState::OnGround(_)) {
            continue;
        }
        grounded.0 += time.delta_secs();
        if grounded.0 >= RUST_AFTER_SECS {
            commands.entity(item_e).insert(Rusty);
        }
    }
}

/// Shows the split live: the model line keeps the same entity id and grows
/// components (Rusty), while the view line's entity id changes on every
/// transition.
fn update_hud(
    models: Query<EntityRef, With<Item>>,
    views: Query<EntityRef, With<ViewOf>>,
    components: &Components,
    mut model_text: HudTextQuery<ModelHudText, ViewHudText>,
    mut view_text: HudTextQuery<ViewHudText, ModelHudText>,
) {
    let component_names = |entity: EntityRef| -> String {
        entity
            .archetype()
            .components()
            .iter()
            .map(|&id| {
                components
                    .get_name(id)
                    .map(|name| name.to_string())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let model_line = match models.iter().next() {
        Some(model) => format!("model: {} [{}]", model.id(), component_names(model)),
        None => "model: <none>".to_string(),
    };
    let view_line = match models
        .iter()
        .next()
        .and_then(|model| model.get::<View>())
        .and_then(View::entity)
        .and_then(|view| views.get(view).ok())
    {
        Some(view) => format!("view: {} [{}]", view.id(), component_names(view)),
        None => "view: <none>".to_string(),
    };

    // Only write on change so the UI doesn't relayout every frame.
    if let Ok(mut text) = model_text.single_mut()
        && text.0 != model_line
    {
        text.0 = model_line;
    }
    if let Ok(mut text) = view_text.single_mut()
        && text.0 != view_line
    {
        text.0 = view_line;
    }
}

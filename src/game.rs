use bevy::ecs::component::Components;
use bevy::prelude::*;

#[cfg(debug_assertions)]
use crate::check_item_invariants;
use crate::{
    Contains, CursorLocked, EYE_HEIGHT, GroundedSecs, Gun, Item, ItemComponentsPlugin, ItemKey,
    ItemRegistry, ItemState, ItemTransitions, PLATFORM_HALF, PLATFORM_THICKNESS, PLATFORM_TOP_Y,
    Player, View, ViewOf, ground_items_of_dying_holder, look_around, register_builtin_items,
    repair_on_link_lost, toggle_cursor, update_player, view_on_state_change,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let mut registry = ItemRegistry::default();
        register_builtin_items(&mut registry);

        app.insert_resource(CursorLocked::default())
            .insert_resource(registry)
            .add_plugins(ItemComponentsPlugin)
            .add_observer(view_on_state_change)
            .add_observer(ground_items_of_dying_holder)
            .add_observer(repair_on_link_lost)
            .add_systems(Startup, (setup, spawn_guns))
            .add_systems(
                Update,
                (
                    toggle_cursor,
                    look_around,
                    update_player,
                    pickup_items,
                    equip_from_bag,
                    drop_equipped,
                    update_hud,
                ),
            );
        #[cfg(debug_assertions)]
        app.add_systems(Last, check_item_invariants);
    }
}

const PICKUP_RANGE: f32 = 1.5;
const DROP_DISTANCE: f32 = 2.0;

/// Which line of the HUD a text node renders.
#[derive(Component)]
enum HudLine {
    Models,
    Views,
    Carrying,
}

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

    // One flex column: blocks are multi-line and wrap, so they must push
    // each other down instead of sitting at fixed offsets.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            max_width: Val::Percent(55.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|parent| {
            for line in [HudLine::Carrying, HudLine::Models, HudLine::Views] {
                parent.spawn((
                    Text::default(),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    line,
                ));
            }
        });
}

/// An item enters the world by being dropped into it: the model bundle
/// carries only durable data, and `drop_at` gives it its state and position
/// through the same door every other transition uses.
fn spawn_guns(mut commands: Commands) {
    for (n, pos) in [Vec3::new(0.0, 0.0, -5.0), Vec3::new(3.0, 0.0, -6.5)]
        .into_iter()
        .enumerate()
    {
        commands
            .spawn((
                Item {
                    key: ItemKey("core::item::gun".to_string()),
                    label: format!("Gun {}", n + 1),
                },
                Gun,
                GroundedSecs::default(),
                Visibility::default(),
            ))
            .drop_at(pos);
    }
}

/// Walking over a grounded item stows it in the bag. `Q` draws it.
fn pickup_items(
    player: Query<(Entity, &Transform), With<Player>>,
    items: Query<(Entity, &ItemState, &Transform), With<Item>>,
    mut commands: Commands,
) {
    let Ok((player_e, player_t)) = player.single() else {
        return;
    };

    for (item_e, state, item_t) in &items {
        if !state.is_on_ground() {
            continue;
        }
        let dist = (player_t.translation - item_t.translation)
            .with_y(0.0)
            .length();
        if dist < PICKUP_RANGE {
            commands.entity(item_e).store_in(player_e);
        }
    }
}

/// `Q` equips the first stowed item. `equip_to`'s policy stows whatever was
/// in the hand, so repeated presses cycle through the bag.
fn equip_from_bag(
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(Entity, Option<&Contains>), With<Player>>,
    states: Query<&ItemState, With<Item>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let Ok((player_e, Some(contains))) = player.single() else {
        return;
    };
    let stored = contains
        .iter()
        .find(|&held| states.get(held).is_ok_and(ItemState::is_stored));
    if let Some(item) = stored {
        commands.entity(item).equip_to(player_e);
    }
}

/// `G` drops the equipped item in front of the player.
fn drop_equipped(
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(&Transform, Option<&Contains>), With<Player>>,
    states: Query<&ItemState, With<Item>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyG) {
        return;
    }
    let Ok((player_t, Some(contains))) = player.single() else {
        return;
    };
    let equipped = contains
        .iter()
        .find(|&held| states.get(held).is_ok_and(ItemState::is_equipped));
    if let Some(item) = equipped {
        let forward = player_t.forward().with_y(0.0).normalize_or_zero();
        let pos = (player_t.translation + forward * DROP_DISTANCE).with_y(PLATFORM_TOP_Y);
        commands.entity(item).drop_at(pos);
    }
}

/// Shows the split live: model lines keep their entity ids and grow
/// components across transitions, view lines change entity per transition,
/// and the carrying line demonstrates the O(1) reverse query through
/// `Contains`.
type HudTexts<'w, 's> =
    Query<'w, 's, (&'static mut Text, &'static HudLine), (Without<Item>, Without<ViewOf>)>;

fn update_hud(
    models: Query<EntityRef, With<Item>>,
    views: Query<EntityRef, With<ViewOf>>,
    player: Query<Option<&Contains>, With<Player>>,
    components: &Components,
    mut texts: HudTexts,
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
    fn label_of<'a>(model: EntityRef<'a>) -> &'a str {
        model.get::<Item>().map_or("?", |item| item.label.as_str())
    }

    let mut model_lines: Vec<String> = models
        .iter()
        .map(|model| {
            let state = model.get::<ItemState>().map(ItemState::kind);
            format!(
                "{} {} {:?} [{}]",
                label_of(model),
                model.id(),
                state,
                component_names(model)
            )
        })
        .collect();
    model_lines.sort();

    let mut view_lines: Vec<String> = models
        .iter()
        .map(|model| {
            let view = model
                .get::<View>()
                .and_then(View::entity)
                .and_then(|view| views.get(view).ok());
            match view {
                Some(view) => format!(
                    "{} view {} [{}]",
                    label_of(model),
                    view.id(),
                    component_names(view)
                ),
                None => format!("{} view <none>", label_of(model)),
            }
        })
        .collect();
    view_lines.sort();

    let carrying = match player.single() {
        Ok(Some(contains)) => {
            let mut held: Vec<String> = contains
                .iter()
                .map(|item| match models.get(item) {
                    Ok(model) => format!(
                        "{} ({:?})",
                        label_of(model),
                        model.get::<ItemState>().map(ItemState::kind)
                    ),
                    Err(_) => format!("{item}"),
                })
                .collect();
            held.sort();
            format!("carrying: {}", held.join(", "))
        }
        _ => "carrying: <nothing>".to_string(),
    };

    for (mut text, line) in &mut texts {
        let new = match line {
            HudLine::Models => model_lines.join("\n"),
            HudLine::Views => view_lines.join("\n"),
            HudLine::Carrying => carrying.clone(),
        };
        // Only write on change so the UI doesn't relayout every frame.
        if text.0 != new {
            text.0 = new;
        }
    }
}

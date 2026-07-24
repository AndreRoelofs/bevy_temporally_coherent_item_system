use bevy::ecs::component::Components;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use crate::{
    Ammo, Cooldown, CursorLocked, CursorSystems, EYE_HEIGHT, EquippedBy, Equips, Firearm,
    GroundedSecs, HandSocket, InspectContributors, InventoryGrid, InventoryOpen, InventoryUi,
    InventoryUiOf, Item, ItemFootprint, ItemKey, ItemLabel, ItemPlugin, ItemStateMarkers,
    LookTarget, OnGround, PLATFORM_HALF, PLATFORM_THICKNESS, PLATFORM_TOP_Y, PackedAt, Player,
    StateKey, StoredIn, Stores, View, ViewOf, find_free_slot, inspect_lines, inventory_closed,
    look_around, on_ground_at, set_cursor_lock, toggle_cursor, update_player,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorLocked::default())
            .add_plugins(ItemPlugin)
            .add_systems(Startup, (setup, spawn_guns))
            .add_systems(
                Update,
                (
                    toggle_cursor.in_set(CursorSystems).run_if(inventory_closed),
                    toggle_inventory,
                    look_around,
                    update_player,
                    pickup_items,
                    equip_from_bag,
                    drop_equipped,
                    update_hud,
                ),
            );
    }
}

const PICKUP_RANGE: f32 = 1.5;
const DROP_DISTANCE: f32 = 2.0;

#[derive(Component)]
enum HudLine {
    Target,
    Carrying,
    Models,
    Views,
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
            InventoryGrid::new(UVec2::new(12, 8)),
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
            parent.spawn((
                HandSocket,
                Transform::from_xyz(-0.3, -0.3, -0.6),
                Visibility::default(),
            ));
        });

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
            for line in [
                HudLine::Target,
                HudLine::Carrying,
                HudLine::Models,
                HudLine::Views,
            ] {
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

fn spawn_guns(mut commands: Commands) {
    let guns = [
        (Vec3::new(0.0, 0.0, -5.0), UVec2::new(4, 4)),
        (Vec3::new(3.0, 0.0, -6.5), UVec2::new(8, 4)),
    ];
    for (n, (pos, footprint)) in guns.into_iter().enumerate() {
        commands
            .spawn((
                Item {
                    key: ItemKey("core::item::gun".to_string()),
                    label: ItemLabel(format!("Gun {}", n + 1)),
                },
                ItemFootprint(footprint),
                Firearm {
                    cooldown: Cooldown(0.5),
                    magazine_size: 8,
                },
                Ammo(8),
                GroundedSecs::default(),
                Visibility::default(),
            ))
            .insert(on_ground_at(pos));
    }
}

#[expect(clippy::type_complexity)]
fn pickup_items(
    player: Query<(Entity, &Transform, Option<&InventoryGrid>, Option<&Stores>), With<Player>>,
    items: Query<
        (Entity, &Transform, &ItemFootprint, Option<&PackedAt>),
        (With<Item>, With<OnGround>),
    >,
    layouts: Query<(&PackedAt, &ItemFootprint), With<Item>>,
    mut commands: Commands,
) {
    let Ok((player_e, player_t, grid, stores)) = player.single() else {
        return;
    };

    let mut occupied: Vec<(UVec2, UVec2)> = stores
        .map(|stores| {
            stores
                .iter()
                .filter_map(|held| layouts.get(held).ok())
                .map(|(packed, footprint)| (packed.origin(), footprint.0))
                .collect()
        })
        .unwrap_or_default();

    for (item_e, item_t, footprint, packed) in &items {
        let dist = (player_t.translation - item_t.translation)
            .with_y(0.0)
            .length();
        if dist >= PICKUP_RANGE {
            continue;
        }
        if let Some(grid) = grid {
            let preferred = packed.map(PackedAt::origin);
            let Some(slot) = find_free_slot(grid.size(), &occupied, footprint.0, preferred) else {
                continue;
            };
            occupied.push((slot, footprint.0));
        }
        commands.entity(item_e).insert(StoredIn(player_e));
    }
}

fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<InventoryOpen>,
    player: Query<&InventoryUi, With<Player>>,
    mut panels: Query<&mut Visibility, With<InventoryUiOf>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut locked: ResMut<CursorLocked>,
) {
    let tab = keys.just_pressed(KeyCode::Tab);
    let escape = keys.just_pressed(KeyCode::Escape) && open.0;
    if !tab && !escape {
        return;
    }
    open.0 = !open.0;
    if let Ok(Some(panel)) = player.single().map(InventoryUi::entity)
        && let Ok(mut visibility) = panels.get_mut(panel)
    {
        *visibility = if open.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut cursor) = cursors.single_mut() {
        set_cursor_lock(&mut cursor, &mut locked, !open.0 && tab);
    }
}

fn equip_from_bag(
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(Entity, &Stores), With<Player>>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let Ok((player_e, stores)) = player.single() else {
        return;
    };
    let stored = stores.iter().find(|&held| items.get(held).is_ok());
    if let Some(item) = stored {
        commands.entity(item).insert(EquippedBy(player_e));
    }
}

fn drop_equipped(
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(&Transform, &Equips), With<Player>>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyG) {
        return;
    }
    let Ok((player_t, equips)) = player.single() else {
        return;
    };
    let equipped = equips.iter().find(|&held| items.get(held).is_ok());
    if let Some(item) = equipped {
        let forward = player_t.forward().with_y(0.0).normalize_or_zero();
        let pos = (player_t.translation + forward * DROP_DISTANCE).with_y(PLATFORM_TOP_Y);
        commands.entity(item).insert(on_ground_at(pos));
    }
}

type HudTexts<'w, 's> =
    Query<'w, 's, (&'static mut Text, &'static HudLine), (Without<Item>, Without<ViewOf>)>;

#[expect(clippy::too_many_arguments)]
fn update_hud(
    models: Query<EntityRef, With<Item>>,
    views: Query<EntityRef, With<ViewOf>>,
    player: Query<(Option<&Equips>, Option<&Stores>), With<Player>>,
    target: Res<LookTarget>,
    contributors: Res<InspectContributors>,
    markers: Res<ItemStateMarkers>,
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
        model
            .get::<Item>()
            .map_or("?", |item| item.label.0.as_str())
    }

    let mut model_lines: Vec<String> = models
        .iter()
        .map(|model| {
            let state = markers
                .key_of(model)
                .map_or("<stateless>", StateKey::as_str);
            format!(
                "{} {} {} [{}]",
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

    let target_line = match target.0.and_then(|model| models.get(model).ok()) {
        Some(model) => format!(
            "target: {}",
            inspect_lines(model, &contributors, &markers).join(" · ")
        ),
        None => "target: <none>".to_string(),
    };

    let carrying = match player.single() {
        Ok((equips, stores)) => {
            let equipped = equips.into_iter().flat_map(|equips| equips.iter());
            let stored = stores.into_iter().flat_map(|stores| stores.iter());
            let mut held: Vec<String> = equipped
                .chain(stored)
                .map(|item| match models.get(item) {
                    Ok(model) => inspect_lines(model, &contributors, &markers).join(" · "),
                    Err(_) => format!("{item}"),
                })
                .collect();
            if held.is_empty() {
                "carrying: <nothing>".to_string()
            } else {
                held.sort();
                format!("carrying:\n  {}", held.join("\n  "))
            }
        }
        Err(_) => "carrying: <nothing>".to_string(),
    };

    for (mut text, line) in &mut texts {
        let new = match line {
            HudLine::Target => target_line.clone(),
            HudLine::Models => model_lines.join("\n"),
            HudLine::Views => view_lines.join("\n"),
            HudLine::Carrying => carrying.clone(),
        };
        if text.0 != new {
            text.0 = new;
        }
    }
}

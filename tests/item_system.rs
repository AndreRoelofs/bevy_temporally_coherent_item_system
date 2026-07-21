//! Headless proof of the architecture's guarantees: model components persist
//! across every transition, views are disposable, the two axes stay
//! coherent, and holders can carry equipped and stowed items at once.
//!
//! Note what these tests *cannot* do: construct an `ItemState` or a
//! `ContainedBy`. Both are sealed, so every transition below goes through
//! the `ItemTransitions` trait — the same door the game uses.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_temporally_coherent_item_system::{
    Ammo, CELL_PX, Contains, Cooldown, CooldownModifiers, CursorLocked, FireOutcome, Firearm,
    GroundedSecs, HandSocket, InspectContributors, InventoryGrid, InventoryUi, Item, ItemFootprint,
    ItemKey, ItemPacking, ItemPlugin, ItemState, ItemStateKind, ItemTransitions, LastShotAt,
    PackedAt, Player, Rusty, StatModifierCommands, StatOp, View, ViewOf, inspect_lines, try_fire,
};

/// Counts view spawns, so tests can assert refresh exactness.
#[derive(Resource, Default)]
struct ViewSpawns(usize);

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<bevy::scene::ScenePatch>();
    app.add_plugins(ItemPlugin);
    app.init_resource::<ButtonInput<MouseButton>>();

    app.init_resource::<ViewSpawns>();
    app.add_observer(|_: On<Add, ViewOf>, mut spawns: ResMut<ViewSpawns>| spawns.0 += 1);
    app
}

/// Run `f` against a fresh `Commands`, then flush and step the app.
fn with_commands(app: &mut App, f: impl FnOnce(&mut Commands)) {
    {
        let world = app.world_mut();
        let mut commands = world.commands();
        f(&mut commands);
    }
    app.world_mut().flush();
    app.update();
}

fn spawn_gun(app: &mut App, label: &str, pos: Vec3) -> Entity {
    let mut model = Entity::PLACEHOLDER;
    with_commands(app, |commands| {
        model = commands
            .spawn((
                Item {
                    key: ItemKey("core::item::gun".to_string()),
                    label: label.to_string(),
                },
                Firearm {
                    cooldown: Cooldown(0.5),
                    magazine_size: 8,
                },
                Ammo(8),
                GroundedSecs::default(),
                Visibility::default(),
            ))
            .drop_at(pos)
            .id();
    });
    model
}

fn view_entity(app: &App, model: Entity) -> Option<Entity> {
    app.world().get::<View>(model).and_then(View::entity)
}

fn kind(app: &App, model: Entity) -> Option<ItemStateKind> {
    app.world().get::<ItemState>(model).map(ItemState::kind)
}

fn view_spawns(app: &App) -> usize {
    app.world().resource::<ViewSpawns>().0
}

/// Read the folded cooldown the way any consumer does: the fold's one home
/// on `Firearm`, over the modifier list read straight off the model.
fn cooldown_of(app: &App, model: Entity) -> Option<f32> {
    let firearm = app.world().get::<Firearm>(model)?;
    Some(
        firearm
            .cooldown
            .effective(app.world().get::<CooldownModifiers>(model))
            .0,
    )
}

#[test]
fn model_components_persist_and_views_swap() {
    let mut app = test_app();
    let holder = app.world_mut().spawn(Transform::default()).id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);

    let ground_view = view_entity(&app, model).expect("grounded item has a view");
    assert!(app.world().get::<Mesh3d>(ground_view).is_some());
    assert_eq!(
        app.world().get::<ViewOf>(ground_view).map(|v| v.0),
        Some(model)
    );
    assert_eq!(
        app.world().get::<ChildOf>(ground_view).map(ChildOf::parent),
        Some(model),
        "ground views are placed by the model's own Transform"
    );

    // Arbitrary components accumulate on the model — including one this
    // library has never heard of.
    #[derive(Component)]
    struct Engraved(#[expect(dead_code)] String);
    app.world_mut()
        .entity_mut(model)
        .insert((Rusty, Engraved("To Andre".to_string())));
    app.update();

    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(holder);
    });
    assert_eq!(kind(&app, model), Some(ItemStateKind::Equipped));
    assert!(app.world().get::<Rusty>(model).is_some());
    assert!(app.world().get::<Engraved>(model).is_some());

    let hand_view = view_entity(&app, model).expect("equipped item has a view");
    assert_ne!(ground_view, hand_view);
    assert!(app.world().get_entity(ground_view).is_err());
    assert_eq!(
        app.world().get::<ChildOf>(hand_view).map(ChildOf::parent),
        Some(holder)
    );

    let drop_pos = Vec3::new(1.0, 0.0, 2.0);
    with_commands(&mut app, |commands| {
        commands.entity(model).drop_at(drop_pos);
    });
    assert_eq!(kind(&app, model), Some(ItemStateKind::OnGround));
    assert!(app.world().get::<ContainedBy>(model).is_none());
    assert_eq!(
        app.world().get::<Transform>(model).map(|t| t.translation),
        Some(drop_pos)
    );
    assert!(app.world().get::<Rusty>(model).is_some());
    assert!(app.world().get::<Engraved>(model).is_some());

    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(holder);
    });
    assert_eq!(kind(&app, model), Some(ItemStateKind::Stored));
    assert!(
        view_entity(&app, model).is_none(),
        "stored items have no view"
    );
    assert!(app.world().get::<Rusty>(model).is_some());
    assert!(app.world().get::<Engraved>(model).is_some());
}

use bevy_temporally_coherent_item_system::ContainedBy;

#[test]
fn unknown_key_leaves_model_intact() {
    let mut app = test_app();
    let mut model = Entity::PLACEHOLDER;
    with_commands(&mut app, |commands| {
        model = commands
            .spawn((
                Item {
                    key: ItemKey("core::item::typo".to_string()),
                    label: "Typo".to_string(),
                },
                Visibility::default(),
            ))
            .drop_at(Vec3::ZERO)
            .id();
    });

    assert!(view_entity(&app, model).is_none());
    assert!(app.world().get::<Item>(model).is_some());
    assert_eq!(kind(&app, model), Some(ItemStateKind::OnGround));
}

#[test]
fn despawning_the_model_despawns_the_view() {
    let mut app = test_app();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    let view = view_entity(&app, model).expect("view exists");

    app.world_mut().entity_mut(model).despawn();
    app.update();
    assert!(
        app.world().get_entity(view).is_err(),
        "linked_spawn ties the view's lifetime to the model"
    );
}

#[test]
fn view_refresh_is_exact() {
    let mut app = test_app();
    let holder = app.world_mut().spawn(Transform::default()).id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    assert_eq!(view_spawns(&app), 1, "spawn-by-drop builds one view");

    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(holder);
    });
    assert_eq!(view_spawns(&app), 1, "storing spawns no view");

    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(holder);
    });
    assert_eq!(view_spawns(&app), 2, "equipping builds exactly one view");

    with_commands(&mut app, |commands| {
        commands.entity(model).drop_at(Vec3::X);
    });
    assert_eq!(
        view_spawns(&app),
        3,
        "dropping builds exactly one view — the link removal must not double-refresh"
    );
    assert_eq!(kind(&app, model), Some(ItemStateKind::OnGround));
}

#[test]
fn carry_both_and_reverse_query() {
    let mut app = test_app();
    let player = app.world_mut().spawn(Transform::default()).id();
    let sword = spawn_gun(&mut app, "Sword", Vec3::ZERO);
    let gun = spawn_gun(&mut app, "Gun", Vec3::X);

    with_commands(&mut app, |commands| {
        commands.entity(gun).store_in(player);
        commands.entity(sword).equip_to(player);
    });

    assert_eq!(kind(&app, sword), Some(ItemStateKind::Equipped));
    assert_eq!(kind(&app, gun), Some(ItemStateKind::Stored));

    // The O(1) reverse query: everything the player carries, one lookup.
    let carried: Vec<Entity> = app
        .world()
        .get::<Contains>(player)
        .expect("player carries items")
        .iter()
        .collect();
    assert_eq!(carried.len(), 2);
    assert!(carried.contains(&sword) && carried.contains(&gun));
}

#[test]
fn single_equipped_policy_demotes_previous_weapon() {
    let mut app = test_app();
    let player = app.world_mut().spawn(Transform::default()).id();
    let first = spawn_gun(&mut app, "First", Vec3::ZERO);
    let second = spawn_gun(&mut app, "Second", Vec3::X);

    with_commands(&mut app, |commands| {
        commands.entity(first).equip_to(player);
    });
    with_commands(&mut app, |commands| {
        commands.entity(second).equip_to(player);
    });

    assert_eq!(
        kind(&app, first),
        Some(ItemStateKind::Stored),
        "equipping a second weapon stows the first"
    );
    assert_eq!(kind(&app, second), Some(ItemStateKind::Equipped));
    assert_eq!(
        app.world().get::<Contains>(player).map(Contains::len),
        Some(2)
    );
}

#[test]
fn holder_death_drops_entire_inventory() {
    let mut app = test_app();
    let death_pos = Vec3::new(3.0, 0.0, 4.0);
    let player = app
        .world_mut()
        .spawn(Transform::from_translation(death_pos))
        .id();
    let sword = spawn_gun(&mut app, "Sword", Vec3::ZERO);
    let gun = spawn_gun(&mut app, "Gun", Vec3::X);

    app.world_mut().entity_mut(sword).insert(Rusty);
    with_commands(&mut app, |commands| {
        commands.entity(sword).equip_to(player);
        commands.entity(gun).store_in(player);
    });

    app.world_mut().entity_mut(player).despawn();
    app.update();

    for model in [sword, gun] {
        assert_eq!(
            kind(&app, model),
            Some(ItemStateKind::OnGround),
            "everything the holder carried drops, equipped and stowed alike"
        );
        assert!(app.world().get::<ContainedBy>(model).is_none());
        assert_eq!(
            app.world().get::<Transform>(model).map(|t| t.translation),
            Some(death_pos),
            "items land where the holder died"
        );
        let view = view_entity(&app, model).expect("dropped item has a ground view");
        assert_eq!(
            app.world().get::<ChildOf>(view).map(ChildOf::parent),
            Some(model)
        );
    }
    assert!(app.world().get::<Rusty>(sword).is_some());
}

#[test]
fn cross_container_move_updates_both_sides() {
    let mut app = test_app();
    let chest = app.world_mut().spawn(Transform::default()).id();
    let player = app.world_mut().spawn(Transform::default()).id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);

    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(chest);
    });
    let spawns_before = view_spawns(&app);

    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(player);
    });

    assert_eq!(kind(&app, model), Some(ItemStateKind::Equipped));
    assert!(
        app.world().get::<Contains>(chest).is_none(),
        "the chest's side of the relationship empties automatically"
    );
    assert_eq!(
        app.world().get::<Contains>(player).map(Contains::len),
        Some(1)
    );
    assert_eq!(
        view_spawns(&app),
        spawns_before + 1,
        "a cross-container move is one refresh; the link replacement is silent"
    );
}

#[test]
fn model_despawn_while_contained_does_not_panic() {
    let mut app = test_app();
    let player = app.world_mut().spawn(Transform::default()).id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(player);
    });
    let view = view_entity(&app, model).expect("view exists");

    app.world_mut().entity_mut(model).despawn();
    app.update();
    assert!(app.world().get_entity(view).is_err());
    assert_eq!(app.world().get::<Contains>(player).map(Contains::len), None);
}

#[test]
fn grounded_movement_does_not_rebuild_view() {
    let mut app = test_app();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    let view = view_entity(&app, model).expect("view exists");
    let spawns_before = view_spawns(&app);

    // Moving a grounded item is a plain Transform mutation — the cadence
    // win: no transition, no observer, no rebuild.
    let new_pos = Vec3::new(5.0, 0.0, -2.0);
    app.world_mut()
        .entity_mut(model)
        .get_mut::<Transform>()
        .expect("model has a Transform")
        .translation = new_pos;
    app.update();

    assert_eq!(view_entity(&app, model), Some(view), "same view entity");
    assert_eq!(view_spawns(&app), spawns_before, "no view respawn");
    assert_eq!(
        app.world()
            .get::<GlobalTransform>(view)
            .map(GlobalTransform::translation),
        Some(new_pos),
        "the view follows through transform propagation"
    );
}

#[test]
fn rust_recolors_the_view_in_place() {
    let mut app = test_app();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    let view = view_entity(&app, model).expect("view exists");
    let base_material = app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(view)
        .expect("view has a material")
        .0
        .clone();
    let spawns_before = view_spawns(&app);

    app.world_mut().entity_mut(model).insert(Rusty);
    app.update();

    assert_eq!(
        view_entity(&app, model),
        Some(view),
        "a cosmetic change adjusts the view in place, no respawn"
    );
    assert_eq!(view_spawns(&app), spawns_before);
    let rusted = app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(view)
        .expect("view has a material")
        .0
        .clone();
    assert_ne!(base_material, rusted, "the material was swapped");

    // A rebuilt view (state transition) comes out rusty as well.
    let holder = app.world_mut().spawn(Transform::default()).id();
    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(holder);
    });
    let hand_view = view_entity(&app, model).expect("hand view exists");
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<StandardMaterial>>(hand_view)
            .map(|material| material.0.clone()),
        Some(rusted)
    );
}

#[test]
fn stat_fold_reacts_to_rust() {
    let mut app = test_app();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    assert_eq!(
        cooldown_of(&app, model),
        Some(0.5),
        "the fold over zero modifiers is the base"
    );

    app.world_mut().entity_mut(model).insert(Rusty);
    app.update();
    assert_eq!(
        cooldown_of(&app, model),
        Some(1.0),
        "rust doubles the cooldown through the fold, not by mutating Firearm"
    );
    assert!(
        app.world().get::<CooldownModifiers>(model).is_some(),
        "rust registered its tagged entry on the model"
    );
    assert_eq!(
        app.world()
            .get::<Firearm>(model)
            .map(|firearm| firearm.cooldown),
        Some(Cooldown(0.5)),
        "the base fact is untouched"
    );

    app.world_mut().entity_mut(model).remove::<Rusty>();
    app.update();
    assert_eq!(
        cooldown_of(&app, model),
        Some(0.5),
        "modifiers un-apply cleanly because they never wrote the base"
    );
    assert!(
        app.world().get::<CooldownModifiers>(model).is_none(),
        "rust cleaned up its entry; the emptied list is removed with it"
    );
}

#[test]
fn stat_stages_fold_deterministically_from_any_source() {
    // Sources defined by the TEST crate — extending the stat system needs
    // no library changes.
    #[derive(Component)]
    struct HeavyBarrel;
    #[derive(Component)]
    struct Blessing;

    let mut app = test_app();
    let a = spawn_gun(&mut app, "A", Vec3::ZERO);
    let b = spawn_gun(&mut app, "B", Vec3::X);
    app.world_mut()
        .entity_mut(a)
        .insert((HeavyBarrel, Blessing));
    app.world_mut()
        .entity_mut(b)
        .insert((HeavyBarrel, Blessing));

    with_commands(&mut app, |commands| {
        commands
            .entity(a)
            .set_stat_modifier::<HeavyBarrel, Cooldown>(StatOp::Flat(0.2))
            .set_stat_modifier::<Blessing, Cooldown>(StatOp::Mult(2.0));
        // The same two effects in the opposite order.
        commands
            .entity(b)
            .set_stat_modifier::<Blessing, Cooldown>(StatOp::Mult(2.0))
            .set_stat_modifier::<HeavyBarrel, Cooldown>(StatOp::Flat(0.2));
    });

    assert_eq!(
        cooldown_of(&app, a),
        Some(1.4),
        "(0.5 + 0.2) x 2.0 — flat folds before multipliers"
    );
    assert_eq!(
        cooldown_of(&app, a),
        cooldown_of(&app, b),
        "attach order is irrelevant"
    );

    with_commands(&mut app, |commands| {
        commands
            .entity(a)
            .set_stat_modifier::<HeavyBarrel, Cooldown>(StatOp::Flat(0.3));
    });
    assert_eq!(
        cooldown_of(&app, a),
        Some(1.6),
        "re-setting a source replaces its entry, never stacks"
    );
}

#[test]
fn try_fire_rules() {
    assert_eq!(
        try_fire(10.0, Cooldown(0.5), &Ammo(0), None),
        FireOutcome::Empty
    );
    assert_eq!(
        try_fire(10.0, Cooldown(0.5), &Ammo(3), None),
        FireOutcome::Fired
    );
    assert_eq!(
        try_fire(10.2, Cooldown(0.5), &Ammo(3), Some(&LastShotAt(10.0))),
        FireOutcome::Cooldown
    );
    assert_eq!(
        try_fire(10.6, Cooldown(0.5), &Ammo(3), Some(&LastShotAt(10.0))),
        FireOutcome::Fired
    );
}

#[test]
fn fired_ammo_persists_across_transitions() {
    let mut app = test_app();
    app.insert_resource(CursorLocked(true));
    let player = app
        .world_mut()
        .spawn((Player::default(), Transform::default()))
        .id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(player);
    });

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().get::<Ammo>(model).map(|ammo| ammo.0),
        Some(7),
        "one click, one shot"
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);

    // The thesis: the spent round survives the full round trip.
    with_commands(&mut app, |commands| {
        commands.entity(model).drop_at(Vec3::X);
    });
    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(player);
    });
    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(player);
    });
    assert_eq!(app.world().get::<Ammo>(model).map(|ammo| ammo.0), Some(7));
}

#[test]
fn chrome_handles_are_reused_across_rebuilds() {
    let mut app = test_app();
    let holder = app.world_mut().spawn(Transform::default()).id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    let meshes_before = app.world().resource::<Assets<Mesh>>().len();
    let patches_before = app
        .world()
        .resource::<Assets<bevy::scene::ScenePatch>>()
        .len();

    for _ in 0..3 {
        with_commands(&mut app, |commands| {
            commands.entity(model).equip_to(holder);
        });
        with_commands(&mut app, |commands| {
            commands.entity(model).drop_at(Vec3::X);
        });
    }
    assert_eq!(
        app.world().resource::<Assets<Mesh>>().len(),
        meshes_before,
        "chrome scenes reference pre-made handles instead of minting new assets"
    );
    assert_eq!(
        app.world()
            .resource::<Assets<bevy::scene::ScenePatch>>()
            .len(),
        patches_before,
        "patches are built once at plugin build, never per spawn"
    );
}

#[test]
fn equipped_view_parents_to_the_hand_socket() {
    let mut app = test_app();
    let holder = app
        .world_mut()
        .spawn((Player::default(), Transform::default()))
        .id();
    let socket = app
        .world_mut()
        .spawn((HandSocket, Transform::default(), ChildOf(holder)))
        .id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);

    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(holder);
    });
    let view = view_entity(&app, model).expect("equipped view exists");
    assert_eq!(
        app.world().get::<ChildOf>(view).map(ChildOf::parent),
        Some(socket),
        "views attach to the socket, not the holder root"
    );
}

#[test]
fn inspection_routes_agree_and_show_the_fold() {
    let mut app = test_app();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    app.world_mut().entity_mut(model).insert(Rusty);
    app.update();

    // Route 1: via the view (what the crosshair raycast resolves to).
    let view = view_entity(&app, model).expect("view exists");
    let via_view = app.world().get::<ViewOf>(view).expect("view links back").0;
    // Route 2: as an inventory listing would, via the model directly.
    assert_eq!(via_view, model, "both routes reach the same entity");

    let world = app.world();
    let lines = inspect_lines(world.entity(model), world.resource::<InspectContributors>());
    assert!(
        lines.iter().any(|line| line.contains("ammo 8/8")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("cooldown 1.00s")),
        "the tooltip shows the folded (rusted) cooldown: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("rusty")),
        "rust contributes its own line: {lines:?}"
    );
}

/// A holder whose storage is a grid — the survival-game backpack.
fn spawn_bag_holder(app: &mut App, grid: UVec2) -> Entity {
    let holder = app
        .world_mut()
        .spawn((Transform::default(), InventoryGrid::new(grid)))
        .id();
    app.update();
    holder
}

fn spawn_sized_gun(app: &mut App, label: &str, pos: Vec3, footprint: UVec2) -> Entity {
    let model = spawn_gun(app, label, pos);
    app.world_mut()
        .entity_mut(model)
        .insert(ItemFootprint(footprint));
    model
}

fn panel_of(app: &App, holder: Entity) -> Entity {
    app.world()
        .get::<InventoryUi>(holder)
        .and_then(InventoryUi::entity)
        .expect("gridded holder has an inventory panel")
}

fn packed_origin(app: &App, model: Entity) -> Option<UVec2> {
    app.world().get::<PackedAt>(model).map(PackedAt::origin)
}

fn background_of(app: &App, entity: Entity) -> Option<Color> {
    app.world()
        .get::<BackgroundColor>(entity)
        .map(|background| background.0)
}

#[test]
fn stored_items_pack_into_the_grid_with_icons() {
    let mut app = test_app();
    let holder = spawn_bag_holder(&mut app, UVec2::new(12, 8));
    let panel = panel_of(&app, holder);
    let pistol = spawn_sized_gun(&mut app, "Pistol", Vec3::ZERO, UVec2::new(4, 4));
    let rifle = spawn_sized_gun(&mut app, "Rifle", Vec3::X, UVec2::new(8, 4));

    with_commands(&mut app, |commands| {
        commands.entity(pistol).store_in(holder);
        commands.entity(rifle).store_in(holder);
    });

    assert_eq!(packed_origin(&app, pistol), Some(UVec2::ZERO));
    assert_eq!(
        packed_origin(&app, rifle),
        Some(UVec2::new(4, 0)),
        "first fit packs the rifle beside the pistol"
    );

    // The stored state has a view after all: a 2D icon on the bag's panel,
    // spawned through the same registry/refresh pipeline as the 3D views.
    let icon = view_entity(&app, rifle).expect("stored item in a gridded bag has an icon");
    assert_eq!(
        app.world().get::<ChildOf>(icon).map(ChildOf::parent),
        Some(panel),
        "icons live on the container's panel"
    );
    let node = app.world().get::<Node>(icon).expect("icons are UI nodes");
    assert_eq!(node.left, Val::Px(4.0 * CELL_PX));
    assert_eq!(node.top, Val::Px(0.0));
    assert_eq!(node.width, Val::Px(8.0 * CELL_PX));
    assert_eq!(node.height, Val::Px(4.0 * CELL_PX));
    assert_eq!(
        background_of(&app, icon),
        Some(Color::WHITE),
        "the item image is a pure white fill"
    );
    assert_eq!(
        node.border,
        UiRect::all(Val::Px(2.0)),
        "icons carry their own outline"
    );
    assert_eq!(
        app.world().get::<BorderColor>(icon),
        Some(&BorderColor::all(Color::BLACK)),
        "the outline is black so adjacent icons stay distinct"
    );
}

#[test]
fn a_full_bag_refuses_the_item_and_regrounds_it() {
    let mut app = test_app();
    let holder_pos = Vec3::new(2.0, 0.0, 3.0);
    let holder = app
        .world_mut()
        .spawn((
            Transform::from_translation(holder_pos),
            InventoryGrid::new(UVec2::new(4, 4)),
        ))
        .id();
    app.update();
    let first = spawn_sized_gun(&mut app, "First", Vec3::ZERO, UVec2::new(4, 4));
    let second = spawn_sized_gun(&mut app, "Second", Vec3::X, UVec2::new(4, 4));

    with_commands(&mut app, |commands| {
        commands.entity(first).store_in(holder);
        commands.entity(second).store_in(holder);
    });

    assert_eq!(kind(&app, first), Some(ItemStateKind::Stored));
    assert_eq!(
        kind(&app, second),
        Some(ItemStateKind::OnGround),
        "no room — the repair net re-grounds the item instead of leaving it spotless"
    );
    assert_eq!(
        app.world().get::<Transform>(second).map(|t| t.translation),
        Some(holder_pos),
        "the refused item lands at the holder"
    );
    assert!(packed_origin(&app, second).is_none());
    let view = view_entity(&app, second).expect("re-grounded items get a ground view");
    assert_eq!(
        app.world().get::<ChildOf>(view).map(ChildOf::parent),
        Some(second)
    );
}

#[test]
fn rust_tints_the_icon_and_repair_restores_it() {
    let mut app = test_app();
    let holder = spawn_bag_holder(&mut app, UVec2::new(12, 8));
    let model = spawn_sized_gun(&mut app, "Gun", Vec3::ZERO, UVec2::new(4, 4));
    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(holder);
    });
    let icon = view_entity(&app, model).expect("icon exists");
    assert_eq!(background_of(&app, icon), Some(Color::WHITE));

    app.world_mut().entity_mut(model).insert(Rusty);
    app.update();
    assert_eq!(
        view_entity(&app, model),
        Some(icon),
        "a cosmetic change adjusts the icon in place, no respawn"
    );
    let rusted = background_of(&app, icon).expect("icon still has a background");
    assert_ne!(rusted, Color::WHITE, "rust tints the 2D image too");

    // An icon rebuilt after a round trip through the world comes out rusty.
    with_commands(&mut app, |commands| {
        commands.entity(model).drop_at(Vec3::X);
    });
    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(holder);
    });
    let icon = view_entity(&app, model).expect("icon exists");
    assert_eq!(background_of(&app, icon), Some(rusted));

    app.world_mut().entity_mut(model).remove::<Rusty>();
    app.update();
    assert_eq!(
        background_of(&app, icon),
        Some(Color::WHITE),
        "repairing the rust restores the pure white image"
    );
}

#[test]
fn packing_memory_survives_transitions() {
    let mut app = test_app();
    let holder = spawn_bag_holder(&mut app, UVec2::new(12, 8));
    let model = spawn_sized_gun(&mut app, "Gun", Vec3::ZERO, UVec2::new(4, 4));
    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(holder);
    });
    assert_eq!(packed_origin(&app, model), Some(UVec2::ZERO));

    with_commands(&mut app, |commands| {
        commands.entity(model).repack_at(UVec2::new(5, 3));
    });
    assert_eq!(
        packed_origin(&app, model),
        Some(UVec2::new(5, 3)),
        "deliberate packing moves the item"
    );
    let icon = view_entity(&app, model).expect("icon exists");
    let node = app.world().get::<Node>(icon).expect("icon node");
    assert_eq!(
        node.left,
        Val::Px(5.0 * CELL_PX),
        "the icon follows the repack"
    );
    assert_eq!(node.top, Val::Px(3.0 * CELL_PX));

    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(holder);
    });
    with_commands(&mut app, |commands| {
        commands.entity(model).drop_at(Vec3::X);
    });
    assert_eq!(
        packed_origin(&app, model),
        Some(UVec2::new(5, 3)),
        "the spot is remembered while the item is out of the bag"
    );

    with_commands(&mut app, |commands| {
        commands.entity(model).store_in(holder);
    });
    assert_eq!(
        packed_origin(&app, model),
        Some(UVec2::new(5, 3)),
        "re-stowing returns the item to its remembered spot"
    );
}

#[test]
fn repacking_rejects_overlap_and_out_of_bounds() {
    let mut app = test_app();
    let holder = spawn_bag_holder(&mut app, UVec2::new(12, 8));
    let pistol = spawn_sized_gun(&mut app, "Pistol", Vec3::ZERO, UVec2::new(4, 4));
    let rifle = spawn_sized_gun(&mut app, "Rifle", Vec3::X, UVec2::new(8, 4));
    with_commands(&mut app, |commands| {
        commands.entity(pistol).store_in(holder);
        commands.entity(rifle).store_in(holder);
    });
    assert_eq!(packed_origin(&app, rifle), Some(UVec2::new(4, 0)));

    with_commands(&mut app, |commands| {
        commands.entity(rifle).repack_at(UVec2::new(2, 2));
    });
    assert_eq!(
        packed_origin(&app, rifle),
        Some(UVec2::new(4, 0)),
        "a spot overlapping the pistol is refused"
    );

    with_commands(&mut app, |commands| {
        commands.entity(rifle).repack_at(UVec2::new(5, 0));
    });
    assert_eq!(
        packed_origin(&app, rifle),
        Some(UVec2::new(4, 0)),
        "a spot past the right edge is refused"
    );

    with_commands(&mut app, |commands| {
        commands.entity(rifle).repack_at(UVec2::new(2, 4));
    });
    assert_eq!(
        packed_origin(&app, rifle),
        Some(UVec2::new(2, 4)),
        "a free in-bounds spot is accepted"
    );
}

#[test]
fn equip_swap_repacks_the_demoted_weapon() {
    let mut app = test_app();
    // A bag exactly the rifle's size: the swap only works because the
    // incoming weapon is promoted out of the grid before the old one
    // returns to Stored.
    let holder = spawn_bag_holder(&mut app, UVec2::new(8, 4));
    let rifle = spawn_sized_gun(&mut app, "Rifle", Vec3::ZERO, UVec2::new(8, 4));
    let pistol = spawn_sized_gun(&mut app, "Pistol", Vec3::X, UVec2::new(4, 4));

    with_commands(&mut app, |commands| {
        commands.entity(rifle).store_in(holder);
    });
    with_commands(&mut app, |commands| {
        commands.entity(rifle).equip_to(holder);
    });
    with_commands(&mut app, |commands| {
        commands.entity(pistol).store_in(holder);
    });
    with_commands(&mut app, |commands| {
        commands.entity(pistol).equip_to(holder);
    });

    assert_eq!(
        kind(&app, rifle),
        Some(ItemStateKind::Stored),
        "the demoted rifle fits back in because the pistol left the grid first"
    );
    assert_eq!(packed_origin(&app, rifle), Some(UVec2::ZERO));
    assert_eq!(kind(&app, pistol), Some(ItemStateKind::Equipped));
}

#[test]
fn raw_link_removal_is_repaired() {
    let mut app = test_app();
    let player = app.world_mut().spawn(Transform::default()).id();
    let model = spawn_gun(&mut app, "Gun", Vec3::ZERO);
    with_commands(&mut app, |commands| {
        commands.entity(model).equip_to(player);
    });

    // Misuse: yank the link without a transition. The repair net re-grounds
    // the item where it last lay instead of leaving a contradiction.
    with_commands(&mut app, |commands| {
        commands.entity(model).remove::<ContainedBy>();
    });

    assert_eq!(kind(&app, model), Some(ItemStateKind::OnGround));
    let view = view_entity(&app, model).expect("repaired item has a ground view");
    assert_eq!(
        app.world().get::<ChildOf>(view).map(ChildOf::parent),
        Some(model)
    );
}

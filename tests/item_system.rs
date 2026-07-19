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
    Contains, GroundedSecs, Gun, Item, ItemKey, ItemRegistry, ItemState, ItemStateKind,
    ItemTransitions, Rusty, View, ViewOf, ground_items_of_dying_holder, register_builtin_items,
    repair_on_link_lost, view_on_rust, view_on_state_change,
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

    let mut registry = ItemRegistry::default();
    register_builtin_items(&mut registry);
    app.insert_resource(registry);
    app.add_observer(view_on_state_change);
    app.add_observer(view_on_rust);
    app.add_observer(ground_items_of_dying_holder);
    app.add_observer(repair_on_link_lost);

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
                Gun,
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
    assert!(app.world().get::<Gun>(model).is_some());

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

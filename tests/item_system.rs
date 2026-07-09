//! Headless proof of the architecture's core guarantee: model components
//! persist across every state transition, while view entities are disposable.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_temporally_coherent_item_system::{
    GroundedSecs, Gun, Item, ItemKey, ItemRegistry, ItemState, Rusty, View, ViewOf,
    register_builtin_items, view_on_rust, view_on_state_change,
};

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<bevy::scene::ScenePatch>();

    let mut registry = ItemRegistry::default();
    register_builtin_items(&mut registry);
    app.insert_resource(registry);
    app.add_observer(view_on_state_change);
    app.add_observer(view_on_rust);
    app
}

fn spawn_gun(app: &mut App, state: ItemState) -> Entity {
    app.world_mut()
        .spawn((
            Item {
                key: ItemKey("core::item::gun".to_string()),
                label: "Gun".to_string(),
            },
            Gun,
            GroundedSecs::default(),
            state,
        ))
        .id()
}

fn view_entity(app: &App, model: Entity) -> Option<Entity> {
    app.world().get::<View>(model).and_then(View::entity)
}

#[test]
fn model_components_persist_and_views_swap() {
    let mut app = test_app();
    let holder = app.world_mut().spawn_empty().id();
    let model = spawn_gun(&mut app, ItemState::OnGround(Vec3::ZERO));
    app.update();

    let ground_view = view_entity(&app, model).expect("grounded item has a view");
    assert!(
        app.world().get::<Mesh3d>(ground_view).is_some(),
        "ground view carries the scene's mesh"
    );
    assert_eq!(
        app.world().get::<ViewOf>(ground_view).map(|v| v.0),
        Some(model)
    );

    // Something accumulates on the model while it lies around — including a
    // component the item system has never heard of (defined in this test
    // crate, not the library).
    #[derive(Component)]
    struct Engraved(#[expect(dead_code)] String);

    app.world_mut()
        .entity_mut(model)
        .insert((Rusty, Engraved("To Andre".to_string())));
    app.update();

    // Pick it up: one component re-insert, observer swaps the view.
    app.world_mut()
        .entity_mut(model)
        .insert(ItemState::EquippedBy(holder));
    app.update();

    assert!(
        app.world().get::<Rusty>(model).is_some(),
        "Rusty must survive pickup — the claim the old architecture broke"
    );
    assert!(
        app.world().get::<Engraved>(model).is_some(),
        "components unknown to the item system survive too"
    );
    assert!(app.world().get::<Gun>(model).is_some());

    let hand_view = view_entity(&app, model).expect("equipped item has a view");
    assert_ne!(ground_view, hand_view, "the view is a fresh entity");
    assert!(
        app.world().get_entity(ground_view).is_err(),
        "the old view is despawned"
    );
    assert_eq!(
        app.world().get::<ChildOf>(hand_view).map(ChildOf::parent),
        Some(holder),
        "the equipped view is parented to the holder"
    );

    // Drop it again: rust persists through the round trip.
    app.world_mut()
        .entity_mut(model)
        .insert(ItemState::OnGround(Vec3::new(1.0, 0.0, 0.0)));
    app.update();
    assert!(app.world().get::<Rusty>(model).is_some());
    assert!(view_entity(&app, model).is_some());

    // Stored items have no view at all, and the model still keeps everything.
    app.world_mut()
        .entity_mut(model)
        .insert(ItemState::StoredIn(holder));
    app.update();
    assert!(view_entity(&app, model).is_none());
    assert!(app.world().get::<Rusty>(model).is_some());
}

#[test]
fn unknown_key_leaves_model_intact() {
    let mut app = test_app();
    let model = app
        .world_mut()
        .spawn((
            Item {
                key: ItemKey("core::item::typo".to_string()),
                label: "Typo".to_string(),
            },
            ItemState::OnGround(Vec3::ZERO),
        ))
        .id();
    app.update();

    // No view, but the model keeps all its data — nothing was destroyed.
    assert!(view_entity(&app, model).is_none());
    assert!(app.world().get::<Item>(model).is_some());
    assert!(app.world().get::<ItemState>(model).is_some());
}

#[test]
fn despawning_the_model_despawns_the_view() {
    let mut app = test_app();
    let model = spawn_gun(&mut app, ItemState::OnGround(Vec3::ZERO));
    app.update();
    let view = view_entity(&app, model).expect("view exists");

    app.world_mut().entity_mut(model).despawn();
    app.update();
    assert!(
        app.world().get_entity(view).is_err(),
        "linked_spawn ties the view's lifetime to the model"
    );
}

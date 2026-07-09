use bevy::prelude::*;

use super::{Item, ItemRegistry, ItemState, Rusty};

/// Points from a view entity to the item model it renders.
#[derive(Component)]
#[relationship(relationship_target = View)]
pub struct ViewOf(pub Entity);

/// The model's link to its current view entity. The bare `Entity` field
/// makes the relationship one-to-one: linking a new view automatically
/// unlinks the previous one. `linked_spawn` despawns the view together with
/// the model.
#[derive(Component)]
#[relationship_target(relationship = ViewOf, linked_spawn)]
pub struct View(Entity);

impl View {
    pub fn entity(&self) -> Option<Entity> {
        (self.0 != Entity::PLACEHOLDER).then_some(self.0)
    }
}

/// Rebuild the view when an item's state changes.
pub fn view_on_state_change(
    insert: On<Insert, ItemState>,
    models: Query<EntityRef, With<Item>>,
    registry: Res<ItemRegistry>,
    commands: Commands,
) {
    refresh_view(insert.event().entity, &models, &registry, commands);
}

/// Rebuild the view when rust appears, so the appearance tracks the model.
/// Any other component that affects appearance gets the same one-liner hook.
pub fn view_on_rust(
    add: On<Add, Rusty>,
    models: Query<EntityRef, With<Item>>,
    registry: Res<ItemRegistry>,
    commands: Commands,
) {
    refresh_view(add.event().entity, &models, &registry, commands);
}

/// The core of the model/view split: the view is a pure function of the
/// model. Despawn the old view, ask the registry for a scene describing the
/// new one, spawn it, and relate it back to the model. The model entity is
/// never touched, so accumulated components survive by construction.
fn refresh_view(
    model: Entity,
    models: &Query<EntityRef, With<Item>>,
    registry: &ItemRegistry,
    mut commands: Commands,
) {
    let Ok(model_ref) = models.get(model) else {
        return;
    };

    commands.entity(model).despawn_related::<View>();

    let Some(scene) = registry.view_scene(model_ref) else {
        return;
    };
    let mut view = commands.spawn_scene(scene);
    view.insert(ViewOf(model));

    if let Some(&ItemState::EquippedBy(holder)) = model_ref.get::<ItemState>() {
        view.insert(ChildOf(holder));
    }
}

use bevy::prelude::*;

use super::{ContainedBy, Contains, Item, ItemRegistry, ItemState, ItemStateKind};

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
        // The one-to-one collection uses `Entity::PLACEHOLDER` as its empty
        // sentinel; don't leak it to callers.
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

/// The core of the model/view split: the view is a pure function of the
/// model. Despawn the old view, ask the registry for a scene describing the
/// new one, spawn it, and relate it back to the model. The model entity is
/// never touched, so accumulated components survive by construction.
///
/// Placement is per state: a grounded view is a child of the model itself
/// (the model's `Transform` positions it via propagation), an equipped view
/// is a child of the holder, a stored item has no view at all.
pub(crate) fn refresh_view(
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

    match model_ref.get::<ItemState>().map(ItemState::kind) {
        Some(ItemStateKind::OnGround) => {
            view.insert(ChildOf(model));
        }
        Some(ItemStateKind::Equipped) => {
            if let Some(contained) = model_ref.get::<ContainedBy>() {
                view.insert(ChildOf(contained.container()));
            } else {
                // Axis contradiction; the dev checker reports it. Parenting
                // to the model keeps the view visible at the item's last
                // position instead of floating at the origin.
                warn!("equipped item {model} has no holder; parenting view to the model");
                view.insert(ChildOf(model));
            }
        }
        Some(ItemStateKind::Stored) | None => {}
    }
}

/// When an entity holding items despawns, everything it carried — equipped
/// and stowed alike — drops at its death position. `Despawn` observers run
/// before the dying entity's components are stripped, so its `Transform`
/// and `Contains` list are still readable here. The link removal itself is
/// handled afterwards by the relationship hook; by the time it fires, these
/// items are already `OnGround`, so `repair_on_link_lost` stays silent.
pub fn ground_items_of_dying_holder(
    despawn: On<Despawn, Contains>,
    holders: Query<(&Transform, &Contains)>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    let Ok((transform, contains)) = holders.get(despawn.event().entity) else {
        return;
    };
    for held in contains.iter() {
        if items.get(held).is_err() {
            continue;
        }
        if let Ok(mut item) = commands.get_entity(held) {
            item.insert((
                super::on_ground_state(),
                Transform::from_translation(transform.translation),
            ));
        }
    }
}

/// Safety net for links lost outside the sanctioned paths (e.g. a raw
/// `remove::<ContainedBy>`). An item that is still `Equipped`/`Stored` with
/// no link falls back to the ground at its own `Transform` — the last place
/// it lay. `try_insert` is load-bearing: when the *model* is despawned
/// while contained, this observer fires with the entity dead by flush time,
/// and a plain insert would error.
pub fn repair_on_link_lost(
    removed: On<Remove, ContainedBy>,
    states: Query<&ItemState, With<Item>>,
    mut commands: Commands,
) {
    let model = removed.event().entity;
    let held = states
        .get(model)
        .is_ok_and(|state| state.is_equipped() || state.is_stored());
    if !held {
        return;
    }
    warn!("item {model} lost its container without a transition; re-grounding it in place");
    if let Ok(mut item) = commands.get_entity(model) {
        item.try_insert(super::on_ground_state());
    }
}

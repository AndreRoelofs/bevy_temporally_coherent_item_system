use bevy::prelude::*;

mod registry;
mod scenes;
mod view;

pub use registry::*;
pub use scenes::*;
pub use view::*;

/// Stable identifier for an item kind, e.g. `"core::item::gun"`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ItemKey(pub String);

/// Durable identity of an item. Lives on the model entity, which is never
/// rebuilt, so this — like every other model component — survives every
/// state transition.
#[derive(Component, Clone)]
pub struct Item {
    pub key: ItemKey,
    pub label: String,
}

/// Source of truth for where an item is. Immutable on purpose: the only way
/// to transition is to re-insert the component, which fires
/// `On<Insert, ItemState>` exactly once per transition.
#[derive(Component, Clone, Debug)]
#[component(immutable)]
pub enum ItemState {
    OnGround(Vec3),
    EquippedBy(Entity),
    StoredIn(Entity),
}

/// Marker for gun-specific systems.
#[derive(Component, Clone, Default)]
pub struct Gun;

/// Cumulative seconds this item has spent on the ground, across any number
/// of pickups and drops.
#[derive(Component, Clone, Default)]
pub struct GroundedSecs(pub f32);

/// Grows on items left on the ground too long. Persisting this across
/// pickup is the point of the whole design: it lives on the model entity,
/// so no view rebuild can lose it.
#[derive(Component, Clone, Default)]
pub struct Rusty;

/// Marker for anything an `ItemState` variant can point at — players,
/// chests, corpses. Holders must carry it so `ground_items_of_lost_holder`
/// can find stranded items when one dies.
#[derive(Component, Clone, Default)]
pub struct ItemHolder;

/// The dangling-reference guard. Without it, despawning a holder strands
/// every item it held: the equipped view dies with the holder (it is a
/// `ChildOf`), the model keeps `EquippedBy(dead)`, and no observer ever
/// re-fires — the item is invisible and unreachable forever.
///
/// `Despawn` fires before the holder's components are stripped, so its
/// `Transform` is still readable for the landing position. The re-insert is
/// an ordinary transition; the view rebuilds through the normal path.
pub fn ground_items_of_lost_holder(
    despawn: On<Despawn, ItemHolder>,
    holders: Query<&Transform>,
    items: Query<(Entity, &ItemState), With<Item>>,
    mut commands: Commands,
) {
    let dead = despawn.event().entity;
    let pos = holders
        .get(dead)
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);
    for (item_e, state) in &items {
        let stranded = match state {
            ItemState::EquippedBy(holder) | ItemState::StoredIn(holder) => *holder == dead,
            ItemState::OnGround(_) => false,
        };
        if stranded {
            commands.entity(item_e).insert(ItemState::OnGround(pos));
        }
    }
}

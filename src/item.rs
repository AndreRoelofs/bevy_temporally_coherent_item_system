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

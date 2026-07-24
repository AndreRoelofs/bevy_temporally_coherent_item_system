use bevy::prelude::*;

mod components;
mod inspect;
mod inventory;
mod registry;
mod state;
mod stats;
mod views;

pub use components::*;
pub use inspect::*;
pub use inventory::*;
pub use registry::*;
pub use state::*;
pub use stats::*;
pub use views::*;

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookTarget>()
            .init_resource::<InspectContributors>()
            .add_plugins((ItemComponentsPlugin, InventoryPlugin, ItemViewsPlugin))
            .add_observer(state::coherence_on_equip)
            .add_observer(state::coherence_on_store)
            .add_observer(state::coherence_on_ground)
            .add_observer(state::ground_items_of_dying_holder)
            .add_observer(state::repair_on_link_lost)
            .add_systems(Update, inspect::look_at_target);
        #[cfg(debug_assertions)]
        app.add_systems(
            Last,
            (
                check_item_invariants,
                stats::check_stat_source_leaks::<Cooldown>,
            ),
        );
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ItemKey(pub String);

#[derive(Component, Clone)]
#[require(ItemFootprint)]
pub struct Item {
    pub key: ItemKey,
    pub label: String,
}

fn axis_violation(on_ground: bool, equipped: bool, stored: bool) -> Option<&'static str> {
    match usize::from(on_ground) + usize::from(equipped) + usize::from(stored) {
        0 => Some("item has no state marker"),
        1 => None,
        _ => Some("item has more than one state marker"),
    }
}

#[cfg(debug_assertions)]
#[expect(clippy::type_complexity)]
fn check_item_invariants(
    items: Query<(Entity, Has<OnGround>, Has<EquippedBy>, Has<StoredIn>), With<Item>>,
) {
    for (entity, on_ground, equipped, stored) in &items {
        if let Some(violation) = axis_violation(on_ground, equipped, stored) {
            error!("item axis invariant broken on {entity}: {violation}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_violations_are_detected() {
        assert!(axis_violation(true, false, false).is_none());
        assert!(axis_violation(false, true, false).is_none());
        assert!(axis_violation(false, false, true).is_none());
        assert!(axis_violation(false, false, false).is_some());
        assert!(axis_violation(true, true, false).is_some());
        assert!(axis_violation(false, true, true).is_some());
        assert!(axis_violation(true, true, true).is_some());
    }
}

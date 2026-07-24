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
            .add_plugins((ItemComponentsPlugin, InventoryPlugin, ItemViewsPlugin));
        // The exclusion observers must be registered before the demotion
        // observer: a demoted item repacks against the other items' states,
        // which must be settled by then.
        register_item_state::<OnGround>(app);
        register_item_state::<EquippedBy>(app);
        register_item_state::<StoredIn>(app);
        app.add_observer(state::demote_other_equipped)
            .add_observer(state::ground_items_of_dying_holder)
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

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ItemLabel(pub String);

#[derive(Component, Clone)]
#[require(ItemFootprint)]
pub struct Item {
    pub key: ItemKey,
    pub label: ItemLabel,
}

fn axis_violation(marker_count: usize) -> Option<&'static str> {
    match marker_count {
        0 => Some("item has no state marker"),
        1 => None,
        _ => Some("item has more than one state marker"),
    }
}

#[cfg(debug_assertions)]
fn check_item_invariants(items: Query<EntityRef, With<Item>>, markers: Res<ItemStateMarkers>) {
    for model in &items {
        if let Some(violation) = axis_violation(markers.count_on(model)) {
            error!("item axis invariant broken on {}: {violation}", model.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_violations_are_detected() {
        assert!(axis_violation(1).is_none());
        assert!(axis_violation(0).is_some());
        assert!(axis_violation(2).is_some());
        assert!(axis_violation(3).is_some());
    }
}

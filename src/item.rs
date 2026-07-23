use bevy::prelude::*;

mod components;
mod inspect;
mod inventory;
mod registry;
mod stats;
mod views;

pub use components::*;
pub use inspect::*;
pub use inventory::*;
pub use registry::*;
pub use stats::*;
pub use views::*;

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookTarget>()
            .init_resource::<InspectContributors>()
            .add_plugins((ItemComponentsPlugin, InventoryPlugin, ItemViewsPlugin))
            .add_observer(ground_items_of_dying_holder)
            .add_observer(repair_on_link_lost)
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

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[component(immutable)]
pub enum ItemState {
    OnGround,
    Equipped,
    Stored,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnGround;

/// Used by moving entities to indicate who is carrying the item such as pawns and animals.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EquippedBy(pub Entity);

/// Used by stationary entities to indicate who is carrying the item such as chests and crates.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoredIn(pub Entity);

#[derive(Component)]
#[relationship(relationship_target = Contains)]
pub struct ContainedBy(Entity);

impl ContainedBy {
    pub fn container(&self) -> Entity {
        self.0
    }
}

#[derive(Component)]
#[relationship_target(relationship = ContainedBy)]
pub struct Contains(Vec<Entity>);

impl Contains {
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub trait ItemTransitions {
    fn equip_to(&mut self, holder: Entity) -> &mut Self;

    fn store_in(&mut self, container: Entity) -> &mut Self;

    fn drop_at(&mut self, pos: Vec3) -> &mut Self;
}

impl ItemTransitions for EntityCommands<'_> {
    fn equip_to(&mut self, holder: Entity) -> &mut Self {
        self.queue(move |mut item: EntityWorldMut| {
            let model = item.id();
            let demote: Vec<Entity> = item.world_scope(|world| {
                let Ok(holder_ref) = world.get_entity(holder) else {
                    warn!("equip_to: holder {holder} does not exist");
                    return Vec::new();
                };
                holder_ref
                    .get::<Contains>()
                    .map(|contains| {
                        contains
                            .iter()
                            .filter(|&held| {
                                held != model
                                    && world
                                        .get::<ItemState>(held)
                                        .is_some_and(|state| state == &ItemState::Equipped)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            });
            if item.world().get_entity(holder).is_err() {
                return;
            }
            item.insert((ItemState::Equipped, ContainedBy(holder)));
            item.world_scope(|world| {
                for held in demote {
                    world.entity_mut(held).insert(ItemState::Stored);
                }
            });
        });
        self
    }

    fn store_in(&mut self, container: Entity) -> &mut Self {
        self.queue(move |mut item: EntityWorldMut| {
            if item.world().get_entity(container).is_err() {
                warn!("store_in: container {container} does not exist");
                return;
            }
            item.insert((ItemState::Stored, ContainedBy(container)));
        });
        self
    }

    fn drop_at(&mut self, pos: Vec3) -> &mut Self {
        self.insert((ItemState::OnGround, Transform::from_translation(pos)))
            .remove::<ContainedBy>()
    }
}

fn ground_items_of_dying_holder(
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
                ItemState::OnGround,
                Transform::from_translation(transform.translation),
            ));
        }
    }
}

fn repair_on_link_lost(
    removed: On<Remove, ContainedBy>,
    states: Query<&ItemState, With<Item>>,
    mut commands: Commands,
) {
    let model = removed.event().entity;
    let held = states
        .get(model)
        .is_ok_and(|state| state == &ItemState::Equipped || state == &ItemState::Stored);
    if !held {
        return;
    }
    warn!("item {model} lost its container without a transition; re-grounding it in place");
    if let Ok(mut item) = commands.get_entity(model) {
        item.try_insert(ItemState::OnGround);
    }
}

fn axis_violation(state: &ItemState, contained: Option<&ContainedBy>) -> Option<&'static str> {
    match (state, contained) {
        (ItemState::OnGround, Some(_)) => Some("OnGround item still has a ContainedBy link"),
        (ItemState::Equipped | ItemState::Stored, None) => {
            Some("held item has no ContainedBy link")
        }
        _ => None,
    }
}

#[cfg(debug_assertions)]
fn check_item_invariants(items: Query<(Entity, &ItemState, Option<&ContainedBy>), With<Item>>) {
    for (entity, state, contained) in &items {
        if let Some(violation) = axis_violation(state, contained) {
            error!("item axis invariant broken on {entity}: {violation} (state: {state:?})");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO: Rewrite this test
    #[test]
    fn axis_violations_are_detected() {
        let holder = Entity::from_raw_u32(1).unwrap();
        assert!(axis_violation(&ItemState::OnGround, None).is_none());
        assert!(axis_violation(&ItemState::OnGround, Some(&ContainedBy(holder))).is_some());
        assert!(axis_violation(&ItemState::Equipped, Some(&ContainedBy(holder))).is_none());
        assert!(axis_violation(&ItemState::Equipped, None).is_some());
        assert!(axis_violation(&ItemState::Stored, None).is_some());
    }
}

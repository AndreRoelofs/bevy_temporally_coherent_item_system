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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemStateKind {
    OnGround,
    Equipped,
    Stored,
}

#[derive(Component, Debug)]
#[component(immutable)]
pub struct ItemState(ItemStateKind);

impl ItemState {
    pub fn kind(&self) -> ItemStateKind {
        self.0
    }

    pub fn is_on_ground(&self) -> bool {
        self.0 == ItemStateKind::OnGround
    }

    pub fn is_equipped(&self) -> bool {
        self.0 == ItemStateKind::Equipped
    }

    pub fn is_stored(&self) -> bool {
        self.0 == ItemStateKind::Stored
    }
}

impl PartialEq<ItemStateKind> for ItemState {
    fn eq(&self, kind: &ItemStateKind) -> bool {
        self.0 == *kind
    }
}

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
                                        .is_some_and(ItemState::is_equipped)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            });
            if item.world().get_entity(holder).is_err() {
                return;
            }
            item.insert((ItemState(ItemStateKind::Equipped), ContainedBy(holder)));
            item.world_scope(|world| {
                for held in demote {
                    world
                        .entity_mut(held)
                        .insert(ItemState(ItemStateKind::Stored));
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
            item.insert((ItemState(ItemStateKind::Stored), ContainedBy(container)));
        });
        self
    }

    fn drop_at(&mut self, pos: Vec3) -> &mut Self {
        self.insert((
            ItemState(ItemStateKind::OnGround),
            Transform::from_translation(pos),
        ))
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
                ItemState(ItemStateKind::OnGround),
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
        .is_ok_and(|state| state.is_equipped() || state.is_stored());
    if !held {
        return;
    }
    warn!("item {model} lost its container without a transition; re-grounding it in place");
    if let Ok(mut item) = commands.get_entity(model) {
        item.try_insert(ItemState(ItemStateKind::OnGround));
    }
}

fn axis_violation(state: &ItemState, contained: Option<&ContainedBy>) -> Option<&'static str> {
    match (state.kind(), contained) {
        (ItemStateKind::OnGround, Some(_)) => Some("OnGround item still has a ContainedBy link"),
        (ItemStateKind::Equipped | ItemStateKind::Stored, None) => {
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

    #[test]
    fn axis_violations_are_detected() {
        let holder = Entity::from_raw_u32(1).unwrap();
        assert!(axis_violation(&ItemState(ItemStateKind::OnGround), None).is_none());
        assert!(
            axis_violation(
                &ItemState(ItemStateKind::OnGround),
                Some(&ContainedBy(holder))
            )
            .is_some()
        );
        assert!(
            axis_violation(
                &ItemState(ItemStateKind::Equipped),
                Some(&ContainedBy(holder))
            )
            .is_none()
        );
        assert!(axis_violation(&ItemState(ItemStateKind::Equipped), None).is_some());
        assert!(axis_violation(&ItemState(ItemStateKind::Stored), None).is_some());
    }
}

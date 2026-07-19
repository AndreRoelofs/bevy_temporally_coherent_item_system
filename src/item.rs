use bevy::prelude::*;

mod components;
mod registry;
mod scenes;
mod view;

pub use components::*;
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

/// The kind axis of an item's location, and nothing else: the holder or
/// container lives in [`ContainedBy`], the position in the model's
/// [`Transform`]. Exhaustively matchable via [`ItemState::kind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemStateKind {
    OnGround,
    Equipped,
    Stored,
}

/// Sealed wrapper for the state axis. Readable everywhere, constructible
/// only inside this module — the [`ItemTransitions`] trait is the only way
/// to change it. Immutable, so re-insertion is the only write, firing
/// `On<Insert, ItemState>` exactly once per transition. Deliberately not
/// `Clone`: cloning a state off one entity and inserting it on another
/// would be a transition that bypassed the API.
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

/// The reference axis: which entity holds or contains this item. The state
/// axis says *how* (`Equipped` in hand vs `Stored` in the bag/chest), so one
/// relationship serves both — which is what lets a character equip a sword
/// and carry a gun at the same time. The private field seals construction:
/// linking an item to a holder is only possible through [`ItemTransitions`].
#[derive(Component)]
#[relationship(relationship_target = Contains)]
pub struct ContainedBy(Entity);

impl ContainedBy {
    pub fn container(&self) -> Entity {
        self.0
    }
}

/// Everything an entity holds or contains, equipped and stowed alike.
/// Deliberately NOT `linked_spawn`: a dying holder must strip the link, not
/// despawn the persistent models.
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

/// The only sanctioned way to change an item's state or reference axis.
/// Each method keeps the two axes coherent; the load-bearing orderings live
/// here, once, instead of at every call site.
pub trait ItemTransitions {
    /// Equip onto `holder`'s hand. Policy lives here too: anything the
    /// holder already has equipped is demoted to `Stored` first (the old
    /// weapon slides into the bag). No-op with a warning if `holder` is
    /// dead.
    fn equip_to(&mut self, holder: Entity) -> &mut Self;

    /// Stow inside `container` (a bag, a chest — any entity). No-op with a
    /// warning if `container` is dead.
    fn store_in(&mut self, container: Entity) -> &mut Self;

    /// Put the item on the ground at `pos`. Also how an item first enters
    /// the world: spawn the model, then drop it.
    fn drop_at(&mut self, pos: Vec3) -> &mut Self;
}

impl ItemTransitions for EntityCommands<'_> {
    fn equip_to(&mut self, holder: Entity) -> &mut Self {
        self.queue(move |mut item: EntityWorldMut| {
            // Demote whatever the holder currently has equipped. Needs world
            // access, which is also what lets us validate the holder.
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
                                world
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
            item.world_scope(|world| {
                for held in demote {
                    world
                        .entity_mut(held)
                        .insert(ItemState(ItemStateKind::Stored));
                }
            });
            item.insert((ItemState(ItemStateKind::Equipped), ContainedBy(holder)));
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
        // Order is load-bearing: state first, so the `Remove` fired by the
        // link removal finds `OnGround` and the repair observer stays
        // silent. Reversed, the repair would re-ground mid-transition and
        // the view would rebuild twice.
        self.insert((
            ItemState(ItemStateKind::OnGround),
            Transform::from_translation(pos),
        ))
        .remove::<ContainedBy>()
    }
}

/// In-module constructor for observers that must transition items without
/// going through commands-on-self (e.g. re-grounding a dying holder's
/// inventory). Not exported: outside this module the trait is the only door.
pub(crate) fn on_ground_state() -> ItemState {
    ItemState(ItemStateKind::OnGround)
}

/// The axes are coherent iff: `Equipped`/`Stored` have a link, `OnGround`
/// does not. Pure so it can be unit-tested; the system below reports
/// violations in dev builds.
fn axis_violation(state: &ItemState, contained: Option<&ContainedBy>) -> Option<&'static str> {
    match (state.kind(), contained) {
        (ItemStateKind::OnGround, Some(_)) => Some("OnGround item still has a ContainedBy link"),
        (ItemStateKind::Equipped | ItemStateKind::Stored, None) => {
            Some("held item has no ContainedBy link")
        }
        _ => None,
    }
}

/// Dev-build watchdog for in-module mistakes; code outside this module
/// cannot create contradictions because both axes are sealed.
#[cfg(debug_assertions)]
pub fn check_item_invariants(items: Query<(Entity, &ItemState, Option<&ContainedBy>), With<Item>>) {
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

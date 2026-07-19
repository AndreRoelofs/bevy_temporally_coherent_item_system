use bevy::prelude::*;

mod components;
mod inspect;
mod registry;
mod stats;
mod views;

pub use components::*;
pub use inspect::*;
pub use registry::*;
pub use stats::*;
pub use views::*;

/// Wires the whole item system: the model-side component behaviors, the
/// view side, and the lifecycle observers that keep the axes coherent.
pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        // Inspection resources exist before the hubs build, so contributor
        // plugins can register lines at build time.
        app.init_resource::<LookTarget>()
            .init_resource::<InspectContributors>()
            .add_plugins((ItemComponentsPlugin, ItemViewsPlugin))
            .add_observer(ground_items_of_dying_holder)
            .add_observer(repair_on_link_lost)
            .add_systems(Update, inspect::look_at_target);
        #[cfg(debug_assertions)]
        app.add_systems(Last, check_item_invariants);
    }
}

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// When an entity holding items despawns, everything it carried — equipped
/// and stowed alike — drops at its death position. `Despawn` observers run
/// before the dying entity's components are stripped, so its `Transform`
/// and `Contains` list are still readable here. The link removal itself is
/// handled afterwards by the relationship hook; by the time it fires, these
/// items are already `OnGround`, so `repair_on_link_lost` stays silent.
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

/// Safety net for links lost outside the sanctioned paths (e.g. a raw
/// `remove::<ContainedBy>`). An item that is still `Equipped`/`Stored` with
/// no link falls back to the ground at its own `Transform` — the last place
/// it lay. `try_insert` is load-bearing: when the *model* is despawned
/// while contained, this observer fires with the entity dead by flush time,
/// and a plain insert would error.
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

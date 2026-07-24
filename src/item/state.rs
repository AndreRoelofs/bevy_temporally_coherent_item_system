use bevy::prelude::*;

use super::Item;

/// The three mutually exclusive placements of an item, derived from whichever
/// marker component the item carries. Kept as a plain value for registry keys
/// and inspection; the markers below are the source of truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemState {
    OnGround,
    Equipped,
    Stored,
}

impl ItemState {
    pub fn of(model: EntityRef) -> Option<Self> {
        if model.contains::<OnGround>() {
            Some(Self::OnGround)
        } else if model.contains::<EquippedBy>() {
            Some(Self::Equipped)
        } else if model.contains::<StoredIn>() {
            Some(Self::Stored)
        } else {
            None
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnGround;

/// Used by moving entities to indicate who is carrying the item such as pawns and animals.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[relationship(relationship_target = Equips)]
pub struct EquippedBy(pub Entity);

impl EquippedBy {
    pub fn holder(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Debug)]
#[relationship_target(relationship = EquippedBy)]
pub struct Equips(Vec<Entity>);

impl Equips {
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

/// Used by stationary entities to indicate who is storing the item such as chests and crates.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[relationship(relationship_target = Stores)]
pub struct StoredIn(pub Entity);

impl StoredIn {
    pub fn container(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Debug)]
#[relationship_target(relationship = StoredIn)]
pub struct Stores(Vec<Entity>);

impl Stores {
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

/// Sanctioned state transitions. Each inserts only the new marker; the
/// coherence observers clear the old one immediately afterwards, so the
/// item never has zero markers and the moment with two never outlives the
/// insert. Removing the old marker first is not an option: the repair net
/// would see the between-markers gap and re-ground the item mid-transition.
pub trait ItemTransitions {
    fn equip_to(&mut self, holder: Entity) -> &mut Self;

    fn store_in(&mut self, container: Entity) -> &mut Self;

    fn drop_at(&mut self, pos: Vec3) -> &mut Self;
}

impl ItemTransitions for EntityCommands<'_> {
    fn equip_to(&mut self, holder: Entity) -> &mut Self {
        self.queue(move |mut item: EntityWorldMut| {
            if item.world().get_entity(holder).is_err() {
                warn!("equip_to: holder {holder} does not exist");
                return;
            }
            item.insert(EquippedBy(holder));
        });
        self
    }

    fn store_in(&mut self, container: Entity) -> &mut Self {
        self.queue(move |mut item: EntityWorldMut| {
            if item.world().get_entity(container).is_err() {
                warn!("store_in: container {container} does not exist");
                return;
            }
            item.insert(StoredIn(container));
        });
        self
    }

    fn drop_at(&mut self, pos: Vec3) -> &mut Self {
        self.insert((OnGround, Transform::from_translation(pos)))
    }
}

/// Removes `Stale` markers, but only if the `Kept` marker that queued this
/// cleanup is still the item's state by the time the command runs; a later
/// transition in the same batch must not have its fresh marker stripped.
fn clear_conflicting<Kept: Component, Stale: Bundle>(world: &mut World, model: Entity) {
    let Ok(mut model_mut) = world.get_entity_mut(model) else {
        return;
    };
    if model_mut.contains::<Kept>() {
        model_mut.remove::<Stale>();
    }
}

/// Equipping wins over whatever the item was before: stale markers are
/// cleared and any other item the holder had equipped is demoted into
/// storage.
pub(crate) fn coherence_on_equip(
    insert: On<Insert, EquippedBy>,
    equipped: Query<&EquippedBy>,
    holders: Query<&Equips>,
    mut commands: Commands,
) {
    let model = insert.event().entity;
    commands.queue(move |world: &mut World| {
        clear_conflicting::<EquippedBy, (StoredIn, OnGround)>(world, model);
    });
    let Ok(equipped_by) = equipped.get(model) else {
        return;
    };
    let holder = equipped_by.holder();
    let Ok(equips) = holders.get(holder) else {
        return;
    };
    for held in equips.iter().filter(|&held| held != model) {
        commands.entity(held).store_in(holder);
    }
}

pub(crate) fn coherence_on_store(insert: On<Insert, StoredIn>, mut commands: Commands) {
    let model = insert.event().entity;
    commands.queue(move |world: &mut World| {
        clear_conflicting::<StoredIn, (EquippedBy, OnGround)>(world, model);
    });
}

pub(crate) fn coherence_on_ground(insert: On<Insert, OnGround>, mut commands: Commands) {
    let model = insert.event().entity;
    commands.queue(move |world: &mut World| {
        clear_conflicting::<OnGround, (EquippedBy, StoredIn)>(world, model);
    });
}

/// An item whose holder link vanished without a transition putting it
/// somewhere else is re-grounded in place. The check runs deferred so a
/// replacement marker inserted later in the same batch counts as a
/// transition.
pub(crate) fn repair_on_link_lost(
    removed: On<Remove, (EquippedBy, StoredIn)>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    let model = removed.event().entity;
    if items.get(model).is_err() {
        return;
    }
    commands.queue(move |world: &mut World| {
        let Ok(model_ref) = world.get_entity(model) else {
            return;
        };
        if ItemState::of(model_ref).is_some() {
            return;
        }
        warn!("item {model} lost its container without a transition; re-grounding it in place");
        world.entity_mut(model).insert(OnGround);
    });
}

pub(crate) fn ground_items_of_dying_holder(
    despawn: On<Despawn, (Equips, Stores)>,
    holders: Query<(&Transform, Option<&Equips>, Option<&Stores>)>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    let Ok((transform, equips, stores)) = holders.get(despawn.event().entity) else {
        return;
    };
    let equipped = equips.into_iter().flat_map(|equips| equips.iter());
    let stored = stores.into_iter().flat_map(|stores| stores.iter());
    for held in equipped.chain(stored) {
        if items.get(held).is_err() {
            continue;
        }
        if let Ok(mut item) = commands.get_entity(held) {
            item.drop_at(transform.translation);
        }
    }
}

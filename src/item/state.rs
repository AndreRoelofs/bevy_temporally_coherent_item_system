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

/// Sanctioned state transitions — the only supported door for moving an
/// item between states. Each swaps the old marker for the new one in a
/// single command, and `equip_to` demotes whatever the holder already had
/// equipped into storage. Touching the markers directly, whether inserting
/// or removing, is a contract violation: it leaves the item with zero or
/// two markers, which the debug invariant check reports.
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
            let model = item.id();
            item.remove::<(StoredIn, OnGround)>();
            item.insert(EquippedBy(holder));
            item.world_scope(|world| {
                let demote: Vec<Entity> = world
                    .get::<Equips>(holder)
                    .map(|equips| equips.iter().filter(|&held| held != model).collect())
                    .unwrap_or_default();
                for held in demote {
                    world.commands().entity(held).store_in(holder);
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
            item.remove::<(EquippedBy, OnGround)>();
            item.insert(StoredIn(container));
        });
        self
    }

    fn drop_at(&mut self, pos: Vec3) -> &mut Self {
        self.remove::<(EquippedBy, StoredIn)>()
            .insert((OnGround, Transform::from_translation(pos)))
    }
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

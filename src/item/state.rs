use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use super::{ContainedBy, Contains, ItemState};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnGround;

/// Used by moving entities to indicate who is carrying the item such as pawns and animals.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EquippedBy(pub Entity);

/// Used by stationary entities to indicate who is carrying the item such as chests and crates.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoredIn(pub Entity);

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

pub fn sync_state_markers(mut world: DeferredWorld, context: HookContext) {
    let Some(&state) = world.get::<ItemState>(context.entity) else {
        return;
    };
    let mut commands = world.commands();
    let mut entity = commands.entity(context.entity);
    entity.remove::<(OnGround, EquippedBy, StoredIn)>();
    match state {
        ItemState::OnGround => entity.insert(OnGround),
        ItemState::Equipped => entity.insert(EquippedBy(context.entity)),
        ItemState::Stored => entity.insert(StoredIn(context.entity)),
    };
}

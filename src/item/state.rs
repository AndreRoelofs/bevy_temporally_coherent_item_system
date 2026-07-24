use std::{borrow::Borrow, fmt};

use bevy::{ecs::component::ComponentId, prelude::*};

use super::Item;

/// A state marker's name — its own string namespace, so a state key can
/// never be confused with an `ItemKey` or any other string.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StateKey(pub &'static str);

impl StateKey {
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for StateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Borrow<str> for StateKey {
    fn borrow(&self) -> &str {
        self.0
    }
}

/// Names a state marker for the chrome registry and inspection. Keys are
/// namespaced like `ItemKey` ("core::item_state::equipped") so third-party
/// states cannot collide with core's or each other's.
pub trait ItemStateMarker: Component {
    const KEY: StateKey;
}

/// The open set of state markers. Changing an item's state is a plain
/// insert of the new marker - the exclusion observer registered alongside
/// each entry clears the previous one. Removing a marker directly is a
/// contract violation: it leaves the item in no state at all, which the
/// debug invariant check reports.
#[derive(Resource, Default)]
pub struct ItemStateMarkers(Vec<(ComponentId, StateKey)>);

impl ItemStateMarkers {
    pub fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.0.iter().map(|&(id, _)| id)
    }

    fn id_of(&self, key: StateKey) -> Option<ComponentId> {
        self.0
            .iter()
            .find(|&&(_, registered)| registered == key)
            .map(|&(id, _)| id)
    }

    /// The key of the state marker the item currently carries.
    pub fn key_of(&self, model: EntityRef) -> Option<StateKey> {
        self.0
            .iter()
            .find(|&&(id, _)| model.contains_id(id))
            .map(|&(_, key)| key)
    }

    pub fn count_on(&self, model: EntityRef) -> usize {
        self.0
            .iter()
            .filter(|&&(id, _)| model.contains_id(id))
            .count()
    }
}

pub fn register_item_state<S: ItemStateMarker>(app: &mut App) {
    app.init_resource::<ItemStateMarkers>();
    let id = app.world_mut().register_component::<S>();
    let mut markers = app.world_mut().resource_mut::<ItemStateMarkers>();
    if markers.ids().any(|registered| registered == id) {
        warn!("item state `{}` is already registered", S::KEY);
        return;
    }
    markers.0.push((id, S::KEY));
    app.add_observer(exclude_others::<S>);
}

fn exclude_others<S: ItemStateMarker>(
    insert: On<Insert, S>,
    markers: Res<ItemStateMarkers>,
    models: Query<EntityRef>,
    mut commands: Commands,
) {
    let model = insert.event().entity;
    let Ok(model_ref) = models.get(model) else {
        return;
    };
    let own = markers.id_of(S::KEY);
    if !markers
        .ids()
        .any(|id| Some(id) != own && model_ref.contains_id(id))
    {
        return;
    }
    commands.queue(move |world: &mut World| {
        let Some(own) = world.component_id::<S>() else {
            return;
        };
        let Ok(model_ref) = world.get_entity(model) else {
            return;
        };
        if !model_ref.contains::<S>() {
            return;
        }
        let stale: Vec<ComponentId> = world
            .resource::<ItemStateMarkers>()
            .ids()
            .filter(|&id| id != own && model_ref.contains_id(id))
            .collect();
        if !stale.is_empty() {
            world.entity_mut(model).remove_by_ids(&stale);
        }
    });
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnGround;

impl ItemStateMarker for OnGround {
    const KEY: StateKey = StateKey("core::item_state::on_ground");
}

/// Used by moving entities to indicate who is carrying the item such as pawns and animals.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[relationship(relationship_target = Equips)]
pub struct EquippedBy(pub Entity);

impl EquippedBy {
    pub fn holder(&self) -> Entity {
        self.0
    }
}

impl ItemStateMarker for EquippedBy {
    const KEY: StateKey = StateKey("core::item_state::equipped");
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

impl ItemStateMarker for StoredIn {
    const KEY: StateKey = StateKey("core::item_state::stored");
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

/// The grounded-state bundle: `OnGround` plus where the item lies.
pub fn on_ground_at(pos: Vec3) -> impl Bundle {
    (OnGround, Transform::from_translation(pos))
}

/// A holder equips one item at a time: whatever it already had equipped is
/// demoted into its storage.
pub(crate) fn demote_other_equipped(
    insert: On<Insert, EquippedBy>,
    equipped: Query<&EquippedBy>,
    holders: Query<&Equips>,
    mut commands: Commands,
) {
    let model = insert.event().entity;
    let Ok(equipped_by) = equipped.get(model) else {
        return;
    };
    let holder = equipped_by.holder();
    let Ok(equips) = holders.get(holder) else {
        return;
    };
    for held in equips.iter().filter(|&held| held != model) {
        commands.entity(held).insert(StoredIn(holder));
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
            item.insert(on_ground_at(transform.translation));
        }
    }
}

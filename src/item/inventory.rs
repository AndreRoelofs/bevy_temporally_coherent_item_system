use bevy::prelude::*;

use crate::{Item, StoredIn, Stores, on_ground_at};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemFootprint(pub UVec2);

impl Default for ItemFootprint {
    fn default() -> Self {
        Self(UVec2::ONE)
    }
}

#[derive(Component, Debug)]
pub struct InventoryGrid(UVec2);

impl InventoryGrid {
    pub fn new(size: UVec2) -> Self {
        assert!(
            size.cmpgt(UVec2::ZERO).all(),
            "inventory grids need at least one cell"
        );
        Self(size)
    }

    pub fn size(&self) -> UVec2 {
        self.0
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[component(immutable)]
pub struct PackedAt(UVec2);

impl PackedAt {
    pub fn origin(&self) -> UVec2 {
        self.0
    }
}

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(pack_on_store);
        #[cfg(debug_assertions)]
        app.add_systems(Last, check_packing_invariants);
    }
}

fn rects_overlap(a_origin: UVec2, a_size: UVec2, b_origin: UVec2, b_size: UVec2) -> bool {
    a_origin.x < b_origin.x + b_size.x
        && b_origin.x < a_origin.x + a_size.x
        && a_origin.y < b_origin.y + b_size.y
        && b_origin.y < a_origin.y + a_size.y
}

pub fn fits_at(grid: UVec2, occupied: &[(UVec2, UVec2)], footprint: UVec2, origin: UVec2) -> bool {
    origin.x + footprint.x <= grid.x
        && origin.y + footprint.y <= grid.y
        && !occupied.iter().any(|&(other_origin, other_size)| {
            rects_overlap(origin, footprint, other_origin, other_size)
        })
}

pub fn find_free_slot(
    grid: UVec2,
    occupied: &[(UVec2, UVec2)],
    footprint: UVec2,
    preferred: Option<UVec2>,
) -> Option<UVec2> {
    if footprint.cmpeq(UVec2::ZERO).any() || footprint.cmpgt(grid).any() {
        return None;
    }
    if let Some(origin) = preferred
        && fits_at(grid, occupied, footprint, origin)
    {
        return Some(origin);
    }
    for y in 0..=(grid.y - footprint.y) {
        for x in 0..=(grid.x - footprint.x) {
            let origin = UVec2::new(x, y);
            if fits_at(grid, occupied, footprint, origin) {
                return Some(origin);
            }
        }
    }
    None
}

pub fn commit_drag(
    grid: UVec2,
    occupied: &[(UVec2, UVec2)],
    footprint: UVec2,
    start: UVec2,
    drag_px: Vec2,
    cell_px: f32,
) -> Option<UVec2> {
    if footprint.cmpeq(UVec2::ZERO).any() || footprint.cmpgt(grid).any() {
        return None;
    }
    let max_origin = (grid - footprint).as_ivec2();
    let target = (start.as_vec2() + drag_px / cell_px)
        .round()
        .as_ivec2()
        .clamp(IVec2::ZERO, max_origin)
        .as_uvec2();
    fits_at(grid, occupied, footprint, target).then_some(target)
}

pub(crate) fn occupied_cells(
    container: Entity,
    except: Entity,
    containers: &Query<(&InventoryGrid, &Stores)>,
    stored: &Query<(&PackedAt, &ItemFootprint), With<Item>>,
) -> Option<(UVec2, Vec<(UVec2, UVec2)>)> {
    let (grid, stores) = containers.get(container).ok()?;
    let occupied = stores
        .iter()
        .filter(|&held| held != except)
        .filter_map(|held| stored.get(held).ok())
        .map(|(packed, footprint)| (packed.origin(), footprint.0))
        .collect();
    Some((grid.size(), occupied))
}

fn occupied_cells_world(
    world: &mut World,
    container: Entity,
    except: Entity,
) -> Vec<(UVec2, UVec2)> {
    world
        .query_filtered::<(Entity, &StoredIn, &PackedAt, &ItemFootprint), With<Item>>()
        .iter(world)
        .filter(|&(held, stored_in, ..)| held != except && stored_in.container() == container)
        .map(|(_, _, packed, footprint)| (packed.origin(), footprint.0))
        .collect()
}

fn pack_on_store(
    insert: On<Insert, StoredIn>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    let model = insert.event().entity;
    if items.get(model).is_err() {
        return;
    }
    commands.queue(move |world: &mut World| pack_into_grid(world, model));
}

fn pack_into_grid(world: &mut World, model: Entity) {
    let Ok(model_ref) = world.get_entity(model) else {
        return;
    };
    let Some(container) = model_ref.get::<StoredIn>().map(StoredIn::container) else {
        return;
    };
    let footprint = model_ref
        .get::<ItemFootprint>()
        .copied()
        .unwrap_or_default()
        .0;
    let packed = model_ref.get::<PackedAt>().map(PackedAt::origin);
    let Some(grid) = world
        .get::<InventoryGrid>(container)
        .map(InventoryGrid::size)
    else {
        return;
    };
    let occupied = occupied_cells_world(world, container, model);
    match find_free_slot(grid, &occupied, footprint, packed) {
        Some(origin) => {
            if packed != Some(origin) {
                world.entity_mut(model).insert(PackedAt(origin));
            }
        }
        None => {
            warn!("no room in container {container} for item {model}; re-grounding it");
            let pos = world
                .get::<Transform>(container)
                .map(|t| t.translation)
                .unwrap_or_default();
            world.commands().entity(model).insert(on_ground_at(pos));
        }
    }
}

pub trait ItemPacking {
    fn repack_at(&mut self, origin: UVec2) -> &mut Self;
}

impl ItemPacking for EntityCommands<'_> {
    fn repack_at(&mut self, origin: UVec2) -> &mut Self {
        self.queue(move |mut item: EntityWorldMut| {
            let model = item.id();
            let Some(container) = item.get::<StoredIn>().map(StoredIn::container) else {
                warn!("repack_at: item {model} is not stored");
                return;
            };
            let footprint = item.get::<ItemFootprint>().copied().unwrap_or_default().0;
            let fits = item.world_scope(|world| {
                let Some(grid) = world
                    .get::<InventoryGrid>(container)
                    .map(InventoryGrid::size)
                else {
                    warn!("repack_at: container {container} has no inventory grid");
                    return false;
                };
                let occupied = occupied_cells_world(world, container, model);
                fits_at(grid, &occupied, footprint, origin)
            });
            if fits {
                item.insert(PackedAt(origin));
            } else {
                warn!("repack_at: item {model} does not fit at {origin} in {container}");
            }
        });
        self
    }
}

#[cfg(debug_assertions)]
fn check_packing_invariants(
    containers: Query<(Entity, &InventoryGrid, &Stores)>,
    items: Query<(Option<&PackedAt>, &ItemFootprint), With<Item>>,
) {
    for (container, grid, stores) in &containers {
        let mut seen: Vec<(Entity, UVec2, UVec2)> = Vec::new();
        for held in stores.iter() {
            let Ok((packed, footprint)) = items.get(held) else {
                continue;
            };
            let Some(packed) = packed else {
                error!("stored item {held} in gridded container {container} has no PackedAt");
                continue;
            };
            let origin = packed.origin();
            if !fits_at(grid.size(), &[], footprint.0, origin) {
                error!("item {held} sticks out of container {container}'s grid at {origin}");
            }
            for &(other, other_origin, other_size) in &seen {
                if rects_overlap(origin, footprint.0, other_origin, other_size) {
                    error!("items {held} and {other} overlap in container {container}");
                }
            }
            seen.push((held, origin, footprint.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID: UVec2 = UVec2::new(12, 8);

    #[test]
    fn fits_at_respects_bounds_and_overlap() {
        assert!(fits_at(GRID, &[], UVec2::new(8, 4), UVec2::new(4, 4)));
        assert!(
            !fits_at(GRID, &[], UVec2::new(8, 4), UVec2::new(5, 4)),
            "one column past the right edge"
        );
        let occupied = [(UVec2::new(0, 0), UVec2::new(4, 4))];
        assert!(!fits_at(
            GRID,
            &occupied,
            UVec2::new(4, 4),
            UVec2::new(3, 3)
        ));
        assert!(
            fits_at(GRID, &occupied, UVec2::new(4, 4), UVec2::new(4, 0)),
            "rects are half-open; touching edges is not overlap"
        );
    }

    #[test]
    fn find_free_slot_is_first_fit_row_major() {
        let occupied = [(UVec2::new(0, 0), UVec2::new(8, 4))];
        assert_eq!(
            find_free_slot(GRID, &occupied, UVec2::new(4, 4), None),
            Some(UVec2::new(8, 0)),
            "the first free spot on the top row wins"
        );
        assert_eq!(find_free_slot(GRID, &[], UVec2::new(13, 1), None), None);
        assert_eq!(find_free_slot(GRID, &[], UVec2::ZERO, None), None);
    }

    #[test]
    fn find_free_slot_prefers_the_remembered_spot() {
        assert_eq!(
            find_free_slot(GRID, &[], UVec2::new(4, 4), Some(UVec2::new(5, 3))),
            Some(UVec2::new(5, 3))
        );
        let occupied = [(UVec2::new(5, 3), UVec2::ONE)];
        assert_eq!(
            find_free_slot(GRID, &occupied, UVec2::new(4, 4), Some(UVec2::new(5, 3))),
            Some(UVec2::ZERO),
            "a taken spot falls back to first fit"
        );
    }

    #[test]
    fn commit_drag_snaps_to_the_nearest_cell() {
        let cell = 48.0;
        let footprint = UVec2::new(4, 4);
        assert_eq!(
            commit_drag(
                GRID,
                &[],
                footprint,
                UVec2::ZERO,
                Vec2::new(100.0, 20.0),
                cell
            ),
            Some(UVec2::new(2, 0)),
            "100px right of the origin rounds to column 2"
        );
        assert_eq!(
            commit_drag(
                GRID,
                &[],
                footprint,
                UVec2::ZERO,
                Vec2::new(-500.0, 9999.0),
                cell
            ),
            Some(UVec2::new(0, 4)),
            "out-of-bounds drags clamp back onto the grid"
        );
        let occupied = [(UVec2::new(0, 4), footprint)];
        assert_eq!(
            commit_drag(
                GRID,
                &occupied,
                footprint,
                UVec2::ZERO,
                Vec2::new(0.0, 9999.0),
                cell
            ),
            None,
            "an occupied target rejects the drop"
        );
    }
}

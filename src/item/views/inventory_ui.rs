use bevy::picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use super::{View, ViewOf};
use crate::{
    ContainedBy, Contains, InventoryGrid, Item, ItemFootprint, ItemPacking, ItemState, PackedAt,
    commit_drag, occupied_cells,
};

pub const CELL_PX: f32 = 48.0;

const PANEL_BORDER_PX: f32 = 2.0;
const PANEL_COLOR: Color = Color::srgba(0.06, 0.06, 0.08, 0.92);
const PANEL_BORDER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.4);
const CELL_BORDER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);

#[derive(Component, Clone, Default)]
pub struct ItemIcon;

#[derive(Resource, Default)]
pub struct InventoryOpen(pub bool);

pub fn inventory_closed(open: Res<InventoryOpen>) -> bool {
    !open.0
}

#[derive(Component)]
#[relationship(relationship_target = InventoryUi)]
pub struct InventoryUiOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = InventoryUiOf, linked_spawn)]
pub struct InventoryUi(Entity);

impl InventoryUi {
    pub fn entity(&self) -> Option<Entity> {
        (self.0 != Entity::PLACEHOLDER).then_some(self.0)
    }
}

pub struct InventoryUiPlugin;

impl Plugin for InventoryUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryOpen>()
            .add_observer(spawn_panel_for_grid)
            .add_observer(despawn_panel_with_grid)
            .add_observer(wire_icons)
            .add_observer(layout_new_icons)
            .add_observer(layout_on_repack);
    }
}

fn spawn_panel_for_grid(
    add: On<Add, InventoryGrid>,
    grids: Query<&InventoryGrid>,
    mut commands: Commands,
) {
    let container = add.event().entity;
    let Ok(grid) = grids.get(container) else {
        return;
    };
    let cells = grid.size();
    let size = cells.as_vec2() * CELL_PX;
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(size.x + 2.0 * PANEL_BORDER_PX),
                height: Val::Px(size.y + 2.0 * PANEL_BORDER_PX),
                border: UiRect::all(Val::Px(PANEL_BORDER_PX)),
                ..default()
            },
            UiTransform {
                translation: Val2::percent(-50.0, -50.0),
                ..UiTransform::IDENTITY
            },
            BackgroundColor(PANEL_COLOR),
            BorderColor::all(PANEL_BORDER_COLOR),
            GlobalZIndex(1),
            Visibility::Hidden,
            InventoryUiOf(container),
        ))
        .id();
    for y in 0..cells.y {
        for x in 0..cells.x {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x as f32 * CELL_PX),
                    top: Val::Px(y as f32 * CELL_PX),
                    width: Val::Px(CELL_PX),
                    height: Val::Px(CELL_PX),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(CELL_BORDER_COLOR),
                ChildOf(panel),
            ));
        }
    }
}

fn despawn_panel_with_grid(
    remove: On<Remove, InventoryGrid>,
    containers: Query<&InventoryUi>,
    mut commands: Commands,
) {
    let Ok(Some(panel)) = containers
        .get(remove.event().entity)
        .map(InventoryUi::entity)
    else {
        return;
    };
    if let Ok(mut panel) = commands.get_entity(panel) {
        panel.try_despawn();
    }
}

fn wire_icons(add: On<Add, ItemIcon>, mut commands: Commands) {
    commands
        .entity(add.event().entity)
        .observe(icon_drag_start)
        .observe(icon_drag)
        .observe(icon_drag_end);
}

fn icon_rect(node: &mut Node, transform: &mut UiTransform, origin: UVec2, footprint: UVec2) {
    node.position_type = PositionType::Absolute;
    node.left = Val::Px(origin.x as f32 * CELL_PX);
    node.top = Val::Px(origin.y as f32 * CELL_PX);
    node.width = Val::Px(footprint.x as f32 * CELL_PX);
    node.height = Val::Px(footprint.y as f32 * CELL_PX);
    *transform = UiTransform::IDENTITY;
}

fn layout_new_icons(
    add: On<Add, ViewOf>,
    mut icons: Query<(&ViewOf, &mut Node, &mut UiTransform), With<ItemIcon>>,
    models: Query<(&PackedAt, &ItemFootprint), With<Item>>,
) {
    let Ok((view_of, mut node, mut transform)) = icons.get_mut(add.event().entity) else {
        return;
    };
    let Ok((packed, footprint)) = models.get(view_of.0) else {
        return;
    };
    icon_rect(&mut node, &mut transform, packed.origin(), footprint.0);
}

fn layout_on_repack(
    insert: On<Insert, PackedAt>,
    models: Query<(&View, &PackedAt, &ItemFootprint), With<Item>>,
    mut icons: Query<(&mut Node, &mut UiTransform), With<ItemIcon>>,
) {
    let Ok((view, packed, footprint)) = models.get(insert.event().entity) else {
        return;
    };
    let Some(view) = view.entity() else {
        return;
    };
    let Ok((mut node, mut transform)) = icons.get_mut(view) else {
        return;
    };
    icon_rect(&mut node, &mut transform, packed.origin(), footprint.0);
}

fn icon_drag_start(
    drag: On<Pointer<DragStart>>,
    open: Res<InventoryOpen>,
    mut icons: Query<&mut ZIndex, With<ItemIcon>>,
) {
    if !open.0 || drag.event().button != PointerButton::Primary {
        return;
    }
    if let Ok(mut z) = icons.get_mut(drag.original_event_target()) {
        z.0 = 1;
    }
}

fn icon_drag(
    drag: On<Pointer<Drag>>,
    open: Res<InventoryOpen>,
    mut icons: Query<&mut UiTransform, With<ItemIcon>>,
) {
    if !open.0 || drag.event().button != PointerButton::Primary {
        return;
    }
    if let Ok(mut transform) = icons.get_mut(drag.original_event_target()) {
        let offset = drag.event().distance;
        transform.translation = Val2::px(offset.x, offset.y);
    }
}

fn icon_drag_end(
    drag: On<Pointer<DragEnd>>,
    open: Res<InventoryOpen>,
    mut icons: Query<(&ViewOf, &mut UiTransform, &mut ZIndex), With<ItemIcon>>,
    models: Query<(&ContainedBy, &PackedAt, &ItemFootprint), With<Item>>,
    containers: Query<(&InventoryGrid, &Contains)>,
    stored: Query<(&ItemState, &PackedAt, &ItemFootprint), With<Item>>,
    mut commands: Commands,
) {
    let Ok((view_of, mut transform, mut z)) = icons.get_mut(drag.original_event_target()) else {
        return;
    };
    *transform = UiTransform::IDENTITY;
    z.0 = 0;
    if !open.0 || drag.event().button != PointerButton::Primary {
        return;
    }
    let model = view_of.0;
    let Ok((contained, packed, footprint)) = models.get(model) else {
        return;
    };
    let Some((grid, occupied)) = occupied_cells(contained.container(), model, &containers, &stored)
    else {
        return;
    };
    let Some(target) = commit_drag(
        grid,
        &occupied,
        footprint.0,
        packed.origin(),
        drag.event().distance,
        CELL_PX,
    ) else {
        return;
    };
    if target != packed.origin() {
        commands.entity(model).repack_at(target);
    }
}

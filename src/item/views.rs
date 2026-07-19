//! The view side of the item system. Chrome comes from data
//! (`ItemRegistry` definitions), placement from structure (model or hand
//! socket), and behavior from components; per-component decoration lives in
//! its own module (`views/rusty.rs`) without the generic path knowing.

use bevy::prelude::*;

use super::{ContainedBy, Item, ItemRegistry, ItemState, ItemStateKind};

mod gun;
mod rusty;

pub use gun::GunViewPlugin;
pub use rusty::RustyViewPlugin;

/// Wires the view side: the registry of item definitions, the generic
/// rebuild-on-state-change observer, and one plugin per item or component
/// view.
pub struct ItemViewsPlugin;

impl Plugin for ItemViewsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemRegistry>()
            .add_observer(view_on_state_change)
            .add_plugins((GunViewPlugin, RustyViewPlugin));
    }
}

/// An attachment point for equipped items, spawned as a child of a holder.
/// Equipped views are parented here; the socket's `Transform` decides where
/// held items sit, so items need no per-item hand offset.
#[derive(Component, Clone, Default)]
pub struct HandSocket;

/// Points from a view entity to the item model it renders.
#[derive(Component)]
#[relationship(relationship_target = View)]
pub struct ViewOf(pub Entity);

/// The model's link to its current view entity. The bare `Entity` field
/// makes the relationship one-to-one: linking a new view automatically
/// unlinks the previous one. `linked_spawn` despawns the view together with
/// the model.
#[derive(Component)]
#[relationship_target(relationship = ViewOf, linked_spawn)]
pub struct View(Entity);

impl View {
    pub fn entity(&self) -> Option<Entity> {
        (self.0 != Entity::PLACEHOLDER).then_some(self.0)
    }
}

/// Rebuild the view when an item's state changes.
fn view_on_state_change(
    insert: On<Insert, ItemState>,
    models: Query<EntityRef, With<Item>>,
    registry: Res<ItemRegistry>,
    children: Query<&Children>,
    sockets: Query<(), With<HandSocket>>,
    mut commands: Commands,
) {
    refresh_view(
        insert.event().entity,
        &models,
        &registry,
        &children,
        &sockets,
        commands.reborrow(),
    );
}

/// The core of the model/view split: the view derives from the model.
/// Despawn the old view, look up the chrome for the model's key and state,
/// spawn it, and relate it back to the model. The model entity is never
/// touched, so accumulated components survive by construction.
///
/// Placement is structural: a grounded view is a child of the model itself
/// (the model's `Transform` positions it via propagation); an equipped view
/// is a child of the holder's [`HandSocket`] (or the holder root, with a
/// warning, if it has none); a state without chrome has no view at all.
pub(crate) fn refresh_view(
    model: Entity,
    models: &Query<EntityRef, With<Item>>,
    registry: &ItemRegistry,
    children: &Query<&Children>,
    sockets: &Query<(), With<HandSocket>>,
    mut commands: Commands,
) {
    let Ok(model_ref) = models.get(model) else {
        return;
    };

    commands.entity(model).despawn_related::<View>();

    let Some(kind) = model_ref.get::<ItemState>().map(ItemState::kind) else {
        return;
    };
    let Some(chrome) = registry.chrome(model_ref, kind) else {
        return;
    };

    let mut view = commands.spawn((
        Mesh3d(chrome.mesh.clone()),
        MeshMaterial3d(chrome.material.clone()),
        ViewOf(model),
    ));

    match kind {
        ItemStateKind::OnGround => {
            view.insert(ChildOf(model));
        }
        ItemStateKind::Equipped => {
            if let Some(contained) = model_ref.get::<ContainedBy>() {
                let holder = contained.container();
                view.insert(ChildOf(
                    socket_of(holder, children, sockets).unwrap_or_else(|| {
                        warn!("holder {holder} has no HandSocket; parenting view to its root");
                        holder
                    }),
                ));
            } else {
                // Axis contradiction; the dev checker reports it. Parenting
                // to the model keeps the view visible at the item's last
                // position instead of floating at the origin.
                warn!("equipped item {model} has no holder; parenting view to the model");
                view.insert(ChildOf(model));
            }
        }
        ItemStateKind::Stored => {}
    }
}

fn socket_of(
    holder: Entity,
    children: &Query<&Children>,
    sockets: &Query<(), With<HandSocket>>,
) -> Option<Entity> {
    children
        .get(holder)
        .ok()?
        .iter()
        .find(|&child| sockets.get(child).is_ok())
}

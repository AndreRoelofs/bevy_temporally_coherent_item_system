//! Placeholder

use bevy::prelude::*;
use bevy::scene::ScenePatch;

use super::{EquippedBy, Item, ItemRegistry, ItemState, OnGround, StoredIn};

mod gun;
mod inventory_ui;
mod rusty;
mod tint;

pub use gun::GunViewPlugin;
pub use inventory_ui::*;
pub use rusty::RustyViewPlugin;
pub use tint::*;

pub struct ItemViewsPlugin;

impl Plugin for ItemViewsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemRegistry>()
            .add_observer(view_on_ground)
            .add_observer(view_on_equip)
            .add_observer(view_on_store)
            .add_plugins((
                ViewTintPlugin,
                InventoryUiPlugin,
                GunViewPlugin,
                RustyViewPlugin,
            ));
    }
}

#[derive(Component, Clone, Default)]
pub struct HandSocket;

#[derive(Component)]
#[relationship(relationship_target = View)]
pub struct ViewOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = ViewOf, linked_spawn)]
pub struct View(Entity);

impl View {
    pub fn entity(&self) -> Option<Entity> {
        (self.0 != Entity::PLACEHOLDER).then_some(self.0)
    }
}

/// One observer per marker: a transition may briefly leave both the old and
/// the new marker on the item, so the fresh state comes from which insert
/// fired, never from inspecting the markers.
fn view_on_ground(insert: On<Insert, OnGround>, params: ViewRefreshParams, mut commands: Commands) {
    refresh_view(
        insert.event().entity,
        ItemState::OnGround,
        &params,
        commands.reborrow(),
    );
}

fn view_on_equip(
    insert: On<Insert, EquippedBy>,
    params: ViewRefreshParams,
    mut commands: Commands,
) {
    refresh_view(
        insert.event().entity,
        ItemState::Equipped,
        &params,
        commands.reborrow(),
    );
}

fn view_on_store(insert: On<Insert, StoredIn>, params: ViewRefreshParams, mut commands: Commands) {
    refresh_view(
        insert.event().entity,
        ItemState::Stored,
        &params,
        commands.reborrow(),
    );
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct ViewRefreshParams<'w, 's> {
    models: Query<'w, 's, EntityRef<'static>, With<Item>>,
    registry: Res<'w, ItemRegistry>,
    children: Query<'w, 's, &'static Children>,
    sockets: Query<'w, 's, (), With<HandSocket>>,
    panels: Query<'w, 's, &'static InventoryUi>,
}

pub(crate) fn refresh_view(
    model: Entity,
    state: ItemState,
    params: &ViewRefreshParams,
    mut commands: Commands,
) {
    let Ok(model_ref) = params.models.get(model) else {
        return;
    };

    commands.entity(model).despawn_related::<View>();

    let parent = match state {
        ItemState::OnGround => model,
        ItemState::Equipped => match model_ref.get::<EquippedBy>() {
            Some(equipped) => {
                let holder = equipped.holder();
                socket_of(holder, &params.children, &params.sockets).unwrap_or_else(|| {
                    warn!("holder {holder} has no HandSocket; parenting view to its root");
                    holder
                })
            }
            None => {
                warn!("equipped item {model} has no holder; parenting view to the model");
                model
            }
        },
        ItemState::Stored => {
            let Some(stored) = model_ref.get::<StoredIn>() else {
                warn!("stored item {model} has no container; skipping its view");
                return;
            };
            match params
                .panels
                .get(stored.container())
                .ok()
                .and_then(InventoryUi::entity)
            {
                Some(panel) => panel,
                None => return,
            }
        }
    };
    let Some(chrome) = params.registry.chrome(model_ref, state) else {
        return;
    };

    let chrome = chrome.clone();
    let spawned = commands.spawn_empty().id();
    commands.queue(move |world: &mut World| {
        world.resource_scope(|world, patches: Mut<Assets<ScenePatch>>| {
            let Some(patch) = patches.get(&chrome) else {
                error!("chrome scene patch missing for view {spawned}");
                return;
            };
            let Ok(mut view) = world.get_entity_mut(spawned) else {
                return;
            };
            if let Err(err) = patch.apply(&mut view) {
                error!("failed to apply chrome to view {spawned}: {err}");
            }
        });
    });
    commands
        .entity(spawned)
        .insert((ViewOf(model), ChildOf(parent)));
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

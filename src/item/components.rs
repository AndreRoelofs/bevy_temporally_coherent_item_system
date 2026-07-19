use bevy::prelude::*;

mod gun;
mod rusty;

pub use gun::*;
pub use rusty::*;

pub struct ItemComponentsPlugin;

impl Plugin for ItemComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::StatModifierRegistry>()
            .add_plugins((GunPlugin, RustyPlugin));
    }
}

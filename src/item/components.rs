use bevy::prelude::*;

mod firearm;
mod rusty;

pub use firearm::*;
pub use rusty::*;

pub struct ItemComponentsPlugin;

impl Plugin for ItemComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FirearmPlugin, RustyPlugin));
    }
}

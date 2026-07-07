use bevy::prelude::*;

mod building;
mod item;

pub use building::*;
pub use item::*;

pub fn run() {
    App::new().add_plugins(DefaultPlugins).run();
}

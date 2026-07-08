use bevy::prelude::*;

mod building;
mod camera;
mod game;
mod item;

pub use building::*;
pub use camera::*;
pub use item::*;

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(game::GamePlugin)
        .run();
}

# bevy_temporally_coherent_item_system

## Problem
OOP-based systems have separate classes and implementations for the same item in different states. For example the gun that exists on the ground is not the same class/entity as that gun that is equipped by the player. They're different implementations sharing the same mesh. This complicates visual and gameplay consistency in state transitions. Most games solve this by fading away the old object and replacing it with a new one. For example when there is a hurt animal on the ground and you cure it so that it becomes your companion, the old animal disappears and a new one is spawned in its place.

## Solution
A "pure-ECS" item system for Bevy 0.19 where each item is a single persistent entity whose appearance is driven entirely by its state.

Items are spawned once and never despawned/respawned. Instead, an item's `ItemState` (`OnGround`, `EquippedBy`, `StoredIn`) is the source of truth: a reactive system detects state changes and regenerates the item's scene *in place*. Stale components from the previous state are removed and the scene for the new state is applied. This keeps the entity identity stable across transitions. Namely - the gun that the player picked up is the same exact entity as the one in the player's hand

This means that you could have a system which places a "rusty" component on a gun that has been out too long, and that component is persisted after the player picks it up again, in turn affecting the gun's performance. A rusty gun can then have a system listening for the shots and destroy the gun after a bit of usage.

## Points of improvement
The goal of this project is to showcase how a bag-of-components system can be used in a data-driven game. To that end the project can only be completed once the official `bsn` file loader is created so that I can stop inlining the scenes, as you see in `src/item/scenes.rs`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

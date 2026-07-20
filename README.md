# bevy_temporally_coherent_item_system

## Problem

In many games, an item is a different object in each of its states: the gun
lying on the ground, the gun in the player's hand, and the gun in a chest are
separate spawns that merely share a mesh. Transitions then work by despawning
one object and spawning another, and anything that accumulated on the old
object - wear, enchantments, ownership, history - is lost unless it is
manually copied across.

While traditional game engines can and do preserve the accumulated components
between the item states, the more interesting question is what architecture
makes the persistent-item guarantee structural instead of a per-field copying
discipline.

## Solution: model/view split over three decomposed axes

Each item is one persistent model entity that is spawned once and kept track off through
different states (Equipped, OnGround, Stored). During normal gameplay, arbitrary
components are accumulated on the entity. When the item transitions to a different state,
the accumulated components are preserved. They can be removed only by separate systems.
This setup is especially useful for enabling rich mod support as third party code
can easily add completely new mechanics to the game that affect existing systems and items.

The state transitions do require their own components. That is where the view in the model/view
architecture comes in. For example we might want to display the gun as a mesh when it is held
in hands of the player, and as a 2D image when it's in the inventory.

## Controls

- Click to capture the mouse, `Esc` to release
- `WASD` + mouse to move and look
- Walk over a grounded item to stow it in the bag
- `Q` - equip the first stowed item (the current weapon slides into the bag)
- `G` - drop the equipped item just beyond pickup range

## Points of improvement

- The scenes in `src/item/scenes.rs` are still inlined `bsn!` blocks; once
  the official `.bsn` file loader lands, the `ItemRegistry` can map keys to
  scene assets instead of functions and item appearance becomes fully
  data-driven.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
